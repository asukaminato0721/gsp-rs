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
    let line = resolve_line_constraint(file, groups, host_group, group_to_point_index)?;
    let (start_index, end_index) = match line {
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
        } => (start_index, end_index),
        _ => return None,
    };
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

    if let (Some(line), Some((expr, descriptor))) = (
        resolve_intersection_line_constraint(file, groups, left_group, group_to_point_index),
        decode_function_plot_constraint(file, groups, right_group),
    ) {
        return Some(scene_point(
            position,
            group_color(group),
            visible,
            true,
            ScenePointConstraint::LineFunctionIntersection {
                line,
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

    if let (Some((expr, descriptor)), Some(line)) = (
        decode_function_plot_constraint(file, groups, left_group),
        resolve_intersection_line_constraint(file, groups, right_group, group_to_point_index),
    ) {
        return Some(scene_point(
            position,
            group_color(group),
            visible,
            true,
            ScenePointConstraint::LineFunctionIntersection {
                line,
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
        crate::format::GroupKind::PerpendicularLine | crate::format::GroupKind::ParallelLine => {
            if path.refs.len() != 2 {
                return None;
            }
            let through_index = (*group_to_point_index.get(path.refs[0].checked_sub(1)?)?)?;
            let host_group = groups.get(path.refs[1].checked_sub(1)?)?;
            let host = resolve_line_constraint(file, groups, host_group, group_to_point_index)?;
            let (line_start_index, line_end_index, host_is_perpendicular) =
                line_direction_reference(&host)?;
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
        crate::format::GroupKind::Rotation => {
            let binding = try_decode_transform_binding(file, group).ok()?;
            let TransformBindingKind::Rotate { angle_degrees, .. } = binding.kind else {
                return None;
            };
            let source_group = groups.get(binding.source_group_index)?;
            let source_path = find_indexed_path(file, source_group)?;
            if source_path.refs.len() != 2 || !source_group.header.kind().is_line_like() {
                return None;
            }
            let start_group_index = source_path.refs[0].checked_sub(1)?;
            let end_group_index = source_path.refs[1].checked_sub(1)?;
            let start_index = mapped_rotated_endpoint_index(
                file,
                groups,
                group_to_point_index,
                start_group_index,
                binding.center_group_index,
                angle_degrees,
            )?;
            let end_index = mapped_rotated_endpoint_index(
                file,
                groups,
                group_to_point_index,
                end_group_index,
                binding.center_group_index,
                angle_degrees,
            )?;
            Some(match source_group.header.kind() {
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
        LineConstraint::Translated { line, .. } => line_direction_reference(line),
        LineConstraint::AngleBisectorRay { .. } => None,
    }
}

fn mapped_rotated_endpoint_index(
    file: &GspFile,
    groups: &[ObjectGroup],
    group_to_point_index: &[Option<usize>],
    source_group_index: usize,
    center_group_index: usize,
    angle_degrees: f64,
) -> Option<usize> {
    if source_group_index == center_group_index {
        return mapped_point_index(group_to_point_index, source_group_index);
    }
    groups
        .iter()
        .enumerate()
        .find_map(|(candidate_index, candidate)| {
            if candidate.header.kind() != crate::format::GroupKind::Rotation {
                return None;
            }
            let binding = try_decode_transform_binding(file, candidate).ok()?;
            let TransformBindingKind::Rotate {
                angle_degrees: candidate_angle,
                ..
            } = binding.kind
            else {
                return None;
            };
            (binding.source_group_index == source_group_index
                && binding.center_group_index == center_group_index
                && (candidate_angle - angle_degrees).abs() < 1e-6)
                .then(|| mapped_point_index(group_to_point_index, candidate_index))
                .flatten()
        })
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

fn intersection_sample_hint(file: &GspFile, group: &ObjectGroup) -> Option<usize> {
    group
        .records
        .iter()
        .find(|record| record.record_type == 0x07d3)
        .map(|record| record.payload(&file.data))
        .filter(|payload| payload.len() >= 4)
        .map(|payload| crate::format::read_u32(payload, 0) as usize)
}

fn decode_function_plot_constraint(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
) -> Option<(
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
    Some((expr, descriptor))
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
        | crate::format::GroupKind::CircleCircleIntersectionPoint1 => 0,
        crate::format::GroupKind::IntersectionPoint2
        | crate::format::GroupKind::CircleCircleIntersectionPoint2 => 1,
        _ => 0,
    }
}
