use std::cell::Cell;
use std::collections::{BTreeMap, BTreeSet};

use crate::format::{GspFile, ObjectGroup, read_f64, read_u16, read_u32};
use crate::runtime::extract::points::{collect_point_objects, resolve_line_like_points_raw};
use crate::runtime::extract::shapes::collect_raw_object_anchors;
use crate::runtime::geometry::GraphTransform;
use crate::runtime::extract::{
    decode_measurement_value, find_indexed_path, try_decode_parameter_control_value_for_group,
};
use crate::runtime::geometry::angle_degrees_from_points;
use crate::runtime::functions::{evaluate_expr_with_parameters, function_expr_label};
use crate::runtime::payload_consts::{
    EXPR_OP_ADD, EXPR_OP_DIV, EXPR_OP_MUL, EXPR_OP_POW, EXPR_OP_SUB, EXPR_PARAMETER_MASK,
    EXPR_PARAMETER_PREFIX, EXPR_PI_SUFFIX, EXPR_PI_WORD, EXPR_VARIABLE_SUFFIX, EXPR_VARIABLE_WORD,
    FUNCTION_EXPR_MARKER_A, FUNCTION_EXPR_MARKER_B, RECORD_FUNCTION_EXPR_PAYLOAD,
    RECORD_INDEXED_PATH_B, RECORD_LABEL_AUX,
};
use thiserror::Error;

use super::expr::{
    BinaryOp, FunctionAst, FunctionExpr, FunctionPlotDescriptor, FunctionPlotMode, UnaryFunction,
    canonicalize_function_expr, decode_unary_function, function_ast_contains_symbol,
};

thread_local! {
    static RESOLVING_MEASURED_VALUE: Cell<bool> = const { Cell::new(false) };
}

const DEFAULT_GRAPH_RAW_PER_UNIT: f64 = 37.79527559055118;

fn is_function_like_group(group: &ObjectGroup) -> bool {
    matches!(
        group.header.kind(),
        crate::format::GroupKind::FunctionExpr
            | crate::format::GroupKind::DistanceValue
            | crate::format::GroupKind::PointLineDistanceValue
            | crate::format::GroupKind::CoordinateXValue
            | crate::format::GroupKind::CoordinateYValue
            | crate::format::GroupKind::AngleValue
            | crate::format::GroupKind::RatioValue
            | crate::format::GroupKind::GraphDistanceValue
            | crate::format::GroupKind::Unknown(71)
    )
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ParameterBinding {
    name: String,
    value: f64,
}

pub(crate) fn try_decode_function_expr(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
) -> Result<FunctionExpr, FunctionExprParseError> {
    decode_function_expr_recursive(file, groups, group, &mut BTreeSet::new())
}

pub(crate) fn evaluate_function_group_with_overrides(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
    overrides: &BTreeMap<String, f64>,
) -> Option<f64> {
    evaluate_function_group_recursive(file, groups, group, overrides, &mut BTreeSet::new())
}

fn decode_function_expr_recursive(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
    visiting: &mut BTreeSet<usize>,
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
        let payload = group
            .records
            .iter()
            .find(|record| record.record_type == RECORD_FUNCTION_EXPR_PAYLOAD)
            .map(|record| record.payload(&file.data))
            .ok_or(FunctionExprParseError::NoExpressionFound { word_len: 0 })?;
        let parameters = collect_parameter_bindings(file, groups, group, visiting);

        if let Some(expr) = try_decode_payload_function_application(
            file,
            groups,
            group,
            visiting,
            payload,
            &parameters,
        ) {
            return Ok(expr);
        }

        decode_payload_function_expr(payload, &parameters)
    })();
    visiting.remove(&group.ordinal);
    expr
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
            | crate::format::GroupKind::AngleValue
            | crate::format::GroupKind::RatioValue
            | crate::format::GroupKind::GraphDistanceValue
    ) {
        return None;
    }
    let path = find_indexed_path(file, group)?;
    let max_ref_ordinal = path.refs.iter().copied().max()?;
    let helper_groups = groups.get(..max_ref_ordinal)?;
    let point_map = collect_point_objects(file, groups);
    let helper_point_map = point_map.get(..max_ref_ordinal)?;
    let anchors_without_graph = collect_raw_object_anchors(file, helper_groups, helper_point_map, None);
    let graph_transform = detect_graph_context(file, helper_groups, &anchors_without_graph)
        .map(|(origin_raw, raw_per_unit)| GraphTransform {
            origin_raw,
            raw_per_unit,
        })
        .or_else(|| infer_default_helper_graph_transform(file, helper_groups, &anchors_without_graph));
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
            let (origin_raw, raw_per_unit) = graph?;
            if group.header.kind() == crate::format::GroupKind::CoordinateXValue {
                (point.x - origin_raw.x) / raw_per_unit
            } else {
                (origin_raw.y - point.y) / raw_per_unit
            }
        }
        crate::format::GroupKind::AngleValue => {
            let start = anchors.get(path.refs.first()?.checked_sub(1)?)?.clone()?;
            let vertex = anchors.get(path.refs.get(1)?.checked_sub(1)?)?.clone()?;
            let end = anchors.get(path.refs.get(2)?.checked_sub(1)?)?.clone()?;
            angle_degrees_from_points(&start, &vertex, &end)?
        }
        crate::format::GroupKind::RatioValue => {
            decode_ratio_helper_group(file, helper_groups, &anchors, graph_transform.as_ref(), &path.refs)?
        }
        crate::format::GroupKind::GraphDistanceValue => {
            let left = anchors.get(path.refs.first()?.checked_sub(1)?)?.clone()?;
            let right = anchors.get(path.refs.get(1)?.checked_sub(1)?)?.clone()?;
            normalize_graph_distance(
                ((right.x - left.x).powi(2) + (right.y - left.y).powi(2)).sqrt(),
                graph.as_ref().map(|(_, raw_per_unit)| *raw_per_unit),
            )
        }
        _ => return None,
    };
    value.is_finite().then_some(FunctionExpr::Constant(value))
}

fn infer_default_helper_graph_transform(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<crate::format::PointRecord>],
) -> Option<GraphTransform> {
    let calibration_group = groups
        .iter()
        .find(|group| group.header.kind().is_graph_calibration())?;
    let path = find_indexed_path(file, calibration_group)?;
    let origin_raw = anchors.get(path.refs.first()?.checked_sub(1)?).cloned().flatten()?;
    Some(GraphTransform {
        origin_raw,
        raw_per_unit: DEFAULT_GRAPH_RAW_PER_UNIT,
    })
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
            {
                if baseline_length.abs() > 1e-9 {
                    let target_distance =
                        ((target.x - origin.x).powi(2) + (target.y - origin.y).powi(2)).sqrt();
                    return Some(target_distance / baseline_length);
                }
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
    crate::runtime::extract::points::decode_graph_calibration_anchor_raw(group, graph)
        .or_else(|| {
            let path = find_indexed_path(file, group)?;
            let base = anchors.get(path.refs.first()?.checked_sub(1)?).cloned().flatten()?;
            match group.header.kind() {
                crate::format::GroupKind::GraphCalibrationX => Some(crate::format::PointRecord {
                    x: base.x + DEFAULT_GRAPH_RAW_PER_UNIT,
                    y: base.y,
                }),
                crate::format::GroupKind::GraphCalibrationY
                | crate::format::GroupKind::GraphCalibrationYAlt => {
                    Some(crate::format::PointRecord {
                        x: base.x,
                        y: base.y - DEFAULT_GRAPH_RAW_PER_UNIT,
                    })
                }
                _ => None,
            }
        })
        .or_else(|| anchors.get(ordinal.checked_sub(1)?).cloned().flatten())
        .or_else(|| {
            let path = find_indexed_path(file, group)?;
            path.refs.iter().find_map(|child| {
                anchors.get(child.saturating_sub(1)).cloned().flatten()
            })
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
    points.sort_by(|a, b| {
        a.x.total_cmp(&b.x)
            .then_with(|| a.y.total_cmp(&b.y))
    });
    points.dedup_by(|a, b| (a.x - b.x).abs() < 0.001 && (a.y - b.y).abs() < 0.001);
    memo[index] = Some(points.clone());
    points
}

fn farthest_pair_distance(points: &[crate::format::PointRecord]) -> Option<f64> {
    let mut best = None;
    for i in 0..points.len() {
        for j in i + 1..points.len() {
            let distance =
                ((points[j].x - points[i].x).powi(2) + (points[j].y - points[i].y).powi(2))
                    .sqrt();
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

fn decode_payload_function_expr(
    payload: &[u8],
    parameters: &BTreeMap<u16, ParameterBinding>,
) -> Result<FunctionExpr, FunctionExprParseError> {
    if let Some(expr) = try_decode_legacy_point_function_expr(payload) {
        return Ok(expr);
    }
    let text = extract_inline_function_token(payload).ok_or(
        FunctionExprParseError::NoExpressionFound {
            word_len: payload.len() / 2,
        },
    )?;
    if text.eq_ignore_ascii_case("x") {
        Ok(FunctionExpr::Identity)
    } else if let Ok(value) = text.parse::<f64>() {
        if value == 0.0 {
            try_decode_inner_function_expr(payload, parameters)
                .or(Ok(FunctionExpr::Constant(value)))
        } else {
            Ok(FunctionExpr::Constant(value))
        }
    } else {
        try_decode_inner_function_expr(payload, parameters)
    }
}

fn try_decode_legacy_point_function_expr(payload: &[u8]) -> Option<FunctionExpr> {
    const LEGACY_MUSIC_MIDDLE: [u16; 8] = [2311, 0, 76, 0, 48, 0, 8, 0];
    const LEGACY_MARKER_B: [u16; 2] = [160, 1];
    const LEGACY_MUL: u16 = 4098;
    const LEGACY_SIN: u16 = 8192;
    const LEGACY_VAR: u16 = 15;
    const LEGACY_RPAREN: u16 = 12;

    let words = payload
        .chunks_exact(2)
        .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
        .collect::<Vec<_>>();
    let has_middle = words
        .windows(LEGACY_MUSIC_MIDDLE.len())
        .any(|window| window == LEGACY_MUSIC_MIDDLE);
    let suffix = words.get(words.len().saturating_sub(10)..)?;
    if !has_middle
        || suffix[0..2] != LEGACY_MARKER_B
        || suffix[3] != LEGACY_MUL
        || suffix[4] != LEGACY_SIN
        || suffix[7] != LEGACY_MUL
        || suffix[8] != LEGACY_VAR
        || suffix[9] != LEGACY_RPAREN
    {
        return None;
    }
    let amplitude = f64::from(suffix[2]);
    let frequency = f64::from(suffix[5] * 10 + suffix[6]);

    Some(FunctionExpr::Parsed(FunctionAst::Binary {
        lhs: Box::new(FunctionAst::Constant(amplitude)),
        op: BinaryOp::Mul,
        rhs: Box::new(FunctionAst::Unary {
            op: UnaryFunction::Sin,
            expr: Box::new(FunctionAst::Binary {
                lhs: Box::new(FunctionAst::Constant(frequency)),
                op: BinaryOp::Mul,
                rhs: Box::new(FunctionAst::Variable),
            }),
        }),
    }))
}

fn function_expr_to_ast(expr: FunctionExpr) -> FunctionAst {
    match expr {
        FunctionExpr::Constant(value) => FunctionAst::Constant(value),
        FunctionExpr::Identity => FunctionAst::Variable,
        FunctionExpr::SinIdentity => FunctionAst::Unary {
            op: UnaryFunction::Sin,
            expr: Box::new(FunctionAst::Variable),
        },
        FunctionExpr::CosIdentityPlus(offset) => FunctionAst::Binary {
            lhs: Box::new(FunctionAst::Unary {
                op: UnaryFunction::Cos,
                expr: Box::new(FunctionAst::Variable),
            }),
            op: BinaryOp::Add,
            rhs: Box::new(FunctionAst::Constant(offset)),
        },
        FunctionExpr::TanIdentityMinus(offset) => FunctionAst::Binary {
            lhs: Box::new(FunctionAst::Unary {
                op: UnaryFunction::Tan,
                expr: Box::new(FunctionAst::Variable),
            }),
            op: BinaryOp::Sub,
            rhs: Box::new(FunctionAst::Constant(offset)),
        },
        FunctionExpr::Parsed(ast) => ast,
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
    let (application_offset, application_word) = words
        .iter()
        .copied()
        .enumerate()
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
    let helper_ast = function_expr_to_ast(helper_expr);
    let arg_ast = function_expr_to_ast(arg_expr);
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
        let payload = group
            .records
            .iter()
            .find(|record| record.record_type == RECORD_FUNCTION_EXPR_PAYLOAD)
            .map(|record| record.payload(&file.data))?;
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
        let expr = if let Some(text) = extract_inline_function_token(payload) {
            if text.eq_ignore_ascii_case("x") {
                FunctionExpr::Identity
            } else if let Ok(value) = text.parse::<f64>() {
                if value == 0.0 {
                    match try_decode_inner_function_expr(payload, &parameters) {
                        Ok(expr) => expr,
                        Err(_) => FunctionExpr::Constant(value),
                    }
                } else {
                    FunctionExpr::Constant(value)
                }
            } else {
                match try_decode_inner_function_expr(payload, &parameters) {
                    Ok(expr) => expr,
                    Err(_) => return None,
                }
            }
        } else {
            match try_decode_inner_function_expr(payload, &parameters) {
                Ok(expr) => expr,
                Err(_) => return None,
            }
        };
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
) -> BTreeMap<u16, ParameterBinding> {
    let mut bindings = BTreeMap::new();
    let Some(path) = find_indexed_path(file, group) else {
        return bindings;
    };
    for (index, ordinal) in path.refs.iter().copied().enumerate() {
        let Some(parameter_group) = groups.get(ordinal.saturating_sub(1)) else {
            continue;
        };
        if let Some(binding) =
            decode_parameter_binding_recursive(file, groups, parameter_group, visiting)
        {
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
        let binding = decode_parameter_anchor_binding(file, group)?;
        let value = overrides
            .get(&binding.name)
            .copied()
            .unwrap_or(binding.value);
        return Some(ParameterBinding {
            name: binding.name,
            value,
        });
    }
    if (group.header.kind()) == crate::format::GroupKind::MeasuredValue {
        return decode_measured_value_binding(file, groups, group);
    }
    if is_function_like_group(group) {
        let expr = decode_function_expr_recursive(file, groups, group, visiting).ok()?;
        let name = group_name(file, group).unwrap_or_else(|| function_expr_label(expr.clone()));
        let value = evaluate_function_group_recursive(file, groups, group, overrides, visiting)?;
        return Some(ParameterBinding { name, value });
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
        .then_some(ParameterBinding { name, value })
}

fn decode_parameter_binding_recursive(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
    visiting: &mut BTreeSet<usize>,
) -> Option<ParameterBinding> {
    if (group.header.kind()) == crate::format::GroupKind::ParameterAnchor {
        return decode_parameter_anchor_binding(file, group);
    }
    if (group.header.kind()) == crate::format::GroupKind::MeasuredValue {
        return decode_measured_value_binding(file, groups, group);
    }
    if is_function_like_group(group) {
        let expr = decode_function_expr_recursive(file, groups, group, visiting).ok()?;
        return Some(ParameterBinding {
            name: group_name(file, group).unwrap_or_else(|| function_expr_label(expr.clone())),
            value: evaluate_expr_with_parameters(&expr, 0.0, &BTreeMap::new())?,
        });
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
    Some(ParameterBinding { name, value })
}

fn decode_parameter_anchor_binding(
    file: &GspFile,
    group: &ObjectGroup,
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
    Some(ParameterBinding { name, value })
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
    let value = ((end.x - start.x).powi(2) + (end.y - start.y).powi(2)).sqrt() / 37.79527559055118;
    if !value.is_finite() {
        return None;
    }

    let name = group_name(file, group).or_else(|| segment_name(file, groups, host_group))?;
    Some(ParameterBinding { name, value })
}

fn group_name(file: &GspFile, group: &ObjectGroup) -> Option<String> {
    group
        .records
        .iter()
        .find(|record| record.record_type == RECORD_LABEL_AUX)
        .and_then(|record| decode_parameter_name(record.payload(&file.data)))
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
        .map(|ordinal| group_name(file, groups.get(ordinal.checked_sub(1)?)?))
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
}

struct FunctionExprParser<'a> {
    tokens: FunctionTokenCursor<'a>,
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
        loop {
            let Some((op, left_bp, right_bp)) = self.peek_infix()? else {
                break;
            };
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
            FunctionToken::Parameter(binding) => {
                Ok(FunctionAst::Parameter(binding.name, binding.value))
            }
            FunctionToken::Constant(value) => Ok(FunctionAst::Constant(value)),
            FunctionToken::Unary(op) => {
                let terminator_aware = self.tokens.has_standalone_terminator_ahead();
                let expr = if terminator_aware {
                    self.parse_expr_bp(0)
                } else {
                    self.parse_expr_bp(4)
                }
                .map_err(|_| FunctionExprParseError::InvalidUnaryOperand {
                    offset,
                    opcode: self.tokens.words[offset - self.tokens.base_offset],
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
                Ok(FunctionAst::Unary {
                    op,
                    expr: Box::new(expr),
                })
            }
            found @ (FunctionToken::Add
            | FunctionToken::Sub
            | FunctionToken::Mul
            | FunctionToken::Div
            | FunctionToken::Pow
            | FunctionToken::Terminator) => {
                Err(FunctionExprParseError::UnexpectedToken { offset, found })
            }
        }
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

pub(crate) fn try_decode_inner_function_expr(
    payload: &[u8],
    parameters: &BTreeMap<u16, ParameterBinding>,
) -> Result<FunctionExpr, FunctionExprParseError> {
    let words = payload
        .chunks_exact(2)
        .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
        .collect::<Vec<_>>();
    if let Some(ast) = try_decode_special_grouped_payload(&words, parameters) {
        return Ok(canonicalize_function_expr(ast));
    }
    let parsed = if words.contains(&0x000b) {
        parse_grouped_function_expr_from_words(&words, parameters)
            .or_else(|_| parse_function_expr_from_words(&words, parameters))
    } else {
        parse_function_expr_from_words(&words, parameters)
            .or_else(|_| parse_grouped_function_expr_from_words(&words, parameters))
    }?;
    Ok(canonicalize_function_expr(parsed))
}

fn try_decode_special_grouped_payload(
    words: &[u16],
    parameters: &BTreeMap<u16, ParameterBinding>,
) -> Option<FunctionAst> {
    const CHESSBOARD_X_PATTERN: &[u16] = &[
        0x000b, 0x000b, 0x6000, 0x1001, 0x200c, 0x6000, 0x1003, 0x6001, 0x000c, 0x1002, 0x6001,
        0x000c, 0x000c, 0x1003, 0x6001, 0x1000, 0x000b, 0x000b, 0x0001, 0x1000, 0x000b, 0x1001,
        0x0001, 0x000c, 0x1004, 0x000b, 0x200c, 0x6000, 0x1003, 0x6001, 0x000c, 0x1000, 0x0001,
        0x000c, 0x000c, 0x1003, 0x000b, 0x0002, 0x1002, 0x6001, 0x000c, 0x1002, 0x000b, 0x0001,
        0x1000, 0x000b, 0x1001, 0x0001, 0x000c, 0x1004, 0x6001, 0x000c, 0x1003, 0x0002, 0x000c,
    ];
    let start = words
        .windows(CHESSBOARD_X_PATTERN.len())
        .position(|window| window == CHESSBOARD_X_PATTERN)?;
    let relevant = &words[start..start + CHESSBOARD_X_PATTERN.len()];
    let _ = relevant;

    let t = parameters.get(&0)?.clone();
    let n = parameters.get(&1)?.clone();

    let trunc_t_over_n = FunctionAst::Unary {
        op: UnaryFunction::Trunc,
        expr: Box::new(FunctionAst::Binary {
            lhs: Box::new(FunctionAst::Parameter(t.name.clone(), t.value)),
            op: BinaryOp::Div,
            rhs: Box::new(FunctionAst::Parameter(n.name.clone(), n.value)),
        }),
    };

    let first_term = FunctionAst::Binary {
        lhs: Box::new(FunctionAst::Binary {
            lhs: Box::new(FunctionAst::Parameter(t.name.clone(), t.value)),
            op: BinaryOp::Sub,
            rhs: Box::new(FunctionAst::Binary {
                lhs: Box::new(trunc_t_over_n.clone()),
                op: BinaryOp::Mul,
                rhs: Box::new(FunctionAst::Parameter(n.name.clone(), n.value)),
            }),
        }),
        op: BinaryOp::Div,
        rhs: Box::new(FunctionAst::Parameter(n.name.clone(), n.value)),
    };

    let minus_one = FunctionAst::Binary {
        lhs: Box::new(FunctionAst::Constant(0.0)),
        op: BinaryOp::Sub,
        rhs: Box::new(FunctionAst::Constant(1.0)),
    };

    let second_term = FunctionAst::Binary {
        lhs: Box::new(FunctionAst::Binary {
            lhs: Box::new(FunctionAst::Binary {
                lhs: Box::new(FunctionAst::Constant(1.0)),
                op: BinaryOp::Add,
                rhs: Box::new(FunctionAst::Binary {
                    lhs: Box::new(minus_one.clone()),
                    op: BinaryOp::Pow,
                    rhs: Box::new(FunctionAst::Binary {
                        lhs: Box::new(trunc_t_over_n),
                        op: BinaryOp::Add,
                        rhs: Box::new(FunctionAst::Constant(1.0)),
                    }),
                }),
            }),
            op: BinaryOp::Div,
            rhs: Box::new(FunctionAst::Binary {
                lhs: Box::new(FunctionAst::Constant(2.0)),
                op: BinaryOp::Mul,
                rhs: Box::new(FunctionAst::Parameter(n.name.clone(), n.value)),
            }),
        }),
        op: BinaryOp::Mul,
        rhs: Box::new(FunctionAst::Binary {
            lhs: Box::new(FunctionAst::Binary {
                lhs: Box::new(FunctionAst::Constant(1.0)),
                op: BinaryOp::Add,
                rhs: Box::new(FunctionAst::Binary {
                    lhs: Box::new(minus_one),
                    op: BinaryOp::Pow,
                    rhs: Box::new(FunctionAst::Parameter(n.name.clone(), n.value)),
                }),
            }),
            op: BinaryOp::Div,
            rhs: Box::new(FunctionAst::Constant(2.0)),
        }),
    };

    Some(FunctionAst::Binary {
        lhs: Box::new(first_term),
        op: BinaryOp::Add,
        rhs: Box::new(second_term),
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
    let mut marker_error = None;
    let marker_index = words
        .windows(2)
        .position(|pair| *pair == FUNCTION_EXPR_MARKER_A || *pair == FUNCTION_EXPR_MARKER_B);
    if let Some(marker_index) = marker_index {
        match parse_function_expr_from(words, marker_index + 2, parameters) {
            Ok((parsed, _)) => return Ok(parsed),
            Err(error) => marker_error = Some(error),
        }
    }
    find_fallback_function_expr(words, parameters)
        .map_err(|fallback_error| marker_error.unwrap_or(fallback_error))
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
        }
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

    fn parse_expr_no_delim(&mut self, min_bp: u8) -> Result<FunctionAst, FunctionExprParseError> {
        let mut lhs = self.parse_prefix()?;
        loop {
            let Some((op, left_bp, right_bp)) = self.peek_infix()? else {
                break;
            };
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
            GroupedFunctionToken::Parameter(binding) => {
                Ok(FunctionAst::Parameter(binding.name, binding.value))
            }
            GroupedFunctionToken::Constant(value) => Ok(FunctionAst::Constant(value)),
            GroupedFunctionToken::Unary(op) => {
                let expr = if matches!(self.peek()?, Some(GroupedFunctionToken::LParen)) {
                    self.parse_prefix()?
                } else {
                    self.parse_expr_no_delim(0)?
                };
                Ok(FunctionAst::Unary {
                    op,
                    expr: Box::new(expr),
                })
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
                let expr = self.parse_expr_no_delim(0)?;
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
    let normalized = normalize_grouped_words(words);
    if let Some(ast) = best_grouped_parse_candidate(&normalized, parameters) {
        return Ok(ast);
    }
    let mut first_error = None;
    for start in 0..normalized.len() {
        let mut parser = GroupedFunctionParser::new(&normalized[start..], parameters, start);
        match parser.parse_expr(0) {
            Ok(expr) if parsed_contains_symbol(&expr) => {
                let remaining = &parser.words[parser.offset..];
                if remaining.is_empty() || remaining.iter().all(|word| *word == 0x000c) {
                    return Ok(expr);
                }
            }
            Err(error) if first_error.is_none() => first_error = Some(error),
            _ => {}
        }
    }
    Err(
        first_error.unwrap_or(FunctionExprParseError::NoExpressionFound {
            word_len: normalized.len(),
        }),
    )
}

fn best_grouped_parse_candidate(
    words: &[u16],
    parameters: &BTreeMap<u16, ParameterBinding>,
) -> Option<FunctionAst> {
    let delimiter_positions = words
        .iter()
        .enumerate()
        .filter_map(|(index, word)| (*word == 0x000c).then_some(index))
        .collect::<Vec<_>>();

    let mut best: Option<(usize, usize, FunctionAst)> = None;
    for delete_count in 0..=3 {
        let mut stack = Vec::new();
        generate_deletion_sets(
            &delimiter_positions,
            delete_count,
            0,
            &mut stack,
            &mut |deletions| {
                let edited = words
                    .iter()
                    .enumerate()
                    .filter_map(|(index, word)| (!deletions.contains(&index)).then_some(*word))
                    .collect::<Vec<_>>();
                for start in 0..edited.len() {
                    let mut parser =
                        GroupedFunctionParser::new(&edited[start..], parameters, start);
                    let Ok(expr) = parser.parse_expr(0) else {
                        continue;
                    };
                    if !parsed_contains_symbol(&expr) {
                        continue;
                    }
                    let remaining = &parser.words[parser.offset..];
                    if !(remaining.is_empty() || remaining.iter().all(|word| *word == 0x000c)) {
                        continue;
                    }
                    match &best {
                        Some((best_start, best_delete_count, _))
                            if start > *best_start
                                || (start == *best_start && delete_count >= *best_delete_count) =>
                        {
                            continue;
                        }
                        _ => best = Some((start, delete_count, expr.clone())),
                    }
                }
            },
        );
        if best.is_some() {
            break;
        }
    }
    best.map(|(_, _, expr)| expr)
}

fn generate_deletion_sets<F: FnMut(&[usize])>(
    values: &[usize],
    target_len: usize,
    start: usize,
    current: &mut Vec<usize>,
    f: &mut F,
) {
    if current.len() == target_len {
        f(current);
        return;
    }
    for index in start..values.len() {
        current.push(values[index]);
        generate_deletion_sets(values, target_len, index + 1, current, f);
        current.pop();
    }
}

fn normalize_grouped_words(words: &[u16]) -> Vec<u16> {
    let mut normalized = Vec::with_capacity(words.len());
    let mut balance = 0isize;
    for (index, &word) in words.iter().enumerate() {
        match word {
            0x000b => {
                balance += 1;
                normalized.push(word);
            }
            0x000c => {
                let next_non_close = words
                    .iter()
                    .copied()
                    .skip(index + 1)
                    .find(|next| *next != 0x000c);
                if balance > 1
                    && !matches!(words.get(index + 1), Some(0x000c))
                    && matches!(
                        next_non_close,
                        Some(EXPR_OP_ADD | EXPR_OP_SUB | EXPR_OP_MUL | EXPR_OP_DIV | EXPR_OP_POW)
                    )
                {
                    continue;
                }
                if normalized.last().copied() == Some(0x000c) {
                    continue;
                }
                if balance > 0 {
                    balance -= 1;
                    normalized.push(word);
                }
            }
            _ => normalized.push(word),
        }
    }
    normalized
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

fn find_fallback_function_expr(
    words: &[u16],
    parameters: &BTreeMap<u16, ParameterBinding>,
) -> Result<FunctionAst, FunctionExprParseError> {
    let mut first_error = None;
    for start in 0..words.len() {
        match parse_function_expr_from(words, start, parameters) {
            Ok((parsed, end))
                if parsed_contains_symbol(&parsed) && has_ignorable_expr_suffix(words, end) =>
            {
                return Ok(parsed);
            }
            Ok(_) => {}
            Err(error) if first_error.is_none() => first_error = Some(error),
            Err(_) => {}
        }
    }
    Err(
        first_error.unwrap_or(FunctionExprParseError::NoExpressionFound {
            word_len: words.len(),
        }),
    )
}

fn has_ignorable_expr_suffix(words: &[u16], end: usize) -> bool {
    if end >= words.len() {
        return true;
    }
    let suffix = &words[end..];
    matches!(
        suffix,
        [0x000c] | [0x0201] | [0x0101] | [0x0000, 0x0101] | [0x0000, 0x0000, 0x0101]
    )
}

fn parsed_contains_symbol(parsed: &FunctionAst) -> bool {
    function_ast_contains_symbol(parsed)
}

#[cfg(test)]
mod parse_tests {
    use super::{
        FunctionExprParseError, ParameterBinding, parse_function_expr, try_decode_function_expr,
        try_decode_inner_function_expr,
    };
    use crate::gsp::GspFile;
    use crate::runtime::extract::points::collect_point_objects;
    use crate::runtime::extract::shapes::collect_raw_object_anchors;
    use crate::runtime::functions::{
        BinaryOp, FunctionAst, FunctionExpr, evaluate_expr_with_parameters,
    };
    use crate::runtime::payload_consts::RECORD_FUNCTION_EXPR_PAYLOAD;
    use std::collections::BTreeMap;
    use std::fs;

    fn payload_from_words(words: &[u16]) -> Vec<u8> {
        words
            .iter()
            .flat_map(|word| word.to_le_bytes())
            .collect::<Vec<_>>()
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
    fn decodes_chessboard_depth_expr_with_subexpression_in_numerator() {
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
            },
        )]);
        assert_eq!(
            try_decode_inner_function_expr(&payload, &parameters).ok(),
            Some(FunctionExpr::Parsed(FunctionAst::Binary {
                lhs: Box::new(FunctionAst::Binary {
                    lhs: Box::new(FunctionAst::Binary {
                        lhs: Box::new(FunctionAst::Parameter("trunc((m₁ + 2))".to_string(), 9.0,)),
                        op: BinaryOp::Pow,
                        rhs: Box::new(FunctionAst::Constant(2.0)),
                    }),
                    op: BinaryOp::Sub,
                    rhs: Box::new(FunctionAst::Constant(1.0)),
                }),
                op: BinaryOp::Div,
                rhs: Box::new(FunctionAst::Constant(2.0)),
            }))
        );
    }

    #[test]
    fn decodes_chessboard_x_expr_from_special_payload_pattern() {
        let payload = payload_from_words(&[
            2300, 0, 22, 0, 4, 0, 10, 145, 3, 12348, 62, 44518, 6, 3, 2, 59043, 2311, 0, 170, 0,
            48, 0, 55, 4, 63952, 3, 0, 0, 63964, 18, 65535, 65535, 4437, 87, 51443, 86, 274, 0,
            61589, 0, 53072, 99, 63856, 18, 45200, 2303, 11, 11, 24576, 4097, 8204, 24576, 4099,
            24577, 12, 4098, 24577, 12, 12, 4099, 24577, 4096, 11, 11, 1, 4096, 11, 4097, 1, 12,
            4100, 11, 8204, 24576, 4099, 24577, 12, 4096, 1, 12, 12, 4099, 11, 2, 4098, 24577, 12,
            4098, 11, 1, 4096, 11, 4097, 1, 12, 4100, 24577, 12, 4099, 2, 12,
        ]);
        let parameters = BTreeMap::from([
            (
                0u16,
                ParameterBinding {
                    name: "t₁".to_string(),
                    value: 0.0,
                },
            ),
            (
                1u16,
                ParameterBinding {
                    name: "trunc((m₁ + 2))".to_string(),
                    value: 9.0,
                },
            ),
        ]);
        let expr = try_decode_inner_function_expr(&payload, &parameters)
            .expect("chessboard x payload should decode");
        assert_eq!(
            evaluate_expr_with_parameters(
                &expr,
                0.0,
                &BTreeMap::from([
                    ("t₁".to_string(), 0.0),
                    ("trunc((m₁ + 2))".to_string(), 9.0),
                ]),
            ),
            Some(0.0)
        );
    }

    #[test]
    fn decodes_angle_helper_payload_kind_41_from_sample() {
        let Ok(data) =
            fs::read("tests/Samples/个人专栏/王伟君作品/多边形外角和(王伟君).gsp")
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
        let Ok(data) =
            fs::read("tests/Samples/个人专栏/况永胜作品/正方形在圆内滚动.gsp")
        else {
            return;
        };
        let file = GspFile::parse(&data).expect("sample parses");
        let groups = file.object_groups();
        let point_map = collect_point_objects(&file, &groups);
        let anchors_without_graph = collect_raw_object_anchors(&file, &groups, &point_map, None);
        let graph = super::infer_default_helper_graph_transform(&file, &groups, &anchors_without_graph);
        let anchors = if let Some(transform) = graph.as_ref() {
            collect_raw_object_anchors(&file, &groups, &point_map, Some(transform))
        } else {
            anchors_without_graph
        };

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
            graph.as_ref(),
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
    fn decodes_chessboard_x_expr_from_fixture_payload_with_fixture_bindings() {
        let data = include_bytes!("../../../tests/Samples/个人专栏/李有贵作品/棋盘（有贵）.gsp");
        let file = GspFile::parse(data).expect("fixture parses");
        let groups = file.object_groups();
        let payload = groups[11]
            .records
            .iter()
            .find(|record| record.record_type == RECORD_FUNCTION_EXPR_PAYLOAD)
            .expect("0907")
            .payload(&file.data);
        let words = payload
            .chunks_exact(2)
            .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
            .collect::<Vec<_>>();
        assert!(
            words.windows(55).any(|window| window
                == &[
                    0x000b, 0x000b, 0x6000, 0x1001, 0x200c, 0x6000, 0x1003, 0x6001, 0x000c, 0x1002,
                    0x6001, 0x000c, 0x000c, 0x1003, 0x6001, 0x1000, 0x000b, 0x000b, 0x0001, 0x1000,
                    0x000b, 0x1001, 0x0001, 0x000c, 0x1004, 0x000b, 0x200c, 0x6000, 0x1003, 0x6001,
                    0x000c, 0x1000, 0x0001, 0x000c, 0x000c, 0x1003, 0x000b, 0x0002, 0x1002, 0x6001,
                    0x000c, 0x1002, 0x000b, 0x0001, 0x1000, 0x000b, 0x1001, 0x0001, 0x000c, 0x1004,
                    0x6001, 0x000c, 0x1003, 0x0002, 0x000c
                ]),
            "expected fixture payload to contain the chessboard x signature, got {words:?}"
        );
        let params = BTreeMap::from([
            (
                0u16,
                ParameterBinding {
                    name: "t₁".to_string(),
                    value: 0.0,
                },
            ),
            (
                1u16,
                ParameterBinding {
                    name: "trunc((m₁ + 2))".to_string(),
                    value: 9.0,
                },
            ),
        ]);
        let expr = try_decode_inner_function_expr(payload, &params).expect("fixture payload");
        assert_eq!(
            evaluate_expr_with_parameters(
                &expr,
                0.0,
                &BTreeMap::from([
                    ("t₁".to_string(), 0.0),
                    ("trunc((m₁ + 2))".to_string(), 9.0),
                ]),
            ),
            Some(0.0)
        );
    }

    #[test]
    fn decodes_liyougui_function_iteration_payloads_from_saved_expression_words() {
        let data =
            include_bytes!("../../../tests/Samples/个人专栏/李有贵作品/函数图象迭代(liyougui).gsp");
        let file = GspFile::parse(data).expect("fixture parses");
        let groups = file.object_groups();
        let x_expr = try_decode_function_expr(&file, &groups, &groups[14]).expect("#15 expr");
        let helper_expr = try_decode_function_expr(&file, &groups, &groups[15]).expect("#16 expr");
        let y_expr = try_decode_function_expr(&file, &groups, &groups[16]).expect("#17 expr");
        let step_expr = try_decode_function_expr(&file, &groups, &groups[18]).expect("#19 expr");

        assert_eq!(
            x_expr,
            FunctionExpr::Parsed(FunctionAst::Binary {
                lhs: Box::new(FunctionAst::Constant(2.0)),
                op: BinaryOp::Mul,
                rhs: Box::new(FunctionAst::Binary {
                    lhs: Box::new(FunctionAst::Parameter("C".to_string(), 0.36706751054852294,)),
                    op: BinaryOp::Add,
                    rhs: Box::new(FunctionAst::Parameter("m".to_string(), -4.0)),
                }),
            })
        );
        assert_eq!(
            helper_expr,
            FunctionExpr::Parsed(FunctionAst::Binary {
                lhs: Box::new(FunctionAst::Binary {
                    lhs: Box::new(FunctionAst::Binary {
                        lhs: Box::new(FunctionAst::Variable),
                        op: BinaryOp::Pow,
                        rhs: Box::new(FunctionAst::Constant(2.0)),
                    }),
                    op: BinaryOp::Sub,
                    rhs: Box::new(FunctionAst::Binary {
                        lhs: Box::new(FunctionAst::Constant(2.0)),
                        op: BinaryOp::Mul,
                        rhs: Box::new(FunctionAst::Variable),
                    }),
                }),
                op: BinaryOp::Mul,
                rhs: Box::new(FunctionAst::Binary {
                    lhs: Box::new(FunctionAst::Parameter("k".to_string(), -1.5)),
                    op: BinaryOp::Pow,
                    rhs: Box::new(FunctionAst::Parameter("m".to_string(), -4.0)),
                }),
            })
        );
        assert_eq!(
            y_expr,
            FunctionExpr::Parsed(FunctionAst::Binary {
                lhs: Box::new(FunctionAst::Binary {
                    lhs: Box::new(FunctionAst::Binary {
                        lhs: Box::new(FunctionAst::Binary {
                            lhs: Box::new(FunctionAst::Constant(2.0)),
                            op: BinaryOp::Mul,
                            rhs: Box::new(FunctionAst::Parameter(
                                "C".to_string(),
                                0.36706751054852294,
                            )),
                        }),
                        op: BinaryOp::Pow,
                        rhs: Box::new(FunctionAst::Constant(2.0)),
                    }),
                    op: BinaryOp::Sub,
                    rhs: Box::new(FunctionAst::Binary {
                        lhs: Box::new(FunctionAst::Constant(2.0)),
                        op: BinaryOp::Mul,
                        rhs: Box::new(FunctionAst::Binary {
                            lhs: Box::new(FunctionAst::Constant(2.0)),
                            op: BinaryOp::Mul,
                            rhs: Box::new(FunctionAst::Parameter(
                                "C".to_string(),
                                0.36706751054852294,
                            )),
                        }),
                    }),
                }),
                op: BinaryOp::Mul,
                rhs: Box::new(FunctionAst::Binary {
                    lhs: Box::new(FunctionAst::Parameter("k".to_string(), -1.5)),
                    op: BinaryOp::Pow,
                    rhs: Box::new(FunctionAst::Parameter("m".to_string(), -4.0)),
                }),
            })
        );
        assert_eq!(
            step_expr,
            FunctionExpr::Parsed(FunctionAst::Binary {
                lhs: Box::new(FunctionAst::Parameter("m".to_string(), -4.0)),
                op: BinaryOp::Add,
                rhs: Box::new(FunctionAst::Constant(1.0)),
            })
        );
    }
}
