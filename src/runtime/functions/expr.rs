use crate::runtime::geometry::format_number;

pub(crate) use gsp_runtime_core::{BinaryOp, FunctionAst, FunctionExpr, UnaryFunction};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FunctionPlotMode {
    Cartesian,
    Polar,
}

#[derive(Debug, Clone)]
pub(crate) struct FunctionPlotDescriptor {
    pub(crate) x_min: f64,
    pub(crate) x_max: f64,
    pub(crate) sample_count: usize,
    pub(crate) mode: FunctionPlotMode,
}

pub(crate) fn function_expr_ast(expr: FunctionExpr) -> FunctionAst {
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

pub(crate) fn function_expr_label(expr: FunctionExpr) -> String {
    function_expr_label_with_variable(expr, "x")
}

pub(crate) fn function_expr_label_with_variable(expr: FunctionExpr, variable: &str) -> String {
    match expr {
        FunctionExpr::Constant(value) => format_number(value),
        FunctionExpr::Identity => variable.to_string(),
        FunctionExpr::SinIdentity => format!("sin({variable})"),
        FunctionExpr::CosIdentityPlus(offset) => {
            format!("cos({variable}) + {}", format_number(offset))
        }
        FunctionExpr::TanIdentityMinus(offset) => {
            format!("tan({variable}) - {}", format_number(offset))
        }
        FunctionExpr::Parsed(ast) => format_function_ast(&ast, variable, 0),
    }
}

pub(crate) fn function_name_for_index(
    index: usize,
    total: usize,
    expr: &FunctionExpr,
) -> &'static str {
    let _ = (total, expr);
    match index {
        0 => "f",
        1 => "g",
        2 => "h",
        3 => "p",
        _ => "q",
    }
}

fn format_function_ast(expr: &FunctionAst, variable: &str, parent_prec: u8) -> String {
    match expr {
        FunctionAst::Variable => variable.to_string(),
        FunctionAst::Constant(value) => format_number(*value),
        FunctionAst::PiConstant => "π".to_string(),
        FunctionAst::EulerConstant => "e".to_string(),
        FunctionAst::PiAngle => "180".to_string(),
        FunctionAst::Parameter(name, _) => name.clone(),
        FunctionAst::Unary { op, expr } => match op {
            UnaryFunction::Abs => {
                let inner = format_function_ast(expr, variable, 4);
                format!("|{inner}|")
            }
            UnaryFunction::Sqrt if is_atomic(expr) => {
                let inner = format_function_ast(expr, variable, 4);
                format!("√{inner}")
            }
            UnaryFunction::Sqrt => {
                let inner = format_function_ast(expr, variable, 4);
                format!("√({inner})")
            }
            UnaryFunction::Sin => {
                let inner = format_unary_call_arg(expr, variable, parent_prec);
                format!("sin({inner})")
            }
            UnaryFunction::Cos => {
                let inner = format_unary_call_arg(expr, variable, parent_prec);
                format!("cos({inner})")
            }
            UnaryFunction::Tan => {
                let inner = format_unary_call_arg(expr, variable, parent_prec);
                format!("tan({inner})")
            }
            UnaryFunction::Ln => {
                let inner = format_unary_call_arg(expr, variable, parent_prec);
                format!("ln({inner})")
            }
            UnaryFunction::Log10 => {
                let inner = format_unary_call_arg(expr, variable, parent_prec);
                format!("log({inner})")
            }
            UnaryFunction::Sign => {
                let inner = format_unary_call_arg(expr, variable, parent_prec);
                format!("sgn({inner})")
            }
            UnaryFunction::Round => {
                let inner = format_unary_call_arg(expr, variable, parent_prec);
                format!("round({inner})")
            }
            UnaryFunction::Trunc => {
                let inner = format_unary_call_arg(expr, variable, parent_prec);
                format!("trunc({inner})")
            }
        },
        FunctionAst::Binary { lhs, op, rhs } => {
            let (prec, right_assoc) = binary_precedence(*op);
            let left = format_function_ast(lhs, variable, prec);
            let right = format_function_ast(rhs, variable, prec + u8::from(!right_assoc));
            let text = format!(
                "{}{}{}",
                left,
                match op {
                    BinaryOp::Add => " + ",
                    BinaryOp::Sub => " - ",
                    BinaryOp::Mul => "*",
                    BinaryOp::Div => " / ",
                    BinaryOp::Pow => "^",
                },
                right
            );
            if prec < parent_prec {
                format!("({text})")
            } else {
                text
            }
        }
    }
}

fn is_atomic(expr: &FunctionAst) -> bool {
    matches!(
        expr,
        FunctionAst::Variable
            | FunctionAst::Constant(_)
            | FunctionAst::PiAngle
            | FunctionAst::Parameter(_, _)
    )
}

fn format_unary_call_arg(expr: &FunctionAst, variable: &str, _parent_prec: u8) -> String {
    format_function_ast(expr, variable, 0)
}

fn binary_precedence(op: BinaryOp) -> (u8, bool) {
    match op {
        BinaryOp::Add | BinaryOp::Sub => (1, false),
        BinaryOp::Mul | BinaryOp::Div => (2, false),
        BinaryOp::Pow => (3, true),
    }
}

pub(super) fn function_expr_uses_trig(expr: FunctionExpr) -> bool {
    match expr {
        FunctionExpr::SinIdentity
        | FunctionExpr::CosIdentityPlus(_)
        | FunctionExpr::TanIdentityMinus(_) => true,
        FunctionExpr::Parsed(ast) => function_ast_uses_trig(&ast),
        _ => false,
    }
}

pub(crate) fn function_expr_contains_variable(expr: &FunctionExpr) -> bool {
    match expr {
        FunctionExpr::Identity
        | FunctionExpr::SinIdentity
        | FunctionExpr::CosIdentityPlus(_)
        | FunctionExpr::TanIdentityMinus(_) => true,
        FunctionExpr::Parsed(ast) => function_ast_contains_variable(ast),
        FunctionExpr::Constant(_) => false,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RationalPiPeriod {
    pub(crate) numerator: i64,
    pub(crate) denominator: i64,
}

impl RationalPiPeriod {
    fn new(numerator: i64, denominator: i64) -> Option<Self> {
        if numerator <= 0 || denominator <= 0 {
            return None;
        }
        let gcd = gcd_i64(numerator, denominator);
        Some(Self {
            numerator: numerator / gcd,
            denominator: denominator / gcd,
        })
    }

    pub(crate) fn as_f64(self) -> f64 {
        std::f64::consts::PI * (self.numerator as f64) / (self.denominator as f64)
    }
}

pub(crate) fn function_expr_period(expr: &FunctionExpr) -> Option<RationalPiPeriod> {
    match expr {
        FunctionExpr::Constant(_) | FunctionExpr::Identity => None,
        FunctionExpr::SinIdentity | FunctionExpr::CosIdentityPlus(_) => RationalPiPeriod::new(2, 1),
        FunctionExpr::TanIdentityMinus(_) => RationalPiPeriod::new(1, 1),
        FunctionExpr::Parsed(ast) => function_ast_period(ast),
    }
}

pub(crate) fn common_period(
    left: RationalPiPeriod,
    right: RationalPiPeriod,
) -> Option<RationalPiPeriod> {
    let common_denominator = lcm_i64(left.denominator, right.denominator)?;
    let left_scaled = left
        .numerator
        .checked_mul(common_denominator / left.denominator)?;
    let right_scaled = right
        .numerator
        .checked_mul(common_denominator / right.denominator)?;
    RationalPiPeriod::new(lcm_i64(left_scaled, right_scaled)?, common_denominator)
}

fn function_ast_uses_trig(expr: &FunctionAst) -> bool {
    match expr {
        FunctionAst::Unary {
            op: UnaryFunction::Sin | UnaryFunction::Cos | UnaryFunction::Tan,
            ..
        } => true,
        FunctionAst::Unary { expr, .. } => function_ast_uses_trig(expr),
        FunctionAst::Binary { lhs, rhs, .. } => {
            function_ast_uses_trig(lhs) || function_ast_uses_trig(rhs)
        }
        _ => false,
    }
}

pub(super) fn function_ast_contains_symbol(expr: &FunctionAst) -> bool {
    match expr {
        FunctionAst::Variable | FunctionAst::Parameter(_, _) => true,
        FunctionAst::Unary { expr, .. } => function_ast_contains_symbol(expr),
        FunctionAst::Binary { lhs, rhs, .. } => {
            function_ast_contains_symbol(lhs) || function_ast_contains_symbol(rhs)
        }
        FunctionAst::Constant(_)
        | FunctionAst::PiConstant
        | FunctionAst::EulerConstant
        | FunctionAst::PiAngle => false,
    }
}

fn function_ast_contains_variable(expr: &FunctionAst) -> bool {
    match expr {
        FunctionAst::Variable => true,
        FunctionAst::Unary { expr, .. } => function_ast_contains_variable(expr),
        FunctionAst::Binary { lhs, rhs, .. } => {
            function_ast_contains_variable(lhs) || function_ast_contains_variable(rhs)
        }
        FunctionAst::Constant(_)
        | FunctionAst::PiConstant
        | FunctionAst::EulerConstant
        | FunctionAst::PiAngle
        | FunctionAst::Parameter(_, _) => false,
    }
}

fn function_ast_period(expr: &FunctionAst) -> Option<RationalPiPeriod> {
    match expr {
        FunctionAst::Unary {
            op: op @ (UnaryFunction::Sin | UnaryFunction::Cos | UnaryFunction::Tan),
            expr,
        } => period_from_linear_arg(*op, expr),
        FunctionAst::Unary { .. } => None,
        FunctionAst::Binary { lhs, op, rhs } => match op {
            BinaryOp::Add | BinaryOp::Sub => {
                match (function_ast_period(lhs), function_ast_period(rhs)) {
                    (Some(left), Some(right)) => common_period(left, right),
                    (Some(period), None) if !function_ast_contains_variable(rhs) => Some(period),
                    (None, Some(period)) if !function_ast_contains_variable(lhs) => Some(period),
                    _ => None,
                }
            }
            BinaryOp::Mul => match (function_ast_period(lhs), function_ast_period(rhs)) {
                (Some(left), Some(right)) => common_period(left, right),
                (Some(period), None) if !function_ast_contains_variable(rhs) => Some(period),
                (None, Some(period)) if !function_ast_contains_variable(lhs) => Some(period),
                _ => None,
            },
            BinaryOp::Div => match (function_ast_period(lhs), function_ast_period(rhs)) {
                (Some(period), None) if !function_ast_contains_variable(rhs) => Some(period),
                _ => None,
            },
            BinaryOp::Pow => None,
        },
        FunctionAst::Variable
        | FunctionAst::Constant(_)
        | FunctionAst::PiConstant
        | FunctionAst::EulerConstant
        | FunctionAst::PiAngle
        | FunctionAst::Parameter(_, _) => None,
    }
}

fn period_from_linear_arg(op: UnaryFunction, expr: &FunctionAst) -> Option<RationalPiPeriod> {
    let (coefficient, _) = linear_ast(expr)?;
    let coefficient = coefficient.abs();
    if coefficient <= 1e-9 {
        return None;
    }
    let (num, den) = rationalize_f64(coefficient)?;
    let base = match op {
        UnaryFunction::Tan => 1i64,
        UnaryFunction::Sin | UnaryFunction::Cos => 2i64,
        _ => return None,
    };
    RationalPiPeriod::new(base.checked_mul(den)?, num)
}

fn linear_ast(expr: &FunctionAst) -> Option<(f64, f64)> {
    match expr {
        FunctionAst::Variable => Some((1.0, 0.0)),
        FunctionAst::Constant(value) => Some((0.0, *value)),
        FunctionAst::PiConstant => Some((0.0, std::f64::consts::PI)),
        FunctionAst::EulerConstant => Some((0.0, std::f64::consts::E)),
        FunctionAst::Binary { lhs, op, rhs } => match op {
            BinaryOp::Add => {
                let (la, lb) = linear_ast(lhs)?;
                let (ra, rb) = linear_ast(rhs)?;
                Some((la + ra, lb + rb))
            }
            BinaryOp::Sub => {
                let (la, lb) = linear_ast(lhs)?;
                let (ra, rb) = linear_ast(rhs)?;
                Some((la - ra, lb - rb))
            }
            BinaryOp::Mul => {
                if let Some(constant) = ast_constant(lhs) {
                    let (a, b) = linear_ast(rhs)?;
                    Some((constant * a, constant * b))
                } else if let Some(constant) = ast_constant(rhs) {
                    let (a, b) = linear_ast(lhs)?;
                    Some((constant * a, constant * b))
                } else {
                    None
                }
            }
            BinaryOp::Div => {
                let constant = ast_constant(rhs)?;
                if constant.abs() <= 1e-9 {
                    return None;
                }
                let (a, b) = linear_ast(lhs)?;
                Some((a / constant, b / constant))
            }
            BinaryOp::Pow => None,
        },
        _ => None,
    }
}

fn ast_constant(expr: &FunctionAst) -> Option<f64> {
    match expr {
        FunctionAst::Constant(value) => Some(*value),
        FunctionAst::PiConstant => Some(std::f64::consts::PI),
        FunctionAst::EulerConstant => Some(std::f64::consts::E),
        _ => None,
    }
}

fn rationalize_f64(value: f64) -> Option<(i64, i64)> {
    const MAX_DENOMINATOR: i64 = 64;
    const EPSILON: f64 = 1e-9;

    for denominator in 1..=MAX_DENOMINATOR {
        let numerator = (value * denominator as f64).round();
        if numerator <= 0.0 {
            continue;
        }
        let candidate = numerator / denominator as f64;
        if (candidate - value).abs() <= EPSILON {
            return Some((numerator as i64, denominator));
        }
    }
    None
}

fn gcd_i64(mut left: i64, mut right: i64) -> i64 {
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left.abs()
}

fn lcm_i64(left: i64, right: i64) -> Option<i64> {
    let gcd = gcd_i64(left, right);
    left.checked_div(gcd)?.checked_mul(right).map(i64::abs)
}

pub(super) fn decode_unary_function(word: u16) -> Option<UnaryFunction> {
    match word {
        0x2000 => Some(UnaryFunction::Sin),
        0x2001 => Some(UnaryFunction::Cos),
        0x2002 => Some(UnaryFunction::Tan),
        0x2006 => Some(UnaryFunction::Abs),
        0x2007 => Some(UnaryFunction::Sqrt),
        0x2008 => Some(UnaryFunction::Ln),
        0x2009 => Some(UnaryFunction::Log10),
        0x200a => Some(UnaryFunction::Sign),
        0x200b => Some(UnaryFunction::Round),
        0x200c => Some(UnaryFunction::Trunc),
        _ => None,
    }
}

pub(super) fn canonicalize_function_expr(ast: FunctionAst) -> FunctionExpr {
    match &ast {
        FunctionAst::Variable => FunctionExpr::Identity,
        FunctionAst::Unary {
            op: UnaryFunction::Sin,
            expr,
        } if matches!(expr.as_ref(), FunctionAst::Variable) => FunctionExpr::SinIdentity,
        FunctionAst::Binary { lhs, op, rhs }
            if *op == BinaryOp::Add
                && matches!(
                    lhs.as_ref(),
                    FunctionAst::Unary {
                        op: UnaryFunction::Cos,
                        expr
                    } if matches!(expr.as_ref(), FunctionAst::Variable)
                )
                && matches!(rhs.as_ref(), FunctionAst::Constant(value) if (*value - 5.0).abs() < f64::EPSILON) =>
        {
            FunctionExpr::CosIdentityPlus(5.0)
        }
        FunctionAst::Binary { lhs, op, rhs }
            if *op == BinaryOp::Sub
                && matches!(
                    lhs.as_ref(),
                    FunctionAst::Unary {
                        op: UnaryFunction::Tan,
                        expr
                    } if matches!(expr.as_ref(), FunctionAst::Variable)
                )
                && matches!(rhs.as_ref(), FunctionAst::Constant(value) if (*value - 4.0).abs() < f64::EPSILON) =>
        {
            FunctionExpr::TanIdentityMinus(4.0)
        }
        _ => FunctionExpr::Parsed(ast),
    }
}
