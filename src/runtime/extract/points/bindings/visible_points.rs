use super::{
    CoordinatePoint, GspFile, ObjectGroup, ParameterControlledPoint, PointRecord,
    RawPointConstraint, TransformBindingKind, decode_coordinate_point,
    decode_custom_transform_binding, decode_parameter_controlled_point,
    decode_parameter_rotation_binding, decode_point_constraint, decode_reflection_anchor_raw,
    decode_transform_binding, decode_translated_point_constraint, reflection_line_group_indices,
    translation_point_pair_group_indices,
};
use crate::runtime::extract::decode::{decode_label_name, decode_label_visible};
use crate::runtime::extract::find_indexed_path;
use crate::runtime::extract::points::constraints::CoordinatePointSource;
use crate::runtime::functions::decode_function_plot_descriptor;
use crate::runtime::geometry::{GraphTransform, color_from_style};
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
    }
}

pub(crate) fn collect_visible_points(
    file: &GspFile,
    groups: &[ObjectGroup],
    point_map: &[Option<PointRecord>],
    anchors: &[Option<PointRecord>],
    graph: &Option<GraphTransform>,
) -> (Vec<ScenePoint>, Vec<Option<usize>>) {
    let mut group_to_point_index = vec![None; groups.len()];
    let mut points = Vec::<ScenePoint>::new();

    for (index, group) in groups.iter().enumerate() {
        let kind = group.header.kind();
        let visible = !group.header.is_hidden();
        let scene_point = match kind {
            crate::format::GroupKind::Point => {
                (!is_orphan_duplicate_point_helper(file, groups, group))
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
                    })
            }
            crate::format::GroupKind::GraphCalibrationX
            | crate::format::GroupKind::GraphCalibrationY => {
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
                    &group_to_point_index,
                    visible,
                )
            }
            crate::format::GroupKind::Midpoint => scene_point_from_midpoint(
                index,
                file,
                groups,
                anchors,
                &group_to_point_index,
                visible,
            ),
            crate::format::GroupKind::CartesianOffsetPoint
            | crate::format::GroupKind::PolarOffsetPoint => (|| {
                let constraint = decode_translated_point_constraint(file, group)?;
                let origin_index =
                    mapped_point_index(&group_to_point_index, constraint.origin_group_index)?;
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
            crate::format::GroupKind::PointConstraint | crate::format::GroupKind::PathPoint => {
                (|| {
                    let constraint =
                        decode_point_constraint(file, groups, group, Some(anchors), graph)?;
                    scene_point_from_constraint(
                        index,
                        group_color(group),
                        anchors,
                        &group_to_point_index,
                        constraint,
                        visible,
                        kind != crate::format::GroupKind::PathPoint,
                    )
                })()
            }
            crate::format::GroupKind::ParameterControlledPoint => (|| {
                let parameter_point =
                    decode_parameter_controlled_point(file, groups, group, anchors)?;
                scene_point_from_parameter_controlled(
                    &group_to_point_index,
                    parameter_point,
                    group_color(group),
                    visible,
                )
            })(),
            crate::format::GroupKind::CoordinatePoint
            | crate::format::GroupKind::CoordinateExpressionPoint
            | crate::format::GroupKind::CoordinateExpressionPointAlt
            | crate::format::GroupKind::Unknown(20) => (|| {
                let point = decode_coordinate_point(file, groups, group, anchors, graph)?;
                scene_point_from_coordinate(
                    point,
                    &group_to_point_index,
                    group_color(group),
                    visible,
                )
            })(),
            crate::format::GroupKind::CustomTransformPoint => (|| {
                let position = anchors.get(index).cloned().flatten()?;
                let binding = decode_custom_transform_binding(file, groups, group.ordinal)?;
                let source_index =
                    mapped_point_index(&group_to_point_index, binding.source_group_index)?;
                let origin_index =
                    mapped_point_index(&group_to_point_index, binding.origin_group_index)?;
                let axis_end_index =
                    mapped_point_index(&group_to_point_index, binding.axis_end_group_index)?;
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
            crate::format::GroupKind::Reflection => (|| {
                let position = decode_reflection_anchor_raw(file, groups, group, anchors)?;
                let path = find_indexed_path(file, group)?;
                let source_group_index = path.refs.first()?.checked_sub(1)?;
                let (line_start_group_index, line_end_group_index) =
                    reflection_line_group_indices(file, groups, group)?;
                let source_index = mapped_point_index(&group_to_point_index, source_group_index)?;
                let line_start_index =
                    mapped_point_index(&group_to_point_index, line_start_group_index)?;
                let line_end_index =
                    mapped_point_index(&group_to_point_index, line_end_group_index)?;
                let source_group = groups.get(source_group_index)?;
                ((source_group.header.kind()) == crate::format::GroupKind::Point).then(|| {
                    scene_point(
                        position,
                        group_color(group),
                        visible,
                        true,
                        ScenePointConstraint::Free,
                        Some(ScenePointBinding::Reflect {
                            source_index,
                            line_start_index,
                            line_end_index,
                        }),
                    )
                })
            })(),
            crate::format::GroupKind::Translation => (|| {
                let position = anchors.get(index).cloned().flatten()?;
                let path = find_indexed_path(file, group)?;
                let source_group_index = path.refs.first()?.checked_sub(1)?;
                let (vector_start_group_index, vector_end_group_index) =
                    translation_point_pair_group_indices(file, group)?;
                let source_index = mapped_point_index(&group_to_point_index, source_group_index)?;
                let vector_start_index =
                    mapped_point_index(&group_to_point_index, vector_start_group_index)?;
                let vector_end_index =
                    mapped_point_index(&group_to_point_index, vector_end_group_index)?;
                let source_group = groups.get(source_group_index)?;
                ((source_group.header.kind()) == crate::format::GroupKind::Point).then(|| {
                    scene_point(
                        position,
                        group_color(group),
                        visible,
                        true,
                        ScenePointConstraint::Free,
                        Some(ScenePointBinding::Translate {
                            source_index,
                            vector_start_index,
                            vector_end_index,
                        }),
                    )
                })
            })(),
            crate::format::GroupKind::Rotation
            | crate::format::GroupKind::ParameterRotation
            | crate::format::GroupKind::Scale => {
                let binding = if kind == crate::format::GroupKind::ParameterRotation {
                    decode_parameter_rotation_binding(file, groups, group)
                } else {
                    decode_transform_binding(file, group)
                };
                (|| {
                    let binding = binding?;
                    let position = anchors.get(index).cloned().flatten()?;
                    let source_index =
                        mapped_point_index(&group_to_point_index, binding.source_group_index)?;
                    let center_index =
                        mapped_point_index(&group_to_point_index, binding.center_group_index)?;
                    Some(scene_point(
                        position,
                        group_color(group),
                        visible,
                        true,
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
            _ => None,
        };

        if let Some(scene_point) = scene_point {
            group_to_point_index[index] = Some(points.len());
            points.push(scene_point);
        }
    }

    (points, group_to_point_index)
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

fn scene_point_from_constraint(
    index: usize,
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
            Some(scene_point(
                position,
                color,
                visible,
                draggable,
                ScenePointConstraint::OnSegment {
                    start_index,
                    end_index,
                    t: constraint.t,
                },
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
            Some(scene_point(
                parameter_point.position.clone(),
                color,
                visible,
                true,
                ScenePointConstraint::OnSegment {
                    start_index,
                    end_index,
                    t: constraint.t,
                },
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
        resolve_line_constraint(file, groups, left_group, group_to_point_index),
        resolve_line_constraint(file, groups, right_group, group_to_point_index),
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
        resolve_line_constraint(file, groups, left_group, group_to_point_index),
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
        resolve_line_constraint(file, groups, right_group, group_to_point_index),
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
    if let (Some(line), Some((center_index, radius_index))) = (
        resolve_line_constraint(file, groups, left_group, group_to_point_index),
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
        resolve_line_constraint(file, groups, right_group, group_to_point_index),
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

    if let (Some(left), Some(right)) = (
        resolve_circular_constraint(file, groups, left_group, group_to_point_index),
        resolve_circular_constraint(file, groups, right_group, group_to_point_index),
    ) {
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
    let descriptor = decode_function_plot_descriptor(payload)?;
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
    _groups: &[ObjectGroup],
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
