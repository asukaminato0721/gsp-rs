use std::collections::BTreeMap;

use crate::format::PointRecord;

use super::expr::{
    BinaryOp, FunctionAst, FunctionExpr, FunctionPlotDescriptor, FunctionPlotMode, UnaryFunction,
};

pub(crate) fn sample_function_points(
    expr: &FunctionExpr,
    descriptor: &FunctionPlotDescriptor,
) -> Vec<Vec<PointRecord>> {
    let mut segments = Vec::<Vec<PointRecord>>::new();
    let mut points = Vec::with_capacity(descriptor.sample_count);
    let span = descriptor.x_max - descriptor.x_min;
    let last = descriptor.sample_count.saturating_sub(1).max(1) as f64;
    for index in 0..descriptor.sample_count {
        let t = index as f64 / last;
        let x = descriptor.x_min + span * t;
        let y = match expr {
            FunctionExpr::Constant(value) => Some(*value),
            FunctionExpr::Identity => Some(x),
            FunctionExpr::SinIdentity => Some(x.sin()),
            FunctionExpr::CosIdentityPlus(offset) => Some(x.cos() + offset),
            FunctionExpr::TanIdentityMinus(offset) => {
                let y = x.tan() - offset;
                if !y.is_finite() || x.cos().abs() < 0.04 || y.abs() > 5.0 {
                    None
                } else {
                    Some(y)
                }
            }
            FunctionExpr::Parsed(ast) => evaluate_ast(ast, x, &BTreeMap::new()),
        };
        if let Some(y) = y {
            let point = match descriptor.mode {
                FunctionPlotMode::Cartesian => PointRecord { x, y },
                FunctionPlotMode::Polar => PointRecord {
                    x: y * x.cos(),
                    y: y * x.sin(),
                },
            };
            points.push(point);
        } else if points.len() >= 2 {
            segments.push(std::mem::take(&mut points));
        } else {
            points.clear();
        }
    }
    if points.len() >= 2 {
        segments.push(points);
    }
    segments
}

pub(crate) fn evaluate_expr_with_parameters(
    expr: &FunctionExpr,
    x: f64,
    parameters: &BTreeMap<String, f64>,
) -> Option<f64> {
    match expr {
        FunctionExpr::Constant(value) => Some(*value),
        FunctionExpr::Identity => Some(x),
        FunctionExpr::SinIdentity => Some(x.sin()),
        FunctionExpr::CosIdentityPlus(offset) => Some(x.cos() + offset),
        FunctionExpr::TanIdentityMinus(offset) => {
            let y = x.tan() - offset;
            (y.is_finite() && x.cos().abs() >= 0.04 && y.abs() <= 5.0).then_some(y)
        }
        FunctionExpr::Parsed(ast) => evaluate_ast(ast, x, parameters),
    }
}

fn evaluate_ast(expr: &FunctionAst, x: f64, parameters: &BTreeMap<String, f64>) -> Option<f64> {
    let value = match expr {
        FunctionAst::Variable => x,
        FunctionAst::Constant(value) => *value,
        FunctionAst::PiAngle => 180.0,
        FunctionAst::Parameter(name, value) => *parameters.get(name).unwrap_or(value),
        FunctionAst::Unary { op, expr } => {
            let value = evaluate_ast(expr, x, parameters)?;
            match op {
                UnaryFunction::Sin => value.sin(),
                UnaryFunction::Cos => value.cos(),
                UnaryFunction::Tan => {
                    let y = value.tan();
                    if !y.is_finite() || value.cos().abs() < 0.04 || y.abs() > 5.0 {
                        return None;
                    }
                    y
                }
                UnaryFunction::Abs => value.abs(),
                UnaryFunction::Sqrt => (value >= 0.0).then(|| value.sqrt())?,
                UnaryFunction::Ln => (value > 0.0).then(|| value.ln())?,
                UnaryFunction::Log10 => (value > 0.0).then(|| value.log10())?,
                UnaryFunction::Sign => {
                    if value > 0.0 {
                        1.0
                    } else if value < 0.0 {
                        -1.0
                    } else {
                        0.0
                    }
                }
                UnaryFunction::Round => value.round(),
                UnaryFunction::Trunc => value.trunc(),
            }
        }
        FunctionAst::Binary { lhs, op, rhs } => {
            let lhs = evaluate_ast(lhs, x, parameters)?;
            let rhs = evaluate_ast(rhs, x, parameters)?;
            match op {
                BinaryOp::Add => lhs + rhs,
                BinaryOp::Sub => lhs - rhs,
                BinaryOp::Mul => lhs * rhs,
                BinaryOp::Div => (rhs.abs() >= 1e-9).then_some(lhs / rhs)?,
                BinaryOp::Pow => lhs.powf(rhs),
            }
        }
    };
    value.is_finite().then_some(value)
}
