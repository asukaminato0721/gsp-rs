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
    let line = resolve_line_constraint(file, groups, host_group, anchors, group_to_point_index)?;
    let (constraint, binding) = match line {
        LineConstraint::Segment {
            start_index,
            end_index,
        }
        | LineConstraint::Line {
            start_index,
            end_index,
        }
        | LineConstraint::Ray {
            start_index,
            end_index,
        } => (
            ScenePointConstraint::OnSegment {
                start_index,
                end_index,
                t: 0.5,
            },
            Some(ScenePointBinding::Midpoint {
                start_index,
                end_index,
            }),
        ),
        line => (
            ScenePointConstraint::OnLineConstraint { line, t: 0.5 },
            None,
        ),
    };
    let position = anchors.get(index).cloned().flatten()?;

    Some(scene_point(
        position,
        group_color(group),
        visible,
        true,
        constraint,
        binding,
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
        resolve_intersection_line_constraint(file, groups, left_group, anchors, group_to_point_index),
        resolve_intersection_line_constraint(file, groups, right_group, anchors, group_to_point_index),
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

    if let (Some(line), Some(vertex_indices)) = (
        resolve_intersection_line_constraint(file, groups, left_group, anchors, group_to_point_index),
        resolve_polygon_vertex_indices(file, right_group, group_to_point_index),
    ) {
        return Some(scene_point(
            position,
            group_color(group),
            visible,
            true,
            ScenePointConstraint::LinePolygonIntersection {
                line,
                vertex_indices,
                variant: intersection_variant(group.header.kind()),
            },
            None,
        ));
    }

    if let (Some(vertex_indices), Some(line)) = (
        resolve_polygon_vertex_indices(file, left_group, group_to_point_index),
        resolve_intersection_line_constraint(file, groups, right_group, anchors, group_to_point_index),
    ) {
        return Some(scene_point(
            position,
            group_color(group),
            visible,
            true,
            ScenePointConstraint::LinePolygonIntersection {
                line,
                vertex_indices,
                variant: intersection_variant(group.header.kind()),
            },
            None,
        ));
    }

    if let (Some(line), Some((trace_key, point_index, x_min, x_max, sample_count))) = (
        resolve_intersection_line_constraint(file, groups, left_group, anchors, group_to_point_index),
        decode_trace_constraint(file, groups, right_group, group_to_point_index),
    ) {
        return Some(scene_point(
            position,
            group_color(group),
            visible,
            true,
            ScenePointConstraint::LineTraceIntersection {
                line,
                trace_key,
                point_index,
                x_min,
                x_max,
                sample_count,
                variant: trace_intersection_variant(file, group),
            },
            None,
        ));
    }

    if let (Some((trace_key, point_index, x_min, x_max, sample_count)), Some(line)) = (
        decode_trace_constraint(file, groups, left_group, group_to_point_index),
        resolve_intersection_line_constraint(file, groups, right_group, anchors, group_to_point_index),
    ) {
        return Some(scene_point(
            position,
            group_color(group),
            visible,
            true,
            ScenePointConstraint::LineTraceIntersection {
                line,
                trace_key,
                point_index,
                x_min,
                x_max,
                sample_count,
                variant: trace_intersection_variant(file, group),
            },
            None,
        ));
    }

    if let (Some(circle), Some((trace_key, point_index, x_min, x_max, sample_count))) = (
        resolve_circular_constraint(file, groups, left_group, group_to_point_index),
        decode_trace_constraint(file, groups, right_group, group_to_point_index),
    ) {
        return Some(scene_point(
            position,
            group_color(group),
            visible,
            true,
            ScenePointConstraint::CircularTraceIntersection {
                circle,
                trace_key,
                point_index,
                x_min,
                x_max,
                sample_count,
                variant: trace_intersection_variant(file, group),
                sample_hint: intersection_sample_hint(file, group),
            },
            None,
        ));
    }

    if let (Some((trace_key, point_index, x_min, x_max, sample_count)), Some(circle)) = (
        decode_trace_constraint(file, groups, left_group, group_to_point_index),
        resolve_circular_constraint(file, groups, right_group, group_to_point_index),
    ) {
        return Some(scene_point(
            position,
            group_color(group),
            visible,
            true,
            ScenePointConstraint::CircularTraceIntersection {
                circle,
                trace_key,
                point_index,
                x_min,
                x_max,
                sample_count,
                variant: trace_intersection_variant(file, group),
                sample_hint: intersection_sample_hint(file, group),
            },
            None,
        ));
    }

    if let (Some(line), Some((function_key, expr, descriptor))) = (
        resolve_intersection_line_constraint(file, groups, left_group, anchors, group_to_point_index),
        decode_function_plot_constraint(file, groups, right_group),
    ) {
        return Some(scene_point(
            position,
            group_color(group),
            visible,
            true,
            ScenePointConstraint::LineFunctionIntersection {
                line,
                function_key,
                expr,
                x_min: descriptor.x_min,
                x_max: descriptor.x_max,
                sample_count: descriptor.sample_count,
                polar: descriptor.mode == crate::runtime::functions::FunctionPlotMode::Polar,
                sample_hint: intersection_sample_hint(file, group),
            },
            None,
        ));
    }

    if let (Some((function_key, expr, descriptor)), Some(line)) = (
        decode_function_plot_constraint(file, groups, left_group),
        resolve_intersection_line_constraint(file, groups, right_group, anchors, group_to_point_index),
    ) {
        return Some(scene_point(
            position,
            group_color(group),
            visible,
            true,
            ScenePointConstraint::LineFunctionIntersection {
                line,
                function_key,
                expr,
                x_min: descriptor.x_min,
                x_max: descriptor.x_max,
                sample_count: descriptor.sample_count,
                polar: descriptor.mode == crate::runtime::functions::FunctionPlotMode::Polar,
                sample_hint: intersection_sample_hint(file, group),
            },
            None,
        ));
    }

    let variant = intersection_variant(group.header.kind());
    let left_circular = resolve_circular_constraint(file, groups, left_group, group_to_point_index);
    let right_circular =
        resolve_circular_constraint(file, groups, right_group, group_to_point_index);
    if let (Some(line), Some((center_index, radius_index))) = (
        resolve_intersection_line_constraint(file, groups, left_group, anchors, group_to_point_index),
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
        resolve_intersection_line_constraint(file, groups, right_group, anchors, group_to_point_index),
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
        resolve_intersection_line_constraint(file, groups, left_group, anchors, group_to_point_index),
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
        resolve_intersection_line_constraint(file, groups, right_group, anchors, group_to_point_index),
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
    anchors: &[Option<PointRecord>],
    group_to_point_index: &[Option<usize>],
) -> Option<LineConstraint> {
    resolve_line_constraint(file, groups, group, anchors, group_to_point_index)
}

fn resolve_line_constraint(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
    anchors: &[Option<PointRecord>],
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
        crate::format::GroupKind::PerpendicularLine | crate::format::GroupKind::ParallelLine => {
            let (through_group_index, host_group_index) =
                constructed_line_parent_group_indices(file, groups, group)?;
            let through_index = (*group_to_point_index.get(through_group_index)?)?;
            let host_group = groups.get(host_group_index)?;
            let host =
                resolve_line_constraint(file, groups, host_group, anchors, group_to_point_index)?;
            let Some((line_start_index, line_end_index, host_is_perpendicular)) =
                line_direction_reference(&host)
            else {
                return Some(if group.header.kind()
                    == crate::format::GroupKind::PerpendicularLine
                {
                    LineConstraint::PerpendicularTo {
                        through_index,
                        line: Box::new(host),
                    }
                } else {
                    LineConstraint::ParallelTo {
                        through_index,
                        line: Box::new(host),
                    }
                });
            };
            let result_is_perpendicular = (group.header.kind()
                == crate::format::GroupKind::PerpendicularLine)
                ^ host_is_perpendicular;
            Some(if result_is_perpendicular {
                LineConstraint::PerpendicularLine {
                    through_index,
                    line_start_index,
                    line_end_index,
                }
            } else {
                LineConstraint::ParallelLine {
                    through_index,
                    line_start_index,
                    line_end_index,
                }
            })
        }
        crate::format::GroupKind::AngleBisectorRay => {
            if path.refs.len() != 3 {
                return None;
            }
            Some(LineConstraint::AngleBisectorRay {
                start_index: (*group_to_point_index.get(path.refs[0].checked_sub(1)?)?)?,
                vertex_index: (*group_to_point_index.get(path.refs[1].checked_sub(1)?)?)?,
                end_index: (*group_to_point_index.get(path.refs[2].checked_sub(1)?)?)?,
            })
        }
        crate::format::GroupKind::Rotation | crate::format::GroupKind::ParameterRotation => {
            let (
                source_group_index,
                center_group_index,
                angle_degrees,
                parameter_name,
                angle_expr,
                angle_parameter_group_ordinals,
            ) = if group.header.kind() == crate::format::GroupKind::ParameterRotation {
                let binding = decode_parameter_rotation_transform_binding_raw(
                    file, groups, group, anchors,
                )?;
                let TransformBindingKind::Rotate {
                    angle_degrees,
                    parameter_name,
                } = binding.kind
                else {
                    return None;
                };
                let angle_group = groups.get(path.refs.get(2)?.checked_sub(1)?)?;
                let (angle_expr, angle_parameter_group_ordinals) = if angle_group.header.kind()
                    == crate::format::GroupKind::FunctionExpr
                {
                    let (angle_expr, _, _) =
                        expression_runtime_context(file, groups, angle_group, anchors)?;
                    let angle_expr =
                        if crate::runtime::functions::function_expr_uses_degree_units(
                            file,
                            groups,
                            angle_group,
                        ) {
                            angle_expr
                        } else {
                            scale_angle_expr_to_degrees(angle_expr)
                        };
                    (
                        Some(angle_expr),
                        function_parameter_group_ordinals(file, groups, angle_group),
                    )
                } else {
                    (None, std::collections::BTreeMap::new())
                };
                (
                    binding.source_group_index,
                    binding.center_group_index,
                    angle_degrees,
                    parameter_name,
                    angle_expr,
                    angle_parameter_group_ordinals,
                )
            } else {
                let binding = try_decode_transform_binding(file, group).ok()?;
                let TransformBindingKind::Rotate {
                    angle_degrees,
                    parameter_name,
                } = binding.kind
                else {
                    return None;
                };
                (
                    binding.source_group_index,
                    binding.center_group_index,
                    angle_degrees,
                    parameter_name,
                    None,
                    std::collections::BTreeMap::new(),
                )
            };
            let source_group = groups.get(source_group_index)?;
            Some(LineConstraint::Rotated {
                line: Box::new(resolve_line_constraint(
                    file,
                    groups,
                    source_group,
                    anchors,
                    group_to_point_index,
                )?),
                rotation: RotationBinding {
                    center_index: mapped_point_index(
                        group_to_point_index,
                        center_group_index,
                    )?,
                    angle_degrees,
                    parameter_name,
                    angle_expr,
                    angle_parameter_group_ordinals,
                    angle_start_index: None,
                    angle_vertex_index: None,
                    angle_end_index: None,
                },
            })
        }
        crate::format::GroupKind::AngleRotation => {
            let binding = try_decode_angle_rotation_binding(file, group).ok()?;
            let source_group = groups.get(binding.source_group_index)?;
            let angle_start = anchors.get(binding.angle_start_group_index)?.clone()?;
            let angle_vertex = anchors.get(binding.angle_vertex_group_index)?.clone()?;
            let angle_end = anchors.get(binding.angle_end_group_index)?.clone()?;
            Some(LineConstraint::Rotated {
                line: Box::new(resolve_line_constraint(
                    file,
                    groups,
                    source_group,
                    anchors,
                    group_to_point_index,
                )?),
                rotation: RotationBinding {
                    center_index: mapped_point_index(
                        group_to_point_index,
                        binding.center_group_index,
                    )?,
                    angle_degrees: angle_degrees_from_points(
                        &angle_start,
                        &angle_vertex,
                        &angle_end,
                    )?,
                    parameter_name: None,
                    angle_expr: None,
                    angle_parameter_group_ordinals: std::collections::BTreeMap::new(),
                    angle_start_index: Some(mapped_point_index(
                        group_to_point_index,
                        binding.angle_start_group_index,
                    )?),
                    angle_vertex_index: Some(mapped_point_index(
                        group_to_point_index,
                        binding.angle_vertex_group_index,
                    )?),
                    angle_end_index: Some(mapped_point_index(
                        group_to_point_index,
                        binding.angle_end_group_index,
                    )?),
                },
            })
        }
        crate::format::GroupKind::Translation => {
            if path.refs.len() < 3 {
                return None;
            }
            let source_group = groups.get(path.refs[0].checked_sub(1)?)?;
            let line =
                resolve_line_constraint(file, groups, source_group, anchors, group_to_point_index)?;
            Some(LineConstraint::Translated {
                line: Box::new(line),
                vector_start_index: (*group_to_point_index.get(path.refs[1].checked_sub(1)?)?)?,
                vector_end_index: (*group_to_point_index.get(path.refs[2].checked_sub(1)?)?)?,
            })
        }
        crate::format::GroupKind::CartesianOffsetPoint
        | crate::format::GroupKind::PolarOffsetPoint => {
            let translation = decode_translated_point_constraint(file, group)?;
            let source_group = groups.get(translation.origin_group_index)?;
            Some(LineConstraint::TranslatedDelta {
                line: Box::new(resolve_line_constraint(
                    file,
                    groups,
                    source_group,
                    anchors,
                    group_to_point_index,
                )?),
                dx: translation.dx,
                dy: translation.dy,
            })
        }
        crate::format::GroupKind::Reflection => {
            if path.refs.len() != 2 {
                return None;
            }
            let source = groups.get(path.refs[0].checked_sub(1)?)?;
            let axis = groups.get(path.refs[1].checked_sub(1)?)?;
            Some(LineConstraint::Reflected {
                line: Box::new(resolve_line_constraint(
                    file,
                    groups,
                    source,
                    anchors,
                    group_to_point_index,
                )?),
                axis: Box::new(resolve_line_constraint(
                    file,
                    groups,
                    axis,
                    anchors,
                    group_to_point_index,
                )?),
            })
        }
        _ => None,
    }
}

fn line_direction_reference(constraint: &LineConstraint) -> Option<(usize, usize, bool)> {
    match constraint {
        LineConstraint::Segment {
            start_index,
            end_index,
        }
        | LineConstraint::Line {
            start_index,
            end_index,
        }
        | LineConstraint::Ray {
            start_index,
            end_index,
        } => Some((*start_index, *end_index, false)),
        LineConstraint::PerpendicularLine {
            line_start_index,
            line_end_index,
            ..
        } => Some((*line_start_index, *line_end_index, true)),
        LineConstraint::ParallelLine {
            line_start_index,
            line_end_index,
            ..
        } => Some((*line_start_index, *line_end_index, false)),
        LineConstraint::Translated { line, .. }
        | LineConstraint::TranslatedDelta { line, .. } => line_direction_reference(line),
        LineConstraint::PerpendicularTo { .. } | LineConstraint::ParallelTo { .. } => None,
        LineConstraint::AngleBisectorRay { .. }
        | LineConstraint::Reflected { .. }
        | LineConstraint::Rotated { .. } => None,
    }
}

fn decode_trace_constraint(
    file: &GspFile,
    _groups: &[ObjectGroup],
    group: &ObjectGroup,
    group_to_point_index: &[Option<usize>],
) -> Option<(usize, usize, f64, f64, usize)> {
    if !matches!(
        group.header.kind(),
        crate::format::GroupKind::CoordinateTrace
            | crate::format::GroupKind::PointTrace
            | crate::format::GroupKind::CustomTransformTrace
    ) {
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
        .find(|record| {
            record.record_type == crate::runtime::payload_consts::RECORD_FUNCTION_PLOT_DESCRIPTOR
        })
        .map(|record| record.payload(&file.data))?;
    let descriptor = try_decode_function_plot_descriptor(payload).ok()?;
    Some((
        group.ordinal,
        point_index,
        descriptor.x_min,
        descriptor.x_max,
        descriptor.sample_count,
    ))
}

fn intersection_sample_hint(file: &GspFile, group: &ObjectGroup) -> Option<usize> {
    group
        .records
        .iter()
        .find(|record| record.record_type == crate::runtime::payload_consts::RECORD_BINDING_PAYLOAD)
        .map(|record| record.payload(&file.data))
        .filter(|payload| payload.len() >= 4)
        .map(|payload| crate::format::read_u32(payload, 0) as usize)
}

fn trace_intersection_variant(file: &GspFile, group: &ObjectGroup) -> usize {
    group
        .records
        .iter()
        .find(|record| record.record_type == crate::runtime::payload_consts::RECORD_BINDING_PAYLOAD)
        .map(|record| record.payload(&file.data))
        .filter(|payload| payload.len() >= 8)
        .map_or(0, |payload| crate::format::read_u32(payload, 4) as usize)
}

fn decode_function_plot_constraint(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
) -> Option<(
    usize,
    FunctionExpr,
    crate::runtime::functions::FunctionPlotDescriptor,
)> {
    if group.header.kind() != crate::format::GroupKind::FunctionPlot {
        return None;
    }
    let path = find_indexed_path(file, group)?;
    let expr_group = groups.get(path.refs.first()?.checked_sub(1)?)?;
    let descriptor_payload = group
        .records
        .iter()
        .find(|record| {
            record.record_type == crate::runtime::payload_consts::RECORD_FUNCTION_PLOT_DESCRIPTOR
        })
        .map(|record| record.payload(&file.data))?;
    let descriptor = try_decode_function_plot_descriptor(descriptor_payload).ok()?;
    let expr = try_decode_function_expr(file, groups, expr_group).ok()?;
    Some((group.ordinal, expr, descriptor))
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

fn resolve_polygon_vertex_indices(
    file: &GspFile,
    group: &ObjectGroup,
    group_to_point_index: &[Option<usize>],
) -> Option<Vec<usize>> {
    if group.header.kind() != crate::format::GroupKind::Polygon {
        return None;
    }
    let path = find_indexed_path(file, group)?;
    (path.refs.len() >= 2).then_some(())?;
    path.refs
        .iter()
        .map(|ordinal| mapped_point_index(group_to_point_index, ordinal.checked_sub(1)?))
        .collect()
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
            let center_index = (*group_to_point_index.get(path.refs[0].checked_sub(1)?)?)?;
            let radius_group = groups.get(path.refs[1].checked_sub(1)?)?;
            if let Some((line_start_group_index, line_end_group_index)) =
                measured_radius_segment_group_indices(file, groups, radius_group)
            {
                Some(CircularConstraint::SegmentRadiusCircle {
                    center_index,
                    line_start_index: (*group_to_point_index.get(line_start_group_index)?)?,
                    line_end_index: (*group_to_point_index.get(line_end_group_index)?)?,
                })
            } else if radius_group.header.kind() == crate::format::GroupKind::FunctionExpr {
                let expr = try_decode_function_expr(file, groups, radius_group).ok()?;
                let initial_value = evaluate_function_group_with_overrides(
                    file,
                    groups,
                    radius_group,
                    &std::collections::BTreeMap::new(),
                )
                .or_else(|| {
                    evaluate_expr_with_parameters(
                        &expr,
                        0.0,
                        &std::collections::BTreeMap::new(),
                    )
                })
                .unwrap_or(0.0);
                Some(CircularConstraint::ExpressionRadiusCircle {
                    center_index,
                    expr,
                    initial_value,
                    parameter_group_ordinals:
                        crate::runtime::functions::function_parameter_group_ordinals(
                            file,
                            groups,
                            radius_group,
                        ),
                })
            } else if let Some(radius) =
                numeric_helper_scalar_binding(file, groups, radius_group)
            {
                Some(CircularConstraint::ExpressionRadiusCircle {
                    center_index,
                    expr: radius.expr,
                    initial_value: radius.initial_value,
                    parameter_group_ordinals: radius.parameter_group_ordinals,
                })
            } else {
                Some(CircularConstraint::ParameterRadiusCircle {
                    center_index,
                    parameter_name: crate::runtime::extract::decode::decode_label_name(
                        file,
                        radius_group,
                    )?,
                    parameter_value:
                        crate::runtime::extract::try_decode_parameter_control_value_for_group(
                            file,
                            groups,
                            radius_group,
                        )
                        .ok()?,
                    raw_per_unit: crate::runtime::DEFAULT_GRAPH_RAW_PER_UNIT,
                })
            }
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
        crate::format::GroupKind::Translation => {
            if path.refs.len() < 3 {
                return None;
            }
            let source_group = groups.get(path.refs[0].checked_sub(1)?)?;
            let source =
                resolve_circular_constraint(file, groups, source_group, group_to_point_index)?;
            Some(CircularConstraint::VectorTranslateCircle {
                source: Box::new(source),
                vector_start_index: mapped_point_index(
                    group_to_point_index,
                    path.refs[1].checked_sub(1)?,
                )?,
                vector_end_index: mapped_point_index(
                    group_to_point_index,
                    path.refs[2].checked_sub(1)?,
                )?,
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
        crate::format::GroupKind::Rotation => {
            let binding = try_decode_transform_binding(file, group).ok()?;
            let TransformBindingKind::Rotate { angle_degrees, .. } = binding.kind else {
                return None;
            };
            let source_group = groups.get(binding.source_group_index)?;
            let source =
                resolve_circular_constraint(file, groups, source_group, group_to_point_index)?;
            let center_index =
                mapped_point_index(group_to_point_index, binding.center_group_index)?;
            Some(CircularConstraint::RotateCircle {
                source: Box::new(source),
                center_index,
                angle_degrees,
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
        | crate::format::GroupKind::CircleCircleIntersectionPoint1 => 0,
        crate::format::GroupKind::IntersectionPoint2
        | crate::format::GroupKind::CircleCircleIntersectionPoint2 => 1,
        _ => 0,
    }
}
