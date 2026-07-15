use std::collections::BTreeMap;

use crate::runtime::payload_consts::{
    EXPR_EULER_WORD, EXPR_OP_ADD, EXPR_OP_DIV, EXPR_OP_MUL, EXPR_OP_POW, EXPR_OP_SUB,
    EXPR_PARAMETER_MASK, EXPR_PARAMETER_PREFIX, EXPR_PI_SUFFIX, EXPR_PI_WORD, EXPR_VARIABLE_SUFFIX,
    EXPR_VARIABLE_WORD, FUNCTION_EXPR_MARKER_A, FUNCTION_EXPR_MARKER_B,
    RECORD_FUNCTION_EXPR_PAYLOAD,
};

use super::super::expr::{
    BinaryOp, FunctionAst, FunctionExpr, UnaryFunction, canonicalize_function_expr,
    decode_unary_function, function_ast_contains_symbol,
};
use super::{
    FunctionExprParseError, FunctionToken, ParameterBinding,
    try_decode_embedded_static_function_expr, with_function_payload_context,
};

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

fn unary_ast(op: UnaryFunction, expr: FunctionAst) -> FunctionAst {
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
            FunctionToken::PiConstant => Ok(FunctionAst::PiConstant),
            FunctionToken::EulerConstant => Ok(FunctionAst::EulerConstant),
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
            _ => usize::from(matches!(next, Some(0x0100 | 0x0101 | 0x0201))),
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
        EXPR_PI_WORD => LexedFunctionToken {
            kind: FunctionToken::PiConstant,
            width_words: 1 + usize::from(matches!(words.get(1), Some(&EXPR_PI_SUFFIX))),
        },
        EXPR_EULER_WORD => LexedFunctionToken {
            kind: FunctionToken::EulerConstant,
            width_words: 1,
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
            if word < EXPR_OP_ADD {
                LexedFunctionToken {
                    kind: FunctionToken::Constant(f64::from(word)),
                    width_words: 1 + suffix_width(word, words.get(1).copied()),
                }
            } else {
                return Err(FunctionExprParseError::UnknownOpcode {
                    offset,
                    opcode: word,
                });
            }
        }
    };
    Ok(token)
}

fn decode_decimal_digit_literal(words: &[u16]) -> Option<(f64, usize)> {
    if let Some(literal) = decode_terminated_decimal_literal(words) {
        return Some(literal);
    }
    if let Some(literal) = decode_three_digit_literal(words) {
        return Some(literal);
    }
    let first = *words.first()?;
    let second = *words.get(1)?;
    let next = words.get(2).copied();
    if first == 0x000a {
        let mut index = 1;
        let mut divisor = 1.0;
        let mut value = 0.0;
        while let Some(digit) = words.get(index).copied().filter(|word| *word <= 9) {
            divisor *= 10.0;
            value += f64::from(digit) / divisor;
            index += 1;
        }
        if index > 1
            && matches!(
                words.get(index).copied(),
                None | Some(
                    EXPR_OP_ADD | EXPR_OP_SUB | EXPR_OP_MUL | EXPR_OP_DIV | EXPR_OP_POW | 0x000c
                )
            )
        {
            return value.is_finite().then_some((value, index));
        }
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

fn decode_terminated_decimal_literal(words: &[u16]) -> Option<(f64, usize)> {
    let terminator_index = words.iter().position(|word| *word == 0x0101)?;
    let literal_words = words.get(..terminator_index)?;
    if literal_words.is_empty()
        || literal_words.iter().any(|word| *word > 0x000a)
        || literal_words.iter().filter(|word| **word == 0x000a).count() > 1
    {
        return None;
    }
    let radix_index = literal_words.iter().position(|word| *word == 0x000a);
    let (integer_digits, fraction_digits) = match radix_index {
        Some(index) => (&literal_words[..index], &literal_words[index + 1..]),
        None => (literal_words, &[][..]),
    };
    if integer_digits.is_empty() && fraction_digits.is_empty() {
        return None;
    }
    let integer = integer_digits
        .iter()
        .try_fold(0.0, |value, digit| Some(value * 10.0 + f64::from(*digit)))?;
    let (fraction, _) = fraction_digits
        .iter()
        .try_fold((0.0, 10.0), |(value, divisor), digit| {
            Some((value + f64::from(*digit) / divisor, divisor * 10.0))
        })?;
    let value = integer + fraction;
    value.is_finite().then_some((value, terminator_index + 1))
}

fn decode_three_digit_literal(words: &[u16]) -> Option<(f64, usize)> {
    let [hundreds, tens, ones, 0x0101, ..] = words else {
        return None;
    };
    if *hundreds > 9 || *tens > 9 || *ones > 9 {
        return None;
    }
    if !matches!(
        words.get(4).copied(),
        None | Some(EXPR_OP_ADD | EXPR_OP_SUB | EXPR_OP_MUL | EXPR_OP_DIV | EXPR_OP_POW | 0x000c)
    ) {
        return None;
    }
    Some((f64::from(hundreds * 100 + tens * 10 + ones), 4))
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

pub(super) fn decode_grouped_decimal_digit_literal(words: &[u16]) -> Option<(f64, usize)> {
    if let Some(literal) = decode_terminated_decimal_literal(words) {
        return Some(literal);
    }
    if words.first().copied() == Some(0x000a) {
        let mut index = 1;
        let mut divisor = 1.0;
        let mut value = 0.0;
        while let Some(digit) = words.get(index).copied().filter(|word| *word <= 9) {
            divisor *= 10.0;
            value += f64::from(digit) / divisor;
            index += 1;
        }
        if index > 1
            && matches!(
                words.get(index).copied(),
                None | Some(
                    EXPR_OP_ADD | EXPR_OP_SUB | EXPR_OP_MUL | EXPR_OP_DIV | EXPR_OP_POW | 0x000c
                )
            )
        {
            return value.is_finite().then_some((value, index));
        }
    }
    // Some grouped payloads preserve more than one leading zero before the
    // native 0x000a radix marker (for example 00.5). Consume that complete
    // fractional token; integer digit runs retain the established grouped
    // token boundaries because adjacent digits can also be operands there.
    let mut index = words.iter().take_while(|word| **word == 0).count();
    if index == 0 || words.get(index).copied() != Some(0x000a) {
        return None;
    }
    index += 1;
    let mut divisor = 1.0;
    let mut value = 0.0;
    let fraction_start = index;
    while let Some(word) = words.get(index).copied().filter(|word| *word <= 9) {
        divisor *= 10.0;
        value += f64::from(word) / divisor;
        index += 1;
    }
    (index > fraction_start && value.is_finite()).then_some((value, index))
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

pub(super) fn decode_embedded_postfix_payload_function_expr(
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
        if let Some((value, width_words)) = decode_postfix_decimal_digit_literal(&words[index..]) {
            stack.push(FunctionAst::Constant(value));
            index += width_words;
            continue;
        }
        match word {
            EXPR_OP_ADD | EXPR_OP_SUB | EXPR_OP_MUL | EXPR_OP_DIV | EXPR_OP_POW => {
                if stack.len() < 2 {
                    return Err(FunctionExprParseError::InvalidPostfixArity {
                        offset: index,
                        opcode: word,
                        expected: 2,
                        found: stack.len(),
                    });
                }
                let rhs = stack.pop().expect("postfix arity checked");
                let lhs = stack.pop().expect("postfix arity checked");
                let op = match word {
                    EXPR_OP_ADD => BinaryOp::Add,
                    EXPR_OP_SUB => BinaryOp::Sub,
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
            EXPR_PI_WORD => {
                stack.push(FunctionAst::PiConstant);
                index += 1 + usize::from(matches!(words.get(index + 1), Some(&EXPR_PI_SUFFIX)));
            }
            EXPR_EULER_WORD => {
                stack.push(FunctionAst::EulerConstant);
                index += 1;
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
            _ if word < EXPR_OP_ADD => {
                stack.push(FunctionAst::Constant(f64::from(word)));
                index += 1 + postfix_suffix_width(word, words.get(index + 1).copied());
            }
            _ => {
                return Err(FunctionExprParseError::UnknownOpcode {
                    offset: index,
                    opcode: word,
                });
            }
        }
    }
    if stack.len() > 1 {
        return Err(FunctionExprParseError::TrailingPostfixOperands {
            remaining: stack.len(),
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

pub(super) fn decode_trailing_scanned_payload_function_expr(
    payload: &[u8],
    parameters: &BTreeMap<u16, ParameterBinding>,
) -> Result<FunctionExpr, FunctionExprParseError> {
    with_function_payload_context(payload, || {
        let words = payload
            .chunks_exact(2)
            .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
            .collect::<Vec<_>>();
        parse_trailing_scanned_function_expr(&words, parameters).map(canonicalize_function_expr)
    })
}

fn parse_trailing_scanned_function_expr(
    words: &[u16],
    parameters: &BTreeMap<u16, ParameterBinding>,
) -> Result<FunctionAst, FunctionExprParseError> {
    let search_start =
        trailing_calculate_expr_start(words).ok_or(FunctionExprParseError::NoExpressionFound {
            word_len: words.len(),
        })?;
    for start in search_start..words.len() {
        if matches!(words[start], 0x000c | 0x0100 | 0x0101 | 0x0201) {
            continue;
        }
        if let Ok(parsed) = parse_grouped_parameter_control_expr_at(words, start, parameters) {
            return Ok(parsed);
        }
        if let Ok((parsed, end)) = parse_function_expr_from(words, start, parameters)
            && function_ast_contains_symbol(&parsed)
            && has_ignorable_expr_suffix(words, end)
        {
            return Ok(parsed);
        }
    }
    Err(FunctionExprParseError::NoExpressionFound {
        word_len: words.len(),
    })
}

#[cfg(test)]
pub(super) fn parse_function_expr(
    payload: &[u8],
    parameters: &BTreeMap<u16, ParameterBinding>,
) -> Result<FunctionAst, FunctionExprParseError> {
    let words = payload
        .chunks_exact(2)
        .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
        .collect::<Vec<_>>();
    parse_function_expr_from_words(&words, parameters)
}

pub(super) fn parse_function_expr_from_words(
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
    PiConstant,
    EulerConstant,
    Parameter(ParameterBinding),
    Unary(UnaryFunction),
    Constant(f64),
}

#[derive(Debug, Clone, PartialEq)]
struct LexedGroupedFunctionToken {
    kind: GroupedFunctionToken,
    width_words: usize,
}

struct GroupedFunctionParser<'a> {
    words: &'a [u16],
    parameters: &'a BTreeMap<u16, ParameterBinding>,
    base_offset: usize,
    offset: usize,
    allow_unclosed_unary_argument: bool,
    allow_decimal_literals: bool,
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
            allow_decimal_literals: false,
        }
    }

    fn allowing_unclosed_unary_argument(mut self) -> Self {
        self.allow_unclosed_unary_argument = true;
        self
    }

    fn allowing_decimal_literals(mut self) -> Self {
        self.allow_decimal_literals = true;
        self
    }

    fn peek(&self) -> Result<Option<GroupedFunctionToken>, FunctionExprParseError> {
        if self.offset >= self.words.len() {
            return Ok(None);
        }
        lex_grouped_function_token(
            &self.words[self.offset..],
            self.parameters,
            self.base_offset + self.offset,
            self.allow_decimal_literals,
        )
        .map(|token| Some(token.kind))
    }

    fn bump(&mut self) -> Result<GroupedFunctionToken, FunctionExprParseError> {
        if self.offset >= self.words.len() {
            return Err(FunctionExprParseError::UnexpectedEnd {
                offset: self.base_offset + self.offset,
            });
        }
        let token = lex_grouped_function_token(
            &self.words[self.offset..],
            self.parameters,
            self.base_offset + self.offset,
            self.allow_decimal_literals,
        )?;
        self.offset += token.width_words;
        Ok(token.kind)
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

    fn parse_group_body(&mut self, min_bp: u8) -> Result<FunctionAst, FunctionExprParseError> {
        let mut lhs = self.parse_prefix()?;
        loop {
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

    fn parse_prefix(&mut self) -> Result<FunctionAst, FunctionExprParseError> {
        let offset = self.base_offset + self.offset;
        match self.bump()? {
            GroupedFunctionToken::Variable => Ok(FunctionAst::Variable),
            GroupedFunctionToken::PiConstant => Ok(FunctionAst::PiConstant),
            GroupedFunctionToken::EulerConstant => Ok(FunctionAst::EulerConstant),
            GroupedFunctionToken::Parameter(binding) => Ok(binding
                .expr
                .unwrap_or(FunctionAst::Parameter(binding.name, binding.value))),
            GroupedFunctionToken::Constant(value) => Ok(FunctionAst::Constant(value)),
            GroupedFunctionToken::Unary(op) => {
                let expr = if matches!(self.peek()?, Some(GroupedFunctionToken::LParen)) {
                    self.parse_unary_grouped_argument()?
                } else {
                    let expr = self.parse_group_body(0)?;
                    if matches!(self.peek()?, Some(GroupedFunctionToken::RParen)) {
                        let _ = self.bump()?;
                    }
                    expr
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
        GroupedFunctionToken::PiConstant => FunctionToken::PiConstant,
        GroupedFunctionToken::EulerConstant => FunctionToken::EulerConstant,
        GroupedFunctionToken::Parameter(binding) => FunctionToken::Parameter(binding),
        GroupedFunctionToken::Unary(op) => FunctionToken::Unary(op),
        GroupedFunctionToken::Constant(value) => FunctionToken::Constant(value),
        GroupedFunctionToken::LParen => FunctionToken::Terminator,
    }
}

fn lex_grouped_function_token(
    words: &[u16],
    parameters: &BTreeMap<u16, ParameterBinding>,
    offset: usize,
    allow_decimal_literals: bool,
) -> Result<LexedGroupedFunctionToken, FunctionExprParseError> {
    let word = *words
        .first()
        .ok_or(FunctionExprParseError::UnexpectedEnd { offset })?;
    if allow_decimal_literals
        && let Some((value, width_words)) = decode_grouped_decimal_digit_literal(words)
            .or_else(|| decode_three_digit_literal(words))
    {
        return Ok(LexedGroupedFunctionToken {
            kind: GroupedFunctionToken::Constant(value),
            width_words,
        });
    }
    let kind = match word {
        EXPR_OP_ADD => GroupedFunctionToken::Add,
        EXPR_OP_SUB => GroupedFunctionToken::Sub,
        EXPR_OP_MUL => GroupedFunctionToken::Mul,
        EXPR_OP_DIV => GroupedFunctionToken::Div,
        EXPR_OP_POW => GroupedFunctionToken::Pow,
        0x000b => GroupedFunctionToken::LParen,
        0x000c => GroupedFunctionToken::RParen,
        EXPR_PI_WORD => GroupedFunctionToken::PiConstant,
        EXPR_EULER_WORD => GroupedFunctionToken::EulerConstant,
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
        _ if word < EXPR_OP_ADD => {
            return Ok(LexedGroupedFunctionToken {
                kind: GroupedFunctionToken::Constant(f64::from(word)),
                width_words: 1 + usize::from(matches!(
                    words.get(1).copied(),
                    Some(0x0100 | 0x0101 | 0x0201)
                )),
            });
        }
        _ => {
            return Err(FunctionExprParseError::UnknownOpcode {
                offset,
                opcode: word,
            });
        }
    };
    Ok(LexedGroupedFunctionToken {
        kind,
        width_words: 1,
    })
}

pub(super) fn parse_grouped_function_expr_from_words(
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

pub(super) fn parse_grouped_function_expr_at(
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

pub(super) fn parse_grouped_parameter_control_expr_at(
    words: &[u16],
    start: usize,
    parameters: &BTreeMap<u16, ParameterBinding>,
) -> Result<FunctionAst, FunctionExprParseError> {
    parse_grouped_parameter_control_value_at(words, start, parameters, true)
}

pub(super) fn parse_grouped_parameter_control_scalar_at(
    words: &[u16],
    start: usize,
    parameters: &BTreeMap<u16, ParameterBinding>,
) -> Result<FunctionAst, FunctionExprParseError> {
    parse_grouped_parameter_control_value_at(words, start, parameters, false)
}

fn parse_grouped_parameter_control_value_at(
    words: &[u16],
    start: usize,
    parameters: &BTreeMap<u16, ParameterBinding>,
    require_symbol: bool,
) -> Result<FunctionAst, FunctionExprParseError> {
    let mut parser = GroupedFunctionParser::new(&words[start..], parameters, start)
        .allowing_unclosed_unary_argument()
        .allowing_decimal_literals();
    let expr = parser.parse_expr(0)?;
    if require_symbol && !parsed_contains_symbol(&expr) {
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

pub(super) fn parse_function_expr_from(
    words: &[u16],
    start: usize,
    parameters: &BTreeMap<u16, ParameterBinding>,
) -> Result<(FunctionAst, usize), FunctionExprParseError> {
    let mut parser = FunctionExprParser::new(&words[start..], parameters, start);
    let parsed = parser.parse_expr()?;
    Ok((parsed, start + parser.words_consumed()))
}

pub(super) fn embedded_calculate_expr_start(words: &[u16]) -> Option<usize> {
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

    trailing_calculate_expr_start(words)
}

pub(super) fn trailing_calculate_expr_start(words: &[u16]) -> Option<usize> {
    words
        .windows(2)
        .rposition(|pair| pair == [0x0112, 0x0000])
        .map(|marker_index| marker_index + 2)
        .filter(|start| *start < words.len())
}

pub(super) fn has_ignorable_expr_suffix(words: &[u16], end: usize) -> bool {
    if end >= words.len() {
        return true;
    }
    let suffix = &words[end..];
    matches!(
        suffix,
        [0x000c | 0x0201 | 0x0101 | 0x0100] | [0x0000, 0x0101] | [0x0000, 0x0000, 0x0101]
    )
}

fn parsed_contains_symbol(parsed: &FunctionAst) -> bool {
    function_ast_contains_symbol(parsed)
}
