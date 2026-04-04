use super::{
    CoordinatePoint, GspFile, ObjectGroup, ParameterControlledPoint, PointRecord,
    RawPointConstraint, TransformBindingKind, decode_coordinate_point,
    decode_parameter_controlled_point, decode_parameter_rotation_binding, decode_point_constraint,
    decode_reflection_anchor_raw, decode_transform_binding, decode_translated_point_constraint,
    reflection_line_group_indices, translation_point_pair_group_indices,
};
use crate::runtime::extract::find_indexed_path;
use crate::runtime::geometry::GraphTransform;
use crate::runtime::scene::{LineLikeKind, ScenePoint, ScenePointBinding, ScenePointConstraint};

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
                point_map
                    .get(index)
                    .cloned()
                    .flatten()
                    .map(|position| ScenePoint {
                        position,
                        visible,
                        constraint: ScenePointConstraint::Free,
                        binding: None,
                    })
            }
            crate::format::GroupKind::LinearIntersectionPoint
            | crate::format::GroupKind::IntersectionPoint1
            | crate::format::GroupKind::IntersectionPoint2
            | crate::format::GroupKind::CircleCircleIntersectionPoint1
            | crate::format::GroupKind::CircleCircleIntersectionPoint2 => {
                scene_point_from_intersection(
                    index,
                    file,
                    groups,
                    anchors,
                    &group_to_point_index,
                    visible,
                )
            }
            crate::format::GroupKind::Midpoint => {
                scene_point_from_midpoint(
                    index,
                    file,
                    groups,
                    anchors,
                    &group_to_point_index,
                    visible,
                )
            }
            crate::format::GroupKind::CartesianOffsetPoint
            | crate::format::GroupKind::PolarOffsetPoint => {
                decode_translated_point_constraint(file, group).and_then(|constraint| {
                    let origin_index = group_to_point_index
                        .get(constraint.origin_group_index)
                        .and_then(|point_index| *point_index)?;
                    let position = anchors.get(index).cloned().flatten()?;
                    Some(ScenePoint {
                        position,
                        visible,
                        constraint: ScenePointConstraint::Offset {
                            origin_index,
                            dx: constraint.dx,
                            dy: constraint.dy,
                        },
                        binding: None,
                    })
                })
            }
            crate::format::GroupKind::PointConstraint => {
                decode_point_constraint(file, groups, group, graph).and_then(|constraint| {
                    scene_point_from_constraint(
                        index,
                        anchors,
                        &group_to_point_index,
                        constraint,
                        visible,
                    )
                })
            }
            crate::format::GroupKind::ParameterControlledPoint => {
                decode_parameter_controlled_point(file, groups, group, anchors).and_then(
                    |parameter_point| {
                        scene_point_from_parameter_controlled(
                            &group_to_point_index,
                            parameter_point,
                            visible,
                        )
                    },
                )
            }
            crate::format::GroupKind::CoordinatePoint => {
                decode_coordinate_point(file, groups, group, graph)
                    .map(|point| scene_point_from_coordinate(point, visible))
            }
            crate::format::GroupKind::Reflection => {
                decode_reflection_anchor_raw(file, groups, group, anchors).and_then(|position| {
                    let path = find_indexed_path(file, group)?;
                    let source_group_index = path.refs.first()?.checked_sub(1)?;
                    let (line_start_group_index, line_end_group_index) =
                        reflection_line_group_indices(file, groups, group)?;
                    let source_index = group_to_point_index
                        .get(source_group_index)
                        .and_then(|point_index| *point_index)?;
                    let line_start_index = group_to_point_index
                        .get(line_start_group_index)
                        .and_then(|point_index| *point_index)?;
                    let line_end_index = group_to_point_index
                        .get(line_end_group_index)
                        .and_then(|point_index| *point_index)?;
                    groups
                        .get(source_group_index)
                        .filter(|source_group| {
                            (source_group.header.kind()) == crate::format::GroupKind::Point
                        })
                        .map(|_| ScenePoint {
                            position,
                            visible,
                            constraint: ScenePointConstraint::Free,
                            binding: Some(ScenePointBinding::Reflect {
                                source_index,
                                line_start_index,
                                line_end_index,
                            }),
                        })
                })
            }
            crate::format::GroupKind::Translation => {
                anchors.get(index).cloned().flatten().and_then(|position| {
                    let path = find_indexed_path(file, group)?;
                    let source_group_index = path.refs.first()?.checked_sub(1)?;
                    let (vector_start_group_index, vector_end_group_index) =
                        translation_point_pair_group_indices(file, group)?;
                    let source_index = group_to_point_index
                        .get(source_group_index)
                        .and_then(|point_index| *point_index)?;
                    let vector_start_index = group_to_point_index
                        .get(vector_start_group_index)
                        .and_then(|point_index| *point_index)?;
                    let vector_end_index = group_to_point_index
                        .get(vector_end_group_index)
                        .and_then(|point_index| *point_index)?;
                    groups
                        .get(source_group_index)
                        .filter(|source_group| {
                            (source_group.header.kind()) == crate::format::GroupKind::Point
                        })
                        .map(|_| ScenePoint {
                            position,
                            visible,
                            constraint: ScenePointConstraint::Free,
                            binding: Some(ScenePointBinding::Translate {
                                source_index,
                                vector_start_index,
                                vector_end_index,
                            }),
                        })
                })
            }
            crate::format::GroupKind::Rotation
            | crate::format::GroupKind::ParameterRotation
            | crate::format::GroupKind::Scale => {
                let binding = if kind == crate::format::GroupKind::ParameterRotation {
                    decode_parameter_rotation_binding(file, groups, group)
                } else {
                    decode_transform_binding(file, group)
                };
                binding.and_then(|binding| {
                    let position = anchors.get(index).cloned().flatten()?;
                    let source_index = group_to_point_index
                        .get(binding.source_group_index)
                        .and_then(|point_index| *point_index)?;
                    let center_index = group_to_point_index
                        .get(binding.center_group_index)
                        .and_then(|point_index| *point_index)?;
                    Some(ScenePoint {
                        position,
                        visible,
                        constraint: ScenePointConstraint::Free,
                        binding: Some(match binding.kind {
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
                    })
                })
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

fn scene_point_from_constraint(
    index: usize,
    anchors: &[Option<PointRecord>],
    group_to_point_index: &[Option<usize>],
    constraint: RawPointConstraint,
    visible: bool,
) -> Option<ScenePoint> {
    let position = anchors.get(index).cloned().flatten()?;
    match constraint {
        RawPointConstraint::Segment(constraint) => {
            let start_index = group_to_point_index
                .get(constraint.start_group_index)
                .and_then(|point_index| *point_index)?;
            let end_index = group_to_point_index
                .get(constraint.end_group_index)
                .and_then(|point_index| *point_index)?;
            Some(ScenePoint {
                position,
                visible,
                constraint: ScenePointConstraint::OnSegment {
                    start_index,
                    end_index,
                    t: constraint.t,
                },
                binding: None,
            })
        }
        RawPointConstraint::Polyline {
            function_key,
            points,
            segment_index,
            t,
        } => Some(ScenePoint {
            position,
            visible,
            constraint: ScenePointConstraint::OnPolyline {
                function_key,
                points,
                segment_index,
                t,
            },
            binding: None,
        }),
        RawPointConstraint::PolygonBoundary {
            vertex_group_indices,
            edge_index,
            t,
        } => {
            let vertex_indices = vertex_group_indices
                .iter()
                .map(|group_index| {
                    group_to_point_index
                        .get(*group_index)
                        .and_then(|point_index| *point_index)
                })
                .collect::<Option<Vec<_>>>()?;
            Some(ScenePoint {
                position,
                visible,
                constraint: ScenePointConstraint::OnPolygonBoundary {
                    vertex_indices,
                    edge_index,
                    t,
                },
                binding: None,
            })
        }
        RawPointConstraint::Circle(constraint) => {
            let center_index = group_to_point_index
                .get(constraint.center_group_index)
                .and_then(|point_index| *point_index)?;
            let radius_index = group_to_point_index
                .get(constraint.radius_group_index)
                .and_then(|point_index| *point_index)?;
            Some(ScenePoint {
                position,
                visible,
                constraint: ScenePointConstraint::OnCircle {
                    center_index,
                    radius_index,
                    unit_x: constraint.unit_x,
                    unit_y: constraint.unit_y,
                },
                binding: None,
            })
        }
        RawPointConstraint::Arc(constraint) => {
            let start_index = group_to_point_index
                .get(constraint.start_group_index)
                .and_then(|point_index| *point_index)?;
            let mid_index = group_to_point_index
                .get(constraint.mid_group_index)
                .and_then(|point_index| *point_index)?;
            let end_index = group_to_point_index
                .get(constraint.end_group_index)
                .and_then(|point_index| *point_index)?;
            Some(ScenePoint {
                position,
                visible,
                constraint: ScenePointConstraint::OnArc {
                    start_index,
                    mid_index,
                    end_index,
                    t: constraint.t,
                },
                binding: None,
            })
        }
    }
}

fn scene_point_from_parameter_controlled(
    group_to_point_index: &[Option<usize>],
    parameter_point: ParameterControlledPoint,
    visible: bool,
) -> Option<ScenePoint> {
    let binding = parameter_point_binding(group_to_point_index, &parameter_point)?;
    match &parameter_point.constraint {
        RawPointConstraint::Segment(constraint) => {
            let start_index = group_to_point_index
                .get(constraint.start_group_index)
                .and_then(|point_index| *point_index)?;
            let end_index = group_to_point_index
                .get(constraint.end_group_index)
                .and_then(|point_index| *point_index)?;
            Some(ScenePoint {
                position: parameter_point.position.clone(),
                visible,
                constraint: ScenePointConstraint::OnSegment {
                    start_index,
                    end_index,
                    t: constraint.t,
                },
                binding,
            })
        }
        RawPointConstraint::PolygonBoundary {
            vertex_group_indices,
            edge_index,
            t,
        } => {
            let vertex_indices = vertex_group_indices
                .iter()
                .map(|group_index| {
                    group_to_point_index
                        .get(*group_index)
                        .and_then(|point_index| *point_index)
                })
                .collect::<Option<Vec<_>>>()?;
            Some(ScenePoint {
                position: parameter_point.position.clone(),
                visible,
                constraint: ScenePointConstraint::OnPolygonBoundary {
                    vertex_indices,
                    edge_index: *edge_index,
                    t: *t,
                },
                binding,
            })
        }
        RawPointConstraint::Circle(constraint) => {
            let center_index = group_to_point_index
                .get(constraint.center_group_index)
                .and_then(|point_index| *point_index)?;
            let radius_index = group_to_point_index
                .get(constraint.radius_group_index)
                .and_then(|point_index| *point_index)?;
            Some(ScenePoint {
                position: parameter_point.position,
                visible,
                constraint: ScenePointConstraint::OnCircle {
                    center_index,
                    radius_index,
                    unit_x: constraint.unit_x,
                    unit_y: constraint.unit_y,
                },
                binding,
            })
        }
        RawPointConstraint::Arc(constraint) => {
            let start_index = group_to_point_index
                .get(constraint.start_group_index)
                .and_then(|point_index| *point_index)?;
            let mid_index = group_to_point_index
                .get(constraint.mid_group_index)
                .and_then(|point_index| *point_index)?;
            let end_index = group_to_point_index
                .get(constraint.end_group_index)
                .and_then(|point_index| *point_index)?;
            Some(ScenePoint {
                position: parameter_point.position,
                visible,
                constraint: ScenePointConstraint::OnArc {
                    start_index,
                    mid_index,
                    end_index,
                    t: constraint.t,
                },
                binding,
            })
        }
        RawPointConstraint::Polyline { .. } => None,
    }
}

fn parameter_point_binding(
    group_to_point_index: &[Option<usize>],
    parameter_point: &ParameterControlledPoint,
) -> Option<Option<ScenePointBinding>> {
    if let Some(source_group_index) = parameter_point.source_point_group_index {
        let source_index = group_to_point_index
            .get(source_group_index)
            .and_then(|point_index| *point_index)?;
        Some(Some(ScenePointBinding::DerivedParameter { source_index }))
    } else {
        Some(
            (!parameter_point.parameter_name.is_empty()).then(|| ScenePointBinding::Parameter {
                name: parameter_point.parameter_name.clone(),
            }),
        )
    }
}

fn scene_point_from_coordinate(point: CoordinatePoint, visible: bool) -> ScenePoint {
    ScenePoint {
        position: point.position,
        visible,
        constraint: ScenePointConstraint::Free,
        binding: Some(ScenePointBinding::Coordinate {
            name: point.parameter_name,
            expr: point.expr,
        }),
    }
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
    Some(ScenePoint {
        position,
        visible,
        constraint: ScenePointConstraint::OnSegment {
            start_index,
            end_index,
            t: 0.5,
        },
        binding: Some(ScenePointBinding::Midpoint {
            start_index,
            end_index,
        }),
    })
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

    if let (
        Some((left_start_index, left_end_index, left_kind)),
        Some((right_start_index, right_end_index, right_kind)),
    ) = (
        resolve_line_like_point_indices(file, groups, left_group, group_to_point_index),
        resolve_line_like_point_indices(file, groups, right_group, group_to_point_index),
    ) {
        return Some(ScenePoint {
            position,
            visible,
            constraint: ScenePointConstraint::LineIntersection {
                left_kind,
                left_start_index,
                left_end_index,
                right_kind,
                right_start_index,
                right_end_index,
            },
            binding: None,
        });
    }

    let variant = intersection_variant(group.header.kind());
    if let (Some((line_start_index, line_end_index, line_kind)), Some((center_index, radius_index))) = (
        resolve_line_like_point_indices(file, groups, left_group, group_to_point_index),
        resolve_circle_point_indices(file, groups, right_group, group_to_point_index),
    ) {
        return Some(ScenePoint {
            position,
            visible,
            constraint: ScenePointConstraint::LineCircleIntersection {
                line_kind,
                line_start_index,
                line_end_index,
                center_index,
                radius_index,
                variant,
            },
            binding: None,
        });
    }

    if let (Some((center_index, radius_index)), Some((line_start_index, line_end_index, line_kind))) = (
        resolve_circle_point_indices(file, groups, left_group, group_to_point_index),
        resolve_line_like_point_indices(file, groups, right_group, group_to_point_index),
    ) {
        return Some(ScenePoint {
            position,
            visible,
            constraint: ScenePointConstraint::LineCircleIntersection {
                line_kind,
                line_start_index,
                line_end_index,
                center_index,
                radius_index,
                variant,
            },
            binding: None,
        });
    }

    if let (Some((left_center_index, left_radius_index)), Some((right_center_index, right_radius_index))) = (
        resolve_circle_point_indices(file, groups, left_group, group_to_point_index),
        resolve_circle_point_indices(file, groups, right_group, group_to_point_index),
    ) {
        return Some(ScenePoint {
            position,
            visible,
            constraint: ScenePointConstraint::CircleCircleIntersection {
                left_center_index,
                left_radius_index,
                right_center_index,
                right_radius_index,
                variant,
            },
            binding: None,
        });
    }

    Some(ScenePoint {
        position,
        visible,
        constraint: ScenePointConstraint::Free,
        binding: None,
    })
}

fn resolve_line_like_point_indices(
    file: &GspFile,
    _groups: &[ObjectGroup],
    group: &ObjectGroup,
    group_to_point_index: &[Option<usize>],
) -> Option<(usize, usize, LineLikeKind)> {
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
            Some((
                start_index,
                end_index,
                match group.header.kind() {
                    crate::format::GroupKind::Segment => LineLikeKind::Segment,
                    crate::format::GroupKind::Ray => LineLikeKind::Ray,
                    _ => LineLikeKind::Line,
                },
            ))
        }
        _ => None,
    }
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

fn intersection_variant(kind: crate::format::GroupKind) -> usize {
    match kind {
        crate::format::GroupKind::IntersectionPoint1
        | crate::format::GroupKind::CircleCircleIntersectionPoint1 => 1,
        _ => 0,
    }
}
