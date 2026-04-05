use super::super::decode::find_indexed_path;
use super::constraints::{
    RawPointConstraint, decode_parameter_controlled_point, decode_point_constraint,
    decode_translated_point_constraint,
};
use super::{
    GspFile, ObjectGroup, PointRecord, TransformBindingKind,
    decode_non_graph_parameter_value_for_group, decode_parameter_rotation_binding, read_f64,
};
use crate::runtime::geometry::{
    GraphTransform, lerp_point, point_on_circle_arc, point_on_three_point_arc, reflect_across_line,
    rotate_around,
};

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

pub(crate) fn decode_intersection_anchor_raw(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
    anchors: &[Option<PointRecord>],
) -> Option<PointRecord> {
    let kind = group.header.kind();
    let variant = match kind {
        crate::format::GroupKind::LinearIntersectionPoint => None,
        crate::format::GroupKind::IntersectionPoint1 => Some(1),
        crate::format::GroupKind::IntersectionPoint2 => Some(0),
        crate::format::GroupKind::CircleCircleIntersectionPoint1 => Some(1),
        crate::format::GroupKind::CircleCircleIntersectionPoint2 => Some(0),
        _ => return None,
    };

    let path = find_indexed_path(file, group)?;
    if path.refs.len() != 2 {
        return None;
    }

    let left_group = groups.get(path.refs[0].checked_sub(1)?)?;
    let right_group = groups.get(path.refs[1].checked_sub(1)?)?;

    if let (Some((line_start, line_end)), Some((center, radius))) = (
        resolve_line_like_points_raw(file, groups, anchors, left_group),
        resolve_circle_like_raw(file, groups, anchors, right_group),
    ) {
        return select_line_circle_intersection(
            line_start,
            line_end,
            center,
            radius,
            variant.unwrap_or(0),
        );
    }

    if let (Some((center, radius)), Some((line_start, line_end))) = (
        resolve_circle_like_raw(file, groups, anchors, left_group),
        resolve_line_like_points_raw(file, groups, anchors, right_group),
    ) {
        return select_line_circle_intersection(
            line_start,
            line_end,
            center,
            radius,
            variant.unwrap_or(0),
        );
    }

    if let (Some((left_center, left_radius)), Some((right_center, right_radius))) = (
        resolve_circle_like_raw(file, groups, anchors, left_group),
        resolve_circle_like_raw(file, groups, anchors, right_group),
    ) {
        return select_circle_circle_intersection(
            left_center,
            left_radius,
            right_center,
            right_radius,
            variant.unwrap_or(0),
        );
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

fn resolve_circle_like_raw(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
    group: &ObjectGroup,
) -> Option<(PointRecord, f64)> {
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
            (radius > 1e-9).then_some((center, radius))
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
            (radius > 1e-9).then_some((center, radius))
        }
        _ => None,
    }
}

fn resolve_line_like_points_raw(
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

fn select_circle_circle_intersection(
    left_center: PointRecord,
    left_radius: f64,
    right_center: PointRecord,
    right_radius: f64,
    variant: usize,
) -> Option<PointRecord> {
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
    let intersections = [
        PointRecord {
            x: base.x - height * uy,
            y: base.y + height * ux,
        },
        PointRecord {
            x: base.x + height * uy,
            y: base.y - height * ux,
        },
    ];
    let mut ordered = intersections;
    ordered.sort_by(|left, right| {
        left.y
            .total_cmp(&right.y)
            .then_with(|| left.x.total_cmp(&right.x))
    });
    Some(ordered[variant.min(1)].clone())
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

pub(crate) fn decode_parameter_rotation_anchor_raw(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
    anchors: &[Option<PointRecord>],
) -> Option<PointRecord> {
    let binding = decode_parameter_rotation_binding(file, groups, group)?;
    let source = anchors.get(binding.source_group_index)?.clone()?;
    let center = anchors.get(binding.center_group_index)?.clone()?;
    let TransformBindingKind::Rotate { angle_degrees, .. } = binding.kind else {
        return None;
    };
    Some(rotate_around(&source, &center, angle_degrees.to_radians()))
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
    let source_group = groups.get(source_group_index)?;
    if (source_group.header.kind()) != crate::format::GroupKind::Point {
        return None;
    }
    let source = anchors.get(source_group_index)?.clone()?;
    let (line_start_group_index, line_end_group_index) =
        reflection_line_group_indices(file, groups, group)?;
    let line_start = anchors.get(line_start_group_index)?.clone()?;
    let line_end = anchors.get(line_end_group_index)?.clone()?;
    reflect_point_across_line(&source, &line_start, &line_end)
}

pub(crate) fn decode_point_pair_translation_anchor_raw(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
    anchors: &[Option<PointRecord>],
) -> Option<PointRecord> {
    if (group.header.kind()) != crate::format::GroupKind::Translation {
        return None;
    }
    let path = find_indexed_path(file, group)?;
    let source_group_index = path.refs.first()?.checked_sub(1)?;
    let source_group = groups.get(source_group_index)?;
    if (source_group.header.kind()) != crate::format::GroupKind::Point {
        return None;
    }
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
    decode_parameter_controlled_point(file, groups, group, anchors).map(|point| point.position)
}

pub(crate) fn reflection_line_group_indices(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
) -> Option<(usize, usize)> {
    let path = find_indexed_path(file, group)?;
    let line_group = groups.get(path.refs.get(1)?.checked_sub(1)?)?;
    if !matches!(
        line_group.header.kind(),
        crate::format::GroupKind::Segment
            | crate::format::GroupKind::Line
            | crate::format::GroupKind::Ray
    ) {
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
    if (group.header.kind()) != crate::format::GroupKind::PointConstraint {
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
    if !matches!(
        host_group.header.kind(),
        crate::format::GroupKind::Segment
            | crate::format::GroupKind::Line
            | crate::format::GroupKind::Ray
    ) {
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
    match decode_point_constraint(file, groups, group, &graph)? {
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
