use crate::runtime::geometry::format_number;

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

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum FunctionExpr {
    Constant(f64),
    Identity,
    SinIdentity,
    CosIdentityPlus(f64),
    TanIdentityMinus(f64),
    Parsed(FunctionAst),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Pow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UnaryFunction {
    Sin,
    Cos,
    Tan,
    Abs,
    Sqrt,
    Ln,
    Log10,
    Sign,
    Round,
    Trunc,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum FunctionAst {
    Variable,
    Constant(f64),
    PiAngle,
    Parameter(String, f64),
    Unary {
        op: UnaryFunction,
        expr: Box<FunctionAst>,
    },
    Binary {
        lhs: Box<FunctionAst>,
        op: BinaryOp,
        rhs: Box<FunctionAst>,
    },
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
        FunctionAst::PiAngle => "180".to_string(),
        FunctionAst::Parameter(name, _) => name.clone(),
        FunctionAst::Unary { op, expr } => {
            let inner = format_function_ast(expr, variable, 4);
            match op {
                UnaryFunction::Abs => format!("|{inner}|"),
                UnaryFunction::Sqrt if is_atomic(expr) => format!("√{inner}"),
                UnaryFunction::Sqrt => format!("√({inner})"),
                UnaryFunction::Sin => format!("sin({inner})"),
                UnaryFunction::Cos => format!("cos({inner})"),
                UnaryFunction::Tan => format!("tan({inner})"),
                UnaryFunction::Ln => format!("ln({inner})"),
                UnaryFunction::Log10 => format!("log({inner})"),
                UnaryFunction::Sign => format!("sgn({inner})"),
                UnaryFunction::Round => format!("round({inner})"),
                UnaryFunction::Trunc => format!("trunc({inner})"),
            }
        }
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

fn binary_precedence(op: BinaryOp) -> (u8, bool) {
    match op {
        BinaryOp::Add | BinaryOp::Sub => (1, false),
        BinaryOp::Mul | BinaryOp::Div => (2, false),
        BinaryOp::Pow => (3, true),
    }
}

pub(crate) fn function_variable_symbol(mode: FunctionPlotMode) -> &'static str {
    match mode {
        FunctionPlotMode::Cartesian => "x",
        FunctionPlotMode::Polar => "θ",
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
        FunctionAst::Constant(_) | FunctionAst::PiAngle => false,
    }
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
