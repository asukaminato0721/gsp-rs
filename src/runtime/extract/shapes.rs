use std::collections::{BTreeMap, BTreeSet};

use super::decode::{
    decode_bbox_anchor_raw, decode_label_name, decode_transform_anchor_raw, find_indexed_path,
};
use super::*;
use crate::runtime::extract::points::decode_translated_point_constraint;
use crate::runtime::functions::{
    decode_function_expr, decode_function_plot_descriptor, evaluate_expr_with_parameters,
};
use crate::runtime::geometry::{
    color_from_style, fill_color_from_styles, has_distinct_points, reflect_across_line,
    rotate_around, scale_around, to_raw_from_world,
};
use crate::runtime::scene::{
    LineBinding, LineIterationFamily, PolygonIterationFamily, ShapeBinding,
};

pub(super) fn collect_raw_object_anchors(
    file: &GspFile,
    groups: &[ObjectGroup],
    point_map: &[Option<PointRecord>],
    graph: Option<&GraphTransform>,
) -> Vec<Option<PointRecord>> {
    let mut anchors = Vec::with_capacity(groups.len());
    for (index, group) in groups.iter().enumerate() {
        let anchor = if let Some(point) = point_map.get(index).and_then(|point| point.clone()) {
            Some(point)
        } else if let Some(anchor) =
            decode_point_constraint_anchor(file, groups, group, &anchors, graph)
        {
            Some(anchor)
        } else if let Some(anchor) = decode_point_on_ray_anchor_raw(file, groups, group, &anchors) {
            Some(anchor)
        } else if let Some(anchor) = decode_translated_point_anchor_raw(file, group, &anchors) {
            Some(anchor)
        } else if let Some(anchor) =
            decode_parameter_rotation_anchor_raw(file, groups, group, &anchors)
        {
            Some(anchor)
        } else if let Some(anchor) = decode_transform_anchor_raw(file, group, &anchors) {
            Some(anchor)
        } else if let Some(anchor) =
            decode_point_pair_translation_anchor_raw(file, groups, group, &anchors)
        {
            Some(anchor)
        } else if let Some(anchor) = decode_reflection_anchor_raw(file, groups, group, &anchors) {
            Some(anchor)
        } else if let Some(anchor) =
            decode_regular_polygon_vertex_anchor_raw(file, groups, group, &anchors)
        {
            Some(anchor)
        } else if let Some(anchor) = decode_offset_anchor_raw(file, group, &anchors) {
            Some(anchor)
        } else if let Some(anchor) =
            decode_parameter_controlled_anchor_raw(file, groups, group, &anchors)
        {
            Some(anchor)
        } else if let Some(anchor) = decode_bbox_anchor_raw(file, group) {
            Some(anchor)
        } else {
            find_indexed_path(file, group).and_then(|path| {
                path.refs.iter().rev().find_map(|object_ref| {
                    anchors.get(object_ref.saturating_sub(1)).cloned().flatten()
                })
            })
        };
        anchors.push(anchor);
    }
    anchors
}

pub(super) fn collect_line_shapes(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
    kinds: &[u32],
    fallback_generic: bool,
) -> Vec<LineShape> {
    groups
        .iter()
        .filter(|group| {
            let kind = group.header.class_id & 0xffff;
            kinds.contains(&kind)
                || (fallback_generic
                    && matches!(kind, 2 | 5 | 6 | 7)
                    && find_indexed_path(file, group)
                        .map(|path| path.refs.len() == 2)
                        .unwrap_or(false))
        })
        .filter_map(|group| {
            let path = find_indexed_path(file, group)?;
            let points = path
                .refs
                .iter()
                .filter_map(|object_ref| {
                    anchors.get(object_ref.saturating_sub(1)).cloned().flatten()
                })
                .collect::<Vec<_>>();
            (points.len() >= 2 && has_distinct_points(&points)).then_some(LineShape {
                points,
                color: if fallback_generic && !kinds.contains(&(group.header.class_id & 0xffff)) {
                    [40, 40, 40, 255]
                } else {
                    color_from_style(group.header.style_b)
                },
                dashed: (group.header.class_id & 0xffff) == 58,
                binding: None,
            })
        })
        .collect()
}

pub(super) fn collect_bound_line_shapes(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
    kind: u32,
) -> Vec<LineShape> {
    groups
        .iter()
        .filter(|group| (group.header.class_id & 0xffff) == kind)
        .filter_map(|group| {
            let path = find_indexed_path(file, group)?;
            if path.refs.len() != 2 {
                return None;
            }
            let start_group_index = path.refs[0].checked_sub(1)?;
            let end_group_index = path.refs[1].checked_sub(1)?;
            let start = anchors.get(start_group_index)?.clone()?;
            let end = anchors.get(end_group_index)?.clone()?;
            has_distinct_points(&[start.clone(), end.clone()]).then_some(LineShape {
                points: vec![start, end],
                color: color_from_style(group.header.style_b),
                dashed: false,
                binding: Some(match kind {
                    63 => LineBinding::Line {
                        start_index: start_group_index,
                        end_index: end_group_index,
                    },
                    64 => LineBinding::Ray {
                        start_index: start_group_index,
                        end_index: end_group_index,
                    },
                    _ => return None,
                }),
            })
        })
        .collect()
}

pub(super) fn collect_polygon_shapes(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
    kinds: &[u32],
) -> Vec<PolygonShape> {
    groups
        .iter()
        .filter(|group| kinds.contains(&(group.header.class_id & 0xffff)))
        .filter_map(|group| {
            let path = find_indexed_path(file, group)?;
            let points = path
                .refs
                .iter()
                .filter_map(|object_ref| {
                    anchors.get(object_ref.saturating_sub(1)).cloned().flatten()
                })
                .collect::<Vec<_>>();
            (points.len() >= 3).then_some(PolygonShape {
                points,
                color: fill_color_from_styles(group.header.style_b, group.header.style_c),
                binding: None,
            })
        })
        .collect()
}

pub(super) fn collect_circle_shapes(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
) -> Vec<CircleShape> {
    groups
        .iter()
        .filter(|group| (group.header.class_id & 0xffff) == 3)
        .filter_map(|group| {
            let path = find_indexed_path(file, group)?;
            if path.refs.len() != 2 {
                return None;
            }
            let center = anchors.get(path.refs[0].saturating_sub(1))?.clone()?;
            let radius_point = anchors.get(path.refs[1].saturating_sub(1))?.clone()?;
            Some(CircleShape {
                center,
                radius_point,
                color: color_from_style(group.header.style_b),
                binding: None,
            })
        })
        .collect()
}

pub(super) fn collect_rotated_line_shapes(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
) -> Vec<LineShape> {
    groups
        .iter()
        .filter(|group| (group.header.class_id & 0xffff) == 29)
        .filter_map(|group| {
            let binding = decode_parameter_rotation_binding(file, groups, group)?;
            let path = find_indexed_path(file, group)?;
            let source_group = groups.get(path.refs.first()?.checked_sub(1)?)?;
            if (source_group.header.class_id & 0xffff) != 2 {
                return None;
            }
            let source_path = find_indexed_path(file, source_group)?;
            let center = anchors.get(binding.center_group_index)?.clone()?;
            let radians = binding_angle_radians(&binding.kind)?;
            let points = source_path
                .refs
                .iter()
                .filter_map(|object_ref| {
                    anchors.get(object_ref.saturating_sub(1)).cloned().flatten()
                })
                .map(|point| rotate_around(&point, &center, radians))
                .collect::<Vec<_>>();
            (points.len() >= 2 && has_distinct_points(&points)).then_some(LineShape {
                points,
                color: color_from_style(source_group.header.style_b),
                dashed: false,
                binding: Some(LineBinding::RotateLine {
                    source_index: path.refs.first()?.checked_sub(1)?,
                    center_index: binding.center_group_index,
                    angle_degrees: binding_angle_degrees(&binding.kind)?,
                }),
            })
        })
        .collect()
}

pub(super) fn collect_scaled_line_shapes(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
) -> Vec<LineShape> {
    groups
        .iter()
        .filter(|group| (group.header.class_id & 0xffff) == 30)
        .filter_map(|group| {
            let binding = decode_transform_binding(file, group)?;
            let TransformBindingKind::Scale { factor } = binding.kind else {
                return None;
            };
            let path = find_indexed_path(file, group)?;
            let source_group = groups.get(path.refs.first()?.checked_sub(1)?)?;
            if (source_group.header.class_id & 0xffff) != 2 {
                return None;
            }
            let source_path = find_indexed_path(file, source_group)?;
            let center = anchors.get(binding.center_group_index)?.clone()?;
            let points = source_path
                .refs
                .iter()
                .filter_map(|object_ref| {
                    anchors.get(object_ref.saturating_sub(1)).cloned().flatten()
                })
                .map(|point| scale_around(&point, &center, factor))
                .collect::<Vec<_>>();
            (points.len() >= 2 && has_distinct_points(&points)).then_some(LineShape {
                points,
                color: color_from_style(source_group.header.style_b),
                dashed: false,
                binding: Some(LineBinding::ScaleLine {
                    source_index: path.refs.first()?.checked_sub(1)?,
                    center_index: binding.center_group_index,
                    factor,
                }),
            })
        })
        .collect()
}

pub(super) fn collect_reflected_line_shapes(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
) -> Vec<LineShape> {
    groups
        .iter()
        .filter(|group| matches!(group.header.class_id & 0xffff, 16 | 34))
        .filter_map(|group| {
            let path = find_indexed_path(file, group)?;
            let source_group = groups.get(path.refs.first()?.checked_sub(1)?)?;
            if (source_group.header.class_id & 0xffff) != 2 {
                return None;
            }
            let (line_start_group_index, line_end_group_index) =
                reflection_line_group_indices(file, groups, group)?;
            let source_path = find_indexed_path(file, source_group)?;
            let line_start = anchors.get(line_start_group_index)?.clone()?;
            let line_end = anchors.get(line_end_group_index)?.clone()?;
            let points = source_path
                .refs
                .iter()
                .filter_map(|object_ref| {
                    anchors.get(object_ref.saturating_sub(1)).cloned().flatten()
                })
                .filter_map(|point| reflect_across_line(&point, &line_start, &line_end))
                .collect::<Vec<_>>();
            (points.len() >= 2 && has_distinct_points(&points)).then_some(LineShape {
                points,
                color: color_from_style(source_group.header.style_b),
                dashed: false,
                binding: Some(LineBinding::ReflectLine {
                    source_index: path.refs.first()?.checked_sub(1)?,
                    line_start_index: line_start_group_index,
                    line_end_index: line_end_group_index,
                }),
            })
        })
        .collect()
}

pub(super) fn collect_rotated_circle_shapes(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
) -> Vec<CircleShape> {
    groups
        .iter()
        .filter(|group| (group.header.class_id & 0xffff) == 29)
        .filter_map(|group| {
            let binding = decode_parameter_rotation_binding(file, groups, group)?;
            let path = find_indexed_path(file, group)?;
            let source_group = groups.get(path.refs.first()?.checked_sub(1)?)?;
            if (source_group.header.class_id & 0xffff) != 3 {
                return None;
            }
            let source_path = find_indexed_path(file, source_group)?;
            if source_path.refs.len() != 2 {
                return None;
            }
            let center = anchors.get(binding.center_group_index)?.clone()?;
            let radians = binding_angle_radians(&binding.kind)?;
            let source_center = anchors.get(source_path.refs[0].checked_sub(1)?)?.clone()?;
            let source_radius = anchors.get(source_path.refs[1].checked_sub(1)?)?.clone()?;
            Some(CircleShape {
                center: rotate_around(&source_center, &center, radians),
                radius_point: rotate_around(&source_radius, &center, radians),
                color: color_from_style(source_group.header.style_b),
                binding: Some(ShapeBinding::RotateCircle {
                    source_index: path.refs.first()?.checked_sub(1)?,
                    center_index: binding.center_group_index,
                    angle_degrees: binding_angle_degrees(&binding.kind)?,
                }),
            })
        })
        .collect()
}

pub(super) fn collect_translated_polygon_shapes(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
) -> Vec<PolygonShape> {
    groups
        .iter()
        .filter(|group| (group.header.class_id & 0xffff) == 16)
        .filter_map(|group| {
            let path = find_indexed_path(file, group)?;
            let source_group = groups.get(path.refs.first()?.checked_sub(1)?)?;
            if (source_group.header.class_id & 0xffff) != 8 {
                return None;
            }
            let (dx, dy, vector_start_index, vector_end_index) =
                translation_delta(file, group, anchors)?;
            let source_path = find_indexed_path(file, source_group)?;
            let points = source_path
                .refs
                .iter()
                .filter_map(|object_ref| {
                    anchors.get(object_ref.saturating_sub(1)).cloned().flatten()
                })
                .map(|point| PointRecord {
                    x: point.x + dx,
                    y: point.y + dy,
                })
                .collect::<Vec<_>>();
            (points.len() >= 3).then_some(PolygonShape {
                points,
                color: fill_color_from_styles(
                    source_group.header.style_b,
                    source_group.header.style_c,
                ),
                binding: Some(ShapeBinding::TranslatePolygon {
                    source_index: path.refs.first()?.checked_sub(1)?,
                    vector_start_index,
                    vector_end_index,
                }),
            })
        })
        .collect()
}

pub(super) fn collect_transformed_circle_shapes(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
) -> Vec<CircleShape> {
    groups
        .iter()
        .filter(|group| (group.header.class_id & 0xffff) == 30)
        .filter_map(|group| {
            let binding = decode_transform_binding(file, group)?;
            let TransformBindingKind::Scale { factor } = binding.kind else {
                return None;
            };
            let path = find_indexed_path(file, group)?;
            let source_group = groups.get(path.refs.first()?.checked_sub(1)?)?;
            if (source_group.header.class_id & 0xffff) != 3 {
                return None;
            }
            let source_path = find_indexed_path(file, source_group)?;
            if source_path.refs.len() != 2 {
                return None;
            }
            let scale_center = anchors.get(binding.center_group_index)?.clone()?;
            let source_center = anchors.get(source_path.refs[0].checked_sub(1)?)?.clone()?;
            let source_radius = anchors.get(source_path.refs[1].checked_sub(1)?)?.clone()?;
            Some(CircleShape {
                center: scale_around(&source_center, &scale_center, factor),
                radius_point: scale_around(&source_radius, &scale_center, factor),
                color: color_from_style(source_group.header.style_b),
                binding: Some(ShapeBinding::ScaleCircle {
                    source_index: path.refs.first()?.checked_sub(1)?,
                    center_index: binding.center_group_index,
                    factor,
                }),
            })
        })
        .collect()
}

pub(super) fn collect_rotated_polygon_shapes(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
) -> Vec<PolygonShape> {
    groups
        .iter()
        .filter(|group| (group.header.class_id & 0xffff) == 29)
        .filter_map(|group| {
            let binding = decode_parameter_rotation_binding(file, groups, group)?;
            let path = find_indexed_path(file, group)?;
            let source_group = groups.get(path.refs.first()?.checked_sub(1)?)?;
            if (source_group.header.class_id & 0xffff) != 8 {
                return None;
            }
            let source_path = find_indexed_path(file, source_group)?;
            let center = anchors.get(binding.center_group_index)?.clone()?;
            let radians = binding_angle_radians(&binding.kind)?;
            let points = source_path
                .refs
                .iter()
                .filter_map(|object_ref| {
                    anchors.get(object_ref.saturating_sub(1)).cloned().flatten()
                })
                .map(|point| rotate_around(&point, &center, radians))
                .collect::<Vec<_>>();
            (points.len() >= 3).then_some(PolygonShape {
                points,
                color: fill_color_from_styles(
                    source_group.header.style_b,
                    source_group.header.style_c,
                ),
                binding: Some(ShapeBinding::RotatePolygon {
                    source_index: path.refs.first()?.checked_sub(1)?,
                    center_index: binding.center_group_index,
                    angle_degrees: binding_angle_degrees(&binding.kind)?,
                }),
            })
        })
        .collect()
}

pub(super) fn collect_transformed_polygon_shapes(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
) -> Vec<PolygonShape> {
    groups
        .iter()
        .filter(|group| (group.header.class_id & 0xffff) == 30)
        .filter_map(|group| {
            let binding = decode_transform_binding(file, group)?;
            let TransformBindingKind::Scale { factor } = binding.kind else {
                return None;
            };
            let path = find_indexed_path(file, group)?;
            let source_group = groups.get(path.refs.first()?.checked_sub(1)?)?;
            if (source_group.header.class_id & 0xffff) != 8 {
                return None;
            }
            let source_path = find_indexed_path(file, source_group)?;
            let scale_center = anchors.get(binding.center_group_index)?.clone()?;
            let points = source_path
                .refs
                .iter()
                .filter_map(|object_ref| {
                    anchors.get(object_ref.saturating_sub(1)).cloned().flatten()
                })
                .map(|point| scale_around(&point, &scale_center, factor))
                .collect::<Vec<_>>();
            (points.len() >= 3).then_some(PolygonShape {
                points,
                color: fill_color_from_styles(
                    source_group.header.style_b,
                    source_group.header.style_c,
                ),
                binding: Some(ShapeBinding::ScalePolygon {
                    source_index: path.refs.first()?.checked_sub(1)?,
                    center_index: binding.center_group_index,
                    factor,
                }),
            })
        })
        .collect()
}

pub(super) fn collect_reflected_circle_shapes(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
) -> Vec<CircleShape> {
    groups
        .iter()
        .filter(|group| (group.header.class_id & 0xffff) == 34)
        .filter_map(|group| {
            let path = find_indexed_path(file, group)?;
            let source_group = groups.get(path.refs.first()?.checked_sub(1)?)?;
            if (source_group.header.class_id & 0xffff) != 3 {
                return None;
            }
            let (line_start_group_index, line_end_group_index) =
                reflection_line_group_indices(file, groups, group)?;
            let source_path = find_indexed_path(file, source_group)?;
            let source_center = anchors.get(source_path.refs[0].checked_sub(1)?)?.clone()?;
            let source_radius = anchors.get(source_path.refs[1].checked_sub(1)?)?.clone()?;
            let line_start = anchors.get(line_start_group_index)?.clone()?;
            let line_end = anchors.get(line_end_group_index)?.clone()?;
            let center = reflect_across_line(&source_center, &line_start, &line_end)?;
            let radius_point = reflect_across_line(&source_radius, &line_start, &line_end)?;
            Some(CircleShape {
                center,
                radius_point,
                color: color_from_style(source_group.header.style_b),
                binding: Some(ShapeBinding::ReflectCircle {
                    source_index: path.refs.first()?.checked_sub(1)?,
                    line_start_index: line_start_group_index,
                    line_end_index: line_end_group_index,
                }),
            })
        })
        .collect()
}

pub(super) fn collect_reflected_polygon_shapes(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
) -> Vec<PolygonShape> {
    groups
        .iter()
        .filter(|group| (group.header.class_id & 0xffff) == 34)
        .filter_map(|group| {
            let path = find_indexed_path(file, group)?;
            let source_group = groups.get(path.refs.first()?.checked_sub(1)?)?;
            if (source_group.header.class_id & 0xffff) != 8 {
                return None;
            }
            let (line_start_group_index, line_end_group_index) =
                reflection_line_group_indices(file, groups, group)?;
            let source_path = find_indexed_path(file, source_group)?;
            let line_start = anchors.get(line_start_group_index)?.clone()?;
            let line_end = anchors.get(line_end_group_index)?.clone()?;
            let points = source_path
                .refs
                .iter()
                .filter_map(|object_ref| {
                    anchors.get(object_ref.saturating_sub(1)).cloned().flatten()
                })
                .filter_map(|point| reflect_across_line(&point, &line_start, &line_end))
                .collect::<Vec<_>>();
            (points.len() >= 3).then_some(PolygonShape {
                points,
                color: fill_color_from_styles(
                    source_group.header.style_b,
                    source_group.header.style_c,
                ),
                binding: Some(ShapeBinding::ReflectPolygon {
                    source_index: path.refs.first()?.checked_sub(1)?,
                    line_start_index: line_start_group_index,
                    line_end_index: line_end_group_index,
                }),
            })
        })
        .collect()
}

fn binding_angle_degrees(binding: &TransformBindingKind) -> Option<f64> {
    match binding {
        TransformBindingKind::Rotate { angle_degrees } => Some(*angle_degrees),
        TransformBindingKind::Scale { .. } => None,
    }
}

fn binding_angle_radians(binding: &TransformBindingKind) -> Option<f64> {
    binding_angle_degrees(binding).map(f64::to_radians)
}

fn translation_delta(
    file: &GspFile,
    group: &ObjectGroup,
    anchors: &[Option<PointRecord>],
) -> Option<(f64, f64, usize, usize)> {
    let (vector_start_index, vector_end_index) = translation_point_pair_group_indices(file, group)?;
    let vector_start = anchors.get(vector_start_index)?.clone()?;
    let vector_end = anchors.get(vector_end_index)?.clone()?;
    Some((
        vector_end.x - vector_start.x,
        vector_end.y - vector_start.y,
        vector_start_index,
        vector_end_index,
    ))
}

pub(super) fn collect_rotational_iteration_lines(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
) -> Vec<LineShape> {
    groups
        .iter()
        .filter(|group| (group.header.class_id & 0xffff) == 77)
        .filter_map(|group| {
            let path = find_indexed_path(file, group)?;
            let source_group = groups.get(path.refs.first()?.checked_sub(1)?)?;
            if (source_group.header.class_id & 0xffff) != 2 {
                return None;
            }
            let iter_group = groups.get(path.refs.get(1)?.checked_sub(1)?)?;
            if (iter_group.header.class_id & 0xffff) != 89 {
                return None;
            }
            let (center_group_index, angle_expr, parameter_name, n) =
                regular_polygon_iteration_step(file, groups, iter_group)?;
            let angle_degrees = -360.0 / n;
            let center = anchors.get(center_group_index)?.clone()?;
            let source_path = find_indexed_path(file, source_group)?;
            if source_path.refs.len() != 2 {
                return None;
            }
            let seed_vertex_group = groups.get(path.refs.first()?.checked_sub(1)?)?;
            let seed_vertex_path = find_indexed_path(file, seed_vertex_group)?;
            let vertex_group_index = seed_vertex_path.refs.first()?.checked_sub(1)?;
            let vertex = anchors.get(vertex_group_index)?.clone()?;
            let depth = iter_group
                .records
                .iter()
                .find(|record| record.record_type == 0x090a)
                .map(|record| record.payload(&file.data))
                .filter(|payload| payload.len() >= 20)
                .map(|payload| read_u32(payload, 16) as usize)
                .unwrap_or(0);
            let mut lines = Vec::new();
            let rotate = |point: &PointRecord, step: usize| {
                rotate_around(point, &center, (angle_degrees * step as f64).to_radians())
            };
            for step in 0..=depth {
                lines.push(LineShape {
                    points: vec![
                        rotate(&vertex, step),
                        rotate(&vertex, (step + 1) % (depth + 1)),
                    ],
                    color: color_from_style(source_group.header.style_b),
                    dashed: false,
                    binding: Some(LineBinding::RotateEdge {
                        center_index: center_group_index,
                        vertex_index: vertex_group_index,
                        parameter_name: parameter_name.clone(),
                        angle_expr: angle_expr.clone(),
                        start_step: step,
                        end_step: (step + 1) % (depth + 1),
                    }),
                });
            }
            Some(lines)
        })
        .flatten()
        .collect()
}

pub(super) fn collect_carried_iteration_lines(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
) -> Vec<LineShape> {
    groups
        .iter()
        .filter(|group| (group.header.class_id & 0xffff) == 77)
        .filter_map(|group| {
            let path = find_indexed_path(file, group)?;
            let source_group = groups.get(path.refs.first()?.checked_sub(1)?)?;
            if (source_group.header.class_id & 0xffff) != 2 {
                return None;
            }
            let iter_group = groups.get(path.refs.get(1)?.checked_sub(1)?)?;
            if !matches!(iter_group.header.class_id & 0xffff, 76 | 89) {
                return None;
            }
            let source_path = find_indexed_path(file, source_group)?;
            if source_path.refs.len() != 2 {
                return None;
            }
            let start = anchors.get(source_path.refs[0].checked_sub(1)?)?.clone()?;
            let end = anchors.get(source_path.refs[1].checked_sub(1)?)?.clone()?;
            let steps = carried_iteration_steps(file, groups, iter_group, anchors);
            let Some(step) = steps.first().cloned() else {
                return None;
            };
            let secondary_step = steps.get(1).cloned();
            let depth = iter_group
                .records
                .iter()
                .find(|record| record.record_type == 0x090a)
                .map(|record| record.payload(&file.data))
                .filter(|payload| payload.len() >= 20)
                .map(|payload| read_u32(payload, 16) as usize)
                .unwrap_or(3);
            let color = color_from_style(source_group.header.style_b);
            Some(
                carried_iteration_line_deltas(&step, secondary_step.as_ref(), depth)
                    .into_iter()
                    .map(|delta| {
                        LineShape {
                            points: vec![start.clone() + delta.clone(), end.clone() + delta],
                            color,
                            dashed: false,
                            binding: None,
                        }
                    })
                    .collect::<Vec<_>>(),
            )
        })
        .flatten()
        .collect()
}

pub(super) fn collect_carried_line_iteration_families(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
    group_to_point_index: &[Option<usize>],
) -> Vec<LineIterationFamily> {
    groups
        .iter()
        .filter(|group| (group.header.class_id & 0xffff) == 77)
        .filter_map(|group| {
            let path = find_indexed_path(file, group)?;
            let source_group_index = path.refs.first()?.checked_sub(1)?;
            let source_group = groups.get(source_group_index)?;
            if (source_group.header.class_id & 0xffff) != 2 {
                return None;
            }
            let iter_group = groups.get(path.refs.get(1)?.checked_sub(1)?)?;
            if !matches!(iter_group.header.class_id & 0xffff, 76 | 89) {
                return None;
            }
            let source_path = find_indexed_path(file, source_group)?;
            if source_path.refs.len() != 2 {
                return None;
            }
            let start_index = group_to_point_index
                .get(source_path.refs[0].checked_sub(1)?)
                .copied()
                .flatten()?;
            let end_index = group_to_point_index
                .get(source_path.refs[1].checked_sub(1)?)
                .copied()
                .flatten()?;
            let steps = carried_iteration_steps(file, groups, iter_group, anchors);
            let Some(step) = steps.first().cloned() else {
                return None;
            };
            let secondary_step = steps.get(1).cloned();
            let depth = carried_iteration_depth(file, iter_group, 3);
            if depth == 0 {
                return None;
            }
            Some(LineIterationFamily {
                start_index,
                end_index,
                dx: step.x,
                dy: step.y,
                secondary_dx: secondary_step.as_ref().map(|step| step.x),
                secondary_dy: secondary_step.as_ref().map(|step| step.y),
                depth,
                parameter_name: carried_iteration_parameter_name(file, groups, iter_group),
                color: color_from_style(source_group.header.style_b),
                dashed: false,
            })
        })
        .collect()
}

fn carried_iteration_steps(
    file: &GspFile,
    groups: &[ObjectGroup],
    iter_group: &ObjectGroup,
    anchors: &[Option<PointRecord>],
) -> Vec<PointRecord> {
    let Some(iter_path) = find_indexed_path(file, iter_group) else {
        return Vec::new();
    };
    let translated_steps = iter_path
        .refs
        .iter()
        .filter_map(|ordinal| ordinal.checked_sub(1).and_then(|index| groups.get(index)))
        .filter_map(|group| decode_translated_point_constraint(file, group))
        .map(|constraint| PointRecord {
            x: constraint.dx,
            y: constraint.dy,
        })
        .fold(Vec::<PointRecord>::new(), |mut acc, step| {
            let already_present = acc.iter().any(|existing| {
                (existing.x - step.x).abs() < 1e-6 && (existing.y - step.y).abs() < 1e-6
            });
            if !already_present {
                acc.push(step);
            }
            acc
        });
    if !translated_steps.is_empty() {
        return translated_steps;
    }
    if iter_path.refs.len() < 2 {
        return Vec::new();
    }
    let Some(base_start_index) = iter_path.refs[0].checked_sub(1) else {
        return Vec::new();
    };
    let Some(base_end_index) = iter_path.refs[1].checked_sub(1) else {
        return Vec::new();
    };
    let Some(base_start) = anchors.get(base_start_index).cloned().flatten() else {
        return Vec::new();
    };
    let Some(base_end) = anchors.get(base_end_index).cloned().flatten() else {
        return Vec::new();
    };
    vec![PointRecord {
        x: base_end.x - base_start.x,
        y: base_end.y - base_start.y,
    }]
}

fn carried_iteration_line_deltas(
    step: &PointRecord,
    secondary_step: Option<&PointRecord>,
    depth: usize,
) -> Vec<PointRecord> {
    if let Some(secondary) = secondary_step {
        let mut deltas = Vec::new();
        for primary_index in 0..=depth {
            for secondary_index in 0..=depth - primary_index {
                if primary_index == 0 && secondary_index == 0 {
                    continue;
                }
                deltas.push(PointRecord {
                    x: step.x * primary_index as f64 + secondary.x * secondary_index as f64,
                    y: step.y * primary_index as f64 + secondary.y * secondary_index as f64,
                });
            }
        }
        return deltas;
    }

    (1..=depth)
        .map(|index| PointRecord {
            x: step.x * index as f64,
            y: step.y * index as f64,
        })
        .collect()
}

fn carried_iteration_polygon_deltas(
    step: &PointRecord,
    secondary_step: Option<&PointRecord>,
    depth: usize,
) -> Vec<PointRecord> {
    if let Some(secondary) = secondary_step {
        let mut deltas = Vec::new();
        for primary_index in 0..=depth {
            for secondary_index in 0..=depth - primary_index {
                deltas.push(PointRecord {
                    x: step.x * primary_index as f64 + secondary.x * secondary_index as f64,
                    y: step.y * primary_index as f64 + secondary.y * secondary_index as f64,
                });
            }
        }
        return deltas;
    }

    (0..=depth)
        .map(|index| PointRecord {
            x: step.x * index as f64,
            y: step.y * index as f64,
        })
        .collect()
}

fn carried_iteration_depth(file: &GspFile, iter_group: &ObjectGroup, default_depth: usize) -> usize {
    iter_group
        .records
        .iter()
        .find(|record| record.record_type == 0x090a)
        .map(|record| record.payload(&file.data))
        .filter(|payload| payload.len() >= 20)
        .map(|payload| read_u32(payload, 16) as usize)
        .unwrap_or(default_depth)
}

fn carried_iteration_parameter_name(
    file: &GspFile,
    groups: &[ObjectGroup],
    iter_group: &ObjectGroup,
) -> Option<String> {
    let iter_path = find_indexed_path(file, iter_group)?;
    let parameter_group = groups.get(iter_path.refs.first()?.checked_sub(1)?)?;
    ((parameter_group.header.class_id & 0xffff) == 0)
        .then(|| decode_label_name(file, parameter_group))
        .flatten()
        .filter(|name| super::points::is_editable_non_graph_parameter_name(name))
}

pub(super) fn collect_carried_iteration_polygons(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
) -> Vec<PolygonShape> {
    groups
        .iter()
        .filter(|group| (group.header.class_id & 0xffff) == 77)
        .filter_map(|group| {
            let path = find_indexed_path(file, group)?;
            let source_group = groups.get(path.refs.first()?.checked_sub(1)?)?;
            if (source_group.header.class_id & 0xffff) != 8 {
                return None;
            }
            let iter_group = groups.get(path.refs.get(1)?.checked_sub(1)?)?;
            if !matches!(iter_group.header.class_id & 0xffff, 76 | 89) {
                return None;
            }
            let source_path = find_indexed_path(file, source_group)?;
            if source_path.refs.len() < 3 {
                return None;
            }
            let points = source_path
                .refs
                .iter()
                .map(|ordinal| anchors.get(ordinal.checked_sub(1)?).cloned().flatten())
                .collect::<Option<Vec<_>>>()?;
            let steps = carried_iteration_steps(file, groups, iter_group, anchors);
            let Some(step) = steps.first().cloned() else {
                return None;
            };
            let secondary_step = steps.get(1).cloned();
            let depth = iter_group
                .records
                .iter()
                .find(|record| record.record_type == 0x090a)
                .map(|record| record.payload(&file.data))
                .filter(|payload| payload.len() >= 20)
                .map(|payload| read_u32(payload, 16) as usize)
                .unwrap_or(3);
            let color =
                fill_color_from_styles(source_group.header.style_a, source_group.header.style_b);
            Some(
                carried_iteration_polygon_deltas(&step, secondary_step.as_ref(), depth)
                    .into_iter()
                    .map(|delta| {
                        PolygonShape {
                            points: points
                                .iter()
                                .cloned()
                                .map(|point| point + delta.clone())
                                .collect(),
                            color,
                            binding: None,
                        }
                    })
                    .collect::<Vec<_>>(),
            )
        })
        .flatten()
        .collect()
}

pub(super) fn collect_carried_polygon_iteration_families(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
    group_to_point_index: &[Option<usize>],
) -> Vec<PolygonIterationFamily> {
    groups
        .iter()
        .filter(|group| (group.header.class_id & 0xffff) == 77)
        .filter_map(|group| {
            let path = find_indexed_path(file, group)?;
            let source_group = groups.get(path.refs.first()?.checked_sub(1)?)?;
            if (source_group.header.class_id & 0xffff) != 8 {
                return None;
            }
            let iter_group = groups.get(path.refs.get(1)?.checked_sub(1)?)?;
            if !matches!(iter_group.header.class_id & 0xffff, 76 | 89) {
                return None;
            }
            let source_path = find_indexed_path(file, source_group)?;
            if source_path.refs.len() < 3 {
                return None;
            }
            let vertex_indices = source_path
                .refs
                .iter()
                .map(|ordinal| group_to_point_index.get(ordinal.checked_sub(1)?).copied().flatten())
                .collect::<Option<Vec<_>>>()?;
            let steps = carried_iteration_steps(file, groups, iter_group, anchors);
            let Some(step) = steps.first().cloned() else {
                return None;
            };
            let secondary_step = steps.get(1).cloned();
            let depth = carried_iteration_depth(file, iter_group, 3);
            if depth == 0 {
                return None;
            }
            Some(PolygonIterationFamily {
                vertex_indices,
                dx: step.x,
                dy: step.y,
                secondary_dx: secondary_step.as_ref().map(|step| step.x),
                secondary_dy: secondary_step.as_ref().map(|step| step.y),
                depth,
                parameter_name: carried_iteration_parameter_name(file, groups, iter_group),
                color: fill_color_from_styles(source_group.header.style_a, source_group.header.style_b),
            })
        })
        .collect()
}

pub(super) fn collect_iteration_shapes(
    file: &GspFile,
    groups: &[ObjectGroup],
    circles: &[CircleShape],
) -> (Vec<LineShape>, Vec<PolygonShape>) {
    let mut lines = Vec::new();
    let polygons = Vec::new();

    let has_iteration = groups
        .iter()
        .any(|group| (group.header.class_id & 0xffff) == 89);
    if !has_iteration {
        return (lines, polygons);
    }

    for iter_group in groups
        .iter()
        .filter(|group| (group.header.class_id & 0xffff) == 89)
    {
        let Some(iter_path) = find_indexed_path(file, iter_group) else {
            continue;
        };

        let iter_data = iter_group
            .records
            .iter()
            .find(|record| record.record_type == 0x090a)
            .map(|record| record.payload(&file.data));

        let depth = iter_data
            .filter(|payload| payload.len() >= 20)
            .map(|payload| read_u32(payload, 16) as usize)
            .unwrap_or(0);
        if depth == 0 {
            continue;
        }

        let polygon_group_index = iter_path.refs.iter().find_map(|&obj_ref| {
            let index = obj_ref.checked_sub(1)?;
            let group = groups.get(index)?;
            ((group.header.class_id & 0xffff) == 8).then_some(index)
        });

        let Some(polygon_index) = polygon_group_index else {
            continue;
        };
        let polygon_group = &groups[polygon_index];
        let Some(polygon_path) = find_indexed_path(file, polygon_group) else {
            continue;
        };
        if polygon_path.refs.len() < 3 {
            continue;
        }

        let Some(circle) = circles.first() else {
            continue;
        };
        let cx = circle.center.x;
        let cy = circle.center.y;
        let radius =
            ((circle.radius_point.x - cx).powi(2) + (circle.radius_point.y - cy).powi(2)).sqrt();
        if radius < 1.0 {
            continue;
        }

        let px_per_cm = groups
            .iter()
            .filter(|group| (group.header.class_id & 0xffff) == 21)
            .find_map(|group| {
                let payload = group
                    .records
                    .iter()
                    .find(|record| record.record_type == 0x07d3)
                    .map(|record| record.payload(&file.data))?;
                (payload.len() >= 40).then(|| read_f64(payload, 32))
            })
            .filter(|v| v.is_finite() && *v > 1.0)
            .unwrap_or(37.79527559055118);

        let param_value = groups
            .iter()
            .filter(|group| (group.header.class_id & 0xffff) == 21)
            .find_map(|group| {
                let payload = group
                    .records
                    .iter()
                    .find(|record| record.record_type == 0x07d3)
                    .map(|record| record.payload(&file.data))?;
                (payload.len() >= 20).then(|| read_f64(payload, 12))
            })
            .filter(|v| v.is_finite() && *v > 0.0)
            .unwrap_or(1.0);

        let side = param_value * px_per_cm / 2.0;
        if side < 1.0 {
            continue;
        }

        let outline_color = [30, 30, 30, 255];
        let sqrt3 = 3.0_f64.sqrt();
        let col_spacing = sqrt3 * side;
        let row_spacing = 1.5 * side;
        let max_cols = (radius / col_spacing).ceil() as i32 + 2;
        let max_rows = (radius / row_spacing).ceil() as i32 + 2;

        let hex_vertices = |hx: f64, hy: f64| -> Vec<PointRecord> {
            (0..6)
                .map(|i| {
                    let angle =
                        std::f64::consts::FRAC_PI_3 * i as f64 + std::f64::consts::FRAC_PI_6;
                    PointRecord {
                        x: hx + side * angle.cos(),
                        y: hy + side * angle.sin(),
                    }
                })
                .collect()
        };

        for row in -max_rows..=max_rows {
            let y = cy + row as f64 * row_spacing;
            let x_offset = if row.rem_euclid(2) == 1 {
                col_spacing / 2.0
            } else {
                0.0
            };
            for col in -max_cols..=max_cols {
                let x = cx + col as f64 * col_spacing + x_offset;
                let dist = ((x - cx).powi(2) + (y - cy).powi(2)).sqrt();
                if dist > radius + side * 0.5 {
                    continue;
                }
                let verts = hex_vertices(x, y);

                let mut outline = verts.clone();
                outline.push(verts[0].clone());
                lines.push(LineShape {
                    points: outline,
                    color: outline_color,
                    dashed: false,
                    binding: None,
                });
            }
        }
    }

    (lines, polygons)
}

pub(super) fn collect_derived_segments(
    file: &GspFile,
    groups: &[ObjectGroup],
    point_map: &[Option<PointRecord>],
    kinds: &[u32],
) -> Vec<LineShape> {
    let refs = groups
        .iter()
        .map(|group| {
            find_indexed_path(file, group)
                .map(|path| path.refs)
                .unwrap_or_default()
        })
        .collect::<Vec<_>>();
    let class_ids = groups
        .iter()
        .map(|group| group.header.class_id & 0xffff)
        .collect::<Vec<_>>();

    fn descend_points(
        ordinal: usize,
        refs: &[Vec<usize>],
        point_map: &[Option<PointRecord>],
        memo: &mut Vec<Option<Vec<PointRecord>>>,
        visiting: &mut BTreeSet<usize>,
    ) -> Vec<PointRecord> {
        if let Some(cached) = &memo[ordinal - 1] {
            return cached.clone();
        }
        if !visiting.insert(ordinal) {
            return Vec::new();
        }

        let mut points = Vec::new();
        if let Some(point) = point_map.get(ordinal - 1).and_then(|point| point.clone()) {
            points.push(point);
        } else {
            for child in &refs[ordinal - 1] {
                if *child > 0 && *child <= refs.len() {
                    points.extend(descend_points(*child, refs, point_map, memo, visiting));
                }
            }
        }

        visiting.remove(&ordinal);
        points.sort_by(|a, b| {
            a.x.partial_cmp(&b.x)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.y.partial_cmp(&b.y).unwrap_or(std::cmp::Ordering::Equal))
        });
        points.dedup_by(|a, b| (a.x - b.x).abs() < 0.001 && (a.y - b.y).abs() < 0.001);
        memo[ordinal - 1] = Some(points.clone());
        points
    }

    let mut memo = vec![None; groups.len()];
    let mut seen = BTreeSet::<((i32, i32), (i32, i32))>::new();
    let mut segments = Vec::new();

    for (index, class_id) in class_ids.iter().enumerate() {
        if !kinds.contains(class_id) {
            continue;
        }
        let points = descend_points(index + 1, &refs, point_map, &mut memo, &mut BTreeSet::new());
        if points.len() < 2 || points.len() > 12 {
            continue;
        }

        let mut best = None;
        let mut best_dist = -1.0_f64;
        for i in 0..points.len() {
            for j in i + 1..points.len() {
                let dx = points[i].x - points[j].x;
                let dy = points[i].y - points[j].y;
                let dist = dx * dx + dy * dy;
                if dist > best_dist {
                    best_dist = dist;
                    best = Some((points[i].clone(), points[j].clone()));
                }
            }
        }

        let Some((a, b)) = best else { continue };
        let a_key = (a.x.round() as i32, a.y.round() as i32);
        let b_key = (b.x.round() as i32, b.y.round() as i32);
        let key = if a_key <= b_key {
            (a_key, b_key)
        } else {
            (b_key, a_key)
        };
        if !seen.insert(key) {
            continue;
        }

        let color = match *class_id {
            24 => [20, 20, 20, 255],
            48 => [70, 70, 70, 255],
            75 => [120, 120, 120, 255],
            _ => [60, 60, 60, 255],
        };
        segments.push(LineShape {
            points: vec![a, b],
            color,
            dashed: false,
            binding: None,
        });
    }

    segments
}

pub(super) fn collect_coordinate_traces(
    file: &GspFile,
    groups: &[ObjectGroup],
    graph: &Option<GraphTransform>,
) -> Vec<LineShape> {
    groups
        .iter()
        .filter(|group| (group.header.class_id & 0xffff) == 97)
        .filter_map(|group| {
            let path = find_indexed_path(file, group)?;
            if path.refs.len() < 3 {
                return None;
            }
            let parameter_group = groups.get(path.refs[1].checked_sub(1)?)?;
            let calc_group = groups.get(path.refs[2].checked_sub(1)?)?;
            let parameter_name = decode_label_name(file, parameter_group)?;
            let payload = group
                .records
                .iter()
                .find(|record| record.record_type == 0x0902)
                .map(|record| record.payload(&file.data))?;
            let descriptor = decode_function_plot_descriptor(payload)?;
            let expr = decode_function_expr(file, groups, calc_group)?;

            let mut points = Vec::with_capacity(descriptor.sample_count);
            let last = descriptor.sample_count.saturating_sub(1).max(1) as f64;
            for index in 0..descriptor.sample_count {
                let t = index as f64 / last;
                let x = descriptor.x_min + (descriptor.x_max - descriptor.x_min) * t;
                let parameters = BTreeMap::from([(parameter_name.clone(), x)]);
                let y = evaluate_expr_with_parameters(&expr, 0.0, &parameters)?;
                let world = PointRecord { x, y };
                let point = if let Some(transform) = graph {
                    to_raw_from_world(&world, transform)
                } else {
                    world
                };
                points.push(point);
            }

            (points.len() >= 2).then_some(LineShape {
                points,
                color: color_from_style(group.header.style_b),
                dashed: false,
                binding: None,
            })
        })
        .collect()
}
