use std::collections::BTreeMap;

use super::super::decode::{decode_label_name, find_indexed_path};
use super::anchors::{resolve_circle_point_raw, resolve_polygon_boundary_point_raw};
use super::{
    decode_non_graph_parameter_value_for_group, editable_non_graph_parameter_name_for_group,
};
use crate::format::{GspFile, ObjectGroup, PointRecord, read_f64, read_u32};
use crate::runtime::functions::{
    BinaryOp, FunctionExpr, FunctionTerm, ParsedFunctionExpr, decode_function_expr,
    decode_function_plot_descriptor, evaluate_expr_with_parameters, sample_function_points,
};
use crate::runtime::geometry::{GraphTransform, lerp_point, to_raw_from_world};

pub(crate) struct PointOnSegmentConstraint {
    pub(crate) start_group_index: usize,
    pub(crate) end_group_index: usize,
    pub(crate) t: f64,
}

pub(crate) struct PointOnCircleConstraint {
    pub(crate) center_group_index: usize,
    pub(crate) radius_group_index: usize,
    pub(crate) unit_x: f64,
    pub(crate) unit_y: f64,
}

pub(crate) struct TranslatedPointConstraint {
    pub(crate) origin_group_index: usize,
    pub(crate) dx: f64,
    pub(crate) dy: f64,
}

pub(crate) enum RawPointConstraint {
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

pub(crate) struct ParameterControlledPoint {
    pub(crate) position: PointRecord,
    pub(crate) constraint: RawPointConstraint,
    pub(crate) parameter_name: String,
    pub(crate) source_point_group_index: Option<usize>,
}

pub(crate) struct CoordinatePoint {
    pub(crate) position: PointRecord,
    pub(crate) parameter_name: String,
    pub(crate) expr: FunctionExpr,
}

pub(crate) fn regular_polygon_iteration_step(
    file: &GspFile,
    groups: &[ObjectGroup],
    iter_group: &ObjectGroup,
) -> Option<(usize, FunctionExpr, String, f64)> {
    let path = find_indexed_path(file, iter_group)?;
    if path.refs.len() < 3 {
        return None;
    }
    let seed_group = groups.get(path.refs[2].checked_sub(1)?)?;
    if (seed_group.header.kind()) != 29 {
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
    let parameter_name = editable_non_graph_parameter_name_for_group(file, parameter_group)?;
    let n = decode_non_graph_parameter_value_for_group(file, parameter_group)?;
    (n.abs() >= 1.0).then_some((
        center_group_index,
        regular_polygon_angle_expr(&parameter_name, n),
        parameter_name,
        n,
    ))
}

pub(crate) fn regular_polygon_angle_expr(
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

pub(crate) fn polygon_parameter_to_edge(
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

pub(crate) fn decode_translated_point_constraint(
    file: &GspFile,
    group: &ObjectGroup,
) -> Option<TranslatedPointConstraint> {
    let path = find_indexed_path(file, group)?;
    let origin_group_index = path.refs.first()?.checked_sub(1)?;
    let payload = group
        .records
        .iter()
        .find(|record| record.record_type == 0x07d3)
        .map(|record| record.payload(&file.data))?;
    match group.header.kind() {
        21 => {
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
        17 => {
            if payload.len() < 40 {
                return None;
            }
            let x_units_to_raw = read_f64(payload, 4);
            let x_distance = read_f64(payload, 12);
            let y_units_to_raw = read_f64(payload, 24);
            let y_distance = read_f64(payload, 32);
            if !x_units_to_raw.is_finite()
                || !x_distance.is_finite()
                || !y_units_to_raw.is_finite()
                || !y_distance.is_finite()
            {
                return None;
            }
            // Legacy simple-iteration seeds store independent horizontal and vertical offsets
            // instead of the angle+distance layout used by class 21 translated points.
            Some(TranslatedPointConstraint {
                origin_group_index,
                dx: x_units_to_raw * x_distance,
                dy: -(y_units_to_raw * y_distance),
            })
        }
        _ => None,
    }
}

fn decode_point_on_segment_constraint(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
) -> Option<PointOnSegmentConstraint> {
    if (group.header.kind()) != 15 {
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

pub(crate) fn decode_parameter_controlled_point(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
    anchors: &[Option<PointRecord>],
) -> Option<ParameterControlledPoint> {
    if (group.header.kind()) != 95 {
        return None;
    }

    let path = find_indexed_path(file, group)?;
    if path.refs.len() < 2 {
        return None;
    }

    let source_group = groups.get(path.refs[0].checked_sub(1)?)?;
    let host_group = groups.get(path.refs[1].checked_sub(1)?)?;
    let (parameter_name, parameter_value, source_point_group_index) =
        if (source_group.header.kind()) == 0 {
            (
                decode_label_name(file, source_group)?,
                decode_non_graph_parameter_value_for_group(file, source_group)?.clamp(0.0, 1.0),
                None,
            )
        } else if (source_group.header.kind()) == 94 {
            let path = find_indexed_path(file, source_group)?;
            let point_group_index = path.refs.first()?.checked_sub(1)?;
            let point_group = groups.get(point_group_index)?;
            let t = match decode_point_constraint(file, groups, point_group, &None)? {
                RawPointConstraint::Segment(constraint) => constraint.t,
                RawPointConstraint::PolygonBoundary {
                    edge_index,
                    t,
                    vertex_group_indices,
                } => super::super::labels::polygon_boundary_parameter(
                    anchors,
                    &vertex_group_indices,
                    edge_index,
                    t,
                )?,
                RawPointConstraint::Circle(constraint) => super::super::labels::circle_parameter(
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

    match host_group.header.kind() {
        2 => {
            let host_path = find_indexed_path(file, host_group)?;
            if host_path.refs.len() != 2 {
                return None;
            }
            let start_group_index = host_path.refs[0].checked_sub(1)?;
            let end_group_index = host_path.refs[1].checked_sub(1)?;
            let start = anchors.get(start_group_index)?.clone()?;
            let end = anchors.get(end_group_index)?.clone()?;
            let position = lerp_point(&start, &end, parameter_value);
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

pub(crate) fn decode_coordinate_point(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
    graph: &Option<GraphTransform>,
) -> Option<CoordinatePoint> {
    if (group.header.kind()) != 69 {
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

pub(crate) fn decode_point_constraint(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
    graph: &Option<GraphTransform>,
) -> Option<RawPointConstraint> {
    if (group.header.kind()) != 15 {
        return None;
    }

    let host_ref = find_indexed_path(file, group)?
        .refs
        .first()
        .copied()
        .filter(|ordinal| *ordinal > 0)?;
    let host_group = groups.get(host_ref - 1)?;
    let host_kind = host_group.header.kind();
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
