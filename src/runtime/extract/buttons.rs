use std::collections::{BTreeMap, BTreeSet};

use crate::format::{GspFile, ObjectGroup, PointRecord, read_u16, read_u32};
use crate::runtime::functions::evaluate_function_group_with_overrides;
use crate::runtime::scene::{ButtonAction, MovePointTarget, SceneButton, ScreenPoint, ScreenRect};

use super::points::editable_non_graph_parameter_name_for_group;
use super::{
    decode, find_indexed_path, payload_debug_source, try_decode_parameter_control_value_for_group,
};

#[derive(Clone)]
enum RawButtonAction {
    Link {
        href: String,
    },
    ShowHideVisibility {
        refs: Vec<usize>,
    },
    MovePoint {
        targets: Vec<(usize, Option<usize>)>,
        animated: bool,
    },
    AnimatePoint {
        point_group_ordinal: usize,
    },
    AnimatePoints {
        point_group_ordinals: Vec<usize>,
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
    pub(super) line_iteration_group_to_index: &'a BTreeMap<usize, usize>,
    pub(super) polygon_iteration_group_to_index: &'a BTreeMap<usize, usize>,
}

struct VisibilityTargets {
    button_indices: Vec<usize>,
    label_indices: Vec<usize>,
    image_indices: Vec<usize>,
    point_indices: Vec<usize>,
    line_indices: Vec<usize>,
    circle_indices: Vec<usize>,
    polygon_indices: Vec<usize>,
    line_iteration_indices: Vec<usize>,
    polygon_iteration_indices: Vec<usize>,
}

pub(super) fn collect_buttons(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
    lookups: ButtonIndexLookups<'_>,
) -> (Vec<SceneButton>, BTreeMap<usize, usize>) {
    let mut raw_buttons = Vec::<RawButton>::new();
    for group in groups {
        let kind = group.header.kind();
        if kind == crate::format::GroupKind::Point
            && !group.records.iter().any(|record| {
                matches!(
                    record.record_type,
                    crate::runtime::payload_consts::RECORD_POINT_F64_PAIR
                        | crate::runtime::payload_consts::RECORD_FUNCTION_EXPR_PAYLOAD
                )
            })
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
            .find(|record| {
                record.record_type == crate::runtime::payload_consts::RECORD_ACTION_BUTTON_PAYLOAD
            })
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
                    let point_group_ordinals = animate_point_refs(&refs);
                    match point_group_ordinals.as_slice() {
                        [] => None,
                        [point_group_ordinal] => Some(RawButtonAction::AnimatePoint {
                            point_group_ordinal: *point_group_ordinal,
                        }),
                        _ => Some(RawButtonAction::AnimatePoints {
                            point_group_ordinals,
                        }),
                    }
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
                    let targets = refs
                        .chunks(2)
                        .filter_map(|pair| {
                            pair.first()
                                .copied()
                                .map(|point| (point, pair.get(1).copied()))
                        })
                        .collect::<Vec<_>>();
                    (!targets.is_empty()).then_some(RawButtonAction::MovePoint {
                        targets,
                        animated: action_kind_hi == 0,
                    })
                }
                (0..=1, 0..=7) => Some(RawButtonAction::ShowHideVisibility { refs }),
                _ => None,
            };
        let Some(action) = action else {
            continue;
        };

        let Some(anchor) = decode::decode_action_button_anchor(file, groups, group, anchors) else {
            continue;
        };
        let Some(text) = decode::decode_action_button_text(file, group) else {
            continue;
        };

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

    let parameters_by_ordinal = collect_button_parameters(file, groups);
    let parameter_values = parameters_by_ordinal
        .values()
        .map(|parameter| (parameter.name.clone(), parameter.value))
        .collect::<BTreeMap<_, _>>();
    let raw_buttons = raw_buttons
        .into_iter()
        .filter(|button| {
            raw_button_action_is_exportable(&button.action, lookups, &parameters_by_ordinal)
        })
        .collect::<Vec<_>>();
    let button_index_by_ordinal = raw_buttons
        .iter()
        .enumerate()
        .map(|(button_index, button)| (button.group_ordinal, button_index))
        .collect::<BTreeMap<usize, usize>>();

    let buttons = raw_buttons
        .into_iter()
        .filter_map(|button| {
            let action = match button.action {
                RawButtonAction::Link { href } => ButtonAction::Link { href },
                RawButtonAction::ShowHideVisibility { refs } => {
                    let VisibilityTargets {
                        button_indices,
                        label_indices,
                        image_indices,
                        point_indices,
                        line_indices,
                        circle_indices,
                        polygon_indices,
                        line_iteration_indices,
                        polygon_iteration_indices,
                    } = resolve_visibility_targets(&refs, &button_index_by_ordinal, lookups);
                    ButtonAction::ShowHideVisibility {
                        button_indices,
                        label_indices,
                        image_indices,
                        point_indices,
                        line_indices,
                        circle_indices,
                        polygon_indices,
                        line_iteration_indices,
                        polygon_iteration_indices,
                    }
                }
                RawButtonAction::MovePoint { targets, animated } => {
                    let point_targets = resolve_move_point_targets(&targets, lookups);
                    if point_targets.len() == 1 {
                        let target = point_targets[0].clone();
                        ButtonAction::MovePoint {
                            point_index: target.point_index,
                            target_point_index: target.target_point_index,
                        }
                    } else if !point_targets.is_empty() {
                        ButtonAction::MovePoints {
                            targets: point_targets,
                        }
                    } else if let Some((point_group_ordinal, target_group_ordinal)) =
                        targets.first()
                        && let Some(parameter) = parameters_by_ordinal.get(point_group_ordinal)
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
                }
                RawButtonAction::AnimatePoint {
                    point_group_ordinal,
                } => ButtonAction::AnimatePoint {
                    point_index: lookups
                        .group_to_point_index
                        .get(point_group_ordinal.checked_sub(1)?)
                        .copied()
                        .flatten()?,
                },
                RawButtonAction::AnimatePoints {
                    point_group_ordinals,
                } => {
                    let point_indices = point_group_ordinals
                        .into_iter()
                        .filter_map(|ordinal| {
                            lookups
                                .group_to_point_index
                                .get(ordinal.checked_sub(1)?)
                                .copied()
                                .flatten()
                        })
                        .collect::<Vec<_>>();
                    if point_indices.is_empty() {
                        return None;
                    }
                    ButtonAction::AnimatePoints { point_indices }
                }
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

fn raw_button_action_is_exportable(
    action: &RawButtonAction,
    lookups: ButtonIndexLookups<'_>,
    parameters_by_ordinal: &BTreeMap<usize, ButtonParameter>,
) -> bool {
    match action {
        RawButtonAction::Link { .. }
        | RawButtonAction::ShowHideVisibility { .. }
        | RawButtonAction::PlayFunction { .. }
        | RawButtonAction::Sequence { .. } => true,
        RawButtonAction::MovePoint { targets, .. } => {
            !resolve_move_point_targets(targets, lookups).is_empty()
                || targets
                    .first()
                    .is_some_and(|(ordinal, _)| parameters_by_ordinal.contains_key(ordinal))
        }
        RawButtonAction::AnimatePoint {
            point_group_ordinal,
        } => resolve_point_index(*point_group_ordinal, lookups).is_some(),
        RawButtonAction::AnimatePoints {
            point_group_ordinals,
        } => point_group_ordinals
            .iter()
            .any(|ordinal| resolve_point_index(*ordinal, lookups).is_some()),
        RawButtonAction::ScrollPoint {
            point_group_ordinal,
        }
        | RawButtonAction::FocusPoint {
            point_group_ordinal,
        } => resolve_point_index(*point_group_ordinal, lookups).is_some(),
    }
}

fn animate_point_refs(refs: &[usize]) -> Vec<usize> {
    refs.to_vec()
}

fn resolve_move_point_targets(
    targets: &[(usize, Option<usize>)],
    lookups: ButtonIndexLookups<'_>,
) -> Vec<MovePointTarget> {
    targets
        .iter()
        .filter_map(|(point_group_ordinal, target_group_ordinal)| {
            let point_index = resolve_point_index(*point_group_ordinal, lookups)?;
            let target_point_index =
                target_group_ordinal.and_then(|ordinal| resolve_point_index(ordinal, lookups));
            Some(MovePointTarget {
                point_index,
                target_point_index,
            })
        })
        .collect()
}

fn resolve_point_index(group_ordinal: usize, lookups: ButtonIndexLookups<'_>) -> Option<usize> {
    lookups
        .group_to_point_index
        .get(group_ordinal.checked_sub(1)?)
        .copied()
        .flatten()
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
    let line_iteration_indices = refs
        .iter()
        .filter_map(|ordinal| lookups.line_iteration_group_to_index.get(ordinal).copied())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let polygon_iteration_indices = refs
        .iter()
        .filter_map(|ordinal| {
            lookups
                .polygon_iteration_group_to_index
                .get(ordinal)
                .copied()
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
        line_iteration_indices,
        polygon_iteration_indices,
    }
}
