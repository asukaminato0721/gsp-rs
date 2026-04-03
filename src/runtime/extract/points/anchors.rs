use super::super::decode::find_indexed_path;
use super::constraints::{
    RawPointConstraint, decode_parameter_controlled_point, decode_point_constraint,
    decode_translated_point_constraint,
};
use super::{
    GspFile, ObjectGroup, PointRecord, TransformBindingKind,
    decode_non_graph_parameter_value_for_group, decode_parameter_rotation_binding, read_f64,
};
use crate::runtime::geometry::{GraphTransform, lerp_point, reflect_across_line, rotate_around};

pub(crate) fn decode_regular_polygon_vertex_anchor_raw(
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

pub(crate) fn decode_point_pair_translation_anchor_raw(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
    anchors: &[Option<PointRecord>],
) -> Option<PointRecord> {
    if (group.header.class_id & 0xffff) != 16 {
        return None;
    }
    let path = find_indexed_path(file, group)?;
    let source_group_index = path.refs.first()?.checked_sub(1)?;
    let source_group = groups.get(source_group_index)?;
    if (source_group.header.class_id & 0xffff) != 0 {
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
    if !matches!(line_group.header.class_id & 0xffff, 2 | 63 | 64) {
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
    if (group.header.class_id & 0xffff) != 16 {
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
    if (group.header.class_id & 0xffff) != 1 {
        return None;
    }

    let path = find_indexed_path(file, group)?;
    let host_group = groups.get(path.refs.first()?.checked_sub(1)?)?;
    if !matches!(host_group.header.class_id & 0xffff, 2 | 63 | 64) {
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
