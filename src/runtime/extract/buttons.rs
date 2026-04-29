use std::collections::{BTreeMap, BTreeSet};

use crate::format::{GspFile, ObjectGroup, PointRecord, read_u16, read_u32};
use crate::runtime::functions::evaluate_function_group_with_overrides;
use crate::runtime::scene::{ButtonAction, SceneButton, ScreenPoint, ScreenRect};

use super::points::editable_non_graph_parameter_name_for_group;
use super::{
    decode, find_indexed_path, payload_debug_source, try_decode_parameter_control_value_for_group,
};

#[derive(Clone)]
enum RawButtonAction {
    Link {
        href: String,
    },
    ToggleVisibility {
        refs: Vec<usize>,
    },
    SetVisibility {
        refs: Vec<usize>,
        visible: bool,
    },
    ShowHideVisibility {
        refs: Vec<usize>,
    },
    MovePoint {
        point_group_ordinal: usize,
        target_group_ordinal: Option<usize>,
        animated: bool,
    },
    AnimatePoint {
        point_group_ordinal: usize,
    },
    ScrollPoint {
        point_group_ordinal: usize,
    },
    FocusPoint {
        point_group_ordinal: usize,
    },
    PlayFunction {
        function_group_ordinal: usize,
    },
    Sequence {
        button_group_ordinals: Vec<usize>,
        interval_ms: u32,
    },
}

#[derive(Clone)]
struct RawButton {
    group_ordinal: usize,
    text: String,
    anchor: ScreenPoint,
    rect: Option<ScreenRect>,
    visible: bool,
    action: RawButtonAction,
}

struct ButtonParameter {
    name: String,
    value: f64,
}

#[derive(Clone, Copy)]
pub(super) struct ButtonIndexLookups<'a> {
    pub(super) label_group_to_index: &'a BTreeMap<usize, usize>,
    pub(super) image_group_to_index: &'a BTreeMap<usize, usize>,
    pub(super) group_to_point_index: &'a [Option<usize>],
    pub(super) line_group_to_index: &'a [Option<usize>],
    pub(super) circle_group_to_index: &'a [Option<usize>],
    pub(super) polygon_group_to_index: &'a [Option<usize>],
}

struct VisibilityTargets {
    button_indices: Vec<usize>,
    label_indices: Vec<usize>,
    image_indices: Vec<usize>,
    point_indices: Vec<usize>,
    line_indices: Vec<usize>,
    circle_indices: Vec<usize>,
    polygon_indices: Vec<usize>,
}

pub(super) fn collect_buttons(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
    lookups: ButtonIndexLookups<'_>,
) -> (Vec<SceneButton>, BTreeMap<usize, usize>) {
    let button_label_groups = groups
        .iter()
        .filter(|group| (group.header.kind()) == crate::format::GroupKind::ButtonLabel)
        .filter_map(|group| {
            let path = find_indexed_path(file, group)?;
            let button_ordinal = *path.refs.first()?;
            let anchor = decode::decode_label_anchor(file, group, anchors)?;
            Some((button_ordinal, anchor))
        })
        .collect::<BTreeMap<usize, PointRecord>>();

    let mut raw_buttons = Vec::<RawButton>::new();
    for group in groups {
        let kind = group.header.kind();
        if kind == crate::format::GroupKind::Point
            && !group
                .records
                .iter()
                .any(|record| matches!(record.record_type, 0x0899 | 0x0907))
            && let Some(href) = decode::try_decode_link_button_url(file, group)
                .ok()
                .flatten()
            && let Some((x, y, width, height)) =
                decode::try_decode_bbox_rect_raw(file, group).ok().flatten()
        {
            raw_buttons.push(RawButton {
                group_ordinal: group.ordinal,
                text: decode::decode_label_name_raw(file, group)
                    .filter(|label| !label.trim().is_empty())
                    .unwrap_or_else(|| href.clone()),
                anchor: ScreenPoint { x, y },
                rect: Some(ScreenRect { width, height }),
                visible: !group.header.is_hidden(),
                action: RawButtonAction::Link { href },
            });
            continue;
        }

        if !decode::is_action_button_group(group) {
            continue;
        }

        let payload = group
            .records
            .iter()
            .find(|record| record.record_type == 0x0906)
            .map(|record| record.payload(&file.data));
        let action_payload = if let Some(payload) = payload {
            payload
        } else {
            continue;
        };
        if action_payload.len() < 16 {
            continue;
        }

        let refs = find_indexed_path(file, group)
            .map(|path| path.refs)
            .unwrap_or_default();
        let action_kind_lo = read_u16(action_payload, 12);
        let action_kind_hi = read_u16(action_payload, 14);
        let action =
            match (action_kind_lo, action_kind_hi) {
                (2, 0) => {
                    refs.first()
                        .copied()
                        .map(|point_group_ordinal| RawButtonAction::AnimatePoint {
                            point_group_ordinal,
                        })
                }
                (4, 0) => {
                    refs.first()
                        .copied()
                        .map(|point_group_ordinal| RawButtonAction::ScrollPoint {
                            point_group_ordinal,
                        })
                }
                (4, 1) => {
                    refs.first()
                        .copied()
                        .map(|point_group_ordinal| RawButtonAction::FocusPoint {
                            point_group_ordinal,
                        })
                }
                (8, 0) => refs.first().copied().map(|function_group_ordinal| {
                    RawButtonAction::PlayFunction {
                        function_group_ordinal,
                    }
                }),
                // Legacy sample payloads encode the same chained-button sequence family under
                // many (7, *) variants while keeping the timing payload layout unchanged.
                (7, _) => Some(RawButtonAction::Sequence {
                    button_group_ordinals: refs,
                    interval_ms: read_u32(action_payload, 16),
                }),
                (3, 0..=3) => {
                    refs.first()
                        .copied()
                        .map(|point_group_ordinal| RawButtonAction::MovePoint {
                            point_group_ordinal,
                            target_group_ordinal: refs.get(1).copied(),
                            animated: action_kind_hi == 0,
                        })
                }
                (0, 7) => Some(RawButtonAction::ToggleVisibility { refs }),
                (1, 7) => Some(RawButtonAction::ShowHideVisibility { refs }),
                (1, 0..=6) => Some(RawButtonAction::SetVisibility {
                    refs,
                    visible: true,
                }),
                (0, 0..=6) => Some(RawButtonAction::SetVisibility {
                    refs,
                    visible: false,
                }),
                _ => None,
            };
        let Some(action) = action else {
            continue;
        };

        let anchor = decode::decode_button_screen_anchor(file, group)
            .or_else(|| button_label_groups.get(&group.ordinal).cloned())
            .unwrap_or(PointRecord { x: 24.0, y: 24.0 });
        let text = decode::decode_label_name_raw(file, group)
            .filter(|label| !label.trim().is_empty())
            .unwrap_or_else(|| "按钮".to_string());

        raw_buttons.push(RawButton {
            group_ordinal: group.ordinal,
            text,
            anchor: ScreenPoint {
                x: anchor.x,
                y: anchor.y,
            },
            rect: None,
            visible: !group.header.is_hidden(),
            action,
        });
    }

    let button_index_by_ordinal = raw_buttons
        .iter()
        .enumerate()
        .map(|(button_index, button)| (button.group_ordinal, button_index))
        .collect::<BTreeMap<usize, usize>>();
    let parameters_by_ordinal = collect_button_parameters(file, groups);
    let parameter_values = parameters_by_ordinal
        .values()
        .map(|parameter| (parameter.name.clone(), parameter.value))
        .collect::<BTreeMap<_, _>>();

    let buttons = raw_buttons
        .into_iter()
        .filter_map(|button| {
            let action = match button.action {
                RawButtonAction::Link { href } => ButtonAction::Link { href },
                RawButtonAction::ToggleVisibility { refs } => {
                    let VisibilityTargets {
                        button_indices,
                        label_indices,
                        image_indices,
                        point_indices,
                        line_indices,
                        circle_indices,
                        polygon_indices,
                    } = resolve_visibility_targets(&refs, &button_index_by_ordinal, lookups);
                    ButtonAction::ToggleVisibility {
                        button_indices,
                        label_indices,
                        image_indices,
                        point_indices,
                        line_indices,
                        circle_indices,
                        polygon_indices,
                    }
                }
                RawButtonAction::SetVisibility { refs, visible } => {
                    let VisibilityTargets {
                        button_indices,
                        label_indices,
                        image_indices,
                        point_indices,
                        line_indices,
                        circle_indices,
                        polygon_indices,
                    } = resolve_visibility_targets(&refs, &button_index_by_ordinal, lookups);
                    ButtonAction::SetVisibility {
                        visible,
                        button_indices,
                        label_indices,
                        image_indices,
                        point_indices,
                        line_indices,
                        circle_indices,
                        polygon_indices,
                    }
                }
                RawButtonAction::ShowHideVisibility { refs } => {
                    let VisibilityTargets {
                        button_indices,
                        label_indices,
                        image_indices,
                        point_indices,
                        line_indices,
                        circle_indices,
                        polygon_indices,
                    } = resolve_visibility_targets(&refs, &button_index_by_ordinal, lookups);
                    ButtonAction::ShowHideVisibility {
                        button_indices,
                        label_indices,
                        image_indices,
                        point_indices,
                        line_indices,
                        circle_indices,
                        polygon_indices,
                    }
                }
                RawButtonAction::MovePoint {
                    point_group_ordinal,
                    target_group_ordinal,
                    animated,
                } => {
                    if let Some(point_index) = lookups
                        .group_to_point_index
                        .get(point_group_ordinal.checked_sub(1)?)
                        .copied()
                        .flatten()
                    {
                        ButtonAction::MovePoint {
                            point_index,
                            target_point_index: if let Some(ordinal) = target_group_ordinal {
                                lookups
                                    .group_to_point_index
                                    .get(ordinal.checked_sub(1)?)
                                    .copied()
                                    .flatten()
                            } else {
                                None
                            },
                        }
                    } else if let Some(parameter) = parameters_by_ordinal.get(&point_group_ordinal)
                    {
                        let target_value = target_group_ordinal
                            .and_then(|ordinal| {
                                resolve_parameter_button_target_value(
                                    file,
                                    groups,
                                    &parameters_by_ordinal,
                                    &parameter_values,
                                    ordinal,
                                )
                            })
                            .unwrap_or(parameter.value);
                        if animated {
                            ButtonAction::AnimateParameter {
                                parameter_name: parameter.name.clone(),
                                target_value,
                            }
                        } else {
                            ButtonAction::SetParameter {
                                parameter_name: parameter.name.clone(),
                                value: target_value,
                            }
                        }
                    } else {
                        return None;
                    }
                },
                RawButtonAction::AnimatePoint {
                    point_group_ordinal,
                } => ButtonAction::AnimatePoint {
                    point_index: lookups
                        .group_to_point_index
                        .get(point_group_ordinal.checked_sub(1)?)
                        .copied()
                        .flatten()?,
                },
                RawButtonAction::ScrollPoint {
                    point_group_ordinal,
                } => ButtonAction::ScrollPoint {
                    point_index: lookups
                        .group_to_point_index
                        .get(point_group_ordinal.checked_sub(1)?)
                        .copied()
                        .flatten()?,
                },
                RawButtonAction::FocusPoint {
                    point_group_ordinal,
                } => ButtonAction::FocusPoint {
                    point_index: lookups
                        .group_to_point_index
                        .get(point_group_ordinal.checked_sub(1)?)
                        .copied()
                        .flatten()?,
                },
                RawButtonAction::PlayFunction {
                    function_group_ordinal,
                } => ButtonAction::PlayFunction {
                    function_key: function_group_ordinal,
                },
                RawButtonAction::Sequence {
                    button_group_ordinals,
                    interval_ms,
                } => ButtonAction::Sequence {
                    button_indices: button_group_ordinals
                        .into_iter()
                        .filter_map(|ordinal| button_index_by_ordinal.get(&ordinal).copied())
                        .collect(),
                    interval_ms,
                },
            };

            Some(SceneButton {
                text: button.text,
                anchor: button.anchor,
                rect: button.rect,
                visible: button.visible,
                action,
                debug: groups
                    .get(button.group_ordinal.saturating_sub(1))
                    .map(payload_debug_source),
            })
        })
        .collect::<Vec<_>>();
    (buttons, button_index_by_ordinal)
}

fn collect_button_parameters(
    file: &GspFile,
    groups: &[ObjectGroup],
) -> BTreeMap<usize, ButtonParameter> {
    groups
        .iter()
        .filter(|group| decode::is_parameter_control_group(group))
        .filter_map(|group| {
            let name = editable_non_graph_parameter_name_for_group(file, groups, group)?;
            let value = try_decode_parameter_control_value_for_group(file, groups, group).ok()?;
            Some((group.ordinal, ButtonParameter { name, value }))
        })
        .collect()
}

fn resolve_parameter_button_target_value(
    file: &GspFile,
    groups: &[ObjectGroup],
    parameters_by_ordinal: &BTreeMap<usize, ButtonParameter>,
    parameter_values: &BTreeMap<String, f64>,
    target_group_ordinal: usize,
) -> Option<f64> {
    if let Some(parameter) = parameters_by_ordinal.get(&target_group_ordinal) {
        return Some(parameter.value);
    }
    let group = groups.get(target_group_ordinal.checked_sub(1)?)?;
    evaluate_function_group_with_overrides(file, groups, group, parameter_values)
}

fn resolve_visibility_targets(
    refs: &[usize],
    button_index_by_ordinal: &BTreeMap<usize, usize>,
    lookups: ButtonIndexLookups<'_>,
) -> VisibilityTargets {
    let button_indices = refs
        .iter()
        .filter_map(|ordinal| button_index_by_ordinal.get(ordinal).copied())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let label_indices = refs
        .iter()
        .filter_map(|ordinal| lookups.label_group_to_index.get(ordinal).copied())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let image_indices = refs
        .iter()
        .filter_map(|ordinal| lookups.image_group_to_index.get(ordinal).copied())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let point_indices = refs
        .iter()
        .filter_map(|ordinal| {
            lookups
                .group_to_point_index
                .get(ordinal.checked_sub(1)?)
                .copied()
                .flatten()
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let line_indices = refs
        .iter()
        .filter_map(|ordinal| {
            lookups
                .line_group_to_index
                .get(ordinal.checked_sub(1)?)
                .copied()
                .flatten()
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let circle_indices = refs
        .iter()
        .filter_map(|ordinal| {
            lookups
                .circle_group_to_index
                .get(ordinal.checked_sub(1)?)
                .copied()
                .flatten()
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let polygon_indices = refs
        .iter()
        .filter_map(|ordinal| {
            lookups
                .polygon_group_to_index
                .get(ordinal.checked_sub(1)?)
                .copied()
                .flatten()
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    VisibilityTargets {
        button_indices,
        label_indices,
        image_indices,
        point_indices,
        line_indices,
        circle_indices,
        polygon_indices,
    }
}
