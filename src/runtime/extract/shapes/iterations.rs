use std::collections::BTreeSet;

use super::{
    CircleShape, GspFile, LineBinding, LineIterationFamily, LineShape, ObjectGroup, PointRecord,
    PolygonIterationFamily, PolygonShape, color_from_style, decode_label_name,
    decode_translated_point_constraint, fill_color_from_styles, find_indexed_path, line_is_dashed,
    regular_polygon_iteration_step, rotate_around, try_decode_parameter_controlled_point,
    try_decode_point_constraint,
};
use crate::runtime::extract::decode::resolve_circle_points_raw;
use crate::runtime::extract::points::editable_non_graph_parameter_name_for_group;
use crate::runtime::functions::{evaluate_function_group_with_overrides, try_decode_function_expr};
use crate::runtime::scene::{CircleIterationFamily, IterationPointHandle};

#[derive(Clone)]
struct AffinePointMap {
    source_origin: PointRecord,
    source_u: PointRecord,
    source_v: PointRecord,
    target_origin: PointRecord,
    target_u: PointRecord,
    target_v: PointRecord,
}

#[derive(Clone)]
struct SegmentSimilarityCoefficients {
    start_alpha: f64,
    start_beta: f64,
    end_alpha: f64,
    end_beta: f64,
}

#[derive(Clone)]
struct RegularPolygonBranchIteration {
    depth: usize,
    parameter_name: Option<String>,
    target_group_pairs: Vec<(usize, usize)>,
}

impl AffinePointMap {
    fn from_triangles(source: &[PointRecord], target: &[PointRecord]) -> Option<Self> {
        if source.len() < 3 || target.len() < 3 {
            return None;
        }
        let source_origin = source[0].clone();
        let source_u = source[1].clone() - source_origin.clone();
        let source_v = source[2].clone() - source_origin.clone();
        let det = source_u.x * source_v.y - source_u.y * source_v.x;
        if det.abs() < 1e-9 {
            return None;
        }
        let target_origin = target[0].clone();
        Some(Self {
            source_origin,
            source_u,
            source_v,
            target_origin: target_origin.clone(),
            target_u: target[1].clone() - target_origin.clone(),
            target_v: target[2].clone() - target_origin,
        })
    }

    fn map_point(&self, point: &PointRecord) -> PointRecord {
        let relative = point.clone() - self.source_origin.clone();
        let det = self.source_u.x * self.source_v.y - self.source_u.y * self.source_v.x;
        let u = (relative.x * self.source_v.y - relative.y * self.source_v.x) / det;
        let v = (self.source_u.x * relative.y - self.source_u.y * relative.x) / det;
        self.target_origin.clone() + self.target_u.clone() * u + self.target_v.clone() * v
    }
}

pub(crate) fn collect_rotational_iteration_lines(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
) -> Vec<LineShape> {
    groups
        .iter()
        .filter(|group| (group.header.kind()) == crate::format::GroupKind::IterationBinding)
        .filter_map(|group| {
            let path = find_indexed_path(file, group)?;
            let source_group = groups.get(path.refs.first()?.checked_sub(1)?)?;
            if (source_group.header.kind()) != crate::format::GroupKind::Segment {
                return None;
            }
            let iter_group = groups.get(path.refs.get(1)?.checked_sub(1)?)?;
            if (iter_group.header.kind()) != crate::format::GroupKind::RegularPolygonIteration {
                return None;
            }
            let (center_group_index, angle_expr, parameter_name, n) =
                regular_polygon_iteration_step(file, groups, iter_group)?;
            let angle_degrees = -360.0 / n;
            let center = anchors.get(center_group_index)?.clone()?;
            let source_path = find_indexed_path(file, source_group)?;
            if source_path.refs.len() != 2 {
                return None;
            }
            let seed_vertex_group = groups.get(path.refs.first()?.checked_sub(1)?)?;
            let seed_vertex_path = find_indexed_path(file, seed_vertex_group)?;
            let vertex_group_index = seed_vertex_path.refs.first()?.checked_sub(1)?;
            let vertex = anchors.get(vertex_group_index)?.clone()?;
            let depth = iter_group
                .records
                .iter()
                .find(|record| record.record_type == 0x090a)
                .map(|record| record.payload(&file.data))
                .filter(|payload| payload.len() >= 20)
                .map(|payload| super::read_u32(payload, 16) as usize)
                .unwrap_or(0);
            let mut lines = Vec::new();
            let rotate = |point: &PointRecord, step: usize| {
                rotate_around(point, &center, (angle_degrees * step as f64).to_radians())
            };
            for step in 0..=depth {
                lines.push(LineShape {
                    points: vec![
                        rotate(&vertex, step),
                        rotate(&vertex, (step + 1) % (depth + 1)),
                    ],
                    color: color_from_style(source_group.header.style_b),
                    dashed: line_is_dashed(source_group.header.style_a),
                    visible: !iter_group.header.is_hidden(),
                    binding: Some(LineBinding::RotateEdge {
                        center_index: center_group_index,
                        vertex_index: vertex_group_index,
                        parameter_name: parameter_name.clone(),
                        angle_expr: angle_expr.clone(),
                        start_step: step,
                        end_step: (step + 1) % (depth + 1),
                    }),
                });
            }
            Some(lines)
        })
        .flatten()
        .collect()
}

pub(crate) fn collect_rotational_iteration_segment_groups(
    file: &GspFile,
    groups: &[ObjectGroup],
) -> BTreeSet<usize> {
    groups
        .iter()
        .filter(|group| (group.header.kind()) == crate::format::GroupKind::IterationBinding)
        .filter_map(|group| {
            let path = find_indexed_path(file, group)?;
            let source_group_index = path.refs.first()?.checked_sub(1)?;
            let source_group = groups.get(source_group_index)?;
            if (source_group.header.kind()) != crate::format::GroupKind::Segment {
                return None;
            }
            let iter_group = groups.get(path.refs.get(1)?.checked_sub(1)?)?;
            ((iter_group.header.kind()) == crate::format::GroupKind::RegularPolygonIteration
                && regular_polygon_iteration_step(file, groups, iter_group).is_some())
            .then_some(source_group_index)
        })
        .collect()
}

pub(crate) fn collect_carried_iteration_lines(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
    suppressed_source_groups: &BTreeSet<usize>,
) -> Vec<LineShape> {
    groups
        .iter()
        .filter(|group| (group.header.kind()) == crate::format::GroupKind::IterationBinding)
        .filter_map(|group| {
            let path = find_indexed_path(file, group)?;
            let source_group_index = path.refs.first()?.checked_sub(1)?;
            if suppressed_source_groups.contains(&source_group_index) {
                return None;
            }
            let source_group = groups.get(source_group_index)?;
            if (source_group.header.kind()) != crate::format::GroupKind::Segment {
                return None;
            }
            let iter_group = groups.get(path.refs.get(1)?.checked_sub(1)?)?;
            if !iter_group.header.kind().is_carried_iteration() {
                return None;
            }
            if (iter_group.header.kind()) == crate::format::GroupKind::RegularPolygonIteration
                && regular_polygon_iteration_step(file, groups, iter_group).is_some()
            {
                return None;
            }
            let source_path = find_indexed_path(file, source_group)?;
            if source_path.refs.len() != 2 {
                return None;
            }
            let start = anchors.get(source_path.refs[0].checked_sub(1)?)?.clone()?;
            let end = anchors.get(source_path.refs[1].checked_sub(1)?)?.clone()?;
            let depth = carried_iteration_depth(file, iter_group, 3);
            if depth == 0 {
                return None;
            }
            if let Some(branching) =
                regular_polygon_branch_iteration(file, groups, iter_group, source_group)
            {
                let coeffs = branching
                    .target_group_pairs
                    .iter()
                    .map(|(start_group_index, end_group_index)| {
                        let target_start = anchors.get(*start_group_index)?.clone()?;
                        let target_end = anchors.get(*end_group_index)?.clone()?;
                        similarity_coefficients(&start, &end, &target_start, &target_end)
                    })
                    .collect::<Option<Vec<_>>>()?;
                return Some(branching_lines(
                    &start,
                    &end,
                    &coeffs,
                    branching.depth,
                    color_from_style(source_group.header.style_b),
                    line_is_dashed(source_group.header.style_a),
                    !iter_group.header.is_hidden(),
                ));
            }
            if let Some(point_map) = carried_iteration_point_map(file, groups, iter_group, anchors)
            {
                let color = color_from_style(source_group.header.style_b);
                let mut current_start = start.clone();
                let mut current_end = end.clone();
                let mut lines = Vec::with_capacity(depth);
                for _ in 0..depth {
                    current_start = point_map.map_point(&current_start);
                    current_end = point_map.map_point(&current_end);
                    lines.push(LineShape {
                        points: vec![current_start.clone(), current_end.clone()],
                        color,
                        dashed: line_is_dashed(source_group.header.style_a),
                        visible: !iter_group.header.is_hidden(),
                        binding: None,
                    });
                }
                return Some(lines);
            }
            let steps = carried_iteration_steps(file, groups, iter_group, anchors);
            let Some((step, secondary_step, bidirectional)) = carried_iteration_basis(&steps)
            else {
                return None;
            };
            let color = color_from_style(source_group.header.style_b);
            Some(
                carried_iteration_line_deltas(&step, secondary_step.as_ref(), depth, bidirectional)
                    .into_iter()
                    .map(|delta| LineShape {
                        points: vec![start.clone() + delta.clone(), end.clone() + delta],
                        color,
                        dashed: line_is_dashed(source_group.header.style_a),
                        visible: !iter_group.header.is_hidden(),
                        binding: None,
                    })
                    .collect::<Vec<_>>(),
            )
        })
        .flatten()
        .collect()
}

pub(crate) fn collect_carried_line_iteration_families(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
    group_to_point_index: &[Option<usize>],
    line_group_to_index: &[Option<usize>],
    suppressed_source_groups: &BTreeSet<usize>,
) -> Vec<LineIterationFamily> {
    groups
        .iter()
        .filter(|group| (group.header.kind()) == crate::format::GroupKind::IterationBinding)
        .filter_map(|group| {
            let path = find_indexed_path(file, group)?;
            let source_group_index = path.refs.first()?.checked_sub(1)?;
            if suppressed_source_groups.contains(&source_group_index) {
                return None;
            }
            let source_group = groups.get(source_group_index)?;
            let iter_group = groups.get(path.refs.get(1)?.checked_sub(1)?)?;
            if (source_group.header.kind()) == crate::format::GroupKind::PointTrace {
                if (iter_group.header.kind()) != crate::format::GroupKind::RegularPolygonIteration {
                    return None;
                }
                let iter_path = find_indexed_path(file, iter_group)?;
                if iter_path.refs.last().copied() != Some(source_group.ordinal) {
                    return None;
                }
                let source_path = find_indexed_path(file, source_group)?;
                let target_group_index = source_path.refs.first()?.checked_sub(1)?;
                let trace_point_index = group_to_point_index
                    .get(target_group_index)
                    .copied()
                    .flatten()?;
                let driver_group_index = source_path
                    .refs
                    .iter()
                    .filter_map(|ordinal| groups.get(ordinal.checked_sub(1)?))
                    .find(|group| (group.header.kind()) == crate::format::GroupKind::ParameterAnchor)
                    .and_then(|group| find_indexed_path(file, group))
                    .and_then(|path| path.refs.first().copied())
                    .and_then(|ordinal| ordinal.checked_sub(1))
                    .or_else(|| {
                        source_path.refs.iter().copied().find_map(|ordinal| {
                            let group_index = ordinal.checked_sub(1)?;
                            matches!(
                                groups.get(group_index)?.header.kind(),
                                crate::format::GroupKind::PointConstraint
                                    | crate::format::GroupKind::PathPoint
                            )
                            .then_some(group_index)
                        })
                    })?;
                let trace_driver_index = group_to_point_index
                    .get(driver_group_index)
                    .copied()
                    .flatten()?;
                let descriptor = source_group
                    .records
                    .iter()
                    .find(|record| record.record_type == 0x0902)
                    .map(|record| record.payload(&file.data))
                    .and_then(|payload| {
                        crate::runtime::functions::try_decode_function_plot_descriptor(payload).ok()
                    })?;
                let trace_parameter_group = groups.get(iter_path.refs.get(1)?.checked_sub(1)?)?;
                let trace_parameter_name =
                    editable_non_graph_parameter_name_for_group(file, groups, trace_parameter_group)?;
                let trace_step_group = groups.get(iter_path.refs.get(2)?.checked_sub(1)?)?;
                let trace_step_expr = try_decode_function_expr(file, groups, trace_step_group).ok()?;
                let depth = carried_iteration_depth(file, iter_group, 3);
                if depth == 0 {
                    return None;
                }
                return Some(LineIterationFamily {
                    start_index: trace_point_index,
                    end_index: trace_driver_index,
                    dx: 0.0,
                    dy: 0.0,
                    secondary_dx: None,
                    secondary_dy: None,
                    depth,
                    parameter_name: carried_iteration_parameter_name(file, groups, iter_group),
                    bidirectional: false,
                    color: color_from_style(source_group.header.style_b),
                    dashed: line_is_dashed(source_group.header.style_a),
                    affine_source_indices: None,
                    affine_target_handles: None,
                    branch_target_segments: None,
                    trace_point_index: Some(trace_point_index),
                    trace_driver_index: Some(trace_driver_index),
                    trace_parameter_name: Some(trace_parameter_name),
                    trace_step_expr: Some(trace_step_expr),
                    trace_x_min: Some(descriptor.x_min),
                    trace_x_max: Some(descriptor.x_max),
                    trace_sample_count: Some(descriptor.sample_count),
                });
            }
            if (source_group.header.kind()) != crate::format::GroupKind::Segment {
                return None;
            }
            if !iter_group.header.kind().is_carried_iteration() {
                return None;
            }
            if (iter_group.header.kind()) == crate::format::GroupKind::RegularPolygonIteration
                && regular_polygon_iteration_step(file, groups, iter_group).is_some()
            {
                return None;
            }

            let source_path = find_indexed_path(file, source_group)?;
            if source_path.refs.len() != 2 {
                return None;
            }
            let start_index = group_to_point_index
                .get(source_path.refs[0].checked_sub(1)?)
                .copied()
                .flatten()?;
            let end_index = group_to_point_index
                .get(source_path.refs[1].checked_sub(1)?)
                .copied()
                .flatten()?;
            let depth = carried_iteration_depth(file, iter_group, 3);
            if depth == 0 {
                return None;
            }

            if let Some(branching) =
                regular_polygon_branch_iteration(file, groups, iter_group, source_group)
            {
                let target_segments = branching
                    .target_group_pairs
                    .iter()
                    .map(|(start_group_index, end_group_index)| {
                        let start_index = group_to_point_index
                            .get(*start_group_index)
                            .copied()
                            .flatten()?;
                        let end_index = group_to_point_index
                            .get(*end_group_index)
                            .copied()
                            .flatten()?;
                        Some([
                            IterationPointHandle::Point {
                                point_index: start_index,
                            },
                            IterationPointHandle::Point {
                                point_index: end_index,
                            },
                        ])
                    })
                    .collect::<Option<Vec<_>>>()?;
                return Some(LineIterationFamily {
                    start_index,
                    end_index,
                    dx: 0.0,
                    dy: 0.0,
                    secondary_dx: None,
                    secondary_dy: None,
                    depth: branching.depth,
                    parameter_name: branching.parameter_name,
                    bidirectional: false,
                    color: color_from_style(source_group.header.style_b),
                    dashed: line_is_dashed(source_group.header.style_a),
                    affine_source_indices: None,
                    affine_target_handles: None,
                    branch_target_segments: Some(target_segments),
                    trace_point_index: None,
                    trace_driver_index: None,
                    trace_parameter_name: None,
                    trace_step_expr: None,
                    trace_x_min: None,
                    trace_x_max: None,
                    trace_sample_count: None,
                });
            }

            if let Some((source_indices, target_handles)) = carried_iteration_affine_handles(
                file,
                groups,
                iter_group,
                group_to_point_index,
                line_group_to_index,
                anchors,
            ) {
                return Some(LineIterationFamily {
                    start_index,
                    end_index,
                    dx: 0.0,
                    dy: 0.0,
                    secondary_dx: None,
                    secondary_dy: None,
                    depth,
                    parameter_name: None,
                    bidirectional: false,
                    color: color_from_style(source_group.header.style_b),
                    dashed: line_is_dashed(source_group.header.style_a),
                    affine_source_indices: Some(source_indices),
                    affine_target_handles: Some(target_handles),
                    branch_target_segments: None,
                    trace_point_index: None,
                    trace_driver_index: None,
                    trace_parameter_name: None,
                    trace_step_expr: None,
                    trace_x_min: None,
                    trace_x_max: None,
                    trace_sample_count: None,
                });
            }

            let steps = carried_iteration_steps(file, groups, iter_group, anchors);
            let Some((step, secondary_step, bidirectional)) = carried_iteration_basis(&steps)
            else {
                return None;
            };
            Some(LineIterationFamily {
                start_index,
                end_index,
                dx: step.x,
                dy: step.y,
                secondary_dx: secondary_step.as_ref().map(|step| step.x),
                secondary_dy: secondary_step.as_ref().map(|step| step.y),
                depth,
                parameter_name: carried_iteration_parameter_name(file, groups, iter_group),
                bidirectional,
                color: color_from_style(source_group.header.style_b),
                dashed: line_is_dashed(source_group.header.style_a),
                affine_source_indices: None,
                affine_target_handles: None,
                branch_target_segments: None,
                trace_point_index: None,
                trace_driver_index: None,
                trace_parameter_name: None,
                trace_step_expr: None,
                trace_x_min: None,
                trace_x_max: None,
                trace_sample_count: None,
            })
        })
        .collect()
}

fn regular_polygon_branch_iteration(
    file: &GspFile,
    groups: &[ObjectGroup],
    iter_group: &ObjectGroup,
    source_group: &ObjectGroup,
) -> Option<RegularPolygonBranchIteration> {
    if (iter_group.header.kind()) != crate::format::GroupKind::RegularPolygonIteration
        || regular_polygon_iteration_step(file, groups, iter_group).is_some()
    {
        return None;
    }

    let iter_path = find_indexed_path(file, iter_group)?;
    if iter_path.refs.len() < 7 {
        return None;
    }
    let source_path = find_indexed_path(file, source_group)?;
    if source_path.refs.len() != 2
        || source_path.refs[0] != iter_path.refs[1]
        || source_path.refs[1] != iter_path.refs[2]
    {
        return None;
    }

    let parameter_group = groups.get(iter_path.refs[0].checked_sub(1)?)?;
    let parameter_name = editable_non_graph_parameter_name_for_group(file, groups, parameter_group);
    let target_group_pairs = vec![
        (
            iter_path.refs[3].checked_sub(1)?,
            iter_path.refs[4].checked_sub(1)?,
        ),
        (
            iter_path.refs[5].checked_sub(1)?,
            iter_path.refs[6].checked_sub(1)?,
        ),
    ];
    let depth = carried_iteration_depth(file, iter_group, 3);
    (depth > 0).then_some(RegularPolygonBranchIteration {
        depth,
        parameter_name,
        target_group_pairs,
    })
}

fn similarity_coefficients(
    source_start: &PointRecord,
    source_end: &PointRecord,
    target_start: &PointRecord,
    target_end: &PointRecord,
) -> Option<SegmentSimilarityCoefficients> {
    let source_dx = source_end.x - source_start.x;
    let source_dy = source_end.y - source_start.y;
    let source_len_sq = source_dx * source_dx + source_dy * source_dy;
    if source_len_sq <= 1e-9 {
        return None;
    }

    let point_coefficients = |point: &PointRecord| {
        let relative_x = point.x - source_start.x;
        let relative_y = point.y - source_start.y;
        (
            (relative_x * source_dx + relative_y * source_dy) / source_len_sq,
            (relative_x * -source_dy + relative_y * source_dx) / source_len_sq,
        )
    };

    let (start_alpha, start_beta) = point_coefficients(target_start);
    let (end_alpha, end_beta) = point_coefficients(target_end);
    Some(SegmentSimilarityCoefficients {
        start_alpha,
        start_beta,
        end_alpha,
        end_beta,
    })
}

fn apply_similarity_coefficients(
    segment_start: &PointRecord,
    segment_end: &PointRecord,
    alpha: f64,
    beta: f64,
) -> PointRecord {
    let dx = segment_end.x - segment_start.x;
    let dy = segment_end.y - segment_start.y;
    PointRecord {
        x: segment_start.x + alpha * dx - beta * dy,
        y: segment_start.y + alpha * dy + beta * dx,
    }
}

fn branching_lines(
    seed_start: &PointRecord,
    seed_end: &PointRecord,
    coeffs: &[SegmentSimilarityCoefficients],
    depth: usize,
    color: [u8; 4],
    dashed: bool,
    visible: bool,
) -> Vec<LineShape> {
    let mut frontier = vec![(seed_start.clone(), seed_end.clone())];
    let mut lines = Vec::new();
    for _ in 0..depth {
        let mut next = Vec::new();
        for (current_start, current_end) in frontier {
            for coeff in coeffs {
                let child_start = apply_similarity_coefficients(
                    &current_start,
                    &current_end,
                    coeff.start_alpha,
                    coeff.start_beta,
                );
                let child_end = apply_similarity_coefficients(
                    &current_start,
                    &current_end,
                    coeff.end_alpha,
                    coeff.end_beta,
                );
                lines.push(LineShape {
                    points: vec![child_start.clone(), child_end.clone()],
                    color,
                    dashed,
                    visible,
                    binding: None,
                });
                next.push((child_start, child_end));
            }
        }
        frontier = next;
    }
    lines
}

pub(crate) fn collect_carried_polygon_edge_segment_groups(
    file: &GspFile,
    groups: &[ObjectGroup],
) -> BTreeSet<usize> {
    let carried_polygon_edges = groups
        .iter()
        .filter(|group| (group.header.kind()) == crate::format::GroupKind::IterationBinding)
        .filter_map(|group| {
            let path = find_indexed_path(file, group)?;
            let source_group = groups.get(path.refs.first()?.checked_sub(1)?)?;
            if (source_group.header.kind()) != crate::format::GroupKind::Polygon {
                return None;
            }
            let iter_group = groups.get(path.refs.get(1)?.checked_sub(1)?)?;
            if !matches!(
                iter_group.header.kind(),
                crate::format::GroupKind::AffineIteration
                    | crate::format::GroupKind::RegularPolygonIteration
            ) {
                return None;
            }
            if (iter_group.header.kind()) == crate::format::GroupKind::RegularPolygonIteration
                && regular_polygon_iteration_step(file, groups, iter_group).is_some()
            {
                return None;
            }
            let source_path = find_indexed_path(file, source_group)?;
            (source_path.refs.len() >= 3).then_some(source_path.refs)
        })
        .flat_map(|refs| {
            refs.iter()
                .copied()
                .zip(refs.iter().copied().cycle().skip(1))
                .take(refs.len())
                .map(|(start, end)| normalize_segment_refs(start, end))
                .collect::<Vec<_>>()
        })
        .collect::<BTreeSet<_>>();

    groups
        .iter()
        .enumerate()
        .filter_map(|(group_index, group)| {
            if (group.header.kind()) != crate::format::GroupKind::Segment {
                return None;
            }
            let path = find_indexed_path(file, group)?;
            if path.refs.len() != 2 {
                return None;
            }
            carried_polygon_edges
                .contains(&normalize_segment_refs(path.refs[0], path.refs[1]))
                .then_some(group_index)
        })
        .collect()
}

fn normalize_segment_refs(left: usize, right: usize) -> (usize, usize) {
    if left <= right {
        (left, right)
    } else {
        (right, left)
    }
}

fn carried_iteration_steps(
    file: &GspFile,
    groups: &[ObjectGroup],
    iter_group: &ObjectGroup,
    anchors: &[Option<PointRecord>],
) -> Vec<PointRecord> {
    let Some(iter_path) = find_indexed_path(file, iter_group) else {
        return Vec::new();
    };
    let translated_steps = iter_path
        .refs
        .iter()
        .filter_map(|ordinal| {
            let index = ordinal.checked_sub(1)?;
            groups.get(index)
        })
        .filter_map(|group| iteration_step_for_group(file, group, anchors))
        .fold(Vec::<PointRecord>::new(), |mut acc, step| {
            let already_present = acc
                .iter()
                .any(|existing| same_iteration_step(existing, &step));
            if !already_present {
                acc.push(step);
            }
            acc
        });
    if !translated_steps.is_empty() {
        return translated_steps;
    }
    if iter_path.refs.len() < 2 {
        return Vec::new();
    }
    let Some(base_start_index) = iter_path.refs[0].checked_sub(1) else {
        return Vec::new();
    };
    let Some(base_end_index) = iter_path.refs[1].checked_sub(1) else {
        return Vec::new();
    };
    let Some(base_start) = anchors.get(base_start_index).cloned().flatten() else {
        return Vec::new();
    };
    let Some(base_end) = anchors.get(base_end_index).cloned().flatten() else {
        return Vec::new();
    };
    vec![PointRecord {
        x: base_end.x - base_start.x,
        y: base_end.y - base_start.y,
    }]
}

fn carried_iteration_basis(
    steps: &[PointRecord],
) -> Option<(PointRecord, Option<PointRecord>, bool)> {
    let mut axes = Vec::new();
    let mut used = vec![false; steps.len()];
    for (index, step) in steps.iter().enumerate() {
        if used[index] {
            continue;
        }
        if let Some(opposite_index) =
            steps
                .iter()
                .enumerate()
                .find_map(|(candidate_index, candidate)| {
                    (!used[candidate_index]
                        && candidate_index != index
                        && same_iteration_step(
                            candidate,
                            &PointRecord {
                                x: -step.x,
                                y: -step.y,
                            },
                        ))
                    .then_some(candidate_index)
                })
        {
            used[index] = true;
            used[opposite_index] = true;
            axes.push(step.clone());
        }
    }
    if used.iter().all(|flag| *flag) && !axes.is_empty() && axes.len() <= 2 {
        return Some((axes[0].clone(), axes.get(1).cloned(), true));
    }
    Some((steps.first()?.clone(), steps.get(1).cloned(), false))
}

fn iteration_step_for_group(
    file: &GspFile,
    group: &ObjectGroup,
    anchors: &[Option<PointRecord>],
) -> Option<PointRecord> {
    if (group.header.kind()) == crate::format::GroupKind::Translation {
        let path = find_indexed_path(file, group)?;
        let start = anchors.get(path.refs.get(1)?.checked_sub(1)?)?.clone()?;
        let end = anchors.get(path.refs.get(2)?.checked_sub(1)?)?.clone()?;
        let step = end - start;
        return (!is_zero_step(&step)).then_some(step);
    }

    let constraint = decode_translated_point_constraint(file, group)?;
    let step = PointRecord {
        x: constraint.dx,
        y: constraint.dy,
    };
    (!is_zero_step(&step)).then_some(step)
}

fn is_zero_step(step: &PointRecord) -> bool {
    step.x.abs() < 1e-6 && step.y.abs() < 1e-6
}

fn same_iteration_step(left: &PointRecord, right: &PointRecord) -> bool {
    (left.x - right.x).abs() < 1e-6 && (left.y - right.y).abs() < 1e-6
}

fn carried_iteration_line_deltas(
    step: &PointRecord,
    secondary_step: Option<&PointRecord>,
    depth: usize,
    bidirectional: bool,
) -> Vec<PointRecord> {
    if bidirectional {
        if let Some(secondary) = secondary_step {
            let mut deltas = Vec::new();
            for primary_index in -(depth as isize)..=(depth as isize) {
                for secondary_index in -(depth as isize)..=(depth as isize) {
                    if primary_index == 0 && secondary_index == 0 {
                        continue;
                    }
                    if primary_index.unsigned_abs() + secondary_index.unsigned_abs() > depth {
                        continue;
                    }
                    deltas.push(PointRecord {
                        x: step.x * primary_index as f64 + secondary.x * secondary_index as f64,
                        y: step.y * primary_index as f64 + secondary.y * secondary_index as f64,
                    });
                }
            }
            return deltas;
        }
        return (1..=depth)
            .flat_map(|index| {
                [
                    PointRecord {
                        x: step.x * index as f64,
                        y: step.y * index as f64,
                    },
                    PointRecord {
                        x: -step.x * index as f64,
                        y: -step.y * index as f64,
                    },
                ]
            })
            .collect();
    }
    if let Some(secondary) = secondary_step {
        let mut deltas = Vec::new();
        for primary_index in 0..=depth {
            for secondary_index in 0..=depth - primary_index {
                if primary_index == 0 && secondary_index == 0 {
                    continue;
                }
                deltas.push(PointRecord {
                    x: step.x * primary_index as f64 + secondary.x * secondary_index as f64,
                    y: step.y * primary_index as f64 + secondary.y * secondary_index as f64,
                });
            }
        }
        return deltas;
    }

    (1..=depth)
        .map(|index| PointRecord {
            x: step.x * index as f64,
            y: step.y * index as f64,
        })
        .collect()
}

fn carried_iteration_polygon_deltas(
    step: &PointRecord,
    secondary_step: Option<&PointRecord>,
    depth: usize,
    bidirectional: bool,
) -> Vec<PointRecord> {
    if bidirectional {
        if let Some(secondary) = secondary_step {
            let mut deltas = Vec::new();
            for primary_index in -(depth as isize)..=(depth as isize) {
                for secondary_index in -(depth as isize)..=(depth as isize) {
                    if primary_index.unsigned_abs() + secondary_index.unsigned_abs() > depth {
                        continue;
                    }
                    deltas.push(PointRecord {
                        x: step.x * primary_index as f64 + secondary.x * secondary_index as f64,
                        y: step.y * primary_index as f64 + secondary.y * secondary_index as f64,
                    });
                }
            }
            return deltas;
        }
        return std::iter::once(PointRecord { x: 0.0, y: 0.0 })
            .chain((1..=depth).flat_map(|index| {
                [
                    PointRecord {
                        x: step.x * index as f64,
                        y: step.y * index as f64,
                    },
                    PointRecord {
                        x: -step.x * index as f64,
                        y: -step.y * index as f64,
                    },
                ]
            }))
            .collect();
    }
    if let Some(secondary) = secondary_step {
        let mut deltas = Vec::new();
        for primary_index in 0..=depth {
            for secondary_index in 0..=depth - primary_index {
                deltas.push(PointRecord {
                    x: step.x * primary_index as f64 + secondary.x * secondary_index as f64,
                    y: step.y * primary_index as f64 + secondary.y * secondary_index as f64,
                });
            }
        }
        return deltas;
    }

    (0..=depth)
        .map(|index| PointRecord {
            x: step.x * index as f64,
            y: step.y * index as f64,
        })
        .collect()
}

fn carried_iteration_depth(
    file: &GspFile,
    iter_group: &ObjectGroup,
    default_depth: usize,
) -> usize {
    iter_group
        .records
        .iter()
        .find(|record| record.record_type == 0x090a)
        .map(|record| record.payload(&file.data))
        .filter(|payload| payload.len() >= 20)
        .map(|payload| super::read_u32(payload, 16) as usize)
        .unwrap_or(default_depth)
}

fn carried_iteration_parameter_name(
    file: &GspFile,
    groups: &[ObjectGroup],
    iter_group: &ObjectGroup,
) -> Option<String> {
    let iter_path = find_indexed_path(file, iter_group)?;
    let parameter_group = groups.get(iter_path.refs.first()?.checked_sub(1)?)?;
    editable_non_graph_parameter_name_for_group(file, groups, parameter_group)
}

fn carried_iteration_point_map(
    file: &GspFile,
    groups: &[ObjectGroup],
    iter_group: &ObjectGroup,
    anchors: &[Option<PointRecord>],
) -> Option<AffinePointMap> {
    if (iter_group.header.kind()) != crate::format::GroupKind::AffineIteration {
        return None;
    }

    let iter_path = find_indexed_path(file, iter_group)?;
    if iter_path.refs.len() < 6 {
        return None;
    }

    let source_indices = iter_path
        .refs
        .iter()
        .take(3)
        .map(|ordinal| ordinal.checked_sub(1))
        .collect::<Option<Vec<_>>>()?;
    if !source_indices.iter().all(|index| {
        groups
            .get(*index)
            .is_some_and(|group| (group.header.kind()) == crate::format::GroupKind::Point)
    }) {
        return None;
    }

    let image_indices = iter_path
        .refs
        .iter()
        .skip(3)
        .take(3)
        .map(|ordinal| ordinal.checked_sub(1))
        .collect::<Option<Vec<_>>>()?;
    let source = source_indices
        .iter()
        .map(|index| anchors.get(*index)?.clone())
        .collect::<Option<Vec<_>>>()?;
    let target = image_indices
        .iter()
        .map(|index| anchors.get(*index)?.clone())
        .collect::<Option<Vec<_>>>()?;
    AffinePointMap::from_triangles(&source, &target)
}

fn carried_iteration_affine_handles(
    file: &GspFile,
    groups: &[ObjectGroup],
    iter_group: &ObjectGroup,
    group_to_point_index: &[Option<usize>],
    line_group_to_index: &[Option<usize>],
    anchors: &[Option<PointRecord>],
) -> Option<([usize; 3], [IterationPointHandle; 3])> {
    let iter_path = find_indexed_path(file, iter_group)?;
    if (iter_group.header.kind()) != crate::format::GroupKind::AffineIteration
        || iter_path.refs.len() < 6
    {
        return None;
    }

    let source_group_indices = [
        iter_path.refs[0].checked_sub(1)?,
        iter_path.refs[1].checked_sub(1)?,
        iter_path.refs[2].checked_sub(1)?,
    ];
    let source_indices = [
        group_to_point_index
            .get(source_group_indices[0])
            .copied()
            .flatten()?,
        group_to_point_index
            .get(source_group_indices[1])
            .copied()
            .flatten()?,
        group_to_point_index
            .get(source_group_indices[2])
            .copied()
            .flatten()?,
    ];

    let mut target_handles = Vec::with_capacity(3);
    for ordinal in iter_path.refs.iter().skip(3).take(3) {
        let group_index = ordinal.checked_sub(1)?;
        if let Some(point_index) = group_to_point_index.get(group_index).copied().flatten() {
            target_handles.push(IterationPointHandle::Point { point_index });
            continue;
        }
        let group = groups.get(group_index)?;
        if (group.header.kind()) == crate::format::GroupKind::Midpoint {
            let midpoint_path = find_indexed_path(file, group)?;
            let host_group_index = midpoint_path.refs.first()?.checked_sub(1)?;
            let line_index = line_group_to_index
                .get(host_group_index)
                .copied()
                .flatten()?;
            target_handles.push(IterationPointHandle::LinePoint {
                line_index,
                segment_index: 0,
                t: 0.5,
            });
            continue;
        }
        let fixed = anchors.get(group_index).cloned().flatten()?;
        target_handles.push(IterationPointHandle::Fixed(fixed));
    }
    let target_handles: [IterationPointHandle; 3] = target_handles.try_into().ok()?;
    Some((source_indices, target_handles))
}

pub(crate) fn collect_carried_iteration_polygons(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
) -> Vec<PolygonShape> {
    groups
        .iter()
        .filter(|group| (group.header.kind()) == crate::format::GroupKind::IterationBinding)
        .filter_map(|group| {
            let path = find_indexed_path(file, group)?;
            let source_group = groups.get(path.refs.first()?.checked_sub(1)?)?;
            if let Some(polygons) = collect_coordinate_point_polygon_grid_iteration(
                file,
                groups,
                source_group,
                path.refs.get(1).copied()?,
                anchors,
            ) {
                return Some(polygons);
            }
            if (source_group.header.kind()) != crate::format::GroupKind::Polygon {
                return None;
            }
            let iter_group = groups.get(path.refs.get(1)?.checked_sub(1)?)?;
            if !matches!(
                iter_group.header.kind(),
                crate::format::GroupKind::AffineIteration
                    | crate::format::GroupKind::RegularPolygonIteration
            ) {
                return None;
            }
            if (iter_group.header.kind()) == crate::format::GroupKind::RegularPolygonIteration
                && regular_polygon_iteration_step(file, groups, iter_group).is_some()
            {
                return None;
            }
            let source_path = find_indexed_path(file, source_group)?;
            if source_path.refs.len() < 3 {
                return None;
            }
            let points = source_path
                .refs
                .iter()
                .map(|ordinal| anchors.get(ordinal.checked_sub(1)?).cloned().flatten())
                .collect::<Option<Vec<_>>>()?;
            let steps = carried_iteration_steps(file, groups, iter_group, anchors);
            let Some((step, secondary_step, bidirectional)) = carried_iteration_basis(&steps)
            else {
                return None;
            };
            let depth = iter_group
                .records
                .iter()
                .find(|record| record.record_type == 0x090a)
                .map(|record| record.payload(&file.data))
                .filter(|payload| payload.len() >= 20)
                .map(|payload| super::read_u32(payload, 16) as usize)
                .unwrap_or(3);
            let color =
                fill_color_from_styles(source_group.header.style_a, source_group.header.style_b);
            Some(
                carried_iteration_polygon_deltas(
                    &step,
                    secondary_step.as_ref(),
                    depth,
                    bidirectional,
                )
                .into_iter()
                .map(|delta| PolygonShape {
                    points: points
                        .iter()
                        .cloned()
                        .map(|point| point + delta.clone())
                        .collect(),
                    color,
                    visible: !iter_group.header.is_hidden(),
                    binding: None,
                })
                .collect::<Vec<_>>(),
            )
        })
        .flatten()
        .collect()
}

fn collect_coordinate_point_polygon_grid_iteration(
    file: &GspFile,
    groups: &[ObjectGroup],
    source_group: &ObjectGroup,
    iter_group_ordinal: usize,
    anchors: &[Option<PointRecord>],
) -> Option<Vec<PolygonShape>> {
    if !source_group.header.kind().is_coordinate_object() {
        return None;
    }

    let source_path = find_indexed_path(file, source_group)?;
    if source_path.refs.len() < 3 {
        return None;
    }
    let polygon_group = groups.get(source_path.refs.first()?.checked_sub(1)?)?;
    if (polygon_group.header.kind()) != crate::format::GroupKind::Polygon {
        return None;
    }

    let polygon_path = find_indexed_path(file, polygon_group)?;
    if polygon_path.refs.len() != 4 {
        return None;
    }
    let seed_polygon = polygon_path
        .refs
        .iter()
        .map(|ordinal| anchors.get(ordinal.checked_sub(1)?).cloned().flatten())
        .collect::<Option<Vec<_>>>()?;

    let iter_group = groups.get(iter_group_ordinal.checked_sub(1)?)?;
    if (iter_group.header.kind()) != crate::format::GroupKind::RegularPolygonIteration {
        return None;
    }
    let iter_path = find_indexed_path(file, iter_group)?;
    if iter_path.refs.last().copied() != Some(source_group.ordinal) {
        return None;
    }

    let payload_depth = carried_iteration_depth(file, iter_group, 0);
    if payload_depth == 0 {
        return Some(Vec::new());
    }

    let parameter_group = groups.get(iter_path.refs.get(1)?.checked_sub(1)?)?;
    let Some(parameter_name) = decode_label_name(file, parameter_group) else {
        return None;
    };
    let mut parameter_value =
        crate::runtime::extract::try_decode_parameter_control_value_for_group(
            file,
            groups,
            parameter_group,
        )
        .ok()
        .unwrap_or(0.0);
    let parameter_step_group = iter_path
        .refs
        .get(2)
        .and_then(|ordinal| groups.get(ordinal.checked_sub(1)?));
    let base_anchor = evaluate_coordinate_iteration_anchor(
        file,
        groups,
        source_group,
        anchors,
        &parameter_name,
        parameter_value,
    );
    let Some(base_anchor) = base_anchor else {
        return None;
    };
    let color = fill_color_from_styles(polygon_group.header.style_b, polygon_group.header.style_c);
    let mut polygons: Vec<PolygonShape> = Vec::new();
    for _step in 1..=payload_depth {
        if let Some(step_group) = parameter_step_group {
            let overrides =
                std::collections::BTreeMap::from([(parameter_name.clone(), parameter_value)]);
            parameter_value =
                evaluate_function_group_with_overrides(file, groups, step_group, &overrides)
                    .unwrap_or(parameter_value + 1.0);
        } else {
            parameter_value += 1.0;
        }
        let iter_anchor = evaluate_coordinate_iteration_anchor(
            file,
            groups,
            source_group,
            anchors,
            &parameter_name,
            parameter_value,
        )?;
        let delta = iter_anchor - base_anchor.clone();
        let already_present = polygons.iter().any(|polygon| {
            polygon.points.first().is_some_and(|point| {
                let seed = &seed_polygon[0];
                (point.x - (seed.x + delta.x)).abs() < 1e-6
                    && (point.y - (seed.y + delta.y)).abs() < 1e-6
            })
        });
        if already_present {
            continue;
        }
        polygons.push(PolygonShape {
            points: seed_polygon
                .iter()
                .cloned()
                .map(|point| point + delta.clone())
                .collect(),
            color,
            visible: !iter_group.header.is_hidden(),
            binding: None,
        });
    }
    Some(polygons)
}

fn evaluate_coordinate_iteration_anchor(
    file: &GspFile,
    groups: &[ObjectGroup],
    coordinate_group: &ObjectGroup,
    anchors: &[Option<PointRecord>],
    parameter_name: &str,
    parameter_value: f64,
) -> Option<PointRecord> {
    let path = find_indexed_path(file, coordinate_group)?;
    let source_group_index = path.refs.first()?.checked_sub(1)?;
    let source_position = anchors
        .get(source_group_index)
        .cloned()
        .flatten()
        .or_else(|| {
            let source_group = groups.get(source_group_index)?;
            ((source_group.header.kind()) == crate::format::GroupKind::Polygon)
                .then(|| find_indexed_path(file, source_group))
                .flatten()
                .and_then(|polygon_path| anchors.get(polygon_path.refs.first()?.checked_sub(1)?))
                .cloned()
                .flatten()
        });
    let Some(source_position) = source_position else {
        return None;
    };
    match coordinate_group.header.kind() {
        crate::format::GroupKind::Unknown(20) => {
            let x_calc_group = groups.get(path.refs.get(1)?.checked_sub(1)?)?;
            let y_calc_group = groups.get(path.refs.get(2)?.checked_sub(1)?)?;
            let parameters =
                std::collections::BTreeMap::from([(parameter_name.to_string(), parameter_value)]);
            let dx =
                evaluate_function_group_with_overrides(file, groups, x_calc_group, &parameters)?
                    * function_group_raw_scale(file, groups, x_calc_group, &mut BTreeSet::new());
            let dy =
                evaluate_function_group_with_overrides(file, groups, y_calc_group, &parameters)?
                    * function_group_raw_scale(file, groups, y_calc_group, &mut BTreeSet::new());
            Some(PointRecord {
                x: source_position.x + dx,
                y: source_position.y - dy,
            })
        }
        _ => None,
    }
}

fn function_group_raw_scale(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
    visiting: &mut BTreeSet<usize>,
) -> f64 {
    if !visiting.insert(group.ordinal) {
        return 1.0;
    }
    let scale = match group.header.kind() {
        crate::format::GroupKind::MeasuredValue => 37.79527559055118,
        crate::format::GroupKind::FunctionExpr => find_indexed_path(file, group)
            .map(|path| {
                path.refs
                    .iter()
                    .filter_map(|ordinal| groups.get(ordinal.checked_sub(1)?))
                    .map(|ref_group| function_group_raw_scale(file, groups, ref_group, visiting))
                    .fold(1.0, f64::max)
            })
            .unwrap_or(1.0),
        _ => 1.0,
    };
    visiting.remove(&group.ordinal);
    scale
}

pub(crate) fn collect_carried_iteration_circles(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
) -> Vec<CircleShape> {
    groups
        .iter()
        .filter(|group| (group.header.kind()) == crate::format::GroupKind::IterationBinding)
        .filter_map(|group| {
            let path = find_indexed_path(file, group)?;
            let source_group = groups.get(path.refs.first()?.checked_sub(1)?)?;
            if !super::is_circle_group_kind(source_group.header.kind()) {
                return None;
            }
            let iter_group = groups.get(path.refs.get(1)?.checked_sub(1)?)?;
            if !matches!(
                iter_group.header.kind(),
                crate::format::GroupKind::AffineIteration
                    | crate::format::GroupKind::RegularPolygonIteration
            ) {
                return None;
            }
            if let Some(circles) = collect_parameter_controlled_circle_iteration(
                file,
                groups,
                source_group,
                iter_group,
                anchors,
            ) {
                return Some(circles);
            }
            if (iter_group.header.kind()) == crate::format::GroupKind::RegularPolygonIteration
                && regular_polygon_iteration_step(file, groups, iter_group).is_some()
            {
                return None;
            }
            let (center, radius_point): (PointRecord, PointRecord) =
                resolve_circle_points_raw(file, groups, anchors, source_group)?;
            let depth = carried_iteration_depth(file, iter_group, 3);
            if depth == 0 {
                return None;
            }
            let visible = !iter_group.header.is_hidden() && !source_group.header.is_hidden();
            if let Some(point_map) = carried_iteration_point_map(file, groups, iter_group, anchors)
            {
                let mut current_center = center.clone();
                let mut current_radius = radius_point.clone();
                let mut circles = Vec::with_capacity(depth);
                for _ in 0..depth {
                    current_center = point_map.map_point(&current_center);
                    current_radius = point_map.map_point(&current_radius);
                    circles.push(CircleShape {
                        center: current_center.clone(),
                        radius_point: current_radius.clone(),
                        color: color_from_style(source_group.header.style_b),
                        fill_color: None,
                        fill_color_binding: None,
                        dashed: line_is_dashed(source_group.header.style_a),
                        visible,
                        binding: None,
                    });
                }
                return Some(circles);
            }
            let steps = carried_iteration_steps(file, groups, iter_group, anchors);
            let Some((step, secondary_step, bidirectional)) = carried_iteration_basis(&steps)
            else {
                return None;
            };
            Some(
                carried_iteration_polygon_deltas(
                    &step,
                    secondary_step.as_ref(),
                    depth,
                    bidirectional,
                )
                .into_iter()
                .filter(|delta| !is_zero_step(delta))
                .map(|delta| CircleShape {
                    center: center.clone() + delta.clone(),
                    radius_point: radius_point.clone() + delta,
                    color: color_from_style(source_group.header.style_b),
                    fill_color: None,
                    fill_color_binding: None,
                    dashed: line_is_dashed(source_group.header.style_a),
                    visible,
                    binding: None,
                })
                .collect::<Vec<_>>(),
            )
        })
        .flatten()
        .collect()
}

pub(crate) fn collect_carried_circle_iteration_families(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
    group_to_point_index: &[Option<usize>],
    circle_group_to_index: &[Option<usize>],
) -> Vec<CircleIterationFamily> {
    groups
        .iter()
        .filter(|group| (group.header.kind()) == crate::format::GroupKind::IterationBinding)
        .filter_map(|group| {
            let path = find_indexed_path(file, group)?;
            let source_circle_group_index = path.refs.first()?.checked_sub(1)?;
            let source_circle_group = groups.get(source_circle_group_index)?;
            if !super::is_circle_group_kind(source_circle_group.header.kind()) {
                return None;
            }
            let iter_group = groups.get(path.refs.get(1)?.checked_sub(1)?)?;
            let source_circle_index = circle_group_to_index
                .get(source_circle_group_index)
                .copied()
                .flatten()?;
            build_parameter_controlled_circle_iteration_family(
                file,
                groups,
                source_circle_group,
                source_circle_index,
                iter_group,
                anchors,
                group_to_point_index,
            )
        })
        .collect()
}

fn build_parameter_controlled_circle_iteration_family(
    file: &GspFile,
    groups: &[ObjectGroup],
    source_circle_group: &ObjectGroup,
    source_circle_index: usize,
    iter_group: &ObjectGroup,
    anchors: &[Option<PointRecord>],
    group_to_point_index: &[Option<usize>],
) -> Option<CircleIterationFamily> {
    if (iter_group.header.kind()) != crate::format::GroupKind::RegularPolygonIteration {
        return None;
    }
    let source_circle_path = find_indexed_path(file, source_circle_group)?;
    let source_center_group_index = source_circle_path.refs.first()?.checked_sub(1)?;
    let source_center_index = group_to_point_index
        .get(source_center_group_index)
        .copied()
        .flatten()?;
    let source_center_group = groups.get(source_center_group_index)?;
    let crate::runtime::extract::points::RawPointConstraint::PolygonBoundary {
        vertex_group_indices,
        edge_index,
        t,
    } = try_decode_point_constraint(file, groups, source_center_group, Some(anchors), &None)
        .ok()?
    else {
        return None;
    };
    let seed_parameter = super::super::labels::polygon_boundary_parameter(
        anchors,
        &vertex_group_indices,
        edge_index,
        t,
    )?;

    let iter_point_group = groups
        .iter()
        .filter(|group| (group.header.kind()) == crate::format::GroupKind::IterationBinding)
        .find_map(|group| {
            let path = find_indexed_path(file, group)?;
            let target_iter_group = groups.get(path.refs.get(1)?.checked_sub(1)?)?;
            if target_iter_group.ordinal != iter_group.ordinal {
                return None;
            }
            let candidate = groups.get(path.refs.first()?.checked_sub(1)?)?;
            ((candidate.header.kind()) == crate::format::GroupKind::ParameterControlledPoint)
                .then_some(candidate)
        })?;
    let iter_point =
        try_decode_parameter_controlled_point(file, groups, iter_point_group, anchors).ok()?;
    let iter_point_group_index = iter_point_group.ordinal.checked_sub(1)?;
    let source_next_center_index = group_to_point_index
        .get(iter_point_group_index)
        .copied()
        .flatten()?;
    let crate::runtime::extract::points::RawPointConstraint::PolygonBoundary {
        edge_index, t, ..
    } = iter_point.constraint
    else {
        return None;
    };
    let next_parameter = super::super::labels::polygon_boundary_parameter(
        anchors,
        &vertex_group_indices,
        edge_index,
        t,
    )?;
    let step_parameter = (next_parameter - seed_parameter).rem_euclid(1.0);
    if step_parameter <= 1e-9 {
        return None;
    }

    let vertex_indices = vertex_group_indices
        .iter()
        .map(|group_index| group_to_point_index.get(*group_index).copied().flatten())
        .collect::<Option<Vec<_>>>()?;
    let depth = carried_iteration_depth(file, iter_group, 3);
    if depth == 0 {
        return None;
    }

    Some(CircleIterationFamily {
        source_circle_index,
        source_center_index,
        source_next_center_index,
        vertex_indices,
        seed_parameter,
        step_parameter,
        depth,
        depth_parameter_name: regular_polygon_iteration_step(file, groups, iter_group)
            .map(|(_, _, parameter_name, _)| parameter_name)
            .or_else(|| {
                let path = find_indexed_path(file, iter_group)?;
                let parameter_group = groups.get(path.refs.first()?.checked_sub(1)?)?;
                editable_non_graph_parameter_name_for_group(file, groups, parameter_group)
            }),
        visible: !iter_group.header.is_hidden() && !source_circle_group.header.is_hidden(),
    })
}

fn collect_parameter_controlled_circle_iteration(
    file: &GspFile,
    groups: &[ObjectGroup],
    source_circle_group: &ObjectGroup,
    iter_group: &ObjectGroup,
    anchors: &[Option<PointRecord>],
) -> Option<Vec<CircleShape>> {
    if (iter_group.header.kind()) != crate::format::GroupKind::RegularPolygonIteration {
        return None;
    }
    let source_circle_path = find_indexed_path(file, source_circle_group)?;
    let source_center_group_index = source_circle_path.refs.first()?.checked_sub(1)?;
    let source_center_group = groups.get(source_center_group_index)?;
    let crate::runtime::extract::points::RawPointConstraint::PolygonBoundary {
        vertex_group_indices,
        edge_index,
        t,
    } = try_decode_point_constraint(file, groups, source_center_group, Some(anchors), &None)
        .ok()?
    else {
        return None;
    };
    let seed_parameter = super::super::labels::polygon_boundary_parameter(
        anchors,
        &vertex_group_indices,
        edge_index,
        t,
    )?;

    let iter_point_group = groups
        .iter()
        .filter(|group| (group.header.kind()) == crate::format::GroupKind::IterationBinding)
        .find_map(|group| {
            let path = find_indexed_path(file, group)?;
            let target_iter_group = groups.get(path.refs.get(1)?.checked_sub(1)?)?;
            if target_iter_group.ordinal != iter_group.ordinal {
                return None;
            }
            let candidate = groups.get(path.refs.first()?.checked_sub(1)?)?;
            ((candidate.header.kind()) == crate::format::GroupKind::ParameterControlledPoint)
                .then_some(candidate)
        })?;
    let iter_point =
        try_decode_parameter_controlled_point(file, groups, iter_point_group, anchors).ok()?;
    let crate::runtime::extract::points::RawPointConstraint::PolygonBoundary {
        edge_index, t, ..
    } = iter_point.constraint
    else {
        return None;
    };
    let next_parameter = super::super::labels::polygon_boundary_parameter(
        anchors,
        &vertex_group_indices,
        edge_index,
        t,
    )?;
    let step_parameter = (next_parameter - seed_parameter).rem_euclid(1.0);
    if step_parameter <= 1e-9 {
        return None;
    }

    let vertices = vertex_group_indices
        .iter()
        .map(|group_index| anchors.get(*group_index)?.clone())
        .collect::<Option<Vec<_>>>()?;
    let (source_center, source_radius_point): (PointRecord, PointRecord) =
        resolve_circle_points_raw(file, groups, anchors, source_circle_group)?;
    let depth = carried_iteration_depth(file, iter_group, 3);
    if depth == 0 {
        return None;
    }
    let visible = !iter_group.header.is_hidden() && !source_circle_group.header.is_hidden();

    Some(
        (1..=depth)
            .filter_map(|index| {
                let parameter = (seed_parameter + step_parameter * index as f64).rem_euclid(1.0);
                let (edge_index, local_t) = polygon_parameter_to_edge_local(&vertices, parameter)?;
                let center = resolve_polygon_boundary_point_local(&vertices, edge_index, local_t)?;
                let delta = center.clone() - source_center.clone();
                Some(CircleShape {
                    center,
                    radius_point: source_radius_point.clone() + delta,
                    color: color_from_style(source_circle_group.header.style_b),
                    fill_color: None,
                    fill_color_binding: None,
                    dashed: line_is_dashed(source_circle_group.header.style_a),
                    visible,
                    binding: None,
                })
            })
            .collect::<Vec<_>>(),
    )
}

fn polygon_parameter_to_edge_local(
    vertices: &[PointRecord],
    parameter: f64,
) -> Option<(usize, f64)> {
    if vertices.len() < 2 {
        return None;
    }
    let wrapped = parameter.rem_euclid(1.0);
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
    let target = wrapped * perimeter;
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

fn resolve_polygon_boundary_point_local(
    vertices: &[PointRecord],
    edge_index: usize,
    t: f64,
) -> Option<PointRecord> {
    if vertices.len() < 2 {
        return None;
    }
    let start = vertices.get(edge_index % vertices.len())?;
    let end = vertices.get((edge_index + 1) % vertices.len())?;
    Some(PointRecord {
        x: start.x + (end.x - start.x) * t,
        y: start.y + (end.y - start.y) * t,
    })
}

pub(crate) fn collect_carried_polygon_iteration_families(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
    group_to_point_index: &[Option<usize>],
) -> Vec<PolygonIterationFamily> {
    groups
        .iter()
        .filter(|group| (group.header.kind()) == crate::format::GroupKind::IterationBinding)
        .filter_map(|group| {
            let path = find_indexed_path(file, group)?;
            let source_group = groups.get(path.refs.first()?.checked_sub(1)?)?;
            let iter_group = groups.get(path.refs.get(1)?.checked_sub(1)?)?;
            if let Some(family) = build_coordinate_point_polygon_grid_iteration_family(
                file,
                groups,
                source_group,
                iter_group,
                group_to_point_index,
            ) {
                return Some(family);
            }
            if (source_group.header.kind()) != crate::format::GroupKind::Polygon {
                return None;
            }
            if !matches!(
                iter_group.header.kind(),
                crate::format::GroupKind::AffineIteration
                    | crate::format::GroupKind::RegularPolygonIteration
            ) {
                return None;
            }
            if (iter_group.header.kind()) == crate::format::GroupKind::RegularPolygonIteration
                && regular_polygon_iteration_step(file, groups, iter_group).is_some()
            {
                return None;
            }
            let source_path = find_indexed_path(file, source_group)?;
            if source_path.refs.len() < 3 {
                return None;
            }
            let vertex_indices = source_path
                .refs
                .iter()
                .map(|ordinal| {
                    group_to_point_index
                        .get(ordinal.checked_sub(1)?)
                        .copied()
                        .flatten()
                })
                .collect::<Option<Vec<_>>>()?;
            let steps = carried_iteration_steps(file, groups, iter_group, anchors);
            let Some((step, secondary_step, bidirectional)) = carried_iteration_basis(&steps)
            else {
                return None;
            };
            let depth = carried_iteration_depth(file, iter_group, 3);
            if depth == 0 {
                return None;
            }
            Some(PolygonIterationFamily::Translate {
                vertex_indices,
                dx: step.x,
                dy: step.y,
                secondary_dx: secondary_step.as_ref().map(|step| step.x),
                secondary_dy: secondary_step.as_ref().map(|step| step.y),
                depth,
                parameter_name: carried_iteration_parameter_name(file, groups, iter_group),
                bidirectional,
                color: fill_color_from_styles(
                    source_group.header.style_a,
                    source_group.header.style_b,
                ),
            })
        })
        .collect()
}

fn build_coordinate_point_polygon_grid_iteration_family(
    file: &GspFile,
    groups: &[ObjectGroup],
    source_group: &ObjectGroup,
    iter_group: &ObjectGroup,
    group_to_point_index: &[Option<usize>],
) -> Option<PolygonIterationFamily> {
    if !source_group.header.kind().is_coordinate_object() {
        return None;
    }
    if (iter_group.header.kind()) != crate::format::GroupKind::RegularPolygonIteration {
        return None;
    }

    let source_path = find_indexed_path(file, source_group)?;
    if source_path.refs.len() < 3 {
        return None;
    }
    let polygon_group_index = source_path.refs.first()?.checked_sub(1)?;
    let polygon_group = groups.get(polygon_group_index)?;
    if (polygon_group.header.kind()) != crate::format::GroupKind::Polygon {
        return None;
    }
    let polygon_path = find_indexed_path(file, polygon_group)?;
    if polygon_path.refs.len() != 4 {
        return None;
    }
    let vertex_indices = polygon_path
        .refs
        .iter()
        .map(|ordinal| {
            group_to_point_index
                .get(ordinal.checked_sub(1)?)
                .copied()
                .flatten()
        })
        .collect::<Option<Vec<_>>>()?;

    let iter_path = find_indexed_path(file, iter_group)?;
    if iter_path.refs.last().copied() != Some(source_group.ordinal) {
        return None;
    }

    let depth = carried_iteration_depth(file, iter_group, 0);
    if depth == 0 {
        return None;
    }

    let depth_expr = groups
        .get(iter_path.refs.first()?.checked_sub(1)?)
        .filter(|group| (group.header.kind()) == crate::format::GroupKind::FunctionExpr)
        .and_then(|group| try_decode_function_expr(file, groups, group).ok());
    let parameter_group = groups.get(iter_path.refs.get(1)?.checked_sub(1)?)?;
    let parameter_name = decode_label_name(file, parameter_group)?;
    let step_group = groups.get(iter_path.refs.get(2)?.checked_sub(1)?)?;
    let step_expr = try_decode_function_expr(file, groups, step_group).ok()?;
    let x_calc_group = groups.get(source_path.refs.get(1)?.checked_sub(1)?)?;
    let y_calc_group = groups.get(source_path.refs.get(2)?.checked_sub(1)?)?;
    let x_expr = try_decode_function_expr(file, groups, x_calc_group).ok()?;
    let y_expr = try_decode_function_expr(file, groups, y_calc_group).ok()?;

    Some(PolygonIterationFamily::CoordinateGrid {
        vertex_indices,
        parameter_name,
        step_expr,
        x_expr,
        y_expr,
        x_raw_scale: function_group_raw_scale(file, groups, x_calc_group, &mut BTreeSet::new()),
        y_raw_scale: function_group_raw_scale(file, groups, y_calc_group, &mut BTreeSet::new()),
        depth,
        depth_expr,
        color: fill_color_from_styles(polygon_group.header.style_b, polygon_group.header.style_c),
    })
}

pub(crate) fn collect_iteration_shapes(
    file: &GspFile,
    groups: &[ObjectGroup],
    circles: &[CircleShape],
) -> (Vec<LineShape>, Vec<PolygonShape>) {
    let mut lines = Vec::new();
    let polygons = Vec::new();

    let has_iteration = groups
        .iter()
        .any(|group| (group.header.kind()) == crate::format::GroupKind::RegularPolygonIteration);
    if !has_iteration {
        return (lines, polygons);
    }

    for iter_group in groups
        .iter()
        .filter(|group| (group.header.kind()) == crate::format::GroupKind::RegularPolygonIteration)
    {
        let Some(iter_path) = find_indexed_path(file, iter_group) else {
            continue;
        };

        let iter_data = iter_group
            .records
            .iter()
            .find(|record| record.record_type == 0x090a)
            .map(|record| record.payload(&file.data));

        let depth = iter_data
            .filter(|payload| payload.len() >= 20)
            .map(|payload| super::read_u32(payload, 16) as usize)
            .unwrap_or(0);
        if depth == 0 {
            continue;
        }

        let polygon_group_index = iter_path.refs.iter().find_map(|&obj_ref| {
            let index = obj_ref.checked_sub(1)?;
            let group = groups.get(index)?;
            ((group.header.kind()) == crate::format::GroupKind::Polygon).then_some(index)
        });

        let Some(polygon_index) = polygon_group_index else {
            continue;
        };
        let polygon_group = &groups[polygon_index];
        let Some(polygon_path) = find_indexed_path(file, polygon_group) else {
            continue;
        };
        if polygon_path.refs.len() < 3 {
            continue;
        }

        let Some(circle) = circles.first() else {
            continue;
        };
        let cx = circle.center.x;
        let cy = circle.center.y;
        let radius =
            ((circle.radius_point.x - cx).powi(2) + (circle.radius_point.y - cy).powi(2)).sqrt();
        if radius < 1.0 {
            continue;
        }

        let px_per_cm = groups
            .iter()
            .filter(|group| (group.header.kind()) == crate::format::GroupKind::PolarOffsetPoint)
            .find_map(|group| {
                let payload = group
                    .records
                    .iter()
                    .find(|record| record.record_type == 0x07d3)
                    .map(|record| record.payload(&file.data))?;
                (payload.len() >= 40).then(|| super::read_f64(payload, 32))
            })
            .filter(|v| v.is_finite() && *v > 1.0)
            .unwrap_or(37.79527559055118);

        let param_value = groups
            .iter()
            .filter(|group| (group.header.kind()) == crate::format::GroupKind::PolarOffsetPoint)
            .find_map(|group| {
                let payload = group
                    .records
                    .iter()
                    .find(|record| record.record_type == 0x07d3)
                    .map(|record| record.payload(&file.data))?;
                (payload.len() >= 20).then(|| super::read_f64(payload, 12))
            })
            .filter(|v| v.is_finite() && *v > 0.0)
            .unwrap_or(1.0);

        let side = param_value * px_per_cm / 2.0;
        if side < 1.0 {
            continue;
        }

        let outline_color = [30, 30, 30, 255];
        let sqrt3 = 3.0_f64.sqrt();
        let col_spacing = sqrt3 * side;
        let row_spacing = 1.5 * side;
        let max_cols = (radius / col_spacing).ceil() as i32 + 2;
        let max_rows = (radius / row_spacing).ceil() as i32 + 2;

        let hex_vertices = |hx: f64, hy: f64| -> Vec<PointRecord> {
            (0..6)
                .map(|i| {
                    let angle =
                        std::f64::consts::FRAC_PI_3 * i as f64 + std::f64::consts::FRAC_PI_6;
                    PointRecord {
                        x: hx + side * angle.cos(),
                        y: hy + side * angle.sin(),
                    }
                })
                .collect()
        };

        for row in -max_rows..=max_rows {
            let y = cy + row as f64 * row_spacing;
            let x_offset = if row.rem_euclid(2) == 1 {
                col_spacing / 2.0
            } else {
                0.0
            };
            for col in -max_cols..=max_cols {
                let x = cx + col as f64 * col_spacing + x_offset;
                let dist = ((x - cx).powi(2) + (y - cy).powi(2)).sqrt();
                if dist > radius + side * 0.5 {
                    continue;
                }
                let verts = hex_vertices(x, y);

                let mut outline = verts.clone();
                outline.push(verts[0].clone());
                lines.push(LineShape {
                    points: outline,
                    color: outline_color,
                    dashed: false,
                    visible: !iter_group.header.is_hidden(),
                    binding: None,
                });
            }
        }
    }

    (lines, polygons)
}
