use std::collections::{BTreeMap, BTreeSet};

use crate::format::{GspFile, ObjectGroup, PointRecord, read_u16, read_u32};
use crate::runtime::scene::{ButtonAction, SceneButton, ScreenPoint, ScreenRect};

use super::{decode, find_indexed_path};

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
    MovePoint {
        point_group_ordinal: usize,
        target_group_ordinal: Option<usize>,
    },
    AnimatePoint {
        point_group_ordinal: usize,
    },
    ScrollPoint {
        point_group_ordinal: usize,
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
    action: RawButtonAction,
}

pub(super) fn collect_buttons(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
    group_to_point_index: &[Option<usize>],
    line_group_to_index: &[Option<usize>],
    circle_group_to_index: &[Option<usize>],
    polygon_group_to_index: &[Option<usize>],
) -> Vec<SceneButton> {
    let button_label_groups = groups
        .iter()
        .filter(|group| (group.header.class_id & 0xffff) == 73)
        .filter_map(|group| {
            let path = find_indexed_path(file, group)?;
            let button_ordinal = *path.refs.first()?;
            let anchor = decode::decode_label_anchor(file, group, anchors)?;
            Some((button_ordinal, anchor))
        })
        .collect::<BTreeMap<usize, PointRecord>>();

    let mut raw_buttons = Vec::<RawButton>::new();
    for group in groups {
        let kind = group.header.class_id & 0xffff;
        if kind == 0
            && !group
                .records
                .iter()
                .any(|record| matches!(record.record_type, 0x0899 | 0x0907))
            && let Some(href) = decode::decode_link_button_url(file, group)
            && let Some((x, y, width, height)) = decode::decode_bbox_rect_raw(file, group)
        {
            raw_buttons.push(RawButton {
                group_ordinal: group.ordinal,
                text: decode::decode_label_name_raw(file, group)
                    .filter(|label| !label.trim().is_empty())
                    .unwrap_or_else(|| href.clone()),
                anchor: ScreenPoint { x, y },
                rect: Some(ScreenRect { width, height }),
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
        let action = match (action_kind_lo, action_kind_hi) {
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
            (7, 0) => Some(RawButtonAction::Sequence {
                button_group_ordinals: refs,
                interval_ms: read_u32(action_payload, 16),
            }),
            (3, 1) => refs
                .first()
                .copied()
                .map(|point_group_ordinal| RawButtonAction::MovePoint {
                    point_group_ordinal,
                    target_group_ordinal: refs.get(1).copied(),
                }),
            (0, 7) => Some(RawButtonAction::ToggleVisibility { refs }),
            (1, 3) => Some(RawButtonAction::SetVisibility {
                refs,
                visible: true,
            }),
            (0, 3) => Some(RawButtonAction::SetVisibility {
                refs,
                visible: false,
            }),
            _ => None,
        };
        let Some(action) = action else {
            continue;
        };

        let anchor = button_label_groups
            .get(&group.ordinal)
            .cloned()
            .or_else(|| decode::decode_button_screen_anchor(file, group))
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
            action,
        });
    }

    let button_index_by_ordinal = raw_buttons
        .iter()
        .enumerate()
        .map(|(button_index, button)| (button.group_ordinal, button_index))
        .collect::<BTreeMap<usize, usize>>();

    raw_buttons
        .into_iter()
        .filter_map(|button| {
            let action = match button.action {
                RawButtonAction::Link { href } => ButtonAction::Link { href },
                RawButtonAction::ToggleVisibility { refs } => {
                    let (point_indices, line_indices, circle_indices, polygon_indices) =
                        resolve_visibility_targets(
                            &refs,
                            group_to_point_index,
                            line_group_to_index,
                            circle_group_to_index,
                            polygon_group_to_index,
                        );
                    ButtonAction::ToggleVisibility {
                        point_indices,
                        line_indices,
                        circle_indices,
                        polygon_indices,
                    }
                }
                RawButtonAction::SetVisibility { refs, visible } => {
                    let (point_indices, line_indices, circle_indices, polygon_indices) =
                        resolve_visibility_targets(
                            &refs,
                            group_to_point_index,
                            line_group_to_index,
                            circle_group_to_index,
                            polygon_group_to_index,
                        );
                    ButtonAction::SetVisibility {
                        visible,
                        point_indices,
                        line_indices,
                        circle_indices,
                        polygon_indices,
                    }
                }
                RawButtonAction::MovePoint {
                    point_group_ordinal,
                    target_group_ordinal,
                } => ButtonAction::MovePoint {
                    point_index: group_to_point_index
                        .get(point_group_ordinal.checked_sub(1)?)
                        .copied()
                        .flatten()?,
                    target_point_index: target_group_ordinal.and_then(|ordinal| {
                        group_to_point_index
                            .get(ordinal.checked_sub(1)?)
                            .copied()
                            .flatten()
                    }),
                },
                RawButtonAction::AnimatePoint {
                    point_group_ordinal,
                } => ButtonAction::AnimatePoint {
                    point_index: group_to_point_index
                        .get(point_group_ordinal.checked_sub(1)?)
                        .copied()
                        .flatten()?,
                },
                RawButtonAction::ScrollPoint {
                    point_group_ordinal,
                } => ButtonAction::ScrollPoint {
                    point_index: group_to_point_index
                        .get(point_group_ordinal.checked_sub(1)?)
                        .copied()
                        .flatten()?,
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
                action,
            })
        })
        .collect()
}

fn resolve_visibility_targets(
    refs: &[usize],
    group_to_point_index: &[Option<usize>],
    line_group_to_index: &[Option<usize>],
    circle_group_to_index: &[Option<usize>],
    polygon_group_to_index: &[Option<usize>],
) -> (Vec<usize>, Vec<usize>, Vec<usize>, Vec<usize>) {
    let point_indices = refs
        .iter()
        .filter_map(|ordinal| {
            group_to_point_index
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
            line_group_to_index
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
            circle_group_to_index
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
            polygon_group_to_index
                .get(ordinal.checked_sub(1)?)
                .copied()
                .flatten()
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    (point_indices, line_indices, circle_indices, polygon_indices)
}
