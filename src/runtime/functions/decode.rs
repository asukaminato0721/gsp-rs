use std::collections::BTreeMap;

use crate::format::{GspFile, ObjectGroup, read_f64, read_u16, read_u32};
use crate::runtime::extract::find_indexed_path;

use super::expr::{
    BinaryOp, FunctionExpr, FunctionPlotDescriptor, FunctionPlotMode, FunctionTerm,
    ParsedFunctionExpr, canonicalize_function_expr, decode_unary_function,
    function_term_contains_symbol,
};

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ParameterBinding {
    name: String,
    value: f64,
}

pub(crate) fn decode_function_expr(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
) -> Option<FunctionExpr> {
    let payload = group
        .records
        .iter()
        .find(|record| record.record_type == 0x0907)
        .map(|record| record.payload(&file.data))?;
    let parameters = collect_parameter_bindings(file, groups, group);

    let text = extract_inline_function_token(payload)?;
    if text.eq_ignore_ascii_case("x") {
        return Some(FunctionExpr::Identity);
    }
    if let Ok(value) = text.parse::<f64>() {
        if value == 0.0
            && let Some(expr) = decode_inner_function_expr(payload, &parameters)
        {
            return Some(expr);
        }
        return Some(FunctionExpr::Constant(value));
    }
    decode_inner_function_expr(payload, &parameters)
}

pub(crate) fn decode_function_plot_descriptor(payload: &[u8]) -> Option<FunctionPlotDescriptor> {
    if payload.len() < 24 {
        return None;
    }

    let x_min = read_f64(payload, 0);
    let x_max = read_f64(payload, 8);
    let sample_count = read_u32(payload, 16) as usize;
    let mode = match read_u32(payload, 20) & 0xffff {
        2 => FunctionPlotMode::Polar,
        _ => FunctionPlotMode::Cartesian,
    };
    if !x_min.is_finite() || !x_max.is_finite() || x_min == x_max {
        return None;
    }

    Some(FunctionPlotDescriptor {
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
) -> BTreeMap<u16, ParameterBinding> {
    let mut bindings = BTreeMap::new();
    let Some(path) = find_indexed_path(file, group) else {
        return bindings;
    };
    for (index, ordinal) in path.refs.iter().copied().enumerate() {
        let Some(parameter_group) = groups.get(ordinal.saturating_sub(1)) else {
            continue;
        };
        if let Some(binding) = decode_parameter_binding(file, parameter_group) {
            bindings.insert(index as u16, binding);
        }
    }
    bindings
}

fn decode_parameter_binding(file: &GspFile, group: &ObjectGroup) -> Option<ParameterBinding> {
    if (group.header.kind()) == crate::format::GroupKind::ParameterAnchor {
        return decode_parameter_anchor_binding(file, group);
    }

    let payload = group
        .records
        .iter()
        .find(|record| record.record_type == 0x0907)
        .map(|record| record.payload(&file.data))?;
    let label_payload = group
        .records
        .iter()
        .find(|record| record.record_type == 0x07d5)
        .map(|record| record.payload(&file.data))?;
    let name = decode_parameter_name(label_payload)?;
    let value = if is_slider_parameter_name(&name) {
        read_f64(payload, 52)
    } else {
        f64::from(read_u16(payload, payload.len().checked_sub(2)?))
    };
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
    let value = match point_group.header.kind() {
        kind if kind.is_point_constraint() => point_group
            .records
            .iter()
            .find(|record| record.record_type == 0x07d3 && record.length == 12)
            .map(|record| read_f64(record.payload(&file.data), 4))
            .filter(|value| value.is_finite())?,
        crate::format::GroupKind::Point => {
            let payload = point_group
                .records
                .iter()
                .find(|record| record.record_type == 0x0907)
                .map(|record| record.payload(&file.data))?;
            if payload.len() >= 60 {
                read_f64(payload, 52)
            } else {
                f64::from(read_u16(payload, payload.len().checked_sub(2)?))
            }
        }
        _ => return None,
    };
    Some(ParameterBinding {
        name: format!("__param_anchor_{}", group.ordinal),
        value,
    })
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

fn is_slider_parameter_name(name: &str) -> bool {
    name.contains('₁')
        || name.contains('₂')
        || name.contains('₃')
        || name.contains('₄')
        || (name.contains('[') && name.ends_with(']'))
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

pub(crate) fn decode_inner_function_expr(
    payload: &[u8],
    parameters: &BTreeMap<u16, ParameterBinding>,
) -> Option<FunctionExpr> {
    parse_function_expr(payload, parameters).map(canonicalize_function_expr)
}

fn parse_function_expr(
    payload: &[u8],
    parameters: &BTreeMap<u16, ParameterBinding>,
) -> Option<ParsedFunctionExpr> {
    let words = payload
        .chunks_exact(2)
        .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
        .collect::<Vec<_>>();
    let marker_index = words
        .windows(2)
        .position(|pair| matches!(pair, [0x0094, 0x0001] | [0x00a0, 0x0001]));
    if let Some(marker_index) = marker_index
        && let Some((parsed, _)) = parse_function_expr_from(&words, marker_index + 2, parameters)
    {
        return Some(parsed);
    }
    find_fallback_function_expr(&words, parameters)
}

fn parse_function_expr_from(
    words: &[u16],
    start: usize,
    parameters: &BTreeMap<u16, ParameterBinding>,
) -> Option<(ParsedFunctionExpr, usize)> {
    let mut index = start;
    let head = parse_function_term(words, &mut index, parameters)?;
    let mut tail = Vec::new();
    while index < words.len() {
        let op = match words[index] {
            0x1000 => BinaryOp::Add,
            0x1001 => BinaryOp::Sub,
            0x1003 => BinaryOp::Div,
            _ => break,
        };
        index += 1;
        let term = parse_function_term(words, &mut index, parameters)?;
        tail.push((op, term));
    }
    Some((ParsedFunctionExpr { head, tail }, index))
}

fn find_fallback_function_expr(
    words: &[u16],
    parameters: &BTreeMap<u16, ParameterBinding>,
) -> Option<ParsedFunctionExpr> {
    (0..words.len())
        .filter_map(|start| parse_function_expr_from(words, start, parameters))
        .find_map(|(parsed, end)| {
            (parsed_contains_symbol(&parsed) && has_ignorable_expr_suffix(words, end))
                .then_some(parsed)
        })
}

fn has_ignorable_expr_suffix(words: &[u16], end: usize) -> bool {
    if end >= words.len() {
        return true;
    }
    let suffix = &words[end..];
    matches!(
        suffix,
        [0x0201] | [0x0101] | [0x0000, 0x0101] | [0x0000, 0x0000, 0x0101]
    )
}

fn parsed_contains_symbol(parsed: &ParsedFunctionExpr) -> bool {
    function_term_contains_symbol(&parsed.head)
        || parsed
            .tail
            .iter()
            .any(|(_, term)| function_term_contains_symbol(term))
}

fn parse_function_term(
    words: &[u16],
    index: &mut usize,
    parameters: &BTreeMap<u16, ParameterBinding>,
) -> Option<FunctionTerm> {
    let mut term = parse_atomic_term(words, index, parameters)?;
    while *index < words.len() && words[*index] == 0x1002 {
        *index += 1;
        let rhs = parse_atomic_term(words, index, parameters)?;
        term = FunctionTerm::Product(Box::new(term), Box::new(rhs));
    }
    Some(term)
}

fn parse_atomic_term(
    words: &[u16],
    index: &mut usize,
    parameters: &BTreeMap<u16, ParameterBinding>,
) -> Option<FunctionTerm> {
    let mut term = parse_atomic_base(words, index, parameters)?;
    while *index < words.len() && words[*index] == 0x1004 {
        *index += 1;
        let exponent = parse_atomic_base(words, index, parameters)?;
        term = FunctionTerm::Power(Box::new(term), Box::new(exponent));
    }
    Some(term)
}

fn parse_atomic_base(
    words: &[u16],
    index: &mut usize,
    parameters: &BTreeMap<u16, ParameterBinding>,
) -> Option<FunctionTerm> {
    if *index >= words.len() {
        return None;
    }
    if let Some(op) = decode_unary_function(words[*index]) {
        if *index + 2 < words.len() && words[*index + 1] == 0x000f && words[*index + 2] == 0x000c {
            *index += 3;
            return Some(FunctionTerm::UnaryX(op));
        }
        return None;
    }
    if (words[*index] & 0xfff0) == 0x6000 {
        let parameter_index = words[*index] & 0x000f;
        *index += 1;
        let binding = parameters.get(&parameter_index)?.clone();
        return Some(FunctionTerm::Parameter(binding.name, binding.value));
    }
    if *index + 1 < words.len() && words[*index] == 0x000f && words[*index + 1] == 0x000c {
        *index += 2;
        return Some(FunctionTerm::Variable);
    }
    if words[*index] == 0x000f {
        *index += 1;
        return Some(FunctionTerm::Variable);
    }
    let value = words[*index];
    *index += 1;
    Some(FunctionTerm::Constant(f64::from(value)))
}
