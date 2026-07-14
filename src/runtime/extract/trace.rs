use std::collections::{BTreeMap, BTreeSet};

use crate::format::{GroupKind, GspFile, ObjectGroup, PointRecord, read_f64, read_u32};
use crate::runtime::extract::bindings::normalized_hsb;
use crate::runtime::extract::decode::decode_label_name;
use crate::runtime::functions::{evaluate_expr_with_parameters, try_decode_function_expr};
use crate::runtime::geometry::{
    GraphTransform, from_core_point, lerp_point, line_stroke_width_from_style, point_on_circle_arc,
    point_on_three_point_arc, reflect_across_line, rotate_around, scale_around, to_core_point,
    to_raw_from_world,
};
use crate::runtime::payload_consts::{RECORD_BINDING_PAYLOAD, RECORD_ITERATION_DEFINITION};
use crate::runtime::scene::{
    ArcConstraint, CircularConstraint, LineConstraint, LineLikeKind, LineShape, ScenePoint,
    ScenePointBinding, ScenePointConstraint,
};

use super::points::{
    custom_transform_expression_parameter_map, custom_transform_trace_parameter,
    editable_non_graph_parameter_name_for_group, scene_point_from_parameter_controlled,
    try_decode_parameter_controlled_point_on_polyline,
};
use super::{find_indexed_path, payload_debug_source};

const TRACE_DESCRIPTOR_PATH_FLAGS_OFFSET: usize = 20;
const TRACE_DESCRIPTOR_OPEN_PATH_FLAG: u32 = 1 << 16;

fn trace_driver_path_is_open(payload: &[u8]) -> bool {
    read_u32(payload, TRACE_DESCRIPTOR_PATH_FLAGS_OFFSET) & TRACE_DESCRIPTOR_OPEN_PATH_FLAG != 0
}

fn segment_projection_parameter(
    point: &PointRecord,
    start: &PointRecord,
    end: &PointRecord,
) -> Option<f64> {
    gsp_runtime_core::project_to_line_like(
        to_core_point(point),
        to_core_point(start),
        to_core_point(end),
        gsp_runtime_core::LineKind::Segment,
    )
    .map(|projection| projection.t)
}

pub(super) fn collect_point_traces(
    file: &GspFile,
    groups: &[ObjectGroup],
    visible_points: &[ScenePoint],
    group_to_point_index: &[Option<usize>],
    graph_ref: &Option<GraphTransform>,
) -> Vec<crate::runtime::scene::LineShape> {
    groups
        .iter()
        .filter(|group| {
            matches!(
                group.header.kind(),
                crate::format::GroupKind::PointTrace
                    | crate::format::GroupKind::CustomTransformTrace
            )
        })
        .filter_map(|group| {
            let group_kind = group.header.kind();
            let path = find_indexed_path(file, group)?;
            let target_group_index = path.refs.first()?.checked_sub(1)?;
            let target_group = groups.get(target_group_index)?;
            let target_point_index = (*group_to_point_index.get(target_group_index)?)?;
            let payload = group
                .records
                .iter()
                .find(|record| {
                    record.record_type
                        == crate::runtime::payload_consts::RECORD_FUNCTION_PLOT_DESCRIPTOR
                })
                .map(|record| record.payload(&file.data))?;
            let descriptor =
                crate::runtime::functions::try_decode_function_plot_descriptor(payload).ok()?;
            let driver = match group_kind {
                GroupKind::CustomTransformTrace => path.refs.get(2).and_then(|ordinal| {
                    trace_driver_point(visible_points, group_to_point_index, *ordinal)
                }),
                GroupKind::PointTrace => path
                    .refs
                    .iter()
                    .filter_map(|ordinal| {
                        trace_driver_point(visible_points, group_to_point_index, *ordinal)
                    })
                    .find(|(_, group_index)| *group_index != target_group_index)
                    .or_else(|| {
                        visible_points
                            .get(target_point_index)
                            .filter(|point| point_accepts_trace_parameter(point))
                            .map(|_| (target_point_index, target_group_index))
                    }),
                _ => return None,
            };
            let use_raw_parameter = matches!(group_kind, GroupKind::CustomTransformTrace);
            let trace_max = match group_kind {
                GroupKind::CustomTransformTrace => {
                    let (driver_point_index, _) = driver?;
                    custom_transform_trace_parameter(visible_points.get(driver_point_index)?)?
                        .clamp(
                            descriptor.x_min.min(descriptor.x_max),
                            descriptor.x_min.max(descriptor.x_max),
                        )
                }
                GroupKind::PointTrace => descriptor.x_max,
                _ => return None,
            };

            if matches!(group_kind, GroupKind::PointTrace)
                && matches!(target_group.header.kind(), GroupKind::CoordinatePoint)
                && let Some((_, driver_group_index)) = driver
                && let Some(points) = sample_coordinate_point_trace(
                    file,
                    groups,
                    group,
                    target_group,
                    CoordinateTraceSampleSpec {
                        x_min: descriptor.x_min,
                        x_max: trace_max,
                        sample_count: descriptor.sample_count,
                    },
                    graph_ref,
                )
            {
                return Some(crate::runtime::scene::LineShape {
                    points,
                    color: crate::runtime::geometry::color_from_style(group.header.style_b),
                    dashed: false,
                    stroke_width: Some(line_stroke_width_from_style(group.header.style_a)),
                    visible: !group.header.is_hidden(),
                    binding: Some(crate::runtime::scene::LineBinding::PointTrace {
                        point_index: target_group_index,
                        driver_index: driver_group_index,
                        x_min: descriptor.x_min,
                        x_max: descriptor.x_max,
                        sample_count: descriptor.sample_count,
                    }),
                    debug: Some(payload_debug_source(group)),
                });
            }

            let (driver_point_index, driver_group_index) = driver?;

            let mut points = Vec::with_capacity(descriptor.sample_count);
            let last = descriptor.sample_count.saturating_sub(1).max(1) as f64;
            for sample_index in 0..descriptor.sample_count {
                let t = sample_index as f64 / last;
                let parameter = descriptor.x_min + (trace_max - descriptor.x_min) * t;
                let sample = (|| {
                    let mut sampled_points = visible_points.to_vec();
                    let driver_point = sampled_points.get_mut(driver_point_index)?;
                    apply_trace_parameter_with_mode(
                        driver_point,
                        parameter,
                        descriptor.x_min,
                        trace_max,
                        use_raw_parameter,
                    );
                    let trace_parameters = trace_parameter_values(
                        file,
                        groups,
                        &path.refs,
                        &mut sampled_points,
                        group_to_point_index,
                    );
                    refresh_trace_expression_scale_factors(&mut sampled_points, &trace_parameters);
                    refresh_sampled_trace_polylines(
                        file,
                        groups,
                        &mut sampled_points,
                        group_to_point_index,
                        graph_ref,
                    );
                    resolve_trace_point(
                        &mut sampled_points,
                        target_point_index,
                        &mut BTreeSet::new(),
                    )
                })();
                if let Some(sample) = sample {
                    points.push(sample);
                }
            }

            let binding = match group_kind {
                GroupKind::CustomTransformTrace => {
                    Some(crate::runtime::scene::LineBinding::CustomTransformTrace {
                        point_index: target_group_index,
                        driver_index: driver_group_index,
                        x_min: descriptor.x_min,
                        x_max: descriptor.x_max,
                        sample_count: descriptor.sample_count,
                    })
                }
                GroupKind::PointTrace => Some(crate::runtime::scene::LineBinding::PointTrace {
                    point_index: target_group_index,
                    driver_index: driver_group_index,
                    x_min: descriptor.x_min,
                    x_max: descriptor.x_max,
                    sample_count: descriptor.sample_count,
                }),
                _ => return None,
            };

            Some(crate::runtime::scene::LineShape {
                points,
                color: crate::runtime::geometry::color_from_style(group.header.style_b),
                dashed: false,
                stroke_width: Some(line_stroke_width_from_style(group.header.style_a)),
                visible: !group.header.is_hidden(),
                binding,
                debug: Some(payload_debug_source(group)),
            })
        })
        .collect()
}

fn trace_driver_point(
    visible_points: &[ScenePoint],
    group_to_point_index: &[Option<usize>],
    ordinal: usize,
) -> Option<(usize, usize)> {
    let group_index = ordinal.checked_sub(1)?;
    let point_index = (*group_to_point_index.get(group_index)?)?;
    let point = visible_points.get(point_index)?;
    point_accepts_trace_parameter(point).then_some((point_index, group_index))
}

pub(super) fn collect_segment_traces(
    file: &GspFile,
    groups: &[ObjectGroup],
    visible_points: &[ScenePoint],
    group_to_point_index: &[Option<usize>],
    graph_ref: &Option<GraphTransform>,
) -> Vec<LineShape> {
    let traces = groups
        .iter()
        .filter(|group| group.header.kind() == GroupKind::PointTrace)
        .filter_map(|group| {
            let path = find_indexed_path(file, group)?;
            let target_group_index = path.refs.first()?.checked_sub(1)?;
            let target_group = groups.get(target_group_index)?;
            if target_group.header.kind() != GroupKind::Segment {
                return None;
            }
            let target_path = find_indexed_path(file, target_group)?;
            let start_group_index = target_path.refs.first()?.checked_sub(1)?;
            let end_group_index = target_path.refs.get(1)?.checked_sub(1)?;
            let start_point_index = (*group_to_point_index.get(start_group_index)?)?;
            let end_point_index = (*group_to_point_index.get(end_group_index)?)?;
            let payload = group
                .records
                .iter()
                .find(|record| {
                    record.record_type
                        == crate::runtime::payload_consts::RECORD_FUNCTION_PLOT_DESCRIPTOR
                })
                .map(|record| record.payload(&file.data))?;
            let driver_path_is_open = trace_driver_path_is_open(payload);
            let descriptor =
                crate::runtime::functions::try_decode_function_plot_descriptor(payload).ok()?;
            let driver = path
                .refs
                .iter()
                .filter_map(|ordinal| {
                    trace_driver_point(visible_points, group_to_point_index, *ordinal)
                })
                .find(|(_, group_index)| {
                    *group_index != target_group_index
                        && *group_index != start_group_index
                        && *group_index != end_group_index
                })
                .or_else(|| {
                    path.refs
                        .iter()
                        .filter_map(|ordinal| {
                            trace_driver_point(visible_points, group_to_point_index, *ordinal)
                        })
                        .find(|(_, group_index)| *group_index != target_group_index)
                })?;
            let (driver_point_index, driver_group_index) = driver;

            let mut sampled_points = Vec::with_capacity(descriptor.sample_count * 2);
            let last = descriptor.sample_count.saturating_sub(1).max(1) as f64;
            for sample_index in 0..descriptor.sample_count {
                let t = sample_index as f64 / last;
                let parameter = descriptor.x_min + (descriptor.x_max - descriptor.x_min) * t;
                let sample = (|| {
                    let mut sampled_points = visible_points.to_vec();
                    let driver_point = sampled_points.get_mut(driver_point_index)?;
                    apply_trace_parameter_with_mode(
                        driver_point,
                        parameter,
                        descriptor.x_min,
                        descriptor.x_max,
                        false,
                    );
                    let trace_parameters = trace_parameter_values(
                        file,
                        groups,
                        &path.refs,
                        &mut sampled_points,
                        group_to_point_index,
                    );
                    refresh_trace_expression_scale_factors(&mut sampled_points, &trace_parameters);
                    refresh_sampled_trace_polylines(
                        file,
                        groups,
                        &mut sampled_points,
                        group_to_point_index,
                        graph_ref,
                    );
                    let start = resolve_trace_point(
                        &mut sampled_points,
                        start_point_index,
                        &mut BTreeSet::new(),
                    )?;
                    let end = resolve_trace_point(
                        &mut sampled_points,
                        end_point_index,
                        &mut BTreeSet::new(),
                    )?;
                    Some((start, end))
                })();
                if let Some((start, end)) = sample
                    && (end.x - start.x).hypot(end.y - start.y) > 1e-9
                {
                    sampled_points.push(start);
                    sampled_points.push(end);
                }
            }
            Some((
                driver_path_is_open,
                LineShape {
                    points: sampled_points,
                    color: crate::runtime::geometry::color_from_style(group.header.style_b),
                    dashed: false,
                    stroke_width: Some(line_stroke_width_from_style(group.header.style_a)),
                    visible: !group.header.is_hidden(),
                    binding: Some(crate::runtime::scene::LineBinding::SegmentTrace {
                        start_index: start_group_index,
                        end_index: end_group_index,
                        driver_index: driver_group_index,
                        x_min: descriptor.x_min,
                        x_max: descriptor.x_max,
                        sample_count: descriptor.sample_count,
                    }),
                    debug: Some(payload_debug_source(group)),
                },
            ))
        })
        .collect::<Vec<_>>();

    // Sketchpad paints sweeps driven by open paths back-to-front. Closed-path
    // sweeps (such as a cylinder cap) are then painted in construction order.
    let (mut open_path_traces, closed_path_traces): (Vec<_>, Vec<_>) =
        traces.into_iter().partition(|(is_open, _)| *is_open);
    open_path_traces.reverse();
    open_path_traces
        .into_iter()
        .chain(closed_path_traces)
        .map(|(_, line)| line)
        .collect()
}

pub(super) fn bind_points_to_point_traces(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
    visible_points: &mut Vec<ScenePoint>,
    group_to_point_index: &mut [Option<usize>],
    point_trace_lines: &[LineShape],
) {
    for group in groups.iter().filter(|group| {
        matches!(
            group.header.kind(),
            GroupKind::PointConstraint | GroupKind::PathPoint | GroupKind::ParameterControlledPoint
        )
    }) {
        let Some(path) = find_indexed_path(file, group) else {
            continue;
        };
        let host_ref_index =
            usize::from(group.header.kind() == GroupKind::ParameterControlledPoint);
        let Some(host_ordinal) = path.refs.get(host_ref_index).copied() else {
            continue;
        };
        let Some(host_group) = groups.get(host_ordinal.saturating_sub(1)) else {
            continue;
        };
        if host_group.header.kind() != GroupKind::PointTrace {
            continue;
        }
        let Some(group_index) = group.ordinal.checked_sub(1) else {
            continue;
        };
        let Some(existing_point_index) = group_to_point_index.get(group_index).copied() else {
            continue;
        };
        let Some(trace_line) = point_trace_lines.iter().find(|line| {
            line.debug
                .as_ref()
                .is_some_and(|debug| debug.group_ordinal == host_ordinal)
                && matches!(
                    line.binding,
                    Some(crate::runtime::scene::LineBinding::PointTrace { .. })
                )
        }) else {
            continue;
        };
        let trace_points = if trace_line.points.len() >= 2 {
            trace_line.points.clone()
        } else if let Some(point_index) = existing_point_index
            && let Some(point) = visible_points.get(point_index)
        {
            // Keep the payload trace relationship even when its current
            // geometry is degenerate. The runtime will replace this
            // zero-length placeholder as soon as the upstream construction
            // produces a sampled trace again.
            vec![point.position.clone(), point.position.clone()]
        } else {
            continue;
        };
        if group.header.kind() == GroupKind::ParameterControlledPoint
            && let Ok(parameter_point) = try_decode_parameter_controlled_point_on_polyline(
                file,
                groups,
                group,
                anchors,
                host_ordinal,
                &trace_points,
            )
            && let Some(mut point) = scene_point_from_parameter_controlled(
                file,
                groups,
                anchors,
                group_to_point_index,
                parameter_point,
                crate::runtime::geometry::color_from_style(group.header.style_b),
                !group.header.is_hidden(),
            )
        {
            point.debug = Some(payload_debug_source(group));
            let point_index = existing_point_index.unwrap_or_else(|| {
                let next_index = visible_points.len();
                group_to_point_index[group_index] = Some(next_index);
                visible_points.push(point.clone());
                next_index
            });
            if let Some(existing) = visible_points.get_mut(point_index) {
                *existing = point;
            }
            continue;
        }
        let normalized_t = group
            .records
            .iter()
            .find(|record| record.record_type == RECORD_BINDING_PAYLOAD)
            .map(|record| record.payload(&file.data))
            .filter(|payload| payload.len() >= 12)
            .map(|payload| read_f64(payload, 4))
            .filter(|value| value.is_finite())
            .or_else(|| {
                let parameter_anchor = path
                    .refs
                    .first()
                    .and_then(|ordinal| groups.get(ordinal.saturating_sub(1)))?;
                if parameter_anchor.header.kind() != GroupKind::ParameterAnchor {
                    return None;
                }
                let anchor_path = find_indexed_path(file, parameter_anchor)?;
                let source_group_index = anchor_path.refs.first()?.checked_sub(1)?;
                let source_point_index = (*group_to_point_index.get(source_group_index)?)?;
                let source = &visible_points.get(source_point_index)?.position;
                nearest_sampled_polyline_parameter(&trace_points, source)
            });
        let Some(normalized_t) = normalized_t else {
            continue;
        };
        let wrapped_t = normalized_t.rem_euclid(1.0);
        let scaled = wrapped_t * (trace_points.len() - 1) as f64;
        let segment_index = (scaled.floor() as usize).min(trace_points.len() - 2);
        let t = scaled.fract();
        let start = &trace_points[segment_index];
        let end = &trace_points[segment_index + 1];
        let position = PointRecord {
            x: start.x + (end.x - start.x) * t,
            y: start.y + (end.y - start.y) * t,
        };
        let point_index = existing_point_index.unwrap_or_else(|| {
            let next_index = visible_points.len();
            group_to_point_index[group_index] = Some(next_index);
            visible_points.push(ScenePoint {
                position: position.clone(),
                color: crate::runtime::geometry::color_from_style(group.header.style_b),
                visible: !group.header.is_hidden(),
                draggable: true,
                constraint: ScenePointConstraint::Free,
                binding: None,
                debug: Some(payload_debug_source(group)),
            });
            next_index
        });
        if let Some(point) = visible_points.get_mut(point_index) {
            point.position = position;
            point.constraint = ScenePointConstraint::OnPolyline {
                function_key: host_ordinal,
                points: trace_points,
                segment_index,
                t,
            };
            point.draggable = true;
        }
    }
}

fn nearest_sampled_polyline_parameter(points: &[PointRecord], point: &PointRecord) -> Option<f64> {
    if points.len() < 2 {
        return None;
    }
    let mut nearest = None::<(f64, usize, f64)>;
    for (index, segment) in points.windows(2).enumerate() {
        let dx = segment[1].x - segment[0].x;
        let dy = segment[1].y - segment[0].y;
        let length_sq = dx * dx + dy * dy;
        if length_sq <= 1e-12 {
            continue;
        }
        let t = (((point.x - segment[0].x) * dx + (point.y - segment[0].y) * dy) / length_sq)
            .clamp(0.0, 1.0);
        let projected_x = segment[0].x + dx * t;
        let projected_y = segment[0].y + dy * t;
        let distance_sq = (point.x - projected_x).powi(2) + (point.y - projected_y).powi(2);
        if nearest.is_none_or(|(best, _, _)| distance_sq < best) {
            nearest = Some((distance_sq, index, t));
        }
    }
    nearest.map(|(_, index, t)| (index as f64 + t) / (points.len() - 1) as f64)
}

pub(super) fn collect_colorized_spectrum_lines(
    file: &GspFile,
    groups: &[ObjectGroup],
    raw_anchors: &[Option<PointRecord>],
    visible_points: &[ScenePoint],
    group_to_point_index: &[Option<usize>],
) -> Vec<LineShape> {
    let mut lines = Vec::new();
    let mut seen = BTreeSet::new();
    for binding_group in groups
        .iter()
        .filter(|group| group.header.kind() == GroupKind::IterationBinding)
    {
        let Some(binding_path) = find_indexed_path(file, binding_group) else {
            continue;
        };
        let Some(source_ordinal) = binding_path.refs.first().copied() else {
            continue;
        };
        let Some(iter_ordinal) = binding_path.refs.get(1).copied() else {
            continue;
        };
        let Some(source_group) = groups.get(source_ordinal.saturating_sub(1)) else {
            continue;
        };
        if source_group.header.kind() != GroupKind::DerivedSegment75 {
            continue;
        }
        if !seen.insert(source_ordinal) {
            continue;
        }
        let Some(source_path) = find_indexed_path(file, source_group) else {
            continue;
        };
        let Some(host_ordinal) = source_path.refs.first().copied() else {
            continue;
        };
        let Some(host_group) = groups.get(host_ordinal.saturating_sub(1)) else {
            continue;
        };
        if !matches!(
            host_group.header.kind(),
            GroupKind::Segment | GroupKind::Ray
        ) {
            continue;
        }
        let Some(host_path) = find_indexed_path(file, host_group) else {
            continue;
        };
        if host_path.refs.len() != 2 {
            continue;
        }
        let Some((trace_point_ordinal, trace_endpoint_index, other_ordinal)) = host_path
            .refs
            .iter()
            .copied()
            .enumerate()
            .find_map(|(endpoint_index, ordinal)| {
                let point_index = group_to_point_index
                    .get(ordinal.checked_sub(1)?)
                    .copied()
                    .flatten()?;
                let point = visible_points.get(point_index)?;
                matches!(point.constraint, ScenePointConstraint::OnPolyline { .. }).then_some((
                    ordinal,
                    endpoint_index,
                    *host_path
                        .refs
                        .iter()
                        .find(|candidate| **candidate != ordinal)?,
                ))
            })
        else {
            continue;
        };
        let Some(trace_point_group_index) = trace_point_ordinal.checked_sub(1) else {
            continue;
        };
        let Some(trace_point_index) = group_to_point_index
            .get(trace_point_group_index)
            .copied()
            .flatten()
        else {
            continue;
        };
        let Some(host_group_index) = host_ordinal.checked_sub(1) else {
            continue;
        };
        let Some(trace_point) = visible_points.get(trace_point_index) else {
            continue;
        };
        let ScenePointConstraint::OnPolyline {
            function_key,
            points,
            segment_index,
            t,
            ..
        } = &trace_point.constraint
        else {
            continue;
        };
        let Some(trace_line_group_index) = function_key.checked_sub(1) else {
            continue;
        };
        if points.len() < 2 {
            continue;
        }
        let Some(iter_group) = groups.get(iter_ordinal.saturating_sub(1)) else {
            continue;
        };
        let depth = iter_group
            .records
            .iter()
            .find(|record| record.record_type == RECORD_ITERATION_DEFINITION)
            .map(|record| record.payload(&file.data))
            .filter(|payload| payload.len() >= 20)
            .map(|payload| read_u32(payload, 16) as usize)
            .unwrap_or(0);
        if depth == 0 {
            continue;
        }
        let depth_parameter_name = iteration_depth_parameter_name(file, groups, iter_group);
        let base = (*segment_index as f64 + *t) / (points.len() - 1) as f64;
        let other_point = group_to_point_index
            .get(other_ordinal.saturating_sub(1))
            .copied()
            .flatten()
            .and_then(|point_index| visible_points.get(point_index))
            .map(|point| point.position.clone())
            .or_else(|| {
                raw_anchors
                    .get(other_ordinal.saturating_sub(1))
                    .and_then(Clone::clone)
            });
        let reflected_endpoint = groups
            .get(other_ordinal.saturating_sub(1))
            .filter(|group| group.header.kind() == GroupKind::Reflection)
            .and_then(|group| find_indexed_path(file, group))
            .and_then(|path| {
                Some((
                    path.refs.first()?.checked_sub(1)?,
                    path.refs.get(1)?.checked_sub(1)?,
                ))
            });
        let sampled_reflection_axis = reflected_endpoint.and_then(|(_, axis_line_group_index)| {
            sampled_reflection_axis_driver(
                file,
                groups,
                axis_line_group_index,
                trace_point_group_index,
            )
        });
        for step in 0..depth {
            let normalized = (base + step as f64 / depth as f64).rem_euclid(1.0);
            let Some(start) = interpolate_polyline(points, normalized) else {
                continue;
            };
            let Some(end) = other_point.clone() else {
                continue;
            };
            let [red, green, blue] = normalized_hsb(step as f64 / depth as f64, 1.0, 1.0);
            lines.push(LineShape {
                points: vec![start, end],
                color: [red, green, blue, 255],
                dashed: false,
                stroke_width: Some(line_stroke_width_from_style(host_group.header.style_a)),
                visible: !iter_group.header.is_hidden(),
                binding: Some(crate::runtime::scene::LineBinding::ColorizedSpectrum {
                    line_index: host_group_index,
                    trace_line_index: trace_line_group_index,
                    point_index: trace_point_group_index,
                    trace_endpoint_index,
                    reflection_source_index: reflected_endpoint
                        .map(|(source_index, _)| source_index),
                    reflection_axis_line_index: reflected_endpoint
                        .map(|(_, line_index)| line_index),
                    reflection_focus_index: sampled_reflection_axis
                        .map(|(focus_group_index, _)| focus_group_index),
                    reflection_directrix_line_index: sampled_reflection_axis
                        .map(|(_, directrix_line_group_index)| directrix_line_group_index),
                    step_index: step,
                    depth,
                    depth_parameter_name: depth_parameter_name.clone(),
                    ray: host_group.header.kind() == GroupKind::Ray,
                }),
                debug: Some(payload_debug_source(source_group)),
            });
        }
    }
    lines
}

fn sampled_reflection_axis_driver(
    file: &GspFile,
    groups: &[ObjectGroup],
    axis_line_group_index: usize,
    trace_point_group_index: usize,
) -> Option<(usize, usize)> {
    let axis_group = groups.get(axis_line_group_index)?;
    if axis_group.header.kind() != GroupKind::PerpendicularLine {
        return None;
    }
    let axis_path = find_indexed_path(file, axis_group)?;
    let [through_ordinal, host_line_ordinal] = axis_path.refs.as_slice() else {
        return None;
    };
    if through_ordinal.checked_sub(1)? != trace_point_group_index {
        return None;
    }

    let host_line_group = groups.get(host_line_ordinal.checked_sub(1)?)?;
    let host_line_path = find_indexed_path(file, host_line_group)?;
    let intersection_group_index = host_line_path.refs.iter().find_map(|ordinal| {
        let index = ordinal.checked_sub(1)?;
        (index != trace_point_group_index).then_some(index)
    })?;

    let intersection_group = groups.get(intersection_group_index)?;
    let intersection_path = find_indexed_path(file, intersection_group)?;
    let mut directrix_line_group_index = None;
    let mut bisector_group_index = None;
    for ordinal in &intersection_path.refs {
        let index = ordinal.checked_sub(1)?;
        let group = groups.get(index)?;
        if group.header.kind() == GroupKind::AngleBisectorRay {
            bisector_group_index = Some(index);
        } else {
            directrix_line_group_index = Some(index);
        }
    }
    let directrix_line_group_index = directrix_line_group_index?;
    let bisector_group = groups.get(bisector_group_index?)?;
    let bisector_path = find_indexed_path(file, bisector_group)?;
    let [focus_ordinal, vertex_ordinal, _] = bisector_path.refs.as_slice() else {
        return None;
    };
    if vertex_ordinal.checked_sub(1)? != trace_point_group_index {
        return None;
    }
    Some((focus_ordinal.checked_sub(1)?, directrix_line_group_index))
}

fn iteration_depth_parameter_name(
    file: &GspFile,
    groups: &[ObjectGroup],
    iter_group: &ObjectGroup,
) -> Option<String> {
    let iter_path = find_indexed_path(file, iter_group)?;
    let parameter_group = groups.get(iter_path.refs.first()?.checked_sub(1)?)?;
    decode_label_name(file, parameter_group)
        .or_else(|| editable_non_graph_parameter_name_for_group(file, groups, parameter_group))
}

fn interpolate_polyline(points: &[PointRecord], normalized: f64) -> Option<PointRecord> {
    if points.len() < 2 {
        return None;
    }
    let scaled = normalized.rem_euclid(1.0) * (points.len() - 1) as f64;
    let segment_index = (scaled.floor() as usize).min(points.len() - 2);
    let t = scaled.fract();
    let start = points.get(segment_index)?;
    let end = points.get(segment_index + 1)?;
    Some(PointRecord {
        x: start.x + (end.x - start.x) * t,
        y: start.y + (end.y - start.y) * t,
    })
}

fn refresh_sampled_trace_polylines(
    file: &GspFile,
    groups: &[ObjectGroup],
    points: &mut [ScenePoint],
    group_to_point_index: &[Option<usize>],
    graph_ref: &Option<GraphTransform>,
) {
    let updates = points
        .iter()
        .enumerate()
        .filter_map(|(point_index, point)| match &point.constraint {
            ScenePointConstraint::OnPolyline {
                function_key,
                points: polyline,
                segment_index,
                t,
            } => {
                if polyline.len() < 2 {
                    return None;
                }
                let normalized = (*segment_index as f64 + *t)
                    .clamp(0.0, (polyline.len() - 1) as f64)
                    / (polyline.len() - 1) as f64;
                let group = function_key
                    .checked_sub(1)
                    .and_then(|group_index| groups.get(group_index))?;
                let position = sample_point_trace_at_parameter_normalized(
                    file,
                    groups,
                    group,
                    points,
                    group_to_point_index,
                    graph_ref,
                    normalized,
                )?;
                Some((point_index, position))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    for (point_index, position) in updates {
        if let Some(point) = points.get_mut(point_index) {
            point.position = position;
            point.constraint = ScenePointConstraint::Free;
        }
    }
}

fn sample_point_trace_at_parameter_normalized(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
    visible_points: &[ScenePoint],
    group_to_point_index: &[Option<usize>],
    graph_ref: &Option<GraphTransform>,
    normalized_parameter: f64,
) -> Option<PointRecord> {
    if group.header.kind() != GroupKind::PointTrace {
        return None;
    }
    let path = find_indexed_path(file, group)?;
    let target_group_index = path.refs.first()?.checked_sub(1)?;
    let target_group = groups.get(target_group_index)?;
    let target_point_index = (*group_to_point_index.get(target_group_index)?)?;
    let payload = group
        .records
        .iter()
        .find(|record| {
            record.record_type == crate::runtime::payload_consts::RECORD_FUNCTION_PLOT_DESCRIPTOR
        })
        .map(|record| record.payload(&file.data))?;
    let descriptor =
        crate::runtime::functions::try_decode_function_plot_descriptor(payload).ok()?;
    if matches!(target_group.header.kind(), GroupKind::CoordinatePoint)
        && let Some(points) = sample_coordinate_point_trace(
            file,
            groups,
            group,
            target_group,
            CoordinateTraceSampleSpec {
                x_min: descriptor.x_min,
                x_max: descriptor.x_max,
                sample_count: descriptor.sample_count,
            },
            graph_ref,
        )
    {
        if points.len() < 2 {
            return None;
        }
        let scaled = normalized_parameter.clamp(0.0, 1.0) * (points.len() - 1) as f64;
        let segment_index = (scaled.floor() as usize).min(points.len() - 2);
        return Some(lerp_point(
            &points[segment_index],
            &points[segment_index + 1],
            scaled.fract(),
        ));
    }
    let (driver_point_index, _) = path
        .refs
        .iter()
        .filter_map(|ordinal| trace_driver_point(visible_points, group_to_point_index, *ordinal))
        .find(|(_, group_index)| *group_index != target_group_index)?;
    let parameter = descriptor.x_min
        + (descriptor.x_max - descriptor.x_min) * normalized_parameter.clamp(0.0, 1.0);
    let mut sampled_points = visible_points.to_vec();
    let driver_point = sampled_points.get_mut(driver_point_index)?;
    apply_trace_parameter_with_mode(
        driver_point,
        parameter,
        descriptor.x_min,
        descriptor.x_max,
        false,
    );
    let trace_parameters = trace_parameter_values(
        file,
        groups,
        &path.refs,
        &mut sampled_points,
        group_to_point_index,
    );
    refresh_trace_expression_scale_factors(&mut sampled_points, &trace_parameters);
    resolve_trace_point(
        &mut sampled_points,
        target_point_index,
        &mut BTreeSet::new(),
    )
}

fn trace_parameter_values(
    file: &GspFile,
    groups: &[ObjectGroup],
    trace_refs: &[usize],
    points: &mut [ScenePoint],
    group_to_point_index: &[Option<usize>],
) -> BTreeMap<String, f64> {
    trace_refs
        .iter()
        .filter_map(|ordinal| groups.get(ordinal.checked_sub(1)?))
        .filter(|group| group.header.kind() == GroupKind::RatioValue)
        .filter_map(|group| {
            let name = decode_label_name(file, group)?;
            let path = find_indexed_path(file, group)?;
            let origin = trace_point_for_group(points, group_to_point_index, *path.refs.first()?)?;
            let denominator =
                trace_point_for_group(points, group_to_point_index, *path.refs.get(1)?)?;
            let numerator =
                trace_point_for_group(points, group_to_point_index, *path.refs.get(2)?)?;
            let denominator_length = (denominator.x - origin.x).hypot(denominator.y - origin.y);
            if denominator_length <= 1e-9 {
                return None;
            }
            let numerator_length = (numerator.x - origin.x).hypot(numerator.y - origin.y);
            Some((name, numerator_length / denominator_length))
        })
        .collect()
}

fn trace_point_for_group(
    points: &mut [ScenePoint],
    group_to_point_index: &[Option<usize>],
    ordinal: usize,
) -> Option<PointRecord> {
    let group_index = ordinal.checked_sub(1)?;
    let point_index = (*group_to_point_index.get(group_index)?)?;
    resolve_trace_point(points, point_index, &mut BTreeSet::new())
}

fn refresh_trace_expression_scale_factors(
    points: &mut [ScenePoint],
    parameters: &BTreeMap<String, f64>,
) {
    for point in points {
        let Some(ScenePointBinding::Scale {
            factor,
            factor_expr: Some(expr),
            ..
        }) = &mut point.binding
        else {
            continue;
        };
        if let Some(value) = evaluate_expr_with_parameters(expr, 0.0, parameters) {
            *factor = value;
        }
    }
}

fn sample_coordinate_point_trace(
    file: &GspFile,
    groups: &[ObjectGroup],
    trace_group: &ObjectGroup,
    target_group: &ObjectGroup,
    sample_spec: CoordinateTraceSampleSpec,
    graph_ref: &Option<GraphTransform>,
) -> Option<Vec<PointRecord>> {
    let target_path = find_indexed_path(file, target_group)?;
    if target_path.refs.len() < 2 {
        return None;
    }
    let x_calc_group = groups.get(target_path.refs[0].checked_sub(1)?)?;
    let y_calc_group = groups.get(target_path.refs[1].checked_sub(1)?)?;
    let parameter_anchor_group = find_indexed_path(file, trace_group)?
        .refs
        .iter()
        .filter_map(|ordinal| groups.get(ordinal.checked_sub(1)?))
        .find(|group| (group.header.kind()) == crate::format::GroupKind::ParameterAnchor)?;
    let parameter_name = decode_label_name(file, parameter_anchor_group).or_else(|| {
        let path = find_indexed_path(file, parameter_anchor_group)?;
        let point_group = groups.get(path.refs.first()?.checked_sub(1)?)?;
        decode_label_name(file, point_group)
    })?;
    let x_expr = try_decode_function_expr(file, groups, x_calc_group).ok()?;
    let y_expr = try_decode_function_expr(file, groups, y_calc_group).ok()?;
    let last = sample_spec.sample_count.saturating_sub(1).max(1) as f64;
    let mut points = Vec::with_capacity(sample_spec.sample_count);
    for sample_index in 0..sample_spec.sample_count {
        let t = sample_index as f64 / last;
        let value = sample_spec.x_min + (sample_spec.x_max - sample_spec.x_min) * t;
        let parameters = std::collections::BTreeMap::from([(parameter_name.clone(), value)]);
        let x = evaluate_expr_with_parameters(&x_expr, 0.0, &parameters)?;
        let y = evaluate_expr_with_parameters(&y_expr, 0.0, &parameters)?;
        let world = PointRecord { x, y };
        points.push(if let Some(transform) = graph_ref {
            to_raw_from_world(&world, transform)
        } else {
            world
        });
    }
    (points.len() >= 2).then_some(points)
}

#[derive(Clone, Copy)]
struct CoordinateTraceSampleSpec {
    x_min: f64,
    x_max: f64,
    sample_count: usize,
}

fn point_accepts_trace_parameter(point: &ScenePoint) -> bool {
    if matches!(point.binding, Some(ScenePointBinding::Midpoint { .. })) {
        return false;
    }
    matches!(
        point.constraint,
        ScenePointConstraint::OnSegment { .. }
            | ScenePointConstraint::OnLine { .. }
            | ScenePointConstraint::OnLineConstraint { .. }
            | ScenePointConstraint::OnRay { .. }
            | ScenePointConstraint::OnRayConstraint { .. }
            | ScenePointConstraint::OnPolyline { .. }
            | ScenePointConstraint::OnPolygonBoundary { .. }
            | ScenePointConstraint::OnTranslatedPolygonBoundary { .. }
            | ScenePointConstraint::OnCircle { .. }
            | ScenePointConstraint::OnCircularConstraint { .. }
            | ScenePointConstraint::OnCircleArc { .. }
            | ScenePointConstraint::OnArc { .. }
    )
}

fn apply_trace_parameter_with_mode(
    point: &mut ScenePoint,
    value: f64,
    x_min: f64,
    x_max: f64,
    use_raw_value: bool,
) {
    let normalized = if use_raw_value {
        value
    } else if (x_max - x_min).abs() <= 1e-9 {
        0.0
    } else {
        ((value - x_min) / (x_max - x_min)).clamp(0.0, 1.0)
    };
    match &mut point.constraint {
        ScenePointConstraint::OnSegment { t, .. } => {
            *t = normalized;
        }
        ScenePointConstraint::OnLine { t, .. } => {
            *t = value;
        }
        ScenePointConstraint::OnLineConstraint { t, .. } => {
            *t = value;
        }
        ScenePointConstraint::OnRay { t, .. } => {
            *t = value.max(0.0);
        }
        ScenePointConstraint::OnRayConstraint { t, .. } => {
            *t = value.max(0.0);
        }
        ScenePointConstraint::OnPolyline {
            points,
            segment_index,
            t,
            ..
        } => {
            if points.len() < 2 {
                return;
            }
            let scaled = normalized * (points.len() - 1) as f64;
            *segment_index = (scaled.floor() as usize).min(points.len() - 2);
            *t = scaled.fract();
        }
        ScenePointConstraint::OnPolygonBoundary {
            vertex_indices,
            edge_index,
            t,
        } => {
            if vertex_indices.len() < 2 {
                return;
            }
            let scaled = normalized * vertex_indices.len() as f64;
            let next_edge = scaled.floor() as usize;
            *edge_index = next_edge.min(vertex_indices.len() - 1);
            *t = scaled.fract();
        }
        ScenePointConstraint::OnTranslatedPolygonBoundary {
            vertex_indices,
            edge_index,
            t,
            ..
        } => {
            if vertex_indices.len() < 2 {
                return;
            }
            let scaled = normalized * vertex_indices.len() as f64;
            let next_edge = scaled.floor() as usize;
            *edge_index = next_edge.min(vertex_indices.len() - 1);
            *t = scaled.fract();
        }
        ScenePointConstraint::OnCircle { unit_x, unit_y, .. } => {
            let angle = value;
            *unit_x = angle.cos();
            *unit_y = -angle.sin();
        }
        ScenePointConstraint::OnCircularConstraint { unit_x, unit_y, .. } => {
            let angle = value;
            *unit_x = angle.cos();
            *unit_y = -angle.sin();
        }
        ScenePointConstraint::OnCircleArc { t, .. } => {
            *t = normalized;
        }
        ScenePointConstraint::OnArc { t, .. } => {
            *t = normalized;
        }
        _ => {}
    }
}

fn trace_parameter_value_from_point(
    points: &mut [ScenePoint],
    index: usize,
    visiting: &mut BTreeSet<usize>,
) -> Option<f64> {
    let constraint = points.get(index)?.constraint.clone();
    match constraint {
        ScenePointConstraint::OnSegment { t, .. }
        | ScenePointConstraint::OnLine { t, .. }
        | ScenePointConstraint::OnLineConstraint { t, .. }
        | ScenePointConstraint::OnRay { t, .. }
        | ScenePointConstraint::OnRayConstraint { t, .. }
        | ScenePointConstraint::OnPolyline { t, .. }
        | ScenePointConstraint::OnCircleArc { t, .. }
        | ScenePointConstraint::OnArc { t, .. } => Some(t),
        ScenePointConstraint::OnPolygonBoundary {
            vertex_indices,
            edge_index,
            t,
        } => trace_polygon_boundary_parameter(points, &vertex_indices, edge_index, t, visiting),
        ScenePointConstraint::OnCircle { unit_x, unit_y, .. }
        | ScenePointConstraint::OnCircularConstraint { unit_x, unit_y, .. } => {
            Some((-unit_y).atan2(unit_x).rem_euclid(std::f64::consts::TAU) / std::f64::consts::TAU)
        }
        _ => None,
    }
}

fn trace_polygon_boundary_parameter(
    points: &mut [ScenePoint],
    vertex_indices: &[usize],
    edge_index: usize,
    t: f64,
    visiting: &mut BTreeSet<usize>,
) -> Option<f64> {
    if vertex_indices.len() < 2 {
        return None;
    }
    let mut perimeter = 0.0;
    let mut traveled = 0.0;
    for index in 0..vertex_indices.len() {
        let start = resolve_trace_point(points, vertex_indices[index], visiting)?;
        let end = resolve_trace_point(
            points,
            vertex_indices[(index + 1) % vertex_indices.len()],
            visiting,
        )?;
        let length = (end.x - start.x).hypot(end.y - start.y);
        perimeter += length;
        if index < edge_index % vertex_indices.len() {
            traveled += length;
        } else if index == edge_index % vertex_indices.len() {
            traveled += length * t.clamp(0.0, 1.0);
        }
    }
    (perimeter > 1e-9).then_some(traveled / perimeter)
}

fn resolve_trace_point_at_constraint_parameter(
    points: &mut [ScenePoint],
    point: &ScenePoint,
    value: f64,
    visiting: &mut BTreeSet<usize>,
) -> Option<PointRecord> {
    match &point.constraint {
        ScenePointConstraint::OnSegment {
            start_index,
            end_index,
            ..
        }
        | ScenePointConstraint::OnLine {
            start_index,
            end_index,
            ..
        }
        | ScenePointConstraint::OnRay {
            start_index,
            end_index,
            ..
        } => {
            let t = match &point.constraint {
                ScenePointConstraint::OnSegment { .. } => value.rem_euclid(1.0),
                ScenePointConstraint::OnRay { .. } => value.max(0.0),
                _ => value,
            };
            let start = resolve_trace_point(points, *start_index, visiting)?;
            let end = resolve_trace_point(points, *end_index, visiting)?;
            Some(lerp_point(&start, &end, t))
        }
        ScenePointConstraint::OnPolygonBoundary { vertex_indices, .. } => {
            let (edge_index, t) =
                trace_polygon_parameter_to_edge(points, vertex_indices, value, visiting)?;
            let start = resolve_trace_point(
                points,
                vertex_indices[edge_index % vertex_indices.len()],
                visiting,
            )?;
            let end = resolve_trace_point(
                points,
                vertex_indices[(edge_index + 1) % vertex_indices.len()],
                visiting,
            )?;
            Some(lerp_point(&start, &end, t))
        }
        ScenePointConstraint::OnTranslatedPolygonBoundary {
            vertex_indices,
            vector_start_index,
            vector_end_index,
            ..
        } => {
            let (edge_index, t) =
                trace_polygon_parameter_to_edge(points, vertex_indices, value, visiting)?;
            let start = resolve_trace_point(
                points,
                vertex_indices[edge_index % vertex_indices.len()],
                visiting,
            )?;
            let end = resolve_trace_point(
                points,
                vertex_indices[(edge_index + 1) % vertex_indices.len()],
                visiting,
            )?;
            let vector_start = resolve_trace_point(points, *vector_start_index, visiting)?;
            let vector_end = resolve_trace_point(points, *vector_end_index, visiting)?;
            let point = lerp_point(&start, &end, t);
            Some(PointRecord {
                x: point.x + (vector_end.x - vector_start.x),
                y: point.y + (vector_end.y - vector_start.y),
            })
        }
        ScenePointConstraint::OnCircle {
            center_index,
            radius_index,
            ..
        } => {
            let angle = std::f64::consts::TAU * value.rem_euclid(1.0);
            let center = resolve_trace_point(points, *center_index, visiting)?;
            let radius_point = resolve_trace_point(points, *radius_index, visiting)?;
            let radius = (radius_point.x - center.x).hypot(radius_point.y - center.y);
            Some(PointRecord {
                x: center.x + radius * angle.cos(),
                y: center.y - radius * angle.sin(),
            })
        }
        ScenePointConstraint::OnCircleArc {
            center_index,
            start_index,
            end_index,
            ..
        } => {
            let center = resolve_trace_point(points, *center_index, visiting)?;
            let start = resolve_trace_point(points, *start_index, visiting)?;
            let end = resolve_trace_point(points, *end_index, visiting)?;
            point_on_circle_arc(&center, &start, &end, value.rem_euclid(1.0))
        }
        ScenePointConstraint::OnArc {
            start_index,
            mid_index,
            end_index,
            ..
        } => {
            let start = resolve_trace_point(points, *start_index, visiting)?;
            let mid = resolve_trace_point(points, *mid_index, visiting)?;
            let end = resolve_trace_point(points, *end_index, visiting)?;
            point_on_three_point_arc(&start, &mid, &end, value.rem_euclid(1.0))
        }
        _ => None,
    }
}

fn trace_polygon_parameter_to_edge(
    points: &mut [ScenePoint],
    vertex_indices: &[usize],
    value: f64,
    visiting: &mut BTreeSet<usize>,
) -> Option<(usize, f64)> {
    if vertex_indices.len() < 2 {
        return None;
    }
    let mut lengths = Vec::with_capacity(vertex_indices.len());
    let mut perimeter = 0.0;
    for index in 0..vertex_indices.len() {
        let start = resolve_trace_point(points, vertex_indices[index], visiting)?;
        let end = resolve_trace_point(
            points,
            vertex_indices[(index + 1) % vertex_indices.len()],
            visiting,
        )?;
        let length = (end.x - start.x).hypot(end.y - start.y);
        lengths.push(length);
        perimeter += length;
    }
    if perimeter <= 1e-9 {
        return None;
    }
    let target = value.rem_euclid(1.0) * perimeter;
    let mut traveled = 0.0;
    for (edge_index, length) in lengths.iter().copied().enumerate() {
        if traveled + length >= target || edge_index == lengths.len() - 1 {
            let t = if length <= 1e-9 {
                0.0
            } else {
                ((target - traveled) / length).clamp(0.0, 1.0)
            };
            return Some((edge_index, t));
        }
        traveled += length;
    }
    None
}

fn resolve_trace_point(
    points: &mut [ScenePoint],
    index: usize,
    visiting: &mut BTreeSet<usize>,
) -> Option<PointRecord> {
    if !visiting.insert(index) {
        return None;
    }

    let point = points.get(index)?.clone();
    let resolved = match &point.binding {
        Some(ScenePointBinding::DirectedAngleAnchor {
            first_start_index,
            first_end_index,
            second_start_index,
            second_end_index,
            distance,
            parameter,
        }) => gsp_runtime_core::directed_angle_anchor(
            to_core_point(&resolve_trace_point(points, *first_start_index, visiting)?),
            to_core_point(&resolve_trace_point(points, *first_end_index, visiting)?),
            to_core_point(&resolve_trace_point(points, *second_start_index, visiting)?),
            to_core_point(&resolve_trace_point(points, *second_end_index, visiting)?),
            *distance,
            *parameter,
        )
        .map(from_core_point),
        Some(ScenePointBinding::Translate {
            source_index,
            vector_start_index,
            vector_end_index,
        }) => {
            let source = resolve_trace_point(points, *source_index, visiting)?;
            let vector_start = resolve_trace_point(points, *vector_start_index, visiting)?;
            let vector_end = resolve_trace_point(points, *vector_end_index, visiting)?;
            Some(PointRecord {
                x: source.x + (vector_end.x - vector_start.x),
                y: source.y + (vector_end.y - vector_start.y),
            })
        }
        Some(ScenePointBinding::Reflect {
            source_index,
            line_start_index,
            line_end_index,
        }) => {
            let source = resolve_trace_point(points, *source_index, visiting)?;
            let line_start = resolve_trace_point(points, *line_start_index, visiting)?;
            let line_end = resolve_trace_point(points, *line_end_index, visiting)?;
            reflect_across_line(&source, &line_start, &line_end)
        }
        Some(ScenePointBinding::ReflectLineConstraint { source_index, line }) => {
            let source = resolve_trace_point(points, *source_index, visiting)?;
            let (line_start, line_end, _) = resolve_trace_line_constraint(points, line, visiting)?;
            reflect_across_line(&source, &line_start, &line_end)
        }
        Some(ScenePointBinding::Rotate {
            source_index,
            center_index,
            angle_degrees,
            angle_start_index,
            angle_vertex_index,
            angle_end_index,
            angle_parameter_point_index,
            angle_parameter_start_index,
            angle_parameter_end_index,
            angle_parameter_scale,
            ..
        }) => {
            let source = resolve_trace_point(points, *source_index, visiting)?;
            let center = resolve_trace_point(points, *center_index, visiting)?;
            let resolved_angle = if let (
                Some(angle_parameter_point_index),
                Some(angle_parameter_start_index),
                Some(angle_parameter_end_index),
            ) = (
                angle_parameter_point_index,
                angle_parameter_start_index,
                angle_parameter_end_index,
            ) {
                let point = resolve_trace_point(points, *angle_parameter_point_index, visiting)?;
                let start = resolve_trace_point(points, *angle_parameter_start_index, visiting)?;
                let end = resolve_trace_point(points, *angle_parameter_end_index, visiting)?;
                let dx = end.x - start.x;
                let dy = end.y - start.y;
                let len_sq = dx * dx + dy * dy;
                if len_sq <= 1e-9 {
                    return None;
                }
                let t = (((point.x - start.x) * dx + (point.y - start.y) * dy) / len_sq)
                    .clamp(0.0, 1.0);
                t * angle_parameter_scale.unwrap_or(1.0)
            } else {
                match (angle_start_index, angle_vertex_index, angle_end_index) {
                    (Some(angle_start_index), Some(angle_vertex_index), Some(angle_end_index)) => {
                        let angle_start =
                            resolve_trace_point(points, *angle_start_index, visiting)?;
                        let angle_vertex =
                            resolve_trace_point(points, *angle_vertex_index, visiting)?;
                        let angle_end = resolve_trace_point(points, *angle_end_index, visiting)?;
                        crate::runtime::geometry::angle_degrees_from_points(
                            &angle_start,
                            &angle_vertex,
                            &angle_end,
                        )?
                    }
                    _ => *angle_degrees,
                }
            };
            Some(rotate_around(&source, &center, resolved_angle.to_radians()))
        }
        Some(ScenePointBinding::Scale {
            source_index,
            center_index,
            factor,
            factor_parameter_point_index,
            factor_parameter_start_index,
            factor_parameter_end_index,
            ..
        }) => {
            let source = resolve_trace_point(points, *source_index, visiting)?;
            let center = resolve_trace_point(points, *center_index, visiting)?;
            let factor = match (
                factor_parameter_point_index,
                factor_parameter_start_index,
                factor_parameter_end_index,
            ) {
                (Some(point_index), Some(start_index), Some(end_index)) => {
                    let point = resolve_trace_point(points, *point_index, visiting)?;
                    let start = resolve_trace_point(points, *start_index, visiting)?;
                    let end = resolve_trace_point(points, *end_index, visiting)?;
                    segment_projection_parameter(&point, &start, &end)?
                }
                _ => *factor,
            };
            Some(scale_around(&source, &center, factor))
        }
        Some(ScenePointBinding::ScaleByRatio {
            source_index,
            center_index,
            ratio_origin_index,
            ratio_denominator_index,
            ratio_numerator_index,
            signed,
            clamp_to_unit,
        }) => {
            let source = resolve_trace_point(points, *source_index, visiting)?;
            let center = resolve_trace_point(points, *center_index, visiting)?;
            let ratio_origin = resolve_trace_point(points, *ratio_origin_index, visiting)?;
            let ratio_denominator =
                resolve_trace_point(points, *ratio_denominator_index, visiting)?;
            let ratio_numerator = resolve_trace_point(points, *ratio_numerator_index, visiting)?;
            gsp_runtime_core::scale_by_three_point_ratio(
                to_core_point(&source),
                to_core_point(&center),
                to_core_point(&ratio_origin),
                to_core_point(&ratio_denominator),
                to_core_point(&ratio_numerator),
                *signed,
                *clamp_to_unit,
            )
            .map(from_core_point)
        }
        Some(ScenePointBinding::Midpoint {
            start_index,
            end_index,
        }) => {
            let start = resolve_trace_point(points, *start_index, visiting)?;
            let end = resolve_trace_point(points, *end_index, visiting)?;
            Some(lerp_point(&start, &end, 0.5))
        }
        Some(ScenePointBinding::DerivedParameter {
            source_index,
            parameter_start_index,
            parameter_end_index,
        }) => {
            let value = match (parameter_start_index, parameter_end_index) {
                (Some(start_index), Some(end_index)) => {
                    let source = resolve_trace_point(points, *source_index, visiting)?;
                    let start = resolve_trace_point(points, *start_index, visiting)?;
                    let end = resolve_trace_point(points, *end_index, visiting)?;
                    segment_projection_parameter(&source, &start, &end)?
                }
                _ => {
                    let source = points.get(*source_index)?;
                    custom_transform_trace_parameter(source)?
                }
            };
            let derived = point.clone();
            resolve_trace_point_at_constraint_parameter(points, &derived, value, visiting)
        }
        Some(ScenePointBinding::CustomTransform {
            source_index,
            origin_index,
            axis_end_index,
            distance_expr,
            angle_expr,
            distance_raw_scale,
            angle_degrees_scale,
        }) => {
            let source_point = points.get(*source_index)?;
            let t = custom_transform_trace_parameter(source_point)?;
            let origin = resolve_trace_point(points, *origin_index, visiting)?;
            let axis_end = resolve_trace_point(points, *axis_end_index, visiting)?;
            let parameters =
                custom_transform_expression_parameter_map(distance_expr, angle_expr, t);
            let distance = crate::runtime::functions::evaluate_expr_with_parameters(
                distance_expr,
                t,
                &parameters,
            )? * distance_raw_scale;
            let angle_degrees = crate::runtime::functions::evaluate_expr_with_parameters(
                angle_expr,
                t,
                &parameters,
            )? * angle_degrees_scale;
            let base_angle = (-(axis_end.y - origin.y))
                .atan2(axis_end.x - origin.x)
                .to_degrees();
            let radians = (base_angle + angle_degrees).to_radians();
            Some(PointRecord {
                x: origin.x + distance * radians.cos(),
                y: origin.y - distance * radians.sin(),
            })
        }
        Some(ScenePointBinding::ConstraintParameterExpr { expr }) => {
            let value = evaluate_expr_with_parameters(expr, 0.0, &BTreeMap::new())?;
            resolve_trace_point_at_constraint_parameter(points, &point, value, visiting)
        }
        Some(ScenePointBinding::ConstraintParameterFromPointExpr {
            source_index,
            parameter_name,
            expr,
            absolute_value,
            ..
        }) => {
            let source_value = trace_parameter_value_from_point(points, *source_index, visiting)?;
            let mut parameters = BTreeMap::new();
            if !parameter_name.is_empty() {
                parameters.insert(parameter_name.clone(), source_value);
            }
            let expr_value = evaluate_expr_with_parameters(expr, 0.0, &parameters)?;
            let value = if *absolute_value {
                expr_value
            } else {
                source_value + expr_value
            };
            resolve_trace_point_at_constraint_parameter(points, &point, value, visiting)
        }
        _ => match &point.constraint {
            ScenePointConstraint::Free => Some(point.position.clone()),
            ScenePointConstraint::Offset {
                origin_index,
                dx,
                dy,
            } => {
                let origin = resolve_trace_point(points, *origin_index, visiting)?;
                Some(PointRecord {
                    x: origin.x + dx,
                    y: origin.y + dy,
                })
            }
            ScenePointConstraint::OnSegment {
                start_index,
                end_index,
                t,
            } => {
                let start = resolve_trace_point(points, *start_index, visiting)?;
                let end = resolve_trace_point(points, *end_index, visiting)?;
                Some(lerp_point(&start, &end, *t))
            }
            ScenePointConstraint::OnLine {
                start_index,
                end_index,
                t,
            } => {
                let start = resolve_trace_point(points, *start_index, visiting)?;
                let end = resolve_trace_point(points, *end_index, visiting)?;
                Some(lerp_point(&start, &end, *t))
            }
            ScenePointConstraint::OnLineConstraint { line, t } => {
                let (start, end, _) = resolve_trace_line_constraint(points, line, visiting)?;
                Some(lerp_point(&start, &end, *t))
            }
            ScenePointConstraint::OnRay {
                start_index,
                end_index,
                t,
            } => {
                let start = resolve_trace_point(points, *start_index, visiting)?;
                let end = resolve_trace_point(points, *end_index, visiting)?;
                Some(lerp_point(&start, &end, *t))
            }
            ScenePointConstraint::OnRayConstraint { line, t } => {
                let (start, end, _) = resolve_trace_line_constraint(points, line, visiting)?;
                Some(lerp_point(&start, &end, *t))
            }
            ScenePointConstraint::OnPolyline {
                points,
                segment_index,
                t,
                ..
            } => {
                if points.len() < 2 {
                    None
                } else {
                    let start = &points[(*segment_index).min(points.len() - 2)];
                    let end = &points[(*segment_index).min(points.len() - 2) + 1];
                    Some(lerp_point(start, end, *t))
                }
            }
            ScenePointConstraint::OnPolygonBoundary {
                vertex_indices,
                edge_index,
                t,
            } => {
                if vertex_indices.len() < 2 {
                    None
                } else {
                    let start = resolve_trace_point(
                        points,
                        vertex_indices[*edge_index % vertex_indices.len()],
                        visiting,
                    )?;
                    let end = resolve_trace_point(
                        points,
                        vertex_indices[(*edge_index + 1) % vertex_indices.len()],
                        visiting,
                    )?;
                    Some(lerp_point(&start, &end, *t))
                }
            }
            ScenePointConstraint::OnTranslatedPolygonBoundary {
                vertex_indices,
                vector_start_index,
                vector_end_index,
                edge_index,
                t,
            } => {
                if vertex_indices.len() < 2 {
                    None
                } else {
                    let start = resolve_trace_point(
                        points,
                        vertex_indices[*edge_index % vertex_indices.len()],
                        visiting,
                    )?;
                    let end = resolve_trace_point(
                        points,
                        vertex_indices[(*edge_index + 1) % vertex_indices.len()],
                        visiting,
                    )?;
                    let vector_start = resolve_trace_point(points, *vector_start_index, visiting)?;
                    let vector_end = resolve_trace_point(points, *vector_end_index, visiting)?;
                    let point = lerp_point(&start, &end, *t);
                    Some(PointRecord {
                        x: point.x + (vector_end.x - vector_start.x),
                        y: point.y + (vector_end.y - vector_start.y),
                    })
                }
            }
            ScenePointConstraint::OnCircle {
                center_index,
                radius_index,
                unit_x,
                unit_y,
            } => {
                let center = resolve_trace_point(points, *center_index, visiting)?;
                let radius_point = resolve_trace_point(points, *radius_index, visiting)?;
                let radius = ((radius_point.x - center.x).powi(2)
                    + (radius_point.y - center.y).powi(2))
                .sqrt();
                Some(PointRecord {
                    x: center.x + radius * unit_x,
                    y: center.y + radius * unit_y,
                })
            }
            ScenePointConstraint::OnCircularConstraint {
                circle,
                unit_x,
                unit_y,
            } => {
                let circle = resolve_trace_circular_constraint(points, circle, visiting)?;
                match circle {
                    TraceCircularConstraint::Circle { center, radius } => Some(PointRecord {
                        x: center.x + radius * unit_x,
                        y: center.y + radius * unit_y,
                    }),
                    TraceCircularConstraint::ThreePointArc { .. } => None,
                }
            }
            ScenePointConstraint::OnCircleArc {
                center_index,
                start_index,
                end_index,
                t,
            } => {
                let center = resolve_trace_point(points, *center_index, visiting)?;
                let start = resolve_trace_point(points, *start_index, visiting)?;
                let end = resolve_trace_point(points, *end_index, visiting)?;
                point_on_circle_arc(&center, &start, &end, *t)
            }
            ScenePointConstraint::OnArc {
                start_index,
                mid_index,
                end_index,
                t,
            } => {
                let start = resolve_trace_point(points, *start_index, visiting)?;
                let mid = resolve_trace_point(points, *mid_index, visiting)?;
                let end = resolve_trace_point(points, *end_index, visiting)?;
                point_on_three_point_arc(&start, &mid, &end, *t)
            }
            ScenePointConstraint::OnArcConstraint { arc, t } => {
                let [start, mid, end] = resolve_trace_arc_constraint(points, arc, visiting)?;
                point_on_three_point_arc(&start, &mid, &end, *t)
            }
            ScenePointConstraint::LineIntersection { left, right } => {
                let (left_start, left_end, left_kind) =
                    resolve_trace_line_constraint(points, left, visiting)?;
                let (right_start, right_end, right_kind) =
                    resolve_trace_line_constraint(points, right, visiting)?;
                trace_line_line_intersection(
                    &left_start,
                    &left_end,
                    left_kind,
                    &right_start,
                    &right_end,
                    right_kind,
                )
            }
            ScenePointConstraint::LinePolygonIntersection {
                line,
                vertex_indices,
                variant,
            } => {
                let (line_start, line_end, line_kind) =
                    resolve_trace_line_constraint(points, line, visiting)?;
                let mut polygon = vertex_indices
                    .iter()
                    .map(|index| {
                        resolve_trace_point(points, *index, visiting)
                            .map(|point| to_core_point(&point))
                    })
                    .collect::<Option<Vec<_>>>()?;
                polygon.push(*polygon.first()?);
                gsp_runtime_core::line_polyline_intersection(
                    to_core_point(&line_start),
                    to_core_point(&line_end),
                    trace_core_line_kind(line_kind),
                    &polygon,
                    None,
                    *variant,
                )
                .map(from_core_point)
            }
            ScenePointConstraint::LineTraceIntersection { .. }
            | ScenePointConstraint::LineFunctionIntersection { .. } => None,
            ScenePointConstraint::PointCircularTangent {
                point_index,
                circle,
                variant,
            } => {
                let point = resolve_trace_point(points, *point_index, visiting)?;
                let circle = resolve_trace_circular_constraint(points, circle, visiting)?;
                trace_point_circular_tangent(&point, &circle, *variant)
            }
            ScenePointConstraint::LineCircularIntersection {
                line,
                circle,
                variant,
            } => {
                let (line_start, line_end, line_kind) =
                    resolve_trace_line_constraint(points, line, visiting)?;
                let circle = resolve_trace_circular_constraint(points, circle, visiting)?;
                let (center, radius) = trace_circle_center_radius(&circle);
                if radius <= 1e-9 {
                    return None;
                }
                let radius_point = PointRecord {
                    x: center.x + radius,
                    y: center.y,
                };
                trace_line_circle_intersection(
                    &line_start,
                    &line_end,
                    line_kind,
                    &center,
                    &radius_point,
                    *variant,
                    Some(&point.position),
                )
            }
            ScenePointConstraint::LineCircleIntersection {
                line,
                center_index,
                radius_index,
                variant,
            } => {
                let (line_start, line_end, line_kind) =
                    resolve_trace_line_constraint(points, line, visiting)?;
                let center = resolve_trace_point(points, *center_index, visiting)?;
                let radius_point = resolve_trace_point(points, *radius_index, visiting)?;
                trace_line_circle_intersection(
                    &line_start,
                    &line_end,
                    line_kind,
                    &center,
                    &radius_point,
                    *variant,
                    Some(&point.position),
                )
            }
            ScenePointConstraint::CircleCircleIntersection {
                left_center_index,
                left_radius_index,
                right_center_index,
                right_radius_index,
                variant,
            } => {
                let left_center = resolve_trace_point(points, *left_center_index, visiting)?;
                let left_radius = resolve_trace_point(points, *left_radius_index, visiting)?;
                let right_center = resolve_trace_point(points, *right_center_index, visiting)?;
                let right_radius = resolve_trace_point(points, *right_radius_index, visiting)?;
                trace_circle_circle_intersection(
                    &left_center,
                    &left_radius,
                    &right_center,
                    &right_radius,
                    *variant,
                    Some(&point.position),
                )
            }
            ScenePointConstraint::CircularIntersection {
                left,
                right,
                variant,
            } => {
                let left = resolve_trace_circular_constraint(points, left, visiting)?;
                let right = resolve_trace_circular_constraint(points, right, visiting)?;
                trace_circular_intersection(&left, &right, *variant, Some(&point.position))
            }
        },
    };

    visiting.remove(&index);
    if let Some(resolved_point) = resolved.clone()
        && let Some(point) = points.get_mut(index)
    {
        point.position = resolved_point;
    }
    resolved
}

fn resolve_trace_arc_constraint(
    points: &mut [ScenePoint],
    arc: &ArcConstraint,
    visiting: &mut BTreeSet<usize>,
) -> Option<[PointRecord; 3]> {
    match arc {
        ArcConstraint::CenterArc {
            center_index,
            start_index,
            end_index,
        } => crate::runtime::geometry::arc_on_circle_control_points(
            &resolve_trace_point(points, *center_index, visiting)?,
            &resolve_trace_point(points, *start_index, visiting)?,
            &resolve_trace_point(points, *end_index, visiting)?,
        ),
        ArcConstraint::CircleArc {
            circle,
            start_index,
            end_index,
        } => {
            let circle = resolve_trace_circular_constraint(points, circle, visiting)?;
            let (center, _) = trace_circle_center_radius(&circle);
            crate::runtime::geometry::arc_on_circle_control_points(
                &center,
                &resolve_trace_point(points, *start_index, visiting)?,
                &resolve_trace_point(points, *end_index, visiting)?,
            )
        }
        ArcConstraint::ThreePointArc {
            start_index,
            mid_index,
            end_index,
        } => Some([
            resolve_trace_point(points, *start_index, visiting)?,
            resolve_trace_point(points, *mid_index, visiting)?,
            resolve_trace_point(points, *end_index, visiting)?,
        ]),
        ArcConstraint::Reflected { arc, axis } => {
            let [start, mid, end] = resolve_trace_arc_constraint(points, arc, visiting)?;
            let (axis_start, axis_end, _) = resolve_trace_line_constraint(points, axis, visiting)?;
            Some(
                [start, mid, end]
                    .map(|point| reflect_across_line(&point, &axis_start, &axis_end))
                    .into_iter()
                    .collect::<Option<Vec<_>>>()?
                    .try_into()
                    .ok()?,
            )
        }
    }
}

include!("trace/intersections.rs");
