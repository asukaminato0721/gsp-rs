use std::collections::{BTreeMap, BTreeSet};

use crate::format::{GspFile, ObjectGroup, read_f64, read_u16, read_u32};
use crate::runtime::extract::{find_indexed_path, try_decode_parameter_control_value_for_group};
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
        let payload = group
            .records
            .iter()
            .find(|record| record.record_type == RECORD_FUNCTION_EXPR_PAYLOAD)
            .map(|record| record.payload(&file.data))
            .ok_or(FunctionExprParseError::NoExpressionFound { word_len: 0 })?;
        let parameters = collect_parameter_bindings(file, groups, group, visiting);

        let text = extract_inline_function_token(payload).ok_or(
            FunctionExprParseError::NoExpressionFound {
                word_len: payload.len() / 2,
            },
        )?;
        if text.eq_ignore_ascii_case("x") {
            Ok(FunctionExpr::Identity)
        } else if let Ok(value) = text.parse::<f64>() {
            if value == 0.0 {
                try_decode_inner_function_expr(payload, &parameters)
                    .or(Ok(FunctionExpr::Constant(value)))
            } else {
                Ok(FunctionExpr::Constant(value))
            }
        } else {
            try_decode_inner_function_expr(payload, &parameters)
        }
    })();
    visiting.remove(&group.ordinal);
    expr
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

fn decode_parameter_binding_recursive(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
    visiting: &mut BTreeSet<usize>,
) -> Option<ParameterBinding> {
    if (group.header.kind()) == crate::format::GroupKind::ParameterAnchor {
        return decode_parameter_anchor_binding(file, group);
    }
    if (group.header.kind()) == crate::format::GroupKind::FunctionExpr {
        let expr = decode_function_expr_recursive(file, groups, group, visiting).ok()?;
        return Some(ParameterBinding {
            name: function_expr_label(expr.clone()),
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
                let expr = self.parse_expr_bp(4).map_err(|_| {
                    FunctionExprParseError::InvalidUnaryOperand {
                        offset,
                        opcode: self.tokens.words[offset - self.tokens.base_offset],
                    }
                })?;
                Ok(FunctionAst::Unary {
                    op,
                    expr: Box::new(expr),
                })
            }
            found @ (FunctionToken::Add
            | FunctionToken::Sub
            | FunctionToken::Mul
            | FunctionToken::Div
            | FunctionToken::Pow) => Err(FunctionExprParseError::UnexpectedToken { offset, found }),
        }
    }

    fn peek_infix(&mut self) -> Result<Option<(BinaryOp, u8, u8)>, FunctionExprParseError> {
        Ok(match self.tokens.peek()? {
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
                    width_words: 1,
                });
            }
            LexedFunctionToken {
                kind: FunctionToken::Constant(f64::from(word)),
                width_words: 1,
            }
        }
    };
    Ok(token)
}

pub(crate) fn try_decode_inner_function_expr(
    payload: &[u8],
    parameters: &BTreeMap<u16, ParameterBinding>,
) -> Result<FunctionExpr, FunctionExprParseError> {
    parse_function_expr(payload, parameters).map(canonicalize_function_expr)
}

fn parse_function_expr(
    payload: &[u8],
    parameters: &BTreeMap<u16, ParameterBinding>,
) -> Result<FunctionAst, FunctionExprParseError> {
    let words = payload
        .chunks_exact(2)
        .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
        .collect::<Vec<_>>();
    let mut marker_error = None;
    let marker_index = words
        .windows(2)
        .position(|pair| *pair == FUNCTION_EXPR_MARKER_A || *pair == FUNCTION_EXPR_MARKER_B);
    if let Some(marker_index) = marker_index {
        match parse_function_expr_from(&words, marker_index + 2, parameters) {
            Ok((parsed, _)) => return Ok(parsed),
            Err(error) => marker_error = Some(error),
        }
    }
    find_fallback_function_expr(&words, parameters)
        .or_else(|fallback_error| Err(marker_error.unwrap_or(fallback_error)))
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
        [0x0201] | [0x0101] | [0x0000, 0x0101] | [0x0000, 0x0000, 0x0101]
    )
}

fn parsed_contains_symbol(parsed: &FunctionAst) -> bool {
    function_ast_contains_symbol(parsed)
}

#[cfg(test)]
mod parse_tests {
    use super::{FunctionExprParseError, parse_function_expr};
    use std::collections::BTreeMap;

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
}
