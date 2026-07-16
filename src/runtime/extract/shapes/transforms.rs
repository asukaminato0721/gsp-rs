use super::basic::resolve_arc_boundary_points;
use super::{
    ArcShape, CircleShape, GspFile, LineBinding, LineShape, ObjectGroup, PointRecord, PolygonShape,
    SceneContext, ShapeBinding, TransformBindingKind, collect_circle_fill_colors, color_from_style,
    fill_color_from_styles, find_indexed_path, is_circle_group_kind, line_is_dashed,
    line_stroke_width_from_style, payload_debug_source, reflect_across_line, rotate_around,
    scale_around, translation_point_pair_group_indices, try_decode_transform_binding,
};
use crate::runtime::extract::decode::resolve_circle_points_raw;
use crate::runtime::extract::points::{
    TransformBinding, decode_parameter_rotation_transform_binding_raw, expression_runtime_context,
    resolve_line_like_points_raw, scale_angle_expr_to_degrees,
};
use crate::runtime::functions::{FunctionExpr, function_parameter_group_ordinals};
use crate::runtime::geometry::{angle_degrees_from_points, from_core_point, to_core_point};
use crate::runtime::scene::{
    ArcBinding, AxisBinding, GeometryTransformBinding, RatioScaleBinding, RotationBinding,
    ScaleBinding,
};

fn arc_shape_raw(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
    group: &ObjectGroup,
    stack: &mut Vec<usize>,
) -> Option<([PointRecord; 3], Option<PointRecord>, bool)> {
    let group_index = group.ordinal.checked_sub(1)?;
    if stack.contains(&group_index) {
        return None;
    }
    stack.push(group_index);
    let result = match group.header.kind() {
        crate::format::GroupKind::ThreePointArc => {
            let path = find_indexed_path(file, group)?;
            let [start, mid, end] = path.refs.as_slice() else {
                return None;
            };
            Some((
                [
                    anchors.get(start.checked_sub(1)?)?.clone()?,
                    anchors.get(mid.checked_sub(1)?)?.clone()?,
                    anchors.get(end.checked_sub(1)?)?.clone()?,
                ],
                None,
                false,
            ))
        }
        crate::format::GroupKind::CenterArc => {
            let path = find_indexed_path(file, group)?;
            let [center, start, end] = path.refs.as_slice() else {
                return None;
            };
            let center = anchors.get(center.checked_sub(1)?)?.clone()?;
            let start = anchors.get(start.checked_sub(1)?)?.clone()?;
            let end = anchors.get(end.checked_sub(1)?)?.clone()?;
            Some((
                crate::runtime::geometry::arc_on_circle_control_points(&center, &start, &end)?,
                Some(center),
                true,
            ))
        }
        crate::format::GroupKind::ArcOnCircle => {
            let path = find_indexed_path(file, group)?;
            let [circle, start, end] = path.refs.as_slice() else {
                return None;
            };
            let circle_group = groups.get(circle.checked_sub(1)?)?;
            let (center, _) = resolve_circle_points_raw(file, groups, anchors, circle_group)?;
            let start = anchors.get(start.checked_sub(1)?)?.clone()?;
            let end = anchors.get(end.checked_sub(1)?)?.clone()?;
            Some((
                crate::runtime::geometry::arc_on_circle_control_points(&center, &start, &end)?,
                Some(center),
                true,
            ))
        }
        crate::format::GroupKind::Reflection => {
            let path = find_indexed_path(file, group)?;
            let [source, axis] = path.refs.as_slice() else {
                return None;
            };
            let source_group = groups.get(source.checked_sub(1)?)?;
            let axis_group = groups.get(axis.checked_sub(1)?)?;
            let (axis_start, axis_end) =
                resolve_line_like_points_raw(file, groups, anchors, axis_group)?;
            let (points, center, counterclockwise) =
                arc_shape_raw(file, groups, anchors, source_group, stack)?;
            Some((
                points
                    .map(|point| reflect_across_line(&point, &axis_start, &axis_end))
                    .into_iter()
                    .collect::<Option<Vec<_>>>()?
                    .try_into()
                    .ok()?,
                match center {
                    Some(point) => Some(reflect_across_line(&point, &axis_start, &axis_end)?),
                    None => None,
                },
                !counterclockwise,
            ))
        }
        _ => None,
    };
    stack.pop();
    result
}

fn reflection_axis_binding(
    file: &GspFile,
    axis_group: &ObjectGroup,
    axis_group_index: usize,
) -> AxisBinding {
    if matches!(
        axis_group.header.kind(),
        crate::format::GroupKind::Segment
            | crate::format::GroupKind::Line
            | crate::format::GroupKind::Ray
            | crate::format::GroupKind::MeasurementLine
            | crate::format::GroupKind::GraphMeasurementSegment
    ) && let Some(path) = find_indexed_path(file, axis_group)
        && let (Some(start_index), Some(end_index)) = (
            path.refs.first().and_then(|ordinal| ordinal.checked_sub(1)),
            path.refs.get(1).and_then(|ordinal| ordinal.checked_sub(1)),
        )
    {
        return AxisBinding {
            line_start_index: Some(start_index),
            line_end_index: Some(end_index),
            line_index: None,
        };
    }
    AxisBinding {
        line_start_index: None,
        line_end_index: None,
        line_index: Some(axis_group_index),
    }
}

pub(crate) fn collect_reflected_arc_shapes(
    file: &GspFile,
    groups: &[ObjectGroup],
    context: &SceneContext<'_>,
    anchors: &[Option<PointRecord>],
) -> Vec<ArcShape> {
    groups
        .iter()
        .filter(|group| group.header.kind() == crate::format::GroupKind::Reflection)
        .filter_map(|group| {
            let path = context.indexed_path(group)?;
            let source_group_index = context.path_ref_group_index(path, 0)?;
            let source_group = context.path_ref_group(path, 0)?;
            if !matches!(
                source_group.header.kind(),
                crate::format::GroupKind::ThreePointArc
                    | crate::format::GroupKind::CenterArc
                    | crate::format::GroupKind::ArcOnCircle
                    | crate::format::GroupKind::Reflection
            ) {
                return None;
            }
            let line_group_index = context.path_ref_group_index(path, 1)?;
            let line_group = context.path_ref_group(path, 1)?;
            let (points, center, counterclockwise) =
                arc_shape_raw(file, groups, anchors, group, &mut Vec::new())?;
            Some(ArcShape {
                points,
                color: color_from_style(group.header.style_b),
                center,
                counterclockwise,
                visible: !group.header.is_hidden(),
                binding: Some(ArcBinding::MatrixApply {
                    source_index: source_group_index,
                    matrices: vec![GeometryTransformBinding::Reflect(reflection_axis_binding(
                        file,
                        line_group,
                        line_group_index,
                    ))],
                }),
                debug: Some(payload_debug_source(group)),
            })
        })
        .collect()
}

struct RotationTransform {
    binding: TransformBinding,
    angle_group_indices: Option<(usize, usize, usize)>,
    angle_expr: Option<FunctionExpr>,
    angle_parameter_group_ordinals: std::collections::BTreeMap<String, usize>,
}

fn rotation_transform_for_group(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
    anchors: &[Option<PointRecord>],
) -> Option<RotationTransform> {
    if group.header.kind() == crate::format::GroupKind::AngleRotation {
        let binding =
            crate::runtime::extract::points::try_decode_angle_rotation_binding(file, group).ok()?;
        let angle_start = anchors.get(binding.angle_start_group_index)?.clone()?;
        let angle_vertex = anchors.get(binding.angle_vertex_group_index)?.clone()?;
        let angle_end = anchors.get(binding.angle_end_group_index)?.clone()?;
        let angle_degrees = angle_degrees_from_points(&angle_start, &angle_vertex, &angle_end)?;
        return Some(RotationTransform {
            binding: TransformBinding {
                source_group_index: binding.source_group_index,
                center_group_index: binding.center_group_index,
                kind: TransformBindingKind::Rotate {
                    angle_degrees,
                    parameter_name: None,
                },
            },
            angle_group_indices: Some((
                binding.angle_start_group_index,
                binding.angle_vertex_group_index,
                binding.angle_end_group_index,
            )),
            angle_expr: None,
            angle_parameter_group_ordinals: std::collections::BTreeMap::new(),
        });
    }
    if group.header.kind() == crate::format::GroupKind::ParameterRotation {
        let path = find_indexed_path(file, group)?;
        let angle_group = groups.get(path.refs.get(2)?.checked_sub(1)?)?;
        let binding =
            decode_parameter_rotation_transform_binding_raw(file, groups, group, anchors)?;
        let angle_group_indices =
            if angle_group.header.kind() == crate::format::GroupKind::AngleValue {
                let angle_path = find_indexed_path(file, angle_group)?;
                Some((
                    angle_path.refs.first()?.checked_sub(1)?,
                    angle_path.refs.get(1)?.checked_sub(1)?,
                    angle_path.refs.get(2)?.checked_sub(1)?,
                ))
            } else {
                None
            };
        let (angle_expr, angle_parameter_group_ordinals) =
            if angle_group.header.kind() == crate::format::GroupKind::FunctionExpr {
                let (expr, _, _) = expression_runtime_context(file, groups, angle_group, anchors)?;
                let expr = if crate::runtime::functions::function_expr_uses_degree_units(
                    file,
                    groups,
                    angle_group,
                ) {
                    expr
                } else {
                    scale_angle_expr_to_degrees(expr)
                };
                (
                    Some(expr),
                    function_parameter_group_ordinals(file, groups, angle_group),
                )
            } else {
                (None, std::collections::BTreeMap::new())
            };
        return Some(RotationTransform {
            binding,
            angle_group_indices,
            angle_expr,
            angle_parameter_group_ordinals,
        });
    }

    Some(RotationTransform {
        binding: try_decode_transform_binding(file, group).ok()?,
        angle_group_indices: None,
        angle_expr: None,
        angle_parameter_group_ordinals: std::collections::BTreeMap::new(),
    })
}

fn rotation_binding(transform: &RotationTransform, center_index: usize) -> Option<RotationBinding> {
    let binding = &transform.binding.kind;
    Some(RotationBinding {
        center_index,
        angle_degrees: binding_angle_degrees(binding)?,
        parameter_name: binding_parameter_name(binding),
        angle_expr: transform.angle_expr.clone(),
        angle_parameter_group_ordinals: transform.angle_parameter_group_ordinals.clone(),
        angle_start_index: transform.angle_group_indices.map(|indices| indices.0),
        angle_vertex_index: transform.angle_group_indices.map(|indices| indices.1),
        angle_end_index: transform.angle_group_indices.map(|indices| indices.2),
    })
}

pub(crate) fn collect_rotated_line_shapes(
    file: &GspFile,
    groups: &[ObjectGroup],
    context: &SceneContext<'_>,
    anchors: &[Option<PointRecord>],
) -> Vec<LineShape> {
    groups
        .iter()
        .filter(|group| {
            matches!(
                group.header.kind(),
                crate::format::GroupKind::Rotation
                    | crate::format::GroupKind::ParameterRotation
                    | crate::format::GroupKind::AngleRotation
            )
        })
        .filter_map(|group| {
            let transform = rotation_transform_for_group(file, groups, group, anchors)?;
            let binding = &transform.binding;
            let path = context.indexed_path(group)?;
            let source_group_index = context.path_ref_group_index(path, 0)?;
            let source_group = context.path_ref_group(path, 0)?;
            let center = anchors.get(binding.center_group_index)?.clone()?;
            let radians = binding_angle_radians(&binding.kind)?;
            let (start, end) = resolve_line_like_points_raw(file, groups, anchors, source_group)?;
            let points = [start, end]
                .into_iter()
                .map(|point| rotate_around(&point, &center, radians))
                .collect::<Vec<_>>();
            derived_line_shape(
                group,
                source_group_index,
                source_group,
                points,
                GeometryTransformBinding::Rotate(rotation_binding(
                    &transform,
                    binding.center_group_index,
                )?),
            )
        })
        .collect()
}

pub(crate) fn collect_translated_line_shapes(
    file: &GspFile,
    groups: &[ObjectGroup],
    context: &SceneContext<'_>,
    anchors: &[Option<PointRecord>],
) -> Vec<LineShape> {
    groups
        .iter()
        .filter(|group| (group.header.kind()) == crate::format::GroupKind::Translation)
        .filter_map(|group| {
            let path = context.indexed_path(group)?;
            let source_group_index = context.path_ref_group_index(path, 0)?;
            let source_group = context.path_ref_group(path, 0)?;
            if (source_group.header.kind()) != crate::format::GroupKind::Segment {
                return None;
            }
            let (dx, dy, vector_start_index, vector_end_index) =
                translation_delta(file, group, anchors)?;
            let points = source_path_points(context, anchors, source_group)?
                .into_iter()
                .map(|point| PointRecord {
                    x: point.x + dx,
                    y: point.y + dy,
                })
                .collect::<Vec<_>>();
            derived_line_shape(
                group,
                source_group_index,
                source_group,
                points,
                GeometryTransformBinding::TranslateVector {
                    vector_start_index,
                    vector_end_index,
                },
            )
        })
        .collect()
}

pub(crate) fn collect_scaled_line_shapes(
    file: &GspFile,
    groups: &[ObjectGroup],
    context: &SceneContext<'_>,
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
            let path = context.indexed_path(group)?;
            let source_group_index = context.path_ref_group_index(path, 0)?;
            let source_group = context.path_ref_group(path, 0)?;
            if (source_group.header.kind()) != crate::format::GroupKind::Segment {
                return None;
            }
            let center = anchors.get(binding.center_group_index)?.clone()?;
            let points = source_path_points(context, anchors, source_group)?
                .into_iter()
                .map(|point| scale_around(&point, &center, factor))
                .collect::<Vec<_>>();
            derived_line_shape(
                group,
                source_group_index,
                source_group,
                points,
                GeometryTransformBinding::Scale(ScaleBinding {
                    center_index: binding.center_group_index,
                    factor,
                }),
            )
        })
        .collect()
}

pub(crate) fn collect_reflected_line_shapes(
    file: &GspFile,
    groups: &[ObjectGroup],
    context: &SceneContext<'_>,
    anchors: &[Option<PointRecord>],
) -> Vec<LineShape> {
    groups
        .iter()
        .filter(|group| group.header.kind() == crate::format::GroupKind::Reflection)
        .filter_map(|group| {
            let path = context.indexed_path(group)?;
            let source_group_index = context.path_ref_group_index(path, 0)?;
            let source_group = context.path_ref_group(path, 0)?;
            let line_group_index = context.path_ref_group_index(path, 1)?;
            let line_group = context.path_ref_group(path, 1)?;
            let (line_start, line_end) =
                resolve_line_like_points_raw(file, groups, anchors, line_group)?;
            let (source_start, source_end) =
                resolve_line_like_points_raw(file, groups, anchors, source_group)?;
            let points = [source_start, source_end]
                .into_iter()
                .filter_map(|point| reflect_across_line(&point, &line_start, &line_end))
                .collect::<Vec<_>>();
            derived_line_shape(
                group,
                source_group_index,
                source_group,
                points,
                GeometryTransformBinding::Reflect(AxisBinding {
                    line_start_index: None,
                    line_end_index: None,
                    line_index: Some(line_group_index),
                }),
            )
        })
        .collect()
}

pub(crate) fn collect_rotated_circle_shapes(
    file: &GspFile,
    groups: &[ObjectGroup],
    context: &SceneContext<'_>,
    anchors: &[Option<PointRecord>],
) -> Vec<CircleShape> {
    let circle_fill_colors = collect_circle_fill_colors(file, groups, anchors);
    groups
        .iter()
        .filter(|group| (group.header.kind()) == crate::format::GroupKind::ParameterRotation)
        .filter_map(|group| {
            let transform = rotation_transform_for_group(file, groups, group, anchors)?;
            let binding = &transform.binding;
            let path = context.indexed_path(group)?;
            let source_group_index = context.path_ref_group_index(path, 0)?;
            let source_group = context.path_ref_group(path, 0)?;
            let center = anchors.get(binding.center_group_index)?.clone()?;
            let radians = binding_angle_radians(&binding.kind)?;
            let (source_center, source_radius) =
                resolve_circle_points_raw(file, groups, anchors, source_group)?;
            let source_fill = circle_fill_colors.get(&source_group_index);
            Some(derived_circle_shape(
                group,
                source_group_index,
                source_group,
                source_group,
                rotate_around(&source_center, &center, radians),
                rotate_around(&source_radius, &center, radians),
                source_fill,
                GeometryTransformBinding::Rotate(rotation_binding(
                    &transform,
                    binding.center_group_index,
                )?),
            ))
        })
        .collect()
}

pub(crate) fn collect_translated_circle_shapes(
    file: &GspFile,
    groups: &[ObjectGroup],
    context: &SceneContext<'_>,
    anchors: &[Option<PointRecord>],
) -> Vec<CircleShape> {
    let circle_fill_colors = collect_circle_fill_colors(file, groups, anchors);
    groups
        .iter()
        .filter(|group| {
            matches!(
                group.header.kind(),
                crate::format::GroupKind::Translation
                    | crate::format::GroupKind::CartesianOffsetPoint
                    | crate::format::GroupKind::PolarOffsetPoint
            )
        })
        .filter_map(|group| {
            if group.header.kind() == crate::format::GroupKind::Translation {
                let path = context.indexed_path(group)?;
                let source_group_index = context.path_ref_group_index(path, 0)?;
                let source_group = context.path_ref_group(path, 0)?;
                if !is_circle_group_kind(source_group.header.kind()) {
                    return None;
                }
                let (dx, dy, vector_start_index, vector_end_index) =
                    translation_delta(file, group, anchors)?;
                let (source_center, source_radius) =
                    resolve_circle_points_raw(file, groups, anchors, source_group)?;
                let source_fill = circle_fill_colors.get(&source_group_index);
                return Some(derived_circle_shape(
                    group,
                    source_group_index,
                    source_group,
                    group,
                    PointRecord {
                        x: source_center.x + dx,
                        y: source_center.y + dy,
                    },
                    PointRecord {
                        x: source_radius.x + dx,
                        y: source_radius.y + dy,
                    },
                    source_fill,
                    GeometryTransformBinding::TranslateVector {
                        vector_start_index,
                        vector_end_index,
                    },
                ));
            }
            let constraint = super::decode_translated_point_constraint(file, group)?;
            let source_group_index = constraint.origin_group_index;
            let source_group = groups.get(source_group_index)?;
            let (source_center, source_radius) =
                resolve_circle_points_raw(file, groups, anchors, source_group)?;
            let source_fill = circle_fill_colors.get(&source_group_index);
            Some(derived_circle_shape(
                group,
                source_group_index,
                source_group,
                source_group,
                PointRecord {
                    x: source_center.x + constraint.dx,
                    y: source_center.y + constraint.dy,
                },
                PointRecord {
                    x: source_radius.x + constraint.dx,
                    y: source_radius.y + constraint.dy,
                },
                source_fill,
                GeometryTransformBinding::TranslateDelta {
                    dx: constraint.dx,
                    dy: constraint.dy,
                },
            ))
        })
        .collect()
}

pub(crate) fn collect_translated_polygon_shapes(
    file: &GspFile,
    groups: &[ObjectGroup],
    context: &SceneContext<'_>,
    anchors: &[Option<PointRecord>],
) -> Vec<PolygonShape> {
    groups
        .iter()
        .filter(|group| (group.header.kind()) == crate::format::GroupKind::Translation)
        .filter_map(|group| {
            let path = context.indexed_path(group)?;
            let source_group_index = context.path_ref_group_index(path, 0)?;
            let source_group = context.path_ref_group(path, 0)?;
            let (dx, dy, vector_start_index, vector_end_index) =
                translation_delta(file, group, anchors)?;
            let points = polygon_points_raw(file, groups, context, anchors, source_group)?
                .into_iter()
                .map(|point| PointRecord {
                    x: point.x + dx,
                    y: point.y + dy,
                })
                .collect::<Vec<_>>();
            derived_polygon_shape(
                group,
                source_group_index,
                points,
                GeometryTransformBinding::TranslateVector {
                    vector_start_index,
                    vector_end_index,
                },
            )
        })
        .collect()
}

pub(crate) fn collect_transformed_circle_shapes(
    file: &GspFile,
    groups: &[ObjectGroup],
    context: &SceneContext<'_>,
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
            let path = context.indexed_path(group)?;
            let source_group_index = context.path_ref_group_index(path, 0)?;
            let source_group = context.path_ref_group(path, 0)?;
            let scale_center = anchors.get(binding.center_group_index)?.clone()?;
            let (source_center, source_radius) =
                resolve_circle_points_raw(file, groups, anchors, source_group)?;
            let source_fill = circle_fill_colors.get(&source_group_index);
            Some(derived_circle_shape(
                group,
                source_group_index,
                source_group,
                source_group,
                scale_around(&source_center, &scale_center, factor),
                scale_around(&source_radius, &scale_center, factor),
                source_fill,
                GeometryTransformBinding::Scale(ScaleBinding {
                    center_index: binding.center_group_index,
                    factor,
                }),
            ))
        })
        .collect()
}

pub(crate) fn collect_rotated_polygon_shapes(
    file: &GspFile,
    groups: &[ObjectGroup],
    context: &SceneContext<'_>,
    anchors: &[Option<PointRecord>],
) -> Vec<PolygonShape> {
    groups
        .iter()
        .filter(|group| {
            matches!(
                group.header.kind(),
                crate::format::GroupKind::Rotation | crate::format::GroupKind::ParameterRotation
            )
        })
        .filter_map(|group| {
            let transform = rotation_transform_for_group(file, groups, group, anchors)?;
            let binding = &transform.binding;
            let path = context.indexed_path(group)?;
            let source_group_index = context.path_ref_group_index(path, 0)?;
            let source_group = context.path_ref_group(path, 0)?;
            let center = anchors.get(binding.center_group_index)?.clone()?;
            let radians = binding_angle_radians(&binding.kind)?;
            let points = polygon_points_raw(file, groups, context, anchors, source_group)?
                .into_iter()
                .map(|point| rotate_around(&point, &center, radians))
                .collect::<Vec<_>>();
            derived_polygon_shape(
                group,
                source_group_index,
                points,
                GeometryTransformBinding::Rotate(rotation_binding(
                    &transform,
                    binding.center_group_index,
                )?),
            )
        })
        .collect()
}

pub(crate) fn collect_transformed_polygon_shapes(
    file: &GspFile,
    groups: &[ObjectGroup],
    context: &SceneContext<'_>,
    anchors: &[Option<PointRecord>],
) -> Vec<PolygonShape> {
    groups
        .iter()
        .filter(|group| {
            matches!(
                group.header.kind(),
                crate::format::GroupKind::Scale | crate::format::GroupKind::RatioScale
            )
        })
        .filter_map(|group| {
            if group.header.kind() == crate::format::GroupKind::RatioScale {
                let path = context.indexed_path(group)?;
                let source_group_index = context.path_ref_group_index(path, 0)?;
                let source_group = context.path_ref_group(path, 0)?;
                let center_index = context.path_ref_group_index(path, 1)?;
                let ratio_origin_index = context.path_ref_group_index(path, 2)?;
                let ratio_denominator_index = context.path_ref_group_index(path, 3)?;
                let ratio_numerator_index = context.path_ref_group_index(path, 4)?;
                let center = anchors.get(center_index)?.clone()?;
                let ratio_origin = anchors.get(ratio_origin_index)?.clone()?;
                let ratio_denominator = anchors.get(ratio_denominator_index)?.clone()?;
                let ratio_numerator = anchors.get(ratio_numerator_index)?.clone()?;
                let points = polygon_points_raw(file, groups, context, anchors, source_group)?
                    .into_iter()
                    .map(|point| {
                        gsp_runtime_core::scale_by_three_point_ratio(
                            to_core_point(&point),
                            to_core_point(&center),
                            to_core_point(&ratio_origin),
                            to_core_point(&ratio_denominator),
                            to_core_point(&ratio_numerator),
                            true,
                            false,
                        )
                        .map(from_core_point)
                    })
                    .collect::<Option<Vec<PointRecord>>>()?;
                return derived_polygon_shape(
                    group,
                    source_group_index,
                    points,
                    GeometryTransformBinding::ScaleByRatio(RatioScaleBinding {
                        center_index,
                        ratio_origin_index,
                        ratio_denominator_index,
                        ratio_numerator_index,
                        signed: true,
                        clamp_to_unit: false,
                    }),
                );
            }
            let binding = try_decode_transform_binding(file, group).ok()?;
            let TransformBindingKind::Scale { factor } = binding.kind else {
                return None;
            };
            let path = context.indexed_path(group)?;
            let source_group_index = context.path_ref_group_index(path, 0)?;
            let source_group = context.path_ref_group(path, 0)?;
            let scale_center = anchors.get(binding.center_group_index)?.clone()?;
            let points = polygon_points_raw(file, groups, context, anchors, source_group)?
                .into_iter()
                .map(|point| scale_around(&point, &scale_center, factor))
                .collect::<Vec<_>>();
            derived_polygon_shape(
                group,
                source_group_index,
                points,
                GeometryTransformBinding::Scale(ScaleBinding {
                    center_index: binding.center_group_index,
                    factor,
                }),
            )
        })
        .collect()
}

pub(crate) fn collect_reflected_circle_shapes(
    file: &GspFile,
    groups: &[ObjectGroup],
    context: &SceneContext<'_>,
    anchors: &[Option<PointRecord>],
) -> Vec<CircleShape> {
    let circle_fill_colors = collect_circle_fill_colors(file, groups, anchors);
    groups
        .iter()
        .filter(|group| (group.header.kind()) == crate::format::GroupKind::Reflection)
        .filter_map(|group| {
            let path = context.indexed_path(group)?;
            let source_group_index = context.path_ref_group_index(path, 0)?;
            let source_group = context.path_ref_group(path, 0)?;
            let line_group_index = context.path_ref_group_index(path, 1)?;
            let line_group = context.path_ref_group(path, 1)?;
            let (source_center, source_radius) =
                resolve_circle_points_raw(file, groups, anchors, source_group)?;
            let (line_start, line_end) =
                resolve_line_like_points_raw(file, groups, anchors, line_group)?;
            let center = reflect_across_line(&source_center, &line_start, &line_end)?;
            let radius_point = reflect_across_line(&source_radius, &line_start, &line_end)?;
            let source_fill = circle_fill_colors.get(&source_group_index);
            Some(derived_circle_shape(
                group,
                source_group_index,
                source_group,
                source_group,
                center,
                radius_point,
                source_fill,
                GeometryTransformBinding::Reflect(reflection_axis_binding(
                    file,
                    line_group,
                    line_group_index,
                )),
            ))
        })
        .collect()
}

pub(crate) fn collect_reflected_polygon_shapes(
    file: &GspFile,
    groups: &[ObjectGroup],
    context: &SceneContext<'_>,
    anchors: &[Option<PointRecord>],
) -> Vec<PolygonShape> {
    groups
        .iter()
        .filter(|group| (group.header.kind()) == crate::format::GroupKind::Reflection)
        .filter_map(|group| {
            let path = context.indexed_path(group)?;
            let source_group_index = context.path_ref_group_index(path, 0)?;
            let source_group = context.path_ref_group(path, 0)?;
            let line_group_index = context.path_ref_group_index(path, 1)?;
            let line_group = context.path_ref_group(path, 1)?;
            let (line_start, line_end) =
                resolve_line_like_points_raw(file, groups, anchors, line_group)?;
            let points = polygon_points_raw(file, groups, context, anchors, source_group)?
                .into_iter()
                .filter_map(|point| reflect_across_line(&point, &line_start, &line_end))
                .collect::<Vec<_>>();
            derived_polygon_shape(
                group,
                source_group_index,
                points,
                GeometryTransformBinding::Reflect(reflection_axis_binding(
                    file,
                    line_group,
                    line_group_index,
                )),
            )
        })
        .collect()
}

fn source_path_points(
    context: &SceneContext<'_>,
    anchors: &[Option<PointRecord>],
    source_group: &ObjectGroup,
) -> Option<Vec<PointRecord>> {
    Some(
        context
            .indexed_path(source_group)?
            .refs
            .iter()
            .filter_map(|object_ref| anchors.get(object_ref.saturating_sub(1)).cloned().flatten())
            .collect(),
    )
}

pub(super) fn polygon_points_raw(
    file: &GspFile,
    groups: &[ObjectGroup],
    context: &SceneContext<'_>,
    anchors: &[Option<PointRecord>],
    group: &ObjectGroup,
) -> Option<Vec<PointRecord>> {
    polygon_points_raw_inner(file, groups, context, anchors, group, &mut Vec::new())
}

fn polygon_points_raw_inner(
    file: &GspFile,
    groups: &[ObjectGroup],
    context: &SceneContext<'_>,
    anchors: &[Option<PointRecord>],
    group: &ObjectGroup,
    stack: &mut Vec<usize>,
) -> Option<Vec<PointRecord>> {
    let group_index = group.ordinal.checked_sub(1)?;
    if stack.contains(&group_index) {
        return None;
    }
    stack.push(group_index);
    let result = match group.header.kind() {
        crate::format::GroupKind::Polygon => source_path_points(context, anchors, group),
        crate::format::GroupKind::SectorBoundary
        | crate::format::GroupKind::CircularSegmentBoundary => {
            resolve_arc_boundary_points(file, groups, anchors, group)
        }
        crate::format::GroupKind::Translation => {
            let path = context.indexed_path(group)?;
            let source_group = context.path_ref_group(path, 0)?;
            let (dx, dy, _, _) = translation_delta(file, group, anchors)?;
            Some(
                polygon_points_raw_inner(file, groups, context, anchors, source_group, stack)?
                    .into_iter()
                    .map(|point| PointRecord {
                        x: point.x + dx,
                        y: point.y + dy,
                    })
                    .collect(),
            )
        }
        crate::format::GroupKind::Rotation | crate::format::GroupKind::ParameterRotation => {
            let binding = if group.header.kind() == crate::format::GroupKind::ParameterRotation {
                decode_parameter_rotation_transform_binding_raw(file, groups, group, anchors)?
            } else {
                try_decode_transform_binding(file, group).ok()?
            };
            let source_group = groups.get(binding.source_group_index)?;
            let center = anchors.get(binding.center_group_index)?.clone()?;
            let radians = binding_angle_radians(&binding.kind)?;
            Some(
                polygon_points_raw_inner(file, groups, context, anchors, source_group, stack)?
                    .into_iter()
                    .map(|point| rotate_around(&point, &center, radians))
                    .collect(),
            )
        }
        crate::format::GroupKind::Scale => {
            let binding = try_decode_transform_binding(file, group).ok()?;
            let TransformBindingKind::Scale { factor } = binding.kind else {
                return None;
            };
            let source_group = groups.get(binding.source_group_index)?;
            let center = anchors.get(binding.center_group_index)?.clone()?;
            Some(
                polygon_points_raw_inner(file, groups, context, anchors, source_group, stack)?
                    .into_iter()
                    .map(|point| scale_around(&point, &center, factor))
                    .collect(),
            )
        }
        crate::format::GroupKind::RatioScale => {
            let path = context.indexed_path(group)?;
            let source_group = context.path_ref_group(path, 0)?;
            let center = anchors
                .get(context.path_ref_group_index(path, 1)?)?
                .clone()?;
            let ratio_origin = anchors
                .get(context.path_ref_group_index(path, 2)?)?
                .clone()?;
            let ratio_denominator = anchors
                .get(context.path_ref_group_index(path, 3)?)?
                .clone()?;
            let ratio_numerator = anchors
                .get(context.path_ref_group_index(path, 4)?)?
                .clone()?;
            Some(
                polygon_points_raw_inner(file, groups, context, anchors, source_group, stack)?
                    .into_iter()
                    .map(|point| {
                        gsp_runtime_core::scale_by_three_point_ratio(
                            to_core_point(&point),
                            to_core_point(&center),
                            to_core_point(&ratio_origin),
                            to_core_point(&ratio_denominator),
                            to_core_point(&ratio_numerator),
                            true,
                            false,
                        )
                        .map(from_core_point)
                    })
                    .collect::<Option<Vec<PointRecord>>>()?,
            )
        }
        crate::format::GroupKind::Reflection => {
            let path = context.indexed_path(group)?;
            let source_group = context.path_ref_group(path, 0)?;
            let line_group = context.path_ref_group(path, 1)?;
            let (line_start, line_end) =
                resolve_line_like_points_raw(file, groups, anchors, line_group)?;
            Some(
                polygon_points_raw_inner(file, groups, context, anchors, source_group, stack)?
                    .into_iter()
                    .filter_map(|point| reflect_across_line(&point, &line_start, &line_end))
                    .collect(),
            )
        }
        _ => None,
    };
    stack.pop();
    result.filter(|points| points.len() >= 3)
}

fn derived_line_shape(
    group: &ObjectGroup,
    source_group_index: usize,
    source_group: &ObjectGroup,
    points: Vec<PointRecord>,
    transform: GeometryTransformBinding,
) -> Option<LineShape> {
    (points.len() >= 2).then_some(LineShape {
        points,
        color: color_from_style(source_group.header.style_b),
        dashed: line_is_dashed(source_group.header.style_a),
        stroke_width: Some(line_stroke_width_from_style(source_group.header.style_a)),
        visible: !group.header.is_hidden(),
        binding: Some(LineBinding::MatrixApply {
            source_index: Some(source_group_index),
            source_start_index: None,
            source_end_index: None,
            matrices: vec![transform],
        }),
        debug: Some(payload_debug_source(group)),
    })
}

fn derived_circle_shape(
    group: &ObjectGroup,
    source_group_index: usize,
    source_group: &ObjectGroup,
    color_group: &ObjectGroup,
    center: PointRecord,
    radius_point: PointRecord,
    source_fill: Option<&([u8; 4], bool)>,
    transform: GeometryTransformBinding,
) -> CircleShape {
    CircleShape {
        center,
        radius_point,
        color: color_from_style(color_group.header.style_b),
        fill_color: source_fill.map(|fill| fill.0),
        fill_visible: source_fill.is_some_and(|fill| fill.1),
        fill_color_binding: None,
        dashed: line_is_dashed(source_group.header.style_a),
        visible: !group.header.is_hidden(),
        binding: Some(ShapeBinding::MatrixApply {
            source_index: source_group_index,
            matrices: vec![transform],
        }),
        debug: Some(payload_debug_source(group)),
    }
}

fn derived_polygon_shape(
    group: &ObjectGroup,
    source_group_index: usize,
    points: Vec<PointRecord>,
    transform: GeometryTransformBinding,
) -> Option<PolygonShape> {
    (points.len() >= 3).then_some(PolygonShape {
        points,
        color: fill_color_from_styles(group.header.style_b, group.header.style_c),
        color_binding: None,
        visible: !group.header.is_hidden(),
        binding: Some(ShapeBinding::MatrixApply {
            source_index: source_group_index,
            matrices: vec![transform],
        }),
        debug: Some(payload_debug_source(group)),
    })
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
