use std::collections::BTreeMap;

use crate::format::{GspFile, ObjectGroup, PointRecord, read_f64, read_u16, read_u32};

use super::extract::find_indexed_path;
use super::geometry::{
    Bounds, GraphTransform, format_number, has_distinct_points, include_line_bounds,
    to_raw_from_world,
};
use super::scene::{
    LineShape, SceneFunction, SceneParameter, ScenePoint, ScenePointConstraint, TextLabel,
};

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

#[derive(Debug, Clone, PartialEq)]
struct ParameterBinding {
    name: String,
    value: f64,
}

pub(super) fn collect_function_plots(
    file: &GspFile,
    groups: &[ObjectGroup],
    graph: &Option<GraphTransform>,
) -> Vec<LineShape> {
    let Some(transform) = graph.as_ref() else {
        return Vec::new();
    };

    let mut plots = Vec::new();
    for group in groups
        .iter()
        .filter(|group| (group.header.class_id & 0xffff) == 72)
    {
        let Some(path) = find_indexed_path(file, group) else {
            continue;
        };
        if path.refs.len() < 2 {
            continue;
        }

        let Some(definition_index) = path.refs[0].checked_sub(1) else {
            continue;
        };
        let Some(definition_group) = groups.get(definition_index) else {
            continue;
        };
        let Some(descriptor) = group
            .records
            .iter()
            .find(|record| record.record_type == 0x0902)
            .and_then(|record| decode_function_plot_descriptor(record.payload(&file.data)))
        else {
            continue;
        };
        let Some(expr) = decode_function_expr(file, groups, definition_group) else {
            continue;
        };

        for mut points in sample_function_points(&expr, &descriptor) {
            if !has_distinct_points(&points) {
                continue;
            }

            for point in &mut points {
                *point = to_raw_from_world(point, transform);
            }

            plots.push(LineShape {
                points,
                color: super::geometry::color_from_style(group.header.style_b),
                dashed: false,
            });
        }
    }

    plots
}

pub(super) fn collect_function_plot_domain(
    file: &GspFile,
    groups: &[ObjectGroup],
) -> Option<(f64, f64)> {
    let mut min_x = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut found = false;
    for group in groups
        .iter()
        .filter(|group| (group.header.class_id & 0xffff) == 72)
    {
        let Some(descriptor) = group
            .records
            .iter()
            .find(|record| record.record_type == 0x0902)
            .and_then(|record| decode_function_plot_descriptor(record.payload(&file.data)))
        else {
            continue;
        };
        min_x = min_x.min(descriptor.x_min);
        max_x = max_x.max(descriptor.x_max);
        found = true;
    }
    found.then_some((min_x, max_x))
}

pub(super) fn synthesize_function_axes(
    function_plots: &[LineShape],
    domain: Option<(f64, f64)>,
    viewport: Option<Bounds>,
    graph: &Option<GraphTransform>,
) -> Vec<LineShape> {
    let Some(mut world_bounds) =
        viewport.or_else(|| bounds_from_function_plots(function_plots, domain, graph))
    else {
        return Vec::new();
    };
    if (world_bounds.max_y - world_bounds.min_y).abs() < 1e-6 {
        world_bounds.min_y -= 1.0;
        world_bounds.max_y += 1.0;
    }
    if (world_bounds.max_x - world_bounds.min_x).abs() < 1e-6 {
        world_bounds.min_x -= 1.0;
        world_bounds.max_x += 1.0;
    }

    let mut axes = Vec::new();
    if world_bounds.min_x <= 0.0 && 0.0 <= world_bounds.max_x {
        axes.push(LineShape {
            points: vec![
                PointRecord {
                    x: 0.0,
                    y: world_bounds.min_y,
                },
                PointRecord {
                    x: 0.0,
                    y: world_bounds.max_y,
                },
            ]
            .into_iter()
            .map(|point| {
                to_raw_from_world(
                    &point,
                    graph
                        .as_ref()
                        .expect("graph transform required for synthetic axes"),
                )
            })
            .collect(),
            color: [192, 192, 192, 255],
            dashed: false,
        });
    }
    if world_bounds.min_y <= 0.0 && 0.0 <= world_bounds.max_y {
        axes.push(LineShape {
            points: vec![
                PointRecord {
                    x: world_bounds.min_x,
                    y: 0.0,
                },
                PointRecord {
                    x: world_bounds.max_x,
                    y: 0.0,
                },
            ]
            .into_iter()
            .map(|point| {
                to_raw_from_world(
                    &point,
                    graph
                        .as_ref()
                        .expect("graph transform required for synthetic axes"),
                )
            })
            .collect(),
            color: [192, 192, 192, 255],
            dashed: false,
        });
    }

    axes
}

pub(super) fn synthesize_function_labels(
    file: &GspFile,
    groups: &[ObjectGroup],
    function_plots: &[LineShape],
    viewport: Option<Bounds>,
    graph: &Option<GraphTransform>,
) -> Vec<TextLabel> {
    let Some(bounds) = viewport.or_else(|| {
        bounds_from_function_plots(
            function_plots,
            collect_function_plot_domain(file, groups),
            graph,
        )
    }) else {
        return Vec::new();
    };
    let Some(transform) = graph.as_ref() else {
        return Vec::new();
    };

    let parameter_entries = groups
        .iter()
        .filter(|group| (group.header.class_id & 0xffff) == 72)
        .filter_map(|group| {
            let path = find_indexed_path(file, group)?;
            let definition_group = groups.get(path.refs.first()?.checked_sub(1)?)?;
            Some(collect_parameter_bindings(file, groups, definition_group))
        })
        .fold(BTreeMap::<String, f64>::new(), |mut acc, bindings| {
            for binding in bindings.into_values() {
                acc.entry(binding.name).or_insert(binding.value);
            }
            acc
        })
        .into_iter()
        .collect::<Vec<_>>();

    let base_entries = groups
        .iter()
        .filter(|group| (group.header.class_id & 0xffff) == 72)
        .filter_map(|group| {
            let path = find_indexed_path(file, group)?;
            let definition_ordinal = *path.refs.first()?;
            let definition_group = groups.get(definition_ordinal.checked_sub(1)?)?;
            let expr = decode_function_expr(file, groups, definition_group)?;
            Some((definition_ordinal, expr))
        })
        .collect::<Vec<_>>();

    let total = base_entries.len();
    let mut labels = parameter_entries
        .iter()
        .enumerate()
        .map(|(index, (name, value))| {
            let span_x = (bounds.max_x - bounds.min_x).max(1.0);
            let span_y = (bounds.max_y - bounds.min_y).max(1.0);
            let world_anchor = PointRecord {
                x: bounds.min_x + span_x * 0.18,
                y: bounds.max_y - span_y * (0.08 + 0.11 * index as f64),
            };
            TextLabel {
                anchor: to_raw_from_world(&world_anchor, transform),
                text: format!("{name} = {:.2}", value),
                color: [30, 30, 30, 255],
                binding: None,
            }
        })
        .collect::<Vec<_>>();
    let parameter_count = labels.len();
    labels.extend(
        base_entries
            .into_iter()
            .enumerate()
            .map(|(index, (_, expr))| {
                let span_x = (bounds.max_x - bounds.min_x).max(1.0);
                let span_y = (bounds.max_y - bounds.min_y).max(1.0);
                let world_anchor = PointRecord {
                    x: bounds.min_x + span_x * 0.18,
                    y: bounds.max_y - span_y * (0.16 + 0.11 * (index + parameter_count) as f64),
                };
                TextLabel {
                    anchor: to_raw_from_world(&world_anchor, transform),
                    text: format!(
                        "{}(x) = {}",
                        function_name_for_index(index, total, &expr),
                        function_expr_label(expr)
                    ),
                    color: [30, 30, 30, 255],
                    binding: None,
                }
            }),
    );

    let derivative_entries = groups
        .iter()
        .filter(|group| (group.header.class_id & 0xffff) == 78)
        .filter_map(|group| {
            let path = find_indexed_path(file, group)?;
            let base_definition_ordinal = *path.refs.first()?;
            let base_index = groups
                .iter()
                .filter(|candidate| (candidate.header.class_id & 0xffff) == 72)
                .filter_map(|candidate| {
                    find_indexed_path(file, candidate)
                        .and_then(|candidate_path| candidate_path.refs.first().copied())
                })
                .position(|ordinal| ordinal == base_definition_ordinal)?;
            let expr = decode_function_expr(file, groups, group)?;
            Some((base_index, expr))
        })
        .collect::<Vec<_>>();

    let span_x = (bounds.max_x - bounds.min_x).max(1.0);
    let span_y = (bounds.max_y - bounds.min_y).max(1.0);
    let base_count = labels.len();
    labels.extend(derivative_entries.into_iter().enumerate().map(
        |(offset, (base_index, expr))| {
            let label_index = base_count + offset;
            let world_anchor = PointRecord {
                x: bounds.min_x + span_x * 0.18,
                y: bounds.max_y - span_y * (0.16 + 0.11 * label_index as f64),
            };
            TextLabel {
                anchor: to_raw_from_world(&world_anchor, transform),
                text: format!(
                    "{}'(x) = {}",
                    function_name_for_index(base_index, total.max(1), &expr),
                    function_expr_label(expr)
                ),
                color: [30, 30, 30, 255],
                binding: None,
            }
        },
    ));

    labels
}

pub(super) fn collect_scene_parameters(
    file: &GspFile,
    groups: &[ObjectGroup],
    labels: &[TextLabel],
) -> Vec<SceneParameter> {
    groups
        .iter()
        .filter(|group| (group.header.class_id & 0xffff) == 72)
        .filter_map(|group| {
            let path = find_indexed_path(file, group)?;
            let definition_group = groups.get(path.refs.first()?.checked_sub(1)?)?;
            Some(collect_parameter_bindings(file, groups, definition_group))
        })
        .fold(BTreeMap::<String, f64>::new(), |mut acc, bindings| {
            for binding in bindings.into_values() {
                acc.entry(binding.name).or_insert(binding.value);
            }
            acc
        })
        .into_iter()
        .filter_map(|(name, value)| {
            let text = format!("{name} = {:.2}", value);
            let label_index = labels.iter().position(|label| label.text == text)?;
            Some(SceneParameter {
                name,
                value,
                label_index: Some(label_index),
            })
        })
        .collect()
}

pub(super) fn collect_scene_functions(
    file: &GspFile,
    groups: &[ObjectGroup],
    labels: &[TextLabel],
    points: &[ScenePoint],
    plot_line_offset: usize,
) -> Vec<SceneFunction> {
    let base_entries = groups
        .iter()
        .filter(|group| (group.header.class_id & 0xffff) == 72)
        .filter_map(|group| {
            let path = find_indexed_path(file, group)?;
            let definition_ordinal = *path.refs.first()?;
            let definition_group = groups.get(definition_ordinal.checked_sub(1)?)?;
            let expr = decode_function_expr(file, groups, definition_group)?;
            let descriptor = group
                .records
                .iter()
                .find(|record| record.record_type == 0x0902)
                .and_then(|record| decode_function_plot_descriptor(record.payload(&file.data)))?;
            Some((definition_ordinal, expr, descriptor))
        })
        .collect::<Vec<_>>();

    let total = base_entries.len().max(1);
    let mut functions = base_entries
        .iter()
        .enumerate()
        .filter_map(|(index, (definition_ordinal, expr, descriptor))| {
            let name = function_name_for_index(index, total, expr).to_string();
            let label_text = format!("{name}(x) = {}", function_expr_label(expr.clone()));
            let label_index = labels.iter().position(|label| label.text == label_text)?;
            let constrained_point_indices = points
                .iter()
                .enumerate()
                .filter_map(|(point_index, point)| match &point.constraint {
                    ScenePointConstraint::OnPolyline { function_key, .. }
                        if function_key == definition_ordinal =>
                    {
                        Some(point_index)
                    }
                    _ => None,
                })
                .collect::<Vec<_>>();
            Some(SceneFunction {
                key: *definition_ordinal,
                name,
                derivative: false,
                expr: expr.clone(),
                domain: descriptor.clone(),
                line_index: Some(plot_line_offset + index),
                label_index,
                constrained_point_indices,
            })
        })
        .collect::<Vec<_>>();

    functions.extend(
        groups
            .iter()
            .filter(|group| (group.header.class_id & 0xffff) == 78)
            .filter_map(|group| {
                let path = find_indexed_path(file, group)?;
                let base_definition_ordinal = *path.refs.first()?;
                let base_index = base_entries.iter().position(|(definition_ordinal, _, _)| {
                    *definition_ordinal == base_definition_ordinal
                })?;
                let base_name =
                    function_name_for_index(base_index, total, &base_entries[base_index].1);
                let expr = decode_function_expr(file, groups, group)?;
                let label_text =
                    format!("{}'(x) = {}", base_name, function_expr_label(expr.clone()));
                let label_index = labels.iter().position(|label| label.text == label_text)?;
                Some(SceneFunction {
                    key: base_definition_ordinal,
                    name: base_name.to_string(),
                    derivative: true,
                    expr,
                    domain: base_entries[base_index].2.clone(),
                    line_index: None,
                    label_index,
                    constrained_point_indices: Vec::new(),
                })
            }),
    );

    functions
}

pub(super) fn function_uses_pi_scale(file: &GspFile, groups: &[ObjectGroup]) -> bool {
    groups
        .iter()
        .filter(|group| (group.header.class_id & 0xffff) == 72)
        .filter_map(|group| {
            let path = find_indexed_path(file, group)?;
            let definition_group = groups.get(path.refs.first()?.checked_sub(1)?)?;
            decode_function_expr(file, groups, definition_group)
        })
        .any(function_expr_uses_trig)
}

pub(super) fn decode_function_expr(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
) -> Option<FunctionExpr> {
    let payload = group
        .records
        .iter()
        .find(|record| record.record_type == 0x0907)
        .map(|record| record.payload(&file.data))?;
    let parameters = collect_parameter_bindings(file, groups, group);

    let text = extract_inline_function_token(payload)?;
    if text.eq_ignore_ascii_case("x") {
        return Some(FunctionExpr::Identity);
    }
    if let Ok(value) = text.parse::<f64>() {
        if value == 0.0
            && let Some(expr) = decode_inner_function_expr(payload, &parameters)
        {
            return Some(expr);
        }
        return Some(FunctionExpr::Constant(value));
    }
    decode_inner_function_expr(payload, &parameters)
}

pub(super) fn decode_function_plot_descriptor(payload: &[u8]) -> Option<FunctionPlotDescriptor> {
    if payload.len() < 20 {
        return None;
    }

    let x_min = read_f64(payload, 0);
    let x_max = read_f64(payload, 8);
    let sample_count = read_u32(payload, 16) as usize;
    if !x_min.is_finite() || !x_max.is_finite() || x_min == x_max {
        return None;
    }

    Some(FunctionPlotDescriptor {
        x_min,
        x_max,
        sample_count: sample_count.clamp(2, 4096),
    })
}

fn collect_parameter_bindings(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
) -> BTreeMap<u16, ParameterBinding> {
    let mut bindings = BTreeMap::new();
    let Some(path) = find_indexed_path(file, group) else {
        return bindings;
    };
    for (index, ordinal) in path.refs.iter().copied().enumerate() {
        let Some(parameter_group) = groups.get(ordinal.saturating_sub(1)) else {
            continue;
        };
        if let Some(binding) = decode_parameter_binding(file, parameter_group) {
            bindings.insert(index as u16, binding);
        }
    }
    bindings
}

fn decode_parameter_binding(file: &GspFile, group: &ObjectGroup) -> Option<ParameterBinding> {
    let payload = group
        .records
        .iter()
        .find(|record| record.record_type == 0x0907)
        .map(|record| record.payload(&file.data))?;
    let label_payload = group
        .records
        .iter()
        .find(|record| record.record_type == 0x07d5)
        .map(|record| record.payload(&file.data))?;
    let name = decode_parameter_name(label_payload)?;
    let value = if name.contains('₁') || name.contains('₂') || name.contains('₃') || name.contains('₄') {
        read_f64(payload, 52)
    } else {
        f64::from(read_u16(payload, payload.len().checked_sub(2)?))
    };
    if !value.is_finite() {
        return None;
    }
    Some(ParameterBinding {
        name,
        value,
    })
}

fn decode_parameter_name(label_payload: &[u8]) -> Option<String> {
    if label_payload.len() >= 24 {
        let name_len = read_u16(label_payload, 22) as usize;
        if name_len > 0 && 24 + name_len <= label_payload.len() {
            let name = String::from_utf8_lossy(&label_payload[24..24 + name_len]).to_string();
            return Some(
                name.replace("[1]", "₁")
                    .replace("[2]", "₂")
                    .replace("[3]", "₃")
                    .replace("[4]", "₄"),
            );
        }
    }
    if label_payload.len() < 2 {
        return None;
    }
    let name_code = read_u16(label_payload, label_payload.len() - 2);
    char::from_u32(name_code as u32)
        .filter(|ch| ch.is_ascii_alphabetic())?
        .to_string()
        .into()
}

fn extract_inline_function_token(payload: &[u8]) -> Option<String> {
    let text = String::from_utf8_lossy(payload);
    let start = text.find('<')?;
    let end = text[start + 1..].find('>')?;
    let token = text[start + 1..start + 1 + end].trim();
    if token.is_empty() {
        None
    } else {
        Some(token.to_string())
    }
}

pub(super) fn sample_function_points(
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
            points.push(PointRecord { x, y });
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

fn decode_inner_function_expr(
    payload: &[u8],
    parameters: &BTreeMap<u16, ParameterBinding>,
) -> Option<FunctionExpr> {
    parse_function_expr(payload, parameters).map(canonicalize_function_expr)
}

pub(super) fn function_expr_label(expr: FunctionExpr) -> String {
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
                });
                text.push_str(&format_function_term(term));
            }
            text
        }
    }
}

fn function_name_for_index(index: usize, total: usize, expr: &FunctionExpr) -> &'static str {
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

fn evaluate_function_expr(expr: &ParsedFunctionExpr, x: f64) -> Option<f64> {
    let mut value = evaluate_function_term(expr.head.clone(), x)?;
    for (op, term) in &expr.tail {
        let rhs = evaluate_function_term(term.clone(), x)?;
        value = match op {
            BinaryOp::Add => value + rhs,
            BinaryOp::Sub => value - rhs,
            BinaryOp::Mul => value * rhs,
        };
    }
    value.is_finite().then_some(value)
}

pub(super) fn evaluate_expr_with_parameters(
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
            BinaryOp::Mul => value * rhs,
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
    }
}

fn evaluate_function_term(term: FunctionTerm, x: f64) -> Option<f64> {
    match term {
        FunctionTerm::Variable => Some(x),
        FunctionTerm::Constant(value) => Some(value),
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
    }
}

fn parse_function_expr(
    payload: &[u8],
    parameters: &BTreeMap<u16, ParameterBinding>,
) -> Option<ParsedFunctionExpr> {
    let words = payload
        .chunks_exact(2)
        .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
        .collect::<Vec<_>>();
    let marker_index = words
        .windows(2)
        .position(|pair| matches!(pair, [0x0094, 0x0001] | [0x00a0, 0x0001]));
    if let Some(marker_index) = marker_index
        && let Some((parsed, _)) = parse_function_expr_from(&words, marker_index + 2, parameters)
    {
        return Some(parsed);
    }
    find_fallback_function_expr(&words, parameters)
}

fn parse_function_expr_from(
    words: &[u16],
    start: usize,
    parameters: &BTreeMap<u16, ParameterBinding>,
) -> Option<(ParsedFunctionExpr, usize)> {
    let mut index = start;
    let head = parse_function_term(words, &mut index, parameters)?;
    let mut tail = Vec::new();
    while index < words.len() {
        let op = match words[index] {
            0x1000 => BinaryOp::Add,
            0x1001 => BinaryOp::Sub,
            _ => break,
        };
        index += 1;
        let term = parse_function_term(words, &mut index, parameters)?;
        tail.push((op, term));
    }
    Some((ParsedFunctionExpr { head, tail }, index))
}

fn find_fallback_function_expr(
    words: &[u16],
    parameters: &BTreeMap<u16, ParameterBinding>,
) -> Option<ParsedFunctionExpr> {
    (0..words.len())
        .filter_map(|start| parse_function_expr_from(words, start, parameters))
        .find_map(|(parsed, end)| {
            (end == words.len() && parsed_contains_symbol(&parsed)).then_some(parsed)
        })
}

fn parsed_contains_symbol(parsed: &ParsedFunctionExpr) -> bool {
    function_term_contains_symbol(&parsed.head)
        || parsed
            .tail
            .iter()
            .any(|(_, term)| function_term_contains_symbol(term))
}

fn parse_function_term(
    words: &[u16],
    index: &mut usize,
    parameters: &BTreeMap<u16, ParameterBinding>,
) -> Option<FunctionTerm> {
    let mut term = parse_atomic_term(words, index, parameters)?;
    while *index < words.len() && words[*index] == 0x1002 {
        *index += 1;
        let rhs = parse_atomic_term(words, index, parameters)?;
        term = FunctionTerm::Product(Box::new(term), Box::new(rhs));
    }
    Some(term)
}

fn parse_atomic_term(
    words: &[u16],
    index: &mut usize,
    parameters: &BTreeMap<u16, ParameterBinding>,
) -> Option<FunctionTerm> {
    if *index >= words.len() {
        return None;
    }
    if let Some(op) = decode_unary_function(words[*index]) {
        if *index + 2 < words.len() && words[*index + 1] == 0x000f && words[*index + 2] == 0x000c {
            *index += 3;
            return Some(FunctionTerm::UnaryX(op));
        }
        return None;
    }
    if (words[*index] & 0xfff0) == 0x6000 {
        let parameter_index = words[*index] & 0x000f;
        *index += 1;
        let binding = parameters.get(&parameter_index)?.clone();
        return Some(FunctionTerm::Parameter(binding.name, binding.value));
    }
    if *index + 1 < words.len() && words[*index] == 0x000f && words[*index + 1] == 0x000c {
        *index += 2;
        return Some(FunctionTerm::Variable);
    }
    if words[*index] == 0x000f {
        *index += 1;
        return Some(FunctionTerm::Variable);
    }
    let value = words[*index];
    *index += 1;
    Some(FunctionTerm::Constant(f64::from(value)))
}

fn function_expr_uses_trig(expr: FunctionExpr) -> bool {
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

fn function_term_contains_symbol(term: &FunctionTerm) -> bool {
    match term {
        FunctionTerm::Variable | FunctionTerm::UnaryX(_) | FunctionTerm::Parameter(_, _) => true,
        FunctionTerm::Product(left, right) => {
            function_term_contains_symbol(left) || function_term_contains_symbol(right)
        }
        FunctionTerm::Constant(_) => false,
    }
}

fn decode_unary_function(word: u16) -> Option<UnaryFunction> {
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

fn canonicalize_function_expr(parsed: ParsedFunctionExpr) -> FunctionExpr {
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

fn bounds_from_function_plots(
    function_plots: &[LineShape],
    domain: Option<(f64, f64)>,
    graph: &Option<GraphTransform>,
) -> Option<Bounds> {
    let mut bounds =
        if let Some(first) = function_plots.first().and_then(|line| line.points.first()) {
            let first = super::geometry::to_world(first, graph);
            Bounds {
                min_x: first.x,
                max_x: first.x,
                min_y: first.y,
                max_y: first.y,
            }
        } else if let Some((min_x, max_x)) = domain {
            Bounds {
                min_x,
                max_x,
                min_y: 0.0,
                max_y: 0.0,
            }
        } else {
            return None;
        };
    include_line_bounds(&mut bounds, function_plots, graph);
    if let Some((min_x, max_x)) = domain {
        bounds.min_x = bounds.min_x.min(min_x);
        bounds.max_x = bounds.max_x.max(max_x);
    }
    Some(bounds)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::format::GspFile;

    #[test]
    fn extracts_simple_function_token() {
        assert_eq!(
            extract_inline_function_token(b"\0\0<0>\0"),
            Some("0".to_string())
        );
        assert_eq!(
            extract_inline_function_token(b"junk<x>tail"),
            Some("x".to_string())
        );
    }

    #[test]
    fn decodes_f_gsp_function_expr() {
        let data = include_bytes!("../../../f.gsp");
        let file = GspFile::parse(data).expect("fixture parses");
        let groups = file.object_groups();
        let function_group = groups
            .iter()
            .find(|group| {
                group
                    .records
                    .iter()
                    .any(|record| record.record_type == 0x0907)
            })
            .expect("function group");
        let payload = function_group
            .records
            .iter()
            .find(|record| record.record_type == 0x0907)
            .expect("0907 record")
            .payload(&file.data);
        let parameters = BTreeMap::new();
        assert_eq!(
            decode_inner_function_expr(payload, &parameters),
            Some(FunctionExpr::Parsed(ParsedFunctionExpr {
                head: FunctionTerm::UnaryX(UnaryFunction::Abs),
                tail: vec![
                    (BinaryOp::Add, FunctionTerm::UnaryX(UnaryFunction::Sqrt)),
                    (BinaryOp::Add, FunctionTerm::UnaryX(UnaryFunction::Ln)),
                    (BinaryOp::Add, FunctionTerm::UnaryX(UnaryFunction::Log10)),
                    (BinaryOp::Add, FunctionTerm::UnaryX(UnaryFunction::Sign)),
                    (BinaryOp::Add, FunctionTerm::UnaryX(UnaryFunction::Round)),
                    (BinaryOp::Add, FunctionTerm::UnaryX(UnaryFunction::Trunc)),
                ],
            }))
        );
        let expr = decode_function_expr(&file, &groups, function_group);
        assert_eq!(
            expr,
            Some(FunctionExpr::Parsed(ParsedFunctionExpr {
                head: FunctionTerm::UnaryX(UnaryFunction::Abs),
                tail: vec![
                    (BinaryOp::Add, FunctionTerm::UnaryX(UnaryFunction::Sqrt)),
                    (BinaryOp::Add, FunctionTerm::UnaryX(UnaryFunction::Ln)),
                    (BinaryOp::Add, FunctionTerm::UnaryX(UnaryFunction::Log10)),
                    (BinaryOp::Add, FunctionTerm::UnaryX(UnaryFunction::Sign)),
                    (BinaryOp::Add, FunctionTerm::UnaryX(UnaryFunction::Round)),
                    (BinaryOp::Add, FunctionTerm::UnaryX(UnaryFunction::Trunc)),
                ],
            }))
        );
    }
}
