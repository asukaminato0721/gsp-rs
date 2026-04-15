use std::collections::BTreeMap;

use super::super::decode::{decode_label_name, find_indexed_path};
use super::constraints::{
    RawPointConstraint, decode_translated_point_constraint, try_decode_parameter_controlled_point,
    try_decode_point_constraint,
};
use super::{
    GspFile, ObjectGroup, PointRecord, TransformBindingKind,
    decode_non_graph_parameter_value_for_group, read_f64, try_decode_parameter_rotation_binding,
    try_decode_transform_binding,
};
use crate::runtime::functions::{
    evaluate_expr_with_parameters, try_decode_function_expr, try_decode_function_plot_descriptor,
};
use crate::runtime::geometry::{
    GraphTransform, lerp_point, point_on_circle_arc, point_on_three_point_arc, reflect_across_line,
    rotate_around, three_point_arc_geometry, to_raw_from_world, to_world,
};

const PX_PER_CM: f64 = 37.79527559055118;

fn first_path_group<'a>(
    file: &GspFile,
    groups: &'a [ObjectGroup],
    group: &ObjectGroup,
) -> Option<&'a ObjectGroup> {
    let path = find_indexed_path(file, group)?;
    let ordinal = path.refs.first().copied()?;
    let index = ordinal.checked_sub(1)?;
    groups.get(index)
}

#[derive(Clone)]
pub(crate) struct CustomTransformBindingDef {
    pub(crate) source_group_index: usize,
    pub(crate) origin_group_index: usize,
    pub(crate) axis_end_group_index: usize,
    pub(crate) distance_expr: crate::runtime::functions::FunctionExpr,
    pub(crate) angle_expr: crate::runtime::functions::FunctionExpr,
    pub(crate) distance_raw_scale: f64,
    pub(crate) angle_degrees_scale: f64,
}

pub(crate) fn decode_graph_calibration_anchor_raw(
    group: &ObjectGroup,
    graph: Option<&GraphTransform>,
) -> Option<PointRecord> {
    let graph = graph?;
    match group.header.kind() {
        crate::format::GroupKind::GraphCalibrationX => Some(PointRecord {
            x: graph.origin_raw.x + graph.raw_per_unit,
            y: graph.origin_raw.y,
        }),
        crate::format::GroupKind::GraphCalibrationY => Some(PointRecord {
            x: graph.origin_raw.x,
            y: graph.origin_raw.y - graph.raw_per_unit,
        }),
        _ => None,
    }
}

pub(crate) fn decode_coordinate_expression_anchor_raw(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
    anchors: &[Option<PointRecord>],
    graph: Option<&GraphTransform>,
) -> Option<PointRecord> {
    if !matches!(
        group.header.kind(),
        crate::format::GroupKind::CoordinateExpressionPoint
            | crate::format::GroupKind::CoordinateExpressionPointAlt
            | crate::format::GroupKind::Unknown(20)
    ) {
        return None;
    }

    let path = find_indexed_path(file, group)?;
    if path.refs.len() < 2 {
        return None;
    }
    let source_group_index = path.refs[0].checked_sub(1)?;
    let source_position = anchors.get(source_group_index)?.clone()?;
    let source_world = to_world(&source_position, &graph.cloned());
    let world = match group.header.kind() {
        crate::format::GroupKind::Unknown(20) => {
            let x_calc_group = groups.get(path.refs[1].checked_sub(1)?)?;
            let y_calc_group = groups.get(path.refs[2].checked_sub(1)?)?;
            let x_expr = try_decode_function_expr(file, groups, x_calc_group).ok()?;
            let y_expr = try_decode_function_expr(file, groups, y_calc_group).ok()?;
            let x_parameter_group = first_path_group(file, groups, x_calc_group)?;
            let y_parameter_group = first_path_group(file, groups, y_calc_group)?;
            let x_parameter_name = decode_label_name(file, x_parameter_group)?;
            let y_parameter_name = decode_label_name(file, y_parameter_group)?;
            let x_parameter_value =
                decode_non_graph_parameter_value_for_group(file, x_parameter_group)?;
            let y_parameter_value =
                decode_non_graph_parameter_value_for_group(file, y_parameter_group)?;
            let dx = evaluate_expr_with_parameters(
                &x_expr,
                0.0,
                &BTreeMap::from([(x_parameter_name, x_parameter_value)]),
            )?;
            let dy = evaluate_expr_with_parameters(
                &y_expr,
                0.0,
                &BTreeMap::from([(y_parameter_name, y_parameter_value)]),
            )?;
            PointRecord {
                x: source_world.x + dx,
                y: source_world.y + dy,
            }
        }
        _ => {
            let calc_group = groups.get(path.refs[1].checked_sub(1)?)?;
            let expr = try_decode_function_expr(file, groups, calc_group).ok()?;
            let payload = group
                .records
                .iter()
                .find(|record| record.record_type == 0x07d3)
                .map(|record| record.payload(&file.data))?;
            let axis = match group.header.kind() {
                crate::format::GroupKind::CoordinateExpressionPointAlt => {
                    crate::runtime::scene::CoordinateAxis::Horizontal
                }
                _ => match (payload.len() >= 24).then(|| crate::format::read_u32(payload, 20)) {
                    Some(1) => crate::runtime::scene::CoordinateAxis::Vertical,
                    _ => crate::runtime::scene::CoordinateAxis::Horizontal,
                },
            };
            let parameter_group = first_path_group(file, groups, calc_group)?;
            let parameter_name = decode_label_name(file, parameter_group)?;
            match axis {
                crate::runtime::scene::CoordinateAxis::Horizontal => PointRecord {
                    x: source_world.x
                        + evaluate_expr_with_parameters(
                            &expr,
                            0.0,
                            &BTreeMap::from([(
                                parameter_name,
                                decode_non_graph_parameter_value_for_group(file, parameter_group)?,
                            )]),
                        )?,
                    y: source_world.y,
                },
                crate::runtime::scene::CoordinateAxis::Vertical => PointRecord {
                    x: source_world.x,
                    y: evaluate_expr_with_parameters(
                        &expr,
                        0.0,
                        &BTreeMap::from([(parameter_name, source_world.x)]),
                    )?,
                },
            }
        }
    };
    Some(if let Some(transform) = graph {
        to_raw_from_world(&world, transform)
    } else {
        world
    })
}

pub(crate) fn decode_intersection_anchor_raw(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
    anchors: &[Option<PointRecord>],
    graph: Option<&GraphTransform>,
) -> Option<PointRecord> {
    let kind = group.header.kind();
    let variant = match kind {
        crate::format::GroupKind::LinearIntersectionPoint => None,
        crate::format::GroupKind::IntersectionPoint1 => Some(1),
        crate::format::GroupKind::IntersectionPoint2 => Some(0),
        crate::format::GroupKind::CircleCircleIntersectionPoint1 => Some(1),
        crate::format::GroupKind::CircleCircleIntersectionPoint2 => Some(0),
        crate::format::GroupKind::CoordinateTraceIntersectionPoint => Some(0),
        _ => return None,
    };

    let path = find_indexed_path(file, group)?;
    if path.refs.len() != 2 {
        return None;
    }

    let left_group = groups.get(path.refs[0].checked_sub(1)?)?;
    let right_group = groups.get(path.refs[1].checked_sub(1)?)?;

    if kind == crate::format::GroupKind::CoordinateTraceIntersectionPoint {
        if let (Some((line_start, line_end)), Some(trace_points)) = (
            resolve_line_like_points_raw(file, groups, anchors, left_group),
            sample_coordinate_trace_points_raw(file, groups, right_group, anchors, graph),
        ) {
            return line_polyline_intersection(line_start, line_end, &trace_points);
        }

        if let (Some(trace_points), Some((line_start, line_end))) = (
            sample_coordinate_trace_points_raw(file, groups, left_group, anchors, graph),
            resolve_line_like_points_raw(file, groups, anchors, right_group),
        ) {
            return line_polyline_intersection(line_start, line_end, &trace_points);
        }
    }

    if let (Some((line_start, line_end)), Some(circle)) = (
        resolve_line_like_points_raw(file, groups, anchors, left_group),
        resolve_circle_like_raw(file, groups, anchors, right_group),
    ) {
        return select_line_circle_intersection(
            line_start,
            line_end,
            circle.center(),
            circle.radius(),
            variant.unwrap_or(0),
        );
    }

    if let (Some(circle), Some((line_start, line_end))) = (
        resolve_circle_like_raw(file, groups, anchors, left_group),
        resolve_line_like_points_raw(file, groups, anchors, right_group),
    ) {
        return select_line_circle_intersection(
            line_start,
            line_end,
            circle.center(),
            circle.radius(),
            variant.unwrap_or(0),
        );
    }

    if let (Some(left_circle), Some(right_circle)) = (
        resolve_circle_like_raw(file, groups, anchors, left_group),
        resolve_circle_like_raw(file, groups, anchors, right_group),
    ) {
        return select_circular_intersection(&left_circle, &right_circle, variant.unwrap_or(0));
    }

    if let (Some(point), Some(circle)) = (
        anchors.get(path.refs[0].checked_sub(1)?)?.clone(),
        resolve_circle_like_raw(file, groups, anchors, right_group),
    ) {
        return select_point_circle_tangent(&point, &circle, variant.unwrap_or(0));
    }

    if let (Some(circle), Some(point)) = (
        resolve_circle_like_raw(file, groups, anchors, left_group),
        anchors.get(path.refs[1].checked_sub(1)?)?.clone(),
    ) {
        return select_point_circle_tangent(&point, &circle, variant.unwrap_or(0));
    }

    if variant.is_none() {
        let (left_start, left_end) =
            resolve_line_like_points_raw(file, groups, anchors, left_group)?;
        let (right_start, right_end) =
            resolve_line_like_points_raw(file, groups, anchors, right_group)?;
        return line_line_intersection(&left_start, &left_end, &right_start, &right_end);
    }

    let (left_start, left_end) = resolve_line_like_points_raw(file, groups, anchors, left_group)?;
    let (right_start, right_end) =
        resolve_line_like_points_raw(file, groups, anchors, right_group)?;
    line_line_intersection(&left_start, &left_end, &right_start, &right_end)
}

fn sample_coordinate_trace_points_raw(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
    anchors: &[Option<PointRecord>],
    graph: Option<&GraphTransform>,
) -> Option<Vec<PointRecord>> {
    if (group.header.kind()) != crate::format::GroupKind::CoordinateTrace {
        return None;
    }

    let path = find_indexed_path(file, group)?;
    if path.refs.len() < 3 {
        return None;
    }
    let driver_group = groups.get(path.refs[0].checked_sub(1)?)?;
    let parameter_group = groups.get(path.refs[1].checked_sub(1)?)?;
    let calc_group = groups.get(path.refs[2].checked_sub(1)?)?;
    let payload = group
        .records
        .iter()
        .find(|record| record.record_type == 0x0902)
        .map(|record| record.payload(&file.data))?;
    let descriptor = try_decode_function_plot_descriptor(payload).ok()?;
    let expr = try_decode_function_expr(file, groups, calc_group).ok()?;
    let parameter_name =
        super::editable_non_graph_parameter_name_for_group(file, groups, parameter_group)
            .or_else(|| decode_label_name(file, parameter_group))?;

    let mut points = Vec::with_capacity(descriptor.sample_count);
    let last = descriptor.sample_count.saturating_sub(1).max(1) as f64;
    let driver = if matches!(
        driver_group.header.kind(),
        crate::format::GroupKind::CoordinateExpressionPoint
            | crate::format::GroupKind::CoordinateExpressionPointAlt
            | crate::format::GroupKind::Unknown(20)
    ) {
        let driver_path = find_indexed_path(file, driver_group)?;
        let source_group_index = driver_path.refs[0].checked_sub(1)?;
        let source_position = anchors.get(source_group_index)?.clone()?;
        let source_world = to_world(&source_position, &graph.cloned());
        match driver_group.header.kind() {
            crate::format::GroupKind::Unknown(20) => {
                let x_calc_group = groups.get(driver_path.refs[1].checked_sub(1)?)?;
                let y_calc_group = groups.get(driver_path.refs[2].checked_sub(1)?)?;
                let x_expr = try_decode_function_expr(file, groups, x_calc_group).ok()?;
                let y_expr = try_decode_function_expr(file, groups, y_calc_group).ok()?;
                Some((source_world, None, Some((x_expr, y_expr))))
            }
            crate::format::GroupKind::CoordinateExpressionPointAlt => Some((
                source_world,
                Some(crate::runtime::scene::CoordinateAxis::Horizontal),
                None,
            )),
            _ => {
                let payload = driver_group
                    .records
                    .iter()
                    .find(|record| record.record_type == 0x07d3)
                    .map(|record| record.payload(&file.data))?;
                let axis = match (payload.len() >= 24).then(|| crate::format::read_u32(payload, 20))
                {
                    Some(1) => crate::runtime::scene::CoordinateAxis::Vertical,
                    _ => crate::runtime::scene::CoordinateAxis::Horizontal,
                };
                Some((source_world, Some(axis), None))
            }
        }
    } else {
        None
    };
    for index in 0..descriptor.sample_count {
        let t = index as f64 / last;
        let x = descriptor.x_min + (descriptor.x_max - descriptor.x_min) * t;
        let offset = evaluate_expr_with_parameters(
            &expr,
            0.0,
            &BTreeMap::from([(parameter_name.clone(), x)]),
        )?;
        let world = match &driver {
            Some((_source_world, Some(crate::runtime::scene::CoordinateAxis::Horizontal), _)) => {
                PointRecord { x: offset, y: x }
            }
            Some((_source_world, Some(crate::runtime::scene::CoordinateAxis::Vertical), _)) => {
                PointRecord { x, y: offset }
            }
            Some((source_world, None, Some((x_expr, y_expr)))) => {
                let dx = evaluate_expr_with_parameters(
                    x_expr,
                    0.0,
                    &BTreeMap::from([(parameter_name.clone(), x)]),
                )?;
                let dy = evaluate_expr_with_parameters(
                    y_expr,
                    0.0,
                    &BTreeMap::from([(parameter_name.clone(), x)]),
                )?;
                PointRecord {
                    x: source_world.x + dx,
                    y: source_world.y + dy,
                }
            }
            Some((_, None, None)) => return None,
            None => match descriptor.mode {
                crate::runtime::functions::FunctionPlotMode::Cartesian => {
                    PointRecord { x, y: offset }
                }
                crate::runtime::functions::FunctionPlotMode::Polar => PointRecord {
                    x: offset * x.cos(),
                    y: offset * x.sin(),
                },
            },
        };
        points.push(if let Some(transform) = graph {
            to_raw_from_world(&world, transform)
        } else {
            world
        });
    }
    (points.len() >= 2).then_some(points)
}

fn line_polyline_intersection(
    line_start: PointRecord,
    line_end: PointRecord,
    polyline: &[PointRecord],
) -> Option<PointRecord> {
    polyline.windows(2).find_map(|segment| {
        let start = segment.first()?;
        let end = segment.get(1)?;
        line_line_intersection(&line_start, &line_end, start, end)
    })
}

#[derive(Clone)]
pub(crate) enum CircularConstraintRaw {
    Circle {
        center: PointRecord,
        radius: f64,
    },
    ThreePointArc {
        start: PointRecord,
        mid: PointRecord,
        end: PointRecord,
        center: PointRecord,
        radius: f64,
        ccw_span: f64,
        ccw_mid: f64,
    },
}

impl CircularConstraintRaw {
    fn center(&self) -> PointRecord {
        match self {
            Self::Circle { center, .. } | Self::ThreePointArc { center, .. } => center.clone(),
        }
    }

    fn radius(&self) -> f64 {
        match self {
            Self::Circle { radius, .. } | Self::ThreePointArc { radius, .. } => *radius,
        }
    }
}

pub(crate) fn resolve_circle_like_raw(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
    group: &ObjectGroup,
) -> Option<CircularConstraintRaw> {
    let path = find_indexed_path(file, group)?;
    match group.header.kind() {
        crate::format::GroupKind::Circle => {
            if path.refs.len() != 2 {
                return None;
            }
            let center = anchors.get(path.refs[0].checked_sub(1)?)?.clone()?;
            let radius_point = anchors.get(path.refs[1].checked_sub(1)?)?.clone()?;
            let radius =
                ((radius_point.x - center.x).powi(2) + (radius_point.y - center.y).powi(2)).sqrt();
            (radius > 1e-9).then_some(CircularConstraintRaw::Circle { center, radius })
        }
        crate::format::GroupKind::CircleCenterRadius => {
            if path.refs.len() != 2 {
                return None;
            }
            let center = anchors.get(path.refs[0].checked_sub(1)?)?.clone()?;
            let segment_group = groups.get(path.refs[1].checked_sub(1)?)?;
            let segment_path = find_indexed_path(file, segment_group)?;
            if segment_path.refs.len() != 2 {
                return None;
            }
            let start = anchors.get(segment_path.refs[0].checked_sub(1)?)?.clone()?;
            let end = anchors.get(segment_path.refs[1].checked_sub(1)?)?.clone()?;
            let radius = ((end.x - start.x).powi(2) + (end.y - start.y).powi(2)).sqrt();
            (radius > 1e-9).then_some(CircularConstraintRaw::Circle { center, radius })
        }
        crate::format::GroupKind::CartesianOffsetPoint
        | crate::format::GroupKind::PolarOffsetPoint => {
            let constraint = decode_translated_point_constraint(file, group)?;
            let source_group = groups.get(constraint.origin_group_index)?;
            let source = resolve_circle_like_raw(file, groups, anchors, source_group)?;
            match source {
                CircularConstraintRaw::Circle { center, radius } => {
                    (radius > 1e-9).then_some(CircularConstraintRaw::Circle {
                        center: PointRecord {
                            x: center.x + constraint.dx,
                            y: center.y + constraint.dy,
                        },
                        radius,
                    })
                }
                CircularConstraintRaw::ThreePointArc {
                    start,
                    mid,
                    end,
                    center,
                    radius,
                    ccw_span,
                    ccw_mid,
                } => Some(CircularConstraintRaw::ThreePointArc {
                    start: PointRecord {
                        x: start.x + constraint.dx,
                        y: start.y + constraint.dy,
                    },
                    mid: PointRecord {
                        x: mid.x + constraint.dx,
                        y: mid.y + constraint.dy,
                    },
                    end: PointRecord {
                        x: end.x + constraint.dx,
                        y: end.y + constraint.dy,
                    },
                    center: PointRecord {
                        x: center.x + constraint.dx,
                        y: center.y + constraint.dy,
                    },
                    radius,
                    ccw_span,
                    ccw_mid,
                }),
            }
        }
        crate::format::GroupKind::Scale => {
            let binding = try_decode_transform_binding(file, group).ok()?;
            let center = anchors.get(binding.center_group_index)?.clone()?;
            let factor = match binding.kind {
                TransformBindingKind::Scale { factor } => factor,
                _ => return None,
            };
            let source_group = groups.get(binding.source_group_index)?;
            let source = resolve_circle_like_raw(file, groups, anchors, source_group)?;
            match source {
                CircularConstraintRaw::Circle { center: source_center, radius } => {
                    (radius > 1e-9).then_some(CircularConstraintRaw::Circle {
                        center: PointRecord {
                            x: center.x + (source_center.x - center.x) * factor,
                            y: center.y + (source_center.y - center.y) * factor,
                        },
                        radius: radius * factor.abs(),
                    })
                }
                CircularConstraintRaw::ThreePointArc { .. } => None,
            }
        }
        crate::format::GroupKind::Reflection => {
            let source_group = groups.get(path.refs.first()?.checked_sub(1)?)?;
            let line_group = groups.get(path.refs.get(1)?.checked_sub(1)?)?;
            let source = resolve_circle_like_raw(file, groups, anchors, source_group)?;
            let (line_start, line_end) =
                super::resolve_line_like_points_raw(file, groups, anchors, line_group)?;
            match source {
                CircularConstraintRaw::Circle { center, radius } => {
                    let reflected_center = reflect_across_line(&center, &line_start, &line_end)?;
                    (radius > 1e-9).then_some(CircularConstraintRaw::Circle {
                        center: reflected_center,
                        radius,
                    })
                }
                CircularConstraintRaw::ThreePointArc { .. } => None,
            }
        }
        crate::format::GroupKind::CenterArc => {
            if path.refs.len() != 3 {
                return None;
            }
            let center = anchors.get(path.refs[0].checked_sub(1)?)?.clone()?;
            let start = anchors.get(path.refs[1].checked_sub(1)?)?.clone()?;
            let end = anchors.get(path.refs[2].checked_sub(1)?)?.clone()?;
            let [start, mid, end] =
                crate::runtime::geometry::arc_on_circle_control_points(&center, &start, &end)?;
            let radius = ((start.x - center.x).powi(2) + (start.y - center.y).powi(2)).sqrt();
            let start_angle = (start.y - center.y).atan2(start.x - center.x);
            let end_angle = (end.y - center.y).atan2(end.x - center.x);
            let ccw_span = normalize_angle_delta_raw(start_angle, end_angle);
            let ccw_mid =
                normalize_angle_delta_raw(start_angle, (mid.y - center.y).atan2(mid.x - center.x));
            Some(CircularConstraintRaw::ThreePointArc {
                start,
                mid,
                end,
                center,
                radius,
                ccw_span,
                ccw_mid,
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
            let center = anchors.get(circle_path.refs[0].checked_sub(1)?)?.clone()?;
            let start = anchors.get(path.refs[1].checked_sub(1)?)?.clone()?;
            let end = anchors.get(path.refs[2].checked_sub(1)?)?.clone()?;
            let [start, mid, end] =
                crate::runtime::geometry::arc_on_circle_control_points(&center, &start, &end)?;
            let radius = ((start.x - center.x).powi(2) + (start.y - center.y).powi(2)).sqrt();
            let start_angle = (start.y - center.y).atan2(start.x - center.x);
            let end_angle = (end.y - center.y).atan2(end.x - center.x);
            let ccw_span = normalize_angle_delta_raw(start_angle, end_angle);
            let ccw_mid =
                normalize_angle_delta_raw(start_angle, (mid.y - center.y).atan2(mid.x - center.x));
            Some(CircularConstraintRaw::ThreePointArc {
                start,
                mid,
                end,
                center,
                radius,
                ccw_span,
                ccw_mid,
            })
        }
        crate::format::GroupKind::ThreePointArc => {
            if path.refs.len() != 3 {
                return None;
            }
            let start = anchors.get(path.refs[0].checked_sub(1)?)?.clone()?;
            let mid = anchors.get(path.refs[1].checked_sub(1)?)?.clone()?;
            let end = anchors.get(path.refs[2].checked_sub(1)?)?.clone()?;
            let geometry = three_point_arc_geometry(&start, &mid, &end)?;
            let ccw_span = normalize_angle_delta_raw(geometry.start_angle, geometry.end_angle);
            let ccw_mid = normalize_angle_delta_raw(
                geometry.start_angle,
                (mid.y - geometry.center.y).atan2(mid.x - geometry.center.x),
            );
            Some(CircularConstraintRaw::ThreePointArc {
                start,
                mid,
                end,
                center: geometry.center,
                radius: geometry.radius,
                ccw_span,
                ccw_mid,
            })
        }
        _ => None,
    }
}

pub(crate) fn resolve_line_like_points_raw(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
    group: &ObjectGroup,
) -> Option<(PointRecord, PointRecord)> {
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
            let start = anchors.get(path.refs[0].checked_sub(1)?)?.clone()?;
            let end = anchors.get(path.refs[1].checked_sub(1)?)?.clone()?;
            distinct_pair(start, end)
        }
        crate::format::GroupKind::LineKind5 => {
            if path.refs.len() != 2 {
                return None;
            }
            let through = anchors.get(path.refs[0].checked_sub(1)?)?.clone()?;
            let host_group = groups.get(path.refs[1].checked_sub(1)?)?;
            let (host_start, host_end) =
                resolve_line_like_points_raw(file, groups, anchors, host_group)?;
            let dx = host_end.x - host_start.x;
            let dy = host_end.y - host_start.y;
            let len = (dx * dx + dy * dy).sqrt();
            if len <= 1e-9 {
                return None;
            }
            distinct_pair(
                through.clone(),
                PointRecord {
                    x: through.x - dy / len,
                    y: through.y + dx / len,
                },
            )
        }
        crate::format::GroupKind::LineKind6 => {
            if path.refs.len() != 2 {
                return None;
            }
            let through = anchors.get(path.refs[0].checked_sub(1)?)?.clone()?;
            let host_group = groups.get(path.refs[1].checked_sub(1)?)?;
            let (host_start, host_end) =
                resolve_line_like_points_raw(file, groups, anchors, host_group)?;
            let dx = host_end.x - host_start.x;
            let dy = host_end.y - host_start.y;
            let len = (dx * dx + dy * dy).sqrt();
            if len <= 1e-9 {
                return None;
            }
            distinct_pair(
                through.clone(),
                PointRecord {
                    x: through.x + dx / len,
                    y: through.y + dy / len,
                },
            )
        }
        crate::format::GroupKind::LineKind7 => {
            if path.refs.len() != 3 {
                return None;
            }
            let start = anchors.get(path.refs[0].checked_sub(1)?)?.clone()?;
            let vertex = anchors.get(path.refs[1].checked_sub(1)?)?.clone()?;
            let end = anchors.get(path.refs[2].checked_sub(1)?)?.clone()?;
            let first_dx = start.x - vertex.x;
            let first_dy = start.y - vertex.y;
            let first_len = (first_dx * first_dx + first_dy * first_dy).sqrt();
            let second_dx = end.x - vertex.x;
            let second_dy = end.y - vertex.y;
            let second_len = (second_dx * second_dx + second_dy * second_dy).sqrt();
            if first_len <= 1e-9 || second_len <= 1e-9 {
                return None;
            }
            let sum_x = first_dx / first_len + second_dx / second_len;
            let sum_y = first_dy / first_len + second_dy / second_len;
            let sum_len = (sum_x * sum_x + sum_y * sum_y).sqrt();
            let (dir_x, dir_y) = if sum_len > 1e-9 {
                (sum_x / sum_len, sum_y / sum_len)
            } else {
                (-first_dy / first_len, first_dx / first_len)
            };
            distinct_pair(
                vertex.clone(),
                PointRecord {
                    x: vertex.x + dir_x,
                    y: vertex.y + dir_y,
                },
            )
        }
        crate::format::GroupKind::Translation => {
            if path.refs.len() < 3 {
                return None;
            }
            let source_group = groups.get(path.refs[0].checked_sub(1)?)?;
            let (start, end) = resolve_line_like_points_raw(file, groups, anchors, source_group)?;
            let vector_start = anchors.get(path.refs[1].checked_sub(1)?)?.clone()?;
            let vector_end = anchors.get(path.refs[2].checked_sub(1)?)?.clone()?;
            let dx = vector_end.x - vector_start.x;
            let dy = vector_end.y - vector_start.y;
            distinct_pair(
                PointRecord {
                    x: start.x + dx,
                    y: start.y + dy,
                },
                PointRecord {
                    x: end.x + dx,
                    y: end.y + dy,
                },
            )
        }
        _ => None,
    }
}

fn distinct_pair(start: PointRecord, end: PointRecord) -> Option<(PointRecord, PointRecord)> {
    (((end.x - start.x).powi(2) + (end.y - start.y).powi(2)).sqrt() > 1e-9).then_some((start, end))
}

fn select_line_circle_intersection(
    line_start: PointRecord,
    line_end: PointRecord,
    center: PointRecord,
    radius: f64,
    variant: usize,
) -> Option<PointRecord> {
    let dx = line_end.x - line_start.x;
    let dy = line_end.y - line_start.y;
    let a = dx * dx + dy * dy;
    if a <= 1e-9 {
        return None;
    }
    let fx = line_start.x - center.x;
    let fy = line_start.y - center.y;
    let b = 2.0 * (fx * dx + fy * dy);
    let c = fx * fx + fy * fy - radius * radius;
    let discriminant = b * b - 4.0 * a * c;
    if discriminant < -1e-9 {
        return None;
    }
    let root = discriminant.max(0.0).sqrt();
    let mut ts = [(-b - root) / (2.0 * a), (-b + root) / (2.0 * a)];
    ts.sort_by(|left, right| left.total_cmp(right));
    let t = ts[variant.min(1)];
    Some(PointRecord {
        x: line_start.x + dx * t,
        y: line_start.y + dy * t,
    })
}

fn select_circular_intersection(
    left: &CircularConstraintRaw,
    right: &CircularConstraintRaw,
    variant: usize,
) -> Option<PointRecord> {
    let intersections = circle_circle_intersections(
        &left.center(),
        left.radius(),
        &right.center(),
        right.radius(),
    )?;
    let on_both = intersections
        .iter()
        .filter(|point| point_lies_on_circular_constraint(point, left))
        .filter(|point| point_lies_on_circular_constraint(point, right))
        .cloned()
        .collect::<Vec<_>>();
    on_both
        .get(variant.min(on_both.len().saturating_sub(1)))
        .cloned()
}

fn select_point_circle_tangent(
    point: &PointRecord,
    circle: &CircularConstraintRaw,
    variant: usize,
) -> Option<PointRecord> {
    let center = circle.center();
    let radius = circle.radius();
    let dx = point.x - center.x;
    let dy = point.y - center.y;
    let distance_sq = dx * dx + dy * dy;
    if distance_sq <= radius * radius + 1e-9 {
        return None;
    }
    let distance = distance_sq.sqrt();
    let base_angle = dy.atan2(dx);
    let offset = (radius / distance).acos();
    let mut tangents = vec![
        PointRecord {
            x: center.x + radius * (base_angle - offset).cos(),
            y: center.y + radius * (base_angle - offset).sin(),
        },
        PointRecord {
            x: center.x + radius * (base_angle + offset).cos(),
            y: center.y + radius * (base_angle + offset).sin(),
        },
    ];
    tangents.sort_by(|left, right| {
        left.y
            .total_cmp(&right.y)
            .then_with(|| left.x.total_cmp(&right.x))
    });
    tangents
        .into_iter()
        .filter(|candidate| point_lies_on_circular_constraint(candidate, circle))
        .nth(variant.min(1))
}

fn circle_circle_intersections(
    left_center: &PointRecord,
    left_radius: f64,
    right_center: &PointRecord,
    right_radius: f64,
) -> Option<Vec<PointRecord>> {
    let dx = right_center.x - left_center.x;
    let dy = right_center.y - left_center.y;
    let distance = (dx * dx + dy * dy).sqrt();
    if distance <= 1e-9
        || distance > left_radius + right_radius + 1e-9
        || distance < (left_radius - right_radius).abs() - 1e-9
    {
        return None;
    }

    let along = (left_radius * left_radius - right_radius * right_radius + distance * distance)
        / (2.0 * distance);
    let height_sq = left_radius * left_radius - along * along;
    if height_sq < -1e-9 {
        return None;
    }
    let height = height_sq.max(0.0).sqrt();
    let ux = dx / distance;
    let uy = dy / distance;
    let base = PointRecord {
        x: left_center.x + along * ux,
        y: left_center.y + along * uy,
    };
    let mut intersections = vec![
        PointRecord {
            x: base.x - height * uy,
            y: base.y + height * ux,
        },
        PointRecord {
            x: base.x + height * uy,
            y: base.y - height * ux,
        },
    ];
    intersections.sort_by(|left, right| {
        left.y
            .total_cmp(&right.y)
            .then_with(|| left.x.total_cmp(&right.x))
    });
    Some(intersections)
}

fn point_lies_on_circular_constraint(
    point: &PointRecord,
    constraint: &CircularConstraintRaw,
) -> bool {
    match constraint {
        CircularConstraintRaw::Circle { .. } => true,
        CircularConstraintRaw::ThreePointArc {
            start,
            mid,
            end,
            center,
            radius,
            ccw_span,
            ccw_mid,
        } => {
            let radial = ((point.x - center.x).powi(2) + (point.y - center.y).powi(2)).sqrt();
            if (radial - radius).abs() > 1e-6 {
                return false;
            }
            let angle = (point.y - center.y).atan2(point.x - center.x);
            let on_arc = if *ccw_mid <= *ccw_span + 1e-9 {
                normalize_angle_delta_raw((start.y - center.y).atan2(start.x - center.x), angle)
                    <= *ccw_span + 1e-9
            } else {
                normalize_angle_delta_raw(angle, (start.y - center.y).atan2(start.x - center.x))
                    <= normalize_angle_delta_raw(
                        (end.y - center.y).atan2(end.x - center.x),
                        (start.y - center.y).atan2(start.x - center.x),
                    ) + 1e-9
            };
            on_arc
                || ((point.x - start.x).abs() < 1e-6 && (point.y - start.y).abs() < 1e-6)
                || ((point.x - mid.x).abs() < 1e-6 && (point.y - mid.y).abs() < 1e-6)
                || ((point.x - end.x).abs() < 1e-6 && (point.y - end.y).abs() < 1e-6)
        }
    }
}

fn normalize_angle_delta_raw(from: f64, to: f64) -> f64 {
    let tau = std::f64::consts::TAU;
    (to - from).rem_euclid(tau)
}

fn line_line_intersection(
    left_start: &PointRecord,
    left_end: &PointRecord,
    right_start: &PointRecord,
    right_end: &PointRecord,
) -> Option<PointRecord> {
    let left_dx = left_end.x - left_start.x;
    let left_dy = left_end.y - left_start.y;
    let right_dx = right_end.x - right_start.x;
    let right_dy = right_end.y - right_start.y;
    let determinant = left_dx * right_dy - left_dy * right_dx;
    if determinant.abs() <= 1e-9 {
        return None;
    }
    let delta_x = right_start.x - left_start.x;
    let delta_y = right_start.y - left_start.y;
    let t = (delta_x * right_dy - delta_y * right_dx) / determinant;
    Some(PointRecord {
        x: left_start.x + t * left_dx,
        y: left_start.y + t * left_dy,
    })
}

pub(crate) fn decode_regular_polygon_vertex_anchor_raw(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
    anchors: &[Option<PointRecord>],
) -> Option<PointRecord> {
    if (group.header.kind()) != crate::format::GroupKind::ParameterRotation {
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
    Some(rotate_around(&source, &center, (-360.0 / n).to_radians()))
}

pub(crate) fn decode_custom_transform_anchor_raw(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
    anchors: &[Option<PointRecord>],
) -> Option<PointRecord> {
    if (group.header.kind()) != crate::format::GroupKind::CustomTransformPoint {
        return None;
    }
    let binding = decode_custom_transform_binding(file, groups, group.ordinal)?;
    let t = decode_custom_transform_parameter(file, groups, binding.source_group_index, anchors)?;
    resolve_custom_transform_point(anchors, &binding, t)
}

pub(crate) fn decode_custom_transform_binding(
    file: &GspFile,
    groups: &[ObjectGroup],
    target_ordinal: usize,
) -> Option<CustomTransformBindingDef> {
    let transform_group = groups.iter().find(|candidate| {
        (candidate.header.kind()) == crate::format::GroupKind::CustomTransformTrace
            && find_indexed_path(file, candidate).is_some_and(|path| {
                path.refs.first().copied() == Some(target_ordinal)
                    || path.refs.last().copied() == Some(target_ordinal)
            })
    })?;
    let path = find_indexed_path(file, transform_group)?;
    if path.refs.len() < 6 {
        return None;
    }
    let source_group_index = path.refs.get(2)?.checked_sub(1)?;
    let (origin_group_index, axis_end_group_index) =
        custom_transform_basis_indices(file, groups, source_group_index).or_else(|| {
            let axis_group = groups.get(path.refs.get(1)?.checked_sub(1)?)?;
            let axis_path = find_indexed_path(file, axis_group)?;
            Some((
                axis_path.refs.first()?.checked_sub(1)?,
                axis_path.refs.get(1)?.checked_sub(1)?,
            ))
        })?;
    let distance_expr_group = groups.get(path.refs.get(4)?.checked_sub(1)?)?;
    let angle_expr_group = groups.get(path.refs.get(5)?.checked_sub(1)?)?;
    let distance_expr = try_decode_function_expr(file, groups, distance_expr_group).ok()?;
    let angle_expr = try_decode_function_expr(file, groups, angle_expr_group).ok()?;
    Some(CustomTransformBindingDef {
        source_group_index,
        origin_group_index,
        axis_end_group_index,
        distance_expr,
        angle_expr,
        distance_raw_scale: decode_custom_transform_distance_scale(file, distance_expr_group)?,
        angle_degrees_scale: decode_custom_transform_angle_scale(file, angle_expr_group)?,
    })
}

fn custom_transform_basis_indices(
    file: &GspFile,
    groups: &[ObjectGroup],
    source_group_index: usize,
) -> Option<(usize, usize)> {
    let source_group = groups.get(source_group_index)?;
    match source_group.header.kind() {
        kind if kind.is_point_constraint() => {
            let host_group = groups.get(
                find_indexed_path(file, source_group)?
                    .refs
                    .first()?
                    .checked_sub(1)?,
            )?;
            if (host_group.header.kind()) != crate::format::GroupKind::Segment {
                return None;
            }
            let host_path = find_indexed_path(file, host_group)?;
            Some((
                host_path.refs.first()?.checked_sub(1)?,
                host_path.refs.get(1)?.checked_sub(1)?,
            ))
        }
        crate::format::GroupKind::ParameterControlledPoint => {
            let host_group = groups.get(
                find_indexed_path(file, source_group)?
                    .refs
                    .get(1)?
                    .checked_sub(1)?,
            )?;
            if (host_group.header.kind()) != crate::format::GroupKind::Segment {
                return None;
            }
            let host_path = find_indexed_path(file, host_group)?;
            Some((
                host_path.refs.first()?.checked_sub(1)?,
                host_path.refs.get(1)?.checked_sub(1)?,
            ))
        }
        _ => None,
    }
}

pub(crate) fn resolve_custom_transform_point(
    anchors: &[Option<PointRecord>],
    binding: &CustomTransformBindingDef,
    t: f64,
) -> Option<PointRecord> {
    let origin = anchors.get(binding.origin_group_index)?.clone()?;
    let axis_end = anchors.get(binding.axis_end_group_index)?.clone()?;
    let parameters = expression_parameter_map(&binding.distance_expr, &binding.angle_expr, t);
    let distance = evaluate_expr_with_parameters(&binding.distance_expr, t, &parameters)?
        * binding.distance_raw_scale;
    let angle_degrees = evaluate_expr_with_parameters(&binding.angle_expr, t, &parameters)?
        * binding.angle_degrees_scale;
    let base_angle = (-(axis_end.y - origin.y))
        .atan2(axis_end.x - origin.x)
        .to_degrees();
    let total_radians = (base_angle + angle_degrees).to_radians();
    Some(PointRecord {
        x: origin.x + distance * total_radians.cos(),
        y: origin.y - distance * total_radians.sin(),
    })
}

pub(crate) fn decode_custom_transform_parameter(
    file: &GspFile,
    groups: &[ObjectGroup],
    source_group_index: usize,
    anchors: &[Option<PointRecord>],
) -> Option<f64> {
    let source_group = groups.get(source_group_index)?;
    match source_group.header.kind() {
        kind if kind.is_point_constraint() => {
            match try_decode_point_constraint(file, groups, source_group, Some(anchors), &None)
                .ok()?
            {
                RawPointConstraint::Segment(constraint) => Some(constraint.t),
                RawPointConstraint::Polyline { t, .. } => Some(t),
                RawPointConstraint::PolygonBoundary { t, .. } => Some(t),
                RawPointConstraint::Circle(constraint) => {
                    let angle = (-constraint.unit_y).atan2(constraint.unit_x);
                    let tau = std::f64::consts::TAU;
                    Some(((angle % tau) + tau) % tau / tau)
                }
                RawPointConstraint::Circular(constraint) => {
                    let angle = (-constraint.unit_y).atan2(constraint.unit_x);
                    let tau = std::f64::consts::TAU;
                    let _ = constraint.circle_group_index;
                    Some(((angle % tau) + tau) % tau / tau)
                }
                RawPointConstraint::CircleArc(constraint) => Some(constraint.t),
                RawPointConstraint::Arc(constraint) => Some(constraint.t),
            }
        }
        crate::format::GroupKind::ParameterControlledPoint => {
            let parameter_point =
                try_decode_parameter_controlled_point(file, groups, source_group, anchors).ok()?;
            match parameter_point.constraint {
                RawPointConstraint::Segment(constraint) => Some(constraint.t),
                RawPointConstraint::Polyline { t, .. } => Some(t),
                RawPointConstraint::PolygonBoundary { t, .. } => Some(t),
                RawPointConstraint::Circle(constraint) => {
                    let angle = (-constraint.unit_y).atan2(constraint.unit_x);
                    let tau = std::f64::consts::TAU;
                    Some(((angle % tau) + tau) % tau / tau)
                }
                RawPointConstraint::Circular(constraint) => {
                    let angle = (-constraint.unit_y).atan2(constraint.unit_x);
                    let tau = std::f64::consts::TAU;
                    let _ = constraint.circle_group_index;
                    Some(((angle % tau) + tau) % tau / tau)
                }
                RawPointConstraint::CircleArc(constraint) => Some(constraint.t),
                RawPointConstraint::Arc(constraint) => Some(constraint.t),
            }
        }
        _ => None,
    }
}

fn decode_custom_transform_distance_scale(file: &GspFile, expr_group: &ObjectGroup) -> Option<f64> {
    Some(match custom_transform_suffix(file, expr_group)? {
        0x0201 => PX_PER_CM,
        _ => 1.0,
    })
}

fn decode_custom_transform_angle_scale(file: &GspFile, expr_group: &ObjectGroup) -> Option<f64> {
    Some(match custom_transform_suffix(file, expr_group)? {
        0x0101 => 100.0,
        _ => 1.0,
    })
}

fn custom_transform_suffix(file: &GspFile, expr_group: &ObjectGroup) -> Option<u16> {
    let payload = expr_group
        .records
        .iter()
        .find(|record| record.record_type == 0x0907)?
        .payload(&file.data);
    let words = payload
        .chunks_exact(2)
        .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
        .collect::<Vec<_>>();
    words.last().copied().or_else(|| {
        (words.len() >= 3 && words[words.len() - 3..] == [0x0000, 0x0000, 0x0101]).then_some(0x0101)
    })
}

fn expression_parameter_map(
    left: &crate::runtime::functions::FunctionExpr,
    right: &crate::runtime::functions::FunctionExpr,
    t: f64,
) -> BTreeMap<String, f64> {
    let mut parameters = BTreeMap::new();
    collect_expr_parameter_names(left, &mut parameters, t);
    collect_expr_parameter_names(right, &mut parameters, t);
    parameters
}

pub(crate) fn custom_transform_expression_parameter_map(
    left: &crate::runtime::functions::FunctionExpr,
    right: &crate::runtime::functions::FunctionExpr,
    t: f64,
) -> BTreeMap<String, f64> {
    expression_parameter_map(left, right, t)
}

pub(crate) fn custom_transform_trace_parameter(
    point: &crate::runtime::scene::ScenePoint,
) -> Option<f64> {
    match &point.constraint {
        crate::runtime::scene::ScenePointConstraint::OnSegment { t, .. }
        | crate::runtime::scene::ScenePointConstraint::OnLine { t, .. }
        | crate::runtime::scene::ScenePointConstraint::OnRay { t, .. }
        | crate::runtime::scene::ScenePointConstraint::OnCircleArc { t, .. }
        | crate::runtime::scene::ScenePointConstraint::OnArc { t, .. } => Some(*t),
        crate::runtime::scene::ScenePointConstraint::OnPolygonBoundary { t, .. } => Some(*t),
        crate::runtime::scene::ScenePointConstraint::OnCircle { unit_x, unit_y, .. } => {
            let angle = (-*unit_y).atan2(*unit_x);
            let tau = std::f64::consts::TAU;
            Some(((angle % tau) + tau) % tau / tau)
        }
        _ => None,
    }
}

fn collect_expr_parameter_names(
    expr: &crate::runtime::functions::FunctionExpr,
    parameters: &mut BTreeMap<String, f64>,
    value: f64,
) {
    if let crate::runtime::functions::FunctionExpr::Parsed(ast) = expr {
        collect_term_parameter_names(ast, parameters, value);
    }
}

fn collect_term_parameter_names(
    term: &crate::runtime::functions::FunctionAst,
    parameters: &mut BTreeMap<String, f64>,
    value: f64,
) {
    match term {
        crate::runtime::functions::FunctionAst::Parameter(name, _) => {
            parameters.insert(name.clone(), value);
        }
        crate::runtime::functions::FunctionAst::Unary { expr, .. } => {
            collect_term_parameter_names(expr, parameters, value);
        }
        crate::runtime::functions::FunctionAst::Binary {
            lhs: left,
            rhs: right,
            ..
        } => {
            collect_term_parameter_names(left, parameters, value);
            collect_term_parameter_names(right, parameters, value);
        }
        _ => {}
    }
}

pub(crate) fn decode_parameter_rotation_anchor_raw(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
    anchors: &[Option<PointRecord>],
) -> Option<PointRecord> {
    let binding = try_decode_parameter_rotation_binding(file, groups, group).ok()?;
    let source = anchors.get(binding.source_group_index)?.clone()?;
    let center = anchors.get(binding.center_group_index)?.clone()?;
    let TransformBindingKind::Rotate { angle_degrees, .. } = binding.kind else {
        return None;
    };
    Some(rotate_around(&source, &center, angle_degrees.to_radians()))
}

pub(crate) fn decode_ratio_scale_anchor_raw(
    file: &GspFile,
    _groups: &[ObjectGroup],
    group: &ObjectGroup,
    anchors: &[Option<PointRecord>],
) -> Option<PointRecord> {
    if (group.header.kind()) != crate::format::GroupKind::RatioScale {
        return None;
    }
    let path = find_indexed_path(file, group)?;
    if path.refs.len() < 5 {
        return None;
    }
    let source = anchors.get(path.refs[0].checked_sub(1)?)?.clone()?;
    let center = anchors.get(path.refs[1].checked_sub(1)?)?.clone()?;
    let ratio_origin = anchors.get(path.refs[2].checked_sub(1)?)?.clone()?;
    let ratio_denominator = anchors.get(path.refs[3].checked_sub(1)?)?.clone()?;
    let ratio_numerator = anchors.get(path.refs[4].checked_sub(1)?)?.clone()?;
    let denominator =
        (ratio_denominator.x - ratio_origin.x).hypot(ratio_denominator.y - ratio_origin.y);
    if denominator <= 1e-9 {
        return None;
    }
    let numerator = (ratio_numerator.x - ratio_origin.x).hypot(ratio_numerator.y - ratio_origin.y);
    let factor = numerator / denominator;
    Some(crate::runtime::geometry::scale_around(
        &source, &center, factor,
    ))
}

pub(crate) fn decode_reflection_anchor_raw(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
    anchors: &[Option<PointRecord>],
) -> Option<PointRecord> {
    if (group.header.kind()) != crate::format::GroupKind::Reflection {
        return None;
    }
    let path = find_indexed_path(file, group)?;
    let source_group_index = path.refs.first()?.checked_sub(1)?;
    let source = anchors.get(source_group_index)?.clone()?;
    let line_group = groups.get(path.refs.get(1)?.checked_sub(1)?)?;
    let (line_start, line_end) = resolve_line_like_points_raw(file, groups, anchors, line_group)?;
    reflect_point_across_line(&source, &line_start, &line_end)
}

pub(crate) fn decode_point_pair_translation_anchor_raw(
    file: &GspFile,
    _groups: &[ObjectGroup],
    group: &ObjectGroup,
    anchors: &[Option<PointRecord>],
) -> Option<PointRecord> {
    if (group.header.kind()) != crate::format::GroupKind::Translation {
        return None;
    }
    let path = find_indexed_path(file, group)?;
    let source_group_index = path.refs.first()?.checked_sub(1)?;
    let (vector_start_group_index, vector_end_group_index) =
        translation_point_pair_group_indices(file, group)?;
    let source = anchors.get(source_group_index)?.clone()?;
    let vector_start = anchors.get(vector_start_group_index)?.clone()?;
    let vector_end = anchors.get(vector_end_group_index)?.clone()?;
    Some(PointRecord {
        x: source.x + (vector_end.x - vector_start.x),
        y: source.y + (vector_end.y - vector_start.y),
    })
}

pub(crate) fn decode_parameter_controlled_anchor_raw(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
    anchors: &[Option<PointRecord>],
) -> Option<PointRecord> {
    try_decode_parameter_controlled_point(file, groups, group, anchors)
        .ok()
        .map(|point| point.position)
}

pub(crate) fn reflection_line_group_indices(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
) -> Option<(usize, usize)> {
    let path = find_indexed_path(file, group)?;
    let line_group = groups.get(path.refs.get(1)?.checked_sub(1)?)?;
    if !line_group.header.kind().is_line_like() {
        return None;
    }
    let line_path = find_indexed_path(file, line_group)?;
    Some((
        line_path.refs.first()?.checked_sub(1)?,
        line_path.refs.get(1)?.checked_sub(1)?,
    ))
}

pub(crate) fn translation_point_pair_group_indices(
    file: &GspFile,
    group: &ObjectGroup,
) -> Option<(usize, usize)> {
    if (group.header.kind()) != crate::format::GroupKind::Translation {
        return None;
    }
    let path = find_indexed_path(file, group)?;
    Some((
        path.refs.get(1)?.checked_sub(1)?,
        path.refs.get(2)?.checked_sub(1)?,
    ))
}

pub(crate) fn reflect_point_across_line(
    point: &PointRecord,
    line_start: &PointRecord,
    line_end: &PointRecord,
) -> Option<PointRecord> {
    reflect_across_line(point, line_start, line_end)
}

pub(crate) fn decode_point_on_ray_anchor_raw(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
    anchors: &[Option<PointRecord>],
) -> Option<PointRecord> {
    if !group.header.kind().is_point_constraint() {
        return None;
    }

    let host_ref = find_indexed_path(file, group)?
        .refs
        .first()
        .copied()
        .filter(|ordinal| *ordinal > 0)?;
    let host_group = groups.get(host_ref - 1)?;
    if (host_group.header.kind()) != crate::format::GroupKind::Ray {
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

pub(crate) fn decode_translated_point_anchor_raw(
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

pub(crate) fn decode_line_midpoint_anchor_raw(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
    anchors: &[Option<PointRecord>],
) -> Option<PointRecord> {
    if (group.header.kind()) != crate::format::GroupKind::Midpoint {
        return None;
    }

    let path = find_indexed_path(file, group)?;
    let host_group = groups.get(path.refs.first()?.checked_sub(1)?)?;
    if !host_group.header.kind().is_line_like() {
        return None;
    }

    let host_path = find_indexed_path(file, host_group)?;
    let start = anchors
        .get(host_path.refs.first()?.checked_sub(1)?)?
        .clone()?;
    let end = anchors
        .get(host_path.refs.get(1)?.checked_sub(1)?)?
        .clone()?;
    Some(PointRecord {
        x: (start.x + end.x) * 0.5,
        y: (start.y + end.y) * 0.5,
    })
}

pub(crate) fn decode_offset_anchor_raw(
    file: &GspFile,
    group: &ObjectGroup,
    anchors: &[Option<PointRecord>],
) -> Option<PointRecord> {
    if (group.header.kind()) != crate::format::GroupKind::OffsetAnchor {
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

pub(crate) fn decode_point_constraint_anchor(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
    anchors: &[Option<PointRecord>],
    graph: Option<&GraphTransform>,
) -> Option<PointRecord> {
    let graph = graph.cloned();
    match try_decode_point_constraint(file, groups, group, Some(anchors), &graph).ok()? {
        RawPointConstraint::Segment(constraint) => {
            let start = anchors.get(constraint.start_group_index)?.clone()?;
            let end = anchors.get(constraint.end_group_index)?.clone()?;

            Some(lerp_point(&start, &end, constraint.t))
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
        RawPointConstraint::Circular(constraint) => {
            let circle_group = groups.get(constraint.circle_group_index)?;
            let circle = resolve_circle_like_raw(file, groups, anchors, circle_group)?;
            match circle {
                CircularConstraintRaw::Circle { center, radius } => Some(PointRecord {
                    x: center.x + radius * constraint.unit_x,
                    y: center.y - radius * constraint.unit_y,
                }),
                CircularConstraintRaw::ThreePointArc { .. } => None,
            }
        }
        RawPointConstraint::CircleArc(constraint) => {
            let center = anchors.get(constraint.center_group_index)?.clone()?;
            let start = anchors.get(constraint.start_group_index)?.clone()?;
            let end = anchors.get(constraint.end_group_index)?.clone()?;
            point_on_circle_arc(&center, &start, &end, constraint.t)
        }
        RawPointConstraint::Arc(constraint) => {
            let start = anchors.get(constraint.start_group_index)?.clone()?;
            let mid = anchors.get(constraint.mid_group_index)?.clone()?;
            let end = anchors.get(constraint.end_group_index)?.clone()?;
            point_on_three_point_arc(&start, &mid, &end, constraint.t)
        }
    }
}

pub(crate) fn resolve_circle_point_raw(
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

pub(crate) fn resolve_polygon_boundary_point_raw(
    vertices: &[PointRecord],
    edge_index: usize,
    t: f64,
) -> Option<PointRecord> {
    if vertices.len() < 2 {
        return None;
    }

    let start = &vertices[edge_index % vertices.len()];
    let end = &vertices[(edge_index + 1) % vertices.len()];
    Some(lerp_point(start, end, t))
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
    Some(lerp_point(start, end, t))
}

#[cfg(test)]
mod tests {
    use super::{CircularConstraintRaw, normalize_angle_delta_raw, select_circular_intersection};
    use crate::format::PointRecord;
    use crate::runtime::geometry::three_point_arc_geometry;

    fn arc(start: PointRecord, mid: PointRecord, end: PointRecord) -> CircularConstraintRaw {
        let geometry = three_point_arc_geometry(&start, &mid, &end).expect("valid arc");
        CircularConstraintRaw::ThreePointArc {
            start,
            mid: mid.clone(),
            end,
            center: geometry.center.clone(),
            radius: geometry.radius,
            ccw_span: normalize_angle_delta_raw(geometry.start_angle, geometry.end_angle),
            ccw_mid: normalize_angle_delta_raw(
                geometry.start_angle,
                (mid.y - geometry.center.y).atan2(mid.x - geometry.center.x),
            ),
        }
    }

    #[test]
    fn arc_intersection_returns_none_when_only_parent_circles_intersect() {
        let left = arc(
            PointRecord { x: -1.0, y: 0.0 },
            PointRecord { x: 0.0, y: 1.0 },
            PointRecord { x: 1.0, y: 0.0 },
        );
        let right = arc(
            PointRecord { x: 2.0, y: 0.0 },
            PointRecord { x: 1.0, y: -1.0 },
            PointRecord { x: 0.0, y: 0.0 },
        );

        assert!(
            select_circular_intersection(&left, &right, 0).is_none(),
            "expected no intersection when arc spans do not overlap"
        );
    }
}
