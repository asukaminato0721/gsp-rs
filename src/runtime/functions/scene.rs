use std::collections::BTreeMap;

use crate::format::{GspFile, ObjectGroup, PointRecord, read_u16};
use crate::runtime::extract::points::is_standalone_function_definition_group;
use crate::runtime::extract::{
    find_indexed_path, payload_debug_source, try_decode_bbox_rect_raw,
    try_decode_parameter_control_value_for_group, try_decode_payload_anchor_point,
};
use crate::runtime::scene::{
    SceneFunction, SceneFunctionDefinition, SceneParameter, ScenePoint, ScenePointConstraint,
    TextLabel,
};

use super::decode::{try_decode_function_expr, try_decode_function_plot_descriptor};
use super::expr::{
    BinaryOp, FunctionAst, FunctionExpr, FunctionPlotDescriptor, function_expr_contains_variable,
    function_expr_label_with_variable, function_expr_uses_trig, function_name_for_index,
};

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ParameterBinding {
    pub(super) name: String,
    pub(super) value: f64,
}

fn source_function_name(file: &GspFile, group: &ObjectGroup) -> Option<String> {
    let record = group
        .records
        .iter()
        .find(|record| record.record_type == crate::runtime::payload_consts::RECORD_LABEL_AUX)?;
    let name = decode_parameter_name(record.payload(&file.data))?;
    name.chars()
        .all(|ch| ch.is_ascii_alphabetic())
        .then_some(name)
}

pub(crate) fn function_name_for_definition(
    file: &GspFile,
    group: &ObjectGroup,
    index: usize,
    total: usize,
    expr: &FunctionExpr,
) -> String {
    source_function_name(file, group)
        .unwrap_or_else(|| function_name_for_index(index, total, expr).to_string())
}

pub(crate) fn collect_scene_parameters(
    file: &GspFile,
    groups: &[ObjectGroup],
    labels: &[TextLabel],
) -> Vec<SceneParameter> {
    groups
        .iter()
        .filter(|group| (group.header.kind()) == crate::format::GroupKind::FunctionPlot)
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
            let label_index = labels.iter().position(|label| {
                matches!(
                    label.binding.as_ref(),
                    Some(crate::runtime::scene::TextLabelBinding::ParameterValue {
                        name: label_name,
                    }) if label_name == &name
                )
            })?;
            Some(SceneParameter {
                name,
                value,
                unit: None,
                label_index: Some(label_index),
                visible: true,
            })
        })
        .collect()
}

pub(crate) fn collect_scene_functions(
    file: &GspFile,
    groups: &[ObjectGroup],
    labels: &[TextLabel],
    points: &[ScenePoint],
    plot_line_offset: usize,
) -> Vec<SceneFunction> {
    let base_entries: Vec<(usize, String, FunctionExpr, FunctionPlotDescriptor)> = groups
        .iter()
        .filter(|group| (group.header.kind()) == crate::format::GroupKind::FunctionPlot)
        .filter_map(|group| {
            let path = find_indexed_path(file, group)?;
            let definition_ordinal = *path.refs.first()?;
            let definition_group = groups.get(definition_ordinal.checked_sub(1)?)?;
            let expr = try_decode_function_expr(file, groups, definition_group).ok()?;
            let descriptor_record = group.records.iter().find(|record| {
                record.record_type
                    == crate::runtime::payload_consts::RECORD_FUNCTION_PLOT_DESCRIPTOR
            })?;
            let descriptor =
                try_decode_function_plot_descriptor(descriptor_record.payload(&file.data)).ok()?;
            let name = source_function_name(file, definition_group).unwrap_or_default();
            Some((definition_ordinal, name, expr, descriptor))
        })
        .collect();

    let total = base_entries.len().max(1);
    let mut functions = base_entries
        .iter()
        .enumerate()
        .filter_map(
            |(index, (definition_ordinal, source_name, expr, descriptor))| {
                let name = if source_name.is_empty() {
                    let definition_group = groups.get(definition_ordinal.checked_sub(1)?)?;
                    function_name_for_definition(file, definition_group, index, total, expr)
                } else {
                    source_name.clone()
                };
                let label_index = labels.iter().position(|label| {
                    matches!(
                        label.binding.as_ref(),
                        Some(crate::runtime::scene::TextLabelBinding::FunctionLabel {
                            function_key,
                            derivative,
                        }) if *function_key == *definition_ordinal && !derivative
                    )
                });
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
            },
        )
        .collect::<Vec<_>>();

    functions.extend(
        groups
            .iter()
            .filter(|group| (group.header.kind()) == crate::format::GroupKind::DerivativeFunction)
            .filter_map(|group| {
                let path = find_indexed_path(file, group)?;
                let base_definition_ordinal = *path.refs.first()?;
                let base_index =
                    base_entries
                        .iter()
                        .position(|(definition_ordinal, _, _, _)| {
                            *definition_ordinal == base_definition_ordinal
                        })?;
                let definition_group = groups.get(base_definition_ordinal.checked_sub(1)?)?;
                let base_name = function_name_for_definition(
                    file,
                    definition_group,
                    base_index,
                    total,
                    &base_entries[base_index].2,
                );
                let expr = try_decode_function_expr(file, groups, group).ok()?;
                let label_index = labels.iter().position(|label| {
                    matches!(
                        label.binding.as_ref(),
                        Some(crate::runtime::scene::TextLabelBinding::FunctionLabel {
                            function_key,
                            derivative,
                        }) if *function_key == base_definition_ordinal && *derivative
                    )
                });
                Some(SceneFunction {
                    key: base_definition_ordinal,
                    name: base_name,
                    derivative: true,
                    expr,
                    domain: base_entries[base_index].3.clone(),
                    line_index: None,
                    label_index,
                    constrained_point_indices: Vec::new(),
                })
            }),
    );

    functions
}

pub(crate) fn collect_standalone_function_definitions(
    file: &GspFile,
    groups: &[ObjectGroup],
    labels: &[TextLabel],
) -> Vec<SceneFunctionDefinition> {
    groups
        .iter()
        .filter(|group| is_standalone_function_definition_group(file, groups, group))
        .filter_map(|group| {
            let expr =
                super::decode::try_decode_standalone_function_expr(file, groups, group).ok()?;
            let name = source_function_name(file, group)?;
            let expected_label = format!(
                "{name}(x) = {}",
                super::expr::function_expr_label(expr.clone())
            );
            function_expr_contains_variable(&expr).then_some(SceneFunctionDefinition {
                key: group.ordinal,
                name,
                expr,
                label_index: labels
                    .iter()
                    .position(|label| {
                        label
                            .debug
                            .as_ref()
                            .is_some_and(|debug| debug.group_ordinal == group.ordinal)
                    })
                    .or_else(|| labels.iter().position(|label| label.text == expected_label)),
            })
        })
        .collect()
}

pub(crate) fn synthesize_standalone_function_definition_labels(
    file: &GspFile,
    groups: &[ObjectGroup],
    existing_labels: &[TextLabel],
) -> Vec<TextLabel> {
    groups
        .iter()
        .filter(|group| standalone_function_label_candidate(file, groups, group).is_some())
        .filter(|group| {
            !existing_labels.iter().any(|label| {
                label
                    .debug
                    .as_ref()
                    .is_some_and(|debug| debug.group_ordinal == group.ordinal)
            })
        })
        .filter_map(|group| {
            let anchor = if group.header.kind() == crate::format::GroupKind::FunctionDefinition {
                function_definition_screen_anchor(file, group)?
            } else {
                try_decode_payload_anchor_point(file, group)
                    .ok()
                    .flatten()?
            };
            let (_expr, text) = standalone_function_label_candidate(file, groups, group)?;
            Some(TextLabel {
                anchor,
                text,
                rich_markup: None,
                color: [30, 30, 30, 255],
                font_size: None,
                font_family: None,
                visible: true,
                binding: None,
                screen_space: true,
                hotspots: Vec::new(),
                debug: Some(payload_debug_source(group)),
            })
        })
        .collect()
}

fn standalone_function_label_candidate(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
) -> Option<(FunctionExpr, String)> {
    if is_standalone_function_definition_group(file, groups, group) {
        let name = source_function_name(file, group)?;
        let expr = super::decode::try_decode_standalone_function_expr(file, groups, group).ok()?;
        let text = format!(
            "{name}(x) = {}",
            super::expr::function_expr_label(expr.clone())
        );
        return Some((expr, text));
    }

    if group.header.kind() != crate::format::GroupKind::FunctionDefinition
        || function_definition_has_plot(file, groups, group.ordinal)
        || !function_definition_label_visible(group)
    {
        return None;
    }

    let expr = try_decode_function_expr(file, groups, group).ok()?;
    if !function_expr_contains_variable(&expr) {
        return None;
    }
    let text = format!("y = {}", function_definition_label(&expr));
    Some((expr, text))
}

fn function_definition_has_plot(file: &GspFile, groups: &[ObjectGroup], ordinal: usize) -> bool {
    groups.iter().any(|group| {
        group.header.kind() == crate::format::GroupKind::FunctionPlot
            && find_indexed_path(file, group)
                .is_some_and(|path| path.refs.first().copied() == Some(ordinal))
    })
}

fn function_definition_label_visible(group: &ObjectGroup) -> bool {
    !group.header.is_hidden()
}

fn function_definition_screen_anchor(file: &GspFile, group: &ObjectGroup) -> Option<PointRecord> {
    let (x, mut y) = try_decode_bbox_rect_raw(file, group)
        .ok()
        .flatten()
        .map(|(left, top, _width, height)| (left + 4.0, top + height - 9.0))
        .map(|(x, y)| {
            let point = file.document_display_point(PointRecord { x, y });
            (point.x, point.y)
        })
        .or_else(|| {
            try_decode_payload_anchor_point(file, group)
                .ok()
                .flatten()
                .map(|anchor| {
                    file.document_display_point(PointRecord {
                        x: anchor.x + 4.0,
                        y: anchor.y + 20.0,
                    })
                })
                .map(|point| (point.x, point.y))
        })?;
    y += 1.0;
    Some(PointRecord { x, y })
}

fn function_definition_label(expr: &FunctionExpr) -> String {
    match expr {
        FunctionExpr::Parsed(ast) => function_definition_ast_label(ast),
        _ => htm_unsubscript_digits(&function_expr_label_with_variable(expr.clone(), "x")),
    }
}

fn function_definition_ast_label(ast: &FunctionAst) -> String {
    match ast {
        FunctionAst::Binary {
            lhs,
            op: BinaryOp::Add,
            rhs,
        } => format!(
            "{} + {}",
            function_definition_ast_label(lhs),
            function_definition_ast_label(rhs)
        ),
        FunctionAst::Binary {
            lhs,
            op: BinaryOp::Sub,
            rhs,
        } => format!(
            "{} - {}",
            function_definition_ast_label(lhs),
            function_definition_ast_label(rhs)
        ),
        FunctionAst::Binary {
            lhs,
            op: BinaryOp::Mul,
            rhs,
        } => {
            let left = function_definition_ast_label(lhs);
            let right = function_definition_ast_label(rhs);
            if matches!(
                **rhs,
                FunctionAst::Binary {
                    op: BinaryOp::Pow,
                    ..
                }
            ) {
                format!("{left}*({right})")
            } else {
                format!("{left}*{right}")
            }
        }
        _ => htm_ast_label(ast, "x", false),
    }
}

fn htm_ast_label(ast: &FunctionAst, variable: &str, wrap_binary: bool) -> String {
    let text = match ast {
        FunctionAst::Variable => variable.to_string(),
        FunctionAst::Constant(value) => crate::runtime::geometry::format_number(*value),
        FunctionAst::PiAngle => "π".to_string(),
        FunctionAst::Parameter(name, _) => htm_unsubscript_digits(name),
        FunctionAst::Unary { op, expr } => {
            let inner = htm_ast_label(expr, variable, false);
            match op {
                super::UnaryFunction::Sin => format!("sin({inner})"),
                super::UnaryFunction::Cos => format!("cos({inner})"),
                super::UnaryFunction::Tan => format!("tan({inner})"),
                super::UnaryFunction::Abs => format!("abs({inner})"),
                super::UnaryFunction::Sqrt => format!("√{inner}"),
                super::UnaryFunction::Ln => format!("ln({inner})"),
                super::UnaryFunction::Log10 => format!("log({inner})"),
                super::UnaryFunction::Sign => format!("sgn({inner})"),
                super::UnaryFunction::Round => format!("round({inner})"),
                super::UnaryFunction::Trunc => format!("trunc({inner})"),
            }
        }
        FunctionAst::Binary { lhs, op, rhs } => {
            let lhs_text = htm_ast_label(lhs, variable, false);
            let rhs_text = match (&**rhs, op) {
                (FunctionAst::Binary { .. }, _) => {
                    format!("({})", htm_ast_label(rhs, variable, false))
                }
                _ => htm_ast_label(rhs, variable, false),
            };
            match op {
                BinaryOp::Add => format!("{lhs_text} + {rhs_text}"),
                BinaryOp::Sub => format!("{lhs_text} - {rhs_text}"),
                BinaryOp::Mul => format!("{lhs_text}*{rhs_text}"),
                BinaryOp::Div => format!("{lhs_text} / {rhs_text}"),
                BinaryOp::Pow => format!("{lhs_text}^{rhs_text}"),
            }
        }
    };
    if wrap_binary && matches!(ast, FunctionAst::Binary { .. }) {
        format!("({text})")
    } else {
        text
    }
}

fn htm_unsubscript_digits(text: &str) -> String {
    text.replace('₁', "[1]")
        .replace('₂', "[2]")
        .replace('₃', "[3]")
        .replace('₄', "[4]")
}

pub(crate) fn function_uses_pi_scale(file: &GspFile, groups: &[ObjectGroup]) -> bool {
    groups
        .iter()
        .filter(|group| (group.header.kind()) == crate::format::GroupKind::FunctionPlot)
        .filter_map(|group| {
            let path = find_indexed_path(file, group)?;
            let definition_group = groups.get(path.refs.first()?.checked_sub(1)?)?;
            try_decode_function_expr(file, groups, definition_group).ok()
        })
        .any(function_expr_uses_trig)
}

pub(crate) fn collect_parameter_bindings(
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
        if let Some(binding) = decode_parameter_binding(file, groups, parameter_group) {
            bindings.insert(index as u16, binding);
        }
    }
    bindings
}

fn decode_parameter_binding(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
) -> Option<ParameterBinding> {
    let label_payload = group
        .records
        .iter()
        .find(|record| record.record_type == crate::runtime::payload_consts::RECORD_LABEL_AUX)
        .map(|record| record.payload(&file.data))?;
    let name = decode_parameter_name(label_payload)?;
    let value = try_decode_parameter_control_value_for_group(file, groups, group).ok()?;
    if !value.is_finite() {
        return None;
    }
    Some(ParameterBinding { name, value })
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
