use std::collections::BTreeMap;

use crate::format::{GspFile, ObjectGroup, PointRecord};
use crate::runtime::extract::points::is_standalone_function_definition_group;
use crate::runtime::extract::{find_indexed_path, try_decode_parameter_control_value_for_group};
use crate::runtime::geometry::{GraphTransform, has_distinct_points, to_raw_from_world};
use crate::runtime::payload_consts::RECORD_FUNCTION_PLOT_DESCRIPTOR;
use crate::runtime::scene::{LineBinding, LineShape};

use super::decode::{
    evaluate_function_group_with_overrides, try_decode_function_expr,
    try_decode_function_plot_descriptor, try_decode_plot_component_expr,
    try_decode_standalone_function_expr,
};
use super::eval::{evaluate_expr_with_parameters, sample_function_points};
use super::expr::{
    FunctionExpr, FunctionPlotDescriptor, FunctionPlotMode, common_period, function_expr_period,
};
use super::scene::collect_parameter_bindings;

pub(crate) fn collect_function_plots(
    file: &GspFile,
    groups: &[ObjectGroup],
    graph: &Option<GraphTransform>,
) -> Vec<LineShape> {
    let Some(transform) = graph.as_ref() else {
        return Vec::new();
    };

    let mut plots = Vec::new();
    for group in groups.iter().filter(|group| {
        matches!(
            group.header.kind(),
            crate::format::GroupKind::FunctionPlot
                | crate::format::GroupKind::ParametricFunctionPlot
        )
    }) {
        let binding = parametric_curve_binding(file, groups, group);
        let Some(segments) = sample_plot_segments(file, groups, group) else {
            continue;
        };
        let mut pushed_plot = false;
        for mut points in segments {
            if !has_distinct_points(&points) {
                continue;
            }

            for point in &mut points {
                *point = to_raw_from_world(point, transform);
            }

            pushed_plot = true;
            plots.push(LineShape {
                points,
                color: crate::runtime::geometry::color_from_style(group.header.style_b),
                dashed: false,
                visible: !group.header.is_hidden(),
                binding: binding.clone(),
                ..Default::default()
            });
        }
        if !pushed_plot && (group.header.kind()) == crate::format::GroupKind::FunctionPlot {
            plots.push(LineShape {
                points: Vec::new(),
                color: crate::runtime::geometry::color_from_style(group.header.style_b),
                dashed: false,
                visible: !group.header.is_hidden(),
                binding,
                ..Default::default()
            });
        }
    }

    plots
}

pub(crate) fn sample_plot_segments(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
) -> Option<Vec<Vec<PointRecord>>> {
    let path = find_indexed_path(file, group)?;
    let descriptor_record = group
        .records
        .iter()
        .find(|record| record.record_type == RECORD_FUNCTION_PLOT_DESCRIPTOR)?;
    let descriptor =
        try_decode_function_plot_descriptor(descriptor_record.payload(&file.data)).ok()?;

    match group.header.kind() {
        crate::format::GroupKind::FunctionPlot => {
            let definition_group = groups.get(path.refs.first()?.checked_sub(1)?)?;
            let expr = try_decode_function_expr(file, groups, definition_group).ok()?;
            Some(sample_function_points(&expr, &descriptor))
        }
        crate::format::GroupKind::ParametricFunctionPlot => {
            if path.refs.len() < 3 {
                return None;
            }
            let x_group = groups.get(path.refs[0].checked_sub(1)?)?;
            let y_group = groups.get(path.refs[1].checked_sub(1)?)?;
            sample_parametric_plot_segments(file, groups, x_group, y_group, &descriptor)
        }
        _ => None,
    }
}

fn sample_parametric_plot_segments(
    file: &GspFile,
    groups: &[ObjectGroup],
    x_group: &ObjectGroup,
    y_group: &ObjectGroup,
    descriptor: &crate::runtime::functions::FunctionPlotDescriptor,
) -> Option<Vec<Vec<PointRecord>>> {
    if let (Ok(x_expr), Ok(y_expr)) = (
        decode_parametric_component_expr(file, groups, x_group),
        decode_parametric_component_expr(file, groups, y_group),
    ) {
        let descriptor = reduce_periodic_parametric_descriptor(&x_expr, &y_expr, descriptor);
        return sample_parametric_expr_segments(&x_expr, &y_expr, &descriptor);
    }

    let mut parameter_names = collect_parameter_bindings(file, groups, x_group)
        .into_values()
        .map(|binding| binding.name)
        .collect::<Vec<_>>();
    parameter_names.extend(
        collect_parameter_bindings(file, groups, y_group)
            .into_values()
            .map(|binding| binding.name),
    );
    parameter_names.sort();
    parameter_names.dedup();

    let mut segments = Vec::<Vec<PointRecord>>::new();
    let mut points = Vec::with_capacity(descriptor.sample_count);
    let span = descriptor.x_max - descriptor.x_min;
    let last = descriptor.sample_count.saturating_sub(1).max(1) as f64;
    for index in 0..descriptor.sample_count {
        let t = index as f64 / last;
        let parameter = descriptor.x_min + span * t;
        let overrides = parameter_names
            .iter()
            .map(|name| (name.clone(), parameter))
            .collect::<std::collections::BTreeMap<_, _>>();
        let x = evaluate_function_group_with_overrides(file, groups, x_group, &overrides)
            .or_else(|| try_decode_parameter_control_value_for_group(file, groups, x_group).ok())?;
        let y = evaluate_function_group_with_overrides(file, groups, y_group, &overrides)
            .or_else(|| try_decode_parameter_control_value_for_group(file, groups, y_group).ok())?;
        let point = PointRecord { x, y };
        if point.x.is_finite() && point.y.is_finite() {
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
    (!segments.is_empty()).then_some(segments)
}

fn sample_parametric_expr_segments(
    x_expr: &FunctionExpr,
    y_expr: &FunctionExpr,
    descriptor: &crate::runtime::functions::FunctionPlotDescriptor,
) -> Option<Vec<Vec<PointRecord>>> {
    let mut segments = Vec::<Vec<PointRecord>>::new();
    let mut points = Vec::with_capacity(descriptor.sample_count);
    let span = descriptor.x_max - descriptor.x_min;
    let last = descriptor.sample_count.saturating_sub(1).max(1) as f64;
    let parameters = BTreeMap::new();
    for index in 0..descriptor.sample_count {
        let t = index as f64 / last;
        let parameter = descriptor.x_min + span * t;
        let x = evaluate_expr_with_parameters(x_expr, parameter, &parameters)?;
        let y = evaluate_expr_with_parameters(y_expr, parameter, &parameters)?;
        let point = PointRecord { x, y };
        if point.x.is_finite() && point.y.is_finite() {
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
    (!segments.is_empty()).then_some(segments)
}

fn parametric_curve_binding(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
) -> Option<LineBinding> {
    if (group.header.kind()) != crate::format::GroupKind::ParametricFunctionPlot {
        return None;
    }
    let path = find_indexed_path(file, group)?;
    if path.refs.len() < 3 {
        return None;
    }
    let x_group = groups.get(path.refs[0].checked_sub(1)?)?;
    let y_group = groups.get(path.refs[1].checked_sub(1)?)?;
    let descriptor_record = group
        .records
        .iter()
        .find(|record| record.record_type == RECORD_FUNCTION_PLOT_DESCRIPTOR)?;
    let descriptor =
        try_decode_function_plot_descriptor(descriptor_record.payload(&file.data)).ok()?;
    let x_expr = decode_parametric_component_expr(file, groups, x_group).ok()?;
    let y_expr = decode_parametric_component_expr(file, groups, y_group).ok()?;
    let descriptor = reduce_periodic_parametric_descriptor(&x_expr, &y_expr, &descriptor);
    Some(LineBinding::ParametricCurve {
        x_expr,
        y_expr,
        x_min: descriptor.x_min,
        x_max: descriptor.x_max,
        sample_count: descriptor.sample_count,
    })
}

fn decode_parametric_component_expr(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
) -> Result<FunctionExpr, super::decode::FunctionExprParseError> {
    if is_standalone_function_definition_group(file, groups, group) {
        try_decode_standalone_function_expr(file, groups, group)
    } else {
        try_decode_plot_component_expr(file, groups, group)
    }
}

fn reduce_periodic_parametric_descriptor(
    x_expr: &FunctionExpr,
    y_expr: &FunctionExpr,
    descriptor: &FunctionPlotDescriptor,
) -> FunctionPlotDescriptor {
    let Some(x_period) = function_expr_period(x_expr) else {
        return descriptor.clone();
    };
    let Some(y_period) = function_expr_period(y_expr) else {
        return descriptor.clone();
    };
    let Some(common_period) = common_period(x_period, y_period) else {
        return descriptor.clone();
    };
    let period = common_period.as_f64();
    if !period.is_finite() || period <= 1e-9 {
        return descriptor.clone();
    }
    let span = descriptor.x_max - descriptor.x_min;
    if !span.is_finite() || span <= period * 1.01 {
        return descriptor.clone();
    }
    FunctionPlotDescriptor {
        x_min: descriptor.x_min,
        x_max: descriptor.x_min + period,
        sample_count: descriptor.sample_count,
        mode: descriptor.mode,
    }
}

pub(crate) fn collect_function_plot_domain(
    file: &GspFile,
    groups: &[ObjectGroup],
) -> Option<(f64, f64)> {
    let mut min_x = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut found = false;
    for group in groups
        .iter()
        .filter(|group| (group.header.kind()) == crate::format::GroupKind::FunctionPlot)
    {
        let Some(descriptor_record) = group
            .records
            .iter()
            .find(|record| record.record_type == RECORD_FUNCTION_PLOT_DESCRIPTOR)
        else {
            continue;
        };
        let Some(descriptor) =
            try_decode_function_plot_descriptor(descriptor_record.payload(&file.data)).ok()
        else {
            continue;
        };
        if descriptor.mode != FunctionPlotMode::Cartesian {
            continue;
        }
        min_x = min_x.min(descriptor.x_min);
        max_x = max_x.max(descriptor.x_max);
        found = true;
    }
    found.then_some((min_x, max_x))
}
