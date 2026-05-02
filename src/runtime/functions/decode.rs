use std::cell::Cell;
use std::collections::{BTreeMap, BTreeSet};

use crate::format::{GspFile, ObjectGroup, PointRecord, read_f64, read_u16, read_u32};
use crate::runtime::DEFAULT_GRAPH_RAW_PER_UNIT;
use crate::runtime::extract::points::{
    collect_point_objects, resolve_circle_like_raw, resolve_line_like_points_raw,
};
use crate::runtime::extract::shapes::collect_raw_object_anchors;
use crate::runtime::extract::{
    decode_measurement_value, find_indexed_path, try_decode_parameter_control_value_for_group,
};
use crate::runtime::functions::{evaluate_expr_with_parameters, function_expr_label};
use crate::runtime::geometry::GraphTransform;
use crate::runtime::geometry::angle_degrees_from_points;
use crate::runtime::payload_consts::{
    EXPR_OP_ADD, EXPR_OP_DIV, EXPR_OP_MUL, EXPR_OP_POW, EXPR_OP_SUB, EXPR_PARAMETER_MASK,
    EXPR_PARAMETER_PREFIX, EXPR_PI_SUFFIX, EXPR_PI_WORD, EXPR_VARIABLE_SUFFIX, EXPR_VARIABLE_WORD,
    FUNCTION_EXPR_MARKER_A, FUNCTION_EXPR_MARKER_B, RECORD_FUNCTION_EXPR_PAYLOAD,
    RECORD_INDEXED_PATH_B, RECORD_LABEL_AUX,
};
use crate::util::hex_bytes;
use thiserror::Error;

use super::expr::{
    BinaryOp, FunctionAst, FunctionExpr, FunctionPlotDescriptor, FunctionPlotMode, UnaryFunction,
    canonicalize_function_expr, decode_unary_function, function_ast_contains_symbol,
    function_expr_ast, function_expr_contains_variable,
};

thread_local! {
    static RESOLVING_MEASURED_VALUE: Cell<bool> = const { Cell::new(false) };
}

fn is_function_like_group(group: &ObjectGroup) -> bool {
    matches!(
        group.header.kind(),
        crate::format::GroupKind::FunctionExpr
            | crate::format::GroupKind::DistanceValue
            | crate::format::GroupKind::PointLineDistanceValue
            | crate::format::GroupKind::CoordinateXValue
            | crate::format::GroupKind::CoordinateYValue
            | crate::format::GroupKind::GraphYValue
            | crate::format::GroupKind::GraphXValue
            | crate::format::GroupKind::AngleValue
            | crate::format::GroupKind::ArcAngleValue
            | crate::format::GroupKind::BoundaryCurveLengthValue
            | crate::format::GroupKind::RadiusValue
            | crate::format::GroupKind::PolygonAreaValue
            | crate::format::GroupKind::RatioValue
            | crate::format::GroupKind::GraphDistanceValue
            | crate::format::GroupKind::GraphSlopeValue
            | crate::format::GroupKind::NamedAlias
            | crate::format::GroupKind::FunctionDefinition
    )
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ParameterBinding {
    name: String,
    value: f64,
    expr: Option<FunctionAst>,
}

impl ParameterBinding {
    fn value(name: String, value: f64) -> Self {
        Self {
            name,
            value,
            expr: None,
        }
    }

    fn expression(name: String, value: f64, expr: FunctionAst) -> Self {
        Self {
            name,
            value,
            expr: Some(expr),
        }
    }
}

pub(crate) fn try_decode_function_expr(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
) -> Result<FunctionExpr, FunctionExprParseError> {
    decode_function_expr_recursive(file, groups, group, &mut BTreeSet::new())
}

pub(crate) fn try_decode_function_expr_with_inlined_refs(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
) -> Result<FunctionExpr, FunctionExprParseError> {
    decode_function_expr_recursive_with_inlined_refs(file, groups, group, &mut BTreeSet::new())
}

pub(crate) fn try_decode_plot_component_expr(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
) -> Result<FunctionExpr, FunctionExprParseError> {
    decode_plot_component_expr_recursive(file, groups, group, &mut BTreeSet::new())
}

pub(crate) fn try_decode_standalone_function_expr(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
) -> Result<FunctionExpr, FunctionExprParseError> {
    decode_standalone_function_expr_recursive(file, groups, group, &mut BTreeSet::new())
}

pub(crate) fn evaluate_function_group_with_overrides(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
    overrides: &BTreeMap<String, f64>,
) -> Option<f64> {
    evaluate_function_group_recursive(file, groups, group, overrides, &mut BTreeSet::new())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PayloadExprDecodeMode {
    Standard,
    EmbeddedPostfixPreferred,
    GroupedPreferred,
}

fn decode_function_expr_recursive(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
    visiting: &mut BTreeSet<usize>,
) -> Result<FunctionExpr, FunctionExprParseError> {
    decode_function_expr_recursive_impl(file, groups, group, visiting, false)
}

fn decode_function_expr_recursive_with_inlined_refs(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
    visiting: &mut BTreeSet<usize>,
) -> Result<FunctionExpr, FunctionExprParseError> {
    decode_function_expr_recursive_impl(file, groups, group, visiting, true)
}

fn decode_function_expr_recursive_impl(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
    visiting: &mut BTreeSet<usize>,
    inline_function_refs: bool,
) -> Result<FunctionExpr, FunctionExprParseError> {
    if !visiting.insert(group.ordinal) {
        return Err(FunctionExprParseError::NoExpressionFound { word_len: 0 });
    }
    let expr = (|| {
        if let Some(expr) = try_decode_numeric_helper_group(file, groups, group) {
            return Ok(expr);
        }
        if (group.header.kind()) == crate::format::GroupKind::ParameterControlledPoint
            && let Some(path) = find_indexed_path(file, group)
            && let Some(source_ordinal) = path.refs.first().copied()
            && let Some(source_group) = groups.get(source_ordinal.saturating_sub(1))
            && is_function_like_group(source_group)
        {
            return decode_function_expr_recursive(file, groups, source_group, visiting);
        }
        let mode = if inline_function_refs {
            PayloadExprDecodeMode::EmbeddedPostfixPreferred
        } else {
            PayloadExprDecodeMode::Standard
        };
        decode_group_function_payload_expr(
            file,
            groups,
            group,
            visiting,
            inline_function_refs,
            mode,
        )
    })();
    visiting.remove(&group.ordinal);
    expr
}

fn decode_group_function_payload_expr(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
    visiting: &mut BTreeSet<usize>,
    inline_function_refs: bool,
    mode: PayloadExprDecodeMode,
) -> Result<FunctionExpr, FunctionExprParseError> {
    let payload = group_function_payload(file, group)?;
    let parameters =
        collect_parameter_bindings(file, groups, group, visiting, inline_function_refs);

    if let Some(expr) =
        try_decode_payload_function_application(file, groups, group, visiting, payload, &parameters)
    {
        return Ok(expr);
    }

    match mode {
        PayloadExprDecodeMode::Standard => decode_payload_function_expr(payload, &parameters),
        PayloadExprDecodeMode::EmbeddedPostfixPreferred => {
            decode_embedded_postfix_payload_function_expr(payload, &parameters)
                .or_else(|_| decode_payload_function_expr(payload, &parameters))
        }
        PayloadExprDecodeMode::GroupedPreferred => {
            decode_grouped_preferred_payload_function_expr(payload, &parameters)
        }
    }
}

fn group_function_payload<'a>(
    file: &'a GspFile,
    group: &ObjectGroup,
) -> Result<&'a [u8], FunctionExprParseError> {
    group
        .records
        .iter()
        .find(|record| record.record_type == RECORD_FUNCTION_EXPR_PAYLOAD)
        .map(|record| record.payload(&file.data))
        .ok_or(FunctionExprParseError::NoExpressionFound { word_len: 0 })
}

fn decode_plot_component_expr_recursive(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
    visiting: &mut BTreeSet<usize>,
) -> Result<FunctionExpr, FunctionExprParseError> {
    if is_function_like_group(group)
        || (group.header.kind()) == crate::format::GroupKind::ParameterControlledPoint
    {
        return decode_function_expr_recursive(file, groups, group, visiting);
    }
    if !visiting.insert(group.ordinal) {
        return Err(FunctionExprParseError::NoExpressionFound { word_len: 0 });
    }
    let expr = decode_group_function_payload_expr(
        file,
        groups,
        group,
        visiting,
        false,
        PayloadExprDecodeMode::Standard,
    );
    visiting.remove(&group.ordinal);
    expr
}

fn decode_standalone_function_expr_recursive(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
    visiting: &mut BTreeSet<usize>,
) -> Result<FunctionExpr, FunctionExprParseError> {
    if is_function_like_group(group)
        || (group.header.kind()) == crate::format::GroupKind::ParameterControlledPoint
    {
        return decode_function_expr_recursive(file, groups, group, visiting);
    }
    if !visiting.insert(group.ordinal) {
        return Err(FunctionExprParseError::NoExpressionFound { word_len: 0 });
    }
    let expr = decode_group_function_payload_expr(
        file,
        groups,
        group,
        visiting,
        false,
        PayloadExprDecodeMode::GroupedPreferred,
    );
    visiting.remove(&group.ordinal);
    expr
}

fn decode_grouped_preferred_payload_function_expr(
    payload: &[u8],
    parameters: &BTreeMap<u16, ParameterBinding>,
) -> Result<FunctionExpr, FunctionExprParseError> {
    with_function_payload_context(payload, || {
        let words = payload
            .chunks_exact(2)
            .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
            .collect::<Vec<_>>();
        let parsed = parse_grouped_function_expr_from_words(&words, parameters)
            .or_else(|_| parse_function_expr_from_words(&words, parameters))?;
        Ok(canonicalize_function_expr(parsed))
    })
}

fn try_decode_numeric_helper_group(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
) -> Option<FunctionExpr> {
    if !matches!(
        group.header.kind(),
        crate::format::GroupKind::DistanceValue
            | crate::format::GroupKind::PointLineDistanceValue
            | crate::format::GroupKind::CoordinateXValue
            | crate::format::GroupKind::CoordinateYValue
            | crate::format::GroupKind::GraphYValue
            | crate::format::GroupKind::GraphXValue
            | crate::format::GroupKind::AngleValue
            | crate::format::GroupKind::ArcAngleValue
            | crate::format::GroupKind::BoundaryCurveLengthValue
            | crate::format::GroupKind::RadiusValue
            | crate::format::GroupKind::PolygonAreaValue
            | crate::format::GroupKind::RatioValue
            | crate::format::GroupKind::GraphDistanceValue
            | crate::format::GroupKind::GraphSlopeValue
            | crate::format::GroupKind::NamedAlias
    ) {
        return None;
    }
    let path = find_indexed_path(file, group)?;
    let max_ref_ordinal = path.refs.iter().copied().max()?;
    let helper_groups = groups.get(..max_ref_ordinal)?;
    let point_map = collect_point_objects(file, groups);
    let helper_point_map = point_map.get(..max_ref_ordinal)?;
    let anchors_without_graph =
        collect_raw_object_anchors(file, helper_groups, helper_point_map, None);
    let graph_transform = detect_graph_context(file, helper_groups, &anchors_without_graph).map(
        |(origin_raw, raw_per_unit)| GraphTransform {
            origin_raw,
            raw_per_unit,
        },
    );
    let anchors = if let Some(transform) = graph_transform.as_ref() {
        collect_raw_object_anchors(file, helper_groups, helper_point_map, Some(transform))
    } else {
        anchors_without_graph
    };
    let graph = graph_transform
        .as_ref()
        .map(|transform| (transform.origin_raw.clone(), transform.raw_per_unit));
    let value = match group.header.kind() {
        crate::format::GroupKind::DistanceValue => {
            let left = anchors.get(path.refs.first()?.checked_sub(1)?)?.clone()?;
            let right = anchors.get(path.refs.get(1)?.checked_sub(1)?)?.clone()?;
            normalize_graph_distance(
                ((right.x - left.x).powi(2) + (right.y - left.y).powi(2)).sqrt(),
                graph.as_ref().map(|(_, raw_per_unit)| *raw_per_unit),
            )
        }
        crate::format::GroupKind::PointLineDistanceValue => {
            let point = anchors.get(path.refs.first()?.checked_sub(1)?)?.clone()?;
            let line_group = helper_groups.get(path.refs.get(1)?.checked_sub(1)?)?;
            let (line_start, line_end) =
                resolve_line_like_points_raw(file, helper_groups, &anchors, line_group)?;
            normalize_graph_distance(
                point_line_distance_raw(&point, &line_start, &line_end)?,
                graph.as_ref().map(|(_, raw_per_unit)| *raw_per_unit),
            )
        }
        crate::format::GroupKind::CoordinateXValue | crate::format::GroupKind::CoordinateYValue => {
            let point = anchors.get(path.refs.first()?.checked_sub(1)?)?.clone()?;
            let (origin_raw, raw_per_unit) = graph.or_else(|| {
                let axis_ordinal = *path.refs.get(1)?;
                explicit_axis_context_for_coordinate_value(
                    file,
                    groups,
                    &anchors,
                    axis_ordinal,
                    group.header.kind(),
                )
            })?;
            if group.header.kind() == crate::format::GroupKind::CoordinateXValue {
                (point.x - origin_raw.x) / raw_per_unit
            } else {
                (origin_raw.y - point.y) / raw_per_unit
            }
        }
        crate::format::GroupKind::GraphYValue | crate::format::GroupKind::GraphXValue => {
            let point = anchors.get(path.refs.first()?.checked_sub(1)?)?.clone()?;
            let axis_group = groups.get(path.refs.get(1)?.checked_sub(1)?)?;
            let axis_path = find_indexed_path(file, axis_group)?;
            let origin_group = groups.get(axis_path.refs.first()?.checked_sub(1)?)?;
            let origin_path = find_indexed_path(file, origin_group)?;
            let source_group_index = origin_path.refs.first()?.checked_sub(1)?;
            let source_position = anchors.get(source_group_index)?.clone()?;
            let source_world =
                crate::runtime::geometry::to_world(&source_position, &graph_transform);
            let point_world = crate::runtime::geometry::to_world(&point, &graph_transform);
            if group.header.kind() == crate::format::GroupKind::GraphXValue {
                point_world.x - source_world.x
            } else {
                point_world.y - source_world.y
            }
        }
        crate::format::GroupKind::AngleValue => {
            let start = anchors.get(path.refs.first()?.checked_sub(1)?)?.clone()?;
            let vertex = anchors.get(path.refs.get(1)?.checked_sub(1)?)?.clone()?;
            let end = anchors.get(path.refs.get(2)?.checked_sub(1)?)?.clone()?;
            angle_degrees_from_points(&start, &vertex, &end)?
        }
        crate::format::GroupKind::ArcAngleValue => {
            let source_group = groups.get(path.refs.first()?.checked_sub(1)?)?;
            arc_angle_degrees_raw(file, groups, &anchors, source_group)?
        }
        crate::format::GroupKind::BoundaryCurveLengthValue => {
            let source_group = groups.get(path.refs.first()?.checked_sub(1)?)?;
            boundary_curve_length_raw(
                file,
                groups,
                &anchors,
                source_group,
                graph.as_ref().map(|(_, raw_per_unit)| *raw_per_unit),
            )?
        }
        crate::format::GroupKind::RadiusValue => {
            let source_group = groups.get(path.refs.first()?.checked_sub(1)?)?;
            if let Some(circle) = resolve_circle_like_raw(file, groups, &anchors, source_group) {
                normalize_graph_distance(
                    circle.radius(),
                    graph.as_ref().map(|(_, raw_per_unit)| *raw_per_unit),
                )
            } else {
                evaluate_function_group_with_overrides(
                    file,
                    groups,
                    source_group,
                    &std::collections::BTreeMap::new(),
                )?
            }
        }
        crate::format::GroupKind::PolygonAreaValue => {
            let polygon_group = groups.get(path.refs.first()?.checked_sub(1)?)?;
            let polygon_path = find_indexed_path(file, polygon_group)?;
            let points = polygon_path
                .refs
                .iter()
                .filter_map(|ordinal| anchors.get(ordinal.saturating_sub(1)).cloned().flatten())
                .collect::<Vec<_>>();
            normalize_graph_area(
                polygon_area_raw(&points)?,
                graph.as_ref().map(|(_, raw_per_unit)| *raw_per_unit),
            )
        }
        crate::format::GroupKind::RatioValue => decode_ratio_helper_group(
            file,
            helper_groups,
            &anchors,
            graph_transform.as_ref(),
            &path.refs,
        )?,
        crate::format::GroupKind::GraphDistanceValue => {
            let left = anchors.get(path.refs.first()?.checked_sub(1)?)?.clone()?;
            let right = anchors.get(path.refs.get(1)?.checked_sub(1)?)?.clone()?;
            normalize_graph_distance(
                ((right.x - left.x).powi(2) + (right.y - left.y).powi(2)).sqrt(),
                graph.as_ref().map(|(_, raw_per_unit)| *raw_per_unit),
            )
        }
        crate::format::GroupKind::GraphSlopeValue => {
            let line_group = groups.get(path.refs.first()?.checked_sub(1)?)?;
            let (start, end) = resolve_line_like_points_raw(file, groups, &anchors, line_group)?;
            let start_world = crate::runtime::geometry::to_world(&start, &graph_transform);
            let end_world = crate::runtime::geometry::to_world(&end, &graph_transform);
            let dx = end_world.x - start_world.x;
            let dy = end_world.y - start_world.y;
            (dx.abs() > 1e-9).then_some(dy / dx)?
        }
        crate::format::GroupKind::NamedAlias => {
            let source_group = groups.get(path.refs.first()?.checked_sub(1)?)?;
            if matches!(
                source_group.header.kind(),
                crate::format::GroupKind::AngleMarker | crate::format::GroupKind::LegacyAngleMarker
            ) {
                let source_path = find_indexed_path(file, source_group)?;
                let start = anchors
                    .get(source_path.refs.first()?.checked_sub(1)?)?
                    .clone()?;
                let vertex = anchors
                    .get(source_path.refs.get(1)?.checked_sub(1)?)?
                    .clone()?;
                let end = anchors
                    .get(source_path.refs.get(2)?.checked_sub(1)?)?
                    .clone()?;
                angle_degrees_from_points(&start, &vertex, &end)?
            } else {
                try_decode_function_expr(file, groups, source_group)
                    .ok()
                    .and_then(|expr| match expr {
                        FunctionExpr::Constant(value) => Some(value),
                        _ => None,
                    })?
            }
        }
        _ => return None,
    };
    value.is_finite().then_some(FunctionExpr::Constant(value))
}

fn explicit_axis_context_for_coordinate_value(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<crate::format::PointRecord>],
    axis_ordinal: usize,
    coordinate_kind: crate::format::GroupKind,
) -> Option<(crate::format::PointRecord, f64)> {
    let axis_group = groups.get(axis_ordinal.checked_sub(1)?)?;
    let axis_path = find_indexed_path(file, axis_group)?;
    let measurement_ordinal = match coordinate_kind {
        crate::format::GroupKind::CoordinateXValue => *axis_path.refs.first()?,
        crate::format::GroupKind::CoordinateYValue => *axis_path.refs.get(1)?,
        _ => return None,
    };
    let measurement_group = groups.get(measurement_ordinal.checked_sub(1)?)?;
    let measurement_path = find_indexed_path(file, measurement_group)?;
    let origin_raw = anchors
        .get(measurement_path.refs.first()?.checked_sub(1)?)?
        .clone()?;
    let unit_expr_group = groups.get(measurement_path.refs.get(1)?.checked_sub(1)?)?;
    let raw_per_unit = try_decode_function_expr(file, groups, unit_expr_group)
        .ok()
        .and_then(|unit_expr| evaluate_expr_with_parameters(&unit_expr, 0.0, &BTreeMap::new()))
        .map(f64::abs)
        .filter(|value| *value > 1e-9)
        .or_else(|| {
            unit_expr_group
                .records
                .iter()
                .find(|record| record.record_type == 0x07d3 && record.length == 12)
                .and_then(|record| decode_measurement_value(record.payload(&file.data)))
        })
        .or_else(|| {
            let unit_raw = anchors
                .get(measurement_path.refs.get(1)?.checked_sub(1)?)?
                .clone()?;
            let distance = (unit_raw.x - origin_raw.x).hypot(unit_raw.y - origin_raw.y);
            (distance > 1e-9).then_some(distance)
        })?;
    (raw_per_unit > 1e-9).then_some((origin_raw, raw_per_unit))
}

fn decode_ratio_helper_group(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<crate::format::PointRecord>],
    graph: Option<&GraphTransform>,
    refs: &[usize],
) -> Option<f64> {
    let anchor =
        |ordinal: usize| resolve_helper_group_anchor(file, groups, anchors, graph, ordinal);
    let length = |ordinal: usize| resolve_helper_group_length_raw(file, groups, anchors, ordinal);
    match refs {
        [left, right] => {
            let numerator = length(*left).or_else(|| {
                let start = anchor(*left)?;
                let end = anchor(*right)?;
                Some(((end.x - start.x).powi(2) + (end.y - start.y).powi(2)).sqrt())
            })?;
            let denominator = length(*right)?;
            (denominator.abs() > 1e-9).then_some(numerator / denominator)
        }
        [origin, baseline, target, ..] => {
            if let (Some(origin), Some(baseline), Some(target)) =
                (anchor(*origin), anchor(*baseline), anchor(*target))
            {
                let baseline_distance =
                    ((baseline.x - origin.x).powi(2) + (baseline.y - origin.y).powi(2)).sqrt();
                let target_distance =
                    ((target.x - origin.x).powi(2) + (target.y - origin.y).powi(2)).sqrt();
                if baseline_distance.abs() > 1e-9 {
                    return Some(target_distance / baseline_distance);
                }
            }

            if let (Some(origin), Some(baseline_length), Some(target)) =
                (anchor(*origin), length(*baseline), anchor(*target))
                && baseline_length.abs() > 1e-9
            {
                let target_distance =
                    ((target.x - origin.x).powi(2) + (target.y - origin.y).powi(2)).sqrt();
                return Some(target_distance / baseline_length);
            }

            let numerator = length(*origin)?;
            let denominator = length(*baseline)?;
            (denominator.abs() > 1e-9).then_some(numerator / denominator)
        }
        _ => None,
    }
}

fn resolve_helper_group_anchor(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<crate::format::PointRecord>],
    graph: Option<&GraphTransform>,
    ordinal: usize,
) -> Option<crate::format::PointRecord> {
    let group = groups.get(ordinal.checked_sub(1)?)?;
    crate::runtime::extract::points::decode_graph_calibration_anchor_raw(
        file, groups, group, anchors, graph,
    )
    .or_else(|| anchors.get(ordinal.checked_sub(1)?).cloned().flatten())
    .or_else(|| {
        let path = find_indexed_path(file, group)?;
        path.refs
            .iter()
            .find_map(|child| anchors.get(child.saturating_sub(1)).cloned().flatten())
    })
}

fn resolve_helper_group_length_raw(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<crate::format::PointRecord>],
    ordinal: usize,
) -> Option<f64> {
    let group = groups.get(ordinal.checked_sub(1)?)?;
    if let Some((start, end)) = resolve_line_like_points_raw(file, groups, anchors, group) {
        return Some(((end.x - start.x).powi(2) + (end.y - start.y).powi(2)).sqrt());
    }

    match group.header.kind() {
        crate::format::GroupKind::DerivedSegment24 | crate::format::GroupKind::DerivedSegment75 => {
            let mut memo = vec![None; groups.len()];
            let mut visiting = BTreeSet::new();
            let points =
                descend_helper_points(file, groups, anchors, ordinal, &mut memo, &mut visiting);
            farthest_pair_distance(&points)
        }
        _ => None,
    }
}

fn descend_helper_points(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<crate::format::PointRecord>],
    ordinal: usize,
    memo: &mut Vec<Option<Vec<crate::format::PointRecord>>>,
    visiting: &mut BTreeSet<usize>,
) -> Vec<crate::format::PointRecord> {
    let Some(index) = ordinal.checked_sub(1) else {
        return Vec::new();
    };
    if let Some(cached) = &memo[index] {
        return cached.clone();
    }
    if !visiting.insert(ordinal) {
        return Vec::new();
    }

    let mut points = Vec::new();
    if let Some(point) = anchors.get(index).cloned().flatten() {
        points.push(point);
    } else if let Some(group) = groups.get(index)
        && let Some(path) = find_indexed_path(file, group)
    {
        for child in path.refs {
            if child > 0 && child <= groups.len() {
                points.extend(descend_helper_points(
                    file, groups, anchors, child, memo, visiting,
                ));
            }
        }
    }

    visiting.remove(&ordinal);
    points.sort_by(|a, b| a.x.total_cmp(&b.x).then_with(|| a.y.total_cmp(&b.y)));
    points.dedup_by(|a, b| (a.x - b.x).abs() < 0.001 && (a.y - b.y).abs() < 0.001);
    memo[index] = Some(points.clone());
    points
}

fn farthest_pair_distance(points: &[crate::format::PointRecord]) -> Option<f64> {
    let mut best = None;
    for i in 0..points.len() {
        for j in i + 1..points.len() {
            let distance =
                ((points[j].x - points[i].x).powi(2) + (points[j].y - points[i].y).powi(2)).sqrt();
            if distance.is_finite() && best.is_none_or(|current| distance > current) {
                best = Some(distance);
            }
        }
    }
    best
}

fn detect_graph_context(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<crate::format::PointRecord>],
) -> Option<(crate::format::PointRecord, f64)> {
    let raw_per_unit = groups
        .iter()
        .filter(|group| group.header.kind().is_graph_calibration())
        .find_map(|group| {
            let record = group
                .records
                .iter()
                .find(|record| record.record_type == 0x07d3 && record.length == 12)?;
            decode_measurement_value(record.payload(&file.data))
        })?;
    let origin_raw = groups.iter().find_map(|group| {
        if !group.header.kind().is_graph_calibration() {
            return None;
        }
        let path = find_indexed_path(file, group)?;
        path.refs
            .iter()
            .find_map(|object_ref| anchors.get(object_ref.saturating_sub(1)).cloned().flatten())
    })?;
    Some((origin_raw, raw_per_unit))
}

fn normalize_graph_distance(raw_distance: f64, raw_per_unit: Option<f64>) -> f64 {
    match raw_per_unit {
        Some(scale) if scale.is_finite() && scale > 1e-9 => raw_distance / scale,
        _ => raw_distance,
    }
}

fn normalize_graph_area(raw_area: f64, raw_per_unit: Option<f64>) -> f64 {
    match raw_per_unit {
        Some(scale) if scale.is_finite() && scale > 1e-9 => raw_area / (scale * scale),
        _ => raw_area,
    }
}

fn arc_angle_degrees_raw(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<crate::format::PointRecord>],
    group: &ObjectGroup,
) -> Option<f64> {
    let path = find_indexed_path(file, group)?;
    match group.header.kind() {
        crate::format::GroupKind::CenterArc => {
            let center = anchors.get(path.refs.first()?.checked_sub(1)?)?.clone()?;
            let start = anchors.get(path.refs.get(1)?.checked_sub(1)?)?.clone()?;
            let end = anchors.get(path.refs.get(2)?.checked_sub(1)?)?.clone()?;
            angle_degrees_from_points(&start, &center, &end)
        }
        crate::format::GroupKind::ArcOnCircle => {
            let circle_group = groups.get(path.refs.first()?.checked_sub(1)?)?;
            let circle_path = find_indexed_path(file, circle_group)?;
            let center = anchors
                .get(circle_path.refs.first()?.checked_sub(1)?)?
                .clone()?;
            let start = anchors.get(path.refs.get(1)?.checked_sub(1)?)?.clone()?;
            let end = anchors.get(path.refs.get(2)?.checked_sub(1)?)?.clone()?;
            angle_degrees_from_points(&start, &center, &end)
        }
        crate::format::GroupKind::SectorBoundary
        | crate::format::GroupKind::CircularSegmentBoundary => {
            let host_group = groups.get(path.refs.first()?.checked_sub(1)?)?;
            arc_angle_degrees_raw(file, groups, anchors, host_group)
        }
        _ => None,
    }
}

fn boundary_curve_length_raw(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<crate::format::PointRecord>],
    group: &ObjectGroup,
    raw_per_unit: Option<f64>,
) -> Option<f64> {
    match group.header.kind() {
        crate::format::GroupKind::Segment
        | crate::format::GroupKind::MeasurementLine
        | crate::format::GroupKind::GraphMeasurementSegment => {
            let (start, end) = resolve_line_like_points_raw(file, groups, anchors, group)?;
            Some(normalize_graph_distance(
                ((end.x - start.x).powi(2) + (end.y - start.y).powi(2)).sqrt(),
                raw_per_unit,
            ))
        }
        crate::format::GroupKind::ArcOnCircle
        | crate::format::GroupKind::CenterArc
        | crate::format::GroupKind::SectorBoundary
        | crate::format::GroupKind::CircularSegmentBoundary => {
            let degrees = arc_angle_degrees_raw(file, groups, anchors, group)?;
            let host_group = if matches!(
                group.header.kind(),
                crate::format::GroupKind::SectorBoundary
                    | crate::format::GroupKind::CircularSegmentBoundary
            ) {
                let path = find_indexed_path(file, group)?;
                groups.get(path.refs.first()?.checked_sub(1)?)?
            } else {
                group
            };
            let path = find_indexed_path(file, host_group)?;
            let circle_group = match host_group.header.kind() {
                crate::format::GroupKind::CenterArc => None,
                crate::format::GroupKind::ArcOnCircle => {
                    Some(groups.get(path.refs.first()?.checked_sub(1)?)?)
                }
                _ => None,
            };
            let radius_raw = if let Some(circle_group) = circle_group {
                resolve_circle_like_raw(file, groups, anchors, circle_group)?.radius()
            } else {
                let center = anchors.get(path.refs.first()?.checked_sub(1)?)?.clone()?;
                let start = anchors.get(path.refs.get(1)?.checked_sub(1)?)?.clone()?;
                ((start.x - center.x).powi(2) + (start.y - center.y).powi(2)).sqrt()
            };
            Some(normalize_graph_distance(
                radius_raw * degrees.to_radians(),
                raw_per_unit,
            ))
        }
        crate::format::GroupKind::Polygon => {
            let path = find_indexed_path(file, group)?;
            let points = path
                .refs
                .iter()
                .filter_map(|ordinal| anchors.get(ordinal.saturating_sub(1)).cloned().flatten())
                .collect::<Vec<_>>();
            if points.len() < 2 {
                return None;
            }
            let length = (0..points.len())
                .map(|index| {
                    let start = &points[index];
                    let end = &points[(index + 1) % points.len()];
                    ((end.x - start.x).powi(2) + (end.y - start.y).powi(2)).sqrt()
                })
                .sum::<f64>();
            Some(normalize_graph_distance(length, raw_per_unit))
        }
        _ => None,
    }
}

fn point_line_distance_raw(
    point: &crate::format::PointRecord,
    line_start: &crate::format::PointRecord,
    line_end: &crate::format::PointRecord,
) -> Option<f64> {
    let dx = line_end.x - line_start.x;
    let dy = line_end.y - line_start.y;
    let len_sq = dx * dx + dy * dy;
    if len_sq <= 1e-9 {
        return None;
    }
    let cross = (point.x - line_start.x) * dy - (point.y - line_start.y) * dx;
    Some(cross.abs() / len_sq.sqrt())
}

fn polygon_area_raw(points: &[crate::format::PointRecord]) -> Option<f64> {
    if points.len() < 3 {
        return None;
    }
    let mut twice_area = 0.0;
    for index in 0..points.len() {
        let current = &points[index];
        let next = &points[(index + 1) % points.len()];
        twice_area += current.x * next.y - next.x * current.y;
    }
    let area = twice_area.abs() * 0.5;
    area.is_finite().then_some(area)
}

fn decode_payload_function_expr(
    payload: &[u8],
    parameters: &BTreeMap<u16, ParameterBinding>,
) -> Result<FunctionExpr, FunctionExprParseError> {
    with_function_payload_context(payload, || {
        let text = extract_inline_function_token(payload).ok_or(
            FunctionExprParseError::NoExpressionFound {
                word_len: payload.len() / 2,
            },
        )?;
        if text.eq_ignore_ascii_case("x") {
            Ok(FunctionExpr::Identity)
        } else if let Ok(value) = text.parse::<f64>() {
            if value == 0.0 {
                match try_decode_embedded_static_function_expr(payload, parameters)
                    .or_else(|_| try_decode_inner_function_expr(payload, parameters))
                {
                    Ok(expr) => Ok(expr),
                    Err(error) if payload_has_function_expr_evidence(payload) => Err(error),
                    Err(_) => Ok(FunctionExpr::Constant(value)),
                }
            } else {
                Ok(FunctionExpr::Constant(value))
            }
        } else {
            try_decode_inner_function_expr(payload, parameters)
        }
    })
}

fn payload_has_function_expr_evidence(payload: &[u8]) -> bool {
    let words = payload
        .chunks_exact(2)
        .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
        .collect::<Vec<_>>();
    words
        .windows(2)
        .any(|pair| *pair == FUNCTION_EXPR_MARKER_A || *pair == FUNCTION_EXPR_MARKER_B)
        || embedded_calculate_expr_start(&words).is_some()
        || words.contains(&0x000b)
}

fn try_decode_embedded_static_function_expr(
    payload: &[u8],
    parameters: &BTreeMap<u16, ParameterBinding>,
) -> Result<FunctionExpr, FunctionExprParseError> {
    let words = payload
        .chunks_exact(2)
        .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
        .collect::<Vec<_>>();
    let start =
        embedded_calculate_expr_start(&words).ok_or(FunctionExprParseError::NoExpressionFound {
            word_len: words.len(),
        })?;
    let (parsed, end) = parse_function_expr_from(&words, start, parameters)?;
    if has_ignorable_expr_suffix(&words, end) {
        Ok(canonicalize_function_expr(parsed))
    } else {
        Err(FunctionExprParseError::NoExpressionFound {
            word_len: words.len(),
        })
    }
}

fn substitute_variable(ast: FunctionAst, replacement: &FunctionAst) -> FunctionAst {
    match ast {
        FunctionAst::Variable => replacement.clone(),
        FunctionAst::Constant(_) | FunctionAst::PiAngle | FunctionAst::Parameter(_, _) => ast,
        FunctionAst::Unary { op, expr } => FunctionAst::Unary {
            op,
            expr: Box::new(substitute_variable(*expr, replacement)),
        },
        FunctionAst::Binary { lhs, op, rhs } => FunctionAst::Binary {
            lhs: Box::new(substitute_variable(*lhs, replacement)),
            op,
            rhs: Box::new(substitute_variable(*rhs, replacement)),
        },
    }
}

const EXPR_FUNCTION_REF_MASK: u16 = 0xfff0;
const EXPR_FUNCTION_REF_PREFIX: u16 = 0x7000;

fn try_decode_payload_function_application(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
    visiting: &mut BTreeSet<usize>,
    payload: &[u8],
    parameters: &BTreeMap<u16, ParameterBinding>,
) -> Option<FunctionExpr> {
    let words = payload
        .chunks_exact(2)
        .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
        .collect::<Vec<_>>();
    let expression_start = words
        .windows(2)
        .position(|pair| *pair == FUNCTION_EXPR_MARKER_A || *pair == FUNCTION_EXPR_MARKER_B)
        .map_or(0, |marker_index| marker_index + 2);
    let (application_offset, application_word) = words
        .iter()
        .copied()
        .enumerate()
        .skip(expression_start)
        .find(|(_, word)| (*word & EXPR_FUNCTION_REF_MASK) == EXPR_FUNCTION_REF_PREFIX)?;
    let helper_index = usize::from(application_word & 0x000f);
    let path = find_indexed_path(file, group)?;
    let helper_group = groups.get(path.refs.get(helper_index)?.checked_sub(1)?)?;
    if !is_function_like_group(helper_group) {
        return None;
    }

    let helper_expr = decode_function_expr_recursive(file, groups, helper_group, visiting).ok()?;
    let arg_payload = words
        .get(application_offset + 1..)?
        .iter()
        .flat_map(|word| word.to_le_bytes())
        .collect::<Vec<_>>();
    let arg_expr = try_decode_inner_function_expr(&arg_payload, parameters).ok()?;
    let helper_ast = function_expr_ast(helper_expr);
    let arg_ast = function_expr_ast(arg_expr);
    Some(canonicalize_function_expr(substitute_variable(
        helper_ast, &arg_ast,
    )))
}

fn evaluate_function_group_recursive(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
    overrides: &BTreeMap<String, f64>,
    visiting: &mut BTreeSet<usize>,
) -> Option<f64> {
    if !visiting.insert(group.ordinal) {
        return None;
    }
    let result = (|| {
        if !is_function_like_group(group) {
            return decode_runtime_parameter_binding(file, groups, group, overrides, visiting)
                .map(|binding| binding.value);
        }
        let payload = group_function_payload(file, group).ok()?;
        let mut parameters = BTreeMap::new();
        if let Some(path) = find_indexed_path(file, group) {
            for (index, ordinal) in path.refs.iter().copied().enumerate() {
                let parameter_group = groups.get(ordinal.checked_sub(1)?)?;
                let binding = decode_runtime_parameter_binding(
                    file,
                    groups,
                    parameter_group,
                    overrides,
                    visiting,
                );
                let binding = binding?;
                parameters.insert(index as u16, binding);
            }
        }
        let expr = decode_payload_function_expr(payload, &parameters).ok()?;
        evaluate_expr_with_parameters(&expr, 0.0, overrides)
    })();
    visiting.remove(&group.ordinal);
    result
}

#[derive(Debug, Clone, PartialEq, Error)]
pub(crate) enum FunctionPlotDescriptorDecodeError {
    #[error("function plot descriptor payload too short ({byte_len} bytes)")]
    PayloadTooShort { byte_len: usize },
    #[error("invalid function plot range [{x_min}, {x_max}]")]
    InvalidRange { x_min: f64, x_max: f64 },
}

pub(crate) fn try_decode_function_plot_descriptor(
    payload: &[u8],
) -> Result<FunctionPlotDescriptor, FunctionPlotDescriptorDecodeError> {
    if payload.len() < 24 {
        return Err(FunctionPlotDescriptorDecodeError::PayloadTooShort {
            byte_len: payload.len(),
        });
    }

    let x_min = read_f64(payload, 0);
    let x_max = read_f64(payload, 8);
    let sample_count = read_u32(payload, 16) as usize;
    let mode = match read_u32(payload, 20) & 0xffff {
        2 => FunctionPlotMode::Polar,
        _ => FunctionPlotMode::Cartesian,
    };
    if !x_min.is_finite() || !x_max.is_finite() || x_min == x_max {
        return Err(FunctionPlotDescriptorDecodeError::InvalidRange { x_min, x_max });
    }

    Ok(FunctionPlotDescriptor {
        x_min,
        x_max,
        sample_count: sample_count.clamp(2, 4096),
        mode,
    })
}

fn collect_parameter_bindings(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
    visiting: &mut BTreeSet<usize>,
    inline_function_refs: bool,
) -> BTreeMap<u16, ParameterBinding> {
    let mut bindings = BTreeMap::new();
    let Some(path) = find_indexed_path(file, group) else {
        return bindings;
    };
    let inline_function_refs =
        inline_function_refs || group.header.kind() == crate::format::GroupKind::FunctionDefinition;
    for (index, ordinal) in path.refs.iter().copied().enumerate() {
        let Some(parameter_group) = groups.get(ordinal.saturating_sub(1)) else {
            continue;
        };
        if inline_function_refs && parameter_group.ordinal == group.ordinal {
            continue;
        }
        if let Some(binding) = decode_parameter_binding_recursive(
            file,
            groups,
            parameter_group,
            visiting,
            inline_function_refs,
            false,
        ) {
            bindings.insert(index as u16, binding);
        }
    }
    bindings
}

fn decode_runtime_parameter_binding(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
    overrides: &BTreeMap<String, f64>,
    visiting: &mut BTreeSet<usize>,
) -> Option<ParameterBinding> {
    if (group.header.kind()) == crate::format::GroupKind::ParameterAnchor {
        let binding = decode_parameter_anchor_binding(file, group, false)?;
        let value = overrides
            .get(&binding.name)
            .copied()
            .unwrap_or(binding.value);
        return Some(ParameterBinding {
            name: binding.name,
            value,
            expr: None,
        });
    }
    if (group.header.kind()) == crate::format::GroupKind::MeasuredValue {
        return decode_measured_value_binding(file, groups, group);
    }
    if is_function_like_group(group) {
        let expr = decode_function_expr_recursive(file, groups, group, visiting).ok()?;
        let name =
            group_name(file, groups, group).unwrap_or_else(|| function_expr_label(expr.clone()));
        let value = evaluate_function_group_recursive(file, groups, group, overrides, visiting)?;
        return Some(ParameterBinding::value(name, value));
    }

    let label_payload = group
        .records
        .iter()
        .find(|record| record.record_type == RECORD_LABEL_AUX)
        .map(|record| record.payload(&file.data))?;
    let name = decode_parameter_name(label_payload)?;
    let value = overrides
        .get(&name)
        .copied()
        .or_else(|| try_decode_parameter_control_value_for_group(file, groups, group).ok())?;
    value
        .is_finite()
        .then_some(ParameterBinding::value(name, value))
}

fn decode_parameter_binding_recursive(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
    visiting: &mut BTreeSet<usize>,
    inline_function_refs: bool,
    allow_constraint_anchor_bindings: bool,
) -> Option<ParameterBinding> {
    if (group.header.kind()) == crate::format::GroupKind::ParameterAnchor {
        return decode_parameter_anchor_binding(file, group, allow_constraint_anchor_bindings);
    }
    if (group.header.kind()) == crate::format::GroupKind::MeasuredValue {
        return decode_measured_value_binding(file, groups, group);
    }
    if is_function_like_group(group) {
        let expr = decode_function_expr_recursive(file, groups, group, visiting).ok()?;
        let name =
            group_name(file, groups, group).unwrap_or_else(|| function_expr_label(expr.clone()));
        let value = evaluate_expr_with_parameters(&expr, 0.0, &BTreeMap::new());
        if !inline_function_refs || function_expr_contains_variable(&expr) {
            return value.map(|value| ParameterBinding::value(name, value));
        }
        return Some(ParameterBinding::expression(
            name,
            value.unwrap_or(0.0),
            function_expr_ast(expr),
        ));
    }

    let label_payload = group
        .records
        .iter()
        .find(|record| record.record_type == RECORD_LABEL_AUX)
        .map(|record| record.payload(&file.data))?;
    let name = decode_parameter_name(label_payload)?;
    let value = try_decode_parameter_control_value_for_group(file, groups, group).ok()?;
    if !value.is_finite() {
        return None;
    }
    Some(ParameterBinding::value(name, value))
}

fn decode_parameter_anchor_binding(
    file: &GspFile,
    group: &ObjectGroup,
    allow_constraint_anchor_bindings: bool,
) -> Option<ParameterBinding> {
    let groups = file.object_groups();
    let path = find_indexed_path(file, group)?;
    let point_group = groups.get(path.refs.first()?.checked_sub(1)?)?;
    let name = group
        .records
        .iter()
        .find(|record| record.record_type == RECORD_LABEL_AUX)
        .and_then(|record| decode_parameter_name(record.payload(&file.data)))
        .or_else(|| {
            point_group
                .records
                .iter()
                .find(|record| record.record_type == RECORD_LABEL_AUX)
                .and_then(|record| decode_parameter_name(record.payload(&file.data)))
        })
        .unwrap_or_else(|| format!("__param_anchor_{}", group.ordinal));
    let value = match point_group.header.kind() {
        kind if kind.is_point_constraint() && allow_constraint_anchor_bindings => {
            decode_parameter_anchor_constraint_value(file, &groups, point_group)?
        }
        kind if kind.is_point_constraint() => point_group
            .records
            .iter()
            .find(|record| record.record_type == RECORD_INDEXED_PATH_B && record.length == 12)
            .map(|record| read_f64(record.payload(&file.data), 4))
            .filter(|value| value.is_finite())?,
        crate::format::GroupKind::Point => {
            let payload = point_group
                .records
                .iter()
                .find(|record| record.record_type == RECORD_FUNCTION_EXPR_PAYLOAD)
                .map(|record| record.payload(&file.data))?;
            if payload.len() >= 60 {
                read_f64(payload, 52)
            } else {
                f64::from(read_u16(payload, payload.len().checked_sub(2)?))
            }
        }
        _ => return None,
    };
    Some(ParameterBinding::value(name, value))
}

fn decode_parameter_anchor_constraint_value(
    file: &GspFile,
    groups: &[ObjectGroup],
    point_group: &ObjectGroup,
) -> Option<f64> {
    let point_path = find_indexed_path(file, point_group)?;
    let host_group = groups.get(point_path.refs.first()?.checked_sub(1)?)?;
    let payload = point_group
        .records
        .iter()
        .find(|record| record.record_type == RECORD_INDEXED_PATH_B && record.length >= 12)
        .map(|record| record.payload(&file.data))?;
    let t = read_f64(payload, 4);
    if !t.is_finite() {
        return None;
    }
    if host_group.header.kind() != crate::format::GroupKind::Polygon {
        return Some(t);
    }

    let edge_index = decode_polygon_edge_index_for_anchor(host_group, file, payload)?;
    let host_path = find_indexed_path(file, host_group)?;
    let max_ref_ordinal = host_path.refs.iter().copied().max()?;
    let helper_groups = groups.get(..max_ref_ordinal)?;
    let point_map = collect_point_objects(file, groups);
    let helper_point_map = point_map.get(..max_ref_ordinal)?;
    let anchors = collect_raw_object_anchors(file, helper_groups, helper_point_map, None);
    let vertex_group_indices = host_path
        .refs
        .iter()
        .map(|ordinal| ordinal.checked_sub(1))
        .collect::<Option<Vec<_>>>()?;
    polygon_boundary_parameter_for_anchor(&anchors, &vertex_group_indices, edge_index, t)
}

fn decode_polygon_edge_index_for_anchor(
    polygon_group: &ObjectGroup,
    file: &GspFile,
    payload: &[u8],
) -> Option<usize> {
    let vertex_count = find_indexed_path(file, polygon_group)?.refs.len();
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

fn polygon_boundary_parameter_for_anchor(
    anchors: &[Option<PointRecord>],
    vertex_group_indices: &[usize],
    edge_index: usize,
    t: f64,
) -> Option<f64> {
    if vertex_group_indices.len() < 2 {
        return None;
    }
    let vertices = vertex_group_indices
        .iter()
        .map(|group_index| anchors.get(*group_index)?.clone())
        .collect::<Option<Vec<_>>>()?;
    let mut perimeter = 0.0;
    let mut traveled = 0.0;
    for index in 0..vertices.len() {
        let start = &vertices[index];
        let end = &vertices[(index + 1) % vertices.len()];
        let length = ((end.x - start.x).powi(2) + (end.y - start.y).powi(2)).sqrt();
        perimeter += length;
        if index < edge_index % vertices.len() {
            traveled += length;
        } else if index == edge_index % vertices.len() {
            traveled += length * t.clamp(0.0, 1.0);
        }
    }
    (perimeter > 1e-9).then_some(traveled / perimeter)
}

fn decode_measured_value_binding(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
) -> Option<ParameterBinding> {
    let path = find_indexed_path(file, group)?;
    let host_group = groups.get(path.refs.first()?.checked_sub(1)?)?;
    if !host_group.header.kind().is_line_like() {
        return None;
    }
    let host_path = find_indexed_path(file, host_group)?;
    if host_path.refs.len() != 2 {
        return None;
    }

    let anchors = RESOLVING_MEASURED_VALUE.with(|flag| {
        if flag.replace(true) {
            return None;
        }
        let point_map = collect_point_objects(file, groups);
        let anchors = collect_raw_object_anchors(file, groups, &point_map, None);
        flag.set(false);
        Some(anchors)
    })?;
    let start = anchors.get(host_path.refs[0].checked_sub(1)?)?.clone()?;
    let end = anchors.get(host_path.refs[1].checked_sub(1)?)?.clone()?;
    let value =
        ((end.x - start.x).powi(2) + (end.y - start.y).powi(2)).sqrt() / DEFAULT_GRAPH_RAW_PER_UNIT;
    if !value.is_finite() {
        return None;
    }

    let name =
        group_name(file, groups, group).or_else(|| segment_name(file, groups, host_group))?;
    Some(ParameterBinding::value(name, value))
}

fn group_name(file: &GspFile, groups: &[ObjectGroup], group: &ObjectGroup) -> Option<String> {
    group
        .records
        .iter()
        .find(|record| record.record_type == RECORD_LABEL_AUX)
        .and_then(|record| decode_parameter_name(record.payload(&file.data)))
        .or_else(|| numeric_helper_group_name(file, groups, group))
}

fn numeric_helper_group_name(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
) -> Option<String> {
    if group.header.kind() != crate::format::GroupKind::DistanceValue {
        return None;
    }
    let path = find_indexed_path(file, group)?;
    if path.refs.len() == 1 {
        return path
            .refs
            .first()
            .and_then(|ordinal| groups.get(ordinal.checked_sub(1)?))
            .and_then(|group| group_name(file, groups, group));
    }
    let left = path
        .refs
        .first()
        .and_then(|ordinal| groups.get(ordinal.checked_sub(1)?))
        .and_then(|group| group_name(file, groups, group))
        .unwrap_or_else(|| "P".to_string());
    let right = path
        .refs
        .get(1)
        .and_then(|ordinal| groups.get(ordinal.checked_sub(1)?))
        .and_then(|group| group_name(file, groups, group))
        .unwrap_or_else(|| "Q".to_string());
    Some(format!("{left}{right}"))
}

fn segment_name(
    file: &GspFile,
    groups: &[ObjectGroup],
    segment_group: &ObjectGroup,
) -> Option<String> {
    let path = find_indexed_path(file, segment_group)?;
    let names = path
        .refs
        .iter()
        .map(|ordinal| group_name(file, groups, groups.get(ordinal.checked_sub(1)?)?))
        .collect::<Option<Vec<_>>>()?;
    (names.len() >= 2).then(|| names.join(""))
}

fn decode_parameter_name(label_payload: &[u8]) -> Option<String> {
    if label_payload.len() >= 24 {
        let name_len = read_u16(label_payload, 22) as usize;
        if name_len > 0 && 24 + name_len <= label_payload.len() {
            let name = String::from_utf8_lossy(&label_payload[24..24 + name_len]).to_string();
            return Some(
                name.replace("[1]", "₁")
                    .replace("[2]", "₂")
                    .replace("[3]", "₃")
                    .replace("[4]", "₄"),
            );
        }
    }
    if label_payload.len() < 2 {
        return None;
    }
    let name_code = read_u16(label_payload, label_payload.len() - 2);
    char::from_u32(name_code as u32)
        .filter(|ch| ch.is_ascii_alphabetic())?
        .to_string()
        .into()
}

pub(crate) fn extract_inline_function_token(payload: &[u8]) -> Option<String> {
    let text = String::from_utf8_lossy(payload);
    let start = text.find('<')?;
    let end = text[start + 1..].find('>')?;
    let token = text[start + 1..start + 1 + end].trim();
    if token.is_empty() {
        None
    } else {
        Some(token.to_string())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum FunctionToken {
    Add,
    Sub,
    Mul,
    Div,
    Pow,
    Terminator,
    Variable,
    PiAngle,
    Parameter(ParameterBinding),
    Unary(UnaryFunction),
    Constant(f64),
}

#[derive(Debug, Clone, PartialEq, Error)]
pub(crate) enum FunctionExprParseError {
    #[error("unexpected end of function payload at word offset {offset}")]
    UnexpectedEnd { offset: usize },
    #[error("unexpected token {found:?} at function payload word offset {offset}")]
    UnexpectedToken { offset: usize, found: FunctionToken },
    #[error(
        "invalid unary operand for opcode 0x{opcode:04x} at function payload word offset {offset}"
    )]
    InvalidUnaryOperand { offset: usize, opcode: u16 },
    #[error(
        "missing parameter binding #{parameter_index} at function payload word offset {offset}"
    )]
    MissingParameterBinding { offset: usize, parameter_index: u16 },
    #[error("no parseable function expression found in {word_len} payload words")]
    NoExpressionFound { word_len: usize },
    #[error(
        "failed to parse function expression payload ({byte_len} bytes): {source}; payload={payload_hex}"
    )]
    PayloadParseFailed {
        byte_len: usize,
        payload_hex: String,
        #[source]
        source: Box<FunctionExprParseError>,
    },
}

fn with_function_payload_context<T>(
    payload: &[u8],
    decode: impl FnOnce() -> Result<T, FunctionExprParseError>,
) -> Result<T, FunctionExprParseError> {
    decode().map_err(|error| function_payload_parse_error(payload, error))
}

fn function_payload_parse_error(
    payload: &[u8],
    error: FunctionExprParseError,
) -> FunctionExprParseError {
    match error {
        FunctionExprParseError::PayloadParseFailed { .. } => error,
        _ => FunctionExprParseError::PayloadParseFailed {
            byte_len: payload.len(),
            payload_hex: hex_bytes(payload),
            source: Box::new(error),
        },
    }
}

#[derive(Debug, Clone, PartialEq)]
struct LexedFunctionToken {
    kind: FunctionToken,
    width_words: usize,
}

#[derive(Clone)]
struct FunctionTokenCursor<'a> {
    words: &'a [u16],
    parameters: &'a BTreeMap<u16, ParameterBinding>,
    base_offset: usize,
    offset: usize,
}

impl<'a> FunctionTokenCursor<'a> {
    fn new(
        words: &'a [u16],
        parameters: &'a BTreeMap<u16, ParameterBinding>,
        base_offset: usize,
    ) -> Self {
        Self {
            words,
            parameters,
            base_offset,
            offset: 0,
        }
    }

    fn peek(&self) -> Result<Option<LexedFunctionToken>, FunctionExprParseError> {
        if self.offset >= self.words.len() {
            return Ok(None);
        }
        lex_function_token(
            &self.words[self.offset..],
            self.parameters,
            self.current_offset(),
        )
        .map(Some)
    }

    fn bump(&mut self) -> Result<FunctionToken, FunctionExprParseError> {
        let token = self.peek()?.ok_or(FunctionExprParseError::UnexpectedEnd {
            offset: self.current_offset(),
        })?;
        self.offset += token.width_words;
        Ok(token.kind)
    }

    fn current_offset(&self) -> usize {
        self.base_offset + self.offset
    }

    fn words_consumed(&self) -> usize {
        self.offset
    }

    fn has_standalone_terminator_ahead(&self) -> bool {
        let remaining = &self.words[self.offset..];
        remaining.iter().enumerate().any(|(index, word)| {
            *word == EXPR_VARIABLE_SUFFIX
                && (index == 0 || remaining[index - 1] != EXPR_VARIABLE_WORD)
        })
    }

    fn argument_terminator_offset(&self) -> Option<usize> {
        self.words[self.offset..]
            .iter()
            .position(|word| *word == EXPR_VARIABLE_SUFFIX)
    }
}

struct FunctionExprParser<'a> {
    tokens: FunctionTokenCursor<'a>,
}

fn is_degree_angle_parameter_name(name: &str) -> bool {
    name.contains('θ') || name.contains('φ')
}

fn ast_contains_degree_angle_parameter(expr: &FunctionAst) -> bool {
    match expr {
        FunctionAst::Parameter(name, _) => is_degree_angle_parameter_name(name),
        FunctionAst::Unary { expr, .. } => ast_contains_degree_angle_parameter(expr),
        FunctionAst::Binary { lhs, rhs, .. } => {
            ast_contains_degree_angle_parameter(lhs) || ast_contains_degree_angle_parameter(rhs)
        }
        _ => false,
    }
}

fn ast_contains_pi_angle_marker(expr: &FunctionAst) -> bool {
    match expr {
        FunctionAst::PiAngle => true,
        FunctionAst::Unary { expr, .. } => ast_contains_pi_angle_marker(expr),
        FunctionAst::Binary { lhs, rhs, .. } => {
            ast_contains_pi_angle_marker(lhs) || ast_contains_pi_angle_marker(rhs)
        }
        _ => false,
    }
}

fn mark_degree_trig_argument(expr: FunctionAst) -> FunctionAst {
    if ast_contains_pi_angle_marker(&expr) || !ast_contains_degree_angle_parameter(&expr) {
        return expr;
    }
    FunctionAst::Binary {
        lhs: Box::new(expr),
        op: BinaryOp::Add,
        rhs: Box::new(FunctionAst::Binary {
            lhs: Box::new(FunctionAst::Constant(0.0)),
            op: BinaryOp::Mul,
            rhs: Box::new(FunctionAst::PiAngle),
        }),
    }
}

fn unary_ast(op: UnaryFunction, expr: FunctionAst) -> FunctionAst {
    let expr = if matches!(
        op,
        UnaryFunction::Sin | UnaryFunction::Cos | UnaryFunction::Tan
    ) {
        mark_degree_trig_argument(expr)
    } else {
        expr
    };
    FunctionAst::Unary {
        op,
        expr: Box::new(expr),
    }
}

impl<'a> FunctionExprParser<'a> {
    fn new(
        words: &'a [u16],
        parameters: &'a BTreeMap<u16, ParameterBinding>,
        base_offset: usize,
    ) -> Self {
        Self {
            tokens: FunctionTokenCursor::new(words, parameters, base_offset),
        }
    }

    fn parse_expr(&mut self) -> Result<FunctionAst, FunctionExprParseError> {
        self.parse_expr_bp(0)
    }

    fn parse_expr_bp(&mut self, min_bp: u8) -> Result<FunctionAst, FunctionExprParseError> {
        let mut lhs = self.parse_prefix()?;
        while let Some((op, left_bp, right_bp)) = self.peek_infix()? {
            if left_bp < min_bp {
                break;
            }
            self.tokens.bump()?;
            let rhs = self.parse_expr_bp(right_bp)?;
            lhs = FunctionAst::Binary {
                lhs: Box::new(lhs),
                op,
                rhs: Box::new(rhs),
            };
        }
        Ok(lhs)
    }

    fn parse_prefix(&mut self) -> Result<FunctionAst, FunctionExprParseError> {
        let offset = self.tokens.current_offset();
        match self.tokens.bump()? {
            FunctionToken::Variable => Ok(FunctionAst::Variable),
            FunctionToken::PiAngle => Ok(FunctionAst::PiAngle),
            FunctionToken::Parameter(binding) => Ok(binding
                .expr
                .unwrap_or(FunctionAst::Parameter(binding.name, binding.value))),
            FunctionToken::Constant(value) => Ok(FunctionAst::Constant(value)),
            FunctionToken::Unary(op) => {
                let expr = self.parse_unary_argument(offset, op)?;
                Ok(unary_ast(op, expr))
            }
            FunctionToken::Add => self.parse_prefix(),
            FunctionToken::Sub => {
                let expr = self.parse_prefix()?;
                Ok(FunctionAst::Binary {
                    lhs: Box::new(FunctionAst::Constant(0.0)),
                    op: BinaryOp::Sub,
                    rhs: Box::new(expr),
                })
            }
            found @ (FunctionToken::Mul
            | FunctionToken::Div
            | FunctionToken::Pow
            | FunctionToken::Terminator) => {
                Err(FunctionExprParseError::UnexpectedToken { offset, found })
            }
        }
    }

    fn parse_unary_argument(
        &mut self,
        unary_offset: usize,
        op: UnaryFunction,
    ) -> Result<FunctionAst, FunctionExprParseError> {
        if matches!(
            op,
            UnaryFunction::Sin | UnaryFunction::Cos | UnaryFunction::Tan
        ) && let Some(argument_word_len) = self.tokens.argument_terminator_offset()
            && argument_word_len > 0
        {
            let start = self.tokens.offset;
            if let Ok(parsed) = parse_function_expr_from_words(
                &self.tokens.words[start..start + argument_word_len],
                self.tokens.parameters,
            ) {
                self.tokens.offset = start + argument_word_len + 1;
                return Ok(parsed);
            }
        }

        let terminator_aware = self.tokens.has_standalone_terminator_ahead();
        let expr = if terminator_aware {
            self.parse_expr_bp(0)
        } else {
            self.parse_expr_bp(4)
        }
        .map_err(|_| FunctionExprParseError::InvalidUnaryOperand {
            offset: unary_offset,
            opcode: self.tokens.words[unary_offset - self.tokens.base_offset],
        })?;
        if terminator_aware
            && matches!(
                self.tokens.peek()?,
                Some(LexedFunctionToken {
                    kind: FunctionToken::Terminator,
                    ..
                })
            )
        {
            let _ = self.tokens.bump()?;
        }
        Ok(expr)
    }

    fn peek_infix(&mut self) -> Result<Option<(BinaryOp, u8, u8)>, FunctionExprParseError> {
        Ok(match self.tokens.peek()? {
            Some(LexedFunctionToken {
                kind: FunctionToken::Terminator,
                ..
            }) => None,
            Some(LexedFunctionToken {
                kind: FunctionToken::Add,
                ..
            }) => Some((BinaryOp::Add, 1, 2)),
            Some(LexedFunctionToken {
                kind: FunctionToken::Sub,
                ..
            }) => Some((BinaryOp::Sub, 1, 2)),
            Some(LexedFunctionToken {
                kind: FunctionToken::Mul,
                ..
            }) => Some((BinaryOp::Mul, 3, 4)),
            Some(LexedFunctionToken {
                kind: FunctionToken::Div,
                ..
            }) => Some((BinaryOp::Div, 3, 4)),
            Some(LexedFunctionToken {
                kind: FunctionToken::Pow,
                ..
            }) => Some((BinaryOp::Pow, 5, 5)),
            _ => None,
        })
    }

    fn words_consumed(&self) -> usize {
        self.tokens.words_consumed()
    }
}

fn lex_function_token(
    words: &[u16],
    parameters: &BTreeMap<u16, ParameterBinding>,
    offset: usize,
) -> Result<LexedFunctionToken, FunctionExprParseError> {
    fn suffix_width(word: u16, next: Option<u16>) -> usize {
        match word {
            EXPR_VARIABLE_WORD | EXPR_PI_WORD => {
                usize::from(matches!(next, Some(EXPR_VARIABLE_SUFFIX)))
            }
            EXPR_PARAMETER_PREFIX..=u16::MAX
                if (word & EXPR_PARAMETER_MASK) == EXPR_PARAMETER_PREFIX =>
            {
                0
            }
            _ => usize::from(matches!(next, Some(0x0101 | 0x0201))),
        }
    }

    let word = *words
        .first()
        .ok_or(FunctionExprParseError::UnexpectedEnd { offset })?;
    let token = match word {
        EXPR_OP_ADD => LexedFunctionToken {
            kind: FunctionToken::Add,
            width_words: 1,
        },
        EXPR_OP_SUB => LexedFunctionToken {
            kind: FunctionToken::Sub,
            width_words: 1,
        },
        EXPR_OP_MUL => LexedFunctionToken {
            kind: FunctionToken::Mul,
            width_words: 1,
        },
        EXPR_OP_DIV => LexedFunctionToken {
            kind: FunctionToken::Div,
            width_words: 1,
        },
        EXPR_OP_POW => LexedFunctionToken {
            kind: FunctionToken::Pow,
            width_words: 1,
        },
        EXPR_PI_WORD if matches!(words.get(1), Some(&EXPR_PI_SUFFIX)) => LexedFunctionToken {
            kind: FunctionToken::PiAngle,
            width_words: 2,
        },
        EXPR_VARIABLE_WORD if matches!(words.get(1), Some(&EXPR_VARIABLE_SUFFIX)) => {
            LexedFunctionToken {
                kind: FunctionToken::Variable,
                width_words: 2,
            }
        }
        EXPR_VARIABLE_WORD => LexedFunctionToken {
            kind: FunctionToken::Variable,
            width_words: 1,
        },
        _ => {
            if let Some((value, width_words)) = decode_decimal_digit_literal(words) {
                return Ok(LexedFunctionToken {
                    kind: FunctionToken::Constant(value),
                    width_words,
                });
            }
            if word == EXPR_VARIABLE_SUFFIX {
                return Ok(LexedFunctionToken {
                    kind: FunctionToken::Terminator,
                    width_words: 1,
                });
            }
            if let Some(op) = decode_unary_function(word) {
                return Ok(LexedFunctionToken {
                    kind: FunctionToken::Unary(op),
                    width_words: 1,
                });
            }
            if (word & EXPR_PARAMETER_MASK) == EXPR_PARAMETER_PREFIX {
                let parameter_index = word & 0x000f;
                return Ok(LexedFunctionToken {
                    kind: FunctionToken::Parameter(
                        parameters.get(&parameter_index).cloned().ok_or(
                            FunctionExprParseError::MissingParameterBinding {
                                offset,
                                parameter_index,
                            },
                        )?,
                    ),
                    width_words: 1 + suffix_width(word, words.get(1).copied()),
                });
            }
            LexedFunctionToken {
                kind: FunctionToken::Constant(f64::from(word)),
                width_words: 1 + suffix_width(word, words.get(1).copied()),
            }
        }
    };
    Ok(token)
}

fn decode_decimal_digit_literal(words: &[u16]) -> Option<(f64, usize)> {
    let first = *words.first()?;
    let second = *words.get(1)?;
    let next = words.get(2).copied();
    if first <= 9
        && second <= 9
        && let (Some(third), Some(suffix)) = (words.get(2).copied(), words.get(3).copied())
        && third == 0
        && suffix == 0x0101
        && words.len() == 4
    {
        return Some((f64::from(first * 100 + second * 10 + third), 4));
    }
    if first == 0 && second == 10 {
        let digit = *words.get(2)?;
        let after_digit = words.get(3).copied();
        if digit < 10
            && matches!(
                after_digit,
                None | Some(
                    EXPR_OP_ADD | EXPR_OP_SUB | EXPR_OP_MUL | EXPR_OP_DIV | EXPR_OP_POW | 0x000c
                )
            )
        {
            return Some((f64::from(digit) / 10.0, 3));
        }
    }
    if first > 9 || second > 9 {
        return None;
    }
    if !matches!(
        next,
        None | Some(EXPR_OP_ADD | EXPR_OP_SUB | EXPR_OP_MUL | EXPR_OP_DIV | EXPR_OP_POW | 0x000c)
    ) {
        return None;
    }
    Some((f64::from(first * 10 + second), 2))
}

fn decode_postfix_decimal_digit_literal(words: &[u16]) -> Option<(f64, usize)> {
    let first = *words.first()?;
    let second = *words.get(1)?;
    let next = words.get(2).copied();
    if first == 0x000a && second < 10 {
        if next == Some(EXPR_OP_MUL)
            && matches!(
                words.get(3).copied(),
                Some(word) if (word & EXPR_PARAMETER_MASK) == EXPR_PARAMETER_PREFIX
            )
        {
            return Some((f64::from(second) / 10.0, 3));
        }
        if matches!(
            next,
            None | Some(
                EXPR_OP_ADD | EXPR_OP_SUB | EXPR_OP_MUL | EXPR_OP_DIV | EXPR_OP_POW | 0x000c
            )
        ) {
            return Some((f64::from(second) / 10.0, 2));
        }
    }
    decode_decimal_digit_literal(words)
}

fn postfix_suffix_width(word: u16, next: Option<u16>) -> usize {
    match word {
        EXPR_VARIABLE_WORD | EXPR_PI_WORD => {
            usize::from(matches!(next, Some(EXPR_VARIABLE_SUFFIX)))
        }
        EXPR_PARAMETER_PREFIX..=u16::MAX
            if (word & EXPR_PARAMETER_MASK) == EXPR_PARAMETER_PREFIX =>
        {
            0
        }
        _ => 0,
    }
}

fn parse_embedded_postfix_function_expr(
    words: &[u16],
    parameters: &BTreeMap<u16, ParameterBinding>,
) -> Result<FunctionAst, FunctionExprParseError> {
    let start =
        embedded_calculate_expr_start(words).ok_or(FunctionExprParseError::NoExpressionFound {
            word_len: words.len(),
        })?;
    if words[start..].contains(&0x000b) {
        return Err(FunctionExprParseError::NoExpressionFound {
            word_len: words.len(),
        });
    }
    parse_postfix_function_expr_from_words(words, start, parameters)
}

fn decode_embedded_postfix_payload_function_expr(
    payload: &[u8],
    parameters: &BTreeMap<u16, ParameterBinding>,
) -> Result<FunctionExpr, FunctionExprParseError> {
    let words = payload
        .chunks_exact(2)
        .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
        .collect::<Vec<_>>();
    let parsed = parse_embedded_postfix_function_expr(&words, parameters)?;
    Ok(canonicalize_function_expr(parsed))
}

fn parse_postfix_function_expr_from_words(
    words: &[u16],
    start: usize,
    parameters: &BTreeMap<u16, ParameterBinding>,
) -> Result<FunctionAst, FunctionExprParseError> {
    let mut stack = Vec::<FunctionAst>::new();
    let mut index = start;
    while index < words.len() {
        let word = words[index];
        if word == 0x000b || word == 0x000c {
            index += 1;
            continue;
        }
        if word == 0
            && matches!(
                words.get(index + 1).copied(),
                Some(EXPR_OP_ADD | EXPR_OP_SUB | EXPR_OP_MUL | EXPR_OP_DIV | EXPR_OP_POW)
            )
        {
            index += 1;
            continue;
        }
        if let Some((value, width_words)) = decode_postfix_decimal_digit_literal(&words[index..]) {
            stack.push(FunctionAst::Constant(value));
            index += width_words;
            continue;
        }
        match word {
            EXPR_OP_ADD => {
                if stack.len() >= 2 {
                    let rhs = stack.pop().unwrap();
                    let lhs = stack.pop().unwrap();
                    stack.push(FunctionAst::Binary {
                        lhs: Box::new(lhs),
                        op: BinaryOp::Add,
                        rhs: Box::new(rhs),
                    });
                }
                index += 1;
            }
            EXPR_OP_SUB => {
                let rhs = stack.pop().ok_or(FunctionExprParseError::UnexpectedToken {
                    offset: index,
                    found: FunctionToken::Sub,
                })?;
                let lhs = stack.pop().unwrap_or(FunctionAst::Constant(0.0));
                stack.push(FunctionAst::Binary {
                    lhs: Box::new(lhs),
                    op: BinaryOp::Sub,
                    rhs: Box::new(rhs),
                });
                index += 1;
            }
            EXPR_OP_MUL | EXPR_OP_DIV | EXPR_OP_POW => {
                let found = match word {
                    EXPR_OP_MUL => FunctionToken::Mul,
                    EXPR_OP_DIV => FunctionToken::Div,
                    EXPR_OP_POW => FunctionToken::Pow,
                    _ => unreachable!(),
                };
                let rhs = stack.pop().ok_or(FunctionExprParseError::UnexpectedToken {
                    offset: index,
                    found: found.clone(),
                })?;
                let lhs = stack.pop().ok_or(FunctionExprParseError::UnexpectedToken {
                    offset: index,
                    found,
                })?;
                let op = match word {
                    EXPR_OP_MUL => BinaryOp::Mul,
                    EXPR_OP_DIV => BinaryOp::Div,
                    EXPR_OP_POW => BinaryOp::Pow,
                    _ => unreachable!(),
                };
                stack.push(FunctionAst::Binary {
                    lhs: Box::new(lhs),
                    op,
                    rhs: Box::new(rhs),
                });
                index += 1;
            }
            EXPR_PI_WORD if matches!(words.get(index + 1), Some(&EXPR_PI_SUFFIX)) => {
                stack.push(FunctionAst::PiAngle);
                index += 2;
            }
            EXPR_VARIABLE_WORD if matches!(words.get(index + 1), Some(&EXPR_VARIABLE_SUFFIX)) => {
                stack.push(FunctionAst::Variable);
                index += 2;
            }
            EXPR_VARIABLE_WORD => {
                stack.push(FunctionAst::Variable);
                index += 1;
            }
            _ if (word & EXPR_PARAMETER_MASK) == EXPR_PARAMETER_PREFIX => {
                let parameter_index = word & 0x000f;
                let binding = parameters.get(&parameter_index).cloned().ok_or(
                    FunctionExprParseError::MissingParameterBinding {
                        offset: index,
                        parameter_index,
                    },
                )?;
                stack.push(
                    binding
                        .expr
                        .unwrap_or(FunctionAst::Parameter(binding.name, binding.value)),
                );
                index += 1 + postfix_suffix_width(word, words.get(index + 1).copied());
            }
            _ if decode_unary_function(word).is_some() => {
                let expr = stack.pop().ok_or(FunctionExprParseError::UnexpectedToken {
                    offset: index,
                    found: FunctionToken::Unary(decode_unary_function(word).unwrap()),
                })?;
                stack.push(unary_ast(decode_unary_function(word).unwrap(), expr));
                index += 1;
            }
            _ => {
                stack.push(FunctionAst::Constant(f64::from(word)));
                index += 1 + postfix_suffix_width(word, words.get(index + 1).copied());
            }
        }
    }
    while stack.len() > 1 {
        let rhs = stack.pop().unwrap();
        let lhs = stack.pop().unwrap();
        stack.push(FunctionAst::Binary {
            lhs: Box::new(lhs),
            op: BinaryOp::Mul,
            rhs: Box::new(rhs),
        });
    }
    stack
        .pop()
        .filter(|expr| stack.is_empty() && parsed_contains_symbol(expr))
        .ok_or(FunctionExprParseError::NoExpressionFound {
            word_len: words.len(),
        })
}

pub(crate) fn try_decode_inner_function_expr(
    payload: &[u8],
    parameters: &BTreeMap<u16, ParameterBinding>,
) -> Result<FunctionExpr, FunctionExprParseError> {
    with_function_payload_context(payload, || {
        let words = payload
            .chunks_exact(2)
            .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
            .collect::<Vec<_>>();
        if embedded_calculate_expr_start(&words).is_some() {
            if let Ok(expr) = try_decode_embedded_static_function_expr(payload, parameters) {
                return Ok(expr);
            }
            if let Ok(expr) = decode_embedded_postfix_payload_function_expr(payload, parameters) {
                return Ok(expr);
            }
        }
        if words.contains(&0x000b)
            && let Ok(ast) = parse_grouped_function_expr_from_words(&words, parameters)
        {
            return Ok(canonicalize_function_expr(ast));
        }
        if let Ok(expr) = decode_embedded_postfix_payload_function_expr(payload, parameters) {
            return Ok(expr);
        }
        let parsed = if words.contains(&0x000b) {
            parse_grouped_function_expr_from_words(&words, parameters)
                .or_else(|_| parse_function_expr_from_words(&words, parameters))
        } else {
            parse_function_expr_from_words(&words, parameters)
                .or_else(|_| parse_grouped_function_expr_from_words(&words, parameters))
        }?;
        Ok(canonicalize_function_expr(parsed))
    })
}

#[allow(dead_code)]
fn parse_function_expr(
    payload: &[u8],
    parameters: &BTreeMap<u16, ParameterBinding>,
) -> Result<FunctionAst, FunctionExprParseError> {
    let words = payload
        .chunks_exact(2)
        .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
        .collect::<Vec<_>>();
    parse_function_expr_from_words(&words, parameters)
}

fn parse_function_expr_from_words(
    words: &[u16],
    parameters: &BTreeMap<u16, ParameterBinding>,
) -> Result<FunctionAst, FunctionExprParseError> {
    let marker_index = words
        .windows(2)
        .position(|pair| *pair == FUNCTION_EXPR_MARKER_A || *pair == FUNCTION_EXPR_MARKER_B);
    let start = marker_index.map_or(0, |marker_index| marker_index + 2);
    let (parsed, end) = parse_function_expr_from(words, start, parameters)?;
    if marker_index.is_some()
        || (parsed_contains_symbol(&parsed) && has_ignorable_expr_suffix(words, end))
    {
        Ok(parsed)
    } else {
        Err(FunctionExprParseError::NoExpressionFound {
            word_len: words.len(),
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
enum GroupedFunctionToken {
    Add,
    Sub,
    Mul,
    Div,
    Pow,
    LParen,
    RParen,
    Variable,
    PiAngle,
    Parameter(ParameterBinding),
    Unary(UnaryFunction),
    Constant(f64),
}

struct GroupedFunctionParser<'a> {
    words: &'a [u16],
    parameters: &'a BTreeMap<u16, ParameterBinding>,
    base_offset: usize,
    offset: usize,
    allow_unclosed_unary_argument: bool,
}

impl<'a> GroupedFunctionParser<'a> {
    fn new(
        words: &'a [u16],
        parameters: &'a BTreeMap<u16, ParameterBinding>,
        base_offset: usize,
    ) -> Self {
        Self {
            words,
            parameters,
            base_offset,
            offset: 0,
            allow_unclosed_unary_argument: false,
        }
    }

    fn allowing_unclosed_unary_argument(mut self) -> Self {
        self.allow_unclosed_unary_argument = true;
        self
    }

    fn peek(&self) -> Result<Option<GroupedFunctionToken>, FunctionExprParseError> {
        if self.offset >= self.words.len() {
            return Ok(None);
        }
        lex_grouped_function_token(
            self.words[self.offset],
            self.parameters,
            self.base_offset + self.offset,
        )
        .map(Some)
    }

    fn bump(&mut self) -> Result<GroupedFunctionToken, FunctionExprParseError> {
        let token = self.peek()?.ok_or(FunctionExprParseError::UnexpectedEnd {
            offset: self.base_offset + self.offset,
        })?;
        self.offset += 1;
        Ok(token)
    }

    fn skip_infix_delimiters(&mut self) {
        if self.offset >= self.words.len() || self.words[self.offset] != 0x000c {
            return;
        }
        let mut lookahead = self.offset;
        while lookahead < self.words.len() && self.words[lookahead] == 0x000c {
            lookahead += 1;
        }
        if lookahead < self.words.len()
            && matches!(
                self.words[lookahead],
                EXPR_OP_ADD | EXPR_OP_SUB | EXPR_OP_MUL | EXPR_OP_DIV | EXPR_OP_POW
            )
        {
            self.offset += 1;
        }
    }

    fn skip_group_delimiter_before_infix(&mut self) {
        if self.offset >= self.words.len() || self.words[self.offset] != 0x000c {
            return;
        }
        let infix_index = self.offset + 1;
        if !matches!(
            self.words.get(infix_index).copied(),
            Some(EXPR_OP_ADD | EXPR_OP_SUB | EXPR_OP_MUL | EXPR_OP_DIV | EXPR_OP_POW)
        ) {
            return;
        }
        let has_group_close_before_nested_group = self.words[infix_index + 1..]
            .iter()
            .copied()
            .find(|word| matches!(*word, 0x000b | 0x000c))
            == Some(0x000c);
        if has_group_close_before_nested_group {
            self.offset += 1;
        }
    }

    fn parse_expr(&mut self, min_bp: u8) -> Result<FunctionAst, FunctionExprParseError> {
        let mut lhs = self.parse_prefix()?;
        loop {
            self.skip_infix_delimiters();
            let Some((op, left_bp, right_bp)) = self.peek_infix()? else {
                break;
            };
            if left_bp < min_bp {
                break;
            }
            let _ = self.bump()?;
            let rhs = self.parse_expr(right_bp)?;
            lhs = FunctionAst::Binary {
                lhs: Box::new(lhs),
                op,
                rhs: Box::new(rhs),
            };
        }
        Ok(lhs)
    }

    fn parse_group_body(&mut self, min_bp: u8) -> Result<FunctionAst, FunctionExprParseError> {
        let mut lhs = self.parse_prefix()?;
        loop {
            self.skip_group_delimiter_before_infix();
            let Some((op, left_bp, right_bp)) = self.peek_infix()? else {
                break;
            };
            if left_bp < min_bp {
                break;
            }
            let _ = self.bump()?;
            let rhs = self.parse_group_body(right_bp)?;
            lhs = FunctionAst::Binary {
                lhs: Box::new(lhs),
                op,
                rhs: Box::new(rhs),
            };
        }
        Ok(lhs)
    }

    fn parse_expr_no_delim(&mut self, min_bp: u8) -> Result<FunctionAst, FunctionExprParseError> {
        let mut lhs = self.parse_prefix()?;
        while let Some((op, left_bp, right_bp)) = self.peek_infix()? {
            if left_bp < min_bp {
                break;
            }
            let _ = self.bump()?;
            let rhs = self.parse_expr_no_delim(right_bp)?;
            lhs = FunctionAst::Binary {
                lhs: Box::new(lhs),
                op,
                rhs: Box::new(rhs),
            };
        }
        Ok(lhs)
    }

    fn parse_prefix(&mut self) -> Result<FunctionAst, FunctionExprParseError> {
        let offset = self.base_offset + self.offset;
        match self.bump()? {
            GroupedFunctionToken::Variable => Ok(FunctionAst::Variable),
            GroupedFunctionToken::PiAngle => Ok(FunctionAst::PiAngle),
            GroupedFunctionToken::Parameter(binding) => Ok(binding
                .expr
                .unwrap_or(FunctionAst::Parameter(binding.name, binding.value))),
            GroupedFunctionToken::Constant(value) => Ok(FunctionAst::Constant(value)),
            GroupedFunctionToken::Unary(op) => {
                let expr = if matches!(self.peek()?, Some(GroupedFunctionToken::LParen)) {
                    self.parse_unary_grouped_argument()?
                } else {
                    self.parse_expr_no_delim(0)?
                };
                Ok(unary_ast(op, expr))
            }
            GroupedFunctionToken::Add => self.parse_prefix(),
            GroupedFunctionToken::Sub => {
                let expr = self.parse_prefix()?;
                Ok(FunctionAst::Binary {
                    lhs: Box::new(FunctionAst::Constant(0.0)),
                    op: BinaryOp::Sub,
                    rhs: Box::new(expr),
                })
            }
            GroupedFunctionToken::LParen => {
                let expr = self.parse_group_body(0)?;
                match self.bump()? {
                    GroupedFunctionToken::RParen => Ok(expr),
                    found => Err(FunctionExprParseError::UnexpectedToken {
                        offset,
                        found: grouped_to_function_token(found),
                    }),
                }
            }
            GroupedFunctionToken::RParen => Err(FunctionExprParseError::UnexpectedToken {
                offset,
                found: FunctionToken::Terminator,
            }),
            found @ (GroupedFunctionToken::Mul
            | GroupedFunctionToken::Div
            | GroupedFunctionToken::Pow) => Err(FunctionExprParseError::UnexpectedToken {
                offset,
                found: grouped_to_function_token(found),
            }),
        }
    }

    fn parse_unary_grouped_argument(&mut self) -> Result<FunctionAst, FunctionExprParseError> {
        let offset = self.base_offset + self.offset;
        match self.bump()? {
            GroupedFunctionToken::LParen => {
                let expr = self.parse_group_body(0)?;
                if self.allow_unclosed_unary_argument && self.offset >= self.words.len() {
                    return Ok(expr);
                }
                match self.bump()? {
                    GroupedFunctionToken::RParen => Ok(expr),
                    found => Err(FunctionExprParseError::UnexpectedToken {
                        offset,
                        found: grouped_to_function_token(found),
                    }),
                }
            }
            found => Err(FunctionExprParseError::UnexpectedToken {
                offset,
                found: grouped_to_function_token(found),
            }),
        }
    }

    fn peek_infix(&self) -> Result<Option<(BinaryOp, u8, u8)>, FunctionExprParseError> {
        Ok(match self.peek()? {
            Some(GroupedFunctionToken::Add) => Some((BinaryOp::Add, 1, 2)),
            Some(GroupedFunctionToken::Sub) => Some((BinaryOp::Sub, 1, 2)),
            Some(GroupedFunctionToken::Mul) => Some((BinaryOp::Mul, 3, 4)),
            Some(GroupedFunctionToken::Div) => Some((BinaryOp::Div, 3, 4)),
            Some(GroupedFunctionToken::Pow) => Some((BinaryOp::Pow, 5, 5)),
            Some(GroupedFunctionToken::RParen) | None => None,
            _ => None,
        })
    }
}

fn grouped_to_function_token(token: GroupedFunctionToken) -> FunctionToken {
    match token {
        GroupedFunctionToken::Add => FunctionToken::Add,
        GroupedFunctionToken::Sub => FunctionToken::Sub,
        GroupedFunctionToken::Mul => FunctionToken::Mul,
        GroupedFunctionToken::Div => FunctionToken::Div,
        GroupedFunctionToken::Pow => FunctionToken::Pow,
        GroupedFunctionToken::RParen => FunctionToken::Terminator,
        GroupedFunctionToken::Variable => FunctionToken::Variable,
        GroupedFunctionToken::PiAngle => FunctionToken::PiAngle,
        GroupedFunctionToken::Parameter(binding) => FunctionToken::Parameter(binding),
        GroupedFunctionToken::Unary(op) => FunctionToken::Unary(op),
        GroupedFunctionToken::Constant(value) => FunctionToken::Constant(value),
        GroupedFunctionToken::LParen => FunctionToken::Terminator,
    }
}

fn lex_grouped_function_token(
    word: u16,
    parameters: &BTreeMap<u16, ParameterBinding>,
    offset: usize,
) -> Result<GroupedFunctionToken, FunctionExprParseError> {
    Ok(match word {
        EXPR_OP_ADD => GroupedFunctionToken::Add,
        EXPR_OP_SUB => GroupedFunctionToken::Sub,
        EXPR_OP_MUL => GroupedFunctionToken::Mul,
        EXPR_OP_DIV => GroupedFunctionToken::Div,
        EXPR_OP_POW => GroupedFunctionToken::Pow,
        0x000b => GroupedFunctionToken::LParen,
        0x000c => GroupedFunctionToken::RParen,
        EXPR_PI_WORD => GroupedFunctionToken::PiAngle,
        EXPR_VARIABLE_WORD => GroupedFunctionToken::Variable,
        _ if (word & EXPR_PARAMETER_MASK) == EXPR_PARAMETER_PREFIX => {
            let parameter_index = word & 0x000f;
            GroupedFunctionToken::Parameter(parameters.get(&parameter_index).cloned().ok_or(
                FunctionExprParseError::MissingParameterBinding {
                    offset,
                    parameter_index,
                },
            )?)
        }
        _ if decode_unary_function(word).is_some() => {
            GroupedFunctionToken::Unary(decode_unary_function(word).unwrap())
        }
        _ => GroupedFunctionToken::Constant(f64::from(word)),
    })
}

#[allow(dead_code)]
fn parse_grouped_function_expr(
    payload: &[u8],
    parameters: &BTreeMap<u16, ParameterBinding>,
) -> Result<FunctionAst, FunctionExprParseError> {
    let words = payload
        .chunks_exact(2)
        .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
        .collect::<Vec<_>>();
    parse_grouped_function_expr_from_words(&words, parameters)
}

fn parse_grouped_function_expr_from_words(
    words: &[u16],
    parameters: &BTreeMap<u16, ParameterBinding>,
) -> Result<FunctionAst, FunctionExprParseError> {
    if let Some(marker_index) = words
        .windows(2)
        .position(|pair| *pair == FUNCTION_EXPR_MARKER_A || *pair == FUNCTION_EXPR_MARKER_B)
    {
        return parse_grouped_function_expr_at(words, marker_index + 2, parameters);
    }
    if words.first().copied() != Some(0x000b) {
        return Err(FunctionExprParseError::NoExpressionFound {
            word_len: words.len(),
        });
    }
    parse_grouped_function_expr_at(words, 0, parameters)
}

fn parse_grouped_function_expr_at(
    words: &[u16],
    start: usize,
    parameters: &BTreeMap<u16, ParameterBinding>,
) -> Result<FunctionAst, FunctionExprParseError> {
    let mut parser = GroupedFunctionParser::new(&words[start..], parameters, start)
        .allowing_unclosed_unary_argument();
    let expr = parser.parse_expr(0)?;
    if !parsed_contains_symbol(&expr) {
        return Err(FunctionExprParseError::NoExpressionFound {
            word_len: words.len(),
        });
    }
    let remaining = &parser.words[parser.offset..];
    if remaining.is_empty() || remaining.iter().all(|word| *word == 0x000c) {
        Ok(expr)
    } else {
        Err(FunctionExprParseError::NoExpressionFound {
            word_len: words.len(),
        })
    }
}

fn parse_function_expr_from(
    words: &[u16],
    start: usize,
    parameters: &BTreeMap<u16, ParameterBinding>,
) -> Result<(FunctionAst, usize), FunctionExprParseError> {
    let mut parser = FunctionExprParser::new(&words[start..], parameters, start);
    let parsed = parser.parse_expr()?;
    Ok((parsed, start + parser.words_consumed()))
}

fn embedded_calculate_expr_start(words: &[u16]) -> Option<usize> {
    if let Some(start) = words
        .windows(7)
        .enumerate()
        .find_map(|(index, window)| {
            (window[0] == RECORD_FUNCTION_EXPR_PAYLOAD as u16
                && window[1] == 0
                && window[4] == crate::format::GroupKind::FunctionExpr.raw()
                && window[5] == 0)
                .then_some(index + 6)
        })
        .and_then(|count_index| {
            let word_count = usize::from(*words.get(count_index)?);
            let start = words.len().checked_sub(word_count)?;
            (word_count > 0 && start > count_index && start < words.len()).then_some(start)
        })
    {
        return Some(start);
    }

    words
        .windows(2)
        .rposition(|pair| pair == [0x0112, 0x0000])
        .map(|marker_index| marker_index + 2)
        .filter(|start| *start < words.len())
}

fn has_ignorable_expr_suffix(words: &[u16], end: usize) -> bool {
    if end >= words.len() {
        return true;
    }
    let suffix = &words[end..];
    matches!(
        suffix,
        [0x000c | 0x0201 | 0x0101] | [0x0000, 0x0101] | [0x0000, 0x0000, 0x0101]
    )
}

fn parsed_contains_symbol(parsed: &FunctionAst) -> bool {
    function_ast_contains_symbol(parsed)
}

#[cfg(test)]
mod parse_tests {
    use super::{
        FunctionExprParseError, ParameterBinding, decode_payload_function_expr,
        group_function_payload, parse_function_expr, try_decode_function_expr,
        try_decode_inner_function_expr, try_decode_plot_component_expr,
        try_decode_standalone_function_expr,
    };
    use crate::gsp::GspFile;
    use crate::runtime::extract::points::collect_point_objects;
    use crate::runtime::extract::shapes::collect_raw_object_anchors;
    use crate::runtime::functions::{
        BinaryOp, FunctionAst, FunctionExpr, UnaryFunction, evaluate_expr_with_parameters,
        function_expr_label,
    };
    use crate::util::hex_bytes;
    use std::collections::BTreeMap;
    use std::fs;

    fn payload_from_words(words: &[u16]) -> Vec<u8> {
        words
            .iter()
            .flat_map(|word| word.to_le_bytes())
            .collect::<Vec<_>>()
    }

    fn assert_payload_parse_failed(error: FunctionExprParseError, payload: &[u8]) {
        let message = error.to_string();
        assert!(
            message.contains("failed to parse function expression payload"),
            "expected payload parse failure, got {message}"
        );
        assert!(
            message.contains(&format!("payload={}", hex_bytes(payload))),
            "expected failed payload bytes in error, got {message}"
        );
    }

    #[test]
    fn reports_missing_parameter_binding_with_offset() {
        let payload = payload_from_words(&[0x0094, 0x0001, 0x6001]);
        assert_eq!(
            parse_function_expr(&payload, &BTreeMap::new()),
            Err(FunctionExprParseError::MissingParameterBinding {
                offset: 2,
                parameter_index: 1,
            })
        );
    }

    #[test]
    fn reports_invalid_unary_operand_with_offset() {
        let payload = payload_from_words(&[0x0094, 0x0001, 0x2006]);
        assert_eq!(
            parse_function_expr(&payload, &BTreeMap::new()),
            Err(FunctionExprParseError::InvalidUnaryOperand {
                offset: 2,
                opcode: 0x2006,
            })
        );
    }

    #[test]
    fn decodes_grouped_calc_expr_with_division_outside_parentheses() {
        let payload = payload_from_words(&[
            2300, 0, 22, 0, 4, 0, 10, 132, 3, 12348, 62, 61361, 6, 0, 2, 43704, 2311, 0, 78, 0, 48,
            0, 9, 4, 1052, 3, 274, 0, 61584, 0, 46661, 91, 0, 0, 274, 0, 940, 31123, 22472, 273,
            63648, 146, 53421, 31129, 160, 1, 11, 24576, 4100, 2, 4096, 1, 12, 4099, 2,
        ]);
        let parameters = BTreeMap::from([(
            0u16,
            ParameterBinding {
                name: "t₁".to_string(),
                value: 1.0,
                expr: None,
            },
        )]);
        assert_eq!(
            try_decode_inner_function_expr(&payload, &parameters).ok(),
            Some(FunctionExpr::Parsed(FunctionAst::Binary {
                lhs: Box::new(FunctionAst::Binary {
                    lhs: Box::new(FunctionAst::Binary {
                        lhs: Box::new(FunctionAst::Parameter("t₁".to_string(), 1.0)),
                        op: BinaryOp::Pow,
                        rhs: Box::new(FunctionAst::Constant(2.0)),
                    }),
                    op: BinaryOp::Add,
                    rhs: Box::new(FunctionAst::Constant(1.0)),
                }),
                op: BinaryOp::Div,
                rhs: Box::new(FunctionAst::Constant(2.0)),
            }))
        );
    }

    #[test]
    fn decodes_grouped_sign_membership_expr_without_postfix_flattening() {
        let payload = payload_from_words(&[
            0x000b, 0x200a, 0x6000, 0x1000, 0x6001, 0x1001, 0x000f, 0x000c, 0x1000, 0x0001, 0x000c,
            0x1003, 0x0002,
        ]);
        let parameters = BTreeMap::from([
            (
                0u16,
                ParameterBinding {
                    name: "R".to_string(),
                    value: 1.0,
                    expr: None,
                },
            ),
            (
                1u16,
                ParameterBinding {
                    name: "r".to_string(),
                    value: 0.5,
                    expr: None,
                },
            ),
        ]);
        let expr = try_decode_inner_function_expr(&payload, &parameters).expect("expression");
        assert_eq!(
            function_expr_label(expr.clone()),
            "(sgn(R + r - x) + 1) / 2"
        );
        assert_eq!(
            evaluate_expr_with_parameters(&expr, 1.6, &BTreeMap::new()),
            Some(0.0)
        );
    }

    #[test]
    fn decodes_inline_zero_display_with_embedded_static_sqrt_expr() {
        let payload = payload_from_words(&[
            2300, 0, 22, 0, 4, 0, 10, 5, 3, 12348, 62, 4102, 6, 0, 2, 24578, 2311, 0, 66, 0, 48, 0,
            3, 4, 0, 3, 42864, 495, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 23, 0, 0, 0,
            59782, 30530, 8199, 6, 12,
        ]);
        let expr = decode_payload_function_expr(&payload, &BTreeMap::new()).expect("expression");
        assert_eq!(
            expr,
            FunctionExpr::Parsed(FunctionAst::Unary {
                op: UnaryFunction::Sqrt,
                expr: Box::new(FunctionAst::Constant(6.0)),
            })
        );
        assert_eq!(
            evaluate_expr_with_parameters(&expr, 0.0, &BTreeMap::new()),
            Some(6.0_f64.sqrt())
        );
    }

    #[test]
    fn rejects_unmarked_chessboard_depth_expr_without_family_decoder() {
        let payload = payload_from_words(&[
            2300, 0, 22, 0, 4, 0, 10, 274, 3, 12348, 62, 0, 6, 0, 2, 0, 2311, 0, 78, 0, 48, 0, 9,
            4, 63952, 3, 0, 0, 63964, 18, 65535, 65535, 4437, 87, 51443, 86, 274, 0, 61589, 0,
            53072, 99, 63856, 18, 45200, 2303, 11, 24576, 4100, 2, 4097, 1, 12, 4099, 2,
        ]);
        let parameters = BTreeMap::from([(
            0u16,
            ParameterBinding {
                name: "trunc((m₁ + 2))".to_string(),
                value: 9.0,
                expr: None,
            },
        )]);
        let error = try_decode_inner_function_expr(&payload, &parameters)
            .expect_err("unmarked grouped payload must not be rescued by broad fallback");
        assert_payload_parse_failed(error, &payload);
    }

    #[test]
    fn decodes_parameter_curve_component_payload_as_plain_function_definition() {
        let data = include_bytes!("../../../tests/fixtures/gsp/static/parameter_curve.gsp");
        let file = GspFile::parse(data).expect("fixture parses");
        let groups = file.object_groups();
        let y_group = &groups[0];
        assert_eq!(
            try_decode_plot_component_expr(&file, &groups, y_group).ok(),
            Some(FunctionExpr::SinIdentity)
        );
    }

    #[test]
    fn decodes_parameter_curve1_second_function_with_grouped_unary_argument() {
        let data = include_bytes!("../../../tests/fixtures/gsp/static/parameter_curve1.gsp");
        let file = GspFile::parse(data).expect("fixture parses");
        let groups = file.object_groups();
        let group = &groups[1];
        assert_eq!(
            try_decode_standalone_function_expr(&file, &groups, group).ok(),
            Some(FunctionExpr::Parsed(FunctionAst::Unary {
                op: crate::runtime::functions::UnaryFunction::Cos,
                expr: Box::new(FunctionAst::Binary {
                    lhs: Box::new(FunctionAst::Constant(2.0)),
                    op: BinaryOp::Mul,
                    rhs: Box::new(FunctionAst::Variable),
                }),
            }))
        );
    }

    #[test]
    fn decodes_music_fixture_function_from_embedded_postfix_payload() {
        let data = include_bytes!("../../../tests/fixtures/gsp/music.gsp");
        let file = GspFile::parse(data).expect("fixture parses");
        let groups = file.object_groups();
        let group = groups.get(6).expect("music function payload object #7");

        assert_eq!(
            try_decode_function_expr(&file, &groups, group).ok(),
            Some(FunctionExpr::Parsed(FunctionAst::Binary {
                lhs: Box::new(FunctionAst::Constant(5.0)),
                op: BinaryOp::Mul,
                rhs: Box::new(FunctionAst::Unary {
                    op: crate::runtime::functions::UnaryFunction::Sin,
                    expr: Box::new(FunctionAst::Binary {
                        lhs: Box::new(FunctionAst::Constant(25.0)),
                        op: BinaryOp::Mul,
                        rhs: Box::new(FunctionAst::Variable),
                    }),
                }),
            }))
        );
    }

    #[test]
    fn decodes_angle_helper_payload_kind_41_from_sample() {
        let Ok(data) = fs::read("tests/Samples/个人专栏/王伟君作品/多边形外角和(王伟君).gsp")
        else {
            return;
        };
        let file = GspFile::parse(&data).expect("sample parses");
        let groups = file.object_groups();
        let angle_group = groups
            .iter()
            .find(|group| group.ordinal == 23)
            .expect("expected angle helper group");

        let expr = try_decode_function_expr(&file, &groups, angle_group)
            .expect("expected kind 41 helper to decode as a function expression");

        let value = match expr {
            FunctionExpr::Constant(value) => value,
            other => panic!("expected constant angle helper, got {other:?}"),
        };

        assert!(
            (value - 75.31158261667414).abs() < 1e-6,
            "expected sample angle helper to decode from payload, got {value}"
        );
    }

    #[test]
    fn decodes_ratio_helper_payload_kind_47_from_square_roll_sample() {
        let Ok(data) = fs::read("tests/Samples/个人专栏/况永胜作品/正方形在圆内滚动.gsp")
        else {
            return;
        };
        let file = GspFile::parse(&data).expect("sample parses");
        let groups = file.object_groups();
        let point_map = collect_point_objects(&file, &groups);
        let anchors_without_graph = collect_raw_object_anchors(&file, &groups, &point_map, None);
        let anchors = anchors_without_graph;

        let ratio_group = groups
            .iter()
            .find(|group| group.ordinal == 9)
            .expect("expected ratio helper group");
        let origin = anchors[3].clone().expect("origin anchor");
        let baseline = anchors[6].clone().expect("baseline anchor");
        let target = anchors[7].clone().expect("target anchor");
        let expected = ((target.x - origin.x).powi(2) + (target.y - origin.y).powi(2)).sqrt()
            / ((baseline.x - origin.x).powi(2) + (baseline.y - origin.y).powi(2)).sqrt();

        let refs = crate::runtime::extract::find_indexed_path(&file, ratio_group)
            .expect("ratio helper path")
            .refs;
        let value = super::decode_ratio_helper_group(
            &file,
            &groups,
            &anchors,
            None,
            &refs,
        )
        .unwrap_or_else(|| {
            panic!(
                "expected kind 47 helper to decode from payload geometry; refs={refs:?} origin={origin:?} baseline={baseline:?} target={target:?} expected={expected}"
            )
        });

        assert!(
            (value - expected).abs() < 1e-6,
            "expected sample ratio helper to decode from payload, got {value} vs {expected}"
        );
    }

    #[test]
    fn decodes_distance_helper_payload_kind_86_from_russian_exam_sample() {
        let Ok(data) =
            fs::read("tests/Samples/个人专栏/方益初作品/俄罗斯2004高考题（温州小刀）.gsp")
        else {
            return;
        };
        let file = GspFile::parse(&data).expect("sample parses");
        let groups = file.object_groups();
        let expr_group = groups
            .iter()
            .find(|group| group.ordinal == 17)
            .expect("expected distance helper group");

        let expr = try_decode_function_expr(&file, &groups, expr_group)
            .expect("expected kind 86 helper to decode as a function expression");
        let value = match expr {
            FunctionExpr::Constant(value) => value,
            other => panic!("expected constant distance helper, got {other:?}"),
        };

        assert!(
            value.is_finite() && value > 0.0,
            "expected kind 86 helper to decode to a positive finite distance, got {value}"
        );
    }

    #[test]
    fn decodes_named_alias_payload_kind_120_from_refraction_sample() {
        let Ok(data) = fs::read("tests/Samples/个人专栏/侯仰顺作品/光的折射(蚂蚁制作).gsp")
        else {
            return;
        };
        let file = GspFile::parse(&data).expect("sample parses");
        let groups = file.object_groups();
        let angle_alias = groups
            .iter()
            .find(|group| group.ordinal == 42)
            .expect("expected named alias group");

        let expr = try_decode_function_expr(&file, &groups, angle_alias)
            .expect("expected kind 120 alias to decode as a function expression");
        let value = match expr {
            FunctionExpr::Constant(value) => value,
            other => panic!("expected constant named alias, got {other:?}"),
        };

        assert!(
            value.is_finite() && value > 0.0,
            "expected named alias to decode to a positive finite value, got {value}"
        );
    }

    #[test]
    fn decodes_digit_and_carry_function_expressions_from_exponent_calculator() {
        let Ok(data) = fs::read("tests/Samples/个人专栏/李章博作品/指数计算器（李章博）.gsp")
        else {
            return;
        };
        let file = GspFile::parse(&data).expect("sample parses");
        let groups = file.object_groups();

        let carry_group = groups
            .iter()
            .find(|group| group.ordinal == 209)
            .expect("expected carry function expression group");
        let carry_expr = try_decode_function_expr(&file, &groups, carry_group).expect("expression");
        assert_eq!(
            carry_expr,
            FunctionExpr::Parsed(FunctionAst::Binary {
                lhs: Box::new(FunctionAst::Unary {
                    op: UnaryFunction::Trunc,
                    expr: Box::new(FunctionAst::Binary {
                        lhs: Box::new(FunctionAst::Parameter("m[189]".to_string(), 2.0)),
                        op: BinaryOp::Div,
                        rhs: Box::new(FunctionAst::Constant(10.0)),
                    }),
                }),
                op: BinaryOp::Add,
                rhs: Box::new(FunctionAst::Parameter("a₂*底数".to_string(), 0.0)),
            })
        );

        let digit_group = groups
            .iter()
            .find(|group| group.ordinal == 309)
            .expect("expected digit function expression group");
        let digit_expr = try_decode_function_expr(&file, &groups, digit_group).expect("expression");
        assert_eq!(
            digit_expr,
            FunctionExpr::Parsed(FunctionAst::Binary {
                lhs: Box::new(FunctionAst::Parameter("m[189]".to_string(), 2.0)),
                op: BinaryOp::Sub,
                rhs: Box::new(FunctionAst::Binary {
                    lhs: Box::new(FunctionAst::Unary {
                        op: UnaryFunction::Trunc,
                        expr: Box::new(FunctionAst::Binary {
                            lhs: Box::new(FunctionAst::Parameter("m[189]".to_string(), 2.0)),
                            op: BinaryOp::Div,
                            rhs: Box::new(FunctionAst::Constant(10.0)),
                        }),
                    }),
                    op: BinaryOp::Mul,
                    rhs: Box::new(FunctionAst::Constant(10.0)),
                }),
            })
        );
    }

    #[test]
    fn decodes_polygon_area_payload_kind_42_from_rubber_band_sample() {
        let Ok(data) = fs::read("tests/Samples/个人专栏/侯仰顺作品/橡皮筋大战钉子(蚂蚁制作).gsp")
        else {
            return;
        };
        let file = GspFile::parse(&data).expect("sample parses");
        let groups = file.object_groups();
        let area_group = groups
            .iter()
            .find(|group| group.ordinal == 26)
            .expect("expected polygon area helper group");

        let expr = try_decode_function_expr(&file, &groups, area_group)
            .expect("expected kind 42 helper to decode as a function expression");
        let value = match expr {
            FunctionExpr::Constant(value) => value,
            other => panic!("expected constant polygon area value, got {other:?}"),
        };

        assert!(
            value.is_finite() && value > 0.0,
            "expected polygon area helper to decode to a positive finite value, got {value}"
        );
    }

    #[test]
    fn decodes_radius_value_payload_kind_46_from_timer_sample() {
        let Ok(data) = fs::read("tests/Samples/个人专栏/向忠作品/计时器3.gsp") else {
            return;
        };
        let file = GspFile::parse(&data).expect("sample parses");
        let groups = file.object_groups();
        let radius_group = groups
            .iter()
            .find(|group| group.ordinal == 47)
            .expect("expected radius helper group");

        let expr = try_decode_function_expr(&file, &groups, radius_group)
            .expect("expected kind 46 helper to decode as a function expression");
        let value = match expr {
            FunctionExpr::Constant(value) => value,
            other => panic!("expected constant radius helper, got {other:?}"),
        };

        assert!(
            value.is_finite() && value > 0.0,
            "expected radius helper to decode to a positive finite value, got {value}"
        );
    }

    #[test]
    fn decodes_coordinate_x_value_from_explicit_axis_sample() {
        let Ok(data) = fs::read("tests/Samples/个人专栏/田野风作品/函数图象(田野风).gsp")
        else {
            return;
        };
        let file = GspFile::parse(&data).expect("sample parses");
        let groups = file.object_groups();
        let coordinate_group = groups
            .iter()
            .find(|group| group.ordinal == 239)
            .expect("expected coordinate helper group");
        let expr = try_decode_function_expr(&file, &groups, coordinate_group)
            .expect("expected explicit axis coordinate helper to decode");
        let value = match expr {
            FunctionExpr::Constant(value) => value,
            other => panic!("expected constant coordinate helper, got {other:?}"),
        };

        assert!(
            value.is_finite(),
            "expected coordinate helper to decode to a finite value, got {value}"
        );
    }

    #[test]
    fn rejects_unmarked_function_definition_payload_from_sine_transform_sample() {
        let Ok(data) = fs::read("tests/Samples/个人专栏/向忠作品/正弦型函数图象变换.gsp")
        else {
            return;
        };
        let file = GspFile::parse(&data).expect("sample parses");
        let groups = file.object_groups();
        let function_group = groups
            .iter()
            .find(|group| group.ordinal == 306)
            .expect("expected function definition group");

        let payload = group_function_payload(&file, function_group).expect("function payload");
        let error = try_decode_function_expr(&file, &groups, function_group)
            .expect_err("unmarked grouped payload must not be rescued by broad fallback");
        assert_payload_parse_failed(error, payload);
    }

    #[test]
    fn rejects_liyougui_function_iteration_payload_without_family_decoder() {
        let data =
            include_bytes!("../../../tests/Samples/个人专栏/李有贵作品/函数图象迭代(liyougui).gsp");
        let file = GspFile::parse(data).expect("fixture parses");
        let groups = file.object_groups();
        let payload = group_function_payload(&file, &groups[14]).expect("#15 payload");
        let error = try_decode_function_expr(&file, &groups, &groups[14])
            .expect_err("unmarked grouped payload must not be rescued by broad fallback");
        assert_payload_parse_failed(error, payload);
    }
}
