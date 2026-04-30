use super::decode::{decode_label_name, find_indexed_path};
use crate::format::{GspFile, ObjectGroup};
use crate::runtime::functions::{
    BinaryOp, FunctionAst, FunctionExpr, UnaryFunction, try_decode_function_expr,
};

pub(super) fn decode_iteration_depth_expr(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
) -> Option<FunctionExpr> {
    let path = find_indexed_path(file, group)?;
    let source_group = path
        .refs
        .first()
        .and_then(|ordinal| groups.get(ordinal.checked_sub(1)?))?;
    if source_group.header.kind() == crate::format::GroupKind::RatioValue {
        let name = decode_label_name(file, source_group)?;
        return Some(FunctionExpr::Parsed(FunctionAst::Unary {
            op: UnaryFunction::Trunc,
            expr: Box::new(FunctionAst::Binary {
                lhs: Box::new(FunctionAst::Parameter(name, 0.0)),
                op: BinaryOp::Sub,
                rhs: Box::new(FunctionAst::Constant(0.5)),
            }),
        }));
    }
    if source_group.header.kind() == crate::format::GroupKind::FunctionExpr {
        let source_expr = decode_iteration_depth_expr(file, groups, source_group)?;
        if decoded_expr_is_placeholder_minus_one(file, groups, group) {
            return Some(FunctionExpr::Parsed(FunctionAst::Binary {
                lhs: Box::new(function_expr_ast(source_expr)),
                op: BinaryOp::Sub,
                rhs: Box::new(FunctionAst::Constant(1.0)),
            }));
        }
    }
    try_decode_function_expr(file, groups, group).ok()
}

fn decoded_expr_is_placeholder_minus_one(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
) -> bool {
    matches!(
        try_decode_function_expr(file, groups, group).ok(),
        Some(FunctionExpr::Parsed(FunctionAst::Binary { lhs, op: BinaryOp::Sub, rhs }))
            if matches!(*lhs, FunctionAst::Parameter(_, _))
                && matches!(*rhs, FunctionAst::Constant(value) if (value - 1.0).abs() < 1e-9)
    )
}

fn function_expr_ast(expr: FunctionExpr) -> FunctionAst {
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
