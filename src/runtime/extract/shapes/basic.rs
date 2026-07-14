use std::collections::{BTreeMap, BTreeSet};

use super::{
    ArcShape, CircleShape, GraphTransform, GspFile, LineBinding, LineShape, ObjectGroup,
    PointRecord, PolygonShape, ShapeBinding, color_from_style, decode_label_name,
    evaluate_expr_with_parameters, fill_color_from_styles, find_indexed_path, has_distinct_points,
    line_is_dashed, line_stroke_width_from_style, payload_debug_source, three_point_arc_geometry,
    to_raw_from_world, try_decode_function_expr, try_decode_function_plot_descriptor,
};
use crate::format::{GroupKind, read_f64, read_u16, read_u32};
use crate::runtime::DEFAULT_GRAPH_RAW_PER_UNIT;
use crate::runtime::extract::decode::{
    circle_center_radius_value, detect_perpendicular_segment_payload, is_circle_group_kind,
    measured_radius_segment_group_indices, resolve_circle_points_raw,
};
use crate::runtime::extract::points::{is_non_graph_parameter_group, resolve_line_like_points_raw};
use crate::runtime::geometry::{
    arc_on_circle_control_points, from_core_point, sample_three_point_arc,
    sample_three_point_arc_complement, to_core_point,
};
use crate::runtime::scene::{ArcBinding, ArcBoundaryKind};

const ARC_BOUNDARY_SUBDIVISIONS: usize = 48;

pub(crate) fn collect_circle_fill_colors(
    file: &GspFile,
    groups: &[ObjectGroup],
    _anchors: &[Option<PointRecord>],
) -> BTreeMap<usize, ([u8; 4], bool)> {
    groups
        .iter()
        .filter(|group| (group.header.kind()) == crate::format::GroupKind::CircleInterior)
        .filter_map(|group| {
            let path = find_indexed_path(file, group)?;
            let circle_group_index = path.refs.first()?.checked_sub(1)?;
            Some((
                circle_group_index,
                (
                    fill_color_from_styles(group.header.style_b, group.header.style_c),
                    !group.header.is_hidden(),
                ),
            ))
        })
        .collect()
}

pub(crate) fn collect_line_shapes(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
    kinds: &[crate::format::GroupKind],
    suppressed_group_indices: &BTreeSet<usize>,
) -> Vec<LineShape> {
    groups
        .iter()
        .enumerate()
        .filter(|group| {
            let (group_index, group) = group;
            if suppressed_group_indices.contains(group_index) {
                return false;
            }
            let kind = group.header.kind();
            kinds.contains(&kind)
        })
        .filter(|(_, group)| !is_auxiliary_segment_group(file, groups, group))
        .filter_map(|(_, group)| {
            match group.header.kind() {
                crate::format::GroupKind::PerpendicularLine => {
                    return resolve_perpendicular_line_shape(file, groups, anchors, group);
                }
                crate::format::GroupKind::ParallelLine => {
                    return resolve_parallel_line_shape(file, groups, anchors, group);
                }
                crate::format::GroupKind::AngleBisectorRay => {
                    return resolve_angle_bisector_ray_shape(file, anchors, group);
                }
                crate::format::GroupKind::AngleMarker => {
                    return resolve_angle_marker_shape(file, anchors, group);
                }
                _ => {}
            }
            let path = find_indexed_path(file, group)?;
            let points = path
                .refs
                .iter()
                .filter_map(|object_ref| {
                    anchors.get(object_ref.saturating_sub(1)).cloned().flatten()
                })
                .collect::<Vec<_>>();
            let start_group_index = if let Some(ordinal) = path.refs.first() {
                ordinal.checked_sub(1)
            } else {
                None
            };
            let end_group_index = if let Some(ordinal) = path.refs.get(1) {
                ordinal.checked_sub(1)
            } else {
                None
            };
            let binding = match (group.header.kind(), start_group_index, end_group_index) {
                (crate::format::GroupKind::Segment, Some(start_index), Some(end_index)) => {
                    Some(LineBinding::Segment {
                        start_index,
                        end_index,
                    })
                }
                (crate::format::GroupKind::Line, Some(start_index), Some(end_index)) => {
                    Some(LineBinding::Line {
                        start_index,
                        end_index,
                    })
                }
                (crate::format::GroupKind::Ray, Some(start_index), Some(end_index)) => {
                    Some(LineBinding::Ray {
                        start_index,
                        end_index,
                    })
                }
                (
                    crate::format::GroupKind::MeasurementLine | crate::format::GroupKind::AxisLine,
                    Some(start_index),
                    Some(end_index),
                ) => Some(LineBinding::GraphHelperLine {
                    start_index,
                    end_index,
                }),
                _ => None,
            };
            let has_distinct_refs = path.refs.first() != path.refs.get(1);
            (points.len() >= 2
                && (has_distinct_points(&points) || (binding.is_some() && has_distinct_refs)))
                .then_some(LineShape {
                    points,
                    color: color_from_style(group.header.style_b),
                    dashed: (group.header.kind()) == crate::format::GroupKind::MeasurementLine
                        || line_is_dashed(group.header.style_a),
                    stroke_width: Some(line_stroke_width_from_style(group.header.style_a)),
                    visible: !group.header.is_hidden(),
                    binding,
                    debug: Some(payload_debug_source(group)),
                })
        })
        .collect()
}

pub(crate) fn collect_constructed_line_shapes(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
) -> Vec<LineShape> {
    groups
        .iter()
        .filter_map(|group| match group.header.kind() {
            crate::format::GroupKind::PerpendicularLine => {
                resolve_perpendicular_line_shape(file, groups, anchors, group)
            }
            crate::format::GroupKind::ParallelLine => {
                resolve_parallel_line_shape(file, groups, anchors, group)
            }
            crate::format::GroupKind::AngleBisectorRay => {
                resolve_angle_bisector_ray_shape(file, anchors, group)
            }
            _ => None,
        })
        .collect()
}

fn is_auxiliary_segment_group(file: &GspFile, groups: &[ObjectGroup], group: &ObjectGroup) -> bool {
    if (group.header.kind()) != crate::format::GroupKind::Segment {
        return false;
    }
    if is_perpendicular_segment_helper_group(file, groups, group) {
        return true;
    }
    if segment_references_non_graph_parameter(file, groups, group) {
        return true;
    }
    if groups.iter().any(|candidate| {
        if (candidate.header.kind()) != crate::format::GroupKind::IterationBinding {
            return false;
        }
        let Some(path) = find_indexed_path(file, candidate) else {
            return false;
        };
        let Some(source_ordinal) = path.refs.first().copied() else {
            return false;
        };
        if source_ordinal != group.ordinal {
            return false;
        }
        let Some(iter_group) = path
            .refs
            .get(1)
            .and_then(|ordinal| groups.get(ordinal.checked_sub(1)?))
        else {
            return false;
        };
        (iter_group.header.kind()) == crate::format::GroupKind::RegularPolygonIteration
            && crate::runtime::extract::points::regular_polygon_iteration_step(
                file, groups, iter_group,
            )
            .is_some()
    }) {
        return false;
    }
    if !group.header.is_hidden() {
        return false;
    }
    let Some(path) = find_indexed_path(file, group) else {
        return false;
    };
    path.refs.iter().any(|ordinal| {
        let Some(index) = ordinal.checked_sub(1) else {
            return false;
        };
        let Some(referenced_group) = groups.get(index) else {
            return false;
        };
        match referenced_group.header.kind() {
            crate::format::GroupKind::ParameterRotation
            | crate::format::GroupKind::FunctionExpr => true,
            crate::format::GroupKind::Point => {
                referenced_group.records.iter().any(|record| {
                    record.record_type
                        == crate::runtime::payload_consts::RECORD_FUNCTION_EXPR_PAYLOAD
                }) && !referenced_group.records.iter().any(|record| {
                    record.record_type == crate::runtime::payload_consts::RECORD_POINT_F64_PAIR
                })
            }
            _ => false,
        }
    })
}

fn segment_references_non_graph_parameter(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
) -> bool {
    let Some(path) = find_indexed_path(file, group) else {
        return false;
    };
    if path.refs.len() != 2 {
        return false;
    }
    path.refs.iter().any(|ordinal| {
        let Some(index) = ordinal.checked_sub(1) else {
            return false;
        };
        groups
            .get(index)
            .is_some_and(|referenced| is_non_graph_parameter_group(file, groups, referenced))
    })
}

fn is_perpendicular_segment_helper_group(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
) -> bool {
    let Some(group_index) = group.ordinal.checked_sub(1) else {
        return false;
    };
    let Some(path) = find_indexed_path(file, group) else {
        return false;
    };
    if path.refs.len() != 2 {
        return false;
    }

    path.refs.iter().any(|object_ref| {
        let Some(foot_group_index) = object_ref.checked_sub(1) else {
            return false;
        };
        detect_perpendicular_segment_payload(file, groups, foot_group_index)
            .map(|payload| payload.helper_segment_group_index == group_index)
            .unwrap_or(false)
    })
}

pub(crate) fn collect_segment_marker_shapes(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
) -> Vec<LineShape> {
    groups
        .iter()
        .filter(|group| (group.header.kind()) == crate::format::GroupKind::SegmentMarker)
        .filter_map(|group| {
            let path = find_indexed_path(file, group)?;
            let host_group_index = path.refs.first()?.checked_sub(1)?;
            let (start_group_index, end_group_index) =
                resolve_segment_marker_endpoint_groups(file, groups, host_group_index)?;
            let start = anchors.get(start_group_index)?.clone()?;
            let end = anchors.get(end_group_index)?.clone()?;
            let (t, marker_class) = decode_segment_marker_payload(file, group)?;
            let points = resolve_segment_marker_points(&start, &end, t, marker_class)?;
            Some(LineShape {
                points,
                color: color_from_style(group.header.style_b),
                dashed: line_is_dashed(group.header.style_a),
                stroke_width: Some(line_stroke_width_from_style(group.header.style_a)),
                visible: !group.header.is_hidden(),
                binding: Some(LineBinding::SegmentMarker {
                    start_index: start_group_index,
                    end_index: end_group_index,
                    t,
                    marker_class,
                }),
                debug: Some(payload_debug_source(group)),
            })
        })
        .collect()
}

pub(crate) fn collect_arc_boundary_shapes(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
) -> Vec<LineShape> {
    groups
        .iter()
        .filter(|group| {
            matches!(
                group.header.kind(),
                crate::format::GroupKind::SectorBoundary
                    | crate::format::GroupKind::CircularSegmentBoundary
            )
        })
        .filter_map(|group| {
            let binding = resolve_arc_boundary_binding(file, groups, group)?;
            let points = resolve_arc_boundary_points(file, groups, anchors, group)
                .or_else(|| resolve_boundary_arc_seed_points(file, groups, anchors, group))?;
            Some(LineShape {
                points,
                color: color_from_style(group.header.style_b),
                dashed: line_is_dashed(group.header.style_a),
                stroke_width: Some(line_stroke_width_from_style(group.header.style_a)),
                visible: !group.header.is_hidden(),
                binding: Some(binding),
                debug: Some(payload_debug_source(group)),
            })
        })
        .collect()
}

pub(crate) fn collect_arc_boundary_fill_polygons(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
) -> Vec<PolygonShape> {
    groups
        .iter()
        .filter(|group| {
            matches!(
                group.header.kind(),
                crate::format::GroupKind::SectorBoundary
                    | crate::format::GroupKind::CircularSegmentBoundary
            )
        })
        .filter_map(|group| {
            let binding = resolve_arc_boundary_polygon_binding(file, groups, group)?;
            let points = resolve_arc_boundary_points(file, groups, anchors, group)
                .or_else(|| resolve_boundary_arc_seed_points(file, groups, anchors, group))?;
            let color = fill_color_from_styles(group.header.style_b, group.header.style_c);
            (color[3] > 0).then_some(PolygonShape {
                points,
                color,
                color_binding: None,
                visible: !group.header.is_hidden(),
                binding: Some(binding),
                debug: Some(payload_debug_source(group)),
            })
        })
        .collect()
}

fn resolve_angle_marker_shape(
    file: &GspFile,
    anchors: &[Option<PointRecord>],
    group: &ObjectGroup,
) -> Option<LineShape> {
    let path = find_indexed_path(file, group)?;
    if path.refs.len() != 3 {
        return None;
    }

    let start = anchors.get(path.refs[0].checked_sub(1)?)?.clone()?;
    let vertex = anchors.get(path.refs[1].checked_sub(1)?)?.clone()?;
    let end = anchors.get(path.refs[2].checked_sub(1)?)?.clone()?;

    let marker_class = decode_angle_marker_class(file, group).max(1);
    let points = gsp_runtime_core::angle_marker_points(
        to_core_point(&start),
        to_core_point(&vertex),
        to_core_point(&end),
        marker_class,
    )?
    .into_iter()
    .map(from_core_point)
    .collect::<Vec<_>>();

    has_distinct_points(&points).then_some(LineShape {
        points,
        color: color_from_style(group.header.style_b),
        dashed: line_is_dashed(group.header.style_a),
        stroke_width: Some(line_stroke_width_from_style(group.header.style_a)),
        visible: !group.header.is_hidden(),
        binding: Some(LineBinding::AngleMarker {
            start_index: path.refs[0].checked_sub(1)?,
            vertex_index: path.refs[1].checked_sub(1)?,
            end_index: path.refs[2].checked_sub(1)?,
            marker_class,
        }),
        debug: Some(payload_debug_source(group)),
    })
}

fn decode_angle_marker_class(file: &GspFile, group: &ObjectGroup) -> u32 {
    group
        .records
        .iter()
        .find(|record| {
            record.record_type == crate::runtime::payload_consts::RECORD_ANGLE_MARKER_CLASS
        })
        .map(|record| record.payload(&file.data))
        .filter(|payload| payload.len() >= 2)
        .map(|payload| u32::from(read_u16(payload, 0)))
        .unwrap_or(1)
}

fn decode_segment_marker_payload(file: &GspFile, group: &ObjectGroup) -> Option<(f64, u32)> {
    let payload = group
        .records
        .iter()
        .find(|record| {
            record.record_type == crate::runtime::payload_consts::RECORD_SEGMENT_MARKER_PAYLOAD
        })
        .map(|record| record.payload(&file.data))?;
    (payload.len() >= 12).then(|| (read_f64(payload, 0), read_u32(payload, 8)))
}

pub(crate) fn resolve_segment_marker_points(
    start: &PointRecord,
    end: &PointRecord,
    t: f64,
    marker_class: u32,
) -> Option<Vec<PointRecord>> {
    gsp_runtime_core::segment_marker_points(
        to_core_point(start),
        to_core_point(end),
        t,
        marker_class,
    )
    .map(|points| points.into_iter().map(from_core_point).collect())
}

fn resolve_segment_marker_endpoint_groups(
    file: &GspFile,
    groups: &[ObjectGroup],
    host_group_index: usize,
) -> Option<(usize, usize)> {
    let host_group = groups.get(host_group_index)?;
    match host_group.header.kind() {
        crate::format::GroupKind::Segment => {
            let path = find_indexed_path(file, host_group)?;
            Some((
                path.refs.first()?.checked_sub(1)?,
                path.refs.get(1)?.checked_sub(1)?,
            ))
        }
        crate::format::GroupKind::Translation => {
            let path = find_indexed_path(file, host_group)?;
            let source_segment_group = groups.get(path.refs.first()?.checked_sub(1)?)?;
            if (source_segment_group.header.kind()) != crate::format::GroupKind::Segment {
                return None;
            }
            let source_path = find_indexed_path(file, source_segment_group)?;
            let vector_start_group_index = path.refs.get(1)?.checked_sub(1)?;
            let vector_end_group_index = path.refs.get(2)?.checked_sub(1)?;
            let start_group_index = resolve_translated_endpoint_group(
                file,
                groups,
                source_path.refs.first()?.checked_sub(1)?,
                vector_start_group_index,
                vector_end_group_index,
            )?;
            let end_group_index = resolve_translated_endpoint_group(
                file,
                groups,
                source_path.refs.get(1)?.checked_sub(1)?,
                vector_start_group_index,
                vector_end_group_index,
            )?;
            Some((start_group_index, end_group_index))
        }
        _ => None,
    }
}

fn resolve_translated_endpoint_group(
    file: &GspFile,
    groups: &[ObjectGroup],
    source_point_group_index: usize,
    vector_start_group_index: usize,
    vector_end_group_index: usize,
) -> Option<usize> {
    if source_point_group_index == vector_start_group_index {
        return Some(vector_end_group_index);
    }
    groups.iter().enumerate().find_map(|(group_index, group)| {
        if (group.header.kind()) != crate::format::GroupKind::Translation {
            return None;
        }
        let path = find_indexed_path(file, group)?;
        (path.refs.len() >= 3
            && path.refs[0].checked_sub(1)? == source_point_group_index
            && path.refs[1].checked_sub(1)? == vector_start_group_index
            && path.refs[2].checked_sub(1)? == vector_end_group_index)
            .then_some(group_index)
    })
}

fn resolve_angle_bisector_ray_shape(
    file: &GspFile,
    anchors: &[Option<PointRecord>],
    group: &ObjectGroup,
) -> Option<LineShape> {
    let path = find_indexed_path(file, group)?;
    if path.refs.len() != 3 {
        return None;
    }

    let start_index = path.refs[0].checked_sub(1)?;
    let vertex_index = path.refs[1].checked_sub(1)?;
    let end_index = path.refs[2].checked_sub(1)?;
    let start = anchors.get(start_index)?.clone()?;
    let vertex = anchors.get(vertex_index)?.clone()?;
    let end = anchors.get(end_index)?.clone()?;

    let bisector_end = angle_bisector_direction(&start, &vertex, &end).map_or_else(
        || vertex.clone(),
        |(dir_x, dir_y)| PointRecord {
            x: vertex.x + dir_x,
            y: vertex.y + dir_y,
        },
    );

    Some(LineShape {
        points: vec![vertex.clone(), bisector_end],
        color: color_from_style(group.header.style_b),
        dashed: line_is_dashed(group.header.style_a),
        stroke_width: Some(line_stroke_width_from_style(group.header.style_a)),
        visible: !group.header.is_hidden(),
        binding: Some(LineBinding::AngleBisectorRay {
            start_index,
            vertex_index,
            end_index,
        }),
        debug: Some(payload_debug_source(group)),
    })
}

fn resolve_perpendicular_line_shape(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
    group: &ObjectGroup,
) -> Option<LineShape> {
    let path = find_indexed_path(file, group)?;
    if path.refs.len() != 2 {
        return None;
    }

    let through_index = path.refs[0].checked_sub(1)?;
    let host_index = path.refs[1].checked_sub(1)?;
    let through = anchors.get(through_index)?.clone()?;
    let (line_start_index, line_end_index, host_start, host_end) =
        resolve_host_line_points(file, groups, anchors, host_index)?;

    let dx = host_end.x - host_start.x;
    let dy = host_end.y - host_start.y;
    let host_len = (dx * dx + dy * dy).sqrt();
    let start = through.clone();
    let end = if host_len <= 1e-9 {
        through.clone()
    } else {
        PointRecord {
            x: through.x - dy / host_len,
            y: through.y + dx / host_len,
        }
    };

    Some(LineShape {
        points: vec![start, end],
        color: color_from_style(group.header.style_b),
        dashed: line_is_dashed(group.header.style_a),
        stroke_width: Some(line_stroke_width_from_style(group.header.style_a)),
        visible: !group.header.is_hidden(),
        binding: Some(LineBinding::PerpendicularLine {
            through_index,
            line_start_index,
            line_end_index,
            line_index: Some(host_index),
        }),
        debug: Some(payload_debug_source(group)),
    })
}

fn resolve_parallel_line_shape(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
    group: &ObjectGroup,
) -> Option<LineShape> {
    let path = find_indexed_path(file, group)?;
    if path.refs.len() != 2 {
        return None;
    }

    let through_index = path.refs[0].checked_sub(1)?;
    let host_index = path.refs[1].checked_sub(1)?;
    let through = anchors.get(through_index)?.clone()?;
    let (line_start_index, line_end_index, host_start, host_end) =
        resolve_host_line_points(file, groups, anchors, host_index)?;

    let dx = host_end.x - host_start.x;
    let dy = host_end.y - host_start.y;
    let host_len = (dx * dx + dy * dy).sqrt();
    let start = through.clone();
    let end = if host_len <= 1e-9 {
        through.clone()
    } else {
        PointRecord {
            x: through.x + dx / host_len,
            y: through.y + dy / host_len,
        }
    };

    Some(LineShape {
        points: vec![start, end],
        color: color_from_style(group.header.style_b),
        dashed: line_is_dashed(group.header.style_a),
        stroke_width: Some(line_stroke_width_from_style(group.header.style_a)),
        visible: !group.header.is_hidden(),
        binding: Some(LineBinding::ParallelLine {
            through_index,
            line_start_index,
            line_end_index,
            line_index: Some(host_index),
        }),
        debug: Some(payload_debug_source(group)),
    })
}

fn angle_bisector_direction(
    start: &PointRecord,
    vertex: &PointRecord,
    end: &PointRecord,
) -> Option<(f64, f64)> {
    let direction = gsp_runtime_core::angle_bisector_direction(
        gsp_runtime_core::Point {
            x: start.x,
            y: start.y,
        },
        gsp_runtime_core::Point {
            x: vertex.x,
            y: vertex.y,
        },
        gsp_runtime_core::Point { x: end.x, y: end.y },
    )?;
    Some((direction.x, direction.y))
}

fn resolve_host_line_points(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
    group_index: usize,
) -> Option<(Option<usize>, Option<usize>, PointRecord, PointRecord)> {
    let group = groups.get(group_index)?;
    let path = find_indexed_path(file, group)?;

    let start_index = path.refs.first().and_then(|ordinal| ordinal.checked_sub(1));
    let end_index = path.refs.get(1).and_then(|ordinal| ordinal.checked_sub(1));
    let (start, end) = resolve_line_like_points_raw(file, groups, anchors, group)?;
    Some((start_index, end_index, start, end))
}

pub(crate) fn collect_bound_line_shapes(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
    kind: crate::format::GroupKind,
    suppressed_group_indices: &std::collections::BTreeSet<usize>,
) -> Vec<LineShape> {
    groups
        .iter()
        .enumerate()
        .filter(|(_, group)| (group.header.kind()) == kind)
        .filter_map(|(group_index, group)| {
            if suppressed_group_indices.contains(&group_index) {
                return None;
            }
            let path = find_indexed_path(file, group)?;
            if path.refs.len() != 2 {
                return None;
            }
            let start_group_index = path.refs[0].checked_sub(1)?;
            let end_group_index = path.refs[1].checked_sub(1)?;
            let start = anchors.get(start_group_index)?.clone()?;
            let end = anchors.get(end_group_index)?.clone()?;
            has_distinct_points(&[start.clone(), end.clone()]).then_some(LineShape {
                points: vec![start, end],
                color: color_from_style(group.header.style_b),
                dashed: line_is_dashed(group.header.style_a),
                stroke_width: Some(line_stroke_width_from_style(group.header.style_a)),
                visible: !group.header.is_hidden(),
                binding: Some(match kind {
                    crate::format::GroupKind::Line => LineBinding::Line {
                        start_index: start_group_index,
                        end_index: end_group_index,
                    },
                    crate::format::GroupKind::Ray => LineBinding::Ray {
                        start_index: start_group_index,
                        end_index: end_group_index,
                    },
                    _ => return None,
                }),
                debug: Some(payload_debug_source(group)),
            })
        })
        .collect()
}

pub(crate) fn collect_materialized_ray_groups(
    file: &GspFile,
    groups: &[ObjectGroup],
) -> BTreeSet<usize> {
    let _ = (file, groups);
    BTreeSet::new()
}

pub(crate) fn collect_polygon_shapes(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
    kinds: &[crate::format::GroupKind],
) -> Vec<PolygonShape> {
    groups
        .iter()
        .filter(|group| kinds.contains(&(group.header.kind())))
        .filter_map(|group| {
            let path = find_indexed_path(file, group)?;
            let points = path
                .refs
                .iter()
                .filter_map(|object_ref| {
                    anchors.get(object_ref.saturating_sub(1)).cloned().flatten()
                })
                .collect::<Vec<_>>();
            (points.len() >= 3).then_some(PolygonShape {
                points,
                color: fill_color_from_styles(group.header.style_b, group.header.style_c),
                color_binding: None,
                visible: !group.header.is_hidden(),
                binding: Some(ShapeBinding::PointPolygon {
                    vertex_indices: path
                        .refs
                        .iter()
                        .filter_map(|object_ref| object_ref.checked_sub(1))
                        .collect(),
                }),
                debug: Some(payload_debug_source(group)),
            })
        })
        .collect()
}

pub(crate) fn collect_circle_shapes(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
) -> Vec<CircleShape> {
    let circle_fill_colors = collect_circle_fill_colors(file, groups, anchors);
    let dashed_circle_indices = groups
        .iter()
        .filter_map(|group| {
            let path = find_indexed_path(file, group)?;
            match group.header.kind() {
                crate::format::GroupKind::ArcOnCircle => {
                    if let Some(ordinal) = path.refs.first() {
                        ordinal.checked_sub(1)
                    } else {
                        None
                    }
                }
                crate::format::GroupKind::CenterArc => {
                    if path.refs.len() < 2 {
                        return None;
                    }
                    let arc_prefix = &path.refs[..2];
                    groups.iter().enumerate().find_map(|(index, candidate)| {
                        ((candidate.header.kind()) == crate::format::GroupKind::Circle)
                            .then(|| find_indexed_path(file, candidate))
                            .flatten()
                            .filter(|circle_path| circle_path.refs.as_slice() == arc_prefix)
                            .map(|_| index)
                    })
                }
                _ => None,
            }
        })
        .collect::<BTreeSet<_>>();

    groups
        .iter()
        .enumerate()
        .filter(|(_, group)| is_circle_group_kind(group.header.kind()))
        .filter_map(|(group_index, group)| {
            let (center, radius_point) = resolve_circle_points_raw(file, groups, anchors, group)?;
            let binding = match group.header.kind() {
                crate::format::GroupKind::Circle => {
                    let path = find_indexed_path(file, group)?;
                    if path.refs.len() != 2 {
                        return None;
                    }
                    Some(ShapeBinding::PointRadiusCircle {
                        center_index: path.refs[0].checked_sub(1)?,
                        radius_index: path.refs[1].checked_sub(1)?,
                    })
                }
                crate::format::GroupKind::CircleCenterRadius => {
                    let path = find_indexed_path(file, group)?;
                    if path.refs.len() != 2 {
                        return None;
                    }
                    let center_index = path.refs[0].checked_sub(1)?;
                    let radius_group_index = path.refs[1].checked_sub(1)?;
                    let radius_group = groups.get(radius_group_index)?;
                    if let Some((line_start_index, line_end_index)) =
                        measured_radius_segment_group_indices(file, groups, radius_group)
                    {
                        Some(ShapeBinding::SegmentRadiusCircle {
                            center_index,
                            line_start_index,
                            line_end_index,
                        })
                    } else if radius_group.header.kind() == GroupKind::FunctionExpr {
                        Some(ShapeBinding::ExpressionRadiusCircle {
                            center_index,
                            expr: try_decode_function_expr(file, groups, radius_group).ok()?,
                        })
                    } else {
                        let parameter_name = decode_label_name(file, radius_group)?;
                        circle_center_radius_value(file, groups, anchors, radius_group)?;
                        Some(ShapeBinding::ParameterRadiusCircle {
                            center_index,
                            parameter_name,
                            raw_per_unit: DEFAULT_GRAPH_RAW_PER_UNIT,
                        })
                    }
                }
                _ => None,
            };
            Some(CircleShape {
                center,
                radius_point,
                color: color_from_style(group.header.style_b),
                fill_color: circle_fill_colors.get(&group_index).map(|fill| fill.0),
                fill_visible: circle_fill_colors
                    .get(&group_index)
                    .is_some_and(|fill| fill.1),
                fill_color_binding: None,
                dashed: dashed_circle_indices.contains(&group_index),
                visible: !group.header.is_hidden(),
                binding,
                debug: Some(payload_debug_source(group)),
            })
        })
        .collect()
}

pub(crate) fn collect_three_point_arc_shapes(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
) -> Vec<ArcShape> {
    groups
        .iter()
        .filter(|group| {
            matches!(
                group.header.kind(),
                crate::format::GroupKind::ThreePointArc
                    | crate::format::GroupKind::ArcOnCircle
                    | crate::format::GroupKind::CenterArc
            )
        })
        .filter_map(|group| {
            let path = find_indexed_path(file, group)?;
            let (points, center, counterclockwise, binding) = match group.header.kind() {
                crate::format::GroupKind::ThreePointArc => {
                    if path.refs.len() != 3 {
                        return None;
                    }
                    let points = [
                        anchors.get(path.refs[0].saturating_sub(1))?.clone()?,
                        anchors.get(path.refs[1].saturating_sub(1))?.clone()?,
                        anchors.get(path.refs[2].saturating_sub(1))?.clone()?,
                    ];
                    three_point_arc_geometry(&points[0], &points[1], &points[2])?;
                    (
                        points,
                        None,
                        false,
                        ArcBinding::ThreePointArc {
                            start_index: path.refs[0].checked_sub(1)?,
                            mid_index: path.refs[1].checked_sub(1)?,
                            end_index: path.refs[2].checked_sub(1)?,
                        },
                    )
                }
                crate::format::GroupKind::ArcOnCircle => {
                    if path.refs.len() != 3 {
                        return None;
                    }
                    let circle_group = groups.get(path.refs[0].checked_sub(1)?)?;
                    if !is_circle_group_kind(circle_group.header.kind()) {
                        return None;
                    }
                    let (center, _) =
                        resolve_circle_points_raw(file, groups, anchors, circle_group)?;
                    let start = anchors.get(path.refs[1].saturating_sub(1))?.clone()?;
                    let end = anchors.get(path.refs[2].saturating_sub(1))?.clone()?;
                    (
                        arc_on_circle_control_points(&center, &start, &end)?,
                        Some(center),
                        true,
                        ArcBinding::CircleArc {
                            circle_index: path.refs[0].checked_sub(1)?,
                            start_index: path.refs[1].checked_sub(1)?,
                            end_index: path.refs[2].checked_sub(1)?,
                        },
                    )
                }
                crate::format::GroupKind::CenterArc => {
                    if path.refs.len() != 3 {
                        return None;
                    }
                    let center = anchors.get(path.refs[0].saturating_sub(1))?.clone()?;
                    let start = anchors.get(path.refs[1].saturating_sub(1))?.clone()?;
                    let end = anchors.get(path.refs[2].saturating_sub(1))?.clone()?;
                    (
                        arc_on_circle_control_points(&center, &start, &end)?,
                        Some(center),
                        true,
                        ArcBinding::CenterArc {
                            center_index: path.refs[0].checked_sub(1)?,
                            start_index: path.refs[1].checked_sub(1)?,
                            end_index: path.refs[2].checked_sub(1)?,
                        },
                    )
                }
                _ => return None,
            };
            Some(ArcShape {
                points,
                color: color_from_style(group.header.style_b),
                center,
                counterclockwise,
                visible: !group.header.is_hidden() && arc_stroke_visible(group),
                binding: Some(binding),
                debug: Some(payload_debug_source(group)),
            })
        })
        .collect()
}

fn arc_stroke_visible(group: &ObjectGroup) -> bool {
    group.header.kind() != crate::format::GroupKind::ThreePointArc
        || group.header.style_c != 0x0000_ffff
        || color_from_style(group.header.style_b) == [0, 0, 0, 255]
}

pub(super) fn resolve_arc_boundary_points(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
    group: &ObjectGroup,
) -> Option<Vec<PointRecord>> {
    let (center, [start, mid, end], starts_from_end, complement) =
        resolve_boundary_arc_components(file, groups, anchors, group)?;
    let arc_points = if complement {
        sample_three_point_arc_complement(&start, &mid, &end, ARC_BOUNDARY_SUBDIVISIONS)?
    } else {
        sample_three_point_arc(&start, &mid, &end, ARC_BOUNDARY_SUBDIVISIONS)?
    };
    match group.header.kind() {
        crate::format::GroupKind::SectorBoundary => {
            let center = center?;
            let mut points = if starts_from_end {
                vec![end.clone(), center.clone(), start.clone()]
            } else {
                vec![center.clone(), start.clone()]
            };
            points.extend(arc_points.into_iter().skip(1));
            if !starts_from_end {
                points.push(center);
            }
            Some(points)
        }
        crate::format::GroupKind::CircularSegmentBoundary => {
            let mut points = if starts_from_end {
                vec![end.clone(), start.clone()]
            } else {
                vec![start.clone()]
            };
            points.extend(arc_points.into_iter().skip(1));
            if !starts_from_end {
                points.push(start);
            }
            Some(points)
        }
        _ => None,
    }
}

fn resolve_boundary_arc_components(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
    group: &ObjectGroup,
) -> Option<(Option<PointRecord>, [PointRecord; 3], bool, bool)> {
    let path = find_indexed_path(file, group)?;
    let arc_group = groups.get(path.refs.first()?.checked_sub(1)?)?;
    match arc_group.header.kind() {
        crate::format::GroupKind::CenterArc => {
            let arc_path = find_indexed_path(file, arc_group)?;
            if arc_path.refs.len() != 3 {
                return None;
            }
            let center = anchors.get(arc_path.refs[0].checked_sub(1)?)?.clone()?;
            let start = anchors.get(arc_path.refs[1].checked_sub(1)?)?.clone()?;
            let end = anchors.get(arc_path.refs[2].checked_sub(1)?)?.clone()?;
            Some((
                Some(center.clone()),
                arc_on_circle_control_points(&center, &start, &end)?,
                true,
                false,
            ))
        }
        crate::format::GroupKind::ArcOnCircle => {
            let arc_path = find_indexed_path(file, arc_group)?;
            if arc_path.refs.len() != 3 {
                return None;
            }
            let circle_group = groups.get(arc_path.refs[0].checked_sub(1)?)?;
            let (center, _) = resolve_circle_points_raw(file, groups, anchors, circle_group)?;
            let start = anchors.get(arc_path.refs[1].checked_sub(1)?)?.clone()?;
            let end = anchors.get(arc_path.refs[2].checked_sub(1)?)?.clone()?;
            Some((
                Some(center.clone()),
                arc_on_circle_control_points(&center, &start, &end)?,
                false,
                false,
            ))
        }
        crate::format::GroupKind::ThreePointArc => {
            let arc_path = find_indexed_path(file, arc_group)?;
            if arc_path.refs.len() != 3 {
                return None;
            }
            let start = anchors.get(arc_path.refs[0].checked_sub(1)?)?.clone()?;
            let mid = anchors.get(arc_path.refs[1].checked_sub(1)?)?.clone()?;
            let end = anchors.get(arc_path.refs[2].checked_sub(1)?)?.clone()?;
            let center =
                three_point_arc_geometry(&start, &mid, &end).map(|geometry| geometry.center);
            Some((
                center,
                [start, mid, end],
                false,
                (group.header.kind()) == crate::format::GroupKind::CircularSegmentBoundary,
            ))
        }
        _ => None,
    }
}

fn resolve_boundary_arc_seed_points(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
    group: &ObjectGroup,
) -> Option<Vec<PointRecord>> {
    let path = find_indexed_path(file, group)?;
    let arc_group = groups.get(path.refs.first()?.checked_sub(1)?)?;
    let arc_path = find_indexed_path(file, arc_group)?;

    match arc_group.header.kind() {
        crate::format::GroupKind::CenterArc => {
            if arc_path.refs.len() != 3 {
                return None;
            }
            Some(vec![
                anchors.get(arc_path.refs[0].checked_sub(1)?)?.clone()?,
                anchors.get(arc_path.refs[1].checked_sub(1)?)?.clone()?,
                anchors.get(arc_path.refs[2].checked_sub(1)?)?.clone()?,
            ])
        }
        crate::format::GroupKind::ArcOnCircle => {
            if arc_path.refs.len() != 3 {
                return None;
            }
            Some(vec![
                anchors.get(arc_path.refs[1].checked_sub(1)?)?.clone()?,
                anchors.get(arc_path.refs[2].checked_sub(1)?)?.clone()?,
                anchors.get(arc_path.refs[1].checked_sub(1)?)?.clone()?,
            ])
        }
        crate::format::GroupKind::ThreePointArc => {
            if arc_path.refs.len() != 3 {
                return None;
            }
            Some(vec![
                anchors.get(arc_path.refs[0].checked_sub(1)?)?.clone()?,
                anchors.get(arc_path.refs[1].checked_sub(1)?)?.clone()?,
                anchors.get(arc_path.refs[2].checked_sub(1)?)?.clone()?,
            ])
        }
        _ => None,
    }
}

fn resolve_arc_boundary_binding(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
) -> Option<LineBinding> {
    let path = find_indexed_path(file, group)?;
    let arc_group = groups.get(path.refs.first()?.checked_sub(1)?)?;
    let boundary_kind = match group.header.kind() {
        crate::format::GroupKind::SectorBoundary => ArcBoundaryKind::Sector,
        crate::format::GroupKind::CircularSegmentBoundary => ArcBoundaryKind::CircularSegment,
        _ => return None,
    };
    match arc_group.header.kind() {
        crate::format::GroupKind::CenterArc => {
            let arc_path = find_indexed_path(file, arc_group)?;
            (arc_path.refs.len() == 3).then_some(LineBinding::ArcBoundary {
                host_key: group.ordinal,
                boundary_kind,
                center_index: Some(arc_path.refs[0].checked_sub(1)?),
                start_index: arc_path.refs[1].checked_sub(1)?,
                mid_index: None,
                end_index: arc_path.refs[2].checked_sub(1)?,
                reversed: true,
                complement: false,
            })
        }
        crate::format::GroupKind::ArcOnCircle => {
            let arc_path = find_indexed_path(file, arc_group)?;
            if arc_path.refs.len() != 3 {
                return None;
            }
            let circle_group = groups.get(arc_path.refs[0].checked_sub(1)?)?;
            let circle_path = find_indexed_path(file, circle_group)?;
            if circle_path.refs.len() != 2 {
                return None;
            }
            Some(LineBinding::ArcBoundary {
                host_key: group.ordinal,
                boundary_kind,
                center_index: Some(circle_path.refs[0].checked_sub(1)?),
                start_index: arc_path.refs[1].checked_sub(1)?,
                mid_index: None,
                end_index: arc_path.refs[2].checked_sub(1)?,
                reversed: false,
                complement: false,
            })
        }
        crate::format::GroupKind::ThreePointArc => {
            let arc_path = find_indexed_path(file, arc_group)?;
            (arc_path.refs.len() == 3).then_some(LineBinding::ArcBoundary {
                host_key: group.ordinal,
                boundary_kind,
                center_index: None,
                start_index: arc_path.refs[0].checked_sub(1)?,
                mid_index: Some(arc_path.refs[1].checked_sub(1)?),
                end_index: arc_path.refs[2].checked_sub(1)?,
                reversed: false,
                complement: boundary_kind == ArcBoundaryKind::CircularSegment,
            })
        }
        _ => None,
    }
}

fn resolve_arc_boundary_polygon_binding(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
) -> Option<ShapeBinding> {
    let binding = resolve_arc_boundary_binding(file, groups, group)?;
    match binding {
        LineBinding::ArcBoundary {
            host_key,
            boundary_kind,
            center_index,
            start_index,
            mid_index,
            end_index,
            reversed,
            complement,
        } => Some(ShapeBinding::ArcBoundaryPolygon {
            host_key,
            boundary_kind,
            center_index,
            start_index,
            mid_index,
            end_index,
            reversed,
            complement,
        }),
        _ => None,
    }
}

pub(crate) fn collect_coordinate_traces(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
    graph: &Option<GraphTransform>,
) -> Vec<LineShape> {
    groups
        .iter()
        .filter(|group| (group.header.kind()) == crate::format::GroupKind::CoordinateTrace)
        .filter_map(|group| {
            let path = find_indexed_path(file, group)?;
            if path.refs.len() < 3 {
                return None;
            }
            let parameter_group = groups.get(path.refs[1].checked_sub(1)?)?;
            let calc_group = groups.get(path.refs[2].checked_sub(1)?)?;
            let parameter_name = decode_label_name(file, parameter_group)?;
            let payload = group
                .records
                .iter()
                .find(|record| {
                    record.record_type
                        == crate::runtime::payload_consts::RECORD_FUNCTION_PLOT_DESCRIPTOR
                })
                .map(|record| record.payload(&file.data))?;
            let descriptor = try_decode_function_plot_descriptor(payload).ok()?;
            let expr = try_decode_function_expr(file, groups, calc_group).ok()?;
            let driver_group = groups.get(path.refs[0].checked_sub(1)?)?;
            let driver = match driver_group.header.kind() {
                GroupKind::CoordinateExpressionPointPair
                | GroupKind::CoordinateExpressionPoint
                | GroupKind::CoordinateExpressionPointAlt => {
                    let driver_path = find_indexed_path(file, driver_group)?;
                    let source_group_index = driver_path.refs[0].checked_sub(1)?;
                    let source_position = anchors.get(source_group_index)?.clone()?;
                    let source_world = crate::runtime::geometry::to_world(&source_position, graph);
                    match driver_group.header.kind() {
                        GroupKind::CoordinateExpressionPointPair => {
                            let x_calc_group = groups.get(driver_path.refs[1].checked_sub(1)?)?;
                            let y_calc_group = groups.get(driver_path.refs[2].checked_sub(1)?)?;
                            let x_expr =
                                try_decode_function_expr(file, groups, x_calc_group).ok()?;
                            let y_expr =
                                try_decode_function_expr(file, groups, y_calc_group).ok()?;
                            Some((source_world, None, Some((x_expr, y_expr))))
                        }
                        GroupKind::CoordinateExpressionPointAlt => Some((
                            source_world,
                            Some(crate::runtime::scene::CoordinateAxis::Horizontal),
                            None,
                        )),
                        GroupKind::CoordinateExpressionPoint => {
                            let driver_payload = driver_group
                                .records
                                .iter()
                                .find(|record| {
                                    record.record_type
                                        == crate::runtime::payload_consts::RECORD_BINDING_PAYLOAD
                                })
                                .map(|record| record.payload(&file.data))?;
                            let axis = match (driver_payload.len() >= 24)
                                .then(|| crate::format::read_u32(driver_payload, 20))
                            {
                                Some(1) => crate::runtime::scene::CoordinateAxis::Vertical,
                                _ => crate::runtime::scene::CoordinateAxis::Horizontal,
                            };
                            Some((source_world, Some(axis), None))
                        }
                        _ => None,
                    }
                }
                _ => None,
            };

            let mut points = Vec::with_capacity(descriptor.sample_count);
            let last = descriptor.sample_count.saturating_sub(1).max(1) as f64;
            for index in 0..descriptor.sample_count {
                let t = index as f64 / last;
                let x = descriptor.x_min + (descriptor.x_max - descriptor.x_min) * t;
                let parameters = BTreeMap::from([(parameter_name.clone(), x)]);
                let offset = evaluate_expr_with_parameters(&expr, 0.0, &parameters)?;
                let world = match &driver {
                    Some((
                        source_world,
                        Some(crate::runtime::scene::CoordinateAxis::Horizontal),
                        _,
                    )) => PointRecord {
                        x: source_world.x + offset,
                        y: source_world.y,
                    },
                    Some((
                        source_world,
                        Some(crate::runtime::scene::CoordinateAxis::Vertical),
                        _,
                    )) => PointRecord {
                        x: source_world.x,
                        y: source_world.y + offset,
                    },
                    Some((source_world, None, Some((x_expr, y_expr)))) => {
                        let dx = evaluate_expr_with_parameters(x_expr, 0.0, &parameters)?;
                        let dy = evaluate_expr_with_parameters(y_expr, 0.0, &parameters)?;
                        PointRecord {
                            x: source_world.x + dx,
                            y: source_world.y + dy,
                        }
                    }
                    Some((_, None, None)) => return None,
                    None => PointRecord { x, y: offset },
                };
                let point = if let Some(transform) = graph {
                    to_raw_from_world(&world, transform)
                } else {
                    world
                };
                points.push(point);
            }

            (points.len() >= 2).then_some(LineShape {
                points,
                color: color_from_style(group.header.style_b),
                dashed: line_is_dashed(group.header.style_a),
                stroke_width: Some(line_stroke_width_from_style(group.header.style_a)),
                visible: !group.header.is_hidden(),
                binding: Some(LineBinding::CoordinateTrace {
                    point_index: path.refs[0].checked_sub(1)?,
                    x_min: descriptor.x_min,
                    x_max: descriptor.x_max,
                    sample_count: descriptor.sample_count,
                }),
                debug: Some(payload_debug_source(group)),
            })
        })
        .collect()
}
