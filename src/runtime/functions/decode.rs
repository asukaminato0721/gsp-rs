use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};

use crate::format::{GspFile, ObjectGroup, PointRecord, read_f64, read_u16, read_u32};
use crate::runtime::DEFAULT_GRAPH_RAW_PER_UNIT;
use crate::runtime::extract::points::{
    collect_point_objects, resolve_circle_like_raw, resolve_line_like_points_raw,
};
use crate::runtime::extract::shapes::collect_raw_object_anchors;
use crate::runtime::extract::{
    decode_measurement_value, find_indexed_path, is_circle_group_kind,
    try_decode_parameter_control_value_for_group,
};
use crate::runtime::functions::{evaluate_expr_with_parameters, function_expr_label};
use crate::runtime::geometry::GraphTransform;
use crate::runtime::geometry::angle_degrees_from_points;
use crate::runtime::payload_consts::{
    EXPR_EULER_WORD, EXPR_OP_ADD, EXPR_OP_DIV, EXPR_OP_MUL, EXPR_OP_POW, EXPR_OP_SUB,
    EXPR_PARAMETER_MASK, EXPR_PARAMETER_PREFIX, EXPR_PI_WORD, EXPR_VARIABLE_WORD,
    FUNCTION_EXPR_MARKER_A, FUNCTION_EXPR_MARKER_B, RECORD_FUNCTION_EXPR_PAYLOAD,
    RECORD_INDEXED_PATH_B, RECORD_LABEL_AUX,
};
use crate::util::hex_bytes;
use thiserror::Error;

use super::expr::{
    BinaryOp, FunctionAst, FunctionExpr, FunctionPlotDescriptor, FunctionPlotMode, UnaryFunction,
    canonicalize_function_expr, decode_unary_function, function_expr_ast,
    function_expr_contains_variable,
};

mod parser;

pub(crate) use self::parser::try_decode_inner_function_expr;
use self::parser::{
    decode_embedded_postfix_payload_function_expr, decode_grouped_decimal_digit_literal,
    decode_trailing_scanned_payload_function_expr, embedded_calculate_expr_start,
    has_ignorable_expr_suffix, parse_function_expr_from, parse_function_expr_from_words,
    parse_grouped_function_expr_at, parse_grouped_function_expr_from_words,
    parse_grouped_parameter_control_expr_at, trailing_calculate_expr_start,
};

#[cfg(test)]
use self::parser::parse_function_expr;

thread_local! {
    static RESOLVING_MEASURED_VALUES: RefCell<BTreeSet<usize>> = RefCell::new(BTreeSet::new());
    static MEASURED_VALUE_ANCHORS_CACHE: RefCell<Option<Vec<Option<PointRecord>>>> = const { RefCell::new(None) };
    static RESOLVING_NUMERIC_HELPERS: RefCell<BTreeSet<usize>> = RefCell::new(BTreeSet::new());
    static NUMERIC_HELPER_CACHE: RefCell<BTreeMap<usize, Option<FunctionExpr>>> = RefCell::new(BTreeMap::new());
}

pub(crate) fn with_numeric_helper_cache<T>(f: impl FnOnce() -> T) -> T {
    NUMERIC_HELPER_CACHE.with(|cache| cache.borrow_mut().clear());
    MEASURED_VALUE_ANCHORS_CACHE.with(|cache| *cache.borrow_mut() = None);
    RESOLVING_NUMERIC_HELPERS.with(|resolving| resolving.borrow_mut().clear());
    RESOLVING_MEASURED_VALUES.with(|resolving| resolving.borrow_mut().clear());
    let result = f();
    NUMERIC_HELPER_CACHE.with(|cache| cache.borrow_mut().clear());
    MEASURED_VALUE_ANCHORS_CACHE.with(|cache| *cache.borrow_mut() = None);
    RESOLVING_NUMERIC_HELPERS.with(|resolving| resolving.borrow_mut().clear());
    RESOLVING_MEASURED_VALUES.with(|resolving| resolving.borrow_mut().clear());
    result
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
            | crate::format::GroupKind::VertexAngleValue
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

pub(crate) fn function_expr_uses_degree_units(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
) -> bool {
    fn visit(
        file: &GspFile,
        groups: &[ObjectGroup],
        group: &ObjectGroup,
        visiting: &mut BTreeSet<usize>,
    ) -> bool {
        if !visiting.insert(group.ordinal) {
            return false;
        }
        let has_degree_literal = group
            .records
            .iter()
            .find(|record| record.record_type == RECORD_FUNCTION_EXPR_PAYLOAD)
            .map(|record| record.payload(&file.data))
            .map(|payload| {
                let words = payload
                    .chunks_exact(2)
                    .map(|word| u16::from_le_bytes([word[0], word[1]]))
                    .collect::<Vec<_>>();
                embedded_calculate_expr_start(&words)
                    .is_some_and(|start| words[start..].contains(&0x0101))
            })
            .unwrap_or(false);
        let inherited_degree_unit = find_indexed_path(file, group).is_some_and(|path| {
            path.refs.iter().any(|ordinal| {
                groups
                    .get(ordinal.saturating_sub(1))
                    .filter(|parent| is_function_like_group(parent))
                    .is_some_and(|parent| visit(file, groups, parent, visiting))
            })
        });
        visiting.remove(&group.ordinal);
        has_degree_literal || inherited_degree_unit
    }

    visit(file, groups, group, &mut BTreeSet::new())
}

pub(crate) fn try_decode_parameter_control_expr(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
) -> Result<(FunctionExpr, bool), FunctionExprParseError> {
    let payload = group_function_payload(file, group)?;
    let mut visiting = BTreeSet::from([group.ordinal]);
    let parameters = collect_parameter_bindings(file, groups, group, &mut visiting, false, true);
    let absolute = with_function_payload_context(payload, || {
        let words = payload
            .chunks_exact(2)
            .map(|word| u16::from_le_bytes([word[0], word[1]]))
            .collect::<Vec<_>>();
        let start = embedded_calculate_expr_start(&words).ok_or(
            FunctionExprParseError::NoExpressionFound {
                word_len: words.len(),
            },
        )?;
        let ast = if words.get(start..start + 2) == Some(&[0x0000, 0x000a]) {
            parse_grouped_parameter_control_expr_at(&words, start, &parameters)?
        } else {
            parse_grouped_function_expr_at(&words, start, &parameters)?
        };
        Ok((canonicalize_function_expr(ast), true))
    });
    absolute.or_else(|_| try_decode_function_expr(file, groups, group).map(|expr| (expr, false)))
}

pub(crate) fn try_decode_function_expr_with_inlined_refs(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
) -> Result<FunctionExpr, FunctionExprParseError> {
    decode_function_expr_recursive_with_inlined_refs(file, groups, group, &mut BTreeSet::new())
}

pub(crate) fn try_decode_embedded_calculate_expr(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
) -> Result<FunctionExpr, FunctionExprParseError> {
    let payload = group_function_payload(file, group)?;
    let mut visiting = BTreeSet::from([group.ordinal]);
    let parameters = collect_parameter_bindings(file, groups, group, &mut visiting, true, false);
    decode_trailing_unit_calculate_expr(payload, &parameters)
        .or_else(|_| decode_embedded_postfix_payload_function_expr(payload, &parameters))
}

fn decode_trailing_unit_calculate_expr(
    payload: &[u8],
    parameters: &BTreeMap<u16, ParameterBinding>,
) -> Result<FunctionExpr, FunctionExprParseError> {
    let words = payload
        .chunks_exact(2)
        .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
        .collect::<Vec<_>>();
    let len = words.len();
    if len < 6
        || !matches!(words[len - 3], 0x0101 | 0x0201)
        || !matches!(
            words[len - 2],
            EXPR_OP_ADD | EXPR_OP_SUB | EXPR_OP_MUL | EXPR_OP_DIV | EXPR_OP_POW
        )
        || (words[len - 1] & EXPR_PARAMETER_MASK) != EXPR_PARAMETER_PREFIX
    {
        return Err(FunctionExprParseError::NoExpressionFound { word_len: len });
    }
    let lhs_start = len - 6;
    let (lhs, lhs_end) = parse_function_expr_from(&words[..len - 2], lhs_start, parameters)?;
    if lhs_end != len - 2 {
        return Err(FunctionExprParseError::NoExpressionFound { word_len: len });
    }
    let (rhs, rhs_end) = parse_function_expr_from(&words, len - 1, parameters)?;
    if rhs_end != len {
        return Err(FunctionExprParseError::NoExpressionFound { word_len: len });
    }
    let op = match words[len - 2] {
        EXPR_OP_ADD => BinaryOp::Add,
        EXPR_OP_SUB => BinaryOp::Sub,
        EXPR_OP_MUL => BinaryOp::Mul,
        EXPR_OP_DIV => BinaryOp::Div,
        EXPR_OP_POW => BinaryOp::Pow,
        _ => unreachable!(),
    };
    Ok(canonicalize_function_expr(FunctionAst::Binary {
        lhs: Box::new(lhs),
        op,
        rhs: Box::new(rhs),
    }))
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
        collect_parameter_bindings(file, groups, group, visiting, inline_function_refs, true);

    let function_ref_expr = try_decode_single_payload_function_application(
        file,
        groups,
        group,
        visiting,
        payload,
        &parameters,
    )
    .or_else(|| {
        try_decode_payload_function_refs(file, groups, group, visiting, payload, &parameters)
    });
    if let Some(expr) = function_ref_expr {
        return Ok(expr);
    }
    if payload_uses_grouped_expression(payload) {
        return decode_grouped_decimal_payload_function_expr(payload, &parameters);
    }

    let decoded = match mode {
        PayloadExprDecodeMode::Standard => decode_payload_function_expr(payload, &parameters),
        PayloadExprDecodeMode::EmbeddedPostfixPreferred => {
            decode_embedded_postfix_payload_function_expr(payload, &parameters)
                .or_else(|_| decode_payload_function_expr(payload, &parameters))
        }
        PayloadExprDecodeMode::GroupedPreferred => {
            decode_grouped_preferred_payload_function_expr(payload, &parameters)
        }
    };
    decoded
        .or_else(|error| {
            if function_expr_is_rotation_calculation(file, groups, group.ordinal)
                || function_definition_is_plotted(file, groups, group)
                || function_expression_is_definition_parameter(file, groups, group.ordinal)
            {
                decode_trailing_scanned_payload_function_expr(payload, &parameters)
            } else {
                Err(error)
            }
        })
        .or_else(|error| {
            if group.header.kind() == crate::format::GroupKind::FunctionExpr {
                decode_inline_parameter_token(payload, &parameters).ok_or(error)
            } else {
                Err(error)
            }
        })
}

fn function_expression_is_definition_parameter(
    file: &GspFile,
    groups: &[ObjectGroup],
    expression_ordinal: usize,
) -> bool {
    fn depends_on(
        file: &GspFile,
        groups: &[ObjectGroup],
        group: &ObjectGroup,
        target_ordinal: usize,
        visiting: &mut BTreeSet<usize>,
    ) -> bool {
        if !visiting.insert(group.ordinal) {
            return false;
        }
        let found = find_indexed_path(file, group).is_some_and(|path| {
            path.refs.iter().any(|ordinal| {
                *ordinal == target_ordinal
                    || groups
                        .get(ordinal.saturating_sub(1))
                        .filter(|parent| {
                            parent.header.kind() == crate::format::GroupKind::FunctionDefinition
                                || is_function_like_group(parent)
                        })
                        .is_some_and(|parent| {
                            depends_on(file, groups, parent, target_ordinal, visiting)
                        })
            })
        });
        visiting.remove(&group.ordinal);
        found
    }

    groups.iter().any(|candidate| {
        candidate.header.kind() == crate::format::GroupKind::FunctionDefinition
            && depends_on(
                file,
                groups,
                candidate,
                expression_ordinal,
                &mut BTreeSet::new(),
            )
    })
}

fn function_definition_is_plotted(
    file: &GspFile,
    groups: &[ObjectGroup],
    function_group: &ObjectGroup,
) -> bool {
    function_group.header.kind() == crate::format::GroupKind::FunctionDefinition
        && groups.iter().any(|candidate| {
            candidate.header.kind() == crate::format::GroupKind::FunctionPlot
                && find_indexed_path(file, candidate).and_then(|path| path.refs.first().copied())
                    == Some(function_group.ordinal)
        })
}

fn function_expr_is_rotation_calculation(
    file: &GspFile,
    groups: &[ObjectGroup],
    expression_ordinal: usize,
) -> bool {
    groups.iter().any(|candidate| {
        matches!(
            candidate.header.kind(),
            crate::format::GroupKind::ParameterRotation
                | crate::format::GroupKind::ExpressionRotation
        ) && find_indexed_path(file, candidate).and_then(|path| path.refs.get(2).copied())
            == Some(expression_ordinal)
    })
}

fn payload_uses_grouped_expression(payload: &[u8]) -> bool {
    let words = payload
        .chunks_exact(2)
        .map(|word| u16::from_le_bytes([word[0], word[1]]))
        .collect::<Vec<_>>();
    let Some(start) = embedded_calculate_expr_start(&words) else {
        return false;
    };
    let expression = &words[start..];
    expression.contains(&0x000b)
}

fn decode_grouped_decimal_payload_function_expr(
    payload: &[u8],
    parameters: &BTreeMap<u16, ParameterBinding>,
) -> Result<FunctionExpr, FunctionExprParseError> {
    with_function_payload_context(payload, || {
        let words = payload
            .chunks_exact(2)
            .map(|word| u16::from_le_bytes([word[0], word[1]]))
            .collect::<Vec<_>>();
        let start = embedded_calculate_expr_start(&words).ok_or(
            FunctionExprParseError::NoExpressionFound {
                word_len: words.len(),
            },
        )?;
        let ast = parse_grouped_parameter_control_expr_at(&words, start, parameters)?;
        Ok(canonicalize_function_expr(ast))
    })
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
    if let Some(cached) =
        NUMERIC_HELPER_CACHE.with(|cache| cache.borrow().get(&group.ordinal).cloned())
    {
        return cached;
    }
    RESOLVING_NUMERIC_HELPERS.with(|resolving| {
        if !resolving.borrow_mut().insert(group.ordinal) {
            return None;
        }
        let result = try_decode_numeric_helper_group_inner(file, groups, group);
        resolving.borrow_mut().remove(&group.ordinal);
        NUMERIC_HELPER_CACHE.with(|cache| {
            cache.borrow_mut().insert(group.ordinal, result.clone());
        });
        result
    })
}

fn try_decode_numeric_helper_group_inner(
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
            | crate::format::GroupKind::VertexAngleValue
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
        crate::format::GroupKind::AngleValue | crate::format::GroupKind::VertexAngleValue => {
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
            if path.refs.len() == 3 && is_circle_group_kind(source_group.header.kind()) {
                let circle = resolve_circle_like_raw(file, groups, &anchors, source_group)?;
                let start = anchors.get(path.refs[1].checked_sub(1)?)?.as_ref()?;
                let end = anchors.get(path.refs[2].checked_sub(1)?)?.as_ref()?;
                let center = circle.center();
                let degrees = angle_degrees_from_points(start, &center, end)?.abs();
                normalize_graph_distance(
                    circle.radius() * degrees.to_radians(),
                    graph.as_ref().map(|(_, raw_per_unit)| *raw_per_unit),
                )
            } else {
                boundary_curve_length_raw(
                    file,
                    groups,
                    &anchors,
                    source_group,
                    graph.as_ref().map(|(_, raw_per_unit)| *raw_per_unit),
                )?
            }
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
                try_decode_numeric_helper_group(file, groups, source_group)
                    .or_else(|| try_decode_function_expr(file, groups, source_group).ok())
                    .and_then(|expr| match expr {
                        FunctionExpr::Constant(value) => Some(value),
                        _ => evaluate_expr_with_parameters(&expr, 0.0, &BTreeMap::new()),
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
                .find(|record| {
                    record.record_type == crate::runtime::payload_consts::RECORD_BINDING_PAYLOAD
                        && record.length == 12
                })
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
            let record = group.records.iter().find(|record| {
                record.record_type == crate::runtime::payload_consts::RECORD_BINDING_PAYLOAD
                    && record.length == 12
            })?;
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

fn decode_inline_parameter_token(
    payload: &[u8],
    parameters: &BTreeMap<u16, ParameterBinding>,
) -> Option<FunctionExpr> {
    let parameter_index = extract_inline_function_token(payload)?
        .parse::<u16>()
        .ok()?;
    let binding = parameters.get(&parameter_index)?;
    Some(canonicalize_function_expr(
        binding
            .expr
            .clone()
            .unwrap_or_else(|| FunctionAst::Parameter(binding.name.clone(), binding.value)),
    ))
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
    let primary = embedded_calculate_expr_start(&words);
    let trailing = trailing_calculate_expr_start(&words);
    if let Some(start) = primary
        && let Ok((parsed, end)) = parse_function_expr_from(&words, start, parameters)
        && has_ignorable_expr_suffix(&words, end)
    {
        return Ok(canonicalize_function_expr(parsed));
    }
    if let Some(start) = trailing
        && trailing_expr_has_explicit_unit_or_unary_prefix(&words, start)
    {
        if let Ok((parsed, end)) = parse_function_expr_from(&words, start, parameters)
            && has_ignorable_expr_suffix(&words, end)
        {
            return Ok(canonicalize_function_expr(parsed));
        }
        if let Some(expr) = decode_trailing_degree_literal_expr(&words, start, parameters) {
            return Ok(expr);
        }
        if let Some(expr) = decode_trailing_negated_expr(&words, start, parameters) {
            return Ok(expr);
        }
    }
    Err(FunctionExprParseError::NoExpressionFound {
        word_len: words.len(),
    })
}

fn decode_trailing_negated_expr(
    words: &[u16],
    start: usize,
    parameters: &BTreeMap<u16, ParameterBinding>,
) -> Option<FunctionExpr> {
    if words.get(start).copied() != Some(EXPR_OP_SUB) {
        return None;
    }
    let (expr, end) = parse_function_expr_from(words, start + 1, parameters).ok()?;
    if !has_ignorable_expr_suffix(words, end) {
        return None;
    }
    Some(canonicalize_function_expr(FunctionAst::Binary {
        lhs: Box::new(FunctionAst::Constant(0.0)),
        op: BinaryOp::Sub,
        rhs: Box::new(expr),
    }))
}

fn trailing_expr_has_explicit_unit_or_unary_prefix(words: &[u16], start: usize) -> bool {
    words.get(start).copied() == Some(EXPR_OP_SUB)
        || matches!(
            words.get(start..start + 3),
            Some([tens, ones, 0x0101]) if *tens <= 9 && *ones <= 9
        )
}

fn decode_trailing_degree_literal_expr(
    words: &[u16],
    start: usize,
    parameters: &BTreeMap<u16, ParameterBinding>,
) -> Option<FunctionExpr> {
    let [tens, ones, 0x0101, op, ..] = words.get(start..)? else {
        return None;
    };
    if *tens > 9 || *ones > 9 {
        return None;
    }
    let op = match *op {
        EXPR_OP_ADD => BinaryOp::Add,
        EXPR_OP_SUB => BinaryOp::Sub,
        EXPR_OP_MUL => BinaryOp::Mul,
        EXPR_OP_DIV => BinaryOp::Div,
        EXPR_OP_POW => BinaryOp::Pow,
        _ => return None,
    };
    let (rhs, end) = parse_function_expr_from(words, start + 4, parameters).ok()?;
    if !has_ignorable_expr_suffix(words, end) {
        return None;
    }
    Some(canonicalize_function_expr(FunctionAst::Binary {
        lhs: Box::new(FunctionAst::Constant(
            f64::from(tens * 10 + ones).to_radians(),
        )),
        op,
        rhs: Box::new(rhs),
    }))
}

fn substitute_variable(ast: FunctionAst, replacement: &FunctionAst) -> FunctionAst {
    match ast {
        FunctionAst::Variable => replacement.clone(),
        FunctionAst::Constant(_)
        | FunctionAst::PiConstant
        | FunctionAst::EulerConstant
        | FunctionAst::PiAngle
        | FunctionAst::Parameter(_, _) => ast,
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

struct FunctionRefParser<'a> {
    words: &'a [u16],
    parameters: &'a BTreeMap<u16, ParameterBinding>,
    helpers: BTreeMap<u16, FunctionAst>,
    offset: usize,
}

impl<'a> FunctionRefParser<'a> {
    fn new(
        words: &'a [u16],
        parameters: &'a BTreeMap<u16, ParameterBinding>,
        helpers: BTreeMap<u16, FunctionAst>,
    ) -> Self {
        Self {
            words,
            parameters,
            helpers,
            offset: 0,
        }
    }

    fn parse_expr(&mut self, min_bp: u8) -> Result<FunctionAst, FunctionExprParseError> {
        let mut lhs = self.parse_prefix()?;
        while let Some((op, left_bp, right_bp)) = self.peek_infix() {
            if left_bp < min_bp {
                break;
            }
            self.offset += 1;
            let rhs = self.parse_expr(right_bp)?;
            lhs = FunctionAst::Binary {
                lhs: Box::new(lhs),
                op,
                rhs: Box::new(rhs),
            };
        }
        Ok(lhs)
    }

    fn parse_prefix(&mut self) -> Result<FunctionAst, FunctionExprParseError> {
        let offset = self.offset;
        if let Some((value, width_words)) =
            decode_grouped_decimal_digit_literal(&self.words[self.offset..])
        {
            self.offset += width_words;
            return Ok(FunctionAst::Constant(value));
        }
        let word = *self
            .words
            .get(self.offset)
            .ok_or(FunctionExprParseError::UnexpectedEnd { offset })?;
        self.offset += 1;
        match word {
            EXPR_VARIABLE_WORD => Ok(FunctionAst::Variable),
            EXPR_PI_WORD => Ok(FunctionAst::PiConstant),
            EXPR_EULER_WORD => Ok(FunctionAst::EulerConstant),
            EXPR_OP_ADD => self.parse_prefix(),
            EXPR_OP_SUB => {
                let expr = self.parse_prefix()?;
                Ok(FunctionAst::Binary {
                    lhs: Box::new(FunctionAst::Constant(0.0)),
                    op: BinaryOp::Sub,
                    rhs: Box::new(expr),
                })
            }
            0x000b => {
                let expr = self.parse_expr(0)?;
                match self.words.get(self.offset).copied() {
                    Some(0x000c) => {
                        self.offset += 1;
                        Ok(expr)
                    }
                    Some(found) => Err(FunctionExprParseError::UnexpectedToken {
                        offset: self.offset,
                        found: function_ref_token_for_word(found, self.parameters)
                            .unwrap_or(FunctionToken::Constant(f64::from(found))),
                    }),
                    None => Err(FunctionExprParseError::UnexpectedEnd {
                        offset: self.offset,
                    }),
                }
            }
            found @ (EXPR_OP_MUL | EXPR_OP_DIV | EXPR_OP_POW | 0x000c) => {
                Err(FunctionExprParseError::UnexpectedToken {
                    offset,
                    found: function_ref_token_for_word(found, self.parameters)
                        .unwrap_or(FunctionToken::Constant(f64::from(found))),
                })
            }
            _ if (word & EXPR_FUNCTION_REF_MASK) == EXPR_FUNCTION_REF_PREFIX => {
                let helper_index = word & 0x000f;
                let helper_ast = self.helpers.get(&helper_index).cloned().ok_or(
                    FunctionExprParseError::MissingParameterBinding {
                        offset,
                        parameter_index: helper_index,
                    },
                )?;
                let argument = self.parse_expr(0)?;
                if self.words.get(self.offset).copied() == Some(0x000c) {
                    self.offset += 1;
                }
                Ok(substitute_variable(helper_ast, &argument))
            }
            _ if (word & EXPR_PARAMETER_MASK) == EXPR_PARAMETER_PREFIX => {
                let parameter_index = word & 0x000f;
                let binding = self.parameters.get(&parameter_index).cloned().ok_or(
                    FunctionExprParseError::MissingParameterBinding {
                        offset,
                        parameter_index,
                    },
                )?;
                Ok(binding
                    .expr
                    .unwrap_or(FunctionAst::Parameter(binding.name, binding.value)))
            }
            _ if let Some(op) = decode_unary_function(word) => {
                let expr = self.parse_expr(0)?;
                if self.words.get(self.offset).copied() == Some(0x000c) {
                    self.offset += 1;
                }
                Ok(FunctionAst::Unary {
                    op,
                    expr: Box::new(expr),
                })
            }
            _ if word < EXPR_OP_ADD => Ok(FunctionAst::Constant(f64::from(word))),
            _ => Err(FunctionExprParseError::UnknownOpcode {
                offset,
                opcode: word,
            }),
        }
    }

    fn peek_infix(&self) -> Option<(BinaryOp, u8, u8)> {
        match self.words.get(self.offset).copied()? {
            EXPR_OP_ADD => Some((BinaryOp::Add, 1, 2)),
            EXPR_OP_SUB => Some((BinaryOp::Sub, 1, 2)),
            EXPR_OP_MUL => Some((BinaryOp::Mul, 3, 4)),
            EXPR_OP_DIV => Some((BinaryOp::Div, 3, 4)),
            EXPR_OP_POW => Some((BinaryOp::Pow, 5, 5)),
            _ => None,
        }
    }

    fn remaining_is_ignorable(&self) -> bool {
        self.words[self.offset..].iter().all(|word| *word == 0x000c)
    }
}

fn function_ref_token_for_word(
    word: u16,
    parameters: &BTreeMap<u16, ParameterBinding>,
) -> Option<FunctionToken> {
    Some(match word {
        EXPR_OP_ADD => FunctionToken::Add,
        EXPR_OP_SUB => FunctionToken::Sub,
        EXPR_OP_MUL => FunctionToken::Mul,
        EXPR_OP_DIV => FunctionToken::Div,
        EXPR_OP_POW => FunctionToken::Pow,
        0x000c => FunctionToken::Terminator,
        EXPR_VARIABLE_WORD => FunctionToken::Variable,
        EXPR_PI_WORD => FunctionToken::PiConstant,
        EXPR_EULER_WORD => FunctionToken::EulerConstant,
        _ if (word & EXPR_PARAMETER_MASK) == EXPR_PARAMETER_PREFIX => {
            let parameter_index = word & 0x000f;
            FunctionToken::Parameter(parameters.get(&parameter_index)?.clone())
        }
        _ if decode_unary_function(word).is_some() => {
            FunctionToken::Unary(decode_unary_function(word).unwrap())
        }
        _ => return None,
    })
}

fn try_decode_single_payload_function_application(
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
    let expression_start = embedded_calculate_expr_start(&words).or_else(|| {
        words
            .windows(2)
            .position(|pair| *pair == FUNCTION_EXPR_MARKER_A || *pair == FUNCTION_EXPR_MARKER_B)
            .map(|marker_index| marker_index + 2)
    })?;
    let (application_offset, application_word) = words
        .iter()
        .copied()
        .enumerate()
        .skip(expression_start)
        .find(|(_, word)| (*word & EXPR_FUNCTION_REF_MASK) == EXPR_FUNCTION_REF_PREFIX)?;
    if application_offset != expression_start {
        return None;
    }
    let helper_index = usize::from(application_word & 0x000f);
    let path = find_indexed_path(file, group)?;
    let helper_group = groups.get(path.refs.get(helper_index)?.checked_sub(1)?)?;
    let helper_expr = decode_function_expr_recursive(file, groups, helper_group, visiting)
        .or_else(|_| decode_embedded_grouped_function_expr(file, groups, helper_group, visiting))
        .ok()?;
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

fn try_decode_payload_function_refs(
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
    let expression_start = embedded_calculate_expr_start(&words)
        .or_else(|| {
            words
                .windows(2)
                .position(|pair| *pair == FUNCTION_EXPR_MARKER_A || *pair == FUNCTION_EXPR_MARKER_B)
                .map(|marker_index| marker_index + 2)
        })
        .unwrap_or(0);
    let helper_indices = words
        .iter()
        .copied()
        .enumerate()
        .skip(expression_start)
        .filter_map(|(_, word)| {
            ((word & EXPR_FUNCTION_REF_MASK) == EXPR_FUNCTION_REF_PREFIX).then_some(word & 0x000f)
        })
        .collect::<BTreeSet<_>>();
    if helper_indices.is_empty() {
        return None;
    }
    let path = find_indexed_path(file, group)?;
    let mut helpers = BTreeMap::new();
    for helper_index in helper_indices {
        let helper_group = groups.get(path.refs.get(usize::from(helper_index))?.checked_sub(1)?)?;
        let helper_expr = decode_function_expr_recursive(file, groups, helper_group, visiting)
            .or_else(|_| {
                decode_embedded_grouped_function_expr(file, groups, helper_group, visiting)
            })
            .ok()?;
        helpers.insert(helper_index, function_expr_ast(helper_expr));
    }
    let mut parser = FunctionRefParser::new(&words[expression_start..], parameters, helpers);
    let parsed = parser.parse_expr(0).ok()?;
    parser
        .remaining_is_ignorable()
        .then(|| canonicalize_function_expr(parsed))
}

fn decode_embedded_grouped_function_expr(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
    visiting: &mut BTreeSet<usize>,
) -> Result<FunctionExpr, FunctionExprParseError> {
    if group.header.kind() != crate::format::GroupKind::FunctionDefinition {
        return Err(FunctionExprParseError::NoExpressionFound { word_len: 0 });
    }
    let payload = group_function_payload(file, group)?;
    let parameters = collect_parameter_bindings(file, groups, group, visiting, true, false);
    with_function_payload_context(payload, || {
        let words = payload
            .chunks_exact(2)
            .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
            .collect::<Vec<_>>();
        let start = embedded_calculate_expr_start(&words).ok_or(
            FunctionExprParseError::NoExpressionFound {
                word_len: words.len(),
            },
        )?;
        let mut parser = FunctionRefParser::new(&words[start..], &parameters, BTreeMap::new());
        let parsed = parser.parse_expr(0)?;
        if parser.remaining_is_ignorable() {
            Ok(canonicalize_function_expr(parsed))
        } else {
            Err(FunctionExprParseError::NoExpressionFound {
                word_len: words.len(),
            })
        }
    })
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
    allow_constraint_anchor_bindings: bool,
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
            allow_constraint_anchor_bindings,
        ) {
            bindings.insert(index as u16, binding);
        }
    }
    bindings
}

pub(crate) fn function_parameter_group_ordinals(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
) -> BTreeMap<String, usize> {
    let Some(path) = find_indexed_path(file, group) else {
        return BTreeMap::new();
    };
    let mut visiting = BTreeSet::from([group.ordinal]);
    let bindings = collect_parameter_bindings(file, groups, group, &mut visiting, false, false);
    bindings
        .into_iter()
        .filter_map(|(index, binding)| {
            path.refs
                .get(usize::from(index))
                .copied()
                .map(|ordinal| (binding.name, ordinal))
        })
        .collect()
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
        if let Ok(expr) = decode_function_expr_recursive(file, groups, group, visiting) {
            let name = group_name(file, groups, group)
                .unwrap_or_else(|| function_expr_label(expr.clone()));
            let value = evaluate_expr_with_parameters(&expr, 0.0, &BTreeMap::new());
            if !inline_function_refs
                || function_expr_contains_variable(&expr)
                || group.header.kind() != crate::format::GroupKind::FunctionDefinition
            {
                return value.map(|value| ParameterBinding::value(name, value));
            }
            return Some(ParameterBinding::expression(
                name,
                value.unwrap_or(0.0),
                function_expr_ast(expr),
            ));
        }
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
        crate::format::GroupKind::Point => point_group
            .records
            .iter()
            .find(|record| record.record_type == RECORD_FUNCTION_EXPR_PAYLOAD)
            .map(|record| record.payload(&file.data))
            .and_then(|payload| {
                if payload.len() >= 60 {
                    Some(read_f64(payload, 52))
                } else {
                    Some(f64::from(read_u16(payload, payload.len().checked_sub(2)?)))
                }
            })
            .filter(|value| value.is_finite())
            .or_else(|| decode_parameter_anchor_host_value(file, &groups, &path))?,
        _ => decode_parameter_anchor_host_value(file, &groups, &path)?,
    };
    Some(ParameterBinding::value(name, value))
}

fn decode_parameter_anchor_host_value(
    file: &GspFile,
    groups: &[ObjectGroup],
    path: &crate::format::IndexedPathRecord,
) -> Option<f64> {
    let point_ordinal = *path.refs.first()?;
    let host_ordinal = *path.refs.get(1)?;
    let max_ordinal = point_ordinal.max(host_ordinal);
    let helper_groups = groups.get(..max_ordinal)?;
    let point_map = collect_point_objects(file, groups);
    let helper_point_map = point_map.get(..max_ordinal)?;
    let anchors = collect_raw_object_anchors(file, helper_groups, helper_point_map, None);
    let point = anchors.get(point_ordinal.checked_sub(1)?)?.as_ref()?;
    let host = groups.get(host_ordinal.checked_sub(1)?)?;

    if host.header.kind() == crate::format::GroupKind::Polygon {
        let host_path = find_indexed_path(file, host)?;
        let vertex_index = host_path
            .refs
            .iter()
            .position(|ordinal| *ordinal == point_ordinal)?;
        let vertex_group_indices = host_path
            .refs
            .iter()
            .map(|ordinal| ordinal.checked_sub(1))
            .collect::<Option<Vec<_>>>()?;
        return polygon_boundary_parameter_for_anchor(
            &anchors,
            &vertex_group_indices,
            vertex_index,
            0.0,
        );
    }

    if let Some(circle) = resolve_circle_like_raw(file, groups, &anchors, host) {
        let center = circle.center();
        let angle = (-(point.y - center.y)).atan2(point.x - center.x);
        return Some(angle.rem_euclid(std::f64::consts::TAU) / std::f64::consts::TAU);
    }
    let (start, end) = resolve_line_like_points_raw(file, groups, &anchors, host)?;
    let dx = end.x - start.x;
    let dy = end.y - start.y;
    let length_squared = dx * dx + dy * dy;
    (length_squared > 1e-18)
        .then_some(((point.x - start.x) * dx + (point.y - start.y) * dy) / length_squared)
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

    let anchors = RESOLVING_MEASURED_VALUES.with(|resolving| {
        if !resolving.borrow_mut().insert(group.ordinal) {
            return None;
        }
        let anchors = collect_measured_value_anchors(file, groups);
        resolving.borrow_mut().remove(&group.ordinal);
        Some(anchors)
    })?;
    let start = anchors.get(host_path.refs[0].checked_sub(1)?)?.clone()?;
    let end = anchors.get(host_path.refs[1].checked_sub(1)?)?.clone()?;
    let value =
        ((end.x - start.x).powi(2) + (end.y - start.y).powi(2)).sqrt() / DEFAULT_GRAPH_RAW_PER_UNIT;
    if !value.is_finite() {
        return None;
    }

    let name = group_name(file, groups, group)
        .or_else(|| segment_name(file, groups, host_group))
        .or_else(|| {
            let left = groups
                .get(host_path.refs[0].checked_sub(1)?)
                .and_then(|group| group_name(file, groups, group))
                .unwrap_or_else(|| "P".to_string());
            let right = groups
                .get(host_path.refs[1].checked_sub(1)?)
                .and_then(|group| group_name(file, groups, group))
                .unwrap_or_else(|| "Q".to_string());
            Some(format!("{left}{right}"))
        })?;
    Some(ParameterBinding::value(name, value))
}

fn collect_measured_value_anchors(
    file: &GspFile,
    groups: &[ObjectGroup],
) -> Vec<Option<PointRecord>> {
    if let Some(anchors) = MEASURED_VALUE_ANCHORS_CACHE.with(|cache| cache.borrow().clone()) {
        return anchors;
    }
    let point_map = collect_point_objects(file, groups);
    let raw_anchors = collect_raw_object_anchors(file, groups, &point_map, None);
    let anchors = if let Some(graph) = detect_function_graph_transform(file, groups, &raw_anchors) {
        collect_raw_object_anchors(file, groups, &point_map, Some(&graph))
    } else {
        raw_anchors
    };
    MEASURED_VALUE_ANCHORS_CACHE.with(|cache| *cache.borrow_mut() = Some(anchors.clone()));
    anchors
}

fn detect_function_graph_transform(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
) -> Option<GraphTransform> {
    let raw_per_unit = groups
        .iter()
        .filter(|group| group.header.kind().is_graph_calibration())
        .find_map(|group| {
            let record = group.records.iter().find(|record| {
                record.record_type == RECORD_INDEXED_PATH_B && record.length == 12
            })?;
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
    Some(GraphTransform {
        origin_raw,
        raw_per_unit,
    })
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
    if group.header.kind() == crate::format::GroupKind::BoundaryCurveLengthValue {
        return boundary_curve_length_group_name(file, groups, group);
    }
    if !matches!(
        group.header.kind(),
        crate::format::GroupKind::DistanceValue | crate::format::GroupKind::GraphDistanceValue
    ) {
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

fn boundary_curve_length_group_name(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
) -> Option<String> {
    let path = find_indexed_path(file, group)?;
    let host_group = groups.get(path.refs.first()?.checked_sub(1)?)?;
    if path.refs.len() == 3 && is_circle_group_kind(host_group.header.kind()) {
        let start = groups
            .get(path.refs[1].checked_sub(1)?)
            .and_then(|group| group_name(file, groups, group))?;
        let end = groups
            .get(path.refs[2].checked_sub(1)?)
            .and_then(|group| group_name(file, groups, group))?;
        return Some(format!("{start}{end}"));
    }
    let host_path = find_indexed_path(file, host_group)?;
    let (start_ordinal, end_ordinal) = match host_group.header.kind() {
        crate::format::GroupKind::CenterArc => (*host_path.refs.get(1)?, *host_path.refs.get(2)?),
        crate::format::GroupKind::ArcOnCircle => (*host_path.refs.get(1)?, *host_path.refs.get(2)?),
        crate::format::GroupKind::SectorBoundary
        | crate::format::GroupKind::CircularSegmentBoundary => {
            let arc_group = groups.get(host_path.refs.first()?.checked_sub(1)?)?;
            let arc_path = find_indexed_path(file, arc_group)?;
            match arc_group.header.kind() {
                crate::format::GroupKind::CenterArc | crate::format::GroupKind::ArcOnCircle => {
                    (*arc_path.refs.get(1)?, *arc_path.refs.get(2)?)
                }
                _ => return None,
            }
        }
        _ => return None,
    };
    let start = groups
        .get(start_ordinal.checked_sub(1)?)
        .and_then(|group| group_name(file, groups, group))?;
    let end = groups
        .get(end_ordinal.checked_sub(1)?)
        .and_then(|group| group_name(file, groups, group))?;
    Some(format!("{start}{end}"))
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
    PiConstant,
    EulerConstant,
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
    #[error(
        "postfix opcode 0x{opcode:04x} at function payload word offset {offset} requires {expected} operands, found {found}"
    )]
    InvalidPostfixArity {
        offset: usize,
        opcode: u16,
        expected: usize,
        found: usize,
    },
    #[error("postfix function expression left {remaining} uncombined operands")]
    TrailingPostfixOperands { remaining: usize },
    #[error("unknown function opcode 0x{opcode:04x} at function payload word offset {offset}")]
    UnknownOpcode { offset: usize, opcode: u16 },
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

#[cfg(test)]
mod parse_tests {
    use crate::runtime::payload_consts::{EXPR_OP_ADD, EXPR_OP_SUB, EXPR_VARIABLE_WORD};

    use super::parser::parse_grouped_parameter_control_expr_at;
    use super::{
        FunctionExprParseError, ParameterBinding, decode_embedded_postfix_payload_function_expr,
        decode_inline_parameter_token, decode_payload_function_expr,
        function_definition_is_plotted, parse_function_expr, try_decode_function_expr,
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
    fn inline_numeric_token_selects_the_exact_payload_parameter() {
        let parameters =
            BTreeMap::from([(0, ParameterBinding::value("distance".to_string(), 12.5))]);
        assert_eq!(
            decode_inline_parameter_token(b"prefix<0>suffix", &parameters),
            Some(FunctionExpr::Parsed(FunctionAst::Parameter(
                "distance".to_string(),
                12.5,
            )))
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
    fn rejects_postfix_binary_operators_with_missing_operands() {
        for opcode in [EXPR_OP_ADD, EXPR_OP_SUB] {
            let payload = payload_from_words(&[0x0112, 0, EXPR_VARIABLE_WORD, opcode]);
            assert_eq!(
                decode_embedded_postfix_payload_function_expr(&payload, &BTreeMap::new()),
                Err(FunctionExprParseError::InvalidPostfixArity {
                    offset: 3,
                    opcode,
                    expected: 2,
                    found: 1,
                })
            );
        }
    }

    #[test]
    fn rejects_uncombined_postfix_operands_instead_of_multiplying_them() {
        let payload = payload_from_words(&[0x0112, 0, EXPR_VARIABLE_WORD, 2]);
        let error = decode_embedded_postfix_payload_function_expr(&payload, &BTreeMap::new())
            .expect_err("two operands without an opcode must be rejected");
        assert!(matches!(
            error,
            FunctionExprParseError::TrailingPostfixOperands { remaining: 2 }
        ));
    }

    #[test]
    fn rejects_unknown_postfix_opcodes_instead_of_treating_them_as_constants() {
        let payload = payload_from_words(&[0x0112, 0, EXPR_VARIABLE_WORD, 0x3000]);
        let error = decode_embedded_postfix_payload_function_expr(&payload, &BTreeMap::new())
            .expect_err("unknown opcode must be rejected");
        assert!(matches!(
            error,
            FunctionExprParseError::UnknownOpcode {
                offset: 3,
                opcode: 0x3000
            }
        ));
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
    fn decodes_parameter_control_grouped_expression_with_leading_zero_decimal() {
        let words = [
            0x0000, 0x000a, 0x0005, 0x1002, 0x000b, 0x000b, 0x200a, 0x0000, 0x000a, 0x0005, 0x1001,
            0x6000, 0x000c, 0x1000, 0x0001, 0x000c, 0x1002, 0x6001, 0x1000, 0x000b, 0x200a, 0x6000,
            0x1001, 0x0000, 0x000a, 0x0005, 0x000c, 0x1000, 0x0001, 0x000c, 0x1002, 0x6002, 0x000c,
        ];
        let parameters = BTreeMap::from([
            (0, ParameterBinding::value("m₁".to_string(), 0.25)),
            (1, ParameterBinding::value("m₂".to_string(), 0.4)),
            (2, ParameterBinding::value("m₃".to_string(), 0.8)),
        ]);
        let expr = parse_grouped_parameter_control_expr_at(&words, 0, &parameters)
            .expect("payload grouped expression");
        let expr = FunctionExpr::Parsed(expr);
        assert_eq!(
            evaluate_expr_with_parameters(&expr, 0.0, &BTreeMap::new()),
            Some(0.4)
        );
    }

    #[test]
    fn decodes_grouped_constants_with_native_0100_suffix() {
        let words = [
            0x000b, 0x6000, 0x1002, 0x0002, 0x1003, 0x6001, 0x1002, 0x0001, 0x0100, 0x1000, 0x0001,
            0x0004, 0x0004, 0x0101, 0x000c, 0x1002, 0x6002,
        ];
        let parameters = BTreeMap::from([
            (0, ParameterBinding::value("r".to_string(), 2.0)),
            (1, ParameterBinding::value("R".to_string(), 4.0)),
            (2, ParameterBinding::value("M".to_string(), 3.0)),
        ]);
        let expr = parse_grouped_parameter_control_expr_at(&words, 0, &parameters)
            .expect("native numeric suffix is structural, not an operand");
        assert_eq!(
            evaluate_expr_with_parameters(&FunctionExpr::Parsed(expr), 0.0, &BTreeMap::new()),
            Some(435.0)
        );
    }

    #[test]
    fn decodes_three_digit_literal_before_an_infix_operator() {
        let payload = payload_from_words(&[
            0x0112, 0x0000, 0x0003, 0x0006, 0x0000, 0x0101, 0x1003, 0x6000,
        ]);
        let parameters = BTreeMap::from([(0, ParameterBinding::value("分母".to_string(), 6.0))]);
        let expr = try_decode_inner_function_expr(&payload, &parameters)
            .expect("the native 360 / parameter payload must decode");
        assert_eq!(function_expr_label(expr.clone()), "360 / 分母");
        assert_eq!(
            evaluate_expr_with_parameters(&expr, 0.0, &BTreeMap::new()),
            Some(60.0)
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
    fn decodes_moving_pulley_function_refs_from_grouped_payload() {
        let data = include_bytes!("../../../tests/Samples/未分类档/动滑轮2.gsp");
        let file = GspFile::parse(data).expect("fixture parses");
        let groups = file.object_groups();
        let expr =
            try_decode_function_expr(&file, &groups, &groups[19]).expect("function h decodes");

        let label = function_expr_label(expr.clone());
        assert!(label.contains("√"), "expected square roots in {label}");
        assert!(
            label.contains("tan(x)"),
            "expected tangent helper in {label}"
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
    fn decodes_plotted_function_definition_payload_from_sine_transform_sample() {
        let Ok(data) =
            fs::read("tests/Samples/个人专栏/郑飞宇作品/正弦型函数图像变换(修正颜色).gsp")
        else {
            return;
        };
        let file = GspFile::parse(&data).expect("sample parses");
        let groups = file.object_groups();
        let function_group = groups
            .iter()
            .find(|group| group.ordinal == 42)
            .expect("expected function definition group");
        assert!(
            function_definition_is_plotted(&file, &groups, function_group),
            "expected #42 to be referenced directly by a FunctionPlot"
        );

        let expression = try_decode_function_expr(&file, &groups, function_group)
            .expect("the exact FunctionDefinition -> FunctionPlot family should decode");
        assert!(crate::runtime::functions::function_expr_contains_variable(
            &expression
        ));
    }

    #[test]
    fn decodes_liyougui_grouped_function_iteration_payload() {
        let data =
            include_bytes!("../../../tests/Samples/个人专栏/李有贵作品/函数图象迭代(liyougui).gsp");
        let file = GspFile::parse(data).expect("fixture parses");
        let groups = file.object_groups();
        let expression = try_decode_function_expr(&file, &groups, &groups[14])
            .expect("native 0x0100 numeric suffix makes the grouped payload complete");
        assert_eq!(function_expr_label(expression), "2*(C + m)");
    }

    #[test]
    fn decodes_nested_function_reference_plot_from_normal_distribution() {
        let data = include_bytes!("../../../tests/Samples/个人专栏/向忠作品/正态分布.gsp");
        let file = GspFile::parse(data).expect("fixture parses");
        let groups = file.object_groups();
        let density = try_decode_function_expr(&file, &groups, &groups[2])
            .expect("normal density function should decode from its native constants");
        let density_at_mean =
            evaluate_expr_with_parameters(&density, 0.0, &BTreeMap::new()).unwrap();
        assert!((density_at_mean - 0.398_942_280_401_432_7).abs() < 1e-12);
        let expression = try_decode_function_expr(&file, &groups, &groups[90])
            .expect("nested plotted function references should decode");
        assert!(crate::runtime::functions::function_expr_contains_variable(
            &expression
        ));
    }

    #[test]
    fn decodes_nested_function_reference_plot_from_water_drop() {
        let data = include_bytes!("../../../tests/Samples/未分类档/水滴.gsp");
        let file = GspFile::parse(data).expect("fixture parses");
        let groups = file.object_groups();
        let expression = try_decode_function_expr(&file, &groups, &groups[66])
            .expect("function references are indexed within the function-parent table");
        assert!(crate::runtime::functions::function_expr_contains_variable(
            &expression
        ));
    }

    #[test]
    fn decodes_nested_function_reference_plot_from_exponential_properties() {
        let data =
            include_bytes!("../../../tests/Samples/个人专栏/向忠作品/指数函数的图象和性质.gsp");
        let file = GspFile::parse(data).expect("fixture parses");
        let groups = file.object_groups();
        let expression = try_decode_function_expr(&file, &groups, &groups[318])
            .expect("nested function definition #319 should decode");
        assert!(crate::runtime::functions::function_expr_contains_variable(
            &expression
        ));
    }
}
