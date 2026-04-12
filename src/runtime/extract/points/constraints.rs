use std::collections::BTreeMap;

use super::super::decode::{
    decode_label_name, find_indexed_path, is_circle_group_kind,
    try_decode_parameter_control_value_for_group,
};
use super::anchors::{resolve_circle_point_raw, resolve_polygon_boundary_point_raw};
use super::{
    decode_non_graph_parameter_value_for_group, editable_non_graph_parameter_name_for_group,
};
use crate::format::{GspFile, ObjectGroup, PointRecord, read_f64, read_u32};
use crate::runtime::functions::{
    BinaryOp, FunctionExpr, FunctionTerm, ParsedFunctionExpr, evaluate_expr_with_parameters,
    sample_function_points, try_decode_function_expr, try_decode_function_plot_descriptor,
};
use crate::runtime::geometry::{
    GraphTransform, arc_on_circle_control_points, lerp_point, locate_polyline_parameter_by_length,
    point_on_circle_arc, point_on_three_point_arc, sample_three_point_arc,
    sample_three_point_arc_complement, three_point_arc_geometry, to_raw_from_world, to_world,
};
use crate::runtime::payload_consts::{RECORD_BINDING_PAYLOAD, RECORD_FUNCTION_PLOT_DESCRIPTOR};
use thiserror::Error;

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

pub(crate) struct PointOnArcConstraint {
    pub(crate) start_group_index: usize,
    pub(crate) mid_group_index: usize,
    pub(crate) end_group_index: usize,
    pub(crate) t: f64,
}

pub(crate) struct PointOnCircleArcConstraint {
    pub(crate) center_group_index: usize,
    pub(crate) start_group_index: usize,
    pub(crate) end_group_index: usize,
    pub(crate) t: f64,
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
    CircleArc(PointOnCircleArcConstraint),
    Arc(PointOnArcConstraint),
}

pub(crate) struct ParameterControlledPoint {
    pub(crate) position: PointRecord,
    pub(crate) constraint: RawPointConstraint,
    pub(crate) parameter_name: String,
    pub(crate) source_point_group_index: Option<usize>,
    pub(crate) source_expr: Option<FunctionExpr>,
}

#[derive(Debug, Clone, PartialEq, Error)]
pub(crate) enum ParameterControlledPointDecodeError {
    #[error("group kind {0:?} is not a parameter-controlled point")]
    NotParameterControlledPoint(crate::format::GroupKind),
    #[error("missing indexed path for parameter-controlled point")]
    MissingPath,
    #[error("parameter-controlled point path has too few references ({0})")]
    PathTooShort(usize),
    #[error("parameter-controlled point source group is missing")]
    MissingSourceGroup,
    #[error("parameter-controlled point host group is missing")]
    MissingHostGroup,
    #[error("source parameter/anchor/expression could not be evaluated")]
    InvalidSource,
    #[error("host geometry could not be resolved for parameter-controlled point")]
    InvalidHostGeometry,
}

pub(crate) enum CoordinatePointSource {
    Parameter(String),
    SourcePoint {
        source_group_index: usize,
        parameter_name: String,
        axis: crate::runtime::scene::CoordinateAxis,
    },
    SourcePoint2d {
        source_group_index: usize,
        x_parameter_name: String,
        x_expr: FunctionExpr,
        y_parameter_name: String,
        y_expr: FunctionExpr,
    },
}

#[derive(Debug, Clone, PartialEq, Error)]
pub(crate) enum PointConstraintDecodeError {
    #[error("group kind {0:?} is not a point-constraint kind")]
    NotPointConstraintKind(crate::format::GroupKind),
    #[error("missing indexed path for point constraint")]
    MissingIndexedPath,
    #[error("point constraint path is missing host reference")]
    MissingHostReference,
    #[error("missing 0x07d3 point-constraint payload record")]
    MissingPayloadRecord,
    #[error(
        "host group path too short for {host_kind:?}: expected at least {expected}, got {actual}"
    )]
    HostPathTooShort {
        host_kind: crate::format::GroupKind,
        expected: usize,
        actual: usize,
    },
    #[error("constraint payload contains non-finite parameter")]
    NonFiniteParameter,
    #[error("function plot constraint requires graph transform")]
    MissingGraphTransform,
    #[error("function plot descriptor missing from host group")]
    MissingFunctionPlotDescriptor,
    #[error("invalid function plot descriptor: {0}")]
    InvalidFunctionPlotDescriptor(String),
    #[error("invalid function expression for function-plot constraint: {0}")]
    InvalidFunctionExpr(String),
    #[error("point-constraint payload too short ({byte_len} bytes), expected at least {expected}")]
    PayloadTooShort { byte_len: usize, expected: usize },
    #[error("path-point constraint requires anchors")]
    MissingAnchors,
    #[error("failed to locate point on sampled polyline")]
    PolylineParameterUnavailable,
    #[error("circle host path is invalid")]
    InvalidCircleHostPath,
    #[error("circle constraint contains non-finite unit vector")]
    NonFiniteCircleUnit,
    #[error("polygon host path is invalid")]
    InvalidPolygonHostPath,
    #[error("polygon edge index could not be decoded")]
    InvalidPolygonEdgeIndex,
    #[error("arc-family host path is invalid for {0:?}")]
    InvalidArcHostPath(crate::format::GroupKind),
    #[error("arc-on-circle host does not reference a circle object")]
    ArcHostMissingCircle,
    #[error("arc-on-circle backing circle path is invalid")]
    InvalidArcCirclePath,
    #[error(
        "unsupported or malformed point constraint for host kind {host_kind:?} with payload length {payload_len}"
    )]
    UnsupportedOrMalformed {
        host_kind: crate::format::GroupKind,
        payload_len: usize,
    },
}

pub(crate) struct CoordinatePoint {
    pub(crate) position: PointRecord,
    pub(crate) source: CoordinatePointSource,
    pub(crate) expr: FunctionExpr,
}

const ARC_BOUNDARY_SUBDIVISIONS: usize = 48;

fn wrap_unit_interval(value: f64) -> f64 {
    value.rem_euclid(1.0)
}

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

fn parameter_anchor_value(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
    anchors: &[Option<PointRecord>],
) -> Option<(String, f64, usize)> {
    let path = find_indexed_path(file, group)?;
    let point_group_index = path.refs.first()?.checked_sub(1)?;
    let point_group = groups.get(point_group_index)?;
    let t = match try_decode_point_constraint(file, groups, point_group, None, &None).ok()? {
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
        RawPointConstraint::CircleArc(_) => return None,
        RawPointConstraint::Arc(_) => return None,
        RawPointConstraint::Polyline { .. } => return None,
    };
    let name = decode_label_name(file, group)
        .or_else(|| decode_label_name(file, point_group))
        .unwrap_or_default();
    Some((name, wrap_unit_interval(t), point_group_index))
}

pub(crate) fn regular_polygon_iteration_step(
    file: &GspFile,
    groups: &[ObjectGroup],
    iter_group: &ObjectGroup,
) -> Option<(usize, FunctionExpr, String, f64)> {
    let path = find_indexed_path(file, iter_group)?;
    let seed_group = path
        .refs
        .iter()
        .filter_map(|ordinal| {
            let index = ordinal.checked_sub(1)?;
            groups.get(index)
        })
        .find(|group| (group.header.kind()) == crate::format::GroupKind::ParameterRotation)?;
    let seed_path = find_indexed_path(file, seed_group)?;
    if seed_path.refs.len() < 3 {
        return None;
    }
    let center_group_index = seed_path.refs[1].checked_sub(1)?;
    let calc_group = groups.get(seed_path.refs[2].checked_sub(1)?)?;
    let calc_path = find_indexed_path(file, calc_group)?;
    let parameter_group = groups.get(calc_path.refs.first()?.checked_sub(1)?)?;
    let parameter_name =
        editable_non_graph_parameter_name_for_group(file, groups, parameter_group)?;
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
    let wrapped = wrap_unit_interval(parameter);
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
        crate::format::GroupKind::PolarOffsetPoint => {
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
        crate::format::GroupKind::CartesianOffsetPoint => {
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
    if !group.header.kind().is_point_constraint() {
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

fn try_decode_point_on_segment_constraint(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
) -> Result<PointOnSegmentConstraint, PointConstraintDecodeError> {
    let payload = group
        .records
        .iter()
        .find(|record| record.record_type == RECORD_BINDING_PAYLOAD)
        .map(|record| record.payload(&file.data))
        .ok_or(PointConstraintDecodeError::MissingPayloadRecord)?;
    if payload.len() < 12 {
        return Err(PointConstraintDecodeError::PayloadTooShort {
            byte_len: payload.len(),
            expected: 12,
        });
    }
    let t = read_f64(payload, 4);
    if !t.is_finite() {
        return Err(PointConstraintDecodeError::NonFiniteParameter);
    }
    decode_point_on_segment_constraint(file, groups, group).ok_or(
        PointConstraintDecodeError::UnsupportedOrMalformed {
            host_kind: group.header.kind(),
            payload_len: payload.len(),
        },
    )
}

pub(crate) fn try_decode_parameter_controlled_point(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
    anchors: &[Option<PointRecord>],
) -> Result<ParameterControlledPoint, ParameterControlledPointDecodeError> {
    if (group.header.kind()) != crate::format::GroupKind::ParameterControlledPoint {
        return Err(
            ParameterControlledPointDecodeError::NotParameterControlledPoint(group.header.kind()),
        );
    }

    let path =
        find_indexed_path(file, group).ok_or(ParameterControlledPointDecodeError::MissingPath)?;
    if path.refs.len() < 2 {
        return Err(ParameterControlledPointDecodeError::PathTooShort(
            path.refs.len(),
        ));
    }

    let source_group = groups
        .get(
            path.refs[0]
                .checked_sub(1)
                .ok_or(ParameterControlledPointDecodeError::MissingSourceGroup)?,
        )
        .ok_or(ParameterControlledPointDecodeError::MissingSourceGroup)?;
    let host_group = groups
        .get(
            path.refs[1]
                .checked_sub(1)
                .ok_or(ParameterControlledPointDecodeError::MissingHostGroup)?,
        )
        .ok_or(ParameterControlledPointDecodeError::MissingHostGroup)?;
    let (parameter_name, parameter_value, source_point_group_index, source_expr): (
        String,
        f64,
        Option<usize>,
        Option<FunctionExpr>,
    ) = if (source_group.header.kind()) == crate::format::GroupKind::Point {
        (
            decode_label_name(file, source_group)
                .ok_or(ParameterControlledPointDecodeError::InvalidSource)?,
            try_decode_parameter_control_value_for_group(file, groups, source_group)
                .ok()
                .ok_or(ParameterControlledPointDecodeError::InvalidSource)?,
            None,
            None,
        )
    } else if (source_group.header.kind()) == crate::format::GroupKind::ParameterAnchor {
        let (_name, value, point_group_index) =
            parameter_anchor_value(file, groups, source_group, anchors)
                .ok_or(ParameterControlledPointDecodeError::InvalidSource)?;
        (String::new(), value, Some(point_group_index), None)
    } else if (source_group.header.kind()) == crate::format::GroupKind::FunctionExpr {
        let expr = try_decode_function_expr(file, groups, source_group)
            .map_err(|_| ParameterControlledPointDecodeError::InvalidSource)?;
        let source_path = find_indexed_path(file, source_group)
            .ok_or(ParameterControlledPointDecodeError::InvalidSource)?;
        let mut parameters = BTreeMap::new();
        let mut source_point_group_index = None;
        let mut anchor_parameter_name = None;
        let mut anchor_parameter_value = None;
        for object_ref in &source_path.refs {
            let ref_group = groups
                .get(
                    object_ref
                        .checked_sub(1)
                        .ok_or(ParameterControlledPointDecodeError::InvalidSource)?,
                )
                .ok_or(ParameterControlledPointDecodeError::InvalidSource)?;
            match ref_group.header.kind() {
                crate::format::GroupKind::Point => {
                    let name = decode_label_name(file, ref_group)
                        .ok_or(ParameterControlledPointDecodeError::InvalidSource)?;
                    let value =
                        try_decode_parameter_control_value_for_group(file, groups, ref_group)
                            .ok()
                            .ok_or(ParameterControlledPointDecodeError::InvalidSource)?;
                    parameters.insert(name, value);
                }
                crate::format::GroupKind::ParameterAnchor => {
                    let (name, value, point_group_index) =
                        parameter_anchor_value(file, groups, ref_group, anchors)
                            .ok_or(ParameterControlledPointDecodeError::InvalidSource)?;
                    if !name.is_empty() {
                        anchor_parameter_name = Some(name.clone());
                        anchor_parameter_value = Some(value);
                        parameters.insert(name, value);
                    }
                    source_point_group_index.get_or_insert(point_group_index);
                }
                _ => {}
            }
        }
        let mut value = evaluate_expr_with_parameters(&expr, 0.0, &parameters)
            .ok_or(ParameterControlledPointDecodeError::InvalidSource)?;
        if anchor_parameter_name.is_some() && anchor_parameter_value.is_some() {
            value += anchor_parameter_value.unwrap();
        }
        (
            anchor_parameter_name.unwrap_or_default(),
            wrap_unit_interval(value),
            source_point_group_index,
            Some(expr),
        )
    } else {
        return Err(ParameterControlledPointDecodeError::InvalidSource);
    };

    match host_group.header.kind() {
        crate::format::GroupKind::Segment => {
            let host_path = find_indexed_path(file, host_group)
                .ok_or(ParameterControlledPointDecodeError::InvalidHostGeometry)?;
            if host_path.refs.len() != 2 {
                return Err(ParameterControlledPointDecodeError::InvalidHostGeometry);
            }
            let normalized = wrap_unit_interval(parameter_value);
            let start_group_index = host_path.refs[0]
                .checked_sub(1)
                .ok_or(ParameterControlledPointDecodeError::InvalidHostGeometry)?;
            let end_group_index = host_path.refs[1]
                .checked_sub(1)
                .ok_or(ParameterControlledPointDecodeError::InvalidHostGeometry)?;
            let start = anchors
                .get(start_group_index)
                .and_then(|value| value.clone())
                .ok_or(ParameterControlledPointDecodeError::InvalidHostGeometry)?;
            let end = anchors
                .get(end_group_index)
                .and_then(|value| value.clone())
                .ok_or(ParameterControlledPointDecodeError::InvalidHostGeometry)?;
            let position = lerp_point(&start, &end, normalized);
            Ok(ParameterControlledPoint {
                position,
                constraint: RawPointConstraint::Segment(PointOnSegmentConstraint {
                    start_group_index,
                    end_group_index,
                    t: normalized,
                }),
                parameter_name,
                source_point_group_index,
                source_expr: source_expr.clone(),
            })
        }
        crate::format::GroupKind::Polygon => {
            let host_path = find_indexed_path(file, host_group)
                .ok_or(ParameterControlledPointDecodeError::InvalidHostGeometry)?;
            let vertex_group_indices = host_path
                .refs
                .iter()
                .map(|vertex| vertex.checked_sub(1))
                .collect::<Option<Vec<_>>>()
                .ok_or(ParameterControlledPointDecodeError::InvalidHostGeometry)?;
            let vertices = vertex_group_indices
                .iter()
                .map(|group_index| anchors.get(*group_index).and_then(|value| value.clone()))
                .collect::<Option<Vec<_>>>()
                .ok_or(ParameterControlledPointDecodeError::InvalidHostGeometry)?;
            let (edge_index, t) = polygon_parameter_to_edge(&vertices, parameter_value)
                .ok_or(ParameterControlledPointDecodeError::InvalidHostGeometry)?;
            let position = resolve_polygon_boundary_point_raw(&vertices, edge_index, t)
                .ok_or(ParameterControlledPointDecodeError::InvalidHostGeometry)?;
            Ok(ParameterControlledPoint {
                position,
                constraint: RawPointConstraint::PolygonBoundary {
                    vertex_group_indices,
                    edge_index,
                    t,
                },
                parameter_name,
                source_point_group_index,
                source_expr: source_expr.clone(),
            })
        }
        crate::format::GroupKind::Circle => {
            let host_path = find_indexed_path(file, host_group)
                .ok_or(ParameterControlledPointDecodeError::InvalidHostGeometry)?;
            if host_path.refs.len() != 2 {
                return Err(ParameterControlledPointDecodeError::InvalidHostGeometry);
            }
            let center_group_index = host_path.refs[0]
                .checked_sub(1)
                .ok_or(ParameterControlledPointDecodeError::InvalidHostGeometry)?;
            let radius_group_index = host_path.refs[1]
                .checked_sub(1)
                .ok_or(ParameterControlledPointDecodeError::InvalidHostGeometry)?;
            let center = anchors
                .get(center_group_index)
                .and_then(|value| value.clone())
                .ok_or(ParameterControlledPointDecodeError::InvalidHostGeometry)?;
            let radius_point = anchors
                .get(radius_group_index)
                .and_then(|value| value.clone())
                .ok_or(ParameterControlledPointDecodeError::InvalidHostGeometry)?;
            let angle = std::f64::consts::TAU * parameter_value;
            let unit_x = angle.cos();
            let unit_y = angle.sin();
            let position = resolve_circle_point_raw(&center, &radius_point, unit_x, unit_y);
            Ok(ParameterControlledPoint {
                position,
                constraint: RawPointConstraint::Circle(PointOnCircleConstraint {
                    center_group_index,
                    radius_group_index,
                    unit_x,
                    unit_y,
                }),
                parameter_name,
                source_point_group_index,
                source_expr: source_expr.clone(),
            })
        }
        crate::format::GroupKind::ThreePointArc => {
            let host_path = find_indexed_path(file, host_group)
                .ok_or(ParameterControlledPointDecodeError::InvalidHostGeometry)?;
            if host_path.refs.len() != 3 {
                return Err(ParameterControlledPointDecodeError::InvalidHostGeometry);
            }
            let normalized = wrap_unit_interval(parameter_value);
            let start_group_index = host_path.refs[0]
                .checked_sub(1)
                .ok_or(ParameterControlledPointDecodeError::InvalidHostGeometry)?;
            let mid_group_index = host_path.refs[1]
                .checked_sub(1)
                .ok_or(ParameterControlledPointDecodeError::InvalidHostGeometry)?;
            let end_group_index = host_path.refs[2]
                .checked_sub(1)
                .ok_or(ParameterControlledPointDecodeError::InvalidHostGeometry)?;
            let start = anchors
                .get(start_group_index)
                .and_then(|value| value.clone())
                .ok_or(ParameterControlledPointDecodeError::InvalidHostGeometry)?;
            let mid = anchors
                .get(mid_group_index)
                .and_then(|value| value.clone())
                .ok_or(ParameterControlledPointDecodeError::InvalidHostGeometry)?;
            let end = anchors
                .get(end_group_index)
                .and_then(|value| value.clone())
                .ok_or(ParameterControlledPointDecodeError::InvalidHostGeometry)?;
            let position = point_on_three_point_arc(&start, &mid, &end, normalized)
                .ok_or(ParameterControlledPointDecodeError::InvalidHostGeometry)?;
            Ok(ParameterControlledPoint {
                position,
                constraint: RawPointConstraint::Arc(PointOnArcConstraint {
                    start_group_index,
                    mid_group_index,
                    end_group_index,
                    t: normalized,
                }),
                parameter_name,
                source_point_group_index,
                source_expr: source_expr.clone(),
            })
        }
        crate::format::GroupKind::ArcOnCircle => {
            let host_path = find_indexed_path(file, host_group)
                .ok_or(ParameterControlledPointDecodeError::InvalidHostGeometry)?;
            if host_path.refs.len() != 3 {
                return Err(ParameterControlledPointDecodeError::InvalidHostGeometry);
            }
            let normalized = wrap_unit_interval(parameter_value);
            let circle_group = groups
                .get(
                    host_path.refs[0]
                        .checked_sub(1)
                        .ok_or(ParameterControlledPointDecodeError::InvalidHostGeometry)?,
                )
                .ok_or(ParameterControlledPointDecodeError::InvalidHostGeometry)?;
            if !is_circle_group_kind(circle_group.header.kind()) {
                return Err(ParameterControlledPointDecodeError::InvalidHostGeometry);
            }
            let circle_path = find_indexed_path(file, circle_group)
                .ok_or(ParameterControlledPointDecodeError::InvalidHostGeometry)?;
            if circle_path.refs.len() != 2 {
                return Err(ParameterControlledPointDecodeError::InvalidHostGeometry);
            }
            let center_group_index = circle_path.refs[0]
                .checked_sub(1)
                .ok_or(ParameterControlledPointDecodeError::InvalidHostGeometry)?;
            let start_group_index = host_path.refs[1]
                .checked_sub(1)
                .ok_or(ParameterControlledPointDecodeError::InvalidHostGeometry)?;
            let end_group_index = host_path.refs[2]
                .checked_sub(1)
                .ok_or(ParameterControlledPointDecodeError::InvalidHostGeometry)?;
            let center = anchors
                .get(center_group_index)
                .and_then(|value| value.clone())
                .ok_or(ParameterControlledPointDecodeError::InvalidHostGeometry)?;
            let start = anchors
                .get(start_group_index)
                .and_then(|value| value.clone())
                .ok_or(ParameterControlledPointDecodeError::InvalidHostGeometry)?;
            let end = anchors
                .get(end_group_index)
                .and_then(|value| value.clone())
                .ok_or(ParameterControlledPointDecodeError::InvalidHostGeometry)?;
            let position = point_on_circle_arc(&center, &start, &end, normalized)
                .ok_or(ParameterControlledPointDecodeError::InvalidHostGeometry)?;
            Ok(ParameterControlledPoint {
                position,
                constraint: RawPointConstraint::CircleArc(PointOnCircleArcConstraint {
                    center_group_index,
                    start_group_index,
                    end_group_index,
                    t: normalized,
                }),
                parameter_name,
                source_point_group_index,
                source_expr: source_expr.clone(),
            })
        }
        crate::format::GroupKind::CenterArc => {
            let host_path = find_indexed_path(file, host_group)
                .ok_or(ParameterControlledPointDecodeError::InvalidHostGeometry)?;
            if host_path.refs.len() != 3 {
                return Err(ParameterControlledPointDecodeError::InvalidHostGeometry);
            }
            let normalized = wrap_unit_interval(parameter_value);
            let center_group_index = host_path.refs[0]
                .checked_sub(1)
                .ok_or(ParameterControlledPointDecodeError::InvalidHostGeometry)?;
            let start_group_index = host_path.refs[1]
                .checked_sub(1)
                .ok_or(ParameterControlledPointDecodeError::InvalidHostGeometry)?;
            let end_group_index = host_path.refs[2]
                .checked_sub(1)
                .ok_or(ParameterControlledPointDecodeError::InvalidHostGeometry)?;
            let center = anchors
                .get(center_group_index)
                .and_then(|value| value.clone())
                .ok_or(ParameterControlledPointDecodeError::InvalidHostGeometry)?;
            let start = anchors
                .get(start_group_index)
                .and_then(|value| value.clone())
                .ok_or(ParameterControlledPointDecodeError::InvalidHostGeometry)?;
            let end = anchors
                .get(end_group_index)
                .and_then(|value| value.clone())
                .ok_or(ParameterControlledPointDecodeError::InvalidHostGeometry)?;
            let reversed_t = 1.0 - normalized;
            let position = point_on_circle_arc(&center, &start, &end, reversed_t)
                .ok_or(ParameterControlledPointDecodeError::InvalidHostGeometry)?;
            Ok(ParameterControlledPoint {
                position,
                constraint: RawPointConstraint::CircleArc(PointOnCircleArcConstraint {
                    center_group_index,
                    start_group_index,
                    end_group_index,
                    t: reversed_t,
                }),
                parameter_name,
                source_point_group_index,
                source_expr,
            })
        }
        _ => Err(ParameterControlledPointDecodeError::InvalidHostGeometry),
    }
}

pub(crate) fn decode_coordinate_point(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
    anchors: &[Option<PointRecord>],
    graph: &Option<GraphTransform>,
) -> Option<CoordinatePoint> {
    let kind = group.header.kind();
    if !matches!(
        kind,
        crate::format::GroupKind::CoordinatePoint
            | crate::format::GroupKind::CoordinateExpressionPoint
            | crate::format::GroupKind::CoordinateExpressionPointAlt
            | crate::format::GroupKind::Unknown(20)
    ) {
        return None;
    }

    let path = find_indexed_path(file, group)?;
    if path.refs.len() < 2 {
        return None;
    }
    let calc_group = groups.get(path.refs[1].checked_sub(1)?)?;
    let expr = try_decode_function_expr(file, groups, calc_group).ok()?;

    match kind {
        crate::format::GroupKind::CoordinatePoint => {
            let parameter_group = groups.get(path.refs[0].checked_sub(1)?)?;
            let parameter_name = decode_label_name(file, parameter_group)?;
            let parameter_value =
                try_decode_parameter_control_value_for_group(file, groups, parameter_group).ok()?;
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
                source: CoordinatePointSource::Parameter(parameter_name),
                expr,
            })
        }
        crate::format::GroupKind::CoordinateExpressionPoint
        | crate::format::GroupKind::CoordinateExpressionPointAlt => {
            let source_group_index = path.refs[0].checked_sub(1)?;
            let source_position = anchors.get(source_group_index)?.clone()?;
            let source_world = to_world(&source_position, graph);
            let payload = group
                .records
                .iter()
                .find(|record| record.record_type == 0x07d3)
                .map(|record| record.payload(&file.data))?;
            let axis = match kind {
                crate::format::GroupKind::CoordinateExpressionPointAlt => {
                    crate::runtime::scene::CoordinateAxis::Horizontal
                }
                _ => match (payload.len() >= 24).then(|| read_u32(payload, 20)) {
                    Some(1) => crate::runtime::scene::CoordinateAxis::Vertical,
                    _ => crate::runtime::scene::CoordinateAxis::Horizontal,
                },
            };
            let parameter_group = first_path_group(file, groups, calc_group)?;
            let parameter_name = decode_label_name(file, parameter_group)?;
            let world = match axis {
                crate::runtime::scene::CoordinateAxis::Horizontal => {
                    let parameter_value =
                        try_decode_parameter_control_value_for_group(file, groups, parameter_group)
                            .ok()?;
                    let offset = evaluate_expr_with_parameters(
                        &expr,
                        0.0,
                        &BTreeMap::from([(parameter_name.clone(), parameter_value)]),
                    )?;
                    PointRecord {
                        x: source_world.x + offset,
                        y: source_world.y,
                    }
                }
                crate::runtime::scene::CoordinateAxis::Vertical => {
                    let parameter_value = source_world.x;
                    let y = evaluate_expr_with_parameters(
                        &expr,
                        0.0,
                        &BTreeMap::from([(parameter_name.clone(), parameter_value)]),
                    )?;
                    PointRecord {
                        x: parameter_value,
                        y,
                    }
                }
            };
            let position = if let Some(transform) = graph {
                to_raw_from_world(&world, transform)
            } else {
                world
            };

            Some(CoordinatePoint {
                position,
                source: CoordinatePointSource::SourcePoint {
                    source_group_index,
                    parameter_name,
                    axis,
                },
                expr,
            })
        }
        crate::format::GroupKind::Unknown(20) => {
            let source_group_index = path.refs[0].checked_sub(1)?;
            let source_position = anchors.get(source_group_index)?.clone()?;
            let source_world = to_world(&source_position, graph);
            let x_calc_group = groups.get(path.refs[1].checked_sub(1)?)?;
            let y_calc_group = groups.get(path.refs[2].checked_sub(1)?)?;
            let x_expr = try_decode_function_expr(file, groups, x_calc_group).ok()?;
            let y_expr = try_decode_function_expr(file, groups, y_calc_group).ok()?;

            let x_parameter_group = first_path_group(file, groups, x_calc_group)?;
            let y_parameter_group = first_path_group(file, groups, y_calc_group)?;
            let x_parameter_name = decode_label_name(file, x_parameter_group)?;
            let y_parameter_name = decode_label_name(file, y_parameter_group)?;
            let x_parameter_value =
                try_decode_parameter_control_value_for_group(file, groups, x_parameter_group)
                    .ok()?;
            let y_parameter_value =
                try_decode_parameter_control_value_for_group(file, groups, y_parameter_group)
                    .ok()?;
            let dx = evaluate_expr_with_parameters(
                &x_expr,
                0.0,
                &BTreeMap::from([(x_parameter_name.clone(), x_parameter_value)]),
            )?;
            let dy = evaluate_expr_with_parameters(
                &y_expr,
                0.0,
                &BTreeMap::from([(y_parameter_name.clone(), y_parameter_value)]),
            )?;
            let world = PointRecord {
                x: source_world.x + dx,
                y: source_world.y + dy,
            };
            let position = if let Some(transform) = graph {
                to_raw_from_world(&world, transform)
            } else {
                world
            };

            Some(CoordinatePoint {
                position,
                source: CoordinatePointSource::SourcePoint2d {
                    source_group_index,
                    x_parameter_name,
                    x_expr: x_expr.clone(),
                    y_parameter_name,
                    y_expr: y_expr.clone(),
                },
                expr: x_expr,
            })
        }
        _ => None,
    }
}

pub(crate) fn try_decode_point_constraint(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
    anchors: Option<&[Option<PointRecord>]>,
    graph: &Option<GraphTransform>,
) -> Result<RawPointConstraint, PointConstraintDecodeError> {
    if !group.header.kind().is_point_constraint() {
        return Err(PointConstraintDecodeError::NotPointConstraintKind(
            group.header.kind(),
        ));
    }

    let path =
        find_indexed_path(file, group).ok_or(PointConstraintDecodeError::MissingIndexedPath)?;
    let host_ref = path
        .refs
        .first()
        .copied()
        .filter(|ordinal| *ordinal > 0)
        .ok_or(PointConstraintDecodeError::MissingHostReference)?;
    let host_group = groups
        .get(host_ref - 1)
        .ok_or(PointConstraintDecodeError::MissingHostReference)?;
    let host_kind = host_group.header.kind();
    let payload = group
        .records
        .iter()
        .find(|record| record.record_type == RECORD_BINDING_PAYLOAD)
        .map(|record| record.payload(&file.data))
        .ok_or(PointConstraintDecodeError::MissingPayloadRecord)?;

    if payload.len() < 12 {
        return Err(PointConstraintDecodeError::PayloadTooShort {
            byte_len: payload.len(),
            expected: 12,
        });
    }
    if !read_f64(payload, 4).is_finite() {
        return Err(PointConstraintDecodeError::NonFiniteParameter);
    }

    let host_path = find_indexed_path(file, host_group)
        .ok_or(PointConstraintDecodeError::MissingIndexedPath)?;
    match host_kind {
        crate::format::GroupKind::Circle if host_path.refs.len() != 2 => {
            return Err(PointConstraintDecodeError::HostPathTooShort {
                host_kind,
                expected: 2,
                actual: host_path.refs.len(),
            });
        }
        crate::format::GroupKind::Polygon if host_path.refs.len() < 2 => {
            return Err(PointConstraintDecodeError::HostPathTooShort {
                host_kind,
                expected: 2,
                actual: host_path.refs.len(),
            });
        }
        crate::format::GroupKind::ThreePointArc
        | crate::format::GroupKind::ArcOnCircle
        | crate::format::GroupKind::CenterArc
            if host_path.refs.len() != 3 =>
        {
            return Err(PointConstraintDecodeError::HostPathTooShort {
                host_kind,
                expected: 3,
                actual: host_path.refs.len(),
            });
        }
        crate::format::GroupKind::FunctionPlot => {
            return try_decode_point_on_function_constraint(
                file, groups, host_group, payload, graph,
            );
        }
        _ => {}
    }

    if (group.header.kind()) == crate::format::GroupKind::PathPoint {
        return try_decode_path_point_constraint(file, groups, host_group, payload, anchors, graph);
    }

    if matches!(host_kind, crate::format::GroupKind::Segment) {
        return try_decode_point_on_segment_constraint(file, groups, group)
            .map(RawPointConstraint::Segment);
    }

    if matches!(host_kind, crate::format::GroupKind::Circle) {
        return try_decode_circle_point_constraint(file, host_group, payload);
    }

    if matches!(host_kind, crate::format::GroupKind::Polygon) {
        return try_decode_polygon_boundary_constraint(file, host_group, payload);
    }

    if matches!(
        host_kind,
        crate::format::GroupKind::ThreePointArc
            | crate::format::GroupKind::ArcOnCircle
            | crate::format::GroupKind::CenterArc
    ) {
        return try_decode_arc_family_constraint(file, groups, host_group, payload);
    }

    decode_point_constraint_impl(file, groups, group, anchors, graph).ok_or(
        PointConstraintDecodeError::UnsupportedOrMalformed {
            host_kind,
            payload_len: payload.len(),
        },
    )
}

fn decode_point_constraint_impl(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
    anchors: Option<&[Option<PointRecord>]>,
    graph: &Option<GraphTransform>,
) -> Option<RawPointConstraint> {
    if !group.header.kind().is_point_constraint() {
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
        .find(|record| record.record_type == RECORD_BINDING_PAYLOAD)
        .map(|record| record.payload(&file.data))?;

    if (group.header.kind()) == crate::format::GroupKind::PathPoint {
        return decode_path_point_constraint(file, groups, host_group, payload, anchors, graph);
    }

    match (host_kind, payload.len()) {
        (
            crate::format::GroupKind::SectorBoundary
            | crate::format::GroupKind::CircularSegmentBoundary,
            12,
        ) => {
            let normalized_t = read_f64(payload, 4);
            if !normalized_t.is_finite() {
                return None;
            }
            let points = decode_arc_boundary_polyline(file, groups, host_group, anchors?)?;
            let (segment_index, t) = locate_polyline_parameter_by_length(&points, normalized_t)?;
            Some(RawPointConstraint::Polyline {
                function_key: host_group.ordinal,
                points,
                segment_index,
                t,
            })
        }
        (crate::format::GroupKind::Circle, 20) => {
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
        (crate::format::GroupKind::Polygon, 20) => {
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
        (crate::format::GroupKind::FunctionPlot, 12) => {
            decode_point_on_function_constraint(file, groups, host_group, payload, graph)
        }
        (crate::format::GroupKind::ThreePointArc, 12) => {
            let host_path = find_indexed_path(file, host_group)?;
            if host_path.refs.len() != 3 {
                return None;
            }
            let t = read_f64(payload, 4);
            if !t.is_finite() {
                return None;
            }
            Some(RawPointConstraint::Arc(PointOnArcConstraint {
                start_group_index: host_path.refs[0].checked_sub(1)?,
                mid_group_index: host_path.refs[1].checked_sub(1)?,
                end_group_index: host_path.refs[2].checked_sub(1)?,
                t,
            }))
        }
        (crate::format::GroupKind::ArcOnCircle, 12) => {
            let host_path = find_indexed_path(file, host_group)?;
            if host_path.refs.len() != 3 {
                return None;
            }
            let circle_group = groups.get(host_path.refs[0].checked_sub(1)?)?;
            if !is_circle_group_kind(circle_group.header.kind()) {
                return None;
            }
            let circle_path = find_indexed_path(file, circle_group)?;
            if circle_path.refs.len() != 2 {
                return None;
            }
            let t = read_f64(payload, 4);
            if !t.is_finite() {
                return None;
            }
            Some(RawPointConstraint::CircleArc(PointOnCircleArcConstraint {
                center_group_index: circle_path.refs[0].checked_sub(1)?,
                start_group_index: host_path.refs[1].checked_sub(1)?,
                end_group_index: host_path.refs[2].checked_sub(1)?,
                t,
            }))
        }
        (crate::format::GroupKind::CenterArc, 12) => {
            let host_path = find_indexed_path(file, host_group)?;
            if host_path.refs.len() != 3 {
                return None;
            }
            let t = read_f64(payload, 4);
            if !t.is_finite() {
                return None;
            }
            let reversed_t = 1.0 - t;
            Some(RawPointConstraint::CircleArc(PointOnCircleArcConstraint {
                center_group_index: host_path.refs[0].checked_sub(1)?,
                start_group_index: host_path.refs[1].checked_sub(1)?,
                end_group_index: host_path.refs[2].checked_sub(1)?,
                t: reversed_t,
            }))
        }
        _ => {
            decode_point_on_segment_constraint(file, groups, group).map(RawPointConstraint::Segment)
        }
    }
}

fn decode_path_point_constraint(
    file: &GspFile,
    groups: &[ObjectGroup],
    host_group: &ObjectGroup,
    payload: &[u8],
    anchors: Option<&[Option<PointRecord>]>,
    graph: &Option<GraphTransform>,
) -> Option<RawPointConstraint> {
    if payload.len() < 12 {
        return None;
    }

    let normalized_t = read_f64(payload, 4);
    if !normalized_t.is_finite() {
        return None;
    }

    match host_group.header.kind() {
        crate::format::GroupKind::Segment => {
            let host_path = find_indexed_path(file, host_group)?;
            if host_path.refs.len() != 2 {
                return None;
            }
            Some(RawPointConstraint::Segment(PointOnSegmentConstraint {
                start_group_index: host_path.refs[0].checked_sub(1)?,
                end_group_index: host_path.refs[1].checked_sub(1)?,
                t: wrap_unit_interval(normalized_t),
            }))
        }
        crate::format::GroupKind::Circle => {
            let host_path = find_indexed_path(file, host_group)?;
            if host_path.refs.len() != 2 {
                return None;
            }
            let angle = std::f64::consts::TAU * wrap_unit_interval(normalized_t);
            Some(RawPointConstraint::Circle(PointOnCircleConstraint {
                center_group_index: host_path.refs[0].checked_sub(1)?,
                radius_group_index: host_path.refs[1].checked_sub(1)?,
                unit_x: angle.cos(),
                unit_y: angle.sin(),
            }))
        }
        crate::format::GroupKind::Polygon => {
            let host_path = find_indexed_path(file, host_group)?;
            if host_path.refs.len() < 2 {
                return None;
            }
            let anchors = anchors?;
            let vertex_group_indices = host_path
                .refs
                .iter()
                .map(|vertex| vertex.checked_sub(1))
                .collect::<Option<Vec<_>>>()?;
            let vertices = vertex_group_indices
                .iter()
                .map(|group_index| anchors.get(*group_index)?.clone())
                .collect::<Option<Vec<_>>>()?;
            let (edge_index, t) = polygon_parameter_to_edge(&vertices, normalized_t)?;
            Some(RawPointConstraint::PolygonBoundary {
                vertex_group_indices,
                edge_index,
                t,
            })
        }
        crate::format::GroupKind::SectorBoundary
        | crate::format::GroupKind::CircularSegmentBoundary => {
            let points = decode_arc_boundary_polyline(file, groups, host_group, anchors?)?;
            let (segment_index, t) = locate_polyline_parameter_by_length(&points, normalized_t)?;
            Some(RawPointConstraint::Polyline {
                function_key: host_group.ordinal,
                points,
                segment_index,
                t,
            })
        }
        crate::format::GroupKind::FunctionPlot => {
            decode_point_on_function_constraint(file, groups, host_group, payload, graph)
        }
        crate::format::GroupKind::ThreePointArc => {
            let host_path = find_indexed_path(file, host_group)?;
            if host_path.refs.len() != 3 {
                return None;
            }
            Some(RawPointConstraint::Arc(PointOnArcConstraint {
                start_group_index: host_path.refs[0].checked_sub(1)?,
                mid_group_index: host_path.refs[1].checked_sub(1)?,
                end_group_index: host_path.refs[2].checked_sub(1)?,
                t: wrap_unit_interval(normalized_t),
            }))
        }
        crate::format::GroupKind::ArcOnCircle => {
            let host_path = find_indexed_path(file, host_group)?;
            if host_path.refs.len() != 3 {
                return None;
            }
            let circle_group = groups.get(host_path.refs[0].checked_sub(1)?)?;
            if !is_circle_group_kind(circle_group.header.kind()) {
                return None;
            }
            let circle_path = find_indexed_path(file, circle_group)?;
            if circle_path.refs.len() != 2 {
                return None;
            }
            Some(RawPointConstraint::CircleArc(PointOnCircleArcConstraint {
                center_group_index: circle_path.refs[0].checked_sub(1)?,
                start_group_index: host_path.refs[1].checked_sub(1)?,
                end_group_index: host_path.refs[2].checked_sub(1)?,
                t: wrap_unit_interval(normalized_t),
            }))
        }
        crate::format::GroupKind::CenterArc => {
            let host_path = find_indexed_path(file, host_group)?;
            if host_path.refs.len() != 3 {
                return None;
            }
            Some(RawPointConstraint::CircleArc(PointOnCircleArcConstraint {
                center_group_index: host_path.refs[0].checked_sub(1)?,
                start_group_index: host_path.refs[1].checked_sub(1)?,
                end_group_index: host_path.refs[2].checked_sub(1)?,
                t: 1.0 - wrap_unit_interval(normalized_t),
            }))
        }
        _ => None,
    }
}

fn try_decode_circle_point_constraint(
    file: &GspFile,
    host_group: &ObjectGroup,
    payload: &[u8],
) -> Result<RawPointConstraint, PointConstraintDecodeError> {
    let host_path = find_indexed_path(file, host_group)
        .filter(|path| path.refs.len() == 2)
        .ok_or(PointConstraintDecodeError::InvalidCircleHostPath)?;
    let unit_x = read_f64(payload, 4);
    let unit_y = read_f64(payload, 12);
    if !unit_x.is_finite() || !unit_y.is_finite() {
        return Err(PointConstraintDecodeError::NonFiniteCircleUnit);
    }
    Ok(RawPointConstraint::Circle(PointOnCircleConstraint {
        center_group_index: host_path.refs[0]
            .checked_sub(1)
            .ok_or(PointConstraintDecodeError::InvalidCircleHostPath)?,
        radius_group_index: host_path.refs[1]
            .checked_sub(1)
            .ok_or(PointConstraintDecodeError::InvalidCircleHostPath)?,
        unit_x,
        unit_y,
    }))
}

fn try_decode_polygon_boundary_constraint(
    file: &GspFile,
    host_group: &ObjectGroup,
    payload: &[u8],
) -> Result<RawPointConstraint, PointConstraintDecodeError> {
    let host_path = find_indexed_path(file, host_group)
        .filter(|path| path.refs.len() >= 2)
        .ok_or(PointConstraintDecodeError::InvalidPolygonHostPath)?;
    let t = read_f64(payload, 4);
    if !t.is_finite() {
        return Err(PointConstraintDecodeError::NonFiniteParameter);
    }
    let vertex_group_indices = host_path
        .refs
        .iter()
        .map(|vertex| vertex.checked_sub(1))
        .collect::<Option<Vec<_>>>()
        .ok_or(PointConstraintDecodeError::InvalidPolygonHostPath)?;
    let edge_index = decode_polygon_edge_index(host_path.refs.len(), payload)
        .ok_or(PointConstraintDecodeError::InvalidPolygonEdgeIndex)?;
    Ok(RawPointConstraint::PolygonBoundary {
        vertex_group_indices,
        edge_index,
        t,
    })
}

fn try_decode_arc_family_constraint(
    file: &GspFile,
    groups: &[ObjectGroup],
    host_group: &ObjectGroup,
    payload: &[u8],
) -> Result<RawPointConstraint, PointConstraintDecodeError> {
    let host_kind = host_group.header.kind();
    let host_path = find_indexed_path(file, host_group)
        .filter(|path| path.refs.len() == 3)
        .ok_or(PointConstraintDecodeError::InvalidArcHostPath(host_kind))?;
    let t = read_f64(payload, 4);
    if !t.is_finite() {
        return Err(PointConstraintDecodeError::NonFiniteParameter);
    }
    match host_kind {
        crate::format::GroupKind::ThreePointArc => {
            Ok(RawPointConstraint::Arc(PointOnArcConstraint {
                start_group_index: host_path.refs[0]
                    .checked_sub(1)
                    .ok_or(PointConstraintDecodeError::InvalidArcHostPath(host_kind))?,
                mid_group_index: host_path.refs[1]
                    .checked_sub(1)
                    .ok_or(PointConstraintDecodeError::InvalidArcHostPath(host_kind))?,
                end_group_index: host_path.refs[2]
                    .checked_sub(1)
                    .ok_or(PointConstraintDecodeError::InvalidArcHostPath(host_kind))?,
                t,
            }))
        }
        crate::format::GroupKind::ArcOnCircle => {
            let circle_group = groups
                .get(
                    host_path.refs[0]
                        .checked_sub(1)
                        .ok_or(PointConstraintDecodeError::ArcHostMissingCircle)?,
                )
                .ok_or(PointConstraintDecodeError::ArcHostMissingCircle)?;
            if !is_circle_group_kind(circle_group.header.kind()) {
                return Err(PointConstraintDecodeError::ArcHostMissingCircle);
            }
            let circle_path = find_indexed_path(file, circle_group)
                .filter(|path| path.refs.len() == 2)
                .ok_or(PointConstraintDecodeError::InvalidArcCirclePath)?;
            Ok(RawPointConstraint::CircleArc(PointOnCircleArcConstraint {
                center_group_index: circle_path.refs[0]
                    .checked_sub(1)
                    .ok_or(PointConstraintDecodeError::InvalidArcCirclePath)?,
                start_group_index: host_path.refs[1]
                    .checked_sub(1)
                    .ok_or(PointConstraintDecodeError::InvalidArcHostPath(host_kind))?,
                end_group_index: host_path.refs[2]
                    .checked_sub(1)
                    .ok_or(PointConstraintDecodeError::InvalidArcHostPath(host_kind))?,
                t,
            }))
        }
        crate::format::GroupKind::CenterArc => {
            Ok(RawPointConstraint::CircleArc(PointOnCircleArcConstraint {
                center_group_index: host_path.refs[0]
                    .checked_sub(1)
                    .ok_or(PointConstraintDecodeError::InvalidArcHostPath(host_kind))?,
                start_group_index: host_path.refs[1]
                    .checked_sub(1)
                    .ok_or(PointConstraintDecodeError::InvalidArcHostPath(host_kind))?,
                end_group_index: host_path.refs[2]
                    .checked_sub(1)
                    .ok_or(PointConstraintDecodeError::InvalidArcHostPath(host_kind))?,
                t: 1.0 - t,
            }))
        }
        _ => Err(PointConstraintDecodeError::UnsupportedOrMalformed {
            host_kind,
            payload_len: payload.len(),
        }),
    }
}

fn try_decode_path_point_constraint(
    file: &GspFile,
    groups: &[ObjectGroup],
    host_group: &ObjectGroup,
    payload: &[u8],
    anchors: Option<&[Option<PointRecord>]>,
    graph: &Option<GraphTransform>,
) -> Result<RawPointConstraint, PointConstraintDecodeError> {
    if payload.len() < 12 {
        return Err(PointConstraintDecodeError::PayloadTooShort {
            byte_len: payload.len(),
            expected: 12,
        });
    }
    let normalized_t = read_f64(payload, 4);
    if !normalized_t.is_finite() {
        return Err(PointConstraintDecodeError::NonFiniteParameter);
    }
    if matches!(
        host_group.header.kind(),
        crate::format::GroupKind::SectorBoundary
            | crate::format::GroupKind::CircularSegmentBoundary
            | crate::format::GroupKind::Polygon
    ) && anchors.is_none()
    {
        return Err(PointConstraintDecodeError::MissingAnchors);
    }
    decode_path_point_constraint(file, groups, host_group, payload, anchors, graph).ok_or(
        PointConstraintDecodeError::UnsupportedOrMalformed {
            host_kind: host_group.header.kind(),
            payload_len: payload.len(),
        },
    )
}

fn decode_arc_boundary_polyline(
    file: &GspFile,
    groups: &[ObjectGroup],
    host_group: &ObjectGroup,
    anchors: &[Option<PointRecord>],
) -> Option<Vec<PointRecord>> {
    let (center, [start, mid, end], starts_from_end, complement) =
        resolve_boundary_arc_geometry(file, groups, host_group, anchors)?;
    let arc_points = if complement {
        sample_three_point_arc_complement(&start, &mid, &end, ARC_BOUNDARY_SUBDIVISIONS)?
    } else {
        sample_three_point_arc(&start, &mid, &end, ARC_BOUNDARY_SUBDIVISIONS)?
    };
    match host_group.header.kind() {
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

fn resolve_boundary_arc_geometry(
    file: &GspFile,
    groups: &[ObjectGroup],
    host_group: &ObjectGroup,
    anchors: &[Option<PointRecord>],
) -> Option<(Option<PointRecord>, [PointRecord; 3], bool, bool)> {
    let path = find_indexed_path(file, host_group)?;
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
            let circle_path = find_indexed_path(file, circle_group)?;
            if circle_path.refs.len() != 2 {
                return None;
            }
            let center = anchors.get(circle_path.refs[0].checked_sub(1)?)?.clone()?;
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
                (host_group.header.kind()) == crate::format::GroupKind::CircularSegmentBoundary,
            ))
        }
        _ => None,
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
    let descriptor_record = host_group
        .records
        .iter()
        .find(|record| record.record_type == 0x0902)?;
    let descriptor =
        try_decode_function_plot_descriptor(descriptor_record.payload(&file.data)).ok()?;
    let expr = try_decode_function_expr(file, groups, definition_group).ok()?;
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

fn try_decode_point_on_function_constraint(
    file: &GspFile,
    groups: &[ObjectGroup],
    host_group: &ObjectGroup,
    payload: &[u8],
    graph: &Option<GraphTransform>,
) -> Result<RawPointConstraint, PointConstraintDecodeError> {
    if payload.len() < 12 {
        return Err(PointConstraintDecodeError::PayloadTooShort {
            byte_len: payload.len(),
            expected: 12,
        });
    }
    let normalized_t = read_f64(payload, 4);
    if !normalized_t.is_finite() {
        return Err(PointConstraintDecodeError::NonFiniteParameter);
    }
    if graph.is_none() {
        return Err(PointConstraintDecodeError::MissingGraphTransform);
    }
    let path = find_indexed_path(file, host_group)
        .ok_or(PointConstraintDecodeError::MissingIndexedPath)?;
    let definition_group = groups
        .get(
            *path
                .refs
                .first()
                .ok_or(PointConstraintDecodeError::MissingHostReference)?
                - 1,
        )
        .ok_or(PointConstraintDecodeError::MissingHostReference)?;
    let descriptor_record = host_group
        .records
        .iter()
        .find(|record| record.record_type == RECORD_FUNCTION_PLOT_DESCRIPTOR)
        .ok_or(PointConstraintDecodeError::MissingFunctionPlotDescriptor)?;
    try_decode_function_plot_descriptor(descriptor_record.payload(&file.data)).map_err(
        |error| PointConstraintDecodeError::InvalidFunctionPlotDescriptor(error.to_string()),
    )?;
    try_decode_function_expr(file, groups, definition_group)
        .map_err(|error| PointConstraintDecodeError::InvalidFunctionExpr(error.to_string()))?;
    decode_point_on_function_constraint(file, groups, host_group, payload, graph)
        .ok_or(PointConstraintDecodeError::PolylineParameterUnavailable)
}

fn locate_polyline_parameter(points: &[PointRecord], normalized_t: f64) -> Option<(usize, f64)> {
    if points.len() < 2 {
        return None;
    }

    let wrapped_t = wrap_unit_interval(normalized_t);
    let scaled = wrapped_t * (points.len() - 1) as f64;
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
