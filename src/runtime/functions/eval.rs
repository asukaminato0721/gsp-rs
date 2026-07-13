use std::collections::BTreeMap;

use crate::format::PointRecord;

use super::expr::{FunctionExpr, FunctionPlotDescriptor, FunctionPlotMode};

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
        let y = gsp_runtime_core::evaluate_expr(expr, x, &BTreeMap::new());
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
    gsp_runtime_core::evaluate_expr(expr, x, parameters)
}
