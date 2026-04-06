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
    Parsed(ParsedFunctionExpr),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
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
pub(crate) enum FunctionTerm {
    Variable,
    Constant(f64),
    Parameter(String, f64),
    UnaryX(UnaryFunction),
    Product(Box<FunctionTerm>, Box<FunctionTerm>),
    Power(Box<FunctionTerm>, Box<FunctionTerm>),
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ParsedFunctionExpr {
    pub(crate) head: FunctionTerm,
    pub(crate) tail: Vec<(BinaryOp, FunctionTerm)>,
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
        FunctionExpr::Parsed(parsed) => {
            let mut text = format_function_term(parsed.head, variable);
            for (op, term) in parsed.tail {
                text.push_str(match op {
                    BinaryOp::Add => " + ",
                    BinaryOp::Sub => " - ",
                    BinaryOp::Mul => " * ",
                    BinaryOp::Div => " / ",
                });
                text.push_str(&format_function_term(term, variable));
            }
            text
        }
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

fn format_function_term(term: FunctionTerm, variable: &str) -> String {
    match term {
        FunctionTerm::Variable => variable.to_string(),
        FunctionTerm::Constant(value) => format_number(value),
        FunctionTerm::Parameter(name, _) => name,
        FunctionTerm::UnaryX(op) => match op {
            UnaryFunction::Sin => format!("sin({variable})"),
            UnaryFunction::Cos => format!("cos({variable})"),
            UnaryFunction::Tan => format!("tan({variable})"),
            UnaryFunction::Abs => format!("|{variable}|"),
            UnaryFunction::Sqrt => format!("√{variable}"),
            UnaryFunction::Ln => format!("ln({variable})"),
            UnaryFunction::Log10 => format!("log({variable})"),
            UnaryFunction::Sign => format!("sgn({variable})"),
            UnaryFunction::Round => format!("round({variable})"),
            UnaryFunction::Trunc => format!("trunc({variable})"),
        },
        FunctionTerm::Product(left, right) => {
            format!(
                "{}*{}",
                format_function_term(*left, variable),
                format_function_term(*right, variable)
            )
        }
        FunctionTerm::Power(base, exponent) => {
            format!(
                "{}^{}",
                format_function_term(*base, variable),
                format_function_term(*exponent, variable)
            )
        }
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
        FunctionExpr::Parsed(parsed) => {
            function_term_uses_trig(&parsed.head)
                || parsed
                    .tail
                    .iter()
                    .any(|(_, term)| function_term_uses_trig(term))
        }
        _ => false,
    }
}

fn function_term_uses_trig(term: &FunctionTerm) -> bool {
    match term {
        FunctionTerm::UnaryX(UnaryFunction::Sin | UnaryFunction::Cos | UnaryFunction::Tan) => true,
        FunctionTerm::Product(left, right) => {
            function_term_uses_trig(left) || function_term_uses_trig(right)
        }
        FunctionTerm::Power(base, exponent) => {
            function_term_uses_trig(base) || function_term_uses_trig(exponent)
        }
        _ => false,
    }
}

pub(super) fn function_term_contains_symbol(term: &FunctionTerm) -> bool {
    match term {
        FunctionTerm::Variable | FunctionTerm::UnaryX(_) | FunctionTerm::Parameter(_, _) => true,
        FunctionTerm::Product(left, right) => {
            function_term_contains_symbol(left) || function_term_contains_symbol(right)
        }
        FunctionTerm::Power(base, exponent) => {
            function_term_contains_symbol(base) || function_term_contains_symbol(exponent)
        }
        FunctionTerm::Constant(_) => false,
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

pub(super) fn canonicalize_function_expr(parsed: ParsedFunctionExpr) -> FunctionExpr {
    match (&parsed.head, parsed.tail.as_slice()) {
        (FunctionTerm::Variable, []) => FunctionExpr::Identity,
        (FunctionTerm::UnaryX(UnaryFunction::Sin), []) => FunctionExpr::SinIdentity,
        (
            FunctionTerm::UnaryX(UnaryFunction::Cos),
            [(BinaryOp::Add, FunctionTerm::Constant(value))],
        ) if (*value - 5.0).abs() < f64::EPSILON => FunctionExpr::CosIdentityPlus(5.0),
        (
            FunctionTerm::UnaryX(UnaryFunction::Tan),
            [(BinaryOp::Sub, FunctionTerm::Constant(value))],
        ) if (*value - 4.0).abs() < f64::EPSILON => FunctionExpr::TanIdentityMinus(4.0),
        _ => FunctionExpr::Parsed(parsed),
    }
}
