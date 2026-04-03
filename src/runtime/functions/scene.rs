use std::collections::BTreeMap;

use crate::format::{GspFile, ObjectGroup, read_f64, read_u16};
use crate::runtime::extract::find_indexed_path;
use crate::runtime::scene::{
    SceneFunction, SceneParameter, ScenePoint, ScenePointConstraint, TextLabel,
};

use super::decode::{decode_function_expr, decode_function_plot_descriptor};
use super::expr::{
    FunctionExpr, FunctionPlotDescriptor, function_expr_label, function_expr_uses_trig,
    function_name_for_index,
};

#[derive(Debug, Clone, PartialEq)]
pub(super) struct ParameterBinding {
    pub(super) name: String,
    pub(super) value: f64,
}

pub(crate) fn collect_scene_parameters(
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

pub(crate) fn collect_scene_functions(
    file: &GspFile,
    groups: &[ObjectGroup],
    labels: &[TextLabel],
    points: &[ScenePoint],
    plot_line_offset: usize,
) -> Vec<SceneFunction> {
    let base_entries: Vec<(usize, FunctionExpr, FunctionPlotDescriptor)> = groups
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
        .collect();

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

pub(crate) fn function_uses_pi_scale(file: &GspFile, groups: &[ObjectGroup]) -> bool {
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

pub(super) fn collect_parameter_bindings(
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
    let value = if is_slider_parameter_name(&name) {
        read_f64(payload, 52)
    } else {
        f64::from(read_u16(payload, payload.len().checked_sub(2)?))
    };
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

fn is_slider_parameter_name(name: &str) -> bool {
    name.contains('₁')
        || name.contains('₂')
        || name.contains('₃')
        || name.contains('₄')
        || (name.contains('[') && name.ends_with(']'))
}
