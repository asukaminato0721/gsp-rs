use anyhow::{Context, Result};

use super::{
    CoordinatePoint, GspFile, LegacyCoordinateConstructPoint, ObjectGroup,
    ParameterControlledPoint, PointRecord, RawPointConstraint, TransformBindingKind,
    decode_coordinate_point, decode_custom_transform_binding, decode_expression_offset_binding,
    decode_expression_rotation_binding, decode_iteration_binding_point_alias_raw,
    decode_legacy_coordinate_construct_point, decode_reflection_anchor_raw,
    decode_translated_point_constraint, reflection_line_group_indices,
    regular_polygon_angle_expr_for_calc_group, translation_point_pair_group_indices,
    try_decode_angle_rotation_binding, try_decode_parameter_controlled_point,
    try_decode_parameter_rotation_binding, try_decode_point_constraint,
    try_decode_transform_binding,
};
use crate::runtime::extract::decode::{
    decode_bbox_anchor_raw, decode_label_name, decode_label_visible, is_parameter_control_group,
    try_decode_payload_anchor_point,
};
use crate::runtime::extract::find_indexed_path;
use crate::runtime::extract::points::constraints::CoordinatePointSource;
use crate::runtime::functions::{
    evaluate_expr_with_parameters, try_decode_function_expr, try_decode_function_plot_descriptor,
};
use crate::runtime::geometry::{GraphTransform, angle_degrees_from_points, color_from_style};
use crate::runtime::scene::{
    CircularConstraint, LineConstraint, ScenePoint, ScenePointBinding, ScenePointConstraint,
};

fn mapped_point_index(group_to_point_index: &[Option<usize>], group_index: usize) -> Option<usize> {
    group_to_point_index.get(group_index).copied().flatten()
}

fn group_color(group: &ObjectGroup) -> [u8; 4] {
    color_from_style(group.header.style_b)
}

fn graph_calibration_visible(group: &ObjectGroup) -> bool {
    !group.header.is_hidden() && (group.header.class_id & 0x0004_0000) == 0
}

fn point_marker_visible(group: &ObjectGroup) -> bool {
    (group.header.style_a & 0x0200_0000) != 0
}

fn scene_point(
    position: PointRecord,
    color: [u8; 4],
    visible: bool,
    draggable: bool,
    constraint: ScenePointConstraint,
    binding: Option<ScenePointBinding>,
) -> ScenePoint {
    ScenePoint {
        position,
        color,
        visible,
        draggable,
        constraint,
        binding,
        debug: None,
    }
}

fn build_group_to_point_index(included_groups: &[bool]) -> Vec<Option<usize>> {
    let mut group_to_point_index = vec![None; included_groups.len()];
    let mut point_index = 0usize;
    for (group_index, included) in included_groups.iter().copied().enumerate() {
        if included {
            group_to_point_index[group_index] = Some(point_index);
            point_index += 1;
        }
    }
    group_to_point_index
}

fn build_group_to_line_index(groups: &[ObjectGroup]) -> Vec<Option<usize>> {
    let mut group_to_line_index = vec![None; groups.len()];
    let mut line_index = 0usize;
    for (group_index, group) in groups.iter().enumerate() {
        if group.header.kind().is_rendered_line_group() {
            group_to_line_index[group_index] = Some(line_index);
            line_index += 1;
        }
    }
    group_to_line_index
}

#[allow(clippy::too_many_arguments)]
fn build_scene_point_for_group(
    index: usize,
    group: &ObjectGroup,
    file: &GspFile,
    groups: &[ObjectGroup],
    point_map: &[Option<PointRecord>],
    anchors: &[Option<PointRecord>],
    graph: &Option<GraphTransform>,
    group_to_point_index: &[Option<usize>],
) -> Option<ScenePoint> {
    let kind = group.header.kind();
    let visible = !group.header.is_hidden() && point_marker_visible(group);
    match kind {
        crate::format::GroupKind::Point => (!is_parameter_control_group(group)
            && !is_orphan_duplicate_point_helper(file, groups, group))
        .then(|| point_map.get(index).cloned().flatten())
        .flatten()
        .map(|position| {
            scene_point(
                position,
                group_color(group),
                visible,
                true,
                ScenePointConstraint::Free,
                None,
            )
        }),
        crate::format::GroupKind::GraphCalibrationX
        | crate::format::GroupKind::GraphCalibrationY
        | crate::format::GroupKind::GraphCalibrationYAlt => {
            anchors.get(index).cloned().flatten().map(|position| {
                scene_point(
                    position,
                    group_color(group),
                    visible && graph_calibration_visible(group),
                    true,
                    ScenePointConstraint::Free,
                    Some(ScenePointBinding::GraphCalibration),
                )
            })
        }
        crate::format::GroupKind::LinearIntersectionPoint
        | crate::format::GroupKind::IntersectionPoint1
        | crate::format::GroupKind::IntersectionPoint2
        | crate::format::GroupKind::CircleCircleIntersectionPoint1
        | crate::format::GroupKind::CircleCircleIntersectionPoint2
        | crate::format::GroupKind::CoordinateTraceIntersectionPoint => {
            scene_point_from_intersection(
                index,
                file,
                groups,
                anchors,
                group_to_point_index,
                visible,
            )
        }
        crate::format::GroupKind::Midpoint => {
            scene_point_from_midpoint(index, file, groups, anchors, group_to_point_index, visible)
        }
        crate::format::GroupKind::CartesianOffsetPoint
        | crate::format::GroupKind::PolarOffsetPoint => (|| {
            let constraint = decode_translated_point_constraint(file, group)?;
            let origin_index =
                mapped_point_index(group_to_point_index, constraint.origin_group_index)?;
            let position = anchors.get(index).cloned().flatten()?;
            Some(scene_point(
                position,
                group_color(group),
                visible,
                true,
                ScenePointConstraint::Offset {
                    origin_index,
                    dx: constraint.dx,
                    dy: constraint.dy,
                },
                None,
            ))
        })(),
        crate::format::GroupKind::PointConstraint | crate::format::GroupKind::PathPoint => (|| {
            let constraint =
                try_decode_point_constraint(file, groups, group, Some(anchors), graph).ok()?;
            scene_point_from_constraint(
                index,
                file,
                groups,
                group_color(group),
                anchors,
                group_to_point_index,
                constraint,
                visible,
                kind != crate::format::GroupKind::PathPoint,
            )
        })(
        ),
        crate::format::GroupKind::ParameterControlledPoint => (|| {
            let parameter_point =
                try_decode_parameter_controlled_point(file, groups, group, anchors).ok()?;
            scene_point_from_parameter_controlled(
                file,
                groups,
                group_to_point_index,
                parameter_point,
                group_color(group),
                visible,
            )
        })(),
        crate::format::GroupKind::CoordinatePoint
        | crate::format::GroupKind::CoordinateExpressionPoint
        | crate::format::GroupKind::CoordinateExpressionPointAlt
        | crate::format::GroupKind::FixedCoordinatePoint
        | crate::format::GroupKind::GraphFunctionPoint
        | crate::format::GroupKind::GraphValuePoint
        | crate::format::GroupKind::LegacyCoordinateParameterHelper
        | crate::format::GroupKind::Unknown(20) => (|| {
            let point = decode_coordinate_point(file, groups, group, anchors, graph)?;
            scene_point_from_coordinate(point, group_to_point_index, group_color(group), visible)
        })(),
        crate::format::GroupKind::LegacyCoordinateConstructPoint => (|| {
            let point = decode_legacy_coordinate_construct_point(file, groups, group, anchors)?;
            scene_point_from_legacy_coordinate_construct(
                point,
                group_to_point_index,
                group_color(group),
                visible,
            )
        })(),
        crate::format::GroupKind::CustomTransformPoint => (|| {
            let position = anchors.get(index).cloned().flatten()?;
            let binding = decode_custom_transform_binding(file, groups, group.ordinal)?;
            let source_index =
                mapped_point_index(group_to_point_index, binding.source_group_index)?;
            let origin_index =
                mapped_point_index(group_to_point_index, binding.origin_group_index)?;
            let axis_end_index =
                mapped_point_index(group_to_point_index, binding.axis_end_group_index)?;
            Some(scene_point(
                position,
                group_color(group),
                visible,
                true,
                ScenePointConstraint::Free,
                Some(ScenePointBinding::CustomTransform {
                    source_index,
                    origin_index,
                    axis_end_index,
                    distance_expr: binding.distance_expr,
                    angle_expr: binding.angle_expr,
                    distance_raw_scale: binding.distance_raw_scale,
                    angle_degrees_scale: binding.angle_degrees_scale,
                }),
            ))
        })(),
        crate::format::GroupKind::IterationPointAlias => (|| {
            let alias = decode_iteration_binding_point_alias_raw(file, groups, group, anchors)?;
            let source_index = mapped_point_index(group_to_point_index, alias.source_group_index)?;
            let visible = !group.header.is_hidden() && point_marker_visible(group);
            Some(match alias.kind {
                super::IterationBindingPointAliasKind::Offset { dx, dy } => scene_point(
                    alias.position,
                    group_color(group),
                    visible,
                    false,
                    ScenePointConstraint::Offset {
                        origin_index: source_index,
                        dx,
                        dy,
                    },
                    None,
                ),
                super::IterationBindingPointAliasKind::Rotate {
                    center_group_index,
                    angle_degrees,
                } => {
                    let center_index =
                        mapped_point_index(group_to_point_index, center_group_index)?;
                    scene_point(
                        alias.position,
                        group_color(group),
                        visible,
                        false,
                        ScenePointConstraint::Free,
                        Some(ScenePointBinding::Rotate {
                            source_index,
                            center_index,
                            angle_degrees,
                            parameter_name: None,
                            angle_expr: None,
                            angle_start_index: None,
                            angle_vertex_index: None,
                            angle_end_index: None,
                        }),
                    )
                }
            })
        })(),
        crate::format::GroupKind::Reflection => (|| {
            let position = decode_reflection_anchor_raw(file, groups, group, anchors)?;
            let path = find_indexed_path(file, group)?;
            let source_group_index = path.refs.first()?.checked_sub(1)?;
            let source_index = mapped_point_index(group_to_point_index, source_group_index)?;
            let line_group = groups.get(path.refs.get(1)?.checked_sub(1)?)?;
            let binding = if line_group.header.kind().is_line_like() {
                let (line_start_group_index, line_end_group_index) =
                    reflection_line_group_indices(file, groups, group)?;
                let line_start_index =
                    mapped_point_index(group_to_point_index, line_start_group_index)?;
                let line_end_index =
                    mapped_point_index(group_to_point_index, line_end_group_index)?;
                ScenePointBinding::Reflect {
                    source_index,
                    line_start_index,
                    line_end_index,
                }
            } else {
                let line = resolve_intersection_line_constraint(
                    file,
                    groups,
                    line_group,
                    group_to_point_index,
                )?;
                ScenePointBinding::ReflectLineConstraint { source_index, line }
            };
            Some(scene_point(
                position,
                group_color(group),
                visible,
                true,
                ScenePointConstraint::Free,
                Some(binding),
            ))
        })(),
        crate::format::GroupKind::Translation => (|| {
            let position = anchors.get(index).cloned().flatten()?;
            let path = find_indexed_path(file, group)?;
            let source_group_index = path.refs.first()?.checked_sub(1)?;
            let (vector_start_group_index, vector_end_group_index) =
                translation_point_pair_group_indices(file, group)?;
            let source_index = mapped_point_index(group_to_point_index, source_group_index)?;
            let vector_start_index =
                mapped_point_index(group_to_point_index, vector_start_group_index)?;
            let vector_end_index =
                mapped_point_index(group_to_point_index, vector_end_group_index)?;
            Some(scene_point(
                position,
                group_color(group),
                visible,
                false,
                ScenePointConstraint::Free,
                Some(ScenePointBinding::Translate {
                    source_index,
                    vector_start_index,
                    vector_end_index,
                }),
            ))
        })(),
        crate::format::GroupKind::Rotation
        | crate::format::GroupKind::ParameterRotation
        | crate::format::GroupKind::ExpressionRotation
        | crate::format::GroupKind::Scale => {
            let binding = match kind {
                crate::format::GroupKind::ParameterRotation => {
                    try_decode_parameter_rotation_binding(file, groups, group).ok()
                }
                crate::format::GroupKind::ExpressionRotation => None,
                crate::format::GroupKind::Rotation | crate::format::GroupKind::Scale => {
                    try_decode_transform_binding(file, group).ok()
                }
                _ => None,
            };
            (|| {
                if kind == crate::format::GroupKind::ExpressionRotation {
                    let binding = decode_expression_rotation_binding(file, groups, group, anchors)?;
                    let position = anchors.get(index).cloned().flatten()?;
                    let source_index =
                        mapped_point_index(group_to_point_index, binding.source_group_index)?;
                    let center_index =
                        mapped_point_index(group_to_point_index, binding.center_group_index)?;
                    return Some(scene_point(
                        position,
                        group_color(group),
                        visible,
                        false,
                        ScenePointConstraint::Free,
                        Some(ScenePointBinding::Rotate {
                            source_index,
                            center_index,
                            angle_degrees: binding.angle_degrees,
                            parameter_name: binding.parameter_name,
                            angle_expr: Some(binding.angle_expr),
                            angle_start_index: None,
                            angle_vertex_index: None,
                            angle_end_index: None,
                        }),
                    ));
                }
                let binding = binding?;
                let position = anchors.get(index).cloned().flatten()?;
                let source_index =
                    mapped_point_index(group_to_point_index, binding.source_group_index)?;
                let center_index =
                    mapped_point_index(group_to_point_index, binding.center_group_index)?;
                Some(scene_point(
                    position,
                    group_color(group),
                    visible,
                    false,
                    ScenePointConstraint::Free,
                    Some(match binding.kind {
                        TransformBindingKind::Rotate {
                            angle_degrees,
                            parameter_name,
                        } => ScenePointBinding::Rotate {
                            source_index,
                            center_index,
                            angle_degrees,
                            parameter_name,
                            angle_expr: None,
                            angle_start_index: None,
                            angle_vertex_index: None,
                            angle_end_index: None,
                        },
                        TransformBindingKind::Scale { factor } => ScenePointBinding::Scale {
                            source_index,
                            center_index,
                            factor,
                        },
                    }),
                ))
            })()
        }
        crate::format::GroupKind::ExpressionOffsetPoint => (|| {
            let binding = decode_expression_offset_binding(file, groups, group, anchors)?;
            let position = anchors.get(index).cloned().flatten()?;
            let source_index =
                mapped_point_index(group_to_point_index, binding.source_group_index)?;
            Some(scene_point(
                position,
                group_color(group),
                visible,
                false,
                ScenePointConstraint::Free,
                Some(ScenePointBinding::CoordinateSource {
                    source_index,
                    name: binding.parameter_name.unwrap_or_default(),
                    expr: binding.scaled_expr,
                    axis: crate::runtime::scene::CoordinateAxis::Horizontal,
                }),
            ))
        })(),
        crate::format::GroupKind::AngleRotation => (|| {
            let binding = try_decode_angle_rotation_binding(file, group).ok()?;
            let position = anchors.get(index).cloned().flatten()?;
            let source_index =
                mapped_point_index(group_to_point_index, binding.source_group_index)?;
            let center_index =
                mapped_point_index(group_to_point_index, binding.center_group_index)?;
            let angle_start_index =
                mapped_point_index(group_to_point_index, binding.angle_start_group_index)?;
            let angle_vertex_index =
                mapped_point_index(group_to_point_index, binding.angle_vertex_group_index)?;
            let angle_end_index =
                mapped_point_index(group_to_point_index, binding.angle_end_group_index)?;
            let angle_start = anchors.get(binding.angle_start_group_index)?.clone()?;
            let angle_vertex = anchors.get(binding.angle_vertex_group_index)?.clone()?;
            let angle_end = anchors.get(binding.angle_end_group_index)?.clone()?;
            let angle_degrees = angle_degrees_from_points(&angle_start, &angle_vertex, &angle_end)?;
            Some(scene_point(
                position,
                group_color(group),
                visible,
                false,
                ScenePointConstraint::Free,
                Some(ScenePointBinding::Rotate {
                    source_index,
                    center_index,
                    angle_degrees,
                    parameter_name: None,
                    angle_expr: None,
                    angle_start_index: Some(angle_start_index),
                    angle_vertex_index: Some(angle_vertex_index),
                    angle_end_index: Some(angle_end_index),
                }),
            ))
        })(),
        crate::format::GroupKind::LegacyAngleRotation => (|| {
            let path = find_indexed_path(file, group)?;
            if path.refs.len() < 3 {
                return None;
            }
            let position = anchors.get(index).cloned().flatten()?;
            let source_group_index = path.refs[0].checked_sub(1)?;
            let center_group_index = path.refs[1].checked_sub(1)?;
            let angle_group = groups.get(path.refs[2].checked_sub(1)?)?;
            let angle_path = find_indexed_path(file, angle_group)?;
            if angle_path.refs.len() < 3 {
                return None;
            }
            let source_index = mapped_point_index(group_to_point_index, source_group_index)?;
            let center_index = mapped_point_index(group_to_point_index, center_group_index)?;
            let angle_start_group_index = angle_path.refs[0].checked_sub(1)?;
            let angle_vertex_group_index = angle_path.refs[1].checked_sub(1)?;
            let angle_end_group_index = angle_path.refs[2].checked_sub(1)?;
            let angle_start_index =
                mapped_point_index(group_to_point_index, angle_start_group_index)?;
            let angle_vertex_index =
                mapped_point_index(group_to_point_index, angle_vertex_group_index)?;
            let angle_end_index = mapped_point_index(group_to_point_index, angle_end_group_index)?;
            let angle_start = anchors.get(angle_start_group_index)?.clone()?;
            let angle_vertex = anchors.get(angle_vertex_group_index)?.clone()?;
            let angle_end = anchors.get(angle_end_group_index)?.clone()?;
            let angle_degrees = angle_degrees_from_points(&angle_start, &angle_vertex, &angle_end)?;
            Some(scene_point(
                position,
                group_color(group),
                visible,
                false,
                ScenePointConstraint::Free,
                Some(ScenePointBinding::Rotate {
                    source_index,
                    center_index,
                    angle_degrees,
                    parameter_name: None,
                    angle_expr: None,
                    angle_start_index: Some(angle_start_index),
                    angle_vertex_index: Some(angle_vertex_index),
                    angle_end_index: Some(angle_end_index),
                }),
            ))
        })(),
        crate::format::GroupKind::RatioScale => (|| {
            let position = anchors.get(index).cloned().flatten()?;
            let path = find_indexed_path(file, group)?;
            if path.refs.len() < 5 {
                return None;
            }
            let source_index =
                mapped_point_index(group_to_point_index, path.refs[0].checked_sub(1)?)?;
            let center_index =
                mapped_point_index(group_to_point_index, path.refs[1].checked_sub(1)?)?;
            let ratio_origin_index =
                mapped_point_index(group_to_point_index, path.refs[2].checked_sub(1)?)?;
            let ratio_denominator_index =
                mapped_point_index(group_to_point_index, path.refs[3].checked_sub(1)?)?;
            let ratio_numerator_index =
                mapped_point_index(group_to_point_index, path.refs[4].checked_sub(1)?)?;
            Some(scene_point(
                position,
                group_color(group),
                visible,
                false,
                ScenePointConstraint::Free,
                Some(ScenePointBinding::ScaleByRatio {
                    source_index,
                    center_index,
                    ratio_origin_index,
                    ratio_denominator_index,
                    ratio_numerator_index,
                }),
            ))
        })(),
        _ => None,
    }
}

#[allow(clippy::too_many_arguments)]
fn build_scene_point_for_group_checked(
    index: usize,
    group: &ObjectGroup,
    file: &GspFile,
    groups: &[ObjectGroup],
    point_map: &[Option<PointRecord>],
    anchors: &[Option<PointRecord>],
    graph: &Option<GraphTransform>,
    group_to_point_index: &[Option<usize>],
) -> Result<Option<ScenePoint>> {
    let kind = group.header.kind();
    match kind {
        crate::format::GroupKind::PointConstraint | crate::format::GroupKind::PathPoint => {
            let visible = !group.header.is_hidden() && point_marker_visible(group);
            let Ok(constraint) =
                try_decode_point_constraint(file, groups, group, Some(anchors), graph)
                    .with_context(|| {
                        format!(
                            "failed to decode point constraint for group #{} {:?}",
                            group.ordinal, kind
                        )
                    })
            else {
                return Ok(None);
            };
            Ok(scene_point_from_constraint(
                index,
                file,
                groups,
                group_color(group),
                anchors,
                group_to_point_index,
                constraint,
                visible,
                kind != crate::format::GroupKind::PathPoint,
            ))
        }
        crate::format::GroupKind::Rotation | crate::format::GroupKind::Scale => {
            let visible = !group.header.is_hidden() && point_marker_visible(group);
            let binding = try_decode_transform_binding(file, group).with_context(|| {
                format!(
                    "failed to decode transform binding for group #{} {:?}",
                    group.ordinal, kind
                )
            })?;
            let position = anchors.get(index).cloned().flatten();
            Ok((|| {
                let position = position?;
                let source_index =
                    mapped_point_index(group_to_point_index, binding.source_group_index)?;
                let center_index =
                    mapped_point_index(group_to_point_index, binding.center_group_index)?;
                Some(scene_point(
                    position,
                    group_color(group),
                    visible,
                    false,
                    ScenePointConstraint::Free,
                    Some(match binding.kind {
                        TransformBindingKind::Rotate {
                            angle_degrees,
                            parameter_name,
                        } => ScenePointBinding::Rotate {
                            source_index,
                            center_index,
                            angle_degrees,
                            parameter_name,
                            angle_expr: None,
                            angle_start_index: None,
                            angle_vertex_index: None,
                            angle_end_index: None,
                        },
                        TransformBindingKind::Scale { factor } => ScenePointBinding::Scale {
                            source_index,
                            center_index,
                            factor,
                        },
                    }),
                ))
            })())
        }
        crate::format::GroupKind::ParameterRotation => {
            let visible = !group.header.is_hidden() && point_marker_visible(group);
            let position = anchors.get(index).cloned().flatten();
            Ok((|| {
                let position = position?;
                if let Ok(binding) = try_decode_parameter_rotation_binding(file, groups, group) {
                    let source_index =
                        mapped_point_index(group_to_point_index, binding.source_group_index)?;
                    let center_index =
                        mapped_point_index(group_to_point_index, binding.center_group_index)?;
                    return Some(scene_point(
                        position,
                        group_color(group),
                        visible,
                        false,
                        ScenePointConstraint::Free,
                        Some(match binding.kind {
                            TransformBindingKind::Rotate {
                                angle_degrees,
                                parameter_name,
                            } => ScenePointBinding::Rotate {
                                source_index,
                                center_index,
                                angle_degrees,
                                parameter_name,
                                angle_expr: None,
                                angle_start_index: None,
                                angle_vertex_index: None,
                                angle_end_index: None,
                            },
                            TransformBindingKind::Scale { factor } => ScenePointBinding::Scale {
                                source_index,
                                center_index,
                                factor,
                            },
                        }),
                    ));
                }

                let path = find_indexed_path(file, group)?;
                let source_group_index = path.refs.first()?.checked_sub(1)?;
                let center_group_index = path.refs.get(1)?.checked_sub(1)?;
                let calc_group = groups.get(path.refs.get(2)?.checked_sub(1)?)?;
                if (calc_group.header.kind()) != crate::format::GroupKind::FunctionExpr {
                    return None;
                }
                let source_index = mapped_point_index(group_to_point_index, source_group_index)?;
                let center_index = mapped_point_index(group_to_point_index, center_group_index)?;
                let (angle_expr, parameter_name) = if let Some((angle_expr, parameter_name, _)) =
                    regular_polygon_angle_expr_for_calc_group(file, groups, calc_group)
                {
                    (angle_expr, Some(parameter_name))
                } else {
                    (
                        try_decode_function_expr(file, groups, calc_group).ok()?,
                        None,
                    )
                };
                let angle_degrees = evaluate_expr_with_parameters(
                    &angle_expr,
                    0.0,
                    &std::collections::BTreeMap::new(),
                )?;
                Some(scene_point(
                    position,
                    group_color(group),
                    visible,
                    false,
                    ScenePointConstraint::Free,
                    Some(ScenePointBinding::Rotate {
                        source_index,
                        center_index,
                        angle_degrees,
                        parameter_name,
                        angle_expr: Some(angle_expr),
                        angle_start_index: None,
                        angle_vertex_index: None,
                        angle_end_index: None,
                    }),
                ))
            })())
        }
        crate::format::GroupKind::AngleRotation => {
            let visible = !group.header.is_hidden() && point_marker_visible(group);
            let binding = try_decode_angle_rotation_binding(file, group).with_context(|| {
                format!(
                    "failed to decode angle rotation binding for group #{} {:?}",
                    group.ordinal, kind
                )
            })?;
            let position = anchors.get(index).cloned().flatten();
            Ok((|| {
                let position = position?;
                let source_index =
                    mapped_point_index(group_to_point_index, binding.source_group_index)?;
                let center_index =
                    mapped_point_index(group_to_point_index, binding.center_group_index)?;
                let angle_start_index =
                    mapped_point_index(group_to_point_index, binding.angle_start_group_index)?;
                let angle_vertex_index =
                    mapped_point_index(group_to_point_index, binding.angle_vertex_group_index)?;
                let angle_end_index =
                    mapped_point_index(group_to_point_index, binding.angle_end_group_index)?;
                let angle_start = anchors.get(binding.angle_start_group_index)?.clone()?;
                let angle_vertex = anchors.get(binding.angle_vertex_group_index)?.clone()?;
                let angle_end = anchors.get(binding.angle_end_group_index)?.clone()?;
                let angle_degrees =
                    angle_degrees_from_points(&angle_start, &angle_vertex, &angle_end)?;
                Some(scene_point(
                    position,
                    group_color(group),
                    visible,
                    false,
                    ScenePointConstraint::Free,
                    Some(ScenePointBinding::Rotate {
                        source_index,
                        center_index,
                        angle_degrees,
                        parameter_name: None,
                        angle_expr: None,
                        angle_start_index: Some(angle_start_index),
                        angle_vertex_index: Some(angle_vertex_index),
                        angle_end_index: Some(angle_end_index),
                    }),
                ))
            })())
        }
        crate::format::GroupKind::ParameterControlledPoint => {
            let visible = !group.header.is_hidden() && point_marker_visible(group);
            let Ok(parameter_point) = try_decode_parameter_controlled_point(
                file, groups, group, anchors,
            )
            .with_context(|| {
                format!(
                    "failed to decode parameter-controlled point for group #{} {:?}",
                    group.ordinal, kind
                )
            }) else {
                return Ok(None);
            };
            Ok(scene_point_from_parameter_controlled(
                file,
                groups,
                group_to_point_index,
                parameter_point,
                group_color(group),
                visible,
            ))
        }
        _ => Ok(build_scene_point_for_group(
            index,
            group,
            file,
            groups,
            point_map,
            anchors,
            graph,
            group_to_point_index,
        )),
    }
}

pub(crate) fn collect_visible_points_checked(
    file: &GspFile,
    groups: &[ObjectGroup],
    point_map: &[Option<PointRecord>],
    anchors: &[Option<PointRecord>],
    graph: &Option<GraphTransform>,
) -> Result<(Vec<ScenePoint>, Vec<Option<usize>>)> {
    let mut included_groups = vec![false; groups.len()];

    loop {
        let group_to_point_index = build_group_to_point_index(&included_groups);
        let next_included_groups = groups
            .iter()
            .enumerate()
            .map(|(index, group)| {
                build_scene_point_for_group_checked(
                    index,
                    group,
                    file,
                    groups,
                    point_map,
                    anchors,
                    graph,
                    &group_to_point_index,
                )
                .map(|point| point.is_some())
            })
            .collect::<Result<Vec<_>>>()?;

        if next_included_groups == included_groups {
            let points_by_group = groups
                .iter()
                .enumerate()
                .map(|(index, group)| {
                    build_scene_point_for_group_checked(
                        index,
                        group,
                        file,
                        groups,
                        point_map,
                        anchors,
                        graph,
                        &group_to_point_index,
                    )
                })
                .collect::<Result<Vec<_>>>()?;
            let actual_included_groups = points_by_group
                .iter()
                .map(|point| point.is_some())
                .collect::<Vec<_>>();
            if actual_included_groups == included_groups {
                let final_group_to_point_index =
                    build_group_to_point_index(&actual_included_groups);
                let points = groups
                    .iter()
                    .enumerate()
                    .map(|(index, group)| {
                        build_scene_point_for_group_checked(
                            index,
                            group,
                            file,
                            groups,
                            point_map,
                            anchors,
                            graph,
                            &final_group_to_point_index,
                        )
                    })
                    .collect::<Result<Vec<_>>>()?
                    .into_iter()
                    .flatten()
                    .collect::<Vec<_>>();
                return Ok((points, final_group_to_point_index));
            }
            included_groups = actual_included_groups;
            continue;
        }
        included_groups = next_included_groups;
    }
}

pub(crate) fn collect_standalone_parameter_points(
    file: &GspFile,
    groups: &[ObjectGroup],
) -> Vec<ScenePoint> {
    groups
        .iter()
        .filter(|group| is_parameter_control_group(group))
        .filter_map(|group| standalone_parameter_point(file, group))
        .collect()
}

fn standalone_parameter_point(file: &GspFile, group: &ObjectGroup) -> Option<ScenePoint> {
    let position = try_decode_payload_anchor_point(file, group)
        .ok()
        .flatten()
        .or_else(|| decode_bbox_anchor_raw(file, group))?;
    let binding = decode_label_name(file, group).map(|name| ScenePointBinding::Parameter { name });
    let visible = !group.header.is_hidden() && point_marker_visible(group);
    Some(scene_point(
        position,
        group_color(group),
        visible,
        false,
        ScenePointConstraint::Free,
        binding,
    ))
}

fn is_orphan_duplicate_point_helper(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
) -> bool {
    if (group.header.kind()) != crate::format::GroupKind::Point {
        return false;
    }
    if group
        .records
        .iter()
        .any(|record| record.record_type == 0x0907)
    {
        return false;
    }
    if decode_label_visible(file, group).unwrap_or(true) {
        return false;
    }
    let Some(name) = decode_label_name(file, group) else {
        return false;
    };
    let is_referenced = |ordinal: usize| {
        groups.iter().any(|other| {
            other.ordinal != ordinal
                && find_indexed_path(file, other).is_some_and(|path| path.refs.contains(&ordinal))
        })
    };
    let referenced = is_referenced(group.ordinal);
    if referenced {
        return false;
    }
    groups.iter().any(|other| {
        other.ordinal != group.ordinal
            && decode_label_name(file, other).as_deref() == Some(name.as_str())
            && (is_referenced(other.ordinal)
                || find_indexed_path(file, other).is_some_and(|path| !path.refs.is_empty())
                || other
                    .records
                    .iter()
                    .any(|record| record.record_type == 0x0907))
    })
}

#[allow(clippy::too_many_arguments)]
fn scene_point_from_constraint(
    index: usize,
    file: &GspFile,
    groups: &[ObjectGroup],
    color: [u8; 4],
    anchors: &[Option<PointRecord>],
    group_to_point_index: &[Option<usize>],
    constraint: RawPointConstraint,
    visible: bool,
    draggable: bool,
) -> Option<ScenePoint> {
    let position = anchors.get(index).cloned().flatten()?;
    match constraint {
        RawPointConstraint::Segment(constraint) => {
            let start_index =
                mapped_point_index(group_to_point_index, constraint.start_group_index)?;
            let end_index = mapped_point_index(group_to_point_index, constraint.end_group_index)?;
            let scene_constraint = match constraint.line_like_kind {
                crate::runtime::scene::LineLikeKind::Segment => ScenePointConstraint::OnSegment {
                    start_index,
                    end_index,
                    t: constraint.t,
                },
                crate::runtime::scene::LineLikeKind::Line => ScenePointConstraint::OnLine {
                    start_index,
                    end_index,
                    t: constraint.t,
                },
                crate::runtime::scene::LineLikeKind::Ray => ScenePointConstraint::OnRay {
                    start_index,
                    end_index,
                    t: constraint.t,
                },
            };
            Some(scene_point(
                position,
                color,
                visible,
                draggable,
                scene_constraint,
                None,
            ))
        }
        RawPointConstraint::ConstructedLine {
            host_group_index,
            t,
            line_like_kind,
        } => {
            let line_group = groups.get(host_group_index)?;
            let line = resolve_line_constraint(file, groups, line_group, group_to_point_index)?;
            let scene_constraint = match line_like_kind {
                crate::runtime::scene::LineLikeKind::Line => {
                    ScenePointConstraint::OnLineConstraint { line, t }
                }
                crate::runtime::scene::LineLikeKind::Ray => {
                    ScenePointConstraint::OnRayConstraint { line, t }
                }
                crate::runtime::scene::LineLikeKind::Segment => return None,
            };
            Some(scene_point(
                position,
                color,
                visible,
                draggable,
                scene_constraint,
                None,
            ))
        }
        RawPointConstraint::Polyline {
            function_key,
            points,
            segment_index,
            t,
        } => Some(scene_point(
            position,
            color,
            visible,
            draggable,
            ScenePointConstraint::OnPolyline {
                function_key,
                points,
                segment_index,
                t,
            },
            None,
        )),
        RawPointConstraint::PolygonBoundary {
            vertex_group_indices,
            edge_index,
            t,
        } => {
            let vertex_indices = vertex_group_indices
                .iter()
                .map(|group_index| mapped_point_index(group_to_point_index, *group_index))
                .collect::<Option<Vec<_>>>()?;
            Some(scene_point(
                position,
                color,
                visible,
                draggable,
                ScenePointConstraint::OnPolygonBoundary {
                    vertex_indices,
                    edge_index,
                    t,
                },
                None,
            ))
        }
        RawPointConstraint::Circle(constraint) => {
            let center_index =
                mapped_point_index(group_to_point_index, constraint.center_group_index)?;
            let radius_index =
                mapped_point_index(group_to_point_index, constraint.radius_group_index)?;
            Some(scene_point(
                position,
                color,
                visible,
                draggable,
                ScenePointConstraint::OnCircle {
                    center_index,
                    radius_index,
                    unit_x: constraint.unit_x,
                    unit_y: constraint.unit_y,
                },
                None,
            ))
        }
        RawPointConstraint::Circular(constraint) => {
            let circle_group = groups.get(constraint.circle_group_index)?;
            let circle =
                resolve_circular_constraint(file, groups, circle_group, group_to_point_index)?;
            Some(scene_point(
                position,
                color,
                visible,
                draggable,
                ScenePointConstraint::OnCircularConstraint {
                    circle,
                    unit_x: constraint.unit_x,
                    unit_y: constraint.unit_y,
                },
                None,
            ))
        }
        RawPointConstraint::CircleArc(constraint) => {
            let center_index =
                mapped_point_index(group_to_point_index, constraint.center_group_index)?;
            let start_index =
                mapped_point_index(group_to_point_index, constraint.start_group_index)?;
            let end_index = mapped_point_index(group_to_point_index, constraint.end_group_index)?;
            Some(scene_point(
                position,
                color,
                visible,
                draggable,
                ScenePointConstraint::OnCircleArc {
                    center_index,
                    start_index,
                    end_index,
                    t: constraint.t,
                },
                None,
            ))
        }
        RawPointConstraint::Arc(constraint) => {
            let start_index =
                mapped_point_index(group_to_point_index, constraint.start_group_index)?;
            let mid_index = mapped_point_index(group_to_point_index, constraint.mid_group_index)?;
            let end_index = mapped_point_index(group_to_point_index, constraint.end_group_index)?;
            Some(scene_point(
                position,
                color,
                visible,
                draggable,
                ScenePointConstraint::OnArc {
                    start_index,
                    mid_index,
                    end_index,
                    t: constraint.t,
                },
                None,
            ))
        }
    }
}

fn scene_point_from_parameter_controlled(
    file: &GspFile,
    groups: &[ObjectGroup],
    group_to_point_index: &[Option<usize>],
    parameter_point: ParameterControlledPoint,
    color: [u8; 4],
    visible: bool,
) -> Option<ScenePoint> {
    let binding = parameter_point_binding(group_to_point_index, &parameter_point)?;
    match &parameter_point.constraint {
        RawPointConstraint::Segment(constraint) => {
            let start_index =
                mapped_point_index(group_to_point_index, constraint.start_group_index)?;
            let end_index = mapped_point_index(group_to_point_index, constraint.end_group_index)?;
            let scene_constraint = match constraint.line_like_kind {
                crate::runtime::scene::LineLikeKind::Segment => ScenePointConstraint::OnSegment {
                    start_index,
                    end_index,
                    t: constraint.t,
                },
                crate::runtime::scene::LineLikeKind::Line => ScenePointConstraint::OnLine {
                    start_index,
                    end_index,
                    t: constraint.t,
                },
                crate::runtime::scene::LineLikeKind::Ray => ScenePointConstraint::OnRay {
                    start_index,
                    end_index,
                    t: constraint.t,
                },
            };
            Some(scene_point(
                parameter_point.position.clone(),
                color,
                visible,
                true,
                scene_constraint,
                binding,
            ))
        }
        RawPointConstraint::ConstructedLine {
            host_group_index,
            t,
            line_like_kind,
        } => {
            let line_group = groups.get(*host_group_index)?;
            let line = resolve_line_constraint(file, groups, line_group, group_to_point_index)?;
            let scene_constraint = match line_like_kind {
                crate::runtime::scene::LineLikeKind::Line => {
                    ScenePointConstraint::OnLineConstraint { line, t: *t }
                }
                crate::runtime::scene::LineLikeKind::Ray => {
                    ScenePointConstraint::OnRayConstraint { line, t: *t }
                }
                crate::runtime::scene::LineLikeKind::Segment => return None,
            };
            Some(scene_point(
                parameter_point.position.clone(),
                color,
                visible,
                true,
                scene_constraint,
                binding,
            ))
        }
        RawPointConstraint::PolygonBoundary {
            vertex_group_indices,
            edge_index,
            t,
        } => {
            let vertex_indices = vertex_group_indices
                .iter()
                .map(|group_index| mapped_point_index(group_to_point_index, *group_index))
                .collect::<Option<Vec<_>>>()?;
            Some(scene_point(
                parameter_point.position.clone(),
                color,
                visible,
                true,
                ScenePointConstraint::OnPolygonBoundary {
                    vertex_indices,
                    edge_index: *edge_index,
                    t: *t,
                },
                binding,
            ))
        }
        RawPointConstraint::Circle(constraint) => {
            let center_index =
                mapped_point_index(group_to_point_index, constraint.center_group_index)?;
            let radius_index =
                mapped_point_index(group_to_point_index, constraint.radius_group_index)?;
            Some(scene_point(
                parameter_point.position,
                color,
                visible,
                true,
                ScenePointConstraint::OnCircle {
                    center_index,
                    radius_index,
                    unit_x: constraint.unit_x,
                    unit_y: constraint.unit_y,
                },
                binding,
            ))
        }
        RawPointConstraint::Circular(constraint) => {
            let circle_group = groups.get(constraint.circle_group_index)?;
            let circle =
                resolve_circular_constraint(file, groups, circle_group, group_to_point_index)?;
            Some(scene_point(
                parameter_point.position,
                color,
                visible,
                true,
                ScenePointConstraint::OnCircularConstraint {
                    circle,
                    unit_x: constraint.unit_x,
                    unit_y: constraint.unit_y,
                },
                binding,
            ))
        }
        RawPointConstraint::CircleArc(constraint) => {
            let center_index =
                mapped_point_index(group_to_point_index, constraint.center_group_index)?;
            let start_index =
                mapped_point_index(group_to_point_index, constraint.start_group_index)?;
            let end_index = mapped_point_index(group_to_point_index, constraint.end_group_index)?;
            Some(scene_point(
                parameter_point.position,
                color,
                visible,
                true,
                ScenePointConstraint::OnCircleArc {
                    center_index,
                    start_index,
                    end_index,
                    t: constraint.t,
                },
                binding,
            ))
        }
        RawPointConstraint::Arc(constraint) => {
            let start_index =
                mapped_point_index(group_to_point_index, constraint.start_group_index)?;
            let mid_index = mapped_point_index(group_to_point_index, constraint.mid_group_index)?;
            let end_index = mapped_point_index(group_to_point_index, constraint.end_group_index)?;
            Some(scene_point(
                parameter_point.position,
                color,
                visible,
                true,
                ScenePointConstraint::OnArc {
                    start_index,
                    mid_index,
                    end_index,
                    t: constraint.t,
                },
                binding,
            ))
        }
        RawPointConstraint::Polyline {
            function_key,
            points,
            segment_index,
            t,
        } => Some(scene_point(
            parameter_point.position,
            color,
            visible,
            true,
            ScenePointConstraint::OnPolyline {
                function_key: *function_key,
                points: points.clone(),
                segment_index: *segment_index,
                t: *t,
            },
            binding,
        )),
    }
}

fn parameter_point_binding(
    group_to_point_index: &[Option<usize>],
    parameter_point: &ParameterControlledPoint,
) -> Option<Option<ScenePointBinding>> {
    if let Some(expr) = &parameter_point.source_expr {
        if let Some(source_group_index) = parameter_point.source_point_group_index {
            let source_index = mapped_point_index(group_to_point_index, source_group_index)?;
            return Some(Some(ScenePointBinding::ConstraintParameterFromPointExpr {
                source_index,
                parameter_name: parameter_point.parameter_name.clone(),
                expr: expr.clone(),
            }));
        }
        return Some(Some(ScenePointBinding::ConstraintParameterExpr {
            expr: expr.clone(),
        }));
    }
    if let Some(source_group_index) = parameter_point.source_point_group_index {
        let source_index = mapped_point_index(group_to_point_index, source_group_index)?;
        Some(Some(ScenePointBinding::DerivedParameter { source_index }))
    } else {
        Some(
            (!parameter_point.parameter_name.is_empty()).then(|| ScenePointBinding::Parameter {
                name: parameter_point.parameter_name.clone(),
            }),
        )
    }
}

fn scene_point_from_coordinate(
    point: CoordinatePoint,
    group_to_point_index: &[Option<usize>],
    color: [u8; 4],
    visible: bool,
) -> Option<ScenePoint> {
    let binding = match point.source {
        CoordinatePointSource::Parameter(name) => ScenePointBinding::Coordinate {
            name,
            expr: point.expr,
        },
        CoordinatePointSource::SourcePoint {
            source_group_index,
            parameter_name,
            axis,
        } => ScenePointBinding::CoordinateSource {
            source_index: mapped_point_index(group_to_point_index, source_group_index)?,
            name: parameter_name,
            expr: point.expr,
            axis,
        },
        CoordinatePointSource::SourcePoint2d {
            source_group_index,
            x_parameter_name,
            x_expr,
            y_parameter_name,
            y_expr,
        } => ScenePointBinding::CoordinateSource2d {
            source_index: mapped_point_index(group_to_point_index, source_group_index)?,
            x_name: x_parameter_name,
            x_expr,
            y_name: y_parameter_name,
            y_expr,
        },
    };
    Some(scene_point(
        point.position,
        color,
        visible,
        true,
        ScenePointConstraint::Free,
        Some(binding),
    ))
}

fn scene_point_from_legacy_coordinate_construct(
    point: LegacyCoordinateConstructPoint,
    group_to_point_index: &[Option<usize>],
    color: [u8; 4],
    visible: bool,
) -> Option<ScenePoint> {
    let first_source_index =
        mapped_point_index(group_to_point_index, point.first_source_group_index)?;
    let second_source_index =
        mapped_point_index(group_to_point_index, point.second_source_group_index)?;
    let first_axis_start_index =
        mapped_point_index(group_to_point_index, point.first_axis_start_group_index)?;
    let first_axis_end_index =
        mapped_point_index(group_to_point_index, point.first_axis_end_group_index)?;
    let second_axis_start_index =
        mapped_point_index(group_to_point_index, point.second_axis_start_group_index)?;
    let second_axis_end_index =
        mapped_point_index(group_to_point_index, point.second_axis_end_group_index)?;
    Some(scene_point(
        point.position,
        color,
        visible,
        true,
        ScenePointConstraint::LineIntersection {
            left: LineConstraint::ParallelLine {
                through_index: first_source_index,
                line_start_index: first_axis_start_index,
                line_end_index: first_axis_end_index,
            },
            right: LineConstraint::ParallelLine {
                through_index: second_source_index,
                line_start_index: second_axis_start_index,
                line_end_index: second_axis_end_index,
            },
        },
        None,
    ))
}

fn scene_point_from_midpoint(
    index: usize,
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
    group_to_point_index: &[Option<usize>],
    visible: bool,
) -> Option<ScenePoint> {
    let group = groups.get(index)?;
    let path = find_indexed_path(file, group)?;
    let host_group = groups.get(path.refs.first()?.checked_sub(1)?)?;
    if !matches!(
        host_group.header.kind(),
        crate::format::GroupKind::Segment
            | crate::format::GroupKind::Line
            | crate::format::GroupKind::Ray
    ) {
        return None;
    }
    let host_path = find_indexed_path(file, host_group)?;
    let start_index = (*group_to_point_index.get(host_path.refs.first()?.checked_sub(1)?)?)?;
    let end_index = (*group_to_point_index.get(host_path.refs.get(1)?.checked_sub(1)?)?)?;
    let position = anchors.get(index).cloned().flatten()?;
    Some(scene_point(
        position,
        group_color(group),
        visible,
        true,
        ScenePointConstraint::OnSegment {
            start_index,
            end_index,
            t: 0.5,
        },
        Some(ScenePointBinding::Midpoint {
            start_index,
            end_index,
        }),
    ))
}

fn scene_point_from_intersection(
    index: usize,
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
    group_to_point_index: &[Option<usize>],
    visible: bool,
) -> Option<ScenePoint> {
    let group = groups.get(index)?;
    let path = find_indexed_path(file, group)?;
    if path.refs.len() != 2 {
        return None;
    }

    let left_group = groups.get(path.refs[0].checked_sub(1)?)?;
    let right_group = groups.get(path.refs[1].checked_sub(1)?)?;
    let position = anchors.get(index).cloned().flatten()?;

    if let (Some(left), Some(right)) = (
        resolve_intersection_line_constraint(file, groups, left_group, group_to_point_index),
        resolve_intersection_line_constraint(file, groups, right_group, group_to_point_index),
    ) {
        return Some(scene_point(
            position,
            group_color(group),
            visible,
            true,
            ScenePointConstraint::LineIntersection { left, right },
            None,
        ));
    }

    if let (Some(line), Some((point_index, x_min, x_max, sample_count))) = (
        resolve_intersection_line_constraint(file, groups, left_group, group_to_point_index),
        decode_coordinate_trace_constraint(file, groups, right_group, group_to_point_index),
    ) {
        return Some(scene_point(
            position,
            group_color(group),
            visible,
            true,
            ScenePointConstraint::LineTraceIntersection {
                line,
                point_index,
                x_min,
                x_max,
                sample_count,
            },
            None,
        ));
    }

    if let (Some((point_index, x_min, x_max, sample_count)), Some(line)) = (
        decode_coordinate_trace_constraint(file, groups, left_group, group_to_point_index),
        resolve_intersection_line_constraint(file, groups, right_group, group_to_point_index),
    ) {
        return Some(scene_point(
            position,
            group_color(group),
            visible,
            true,
            ScenePointConstraint::LineTraceIntersection {
                line,
                point_index,
                x_min,
                x_max,
                sample_count,
            },
            None,
        ));
    }

    let variant = intersection_variant(group.header.kind());
    let left_circular = resolve_circular_constraint(file, groups, left_group, group_to_point_index);
    let right_circular =
        resolve_circular_constraint(file, groups, right_group, group_to_point_index);
    if let (Some(line), Some((center_index, radius_index))) = (
        resolve_intersection_line_constraint(file, groups, left_group, group_to_point_index),
        resolve_circle_point_indices(file, groups, right_group, group_to_point_index),
    ) {
        return Some(scene_point(
            position,
            group_color(group),
            visible,
            true,
            ScenePointConstraint::LineCircleIntersection {
                line,
                center_index,
                radius_index,
                variant,
            },
            None,
        ));
    }

    if let (Some((center_index, radius_index)), Some(line)) = (
        resolve_circle_point_indices(file, groups, left_group, group_to_point_index),
        resolve_intersection_line_constraint(file, groups, right_group, group_to_point_index),
    ) {
        return Some(scene_point(
            position,
            group_color(group),
            visible,
            true,
            ScenePointConstraint::LineCircleIntersection {
                line,
                center_index,
                radius_index,
                variant,
            },
            None,
        ));
    }

    if let (Some(line), Some(circle)) = (
        resolve_intersection_line_constraint(file, groups, left_group, group_to_point_index),
        right_circular.clone(),
    ) {
        return Some(scene_point(
            position,
            group_color(group),
            visible,
            true,
            ScenePointConstraint::LineCircularIntersection {
                line,
                circle,
                variant,
            },
            None,
        ));
    }

    if let (Some(circle), Some(line)) = (
        left_circular.clone(),
        resolve_intersection_line_constraint(file, groups, right_group, group_to_point_index),
    ) {
        return Some(scene_point(
            position,
            group_color(group),
            visible,
            true,
            ScenePointConstraint::LineCircularIntersection {
                line,
                circle,
                variant,
            },
            None,
        ));
    }

    if let (Some(point_index), Some(circle)) = (
        mapped_point_index(group_to_point_index, path.refs[0].checked_sub(1)?),
        right_circular.clone(),
    ) {
        return Some(scene_point(
            position,
            group_color(group),
            visible,
            true,
            ScenePointConstraint::PointCircularTangent {
                point_index,
                circle,
                variant,
            },
            None,
        ));
    }

    if let (Some(circle), Some(point_index)) = (
        left_circular.clone(),
        mapped_point_index(group_to_point_index, path.refs[1].checked_sub(1)?),
    ) {
        return Some(scene_point(
            position,
            group_color(group),
            visible,
            true,
            ScenePointConstraint::PointCircularTangent {
                point_index,
                circle,
                variant,
            },
            None,
        ));
    }

    if let (Some(left), Some(right)) = (left_circular, right_circular) {
        if let (
            CircularConstraint::Circle {
                center_index: left_center_index,
                radius_index: left_radius_index,
            },
            CircularConstraint::Circle {
                center_index: right_center_index,
                radius_index: right_radius_index,
            },
        ) = (&left, &right)
        {
            return Some(scene_point(
                position,
                group_color(group),
                visible,
                true,
                ScenePointConstraint::CircleCircleIntersection {
                    left_center_index: *left_center_index,
                    left_radius_index: *left_radius_index,
                    right_center_index: *right_center_index,
                    right_radius_index: *right_radius_index,
                    variant,
                },
                None,
            ));
        }

        return Some(scene_point(
            position,
            group_color(group),
            visible,
            true,
            ScenePointConstraint::CircularIntersection {
                left,
                right,
                variant,
            },
            None,
        ));
    }

    Some(scene_point(
        position,
        group_color(group),
        visible,
        true,
        ScenePointConstraint::Free,
        None,
    ))
}

fn resolve_intersection_line_constraint(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
    group_to_point_index: &[Option<usize>],
) -> Option<LineConstraint> {
    resolve_line_constraint(file, groups, group, group_to_point_index)
}

fn resolve_line_constraint(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
    group_to_point_index: &[Option<usize>],
) -> Option<LineConstraint> {
    let path = find_indexed_path(file, group)?;
    match group.header.kind() {
        crate::format::GroupKind::Segment
        | crate::format::GroupKind::Line
        | crate::format::GroupKind::Ray
        | crate::format::GroupKind::MeasurementLine
        | crate::format::GroupKind::AxisLine => {
            if path.refs.len() != 2 {
                return None;
            }
            let start_index = (*group_to_point_index.get(path.refs[0].checked_sub(1)?)?)?;
            let end_index = (*group_to_point_index.get(path.refs[1].checked_sub(1)?)?)?;
            Some(match group.header.kind() {
                crate::format::GroupKind::Segment => LineConstraint::Segment {
                    start_index,
                    end_index,
                },
                crate::format::GroupKind::Ray => LineConstraint::Ray {
                    start_index,
                    end_index,
                },
                _ => LineConstraint::Line {
                    start_index,
                    end_index,
                },
            })
        }
        crate::format::GroupKind::LineKind5 | crate::format::GroupKind::LineKind6 => {
            if path.refs.len() != 2 {
                return None;
            }
            let through_index = (*group_to_point_index.get(path.refs[0].checked_sub(1)?)?)?;
            let host_group = groups.get(path.refs[1].checked_sub(1)?)?;
            let host_path = find_indexed_path(file, host_group)?;
            if host_path.refs.len() != 2 {
                return None;
            }
            let line_start_index =
                (*group_to_point_index.get(host_path.refs[0].checked_sub(1)?)?)?;
            let line_end_index = (*group_to_point_index.get(host_path.refs[1].checked_sub(1)?)?)?;
            Some(match group.header.kind() {
                crate::format::GroupKind::LineKind5 => LineConstraint::PerpendicularLine {
                    through_index,
                    line_start_index,
                    line_end_index,
                },
                crate::format::GroupKind::LineKind6 => LineConstraint::ParallelLine {
                    through_index,
                    line_start_index,
                    line_end_index,
                },
                _ => unreachable!(),
            })
        }
        crate::format::GroupKind::LineKind7 => {
            if path.refs.len() != 3 {
                return None;
            }
            Some(LineConstraint::AngleBisectorRay {
                start_index: (*group_to_point_index.get(path.refs[0].checked_sub(1)?)?)?,
                vertex_index: (*group_to_point_index.get(path.refs[1].checked_sub(1)?)?)?,
                end_index: (*group_to_point_index.get(path.refs[2].checked_sub(1)?)?)?,
            })
        }
        crate::format::GroupKind::Translation => {
            if path.refs.len() < 3 {
                return None;
            }
            let source_group = groups.get(path.refs[0].checked_sub(1)?)?;
            let line = resolve_line_constraint(file, groups, source_group, group_to_point_index)?;
            Some(LineConstraint::Translated {
                line: Box::new(line),
                vector_start_index: (*group_to_point_index.get(path.refs[1].checked_sub(1)?)?)?,
                vector_end_index: (*group_to_point_index.get(path.refs[2].checked_sub(1)?)?)?,
            })
        }
        _ => None,
    }
}

fn decode_coordinate_trace_constraint(
    file: &GspFile,
    _groups: &[ObjectGroup],
    group: &ObjectGroup,
    group_to_point_index: &[Option<usize>],
) -> Option<(usize, f64, f64, usize)> {
    if (group.header.kind()) != crate::format::GroupKind::CoordinateTrace {
        return None;
    }

    let path = find_indexed_path(file, group)?;
    if path.refs.len() < 3 {
        return None;
    }
    let point_index = (*group_to_point_index.get(path.refs[0].checked_sub(1)?)?)?;
    let payload = group
        .records
        .iter()
        .find(|record| record.record_type == 0x0902)
        .map(|record| record.payload(&file.data))?;
    let descriptor = try_decode_function_plot_descriptor(payload).ok()?;
    Some((
        point_index,
        descriptor.x_min,
        descriptor.x_max,
        descriptor.sample_count,
    ))
}

fn resolve_circle_point_indices(
    file: &GspFile,
    _groups: &[ObjectGroup],
    group: &ObjectGroup,
    group_to_point_index: &[Option<usize>],
) -> Option<(usize, usize)> {
    let path = find_indexed_path(file, group)?;
    match group.header.kind() {
        crate::format::GroupKind::Circle => {
            if path.refs.len() != 2 {
                return None;
            }
            let center_index = (*group_to_point_index.get(path.refs[0].checked_sub(1)?)?)?;
            let radius_index = (*group_to_point_index.get(path.refs[1].checked_sub(1)?)?)?;
            Some((center_index, radius_index))
        }
        _ => None,
    }
}

fn resolve_circular_constraint(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
    group_to_point_index: &[Option<usize>],
) -> Option<CircularConstraint> {
    let path = find_indexed_path(file, group)?;
    match group.header.kind() {
        crate::format::GroupKind::Circle => {
            if path.refs.len() != 2 {
                return None;
            }
            Some(CircularConstraint::Circle {
                center_index: (*group_to_point_index.get(path.refs[0].checked_sub(1)?)?)?,
                radius_index: (*group_to_point_index.get(path.refs[1].checked_sub(1)?)?)?,
            })
        }
        crate::format::GroupKind::CircleCenterRadius => {
            if path.refs.len() != 2 {
                return None;
            }
            let segment_group = groups.get(path.refs[1].checked_sub(1)?)?;
            let segment_path = find_indexed_path(file, segment_group)?;
            if segment_path.refs.len() != 2 {
                return None;
            }
            Some(CircularConstraint::SegmentRadiusCircle {
                center_index: (*group_to_point_index.get(path.refs[0].checked_sub(1)?)?)?,
                line_start_index: (*group_to_point_index
                    .get(segment_path.refs[0].checked_sub(1)?)?)?,
                line_end_index: (*group_to_point_index
                    .get(segment_path.refs[1].checked_sub(1)?)?)?,
            })
        }
        crate::format::GroupKind::CartesianOffsetPoint
        | crate::format::GroupKind::PolarOffsetPoint => {
            let constraint = decode_translated_point_constraint(file, group)?;
            let source_group = groups.get(constraint.origin_group_index)?;
            let source =
                resolve_circular_constraint(file, groups, source_group, group_to_point_index)?;
            Some(CircularConstraint::TranslateCircle {
                source: Box::new(source),
                dx: constraint.dx,
                dy: constraint.dy,
            })
        }
        crate::format::GroupKind::Reflection => {
            let source_group = groups.get(path.refs.first()?.checked_sub(1)?)?;
            let source =
                resolve_circular_constraint(file, groups, source_group, group_to_point_index)?;
            let line_group_index = path.refs.get(1)?.checked_sub(1)?;
            let group_to_line_index = build_group_to_line_index(groups);
            Some(CircularConstraint::ReflectCircle {
                source: Box::new(source),
                line_start_index: None,
                line_end_index: None,
                line_index: group_to_line_index.get(line_group_index).copied().flatten(),
            })
        }
        crate::format::GroupKind::Scale => {
            let binding = try_decode_transform_binding(file, group).ok()?;
            let TransformBindingKind::Scale { factor } = binding.kind else {
                return None;
            };
            let source_group = groups.get(binding.source_group_index)?;
            let source =
                resolve_circular_constraint(file, groups, source_group, group_to_point_index)?;
            let center_index =
                mapped_point_index(group_to_point_index, binding.center_group_index)?;
            Some(CircularConstraint::ScaleCircle {
                source: Box::new(source),
                center_index,
                factor,
            })
        }
        crate::format::GroupKind::CenterArc => {
            if path.refs.len() != 3 {
                return None;
            }
            Some(CircularConstraint::CircleArc {
                center_index: (*group_to_point_index.get(path.refs[0].checked_sub(1)?)?)?,
                start_index: (*group_to_point_index.get(path.refs[1].checked_sub(1)?)?)?,
                end_index: (*group_to_point_index.get(path.refs[2].checked_sub(1)?)?)?,
            })
        }
        crate::format::GroupKind::ArcOnCircle => {
            if path.refs.len() != 3 {
                return None;
            }
            let circle_group = groups.get(path.refs[0].checked_sub(1)?)?;
            let circle_path = find_indexed_path(file, circle_group)?;
            if circle_path.refs.len() != 2 {
                return None;
            }
            Some(CircularConstraint::CircleArc {
                center_index: (*group_to_point_index.get(circle_path.refs[0].checked_sub(1)?)?)?,
                start_index: (*group_to_point_index.get(path.refs[1].checked_sub(1)?)?)?,
                end_index: (*group_to_point_index.get(path.refs[2].checked_sub(1)?)?)?,
            })
        }
        crate::format::GroupKind::ThreePointArc => {
            if path.refs.len() != 3 {
                return None;
            }
            Some(CircularConstraint::ThreePointArc {
                start_index: (*group_to_point_index.get(path.refs[0].checked_sub(1)?)?)?,
                mid_index: (*group_to_point_index.get(path.refs[1].checked_sub(1)?)?)?,
                end_index: (*group_to_point_index.get(path.refs[2].checked_sub(1)?)?)?,
            })
        }
        _ => None,
    }
}

fn intersection_variant(kind: crate::format::GroupKind) -> usize {
    match kind {
        crate::format::GroupKind::IntersectionPoint1
        | crate::format::GroupKind::CircleCircleIntersectionPoint1 => 1,
        _ => 0,
    }
}
