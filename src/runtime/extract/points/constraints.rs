use std::collections::BTreeMap;

use super::super::decode::{
    decode_label_name, find_indexed_path, is_circle_group_kind,
    try_decode_parameter_control_value_for_group,
};
use super::anchors::{
    resolve_circle_like_raw, resolve_circle_point_raw, resolve_polygon_boundary_point_raw,
};
use super::{
    decode_non_graph_parameter_value_for_group, editable_non_graph_parameter_name_for_group,
};
use crate::format::{GroupKind, GspFile, ObjectGroup, PointRecord, read_f64, read_u32};
use crate::runtime::functions::{
    FunctionAst, FunctionExpr, evaluate_expr_with_parameters, try_decode_embedded_calculate_expr,
    try_decode_function_expr, try_decode_function_expr_with_inlined_refs,
    try_decode_function_plot_descriptor, try_decode_parameter_control_expr,
};
use crate::runtime::geometry::{
    GraphTransform, arc_on_circle_control_points, lerp_point, locate_polyline_parameter_by_length,
    point_on_circle_arc, point_on_three_point_arc, sample_three_point_arc,
    sample_three_point_arc_complement, three_point_arc_geometry, to_raw_from_world, to_world,
};
use crate::runtime::payload_consts::{RECORD_BINDING_PAYLOAD, RECORD_FUNCTION_PLOT_DESCRIPTOR};
use crate::runtime::scene::LineLikeKind;
use thiserror::Error;

pub(crate) struct PointOnSegmentConstraint {
    pub(crate) start_group_index: usize,
    pub(crate) end_group_index: usize,
    pub(crate) t: f64,
    pub(crate) line_like_kind: LineLikeKind,
}

pub(crate) struct PointOnCircleConstraint {
    pub(crate) center_group_index: usize,
    pub(crate) radius_group_index: usize,
    pub(crate) unit_x: f64,
    pub(crate) unit_y: f64,
}

pub(crate) struct PointOnCircularConstraint {
    pub(crate) circle_group_index: usize,
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
    ConstructedLine {
        host_group_index: usize,
        t: f64,
        line_like_kind: LineLikeKind,
    },
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
    TranslatedPolygonBoundary {
        vertex_group_indices: Vec<usize>,
        vector_start_group_index: usize,
        vector_end_group_index: usize,
        edge_index: usize,
        t: f64,
    },
    Circle(PointOnCircleConstraint),
    Circular(PointOnCircularConstraint),
    CircleArc(PointOnCircleArcConstraint),
    Arc(PointOnArcConstraint),
}

pub(crate) struct ParameterControlledPoint {
    pub(crate) position: PointRecord,
    pub(crate) constraint: RawPointConstraint,
    pub(crate) parameter_name: String,
    pub(crate) source_point_group_index: Option<usize>,
    pub(crate) source_parameter_segment_group_indices: Option<(usize, usize)>,
    pub(crate) source_expr: Option<FunctionExpr>,
    pub(crate) source_expr_absolute_parameter: bool,
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

pub(crate) struct LegacyCoordinateConstructPoint {
    pub(crate) position: PointRecord,
    pub(crate) first_source_group_index: usize,
    pub(crate) second_source_group_index: usize,
    pub(crate) first_axis_start_group_index: usize,
    pub(crate) first_axis_end_group_index: usize,
    pub(crate) second_axis_start_group_index: usize,
    pub(crate) second_axis_end_group_index: usize,
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

fn segment_projection_parameter(
    point: &PointRecord,
    start: &PointRecord,
    end: &PointRecord,
) -> Option<f64> {
    gsp_runtime_core::project_to_line_like(
        gsp_runtime_core::Point {
            x: point.x,
            y: point.y,
        },
        gsp_runtime_core::Point {
            x: start.x,
            y: start.y,
        },
        gsp_runtime_core::Point { x: end.x, y: end.y },
        gsp_runtime_core::LineKind::Segment,
    )
    .map(|projection| projection.t)
}

fn parameter_anchor_value(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
    anchors: &[Option<PointRecord>],
) -> Option<(String, f64, usize, Option<(usize, usize)>)> {
    let path = find_indexed_path(file, group)?;
    let point_group_index = path.refs.first()?.checked_sub(1)?;
    let point_group = groups.get(point_group_index)?;
    let parameter_segment = path
        .refs
        .get(1)
        .and_then(|ordinal| groups.get(ordinal.checked_sub(1)?))
        .filter(|group| group.header.kind().is_line_like())
        .and_then(|segment_group| {
            let segment_path = find_indexed_path(file, segment_group)?;
            Some((
                segment_path.refs.first()?.checked_sub(1)?,
                segment_path.refs.get(1)?.checked_sub(1)?,
            ))
        });
    if let Some((start_group_index, end_group_index)) = parameter_segment {
        let point = anchors.get(point_group_index)?.as_ref()?;
        let start = anchors.get(start_group_index)?.as_ref()?;
        let end = anchors.get(end_group_index)?.as_ref()?;
        let t = segment_projection_parameter(point, start, end)?;
        let name = decode_label_name(file, group)
            .or_else(|| decode_label_name(file, point_group))
            .unwrap_or_default();
        return Some((
            name,
            t,
            point_group_index,
            Some((start_group_index, end_group_index)),
        ));
    }
    let t = match try_decode_point_constraint(file, groups, point_group, None, &None).ok()? {
        RawPointConstraint::Segment(constraint) => constraint.t,
        RawPointConstraint::ConstructedLine { t, .. } => t,
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
        RawPointConstraint::TranslatedPolygonBoundary {
            edge_index,
            t,
            vertex_group_indices,
            ..
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
        RawPointConstraint::Circular(_) => return None,
        RawPointConstraint::CircleArc(_) => return None,
        RawPointConstraint::Arc(_) => return None,
        RawPointConstraint::Polyline { .. } => return None,
    };
    let name = decode_label_name(file, group)
        .or_else(|| decode_label_name(file, point_group))
        .unwrap_or_default();
    Some((name, wrap_unit_interval(t), point_group_index, None))
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
    let angle_expr = [
        try_decode_embedded_calculate_expr(file, groups, calc_group),
        try_decode_function_expr(file, groups, calc_group),
    ]
    .into_iter()
    .filter_map(Result::ok)
    .find(|expr| {
        evaluate_expr_with_parameters(expr, 0.0, &BTreeMap::from([(parameter_name.clone(), n)]))
            .is_some_and(|value| (value - (360.0 / n)).abs() < 1e-6)
    })?;
    (n.abs() >= 1.0).then_some((center_group_index, angle_expr, parameter_name, n))
}

pub(crate) fn regular_polygon_angle_expr_for_calc_group(
    file: &GspFile,
    groups: &[ObjectGroup],
    calc_group: &ObjectGroup,
) -> Option<(FunctionExpr, String, f64)> {
    if (calc_group.header.kind()) != crate::format::GroupKind::FunctionExpr {
        return None;
    }
    let rotation_group = groups.iter().find(|group| {
        (group.header.kind()) == crate::format::GroupKind::ParameterRotation
            && find_indexed_path(file, group)
                .is_some_and(|path| path.refs.get(2).copied() == Some(calc_group.ordinal))
    })?;
    let iter_group = groups.iter().find(|group| {
        (group.header.kind()) == crate::format::GroupKind::RegularPolygonIteration
            && find_indexed_path(file, group)
                .is_some_and(|path| path.refs.contains(&rotation_group.ordinal))
    })?;
    let path = find_indexed_path(file, iter_group)?;
    let seed_group = path
        .refs
        .iter()
        .filter_map(|ordinal| groups.get(ordinal.checked_sub(1)?))
        .find(|group| (group.header.kind()) == crate::format::GroupKind::ParameterRotation)?;
    if seed_group.ordinal != rotation_group.ordinal {
        return None;
    }
    let (_center_group_index, angle_expr, parameter_name, n) =
        regular_polygon_iteration_step(file, groups, iter_group)?;
    Some((angle_expr, parameter_name, n))
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
            if payload.len() < 40 {
                return None;
            }

            let angle_radians = read_f64(payload, 20);
            let raw_distance = read_f64(payload, 32);
            if !angle_radians.is_finite() || !raw_distance.is_finite() {
                return None;
            }

            let angle_radians = if angle_radians.abs() > std::f64::consts::TAU {
                angle_radians.to_radians()
            } else {
                angle_radians
            };
            Some(TranslatedPointConstraint {
                origin_group_index,
                dx: raw_distance * angle_radians.cos(),
                dy: -raw_distance * angle_radians.sin(),
            })
        }
        crate::format::GroupKind::CartesianOffsetPoint => {
            if payload.len() < 40 {
                return None;
            }
            let raw_dx = read_f64(payload, 4);
            let raw_dy = read_f64(payload, 24);
            if !raw_dx.is_finite() || !raw_dy.is_finite() {
                return None;
            }
            Some(TranslatedPointConstraint {
                origin_group_index,
                dx: raw_dx,
                dy: -raw_dy,
            })
        }
        _ => None,
    }
}

fn decode_point_on_line_like_constraint(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
) -> Option<RawPointConstraint> {
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
    let payload = group
        .records
        .iter()
        .find(|record| record.record_type == 0x07d3 && record.length == 12)
        .map(|record| record.payload(&file.data))?;
    let t = read_f64(payload, 4);
    if !t.is_finite() {
        return None;
    }

    match host_group.header.kind() {
        crate::format::GroupKind::Segment
        | crate::format::GroupKind::Line
        | crate::format::GroupKind::Ray
        | crate::format::GroupKind::MeasurementLine
        | crate::format::GroupKind::GraphMeasurementSegment => {
            let host_path = find_indexed_path(file, host_group)?;
            if host_path.refs.len() != 2 {
                return None;
            }
            let line_like_kind = match host_group.header.kind() {
                crate::format::GroupKind::Segment => LineLikeKind::Segment,
                crate::format::GroupKind::Line => LineLikeKind::Line,
                crate::format::GroupKind::Ray => LineLikeKind::Ray,
                crate::format::GroupKind::MeasurementLine => LineLikeKind::Segment,
                crate::format::GroupKind::GraphMeasurementSegment => LineLikeKind::Segment,
                _ => unreachable!(),
            };
            let (start_group_index, end_group_index) =
                if host_group.header.kind() == crate::format::GroupKind::GraphMeasurementSegment {
                    let line_group = groups.get(host_path.refs[1].checked_sub(1)?)?;
                    let line_path = find_indexed_path(file, line_group)?;
                    if line_path.refs.len() != 2 {
                        return None;
                    }
                    let end_ordinal = if line_path.refs[0] == host_path.refs[0] {
                        line_path.refs[1]
                    } else {
                        line_path.refs[0]
                    };
                    (
                        host_path.refs[0].checked_sub(1)?,
                        end_ordinal.checked_sub(1)?,
                    )
                } else {
                    (
                        host_path.refs[0].checked_sub(1)?,
                        host_path.refs[1].checked_sub(1)?,
                    )
                };
            Some(RawPointConstraint::Segment(PointOnSegmentConstraint {
                start_group_index,
                end_group_index,
                t,
                line_like_kind,
            }))
        }
        crate::format::GroupKind::PerpendicularLine
        | crate::format::GroupKind::ParallelLine
        | crate::format::GroupKind::AngleBisectorRay => Some(RawPointConstraint::ConstructedLine {
            host_group_index,
            t,
            line_like_kind: match host_group.header.kind() {
                crate::format::GroupKind::AngleBisectorRay => LineLikeKind::Ray,
                _ => LineLikeKind::Line,
            },
        }),
        crate::format::GroupKind::Rotation => Some(RawPointConstraint::ConstructedLine {
            host_group_index,
            t,
            line_like_kind: transformed_line_like_kind(file, groups, host_group)?,
        }),
        _ => None,
    }
}

fn transformed_line_like_kind(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
) -> Option<LineLikeKind> {
    let path = find_indexed_path(file, group)?;
    let source_group = groups.get(path.refs.first()?.checked_sub(1)?)?;
    match source_group.header.kind() {
        GroupKind::Segment | GroupKind::MeasurementLine | GroupKind::GraphMeasurementSegment => {
            Some(LineLikeKind::Segment)
        }
        GroupKind::Line => Some(LineLikeKind::Line),
        GroupKind::Ray => Some(LineLikeKind::Ray),
        _ => None,
    }
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
    let (
        parameter_name,
        parameter_value,
        source_point_group_index,
        source_parameter_segment_group_indices,
        source_expr,
        source_expr_absolute_parameter,
    ): (
        String,
        f64,
        Option<usize>,
        Option<(usize, usize)>,
        Option<FunctionExpr>,
        bool,
    ) = if (source_group.header.kind()) == crate::format::GroupKind::Point {
        (
            decode_label_name(file, source_group)
                .ok_or(ParameterControlledPointDecodeError::InvalidSource)?,
            try_decode_parameter_control_value_for_group(file, groups, source_group)
                .ok()
                .ok_or(ParameterControlledPointDecodeError::InvalidSource)?,
            None,
            None,
            None,
            false,
        )
    } else if (source_group.header.kind()) == crate::format::GroupKind::ParameterAnchor {
        let (_name, value, point_group_index, segment_group_indices) =
            parameter_anchor_value(file, groups, source_group, anchors)
                .ok_or(ParameterControlledPointDecodeError::InvalidSource)?;
        (
            String::new(),
            value,
            Some(point_group_index),
            segment_group_indices,
            None,
            false,
        )
    } else if (source_group.header.kind()) == crate::format::GroupKind::FunctionExpr {
        let (expr, source_expr_absolute_parameter) =
            try_decode_parameter_control_expr(file, groups, source_group)
                .map_err(|_| ParameterControlledPointDecodeError::InvalidSource)?;
        let source_path = find_indexed_path(file, source_group)
            .ok_or(ParameterControlledPointDecodeError::InvalidSource)?;
        let mut parameters = BTreeMap::new();
        let mut source_point_group_index = None;
        let mut source_parameter_segment_group_indices = None;
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
                    let (name, value, point_group_index, segment_group_indices) =
                        parameter_anchor_value(file, groups, ref_group, anchors)
                            .ok_or(ParameterControlledPointDecodeError::InvalidSource)?;
                    if !name.is_empty() {
                        anchor_parameter_name = Some(name.clone());
                        anchor_parameter_value = Some(value);
                        parameters.insert(name, value);
                    }
                    source_point_group_index.get_or_insert(point_group_index);
                    if let Some(segment_group_indices) = segment_group_indices {
                        source_parameter_segment_group_indices.get_or_insert(segment_group_indices);
                    }
                }
                _ => {}
            }
        }
        let mut value = evaluate_expr_with_parameters(&expr, 0.0, &parameters)
            .ok_or(ParameterControlledPointDecodeError::InvalidSource)?;
        if !source_expr_absolute_parameter
            && let (Some(_), Some(anchor_value)) =
                (anchor_parameter_name.as_ref(), anchor_parameter_value)
        {
            value += anchor_value;
        }
        (
            anchor_parameter_name.unwrap_or_default(),
            wrap_unit_interval(value),
            source_point_group_index,
            source_parameter_segment_group_indices,
            Some(expr),
            source_expr_absolute_parameter,
        )
    } else {
        return Err(ParameterControlledPointDecodeError::InvalidSource);
    };

    match host_group.header.kind() {
        crate::format::GroupKind::Segment
        | crate::format::GroupKind::Line
        | crate::format::GroupKind::Ray => {
            let host_path = find_indexed_path(file, host_group)
                .ok_or(ParameterControlledPointDecodeError::InvalidHostGeometry)?;
            if host_path.refs.len() != 2 {
                return Err(ParameterControlledPointDecodeError::InvalidHostGeometry);
            }
            let line_like_kind = match host_group.header.kind() {
                crate::format::GroupKind::Segment => LineLikeKind::Segment,
                crate::format::GroupKind::Line => LineLikeKind::Line,
                crate::format::GroupKind::Ray => LineLikeKind::Ray,
                _ => unreachable!(),
            };
            let t = match line_like_kind {
                LineLikeKind::Segment => wrap_unit_interval(parameter_value),
                LineLikeKind::Line => parameter_value,
                LineLikeKind::Ray => parameter_value.max(0.0),
            };
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
            let position = lerp_point(&start, &end, t);
            Ok(ParameterControlledPoint {
                position,
                constraint: RawPointConstraint::Segment(PointOnSegmentConstraint {
                    start_group_index,
                    end_group_index,
                    t,
                    line_like_kind,
                }),
                parameter_name,
                source_point_group_index,
                source_parameter_segment_group_indices,
                source_expr: source_expr.clone(),
                source_expr_absolute_parameter,
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
                source_parameter_segment_group_indices,
                source_expr: source_expr.clone(),
                source_expr_absolute_parameter,
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
                source_parameter_segment_group_indices,
                source_expr: source_expr.clone(),
                source_expr_absolute_parameter,
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
                source_parameter_segment_group_indices,
                source_expr: source_expr.clone(),
                source_expr_absolute_parameter,
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
                source_parameter_segment_group_indices,
                source_expr: source_expr.clone(),
                source_expr_absolute_parameter,
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
                source_parameter_segment_group_indices,
                source_expr,
                source_expr_absolute_parameter,
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
            | crate::format::GroupKind::FixedCoordinatePoint
            | crate::format::GroupKind::GraphFunctionPoint
            | crate::format::GroupKind::GraphValuePoint
            | crate::format::GroupKind::LegacyCoordinateParameterHelper
            | crate::format::GroupKind::CoordinateExpressionPointPair
    ) {
        return None;
    }

    let path = find_indexed_path(file, group)?;
    if path.refs.len() < 2 {
        return None;
    }

    match kind {
        crate::format::GroupKind::CoordinatePoint => {
            if path.refs.len() >= 3 {
                if let Some(point) = (|| {
                    let x_parameter_group = groups.get(path.refs[0].checked_sub(1)?)?;
                    let y_parameter_group = groups.get(path.refs[1].checked_sub(1)?)?;
                    if x_parameter_group.header.kind() == crate::format::GroupKind::FunctionExpr
                        && y_parameter_group.header.kind() == crate::format::GroupKind::FunctionExpr
                    {
                        return None;
                    }
                    let axis_group = groups.get(path.refs[2].checked_sub(1)?)?;
                    let (x_parameter_name, x_parameter_value, x_expr) =
                        coordinate_parameter_binding(file, groups, x_parameter_group, anchors)?;
                    let (y_parameter_name, y_parameter_value, y_expr) =
                        coordinate_parameter_binding(file, groups, y_parameter_group, anchors)?;
                    let axis_path = find_indexed_path(file, axis_group)?;
                    let origin_measurement_group =
                        groups.get(axis_path.refs.first()?.checked_sub(1)?)?;
                    let origin_measurement_path =
                        find_indexed_path(file, origin_measurement_group)?;
                    let source_group_index =
                        origin_measurement_path.refs.first()?.checked_sub(1)?;
                    let source_position = anchors.get(source_group_index)?.clone()?;
                    let source_world = to_world(&source_position, graph);
                    let world = PointRecord {
                        x: source_world.x + x_parameter_value,
                        y: source_world.y + y_parameter_value,
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
                            x_expr,
                            y_parameter_name,
                            y_expr: y_expr.clone(),
                        },
                        expr: y_expr,
                    })
                })() {
                    return Some(point);
                }
                let x_calc_group = groups.get(path.refs[0].checked_sub(1)?)?;
                if let Ok(x_expr) =
                    try_decode_function_expr_with_inlined_refs(file, groups, x_calc_group)
                    && let Some(point) = (|| {
                        let y_calc_group = groups.get(path.refs[1].checked_sub(1)?)?;
                        let axis_group = groups.get(path.refs[2].checked_sub(1)?)?;
                        let axis_path = find_indexed_path(file, axis_group)?;
                        let origin_measurement_group =
                            groups.get(axis_path.refs.first()?.checked_sub(1)?)?;
                        let origin_measurement_path =
                            find_indexed_path(file, origin_measurement_group)?;
                        let source_group_index =
                            origin_measurement_path.refs.first()?.checked_sub(1)?;
                        let source_position = anchors.get(source_group_index)?.clone()?;
                        let source_world = to_world(&source_position, graph);
                        let y_expr =
                            try_decode_function_expr_with_inlined_refs(file, groups, y_calc_group)
                                .ok()?;
                        let x_parameter_group = first_path_group(file, groups, x_calc_group)?;
                        let y_parameter_group = first_path_group(file, groups, y_calc_group)?;
                        let x_parameter_name = decode_label_name(file, x_calc_group)
                            .or_else(|| decode_label_name(file, x_parameter_group))
                            .unwrap_or_else(|| {
                                crate::runtime::functions::function_expr_label(x_expr.clone())
                            });
                        let y_parameter_name = decode_label_name(file, y_calc_group)
                            .or_else(|| decode_label_name(file, y_parameter_group))
                            .unwrap_or_else(|| {
                                crate::runtime::functions::function_expr_label(y_expr.clone())
                            });
                        let parameters = BTreeMap::new();
                        let dx = evaluate_expr_with_parameters(&x_expr, 0.0, &parameters)?;
                        let dy = evaluate_expr_with_parameters(&y_expr, 0.0, &parameters)?;
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
                                x_expr,
                                y_parameter_name,
                                y_expr: y_expr.clone(),
                            },
                            expr: y_expr,
                        })
                    })()
                {
                    return Some(point);
                }
            }

            let calc_group = groups.get(path.refs[1].checked_sub(1)?)?;
            let expr = try_decode_function_expr(file, groups, calc_group).ok()?;
            let parameter_group = groups.get(path.refs[0].checked_sub(1)?)?;
            let parameter_name = decode_label_name(file, parameter_group).or_else(|| {
                try_decode_parameter_controlled_point(file, groups, parameter_group, anchors)
                    .ok()
                    .map(|point| point.parameter_name)
                    .filter(|name| !name.is_empty())
            })?;
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
            let calc_group = groups.get(path.refs[1].checked_sub(1)?)?;
            let expr = try_decode_function_expr(file, groups, calc_group).ok()?;
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
        crate::format::GroupKind::LegacyCoordinateParameterHelper => {
            let parameter_group = groups.get(path.refs[0].checked_sub(1)?)?;
            let axis_group = groups.get(path.refs[1].checked_sub(1)?)?;
            let (parameter_name, parameter_value, expr) =
                coordinate_parameter_binding(file, groups, parameter_group, anchors)?;
            let axis_path = find_indexed_path(file, axis_group)?;
            let origin_measurement_group = groups.get(axis_path.refs.first()?.checked_sub(1)?)?;
            let origin_measurement_path = find_indexed_path(file, origin_measurement_group)?;
            let source_group_index = origin_measurement_path.refs.first()?.checked_sub(1)?;
            let source_position = anchors.get(source_group_index)?.clone()?;
            let source_world = to_world(&source_position, graph);
            let world = PointRecord {
                x: source_world.x,
                y: source_world.y + parameter_value,
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
                    axis: crate::runtime::scene::CoordinateAxis::Vertical,
                },
                expr,
            })
        }
        crate::format::GroupKind::GraphFunctionPoint => {
            if path.refs.len() < 3 {
                return None;
            }
            let y_expr_group = groups.get(path.refs[0].checked_sub(1)?)?;
            let x_group = groups.get(path.refs[1].checked_sub(1)?)?;
            let axis_group = groups.get(path.refs[2].checked_sub(1)?)?;
            let y_expr = try_decode_function_expr(file, groups, y_expr_group).ok()?;
            let (x_parameter_name, x_value, x_expr) =
                coordinate_or_expr_binding(file, groups, x_group, anchors)?;
            let (y_parameter_name, y_value) =
                resolve_function_expr_parameter_binding(file, groups, y_expr_group, anchors)?;
            let axis_path = find_indexed_path(file, axis_group)?;
            let origin_measurement_group = groups.get(axis_path.refs.first()?.checked_sub(1)?)?;
            let origin_measurement_path = find_indexed_path(file, origin_measurement_group)?;
            let source_group_index = origin_measurement_path.refs.first()?.checked_sub(1)?;
            let source_position = anchors.get(source_group_index)?.clone()?;
            let source_world = to_world(&source_position, graph);
            let y = evaluate_expr_with_parameters(
                &y_expr,
                0.0,
                &BTreeMap::from([(y_parameter_name.clone(), y_value)]),
            )?;
            let world = PointRecord {
                x: source_world.x + x_value,
                y: source_world.y + y,
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
                    x_expr,
                    y_parameter_name,
                    y_expr: y_expr.clone(),
                },
                expr: y_expr,
            })
        }
        crate::format::GroupKind::GraphValuePoint => {
            if path.refs.len() < 2 {
                return None;
            }
            let value_group = groups.get(path.refs[0].checked_sub(1)?)?;
            let axis_group = groups.get(path.refs[1].checked_sub(1)?)?;
            let (parameter_name, parameter_value, expr) =
                coordinate_or_expr_binding(file, groups, value_group, anchors)?;
            let axis_path = find_indexed_path(file, axis_group)?;
            let origin_measurement_group = groups.get(axis_path.refs.first()?.checked_sub(1)?)?;
            let origin_measurement_path = find_indexed_path(file, origin_measurement_group)?;
            let source_group_index = origin_measurement_path.refs.first()?.checked_sub(1)?;
            let source_position = anchors.get(source_group_index)?.clone()?;
            let source_world = to_world(&source_position, graph);
            let world = PointRecord {
                x: source_world.x + parameter_value,
                y: source_world.y,
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
                    axis: crate::runtime::scene::CoordinateAxis::Horizontal,
                },
                expr,
            })
        }
        crate::format::GroupKind::FixedCoordinatePoint => {
            let axis_group = groups.get(path.refs[0].checked_sub(1)?)?;
            let axis_path = find_indexed_path(file, axis_group)?;
            let source_group_index = axis_path.refs.first()?.checked_sub(1)?;
            let source_position = anchors.get(source_group_index)?.clone()?;
            let source_world = to_world(&source_position, graph);
            let payload = group
                .records
                .iter()
                .find(|record| record.record_type == RECORD_BINDING_PAYLOAD)
                .map(|record| record.payload(&file.data))?;
            if payload.len() < 20 {
                return None;
            }
            let radius = read_f64(payload, 4);
            let angle_radians = read_f64(payload, 12);
            if !radius.is_finite() || !angle_radians.is_finite() {
                return None;
            }
            let world = PointRecord {
                x: source_world.x + radius * angle_radians.cos(),
                y: source_world.y + radius * angle_radians.sin(),
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
                    x_parameter_name: "x".to_string(),
                    x_expr: FunctionExpr::Constant(radius * angle_radians.cos()),
                    y_parameter_name: "y".to_string(),
                    y_expr: FunctionExpr::Constant(radius * angle_radians.sin()),
                },
                expr: FunctionExpr::Constant(radius),
            })
        }
        crate::format::GroupKind::CoordinateExpressionPointPair => {
            let source_group_index = path.refs[0].checked_sub(1)?;
            let source_position = anchors.get(source_group_index)?.clone()?;
            let source_world = to_world(&source_position, graph);
            let x_calc_group = groups.get(path.refs[1].checked_sub(1)?)?;
            let y_calc_group = groups.get(path.refs[2].checked_sub(1)?)?;
            let x_expr =
                try_decode_function_expr_with_inlined_refs(file, groups, x_calc_group).ok()?;
            let y_expr =
                try_decode_function_expr_with_inlined_refs(file, groups, y_calc_group).ok()?;

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

fn axis_line_group_indices(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
    axis_group_index: usize,
    through_group_index: usize,
) -> Option<(usize, usize)> {
    let axis_group = groups.get(axis_group_index)?;
    if let Some(path) = find_indexed_path(file, axis_group)
        && path.refs.len() >= 2
    {
        return Some((path.refs[0].checked_sub(1)?, path.refs[1].checked_sub(1)?));
    }
    let through_anchor = anchors.get(through_group_index)?.clone()?;
    let axis_anchor = anchors.get(axis_group_index)?.clone()?;
    (((axis_anchor.x - through_anchor.x).powi(2) + (axis_anchor.y - through_anchor.y).powi(2))
        .sqrt()
        > 1e-9)
        .then_some((through_group_index, axis_group_index))
}

fn line_intersection_from_points(
    first_point: PointRecord,
    first_axis_start: PointRecord,
    first_axis_end: PointRecord,
    second_point: PointRecord,
    second_axis_start: PointRecord,
    second_axis_end: PointRecord,
) -> Option<PointRecord> {
    let first_dx = first_axis_end.x - first_axis_start.x;
    let first_dy = first_axis_end.y - first_axis_start.y;
    let second_dx = second_axis_end.x - second_axis_start.x;
    let second_dy = second_axis_end.y - second_axis_start.y;
    let det = first_dx * second_dy - first_dy * second_dx;
    if det.abs() <= 1e-9 {
        return None;
    }
    let offset_x = second_point.x - first_point.x;
    let offset_y = second_point.y - first_point.y;
    let t = (offset_x * second_dy - offset_y * second_dx) / det;
    Some(PointRecord {
        x: first_point.x + first_dx * t,
        y: first_point.y + first_dy * t,
    })
}

pub(crate) fn decode_legacy_coordinate_construct_point(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
    anchors: &[Option<PointRecord>],
) -> Option<LegacyCoordinateConstructPoint> {
    if group.header.kind() != GroupKind::LegacyCoordinateConstructPoint {
        return None;
    }
    let path = find_indexed_path(file, group)?;
    if path.refs.len() < 4 {
        return None;
    }
    let first_source_group_index = path.refs[0].checked_sub(1)?;
    let second_source_group_index = path.refs[1].checked_sub(1)?;
    let first_axis_group_index = path.refs[2].checked_sub(1)?;
    let second_axis_group_index = path.refs[3].checked_sub(1)?;
    let first_point = anchors.get(first_source_group_index)?.clone()?;
    let second_point = anchors.get(second_source_group_index)?.clone()?;
    let (first_axis_start_group_index, first_axis_end_group_index) = axis_line_group_indices(
        file,
        groups,
        anchors,
        first_axis_group_index,
        first_source_group_index,
    )?;
    let (second_axis_start_group_index, second_axis_end_group_index) = axis_line_group_indices(
        file,
        groups,
        anchors,
        second_axis_group_index,
        second_source_group_index,
    )?;
    let first_axis_start = anchors.get(first_axis_start_group_index)?.clone()?;
    let first_axis_end = anchors.get(first_axis_end_group_index)?.clone()?;
    let second_axis_start = anchors.get(second_axis_start_group_index)?.clone()?;
    let second_axis_end = anchors.get(second_axis_end_group_index)?.clone()?;
    let position = line_intersection_from_points(
        first_point,
        first_axis_start,
        first_axis_end,
        second_point,
        second_axis_start,
        second_axis_end,
    )?;
    Some(LegacyCoordinateConstructPoint {
        position,
        first_source_group_index,
        second_source_group_index,
        first_axis_start_group_index,
        first_axis_end_group_index,
        second_axis_start_group_index,
        second_axis_end_group_index,
    })
}

fn resolve_function_expr_parameter_binding(
    file: &GspFile,
    groups: &[ObjectGroup],
    expr_group: &ObjectGroup,
    anchors: &[Option<PointRecord>],
) -> Option<(String, f64)> {
    let path = find_indexed_path(file, expr_group)?;
    let parameter_group = groups.get(path.refs.first()?.checked_sub(1)?)?;
    if parameter_group.header.kind() == crate::format::GroupKind::FunctionExpr {
        return resolve_function_expr_parameter_binding(file, groups, parameter_group, anchors);
    }
    if parameter_group.header.kind() == crate::format::GroupKind::ParameterAnchor {
        let anchor_path = find_indexed_path(file, parameter_group)?;
        let point_group = groups.get(anchor_path.refs.first()?.checked_sub(1)?)?;
        let name = decode_label_name(file, parameter_group)
            .or_else(|| decode_label_name(file, point_group))?;
        let value = parameter_anchor_value(file, groups, parameter_group, anchors)?.1;
        return Some((name, value));
    }
    let name = editable_non_graph_parameter_name_for_group(file, groups, parameter_group)
        .or_else(|| decode_label_name(file, parameter_group))?;
    let value = try_decode_parameter_control_value_for_group(file, groups, parameter_group).ok()?;
    Some((name, value))
}

fn coordinate_or_expr_binding(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
    anchors: &[Option<PointRecord>],
) -> Option<(String, f64, FunctionExpr)> {
    if let Some(binding) = coordinate_parameter_binding(file, groups, group, anchors) {
        return Some(binding);
    }
    if group.header.kind() == crate::format::GroupKind::FunctionExpr {
        let expr = try_decode_function_expr(file, groups, group).ok()?;
        let (name, value) = resolve_function_expr_parameter_binding(file, groups, group, anchors)?;
        let evaluated =
            evaluate_expr_with_parameters(&expr, 0.0, &BTreeMap::from([(name.clone(), value)]))?;
        return Some((name, evaluated, expr));
    }
    None
}

fn coordinate_parameter_binding(
    file: &GspFile,
    groups: &[ObjectGroup],
    parameter_group: &ObjectGroup,
    anchors: &[Option<PointRecord>],
) -> Option<(String, f64, FunctionExpr)> {
    if let Some(name) = decode_label_name(file, parameter_group)
        && let Ok(value) =
            try_decode_parameter_control_value_for_group(file, groups, parameter_group)
    {
        return Some((
            name.clone(),
            value,
            FunctionExpr::Parsed(FunctionAst::Parameter(name, value)),
        ));
    }

    if let Ok(parameter_point) =
        try_decode_parameter_controlled_point(file, groups, parameter_group, anchors)
        && !parameter_point.parameter_name.is_empty()
    {
        let name = parameter_point.parameter_name;
        let value = match parameter_point.source_expr.clone() {
            Some(expr) => evaluate_expr_with_parameters(&expr, 0.0, &BTreeMap::new())?,
            None => return None,
        };
        return Some((
            name.clone(),
            value,
            FunctionExpr::Parsed(FunctionAst::Parameter(name, value)),
        ));
    }

    None
}

include!("constraints/runtime.rs");
