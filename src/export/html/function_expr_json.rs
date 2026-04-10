use crate::runtime::functions::{BinaryOp, FunctionExpr, FunctionTerm, UnaryFunction};
use serde::Serialize;
use ts_rs::TS;

#[derive(Serialize, TS)]
#[serde(tag = "kind")]
pub(super) enum FunctionExprJson {
    #[serde(rename = "constant")]
    Constant { value: f64 },
    #[serde(rename = "identity")]
    Identity,
    #[serde(rename = "parsed")]
    Parsed {
        head: FunctionTermJson,
        tail: Vec<ExprTailJson>,
    },
}

impl FunctionExprJson {
    pub(super) fn from_expr(expr: &FunctionExpr) -> Self {
        match expr {
            FunctionExpr::Constant(value) => Self::Constant { value: *value },
            FunctionExpr::Identity => Self::Identity,
            FunctionExpr::SinIdentity => Self::Parsed {
                head: FunctionTermJson::UnaryX { op: "sin" },
                tail: Vec::new(),
            },
            FunctionExpr::CosIdentityPlus(offset) => Self::Parsed {
                head: FunctionTermJson::UnaryX { op: "cos" },
                tail: vec![ExprTailJson {
                    op: "add",
                    term: FunctionTermJson::Constant { value: *offset },
                }],
            },
            FunctionExpr::TanIdentityMinus(offset) => Self::Parsed {
                head: FunctionTermJson::UnaryX { op: "tan" },
                tail: vec![ExprTailJson {
                    op: "sub",
                    term: FunctionTermJson::Constant { value: *offset },
                }],
            },
            FunctionExpr::Parsed(parsed) => Self::Parsed {
                head: FunctionTermJson::from_term(&parsed.head),
                tail: parsed
                    .tail
                    .iter()
                    .map(|(op, term)| ExprTailJson {
                        op: binary_op_name(*op),
                        term: FunctionTermJson::from_term(term),
                    })
                    .collect(),
            },
        }
    }
}

#[derive(Serialize, TS)]
pub(super) struct ExprTailJson {
    op: &'static str,
    term: FunctionTermJson,
}

#[derive(Serialize, TS)]
#[serde(tag = "kind")]
pub(super) enum FunctionTermJson {
    #[serde(rename = "variable")]
    Variable,
    #[serde(rename = "constant")]
    Constant { value: f64 },
    #[serde(rename = "parameter")]
    Parameter { name: String, value: f64 },
    #[serde(rename = "unary_x")]
    UnaryX { op: &'static str },
    #[serde(rename = "product")]
    Product {
        left: Box<FunctionTermJson>,
        right: Box<FunctionTermJson>,
    },
    #[serde(rename = "power")]
    Power {
        base: Box<FunctionTermJson>,
        exponent: Box<FunctionTermJson>,
    },
}

impl FunctionTermJson {
    fn from_term(term: &FunctionTerm) -> Self {
        match term {
            FunctionTerm::Variable => Self::Variable,
            FunctionTerm::Constant(value) => Self::Constant { value: *value },
            FunctionTerm::PiAngle => Self::Constant { value: 180.0 },
            FunctionTerm::Parameter(name, value) => Self::Parameter {
                name: name.clone(),
                value: *value,
            },
            FunctionTerm::UnaryX(op) => Self::UnaryX {
                op: unary_function_name(*op),
            },
            FunctionTerm::Product(left, right) => Self::Product {
                left: Box::new(Self::from_term(left)),
                right: Box::new(Self::from_term(right)),
            },
            FunctionTerm::Power(base, exponent) => Self::Power {
                base: Box::new(Self::from_term(base)),
                exponent: Box::new(Self::from_term(exponent)),
            },
        }
    }
}

fn binary_op_name(op: BinaryOp) -> &'static str {
    match op {
        BinaryOp::Add => "add",
        BinaryOp::Sub => "sub",
        BinaryOp::Mul => "mul",
        BinaryOp::Div => "div",
    }
}

fn unary_function_name(op: UnaryFunction) -> &'static str {
    match op {
        UnaryFunction::Sin => "sin",
        UnaryFunction::Cos => "cos",
        UnaryFunction::Tan => "tan",
        UnaryFunction::Abs => "abs",
        UnaryFunction::Sqrt => "sqrt",
        UnaryFunction::Ln => "ln",
        UnaryFunction::Log10 => "log10",
        UnaryFunction::Sign => "sign",
        UnaryFunction::Round => "round",
        UnaryFunction::Trunc => "trunc",
    }
}
