use crate::runtime::functions::{BinaryOp, FunctionAst, FunctionExpr, UnaryFunction};
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
    Parsed { expr: FunctionAstJson },
}

impl FunctionExprJson {
    pub(super) fn from_expr(expr: &FunctionExpr) -> Self {
        match expr {
            FunctionExpr::Constant(value) => Self::Constant { value: *value },
            FunctionExpr::Identity => Self::Identity,
            FunctionExpr::SinIdentity => Self::Parsed {
                expr: FunctionAstJson::Unary {
                    op: unary_function_name(UnaryFunction::Sin),
                    expr: Box::new(FunctionAstJson::Variable),
                },
            },
            FunctionExpr::CosIdentityPlus(offset) => Self::Parsed {
                expr: FunctionAstJson::Binary {
                    lhs: Box::new(FunctionAstJson::Unary {
                        op: unary_function_name(UnaryFunction::Cos),
                        expr: Box::new(FunctionAstJson::Variable),
                    }),
                    op: binary_op_name(BinaryOp::Add),
                    rhs: Box::new(FunctionAstJson::Constant { value: *offset }),
                },
            },
            FunctionExpr::TanIdentityMinus(offset) => Self::Parsed {
                expr: FunctionAstJson::Binary {
                    lhs: Box::new(FunctionAstJson::Unary {
                        op: unary_function_name(UnaryFunction::Tan),
                        expr: Box::new(FunctionAstJson::Variable),
                    }),
                    op: binary_op_name(BinaryOp::Sub),
                    rhs: Box::new(FunctionAstJson::Constant { value: *offset }),
                },
            },
            FunctionExpr::Parsed(ast) => Self::Parsed {
                expr: FunctionAstJson::from_ast(ast),
            },
        }
    }
}

#[derive(Serialize, TS)]
#[serde(tag = "kind")]
pub(super) enum FunctionAstJson {
    #[serde(rename = "variable")]
    Variable,
    #[serde(rename = "constant")]
    Constant { value: f64 },
    #[serde(rename = "parameter")]
    Parameter { name: String, value: f64 },
    #[serde(rename = "pi-angle")]
    PiAngle,
    #[serde(rename = "unary")]
    Unary {
        op: &'static str,
        expr: Box<FunctionAstJson>,
    },
    #[serde(rename = "binary")]
    Binary {
        lhs: Box<FunctionAstJson>,
        op: &'static str,
        rhs: Box<FunctionAstJson>,
    },
}

impl FunctionAstJson {
    fn from_ast(ast: &FunctionAst) -> Self {
        match ast {
            FunctionAst::Variable => Self::Variable,
            FunctionAst::Constant(value) => Self::Constant { value: *value },
            FunctionAst::PiAngle => Self::PiAngle,
            FunctionAst::Parameter(name, value) => Self::Parameter {
                name: name.clone(),
                value: *value,
            },
            FunctionAst::Unary { op, expr } => Self::Unary {
                op: unary_function_name(*op),
                expr: Box::new(Self::from_ast(expr)),
            },
            FunctionAst::Binary { lhs, op, rhs } => Self::Binary {
                lhs: Box::new(Self::from_ast(lhs)),
                op: binary_op_name(*op),
                rhs: Box::new(Self::from_ast(rhs)),
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
        BinaryOp::Pow => "pow",
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
