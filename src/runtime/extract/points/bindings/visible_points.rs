use super::{
    CoordinatePoint, GraphTransform, GspFile, ObjectGroup, ParameterControlledPoint, PointRecord,
    RawPointConstraint, ScenePoint, ScenePointBinding, ScenePointConstraint, TransformBindingKind,
    decode_coordinate_point, decode_parameter_controlled_point, decode_parameter_rotation_binding,
    decode_point_constraint, decode_reflection_anchor_raw, decode_transform_binding,
    decode_translated_point_constraint, find_indexed_path, reflection_line_group_indices,
    translation_point_pair_group_indices,
};

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
        let kind = group.header.class_id & 0xffff;
        let scene_point = match kind {
            0 => point_map
                .get(index)
                .cloned()
                .flatten()
                .map(|position| ScenePoint {
                    position,
                    constraint: ScenePointConstraint::Free,
                    binding: None,
                }),
            17 | 21 => decode_translated_point_constraint(file, group).and_then(|constraint| {
                let origin_index = group_to_point_index
                    .get(constraint.origin_group_index)
                    .and_then(|point_index| *point_index)?;
                let position = anchors.get(index).cloned().flatten()?;
                Some(ScenePoint {
                    position,
                    constraint: ScenePointConstraint::Offset {
                        origin_index,
                        dx: constraint.dx,
                        dy: constraint.dy,
                    },
                    binding: None,
                })
            }),
            15 => decode_point_constraint(file, groups, group, graph).and_then(|constraint| {
                scene_point_from_constraint(index, anchors, &group_to_point_index, constraint)
            }),
            95 => decode_parameter_controlled_point(file, groups, group, anchors).and_then(
                |parameter_point| {
                    scene_point_from_parameter_controlled(&group_to_point_index, parameter_point)
                },
            ),
            69 => {
                decode_coordinate_point(file, groups, group, graph).map(scene_point_from_coordinate)
            }
            34 => decode_reflection_anchor_raw(file, groups, group, anchors).and_then(|position| {
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
                    .filter(|source_group| (source_group.header.class_id & 0xffff) == 0)
                    .map(|_| ScenePoint {
                        position,
                        constraint: ScenePointConstraint::Free,
                        binding: Some(ScenePointBinding::Reflect {
                            source_index,
                            line_start_index,
                            line_end_index,
                        }),
                    })
            }),
            16 => anchors.get(index).cloned().flatten().and_then(|position| {
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
                    .filter(|source_group| (source_group.header.class_id & 0xffff) == 0)
                    .map(|_| ScenePoint {
                        position,
                        constraint: ScenePointConstraint::Free,
                        binding: Some(ScenePointBinding::Translate {
                            source_index,
                            vector_start_index,
                            vector_end_index,
                        }),
                    })
            }),
            27 | 29 | 30 => {
                let binding = if kind == 29 {
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
                        constraint: ScenePointConstraint::Free,
                        binding: Some(match binding.kind {
                            TransformBindingKind::Rotate { angle_degrees } => {
                                ScenePointBinding::Rotate {
                                    source_index,
                                    center_index,
                                    angle_degrees,
                                }
                            }
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
                constraint: ScenePointConstraint::OnCircle {
                    center_index,
                    radius_index,
                    unit_x: constraint.unit_x,
                    unit_y: constraint.unit_y,
                },
                binding: None,
            })
        }
    }
}

fn scene_point_from_parameter_controlled(
    group_to_point_index: &[Option<usize>],
    parameter_point: ParameterControlledPoint,
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
                constraint: ScenePointConstraint::OnCircle {
                    center_index,
                    radius_index,
                    unit_x: constraint.unit_x,
                    unit_y: constraint.unit_y,
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

fn scene_point_from_coordinate(point: CoordinatePoint) -> ScenePoint {
    ScenePoint {
        position: point.position,
        constraint: ScenePointConstraint::Free,
        binding: Some(ScenePointBinding::Coordinate {
            name: point.parameter_name,
            expr: point.expr,
        }),
    }
}
