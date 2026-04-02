use super::super::decode::decode_label_name;
use super::anchors::{
    decode_reflection_anchor_raw, reflection_line_group_indices,
    translation_point_pair_group_indices,
};
use super::constraints::{
    CoordinatePoint, ParameterControlledPoint, RawPointConstraint, decode_coordinate_point,
    decode_parameter_controlled_point, decode_point_constraint, decode_translated_point_constraint,
    regular_polygon_iteration_step,
};
use super::*;
use crate::runtime::functions::FunctionExpr;
use crate::runtime::scene::{LineBinding, ShapeBinding};

pub(crate) struct TransformBinding {
    pub(crate) source_group_index: usize,
    pub(crate) center_group_index: usize,
    pub(crate) kind: TransformBindingKind,
}

pub(crate) enum TransformBindingKind {
    Rotate { angle_degrees: f64 },
    Scale { factor: f64 },
}

fn iteration_depth(file: &GspFile, group: &ObjectGroup, default_depth: usize) -> usize {
    group
        .records
        .iter()
        .find(|record| record.record_type == 0x090a)
        .map(|record| record.payload(&file.data))
        .filter(|payload| payload.len() >= 20)
        .map(|payload| read_u32(payload, 16) as usize)
        .unwrap_or(default_depth)
}

pub(crate) enum RawPointIterationFamily {
    Offset {
        seed_index: usize,
        dx: f64,
        dy: f64,
        depth: usize,
        parameter_name: Option<String>,
    },
    Rotate {
        source_index: usize,
        center_index: usize,
        angle_expr: FunctionExpr,
        depth: usize,
        parameter_name: Option<String>,
    },
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

pub(crate) fn remap_label_bindings(
    labels: &mut [TextLabel],
    group_to_point_index: &[Option<usize>],
) {
    for label in labels {
        let Some(binding) = label.binding.as_mut() else {
            continue;
        };
        let point_index = match binding {
            TextLabelBinding::ParameterValue { .. }
            | TextLabelBinding::ExpressionValue { .. }
            | TextLabelBinding::PointExpressionValue { .. } => continue,
            TextLabelBinding::PolygonBoundaryParameter { point_index, .. } => point_index,
            TextLabelBinding::SegmentParameter { point_index, .. } => point_index,
            TextLabelBinding::CircleParameter { point_index, .. } => point_index,
        };
        let Some(mapped_index) = group_to_point_index
            .get(*point_index)
            .and_then(|mapped_index| *mapped_index)
        else {
            label.binding = None;
            continue;
        };
        *point_index = mapped_index;
    }
}

pub(crate) fn remap_circle_bindings(
    circles: &mut [CircleShape],
    group_to_point_index: &[Option<usize>],
    group_to_circle_index: &[Option<usize>],
) {
    for circle in circles {
        let Some(binding) = circle.binding.as_mut() else {
            continue;
        };
        let (source_index, center_index) = match binding {
            ShapeBinding::TranslateCircle { source_index, .. } => {
                let Some(mapped_source_index) = group_to_circle_index
                    .get(*source_index)
                    .and_then(|mapped_index| *mapped_index)
                else {
                    circle.binding = None;
                    continue;
                };
                *source_index = mapped_source_index;
                continue;
            }
            ShapeBinding::RotateCircle {
                source_index,
                center_index,
                ..
            } => (source_index, center_index),
            ShapeBinding::ScaleCircle {
                source_index,
                center_index,
                ..
            } => (source_index, center_index),
            ShapeBinding::ReflectCircle {
                source_index,
                line_start_index,
                line_end_index,
            } => {
                let Some(mapped_source_index) = group_to_circle_index
                    .get(*source_index)
                    .and_then(|mapped_index| *mapped_index)
                else {
                    circle.binding = None;
                    continue;
                };
                let Some(mapped_line_start_index) = group_to_point_index
                    .get(*line_start_index)
                    .and_then(|mapped_index| *mapped_index)
                else {
                    circle.binding = None;
                    continue;
                };
                let Some(mapped_line_end_index) = group_to_point_index
                    .get(*line_end_index)
                    .and_then(|mapped_index| *mapped_index)
                else {
                    circle.binding = None;
                    continue;
                };
                *source_index = mapped_source_index;
                *line_start_index = mapped_line_start_index;
                *line_end_index = mapped_line_end_index;
                continue;
            }
            _ => continue,
        };
        let Some(mapped_source_index) = group_to_circle_index
            .get(*source_index)
            .and_then(|mapped_index| *mapped_index)
        else {
            circle.binding = None;
            continue;
        };
        let Some(mapped_center_index) = group_to_point_index
            .get(*center_index)
            .and_then(|mapped_index| *mapped_index)
        else {
            circle.binding = None;
            continue;
        };
        *source_index = mapped_source_index;
        *center_index = mapped_center_index;
    }
}

pub(crate) fn remap_polygon_bindings(
    polygons: &mut [PolygonShape],
    group_to_point_index: &[Option<usize>],
    group_to_polygon_index: &[Option<usize>],
) {
    for polygon in polygons {
        let Some(binding) = polygon.binding.as_mut() else {
            continue;
        };
        let (source_index, center_index) = match binding {
            ShapeBinding::TranslatePolygon {
                source_index,
                vector_start_index,
                vector_end_index,
            } => {
                let Some(mapped_source_index) = group_to_polygon_index
                    .get(*source_index)
                    .and_then(|mapped_index| *mapped_index)
                else {
                    polygon.binding = None;
                    continue;
                };
                let Some(mapped_vector_start_index) = group_to_point_index
                    .get(*vector_start_index)
                    .and_then(|mapped_index| *mapped_index)
                else {
                    polygon.binding = None;
                    continue;
                };
                let Some(mapped_vector_end_index) = group_to_point_index
                    .get(*vector_end_index)
                    .and_then(|mapped_index| *mapped_index)
                else {
                    polygon.binding = None;
                    continue;
                };
                *source_index = mapped_source_index;
                *vector_start_index = mapped_vector_start_index;
                *vector_end_index = mapped_vector_end_index;
                continue;
            }
            ShapeBinding::RotatePolygon {
                source_index,
                center_index,
                ..
            } => (source_index, center_index),
            ShapeBinding::ScalePolygon {
                source_index,
                center_index,
                ..
            } => (source_index, center_index),
            ShapeBinding::ReflectPolygon {
                source_index,
                line_start_index,
                line_end_index,
            } => {
                let Some(mapped_source_index) = group_to_polygon_index
                    .get(*source_index)
                    .and_then(|mapped_index| *mapped_index)
                else {
                    polygon.binding = None;
                    continue;
                };
                let Some(mapped_line_start_index) = group_to_point_index
                    .get(*line_start_index)
                    .and_then(|mapped_index| *mapped_index)
                else {
                    polygon.binding = None;
                    continue;
                };
                let Some(mapped_line_end_index) = group_to_point_index
                    .get(*line_end_index)
                    .and_then(|mapped_index| *mapped_index)
                else {
                    polygon.binding = None;
                    continue;
                };
                *source_index = mapped_source_index;
                *line_start_index = mapped_line_start_index;
                *line_end_index = mapped_line_end_index;
                continue;
            }
            _ => continue,
        };
        let Some(mapped_source_index) = group_to_polygon_index
            .get(*source_index)
            .and_then(|mapped_index| *mapped_index)
        else {
            polygon.binding = None;
            continue;
        };
        let Some(mapped_center_index) = group_to_point_index
            .get(*center_index)
            .and_then(|mapped_index| *mapped_index)
        else {
            polygon.binding = None;
            continue;
        };
        *source_index = mapped_source_index;
        *center_index = mapped_center_index;
    }
}

pub(crate) fn remap_line_bindings(
    lines: &mut [LineShape],
    group_to_point_index: &[Option<usize>],
    group_to_line_index: &[Option<usize>],
) {
    for line in lines {
        let Some(binding) = line.binding.as_mut() else {
            continue;
        };
        match binding {
            LineBinding::TranslateLine {
                source_index,
                vector_start_index,
                vector_end_index,
            } => {
                let Some(mapped_source_index) = group_to_line_index
                    .get(*source_index)
                    .and_then(|mapped_index| *mapped_index)
                else {
                    line.binding = None;
                    continue;
                };
                let Some(mapped_vector_start_index) = group_to_point_index
                    .get(*vector_start_index)
                    .and_then(|mapped_index| *mapped_index)
                else {
                    line.binding = None;
                    continue;
                };
                let Some(mapped_vector_end_index) = group_to_point_index
                    .get(*vector_end_index)
                    .and_then(|mapped_index| *mapped_index)
                else {
                    line.binding = None;
                    continue;
                };
                *source_index = mapped_source_index;
                *vector_start_index = mapped_vector_start_index;
                *vector_end_index = mapped_vector_end_index;
            }
            LineBinding::Line {
                start_index,
                end_index,
            }
            | LineBinding::Ray {
                start_index,
                end_index,
            } => {
                let Some(mapped_start_index) = group_to_point_index
                    .get(*start_index)
                    .and_then(|mapped_index| *mapped_index)
                else {
                    line.binding = None;
                    continue;
                };
                let Some(mapped_end_index) = group_to_point_index
                    .get(*end_index)
                    .and_then(|mapped_index| *mapped_index)
                else {
                    line.binding = None;
                    continue;
                };
                *start_index = mapped_start_index;
                *end_index = mapped_end_index;
            }
            LineBinding::RotateLine {
                source_index,
                center_index,
                ..
            }
            | LineBinding::ScaleLine {
                source_index,
                center_index,
                ..
            } => {
                let Some(mapped_source_index) = group_to_line_index
                    .get(*source_index)
                    .and_then(|mapped_index| *mapped_index)
                else {
                    line.binding = None;
                    continue;
                };
                let Some(mapped_center_index) = group_to_point_index
                    .get(*center_index)
                    .and_then(|mapped_index| *mapped_index)
                else {
                    line.binding = None;
                    continue;
                };
                *source_index = mapped_source_index;
                *center_index = mapped_center_index;
            }
            LineBinding::ReflectLine {
                source_index,
                line_start_index,
                line_end_index,
            } => {
                let Some(mapped_source_index) = group_to_line_index
                    .get(*source_index)
                    .and_then(|mapped_index| *mapped_index)
                else {
                    line.binding = None;
                    continue;
                };
                let Some(mapped_line_start_index) = group_to_point_index
                    .get(*line_start_index)
                    .and_then(|mapped_index| *mapped_index)
                else {
                    line.binding = None;
                    continue;
                };
                let Some(mapped_line_end_index) = group_to_point_index
                    .get(*line_end_index)
                    .and_then(|mapped_index| *mapped_index)
                else {
                    line.binding = None;
                    continue;
                };
                *source_index = mapped_source_index;
                *line_start_index = mapped_line_start_index;
                *line_end_index = mapped_line_end_index;
            }
            LineBinding::RotateEdge {
                center_index,
                vertex_index,
                ..
            } => {
                let Some(mapped_center_index) = group_to_point_index
                    .get(*center_index)
                    .and_then(|mapped_index| *mapped_index)
                else {
                    line.binding = None;
                    continue;
                };
                let Some(mapped_vertex_index) = group_to_point_index
                    .get(*vertex_index)
                    .and_then(|mapped_index| *mapped_index)
                else {
                    line.binding = None;
                    continue;
                };
                *center_index = mapped_center_index;
                *vertex_index = mapped_vertex_index;
            }
        }
    }
}

pub(crate) fn collect_point_iteration_points(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
    group_to_point_index: &[Option<usize>],
) -> (Vec<ScenePoint>, Vec<RawPointIterationFamily>) {
    let mut derived_points = Vec::new();
    let mut families = Vec::new();

    for group in groups
        .iter()
        .filter(|group| (group.header.class_id & 0xffff) == 77)
    {
        let Some(path) = find_indexed_path(file, group) else {
            continue;
        };
        if path.refs.len() < 2 {
            continue;
        }
        let Some(seed_group_index) = path.refs[0].checked_sub(1) else {
            continue;
        };
        let Some(iter_group_index) = path.refs[1].checked_sub(1) else {
            continue;
        };
        let Some(seed_index) = group_to_point_index
            .get(seed_group_index)
            .and_then(|mapped_index| *mapped_index)
        else {
            continue;
        };
        let Some(iter_group) = groups.get(iter_group_index) else {
            continue;
        };
        match iter_group.header.class_id & 0xffff {
            76 => {
                let Some(iter_path) = find_indexed_path(file, iter_group) else {
                    continue;
                };
                if iter_path.refs.len() < 2 {
                    continue;
                }
                let Some(base_start) = anchors
                    .get(iter_path.refs[0].saturating_sub(1))
                    .cloned()
                    .flatten()
                else {
                    continue;
                };
                let Some(base_end) = anchors
                    .get(iter_path.refs[1].saturating_sub(1))
                    .cloned()
                    .flatten()
                else {
                    continue;
                };
                let dx = base_end.x - base_start.x;
                let dy = base_end.y - base_start.y;
                let depth = iteration_depth(file, iter_group, 3);
                if depth == 0 {
                    continue;
                }
                let Some(seed_position) = anchors.get(seed_group_index).cloned().flatten() else {
                    continue;
                };

                let mut previous_index = seed_index + derived_points.len();
                let mut current_position = seed_position;
                for _ in 0..depth {
                    current_position += PointRecord { x: dx, y: dy };
                    derived_points.push(ScenePoint {
                        position: current_position.clone(),
                        constraint: ScenePointConstraint::Offset {
                            origin_index: previous_index,
                            dx,
                            dy,
                        },
                        binding: None,
                    });
                    previous_index = seed_index + derived_points.len();
                }
                families.push(RawPointIterationFamily::Offset {
                    seed_index,
                    dx,
                    dy,
                    depth,
                    parameter_name: None,
                });
            }
            89 => {
                let Some(iter_path) = find_indexed_path(file, iter_group) else {
                    continue;
                };
                let depth = iteration_depth(file, iter_group, 3);
                if depth == 0 {
                    continue;
                }
                if let Some((parameter_name, dx, dy)) =
                    parameter_iteration_step(groups, iter_group, anchors, file)
                {
                    let Some(seed_position) = anchors.get(seed_group_index).cloned().flatten()
                    else {
                        continue;
                    };
                    let mut previous_index = seed_index + derived_points.len();
                    let mut current_position = seed_position;
                    for _ in 0..depth {
                        current_position += PointRecord { x: dx, y: dy };
                        derived_points.push(ScenePoint {
                            position: current_position.clone(),
                            constraint: ScenePointConstraint::Offset {
                                origin_index: previous_index,
                                dx,
                                dy,
                            },
                            binding: None,
                        });
                        previous_index = seed_index + derived_points.len();
                    }
                    families.push(RawPointIterationFamily::Offset {
                        seed_index,
                        dx,
                        dy,
                        depth,
                        parameter_name: is_editable_non_graph_parameter_name(&parameter_name)
                            .then_some(parameter_name),
                    });
                } else if let Some((center_group_index, _angle_expr, parameter_name, n)) =
                    regular_polygon_iteration_step(file, groups, iter_group)
                {
                    let Some(center_index) = group_to_point_index
                        .get(center_group_index)
                        .and_then(|mapped_index| *mapped_index)
                    else {
                        continue;
                    };
                    let Some(seed_position) = anchors.get(seed_group_index).cloned().flatten()
                    else {
                        continue;
                    };
                    let Some(center_position) = anchors.get(center_group_index).cloned().flatten()
                    else {
                        continue;
                    };
                    let angle_degrees = -360.0 / n;
                    for step in 1..=depth {
                        let radians = (angle_degrees * step as f64).to_radians();
                        let cos = radians.cos();
                        let sin = radians.sin();
                        let dx = seed_position.x - center_position.x;
                        let dy = seed_position.y - center_position.y;
                        let position = PointRecord {
                            x: center_position.x + dx * cos + dy * sin,
                            y: center_position.y - dx * sin + dy * cos,
                        };
                        derived_points.push(ScenePoint {
                            position,
                            constraint: ScenePointConstraint::Free,
                            binding: Some(ScenePointBinding::Rotate {
                                source_index: seed_index,
                                center_index,
                                angle_degrees: angle_degrees * step as f64,
                            }),
                        });
                    }
                    let angle_expr = regular_polygon_angle_expr(&parameter_name, n);
                    families.push(RawPointIterationFamily::Rotate {
                        source_index: seed_index,
                        center_index,
                        angle_expr,
                        depth,
                        parameter_name: is_editable_non_graph_parameter_name(&parameter_name)
                            .then_some(parameter_name),
                    });
                } else if iter_path.refs.len() >= 2 {
                    let _ = iter_path;
                }
            }
            _ => {}
        }
    }

    (derived_points, families)
}

fn parameter_iteration_step(
    groups: &[ObjectGroup],
    iter_group: &ObjectGroup,
    anchors: &[Option<PointRecord>],
    file: &GspFile,
) -> Option<(String, f64, f64)> {
    let path = find_indexed_path(file, iter_group)?;
    if path.refs.len() < 3 {
        return None;
    }
    let parameter_group = groups.get(path.refs[0].checked_sub(1)?)?;
    if (parameter_group.header.class_id & 0xffff) != 0 {
        return None;
    }
    let parameter_name = decode_label_name(file, parameter_group)?;
    let base_start = anchors.get(path.refs[1].checked_sub(1)?)?.clone()?;
    let base_end = anchors.get(path.refs[2].checked_sub(1)?)?.clone()?;
    Some((
        parameter_name,
        base_end.x - base_start.x,
        base_end.y - base_start.y,
    ))
}

pub(crate) fn decode_transform_binding(
    file: &GspFile,
    group: &ObjectGroup,
) -> Option<TransformBinding> {
    let kind = group.header.class_id & 0xffff;
    let path = find_indexed_path(file, group)?;
    let source_group_index = path.refs.first()?.checked_sub(1)?;
    let center_group_index = path.refs.get(1)?.checked_sub(1)?;
    let payload = group
        .records
        .iter()
        .find(|record| record.record_type == 0x07d3)
        .map(|record| record.payload(&file.data))?;

    let kind = match kind {
        27 => {
            let angle_degrees = if payload.len() >= 28 {
                let angle = read_f64(payload, 20);
                if angle.is_finite() {
                    angle
                } else {
                    return None;
                }
            } else {
                let cos = read_f64(payload, 4);
                let sin = read_f64(payload, 12);
                sin.atan2(cos).to_degrees()
            };
            TransformBindingKind::Rotate { angle_degrees }
        }
        30 => {
            if payload.len() < 12 {
                return None;
            }
            let factor = read_f64(payload, 4);
            if !factor.is_finite() {
                return None;
            }
            TransformBindingKind::Scale { factor }
        }
        _ => return None,
    };

    Some(TransformBinding {
        source_group_index,
        center_group_index,
        kind,
    })
}

pub(crate) fn decode_parameter_rotation_binding(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
) -> Option<TransformBinding> {
    if (group.header.class_id & 0xffff) != 29 {
        return None;
    }
    let path = find_indexed_path(file, group)?;
    let source_group_index = path.refs.first()?.checked_sub(1)?;
    let center_group_index = path.refs.get(1)?.checked_sub(1)?;
    let angle_group = groups.get(path.refs.get(2)?.checked_sub(1)?)?;
    if (angle_group.header.class_id & 0xffff) != 0 {
        return None;
    }
    let angle_radians = decode_angle_parameter_value_for_group(file, angle_group)?;
    if !angle_radians.is_finite() {
        return None;
    }

    Some(TransformBinding {
        source_group_index,
        center_group_index,
        kind: TransformBindingKind::Rotate {
            angle_degrees: angle_radians.to_degrees(),
        },
    })
}
