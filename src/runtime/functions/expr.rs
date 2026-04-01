use crate::runtime::geometry::format_number;

#[derive(Debug, Clone)]
pub(crate) struct FunctionPlotDescriptor {
    pub(crate) x_min: f64,
    pub(crate) x_max: f64,
    pub(crate) sample_count: usize,
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
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ParsedFunctionExpr {
    pub(crate) head: FunctionTerm,
    pub(crate) tail: Vec<(BinaryOp, FunctionTerm)>,
}

pub(crate) fn function_expr_label(expr: FunctionExpr) -> String {
    match expr {
        FunctionExpr::Constant(value) => format_number(value),
        FunctionExpr::Identity => "x".to_string(),
        FunctionExpr::SinIdentity => "sin(x)".to_string(),
        FunctionExpr::CosIdentityPlus(offset) => format!("cos(x) + {}", format_number(offset)),
        FunctionExpr::TanIdentityMinus(offset) => format!("tan(x) - {}", format_number(offset)),
        FunctionExpr::Parsed(parsed) => {
            let mut text = format_function_term(parsed.head);
            for (op, term) in parsed.tail {
                text.push_str(match op {
                    BinaryOp::Add => " + ",
                    BinaryOp::Sub => " - ",
                    BinaryOp::Mul => " * ",
                    BinaryOp::Div => " / ",
                });
                text.push_str(&format_function_term(term));
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

fn format_function_term(term: FunctionTerm) -> String {
    match term {
        FunctionTerm::Variable => "x".to_string(),
        FunctionTerm::Constant(value) => format_number(value),
        FunctionTerm::Parameter(name, _) => name,
        FunctionTerm::UnaryX(op) => match op {
            UnaryFunction::Sin => "sin(x)".to_string(),
            UnaryFunction::Cos => "cos(x)".to_string(),
            UnaryFunction::Tan => "tan(x)".to_string(),
            UnaryFunction::Abs => "|x|".to_string(),
            UnaryFunction::Sqrt => "√x".to_string(),
            UnaryFunction::Ln => "ln(x)".to_string(),
            UnaryFunction::Log10 => "log(x)".to_string(),
            UnaryFunction::Sign => "sgn(x)".to_string(),
            UnaryFunction::Round => "round(x)".to_string(),
            UnaryFunction::Trunc => "trunc(x)".to_string(),
        },
        FunctionTerm::Product(left, right) => {
            format!(
                "{}*{}",
                format_function_term(*left),
                format_function_term(*right)
            )
        }
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
        _ => false,
    }
}

pub(super) fn function_term_contains_symbol(term: &FunctionTerm) -> bool {
    match term {
        FunctionTerm::Variable | FunctionTerm::UnaryX(_) | FunctionTerm::Parameter(_, _) => true,
        FunctionTerm::Product(left, right) => {
            function_term_contains_symbol(left) || function_term_contains_symbol(right)
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
