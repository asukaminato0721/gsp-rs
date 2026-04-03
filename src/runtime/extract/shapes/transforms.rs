use super::{
    CircleShape, GspFile, LineBinding, LineShape, ObjectGroup, PointRecord, PolygonShape,
    ShapeBinding, TransformBindingKind, color_from_style, decode_parameter_rotation_binding,
    decode_transform_binding, fill_color_from_styles, find_indexed_path, has_distinct_points,
    reflect_across_line, reflection_line_group_indices, rotate_around, scale_around,
    translation_point_pair_group_indices,
};

pub(crate) fn collect_rotated_line_shapes(
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
                    parameter_name: binding_parameter_name(&binding.kind),
                }),
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

pub(crate) fn collect_reflected_line_shapes(
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

pub(crate) fn collect_rotated_circle_shapes(
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
                    parameter_name: binding_parameter_name(&binding.kind),
                }),
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

pub(crate) fn collect_transformed_circle_shapes(
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

pub(crate) fn collect_rotated_polygon_shapes(
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
                    parameter_name: binding_parameter_name(&binding.kind),
                }),
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

pub(crate) fn collect_reflected_circle_shapes(
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

pub(crate) fn collect_reflected_polygon_shapes(
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
