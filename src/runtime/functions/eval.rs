use std::collections::BTreeMap;

use crate::format::PointRecord;

use super::expr::{
    BinaryOp, FunctionExpr, FunctionPlotDescriptor, FunctionPlotMode, FunctionTerm,
    ParsedFunctionExpr, UnaryFunction,
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
            FunctionExpr::Parsed(parsed) => evaluate_function_expr(parsed, x),
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

fn evaluate_function_expr(expr: &ParsedFunctionExpr, x: f64) -> Option<f64> {
    let mut value = evaluate_function_term(expr.head.clone(), x)?;
    for (op, term) in &expr.tail {
        let rhs = evaluate_function_term(term.clone(), x)?;
        value = match op {
            BinaryOp::Add => value + rhs,
            BinaryOp::Sub => value - rhs,
            BinaryOp::Div => (rhs.abs() >= 1e-9).then_some(value / rhs)?,
        };
    }
    value.is_finite().then_some(value)
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
        FunctionExpr::Parsed(parsed) => evaluate_parsed_with_parameters(parsed, x, parameters),
    }
}

fn evaluate_parsed_with_parameters(
    expr: &ParsedFunctionExpr,
    x: f64,
    parameters: &BTreeMap<String, f64>,
) -> Option<f64> {
    let mut value = evaluate_function_term_with_parameters(expr.head.clone(), x, parameters)?;
    for (op, term) in &expr.tail {
        let rhs = evaluate_function_term_with_parameters(term.clone(), x, parameters)?;
        value = match op {
            BinaryOp::Add => value + rhs,
            BinaryOp::Sub => value - rhs,
            BinaryOp::Div => (rhs.abs() >= 1e-9).then_some(value / rhs)?,
        };
    }
    value.is_finite().then_some(value)
}

fn evaluate_function_term_with_parameters(
    term: FunctionTerm,
    x: f64,
    parameters: &BTreeMap<String, f64>,
) -> Option<f64> {
    match term {
        FunctionTerm::Variable => Some(x),
        FunctionTerm::Constant(value) => Some(value),
        FunctionTerm::PiAngle => Some(180.0),
        FunctionTerm::Parameter(name, value) => Some(*parameters.get(&name).unwrap_or(&value)),
        FunctionTerm::UnaryX(op) => match op {
            UnaryFunction::Sin => Some(x.sin()),
            UnaryFunction::Cos => Some(x.cos()),
            UnaryFunction::Tan => {
                let y = x.tan();
                (y.is_finite() && x.cos().abs() >= 0.04 && y.abs() <= 5.0).then_some(y)
            }
            UnaryFunction::Abs => Some(x.abs()),
            UnaryFunction::Sqrt => (x >= 0.0).then(|| x.sqrt()),
            UnaryFunction::Ln => (x > 0.0).then(|| x.ln()),
            UnaryFunction::Log10 => (x > 0.0).then(|| x.log10()),
            UnaryFunction::Sign => Some(if x > 0.0 {
                1.0
            } else if x < 0.0 {
                -1.0
            } else {
                0.0
            }),
            UnaryFunction::Round => Some(x.round()),
            UnaryFunction::Trunc => Some(x.trunc()),
        },
        FunctionTerm::Product(left, right) => Some(
            evaluate_function_term_with_parameters(*left, x, parameters)?
                * evaluate_function_term_with_parameters(*right, x, parameters)?,
        ),
        FunctionTerm::Power(base, exponent) => {
            let base = evaluate_function_term_with_parameters(*base, x, parameters)?;
            let exponent = evaluate_function_term_with_parameters(*exponent, x, parameters)?;
            let value = base.powf(exponent);
            value.is_finite().then_some(value)
        }
    }
}

fn evaluate_function_term(term: FunctionTerm, x: f64) -> Option<f64> {
    match term {
        FunctionTerm::Variable => Some(x),
        FunctionTerm::Constant(value) => Some(value),
        FunctionTerm::PiAngle => Some(180.0),
        FunctionTerm::Parameter(_, value) => Some(value),
        FunctionTerm::UnaryX(op) => match op {
            UnaryFunction::Sin => Some(x.sin()),
            UnaryFunction::Cos => Some(x.cos()),
            UnaryFunction::Tan => {
                let y = x.tan();
                (y.is_finite() && x.cos().abs() >= 0.04 && y.abs() <= 5.0).then_some(y)
            }
            UnaryFunction::Abs => Some(x.abs()),
            UnaryFunction::Sqrt => (x >= 0.0).then(|| x.sqrt()),
            UnaryFunction::Ln => (x > 0.0).then(|| x.ln()),
            UnaryFunction::Log10 => (x > 0.0).then(|| x.log10()),
            UnaryFunction::Sign => Some(if x > 0.0 {
                1.0
            } else if x < 0.0 {
                -1.0
            } else {
                0.0
            }),
            UnaryFunction::Round => Some(x.round()),
            UnaryFunction::Trunc => Some(x.trunc()),
        },
        FunctionTerm::Product(left, right) => {
            Some(evaluate_function_term(*left, x)? * evaluate_function_term(*right, x)?)
        }
        FunctionTerm::Power(base, exponent) => {
            let value =
                evaluate_function_term(*base, x)?.powf(evaluate_function_term(*exponent, x)?);
            value.is_finite().then_some(value)
        }
    }
}
