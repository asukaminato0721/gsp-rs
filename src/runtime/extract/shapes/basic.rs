use std::collections::{BTreeMap, BTreeSet};

use super::{
    ArcShape, CircleShape, GraphTransform, GspFile, LineBinding, LineShape, ObjectGroup,
    PointRecord, PolygonShape, ShapeBinding, color_from_style, decode_function_expr,
    decode_function_plot_descriptor, decode_label_name, evaluate_expr_with_parameters,
    fill_color_from_styles, find_indexed_path, has_distinct_points, three_point_arc_geometry,
    to_raw_from_world,
};
use crate::format::{read_f64, read_u32};
use crate::runtime::extract::decode::{is_circle_group_kind, resolve_circle_points_raw};
use crate::runtime::geometry::arc_on_circle_control_points;

pub(crate) fn collect_line_shapes(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
    kinds: &[crate::format::GroupKind],
    fallback_generic: bool,
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
                || (fallback_generic
                    && matches!(
                        kind,
                        crate::format::GroupKind::Segment
                            | crate::format::GroupKind::LineKind5
                            | crate::format::GroupKind::LineKind6
                            | crate::format::GroupKind::LineKind7
                    )
                    && find_indexed_path(file, group).is_some())
        })
        .filter_map(|(_, group)| {
            match group.header.kind() {
                crate::format::GroupKind::LineKind5 => {
                    return resolve_perpendicular_line_shape(file, groups, anchors, group);
                }
                crate::format::GroupKind::LineKind6 => {
                    return resolve_parallel_line_shape(file, groups, anchors, group);
                }
                crate::format::GroupKind::LineKind7 => {
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
            let start_group_index = path.refs.first().and_then(|ordinal| ordinal.checked_sub(1));
            let end_group_index = path.refs.get(1).and_then(|ordinal| ordinal.checked_sub(1));
            (points.len() >= 2 && has_distinct_points(&points)).then_some(LineShape {
                points,
                color: if fallback_generic && !kinds.contains(&(group.header.kind())) {
                    [40, 40, 40, 255]
                } else {
                    color_from_style(group.header.style_b)
                },
                dashed: (group.header.kind()) == crate::format::GroupKind::MeasurementLine,
                binding: match (group.header.kind(), start_group_index, end_group_index) {
                    (crate::format::GroupKind::Segment, Some(start_index), Some(end_index)) => {
                        Some(LineBinding::Segment {
                            start_index,
                            end_index,
                        })
                    }
                    (
                        crate::format::GroupKind::MeasurementLine
                        | crate::format::GroupKind::AxisLine,
                        Some(start_index),
                        Some(end_index),
                    ) => Some(LineBinding::Segment {
                        start_index,
                        end_index,
                    }),
                    _ => None,
                },
            })
        })
        .collect()
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
                dashed: false,
                binding: Some(LineBinding::SegmentMarker {
                    start_index: start_group_index,
                    end_index: end_group_index,
                    t,
                    marker_class,
                }),
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

    let first = normalize_direction(&vertex, &start)?;
    let second = normalize_direction(&vertex, &end)?;
    let first_len = ((start.x - vertex.x).powi(2) + (start.y - vertex.y).powi(2)).sqrt();
    let second_len = ((end.x - vertex.x).powi(2) + (end.y - vertex.y).powi(2)).sqrt();
    let shortest_len = first_len.min(second_len);
    if shortest_len <= 1e-9 {
        return None;
    }

    let marker_class = decode_angle_marker_class(file, group).max(1);
    let points = resolve_angle_marker_points(&vertex, first, second, shortest_len, marker_class)?;

    has_distinct_points(&points).then_some(LineShape {
        points,
        color: color_from_style(group.header.style_b),
        dashed: false,
        binding: Some(LineBinding::AngleMarker {
            start_index: path.refs[0].checked_sub(1)?,
            vertex_index: path.refs[1].checked_sub(1)?,
            end_index: path.refs[2].checked_sub(1)?,
            marker_class,
        }),
    })
}

pub(crate) fn resolve_angle_marker_points(
    vertex: &PointRecord,
    first: (f64, f64),
    second: (f64, f64),
    shortest_len: f64,
    marker_class: u32,
) -> Option<Vec<PointRecord>> {
    let dot = (first.0 * second.0 + first.1 * second.1).clamp(-1.0, 1.0);
    let cross = first.0 * second.1 - first.1 * second.0;
    if dot.abs() <= 0.12 {
        return resolve_right_angle_marker_points(vertex, first, second, shortest_len);
    }
    resolve_arc_angle_marker_points(vertex, first, shortest_len, cross, dot, marker_class)
}

fn resolve_right_angle_marker_points(
    vertex: &PointRecord,
    first: (f64, f64),
    second: (f64, f64),
    shortest_len: f64,
) -> Option<Vec<PointRecord>> {
    let side = (shortest_len * 0.125)
        .clamp(10.0, 28.0)
        .min(shortest_len * 0.5);
    if side <= 1e-9 {
        return None;
    }

    let start_on_first = PointRecord {
        x: vertex.x + first.0 * side,
        y: vertex.y + first.1 * side,
    };
    let corner = PointRecord {
        x: vertex.x + (first.0 + second.0) * side,
        y: vertex.y + (first.1 + second.1) * side,
    };
    let end_on_second = PointRecord {
        x: vertex.x + second.0 * side,
        y: vertex.y + second.1 * side,
    };

    Some(vec![start_on_first, corner, end_on_second])
}

fn resolve_arc_angle_marker_points(
    vertex: &PointRecord,
    first: (f64, f64),
    shortest_len: f64,
    cross: f64,
    dot: f64,
    marker_class: u32,
) -> Option<Vec<PointRecord>> {
    let class_scale = 1.0 + 0.18 * (marker_class.saturating_sub(1) as f64);
    let radius = ((shortest_len * 0.12).clamp(10.0, 28.0) * class_scale).min(shortest_len * 0.42);
    if radius <= 1e-9 {
        return None;
    }
    let delta = cross.atan2(dot);
    if delta.abs() <= 1e-6 {
        return None;
    }
    let start_angle = first.1.atan2(first.0);
    let samples = 9usize;
    Some(
        (0..samples)
            .map(|index| {
                let t = index as f64 / (samples - 1) as f64;
                let angle = start_angle + delta * t;
                PointRecord {
                    x: vertex.x + radius * angle.cos(),
                    y: vertex.y + radius * angle.sin(),
                }
            })
            .collect(),
    )
}

fn decode_angle_marker_class(file: &GspFile, group: &ObjectGroup) -> u32 {
    group
        .records
        .iter()
        .find(|record| record.record_type == 0x090e)
        .map(|record| record.payload(&file.data))
        .filter(|payload| payload.len() >= 4)
        .map(|payload| read_u32(payload, 0))
        .unwrap_or(1)
}

fn decode_segment_marker_payload(file: &GspFile, group: &ObjectGroup) -> Option<(f64, u32)> {
    let payload = group
        .records
        .iter()
        .find(|record| record.record_type == 0x090f)
        .map(|record| record.payload(&file.data))?;
    (payload.len() >= 12).then(|| (read_f64(payload, 0), read_u32(payload, 8)))
}

pub(crate) fn resolve_segment_marker_points(
    start: &PointRecord,
    end: &PointRecord,
    t: f64,
    marker_class: u32,
) -> Option<Vec<PointRecord>> {
    let dx = end.x - start.x;
    let dy = end.y - start.y;
    let len = (dx * dx + dy * dy).sqrt();
    if len <= 1e-9 {
        return None;
    }
    let tangent = (dx / len, dy / len);
    let normal = (-tangent.1, tangent.0);
    let center_t = t.clamp(0.0, 1.0);
    let center = PointRecord {
        x: start.x + dx * center_t,
        y: start.y + dy * center_t,
    };
    let half_len = (len * 0.06).clamp(5.0, 10.0);
    let spacing = (len * 0.05).clamp(6.0, 11.0);
    let offset = (marker_class.saturating_sub(1) as f64) * -0.5;
    let center_offset = offset * spacing;
    let slash_center = PointRecord {
        x: center.x + tangent.0 * center_offset,
        y: center.y + tangent.1 * center_offset,
    };
    Some(vec![
        PointRecord {
            x: slash_center.x - normal.0 * half_len,
            y: slash_center.y - normal.1 * half_len,
        },
        PointRecord {
            x: slash_center.x + normal.0 * half_len,
            y: slash_center.y + normal.1 * half_len,
        },
    ])
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

    let (dir_x, dir_y) = angle_bisector_direction(&start, &vertex, &end)?;
    let bisector_end = PointRecord {
        x: vertex.x + dir_x,
        y: vertex.y + dir_y,
    };

    has_distinct_points(&[vertex.clone(), bisector_end.clone()]).then_some(LineShape {
        points: vec![vertex.clone(), bisector_end],
        color: color_from_style(group.header.style_b),
        dashed: false,
        binding: Some(LineBinding::AngleBisectorRay {
            start_index,
            vertex_index,
            end_index,
        }),
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
    if host_len <= 1e-9 {
        return None;
    }

    let perp_x = -dy / host_len;
    let perp_y = dx / host_len;
    let start = through.clone();
    let end = PointRecord {
        x: through.x + perp_x,
        y: through.y + perp_y,
    };

    has_distinct_points(&[start.clone(), end.clone()]).then_some(LineShape {
        points: vec![start, end],
        color: color_from_style(group.header.style_b),
        dashed: false,
        binding: Some(LineBinding::PerpendicularLine {
            through_index,
            line_start_index: Some(line_start_index),
            line_end_index: Some(line_end_index),
            line_index: Some(host_index),
        }),
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
    if host_len <= 1e-9 {
        return None;
    }

    let start = through.clone();
    let end = PointRecord {
        x: through.x + dx / host_len,
        y: through.y + dy / host_len,
    };

    has_distinct_points(&[start.clone(), end.clone()]).then_some(LineShape {
        points: vec![start, end],
        color: color_from_style(group.header.style_b),
        dashed: false,
        binding: Some(LineBinding::ParallelLine {
            through_index,
            line_start_index: Some(line_start_index),
            line_end_index: Some(line_end_index),
            line_index: Some(host_index),
        }),
    })
}

fn angle_bisector_direction(
    start: &PointRecord,
    vertex: &PointRecord,
    end: &PointRecord,
) -> Option<(f64, f64)> {
    let first = normalize_direction(vertex, start)?;
    let second = normalize_direction(vertex, end)?;
    let sum_x = first.0 + second.0;
    let sum_y = first.1 + second.1;
    let sum_len = (sum_x * sum_x + sum_y * sum_y).sqrt();
    if sum_len > 1e-9 {
        return Some((sum_x / sum_len, sum_y / sum_len));
    }

    // A straight angle still has a deterministic bisector: the perpendicular through the vertex.
    Some((-first.1, first.0))
}

fn normalize_direction(from: &PointRecord, to: &PointRecord) -> Option<(f64, f64)> {
    let dx = to.x - from.x;
    let dy = to.y - from.y;
    let len = (dx * dx + dy * dy).sqrt();
    (len > 1e-9).then_some((dx / len, dy / len))
}

fn resolve_host_line_points(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
    group_index: usize,
) -> Option<(usize, usize, PointRecord, PointRecord)> {
    let group = groups.get(group_index)?;
    let path = find_indexed_path(file, group)?;
    if path.refs.len() != 2 {
        return None;
    }

    let start_index = path.refs[0].checked_sub(1)?;
    let end_index = path.refs[1].checked_sub(1)?;
    let start = anchors.get(start_index)?.clone()?;
    let end = anchors.get(end_index)?.clone()?;
    has_distinct_points(&[start.clone(), end.clone()]).then_some((
        start_index,
        end_index,
        start,
        end,
    ))
}

pub(crate) fn collect_bound_line_shapes(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
    kind: crate::format::GroupKind,
) -> Vec<LineShape> {
    groups
        .iter()
        .filter(|group| (group.header.kind()) == kind)
        .filter_map(|group| {
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
                dashed: false,
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
            })
        })
        .collect()
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
                binding: None,
            })
        })
        .collect()
}

pub(crate) fn collect_circle_shapes(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
) -> Vec<CircleShape> {
    let dashed_circle_indices = groups
        .iter()
        .filter(|group| (group.header.kind()) == crate::format::GroupKind::ArcOnCircle)
        .filter_map(|group| find_indexed_path(file, group))
        .filter_map(|path| path.refs.first().and_then(|ordinal| ordinal.checked_sub(1)))
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
                    let segment_group = groups.get(path.refs[1].checked_sub(1)?)?;
                    let segment_path = find_indexed_path(file, segment_group)?;
                    if segment_path.refs.len() != 2 {
                        return None;
                    }
                    Some(ShapeBinding::SegmentRadiusCircle {
                        center_index,
                        line_start_index: segment_path.refs[0].checked_sub(1)?,
                        line_end_index: segment_path.refs[1].checked_sub(1)?,
                    })
                }
                _ => None,
            };
            Some(CircleShape {
                center,
                radius_point,
                color: color_from_style(group.header.style_b),
                dashed: dashed_circle_indices.contains(&group_index),
                binding,
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
                crate::format::GroupKind::ThreePointArc | crate::format::GroupKind::ArcOnCircle
            )
        })
        .filter_map(|group| {
            let path = find_indexed_path(file, group)?;
            let points = match group.header.kind() {
                crate::format::GroupKind::ThreePointArc => {
                    if path.refs.len() != 3 {
                        return None;
                    }
                    [
                        anchors.get(path.refs[0].saturating_sub(1))?.clone()?,
                        anchors.get(path.refs[1].saturating_sub(1))?.clone()?,
                        anchors.get(path.refs[2].saturating_sub(1))?.clone()?,
                    ]
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
                    arc_on_circle_control_points(&center, &start, &end)?
                }
                _ => return None,
            };
            three_point_arc_geometry(&points[0], &points[1], &points[2])?;
            Some(ArcShape {
                points,
                color: color_from_style(group.header.style_b),
                center: match group.header.kind() {
                    crate::format::GroupKind::ArcOnCircle => {
                        let circle_group = groups.get(path.refs[0].checked_sub(1)?)?;
                        let (center, _) =
                            resolve_circle_points_raw(file, groups, anchors, circle_group)?;
                        Some(center)
                    }
                    _ => None,
                },
                counterclockwise: (group.header.kind()) == crate::format::GroupKind::ArcOnCircle,
            })
        })
        .collect()
}

pub(crate) fn collect_derived_segments(
    file: &GspFile,
    groups: &[ObjectGroup],
    point_map: &[Option<PointRecord>],
    kinds: &[crate::format::GroupKind],
) -> Vec<LineShape> {
    let refs = groups
        .iter()
        .map(|group| {
            find_indexed_path(file, group)
                .map(|path| path.refs)
                .unwrap_or_default()
        })
        .collect::<Vec<_>>();
    let class_ids = groups
        .iter()
        .map(|group| group.header.kind())
        .collect::<Vec<_>>();

    fn descend_points(
        ordinal: usize,
        refs: &[Vec<usize>],
        point_map: &[Option<PointRecord>],
        memo: &mut Vec<Option<Vec<PointRecord>>>,
        visiting: &mut BTreeSet<usize>,
    ) -> Vec<PointRecord> {
        if let Some(cached) = &memo[ordinal - 1] {
            return cached.clone();
        }
        if !visiting.insert(ordinal) {
            return Vec::new();
        }

        let mut points = Vec::new();
        if let Some(point) = point_map.get(ordinal - 1).and_then(|point| point.clone()) {
            points.push(point);
        } else {
            for child in &refs[ordinal - 1] {
                if *child > 0 && *child <= refs.len() {
                    points.extend(descend_points(*child, refs, point_map, memo, visiting));
                }
            }
        }

        visiting.remove(&ordinal);
        points.sort_by(|a, b| {
            a.x.partial_cmp(&b.x)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.y.partial_cmp(&b.y).unwrap_or(std::cmp::Ordering::Equal))
        });
        points.dedup_by(|a, b| (a.x - b.x).abs() < 0.001 && (a.y - b.y).abs() < 0.001);
        memo[ordinal - 1] = Some(points.clone());
        points
    }

    let mut memo = vec![None; groups.len()];
    let mut seen = BTreeSet::<((i32, i32), (i32, i32))>::new();
    let mut segments = Vec::new();

    for (index, class_id) in class_ids.iter().enumerate() {
        if !kinds.contains(class_id) {
            continue;
        }
        let points = descend_points(index + 1, &refs, point_map, &mut memo, &mut BTreeSet::new());
        if points.len() < 2 || points.len() > 12 {
            continue;
        }

        let mut best = None;
        let mut best_dist = -1.0_f64;
        for i in 0..points.len() {
            for j in i + 1..points.len() {
                let dx = points[i].x - points[j].x;
                let dy = points[i].y - points[j].y;
                let dist = dx * dx + dy * dy;
                if dist > best_dist {
                    best_dist = dist;
                    best = Some((points[i].clone(), points[j].clone()));
                }
            }
        }

        let Some((a, b)) = best else { continue };
        let a_key = (a.x.round() as i32, a.y.round() as i32);
        let b_key = (b.x.round() as i32, b.y.round() as i32);
        let key = if a_key <= b_key {
            (a_key, b_key)
        } else {
            (b_key, a_key)
        };
        if !seen.insert(key) {
            continue;
        }

        let color = match *class_id {
            crate::format::GroupKind::DerivedSegment24 => [20, 20, 20, 255],
            crate::format::GroupKind::FunctionExpr => [70, 70, 70, 255],
            crate::format::GroupKind::DerivedSegment75 => [120, 120, 120, 255],
            _ => [60, 60, 60, 255],
        };
        segments.push(LineShape {
            points: vec![a, b],
            color,
            dashed: false,
            binding: None,
        });
    }

    segments
}

pub(crate) fn collect_coordinate_traces(
    file: &GspFile,
    groups: &[ObjectGroup],
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
                .find(|record| record.record_type == 0x0902)
                .map(|record| record.payload(&file.data))?;
            let descriptor = decode_function_plot_descriptor(payload)?;
            let expr = decode_function_expr(file, groups, calc_group)?;

            let mut points = Vec::with_capacity(descriptor.sample_count);
            let last = descriptor.sample_count.saturating_sub(1).max(1) as f64;
            for index in 0..descriptor.sample_count {
                let t = index as f64 / last;
                let x = descriptor.x_min + (descriptor.x_max - descriptor.x_min) * t;
                let parameters = BTreeMap::from([(parameter_name.clone(), x)]);
                let y = evaluate_expr_with_parameters(&expr, 0.0, &parameters)?;
                let world = PointRecord { x, y };
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
                dashed: false,
                binding: None,
            })
        })
        .collect()
}
