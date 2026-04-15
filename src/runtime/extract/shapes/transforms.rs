use super::{
    CircleShape, GspFile, LineBinding, LineShape, ObjectGroup, PointRecord, PolygonShape,
    ShapeBinding, TransformBindingKind, collect_circle_fill_colors, color_from_style,
    fill_color_from_styles, find_indexed_path, has_distinct_points, line_is_dashed,
    payload_debug_source, reflect_across_line, rotate_around, scale_around,
    translation_point_pair_group_indices, try_decode_parameter_rotation_binding,
    try_decode_transform_binding,
};
use crate::runtime::extract::decode::resolve_circle_points_raw;
use crate::runtime::extract::points::resolve_line_like_points_raw;
use crate::runtime::scene::{
    AxisBinding, LineTransformBinding, RotationBinding, ScaleBinding, ShapeTransformBinding,
};

pub(crate) fn collect_rotated_line_shapes(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
) -> Vec<LineShape> {
    groups
        .iter()
        .filter(|group| (group.header.kind()) == crate::format::GroupKind::ParameterRotation)
        .filter_map(|group| {
            let binding = try_decode_parameter_rotation_binding(file, groups, group).ok()?;
            let path = find_indexed_path(file, group)?;
            let source_group = groups.get(path.refs.first()?.checked_sub(1)?)?;
            if (source_group.header.kind()) != crate::format::GroupKind::Segment {
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
                dashed: line_is_dashed(source_group.header.style_a),
                visible: !group.header.is_hidden(),
                binding: Some(LineBinding::DerivedTransform {
                    source_index: path.refs.first()?.checked_sub(1)?,
                    transform: LineTransformBinding::Rotate(RotationBinding {
                        center_index: binding.center_group_index,
                        angle_degrees: binding_angle_degrees(&binding.kind)?,
                        parameter_name: binding_parameter_name(&binding.kind),
                    }),
                }),
                debug: Some(payload_debug_source(group)),
            })
        })
        .collect()
}

pub(crate) fn collect_translated_line_shapes(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
) -> Vec<LineShape> {
    groups
        .iter()
        .filter(|group| (group.header.kind()) == crate::format::GroupKind::Translation)
        .filter_map(|group| {
            let path = find_indexed_path(file, group)?;
            let source_group_index = path.refs.first()?.checked_sub(1)?;
            let source_group = groups.get(source_group_index)?;
            if (source_group.header.kind()) != crate::format::GroupKind::Segment {
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
            (points.len() >= 2 && has_distinct_points(&points)).then_some(LineShape {
                points,
                color: color_from_style(source_group.header.style_b),
                dashed: line_is_dashed(source_group.header.style_a),
                visible: !group.header.is_hidden(),
                binding: Some(LineBinding::DerivedTransform {
                    source_index: source_group_index,
                    transform: LineTransformBinding::Translate {
                        vector_start_index,
                        vector_end_index,
                    },
                }),
                debug: Some(payload_debug_source(group)),
            })
        })
        .collect()
}

pub(crate) fn collect_scaled_line_shapes(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
) -> Vec<LineShape> {
    groups
        .iter()
        .filter(|group| (group.header.kind()) == crate::format::GroupKind::Scale)
        .filter_map(|group| {
            let binding = try_decode_transform_binding(file, group).ok()?;
            let TransformBindingKind::Scale { factor } = binding.kind else {
                return None;
            };
            let path = find_indexed_path(file, group)?;
            let source_group = groups.get(path.refs.first()?.checked_sub(1)?)?;
            if (source_group.header.kind()) != crate::format::GroupKind::Segment {
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
                dashed: line_is_dashed(source_group.header.style_a),
                visible: !group.header.is_hidden(),
                binding: Some(LineBinding::DerivedTransform {
                    source_index: path.refs.first()?.checked_sub(1)?,
                    transform: LineTransformBinding::Scale(ScaleBinding {
                        center_index: binding.center_group_index,
                        factor,
                    }),
                }),
                debug: Some(payload_debug_source(group)),
            })
        })
        .collect()
}

pub(crate) fn collect_reflected_line_shapes(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
) -> Vec<LineShape> {
    groups
        .iter()
        .filter(|group| {
            matches!(
                group.header.kind(),
                crate::format::GroupKind::Translation | crate::format::GroupKind::Reflection
            )
        })
        .filter_map(|group| {
            let path = find_indexed_path(file, group)?;
            let source_group = groups.get(path.refs.first()?.checked_sub(1)?)?;
            if (source_group.header.kind()) != crate::format::GroupKind::Segment {
                return None;
            }
            let line_group_index = path.refs.get(1)?.checked_sub(1)?;
            let line_group = groups.get(line_group_index)?;
            let source_path = find_indexed_path(file, source_group)?;
            let (line_start, line_end) =
                resolve_line_like_points_raw(file, groups, anchors, line_group)?;
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
                dashed: line_is_dashed(source_group.header.style_a),
                visible: !group.header.is_hidden(),
                binding: Some(LineBinding::DerivedTransform {
                    source_index: path.refs.first()?.checked_sub(1)?,
                    transform: LineTransformBinding::Reflect(AxisBinding {
                        line_start_index: None,
                        line_end_index: None,
                        line_index: Some(line_group_index),
                    }),
                }),
                debug: Some(payload_debug_source(group)),
            })
        })
        .collect()
}

pub(crate) fn collect_rotated_circle_shapes(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
) -> Vec<CircleShape> {
    let circle_fill_colors = collect_circle_fill_colors(file, groups, anchors);
    groups
        .iter()
        .filter(|group| (group.header.kind()) == crate::format::GroupKind::ParameterRotation)
        .filter_map(|group| {
            let binding = try_decode_parameter_rotation_binding(file, groups, group).ok()?;
            let path = find_indexed_path(file, group)?;
            let source_group = groups.get(path.refs.first()?.checked_sub(1)?)?;
            let center = anchors.get(binding.center_group_index)?.clone()?;
            let radians = binding_angle_radians(&binding.kind)?;
            let (source_center, source_radius) =
                resolve_circle_points_raw(file, groups, anchors, source_group)?;
            Some(CircleShape {
                center: rotate_around(&source_center, &center, radians),
                radius_point: rotate_around(&source_radius, &center, radians),
                color: color_from_style(source_group.header.style_b),
                fill_color: circle_fill_colors
                    .get(&(path.refs.first()?.checked_sub(1)?))
                    .copied(),
                fill_color_binding: None,
                dashed: line_is_dashed(source_group.header.style_a),
                visible: !group.header.is_hidden(),
                binding: Some(ShapeBinding::DerivedTransform {
                    source_index: path.refs.first()?.checked_sub(1)?,
                    transform: ShapeTransformBinding::Rotate(RotationBinding {
                        center_index: binding.center_group_index,
                        angle_degrees: binding_angle_degrees(&binding.kind)?,
                        parameter_name: binding_parameter_name(&binding.kind),
                    }),
                }),
                debug: Some(payload_debug_source(group)),
            })
        })
        .collect()
}

pub(crate) fn collect_translated_circle_shapes(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
) -> Vec<CircleShape> {
    let circle_fill_colors = collect_circle_fill_colors(file, groups, anchors);
    groups
        .iter()
        .filter(|group| {
            matches!(
                group.header.kind(),
                crate::format::GroupKind::CartesianOffsetPoint
                    | crate::format::GroupKind::PolarOffsetPoint
            )
        })
        .filter_map(|group| {
            let constraint = super::decode_translated_point_constraint(file, group)?;
            let source_group_index = constraint.origin_group_index;
            let source_group = groups.get(source_group_index)?;
            let (source_center, source_radius) =
                resolve_circle_points_raw(file, groups, anchors, source_group)?;
            Some(CircleShape {
                center: PointRecord {
                    x: source_center.x + constraint.dx,
                    y: source_center.y + constraint.dy,
                },
                radius_point: PointRecord {
                    x: source_radius.x + constraint.dx,
                    y: source_radius.y + constraint.dy,
                },
                color: color_from_style(source_group.header.style_b),
                fill_color: circle_fill_colors.get(&source_group_index).copied(),
                fill_color_binding: None,
                dashed: line_is_dashed(source_group.header.style_a),
                visible: !group.header.is_hidden(),
                binding: Some(ShapeBinding::DerivedTransform {
                    source_index: source_group_index,
                    transform: ShapeTransformBinding::TranslateDelta {
                        dx: constraint.dx,
                        dy: constraint.dy,
                    },
                }),
                debug: Some(payload_debug_source(group)),
            })
        })
        .collect()
}

pub(crate) fn collect_translated_polygon_shapes(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
) -> Vec<PolygonShape> {
    groups
        .iter()
        .filter(|group| (group.header.kind()) == crate::format::GroupKind::Translation)
        .filter_map(|group| {
            let path = find_indexed_path(file, group)?;
            let source_group = groups.get(path.refs.first()?.checked_sub(1)?)?;
            if (source_group.header.kind()) != crate::format::GroupKind::Polygon {
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
                visible: !group.header.is_hidden(),
                binding: Some(ShapeBinding::DerivedTransform {
                    source_index: path.refs.first()?.checked_sub(1)?,
                    transform: ShapeTransformBinding::TranslateVector {
                        vector_start_index,
                        vector_end_index,
                    },
                }),
                debug: Some(payload_debug_source(group)),
            })
        })
        .collect()
}

pub(crate) fn collect_transformed_circle_shapes(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
) -> Vec<CircleShape> {
    let circle_fill_colors = collect_circle_fill_colors(file, groups, anchors);
    groups
        .iter()
        .filter(|group| (group.header.kind()) == crate::format::GroupKind::Scale)
        .filter_map(|group| {
            let binding = try_decode_transform_binding(file, group).ok()?;
            let TransformBindingKind::Scale { factor } = binding.kind else {
                return None;
            };
            let path = find_indexed_path(file, group)?;
            let source_group = groups.get(path.refs.first()?.checked_sub(1)?)?;
            let scale_center = anchors.get(binding.center_group_index)?.clone()?;
            let (source_center, source_radius) =
                resolve_circle_points_raw(file, groups, anchors, source_group)?;
            Some(CircleShape {
                center: scale_around(&source_center, &scale_center, factor),
                radius_point: scale_around(&source_radius, &scale_center, factor),
                color: color_from_style(source_group.header.style_b),
                fill_color: circle_fill_colors
                    .get(&(path.refs.first()?.checked_sub(1)?))
                    .copied(),
                fill_color_binding: None,
                dashed: line_is_dashed(source_group.header.style_a),
                visible: !group.header.is_hidden(),
                binding: Some(ShapeBinding::DerivedTransform {
                    source_index: path.refs.first()?.checked_sub(1)?,
                    transform: ShapeTransformBinding::Scale(ScaleBinding {
                        center_index: binding.center_group_index,
                        factor,
                    }),
                }),
                debug: Some(payload_debug_source(group)),
            })
        })
        .collect()
}

pub(crate) fn collect_rotated_polygon_shapes(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
) -> Vec<PolygonShape> {
    groups
        .iter()
        .filter(|group| (group.header.kind()) == crate::format::GroupKind::ParameterRotation)
        .filter_map(|group| {
            let binding = try_decode_parameter_rotation_binding(file, groups, group).ok()?;
            let path = find_indexed_path(file, group)?;
            let source_group = groups.get(path.refs.first()?.checked_sub(1)?)?;
            if (source_group.header.kind()) != crate::format::GroupKind::Polygon {
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
                visible: !group.header.is_hidden(),
                binding: Some(ShapeBinding::DerivedTransform {
                    source_index: path.refs.first()?.checked_sub(1)?,
                    transform: ShapeTransformBinding::Rotate(RotationBinding {
                        center_index: binding.center_group_index,
                        angle_degrees: binding_angle_degrees(&binding.kind)?,
                        parameter_name: binding_parameter_name(&binding.kind),
                    }),
                }),
                debug: Some(payload_debug_source(group)),
            })
        })
        .collect()
}

pub(crate) fn collect_transformed_polygon_shapes(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
) -> Vec<PolygonShape> {
    groups
        .iter()
        .filter(|group| (group.header.kind()) == crate::format::GroupKind::Scale)
        .filter_map(|group| {
            let binding = try_decode_transform_binding(file, group).ok()?;
            let TransformBindingKind::Scale { factor } = binding.kind else {
                return None;
            };
            let path = find_indexed_path(file, group)?;
            let source_group = groups.get(path.refs.first()?.checked_sub(1)?)?;
            if (source_group.header.kind()) != crate::format::GroupKind::Polygon {
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
                visible: !group.header.is_hidden(),
                binding: Some(ShapeBinding::DerivedTransform {
                    source_index: path.refs.first()?.checked_sub(1)?,
                    transform: ShapeTransformBinding::Scale(ScaleBinding {
                        center_index: binding.center_group_index,
                        factor,
                    }),
                }),
                debug: Some(payload_debug_source(group)),
            })
        })
        .collect()
}

pub(crate) fn collect_reflected_circle_shapes(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
) -> Vec<CircleShape> {
    let circle_fill_colors = collect_circle_fill_colors(file, groups, anchors);
    groups
        .iter()
        .filter(|group| (group.header.kind()) == crate::format::GroupKind::Reflection)
        .filter_map(|group| {
            let path = find_indexed_path(file, group)?;
            let source_group = groups.get(path.refs.first()?.checked_sub(1)?)?;
            let line_group_index = path.refs.get(1)?.checked_sub(1)?;
            let line_group = groups.get(line_group_index)?;
            let (source_center, source_radius) =
                resolve_circle_points_raw(file, groups, anchors, source_group)?;
            let (line_start, line_end) =
                resolve_line_like_points_raw(file, groups, anchors, line_group)?;
            let center = reflect_across_line(&source_center, &line_start, &line_end)?;
            let radius_point = reflect_across_line(&source_radius, &line_start, &line_end)?;
            Some(CircleShape {
                center,
                radius_point,
                color: color_from_style(source_group.header.style_b),
                fill_color: circle_fill_colors
                    .get(&(path.refs.first()?.checked_sub(1)?))
                    .copied(),
                fill_color_binding: None,
                dashed: line_is_dashed(source_group.header.style_a),
                visible: !group.header.is_hidden(),
                binding: Some(ShapeBinding::DerivedTransform {
                    source_index: path.refs.first()?.checked_sub(1)?,
                    transform: ShapeTransformBinding::Reflect(AxisBinding {
                        line_start_index: None,
                        line_end_index: None,
                        line_index: Some(line_group_index),
                    }),
                }),
                debug: Some(payload_debug_source(group)),
            })
        })
        .collect()
}

pub(crate) fn collect_reflected_polygon_shapes(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
) -> Vec<PolygonShape> {
    groups
        .iter()
        .filter(|group| (group.header.kind()) == crate::format::GroupKind::Reflection)
        .filter_map(|group| {
            let path = find_indexed_path(file, group)?;
            let source_group = groups.get(path.refs.first()?.checked_sub(1)?)?;
            if (source_group.header.kind()) != crate::format::GroupKind::Polygon {
                return None;
            }
            let line_group_index = path.refs.get(1)?.checked_sub(1)?;
            let line_group = groups.get(line_group_index)?;
            let source_path = find_indexed_path(file, source_group)?;
            let (line_start, line_end) =
                resolve_line_like_points_raw(file, groups, anchors, line_group)?;
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
                visible: !group.header.is_hidden(),
                binding: Some(ShapeBinding::DerivedTransform {
                    source_index: path.refs.first()?.checked_sub(1)?,
                    transform: ShapeTransformBinding::Reflect(AxisBinding {
                        line_start_index: None,
                        line_end_index: None,
                        line_index: Some(line_group_index),
                    }),
                }),
                debug: Some(payload_debug_source(group)),
            })
        })
        .collect()
}

fn binding_angle_degrees(binding: &TransformBindingKind) -> Option<f64> {
    match binding {
        TransformBindingKind::Rotate { angle_degrees, .. } => Some(*angle_degrees),
        TransformBindingKind::Scale { .. } => None,
    }
}

fn binding_angle_radians(binding: &TransformBindingKind) -> Option<f64> {
    binding_angle_degrees(binding).map(f64::to_radians)
}

fn binding_parameter_name(binding: &TransformBindingKind) -> Option<String> {
    match binding {
        TransformBindingKind::Rotate { parameter_name, .. } => parameter_name.clone(),
        TransformBindingKind::Scale { .. } => None,
    }
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
