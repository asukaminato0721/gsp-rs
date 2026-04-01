use super::*;
use crate::runtime::scene::{LineBinding, ShapeBinding};

pub(super) fn collect_point_objects(
    file: &GspFile,
    groups: &[ObjectGroup],
) -> Vec<Option<PointRecord>> {
    groups
        .iter()
        .map(|group| {
            if (group.header.class_id & 0xffff) != 0 {
                return None;
            }
            group.records.iter().find_map(|record| {
                (record.record_type == 0x0899)
                    .then(|| decode_point_record(record.payload(&file.data)))
                    .flatten()
            })
        })
        .collect()
}

pub(super) fn collect_non_graph_parameters(
    file: &GspFile,
    groups: &[ObjectGroup],
    labels: &mut [TextLabel],
) -> Vec<SceneParameter> {
    groups
        .iter()
        .filter_map(|group| decode_non_graph_parameter(file, group, labels))
        .collect()
}

fn decode_non_graph_parameter(
    file: &GspFile,
    group: &ObjectGroup,
    labels: &mut [TextLabel],
) -> Option<SceneParameter> {
    if (group.header.class_id & 0xffff) != 0 {
        return None;
    }
    if group
        .records
        .iter()
        .any(|record| record.record_type == 0x0899)
    {
        return None;
    }
    let _payload = group
        .records
        .iter()
        .find(|record| record.record_type == 0x0907)
        .map(|record| record.payload(&file.data))?;
    let name = decode_label_name(file, group)?;
    if !is_editable_non_graph_parameter_name(&name) {
        return None;
    }
    let value = decode_non_graph_parameter_value_for_group(file, group)?;
    let label_index = labels.iter().position(|label| label.text == name);
    if let Some(index) = label_index {
        labels[index].text = format!("{name} = {:.2}", value);
    }
    Some(SceneParameter {
        name,
        value,
        label_index,
    })
}

fn is_slider_parameter_name(name: &str) -> bool {
    name.contains('₁') || name.contains('₂') || name.contains('₃') || name.contains('₄')
}

pub(super) fn is_editable_non_graph_parameter_name(name: &str) -> bool {
    is_slider_parameter_name(name) || name == "n"
}

fn decode_non_graph_parameter_value(payload: &[u8]) -> Option<f64> {
    (payload.len() >= 60)
        .then(|| read_f64(payload, 52))
        .filter(|value| value.is_finite())
}

pub(super) fn decode_non_graph_parameter_value_for_group(
    file: &GspFile,
    group: &ObjectGroup,
) -> Option<f64> {
    let name = decode_label_name(file, group)?;
    let payload = group
        .records
        .iter()
        .find(|record| record.record_type == 0x0907)
        .map(|record| record.payload(&file.data))?;
    if is_slider_parameter_name(&name) {
        decode_non_graph_parameter_value(payload)
    } else {
        let value_code = read_u16(payload, payload.len().checked_sub(2)?);
        Some(f64::from(value_code))
    }
}

pub(super) fn collect_visible_points(
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
            21 => decode_translated_point_constraint(file, group).and_then(|constraint| {
                let origin_index = group_to_point_index
                    .get(constraint.origin_group_index)
                    .and_then(|index| *index)?;
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
                let position = anchors.get(index).cloned().flatten()?;
                match constraint {
                    RawPointConstraint::Segment(constraint) => {
                        let start_index = group_to_point_index
                            .get(constraint.start_group_index)
                            .and_then(|index| *index)?;
                        let end_index = group_to_point_index
                            .get(constraint.end_group_index)
                            .and_then(|index| *index)?;
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
                                    .and_then(|index| *index)
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
                            .and_then(|index| *index)?;
                        let radius_index = group_to_point_index
                            .get(constraint.radius_group_index)
                            .and_then(|index| *index)?;
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
            }),
            95 => decode_parameter_controlled_point(file, groups, group, anchors).and_then(
                |parameter_point| match parameter_point.constraint {
                    RawPointConstraint::Segment(constraint) => {
                        let start_index = group_to_point_index
                            .get(constraint.start_group_index)
                            .and_then(|index| *index)?;
                        let end_index = group_to_point_index
                            .get(constraint.end_group_index)
                            .and_then(|index| *index)?;
                        let binding = if let Some(source_group_index) =
                            parameter_point.source_point_group_index
                        {
                            group_to_point_index
                                .get(source_group_index)
                                .and_then(|index| *index)
                                .map(|source_index| ScenePointBinding::DerivedParameter {
                                    source_index,
                                })
                        } else {
                            (!parameter_point.parameter_name.is_empty()).then(|| {
                                ScenePointBinding::Parameter {
                                    name: parameter_point.parameter_name,
                                }
                            })
                        };
                        Some(ScenePoint {
                            position: parameter_point.position,
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
                                    .and_then(|index| *index)
                            })
                            .collect::<Option<Vec<_>>>()?;
                        let binding = if let Some(source_group_index) =
                            parameter_point.source_point_group_index
                        {
                            group_to_point_index
                                .get(source_group_index)
                                .and_then(|index| *index)
                                .map(|source_index| ScenePointBinding::DerivedParameter {
                                    source_index,
                                })
                        } else {
                            (!parameter_point.parameter_name.is_empty()).then(|| {
                                ScenePointBinding::Parameter {
                                    name: parameter_point.parameter_name,
                                }
                            })
                        };
                        Some(ScenePoint {
                            position: parameter_point.position,
                            constraint: ScenePointConstraint::OnPolygonBoundary {
                                vertex_indices,
                                edge_index,
                                t,
                            },
                            binding,
                        })
                    }
                    RawPointConstraint::Circle(constraint) => {
                        let center_index = group_to_point_index
                            .get(constraint.center_group_index)
                            .and_then(|index| *index)?;
                        let radius_index = group_to_point_index
                            .get(constraint.radius_group_index)
                            .and_then(|index| *index)?;
                        let binding = if let Some(source_group_index) =
                            parameter_point.source_point_group_index
                        {
                            group_to_point_index
                                .get(source_group_index)
                                .and_then(|index| *index)
                                .map(|source_index| ScenePointBinding::DerivedParameter {
                                    source_index,
                                })
                        } else {
                            (!parameter_point.parameter_name.is_empty()).then(|| {
                                ScenePointBinding::Parameter {
                                    name: parameter_point.parameter_name,
                                }
                            })
                        };
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
                },
            ),
            69 => decode_coordinate_point(file, groups, group, graph).map(|point| ScenePoint {
                position: point.position,
                constraint: ScenePointConstraint::Free,
                binding: Some(ScenePointBinding::Coordinate {
                    name: point.parameter_name,
                    expr: point.expr,
                }),
            }),
            34 => decode_reflection_anchor_raw(file, groups, group, anchors).and_then(|position| {
                let path = find_indexed_path(file, group)?;
                let source_group_index = path.refs.first()?.checked_sub(1)?;
                let (line_start_group_index, line_end_group_index) =
                    reflection_line_group_indices(file, groups, group)?;
                let source_index = group_to_point_index
                    .get(source_group_index)
                    .and_then(|index| *index)?;
                let line_start_index = group_to_point_index
                    .get(line_start_group_index)
                    .and_then(|index| *index)?;
                let line_end_index = group_to_point_index
                    .get(line_end_group_index)
                    .and_then(|index| *index)?;
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
            27 | 30 => decode_transform_binding(file, group).and_then(|binding| {
                let position = anchors.get(index).cloned().flatten()?;
                let source_index = group_to_point_index
                    .get(binding.source_group_index)
                    .and_then(|index| *index)?;
                let center_index = group_to_point_index
                    .get(binding.center_group_index)
                    .and_then(|index| *index)?;
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
            }),
            _ => None,
        };

        if let Some(scene_point) = scene_point {
            group_to_point_index[index] = Some(points.len());
            points.push(scene_point);
        }
    }

    (points, group_to_point_index)
}

pub(super) fn remap_label_bindings(
    labels: &mut [TextLabel],
    group_to_point_index: &[Option<usize>],
) {
    for label in labels {
        let Some(binding) = label.binding.as_mut() else {
            continue;
        };
        let point_index = match binding {
            TextLabelBinding::ParameterValue { .. } | TextLabelBinding::ExpressionValue { .. } => {
                continue;
            }
            TextLabelBinding::PolygonBoundaryParameter { point_index, .. } => point_index,
            TextLabelBinding::SegmentParameter { point_index, .. } => point_index,
            TextLabelBinding::CircleParameter { point_index, .. } => point_index,
        };
        let Some(mapped_index) = group_to_point_index
            .get(*point_index)
            .and_then(|index| *index)
        else {
            label.binding = None;
            continue;
        };
        *point_index = mapped_index;
    }
}

pub(super) fn remap_circle_bindings(
    circles: &mut [CircleShape],
    group_to_point_index: &[Option<usize>],
    group_to_circle_index: &[Option<usize>],
) {
    for circle in circles {
        let Some(binding) = circle.binding.as_mut() else {
            continue;
        };
        let (source_index, center_index) = match binding {
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
                    .and_then(|index| *index)
                else {
                    circle.binding = None;
                    continue;
                };
                let Some(mapped_line_start_index) = group_to_point_index
                    .get(*line_start_index)
                    .and_then(|index| *index)
                else {
                    circle.binding = None;
                    continue;
                };
                let Some(mapped_line_end_index) = group_to_point_index
                    .get(*line_end_index)
                    .and_then(|index| *index)
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
            .and_then(|index| *index)
        else {
            circle.binding = None;
            continue;
        };
        let Some(mapped_center_index) = group_to_point_index
            .get(*center_index)
            .and_then(|index| *index)
        else {
            circle.binding = None;
            continue;
        };
        *source_index = mapped_source_index;
        *center_index = mapped_center_index;
    }
}

pub(super) fn remap_polygon_bindings(
    polygons: &mut [PolygonShape],
    group_to_point_index: &[Option<usize>],
    group_to_polygon_index: &[Option<usize>],
) {
    for polygon in polygons {
        let Some(binding) = polygon.binding.as_mut() else {
            continue;
        };
        let (source_index, center_index) = match binding {
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
                    .and_then(|index| *index)
                else {
                    polygon.binding = None;
                    continue;
                };
                let Some(mapped_line_start_index) = group_to_point_index
                    .get(*line_start_index)
                    .and_then(|index| *index)
                else {
                    polygon.binding = None;
                    continue;
                };
                let Some(mapped_line_end_index) = group_to_point_index
                    .get(*line_end_index)
                    .and_then(|index| *index)
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
            .and_then(|index| *index)
        else {
            polygon.binding = None;
            continue;
        };
        let Some(mapped_center_index) = group_to_point_index
            .get(*center_index)
            .and_then(|index| *index)
        else {
            polygon.binding = None;
            continue;
        };
        *source_index = mapped_source_index;
        *center_index = mapped_center_index;
    }
}

pub(super) fn remap_line_bindings(lines: &mut [LineShape], group_to_point_index: &[Option<usize>]) {
    for line in lines {
        let Some(LineBinding::RotateEdge {
            center_index,
            vertex_index,
            ..
        }) = line.binding.as_mut()
        else {
            continue;
        };
        let Some(mapped_center_index) = group_to_point_index
            .get(*center_index)
            .and_then(|index| *index)
        else {
            line.binding = None;
            continue;
        };
        let Some(mapped_vertex_index) = group_to_point_index
            .get(*vertex_index)
            .and_then(|index| *index)
        else {
            line.binding = None;
            continue;
        };
        *center_index = mapped_center_index;
        *vertex_index = mapped_vertex_index;
    }
}

pub(super) fn collect_point_iteration_points(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
    group_to_point_index: &[Option<usize>],
) -> Vec<ScenePoint> {
    let mut derived_points = Vec::new();

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
            .and_then(|index| *index)
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
                let depth = iter_group
                    .records
                    .iter()
                    .find(|record| record.record_type == 0x090a)
                    .map(|record| record.payload(&file.data))
                    .filter(|payload| payload.len() >= 20)
                    .map(|payload| read_u32(payload, 16) as usize)
                    .unwrap_or(0);
                if depth == 0 {
                    continue;
                }
                let Some(seed_position) = anchors.get(seed_group_index).cloned().flatten() else {
                    continue;
                };

                let mut previous_index = seed_index + derived_points.len();
                let mut current_position = seed_position;
                for _ in 0..depth {
                    current_position = PointRecord {
                        x: current_position.x + dx,
                        y: current_position.y + dy,
                    };
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
            }
            89 => {
                let Some(iter_path) = find_indexed_path(file, iter_group) else {
                    continue;
                };
                let depth = iter_group
                    .records
                    .iter()
                    .find(|record| record.record_type == 0x090a)
                    .map(|record| record.payload(&file.data))
                    .filter(|payload| payload.len() >= 20)
                    .map(|payload| read_u32(payload, 16) as usize)
                    .unwrap_or(0);
                if depth == 0 {
                    continue;
                }
                if let Some((dx, dy)) = parameter_iteration_step(groups, iter_group, anchors, file)
                {
                    let Some(seed_position) = anchors.get(seed_group_index).cloned().flatten()
                    else {
                        continue;
                    };
                    let mut previous_index = seed_index + derived_points.len();
                    let mut current_position = seed_position;
                    for _ in 0..depth {
                        current_position = PointRecord {
                            x: current_position.x + dx,
                            y: current_position.y + dy,
                        };
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
                } else if let Some((center_group_index, _angle_expr, _parameter_name, n)) =
                    regular_polygon_iteration_step(file, groups, iter_group)
                {
                    let Some(center_index) = group_to_point_index
                        .get(center_group_index)
                        .and_then(|index| *index)
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
                } else if iter_path.refs.len() >= 2 {
                    let _ = iter_path;
                }
            }
            _ => {}
        }
    }

    derived_points
}

fn parameter_iteration_step(
    groups: &[ObjectGroup],
    iter_group: &ObjectGroup,
    anchors: &[Option<PointRecord>],
    file: &GspFile,
) -> Option<(f64, f64)> {
    let path = find_indexed_path(file, iter_group)?;
    if path.refs.len() < 3 {
        return None;
    }
    let parameter_group = groups.get(path.refs[0].checked_sub(1)?)?;
    if (parameter_group.header.class_id & 0xffff) != 0 {
        return None;
    }
    let base_start = anchors.get(path.refs[1].checked_sub(1)?)?.clone()?;
    let base_end = anchors.get(path.refs[2].checked_sub(1)?)?.clone()?;
    Some((base_end.x - base_start.x, base_end.y - base_start.y))
}

pub(super) fn regular_polygon_iteration_step(
    file: &GspFile,
    groups: &[ObjectGroup],
    iter_group: &ObjectGroup,
) -> Option<(usize, FunctionExpr, String, f64)> {
    let path = find_indexed_path(file, iter_group)?;
    if path.refs.len() < 3 {
        return None;
    }
    let seed_group = groups.get(path.refs[2].checked_sub(1)?)?;
    if (seed_group.header.class_id & 0xffff) != 29 {
        return None;
    }
    let seed_path = find_indexed_path(file, seed_group)?;
    if seed_path.refs.len() < 3 {
        return None;
    }
    let center_group_index = seed_path.refs[1].checked_sub(1)?;
    let calc_group = groups.get(seed_path.refs[2].checked_sub(1)?)?;
    let calc_path = find_indexed_path(file, calc_group)?;
    let parameter_group = groups.get(calc_path.refs.first()?.checked_sub(1)?)?;
    let parameter_name = decode_label_name(file, parameter_group)?;
    let n = decode_non_graph_parameter_value_for_group(file, parameter_group)?;
    (n.abs() >= 1.0).then_some((
        center_group_index,
        regular_polygon_angle_expr(&parameter_name, n),
        parameter_name,
        n,
    ))
}

pub(super) fn regular_polygon_angle_expr(
    parameter_name: &str,
    parameter_value: f64,
) -> FunctionExpr {
    FunctionExpr::Parsed(ParsedFunctionExpr {
        head: FunctionTerm::Constant(360.0),
        tail: vec![(
            BinaryOp::Div,
            FunctionTerm::Parameter(parameter_name.to_string(), parameter_value),
        )],
    })
}

pub(super) fn decode_regular_polygon_vertex_anchor_raw(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
    anchors: &[Option<PointRecord>],
) -> Option<PointRecord> {
    if (group.header.class_id & 0xffff) != 29 {
        return None;
    }
    let path = find_indexed_path(file, group)?;
    if path.refs.len() < 3 {
        return None;
    }
    let source = anchors.get(path.refs[0].checked_sub(1)?)?.clone()?;
    let center = anchors.get(path.refs[1].checked_sub(1)?)?.clone()?;
    let calc_group = groups.get(path.refs[2].checked_sub(1)?)?;
    let calc_path = find_indexed_path(file, calc_group)?;
    let parameter_group = groups.get(calc_path.refs.first()?.checked_sub(1)?)?;
    let n = decode_non_graph_parameter_value_for_group(file, parameter_group)?;
    if n.abs() < 3.0 {
        return None;
    }
    let radians = (-360.0 / n).to_radians();
    let cos = radians.cos();
    let sin = radians.sin();
    Some(PointRecord {
        x: center.x + (source.x - center.x) * cos + (source.y - center.y) * sin,
        y: center.y - (source.x - center.x) * sin + (source.y - center.y) * cos,
    })
}

pub(super) fn decode_reflection_anchor_raw(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
    anchors: &[Option<PointRecord>],
) -> Option<PointRecord> {
    if (group.header.class_id & 0xffff) != 34 {
        return None;
    }
    let path = find_indexed_path(file, group)?;
    let source_group_index = path.refs.first()?.checked_sub(1)?;
    let source_group = groups.get(source_group_index)?;
    if (source_group.header.class_id & 0xffff) != 0 {
        return None;
    }
    let source = anchors.get(source_group_index)?.clone()?;
    let (line_start_group_index, line_end_group_index) =
        reflection_line_group_indices(file, groups, group)?;
    let line_start = anchors.get(line_start_group_index)?.clone()?;
    let line_end = anchors.get(line_end_group_index)?.clone()?;
    reflect_point_across_line(&source, &line_start, &line_end)
}

pub(super) fn decode_parameter_controlled_anchor_raw(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
    anchors: &[Option<PointRecord>],
) -> Option<PointRecord> {
    decode_parameter_controlled_point(file, groups, group, anchors).map(|point| point.position)
}

pub(super) fn reflection_line_group_indices(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
) -> Option<(usize, usize)> {
    let path = find_indexed_path(file, group)?;
    let line_group = groups.get(path.refs.get(1)?.checked_sub(1)?)?;
    if (line_group.header.class_id & 0xffff) != 2 {
        return None;
    }
    let line_path = find_indexed_path(file, line_group)?;
    Some((
        line_path.refs.first()?.checked_sub(1)?,
        line_path.refs.get(1)?.checked_sub(1)?,
    ))
}

pub(super) fn reflect_point_across_line(
    point: &PointRecord,
    line_start: &PointRecord,
    line_end: &PointRecord,
) -> Option<PointRecord> {
    let dx = line_end.x - line_start.x;
    let dy = line_end.y - line_start.y;
    let len_sq = dx * dx + dy * dy;
    if len_sq <= 1e-9 {
        return None;
    }
    let t = ((point.x - line_start.x) * dx + (point.y - line_start.y) * dy) / len_sq;
    let proj_x = line_start.x + t * dx;
    let proj_y = line_start.y + t * dy;
    Some(PointRecord {
        x: proj_x * 2.0 - point.x,
        y: proj_y * 2.0 - point.y,
    })
}

pub(super) fn decode_point_on_ray_anchor_raw(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
    anchors: &[Option<PointRecord>],
) -> Option<PointRecord> {
    if (group.header.class_id & 0xffff) != 15 {
        return None;
    }

    let host_ref = find_indexed_path(file, group)?
        .refs
        .first()
        .copied()
        .filter(|ordinal| *ordinal > 0)?;
    let host_group = groups.get(host_ref - 1)?;
    if (host_group.header.class_id & 0xffff) != 64 {
        return None;
    }

    let host_path = find_indexed_path(file, host_group)?;
    let origin = anchors
        .get(host_path.refs.first()?.checked_sub(1)?)?
        .clone()?;
    let direction_group = groups.get(host_path.refs.get(1)?.checked_sub(1)?)?;
    let direction_payload = direction_group
        .records
        .iter()
        .find(|record| record.record_type == 0x07d3)
        .map(|record| record.payload(&file.data))?;
    if direction_payload.len() < 20 {
        return None;
    }

    let unit_x = read_f64(direction_payload, 4);
    let unit_y = read_f64(direction_payload, 12);
    if !unit_x.is_finite() || !unit_y.is_finite() {
        return None;
    }

    let payload = group
        .records
        .iter()
        .find(|record| record.record_type == 0x07d3)
        .map(|record| record.payload(&file.data))?;
    if payload.len() < 12 {
        return None;
    }

    let distance = read_f64(payload, 4);
    if !distance.is_finite() {
        return None;
    }

    Some(PointRecord {
        x: origin.x + distance * unit_x,
        y: origin.y - distance * unit_y,
    })
}

pub(super) fn decode_translated_point_anchor_raw(
    file: &GspFile,
    group: &ObjectGroup,
    anchors: &[Option<PointRecord>],
) -> Option<PointRecord> {
    let constraint = decode_translated_point_constraint(file, group)?;
    let path = find_indexed_path(file, group)?;
    let origin = anchors.get(path.refs.first()?.checked_sub(1)?)?.clone()?;
    Some(PointRecord {
        x: origin.x + constraint.dx,
        y: origin.y + constraint.dy,
    })
}

fn decode_translated_point_constraint(
    file: &GspFile,
    group: &ObjectGroup,
) -> Option<TranslatedPointConstraint> {
    if (group.header.class_id & 0xffff) != 21 {
        return None;
    }

    let path = find_indexed_path(file, group)?;
    let origin_group_index = path.refs.first()?.checked_sub(1)?;
    let payload = group
        .records
        .iter()
        .find(|record| record.record_type == 0x07d3)
        .map(|record| record.payload(&file.data))?;
    if payload.len() < 48 {
        return None;
    }

    let angle_degrees = read_f64(payload, 20);
    let units_to_raw = read_f64(payload, 32);
    let distance = read_f64(payload, 40);
    if !angle_degrees.is_finite() || !units_to_raw.is_finite() || !distance.is_finite() {
        return None;
    }

    let angle_radians = angle_degrees.to_radians();
    let step = units_to_raw * distance;
    Some(TranslatedPointConstraint {
        origin_group_index,
        dx: step * angle_radians.cos(),
        dy: -step * angle_radians.sin(),
    })
}

pub(super) fn decode_offset_anchor_raw(
    file: &GspFile,
    group: &ObjectGroup,
    anchors: &[Option<PointRecord>],
) -> Option<PointRecord> {
    if (group.header.class_id & 0xffff) != 67 {
        return None;
    }

    let path = find_indexed_path(file, group)?;
    let origin = anchors.get(path.refs.first()?.checked_sub(1)?)?.clone()?;
    let payload = group
        .records
        .iter()
        .find(|record| record.record_type == 0x07d3)
        .map(|record| record.payload(&file.data))?;
    if payload.len() < 20 {
        return None;
    }

    let dx = read_f64(payload, 4);
    let dy = read_f64(payload, 12);
    if !dx.is_finite() || !dy.is_finite() {
        return None;
    }

    Some(PointRecord {
        x: origin.x + dx,
        y: origin.y + dy,
    })
}

pub(super) struct PointOnSegmentConstraint {
    pub(super) start_group_index: usize,
    pub(super) end_group_index: usize,
    pub(super) t: f64,
}

struct TranslatedPointConstraint {
    origin_group_index: usize,
    dx: f64,
    dy: f64,
}

pub(super) struct PointOnCircleConstraint {
    pub(super) center_group_index: usize,
    pub(super) radius_group_index: usize,
    pub(super) unit_x: f64,
    pub(super) unit_y: f64,
}

pub(super) enum RawPointConstraint {
    Segment(PointOnSegmentConstraint),
    Polyline {
        function_key: usize,
        points: Vec<PointRecord>,
        segment_index: usize,
        t: f64,
    },
    PolygonBoundary {
        vertex_group_indices: Vec<usize>,
        edge_index: usize,
        t: f64,
    },
    Circle(PointOnCircleConstraint),
}

struct ParameterControlledPoint {
    position: PointRecord,
    constraint: RawPointConstraint,
    parameter_name: String,
    source_point_group_index: Option<usize>,
}

struct CoordinatePoint {
    position: PointRecord,
    parameter_name: String,
    expr: FunctionExpr,
}

pub(super) struct TransformBinding {
    pub(super) source_group_index: usize,
    pub(super) center_group_index: usize,
    pub(super) kind: TransformBindingKind,
}

pub(super) enum TransformBindingKind {
    Rotate { angle_degrees: f64 },
    Scale { factor: f64 },
}

fn decode_point_on_segment_constraint(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
) -> Option<PointOnSegmentConstraint> {
    if (group.header.class_id & 0xffff) != 15 {
        return None;
    }

    let host_ref = find_indexed_path(file, group)?
        .refs
        .first()
        .copied()
        .filter(|ordinal| *ordinal > 0)?;
    let host_group_index = host_ref - 1;
    let host_group = groups.get(host_group_index)?;
    let host_path = find_indexed_path(file, host_group)?;
    if host_path.refs.len() != 2 {
        return None;
    }

    let payload = group
        .records
        .iter()
        .find(|record| record.record_type == 0x07d3 && record.length == 12)
        .map(|record| record.payload(&file.data))?;
    let t = read_f64(payload, 4);
    if !t.is_finite() {
        return None;
    }

    Some(PointOnSegmentConstraint {
        start_group_index: host_path.refs[0].checked_sub(1)?,
        end_group_index: host_path.refs[1].checked_sub(1)?,
        t,
    })
}

fn decode_parameter_controlled_point(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
    anchors: &[Option<PointRecord>],
) -> Option<ParameterControlledPoint> {
    if (group.header.class_id & 0xffff) != 95 {
        return None;
    }

    let path = find_indexed_path(file, group)?;
    if path.refs.len() < 2 {
        return None;
    }

    let source_group = groups.get(path.refs[0].checked_sub(1)?)?;
    let host_group = groups.get(path.refs[1].checked_sub(1)?)?;
    let (parameter_name, parameter_value, source_point_group_index) =
        if (source_group.header.class_id & 0xffff) == 0 {
            (
                decode_label_name(file, source_group)?,
                decode_non_graph_parameter_value_for_group(file, source_group)?.clamp(0.0, 1.0),
                None,
            )
        } else if (source_group.header.class_id & 0xffff) == 94 {
            let path = find_indexed_path(file, source_group)?;
            let point_group_index = path.refs.first()?.checked_sub(1)?;
            let point_group = groups.get(point_group_index)?;
            let t = match decode_point_constraint(file, groups, point_group, &None)? {
                RawPointConstraint::Segment(constraint) => constraint.t,
                RawPointConstraint::PolygonBoundary {
                    edge_index,
                    t,
                    vertex_group_indices,
                } => super::labels::polygon_boundary_parameter(
                    anchors,
                    &vertex_group_indices,
                    edge_index,
                    t,
                )?,
                RawPointConstraint::Circle(constraint) => super::labels::circle_parameter(
                    anchors,
                    constraint.center_group_index,
                    constraint.radius_group_index,
                    constraint.unit_x,
                    constraint.unit_y,
                )?,
                RawPointConstraint::Polyline { .. } => return None,
            };
            (String::new(), t.clamp(0.0, 1.0), Some(point_group_index))
        } else {
            return None;
        };

    match host_group.header.class_id & 0xffff {
        2 => {
            let host_path = find_indexed_path(file, host_group)?;
            if host_path.refs.len() != 2 {
                return None;
            }
            let start_group_index = host_path.refs[0].checked_sub(1)?;
            let end_group_index = host_path.refs[1].checked_sub(1)?;
            let start = anchors.get(start_group_index)?.clone()?;
            let end = anchors.get(end_group_index)?.clone()?;
            let position = PointRecord {
                x: start.x + (end.x - start.x) * parameter_value,
                y: start.y + (end.y - start.y) * parameter_value,
            };
            Some(ParameterControlledPoint {
                position,
                constraint: RawPointConstraint::Segment(PointOnSegmentConstraint {
                    start_group_index,
                    end_group_index,
                    t: parameter_value,
                }),
                parameter_name,
                source_point_group_index,
            })
        }
        8 => {
            let host_path = find_indexed_path(file, host_group)?;
            let vertex_group_indices = host_path
                .refs
                .iter()
                .map(|vertex| vertex.checked_sub(1))
                .collect::<Option<Vec<_>>>()?;
            let vertices = vertex_group_indices
                .iter()
                .map(|group_index| anchors.get(*group_index)?.clone())
                .collect::<Option<Vec<_>>>()?;
            let (edge_index, t) = polygon_parameter_to_edge(&vertices, parameter_value)?;
            let position = resolve_polygon_boundary_point_raw(&vertices, edge_index, t)?;
            Some(ParameterControlledPoint {
                position,
                constraint: RawPointConstraint::PolygonBoundary {
                    vertex_group_indices,
                    edge_index,
                    t,
                },
                parameter_name,
                source_point_group_index,
            })
        }
        3 => {
            let host_path = find_indexed_path(file, host_group)?;
            if host_path.refs.len() != 2 {
                return None;
            }
            let center_group_index = host_path.refs[0].checked_sub(1)?;
            let radius_group_index = host_path.refs[1].checked_sub(1)?;
            let center = anchors.get(center_group_index)?.clone()?;
            let radius_point = anchors.get(radius_group_index)?.clone()?;
            let angle = std::f64::consts::TAU * parameter_value;
            let unit_x = angle.cos();
            let unit_y = angle.sin();
            let position = resolve_circle_point_raw(&center, &radius_point, unit_x, unit_y);
            Some(ParameterControlledPoint {
                position,
                constraint: RawPointConstraint::Circle(PointOnCircleConstraint {
                    center_group_index,
                    radius_group_index,
                    unit_x,
                    unit_y,
                }),
                parameter_name,
                source_point_group_index,
            })
        }
        _ => None,
    }
}

fn decode_coordinate_point(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
    graph: &Option<GraphTransform>,
) -> Option<CoordinatePoint> {
    if (group.header.class_id & 0xffff) != 69 {
        return None;
    }

    let path = find_indexed_path(file, group)?;
    if path.refs.len() < 2 {
        return None;
    }
    let parameter_group = groups.get(path.refs[0].checked_sub(1)?)?;
    let calc_group = groups.get(path.refs[1].checked_sub(1)?)?;

    let parameter_name = decode_label_name(file, parameter_group)?;
    let parameter_value = decode_non_graph_parameter_value_for_group(file, parameter_group)?;
    let expr = decode_function_expr(file, groups, calc_group)?;
    let parameters = BTreeMap::from([(parameter_name.clone(), parameter_value)]);
    let y = evaluate_expr_with_parameters(&expr, 0.0, &parameters)?;
    let world = PointRecord {
        x: parameter_value,
        y,
    };
    let position = if let Some(transform) = graph {
        to_raw_from_world(&world, transform)
    } else {
        world
    };

    Some(CoordinatePoint {
        position,
        parameter_name,
        expr,
    })
}

pub(super) fn polygon_parameter_to_edge(
    vertices: &[PointRecord],
    parameter: f64,
) -> Option<(usize, f64)> {
    if vertices.len() < 2 {
        return None;
    }
    let clamped = parameter.clamp(0.0, 1.0);
    let lengths = (0..vertices.len())
        .map(|index| {
            let start = &vertices[index];
            let end = &vertices[(index + 1) % vertices.len()];
            ((end.x - start.x).powi(2) + (end.y - start.y).powi(2)).sqrt()
        })
        .collect::<Vec<_>>();
    let perimeter: f64 = lengths.iter().sum();
    if perimeter <= 1e-9 {
        return None;
    }

    let target = clamped * perimeter;
    let mut traveled = 0.0;
    for (edge_index, length) in lengths.iter().enumerate() {
        if traveled + length >= target || edge_index == lengths.len() - 1 {
            let local_t = if *length <= 1e-9 {
                0.0
            } else {
                ((target - traveled) / length).clamp(0.0, 1.0)
            };
            return Some((edge_index, local_t));
        }
        traveled += length;
    }
    None
}

pub(super) fn decode_point_constraint(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
    graph: &Option<GraphTransform>,
) -> Option<RawPointConstraint> {
    if (group.header.class_id & 0xffff) != 15 {
        return None;
    }

    let host_ref = find_indexed_path(file, group)?
        .refs
        .first()
        .copied()
        .filter(|ordinal| *ordinal > 0)?;
    let host_group = groups.get(host_ref - 1)?;
    let host_kind = host_group.header.class_id & 0xffff;
    let payload = group
        .records
        .iter()
        .find(|record| record.record_type == 0x07d3)
        .map(|record| record.payload(&file.data))?;

    match (host_kind, payload.len()) {
        (3, 20) => {
            let host_path = find_indexed_path(file, host_group)?;
            if host_path.refs.len() != 2 {
                return None;
            }

            let unit_x = read_f64(payload, 4);
            let unit_y = read_f64(payload, 12);
            if !unit_x.is_finite() || !unit_y.is_finite() {
                return None;
            }

            Some(RawPointConstraint::Circle(PointOnCircleConstraint {
                center_group_index: host_path.refs[0].checked_sub(1)?,
                radius_group_index: host_path.refs[1].checked_sub(1)?,
                unit_x,
                unit_y,
            }))
        }
        (8, 20) => {
            let host_path = find_indexed_path(file, host_group)?;
            if host_path.refs.len() < 2 {
                return None;
            }

            let t = read_f64(payload, 4);
            if !t.is_finite() {
                return None;
            }

            Some(RawPointConstraint::PolygonBoundary {
                vertex_group_indices: host_path
                    .refs
                    .iter()
                    .map(|vertex| vertex.checked_sub(1))
                    .collect::<Option<Vec<_>>>()?,
                edge_index: decode_polygon_edge_index(host_path.refs.len(), payload)?,
                t,
            })
        }
        (72, 12) => decode_point_on_function_constraint(file, groups, host_group, payload, graph),
        _ => {
            decode_point_on_segment_constraint(file, groups, group).map(RawPointConstraint::Segment)
        }
    }
}

fn decode_point_on_function_constraint(
    file: &GspFile,
    groups: &[ObjectGroup],
    host_group: &ObjectGroup,
    payload: &[u8],
    graph: &Option<GraphTransform>,
) -> Option<RawPointConstraint> {
    let transform = graph.as_ref()?;
    let normalized_t = read_f64(payload, 4);
    if !normalized_t.is_finite() {
        return None;
    }

    let path = find_indexed_path(file, host_group)?;
    let definition_group = groups.get(path.refs.first()?.checked_sub(1)?)?;
    let descriptor = host_group
        .records
        .iter()
        .find(|record| record.record_type == 0x0902)
        .and_then(|record| decode_function_plot_descriptor(record.payload(&file.data)))?;
    let expr = decode_function_expr(file, groups, definition_group)?;
    let points = sample_function_points(&expr, &descriptor)
        .into_iter()
        .flatten()
        .map(|point| to_raw_from_world(&point, transform))
        .collect::<Vec<_>>();
    let (segment_index, t) = locate_polyline_parameter(&points, normalized_t)?;
    Some(RawPointConstraint::Polyline {
        function_key: *path.refs.first()?,
        points,
        segment_index,
        t,
    })
}

fn locate_polyline_parameter(points: &[PointRecord], normalized_t: f64) -> Option<(usize, f64)> {
    if points.len() < 2 {
        return None;
    }

    let clamped_t = normalized_t.clamp(0.0, 1.0);
    let scaled = clamped_t * (points.len() - 1) as f64;
    let segment_index = scaled.floor() as usize;
    Some((segment_index.min(points.len() - 2), scaled.fract()))
}

fn decode_polygon_edge_index(vertex_count: usize, payload: &[u8]) -> Option<usize> {
    if vertex_count < 2 || payload.len() < 16 {
        return None;
    }

    let discrete = read_u32(payload, 12) as usize;
    if discrete < vertex_count {
        return Some(discrete);
    }

    let selector = read_f64(payload, 12);
    if !selector.is_finite() {
        return None;
    }
    let end_vertex = ((selector * vertex_count as f64) - 0.25).round() as isize;
    Some(((end_vertex + vertex_count as isize - 1).rem_euclid(vertex_count as isize)) as usize)
}

pub(super) fn decode_point_constraint_anchor(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
    anchors: &[Option<PointRecord>],
    graph: Option<&GraphTransform>,
) -> Option<PointRecord> {
    let graph = graph.cloned();
    match decode_point_constraint(file, groups, group, &graph)? {
        RawPointConstraint::Segment(constraint) => {
            let start = anchors.get(constraint.start_group_index)?.clone()?;
            let end = anchors.get(constraint.end_group_index)?.clone()?;

            Some(PointRecord {
                x: start.x + (end.x - start.x) * constraint.t,
                y: start.y + (end.y - start.y) * constraint.t,
            })
        }
        RawPointConstraint::Polyline {
            points,
            segment_index,
            t,
            ..
        } => resolve_polyline_point(&points, segment_index, t),
        RawPointConstraint::PolygonBoundary {
            vertex_group_indices,
            edge_index,
            t,
        } => {
            let vertices = vertex_group_indices
                .iter()
                .map(|group_index| anchors.get(*group_index)?.clone())
                .collect::<Option<Vec<_>>>()?;
            resolve_polygon_boundary_point_raw(&vertices, edge_index, t)
        }
        RawPointConstraint::Circle(constraint) => {
            let center = anchors.get(constraint.center_group_index)?.clone()?;
            let radius_point = anchors.get(constraint.radius_group_index)?.clone()?;

            Some(resolve_circle_point_raw(
                &center,
                &radius_point,
                constraint.unit_x,
                constraint.unit_y,
            ))
        }
    }
}

fn resolve_circle_point_raw(
    center: &PointRecord,
    radius_point: &PointRecord,
    unit_x: f64,
    unit_y: f64,
) -> PointRecord {
    let radius = ((radius_point.x - center.x).powi(2) + (radius_point.y - center.y).powi(2)).sqrt();
    PointRecord {
        x: center.x + radius * unit_x,
        y: center.y - radius * unit_y,
    }
}

fn resolve_polygon_boundary_point_raw(
    vertices: &[PointRecord],
    edge_index: usize,
    t: f64,
) -> Option<PointRecord> {
    if vertices.len() < 2 {
        return None;
    }

    let start = &vertices[edge_index % vertices.len()];
    let end = &vertices[(edge_index + 1) % vertices.len()];
    Some(PointRecord {
        x: start.x + (end.x - start.x) * t,
        y: start.y + (end.y - start.y) * t,
    })
}

fn resolve_polyline_point(
    points: &[PointRecord],
    segment_index: usize,
    t: f64,
) -> Option<PointRecord> {
    if points.len() < 2 {
        return None;
    }

    let start = &points[segment_index.min(points.len() - 2)];
    let end = &points[(segment_index.min(points.len() - 2)) + 1];
    Some(PointRecord {
        x: start.x + (end.x - start.x) * t,
        y: start.y + (end.y - start.y) * t,
    })
}

pub(super) fn decode_transform_binding(
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
