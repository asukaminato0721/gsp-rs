use super::decode::{
    decode_discrete_parameter_value, decode_label_name, find_indexed_path,
    is_parameter_control_group, try_decode_parameter_control_value_for_group,
};
use crate::format::{GspFile, ObjectGroup, PointRecord, decode_point_record, read_f64, read_u16};
use crate::runtime::scene::{SceneParameter, TextLabel};

mod anchors;
mod bindings;
mod constraints;

pub(crate) use anchors::{
    IterationBindingPointAliasKind, custom_transform_expression_parameter_map,
    custom_transform_trace_parameter, decode_angle_rotation_anchor_raw,
    decode_coordinate_expression_anchor_raw, decode_custom_transform_anchor_raw,
    decode_custom_transform_binding, decode_expression_offset_anchor_raw,
    decode_expression_offset_binding, decode_expression_rotation_anchor_raw,
    decode_expression_rotation_binding, decode_graph_calibration_anchor_raw,
    decode_intersection_anchor_raw, decode_iteration_binding_point_alias_raw,
    decode_line_midpoint_anchor_raw, decode_offset_anchor_raw,
    decode_parameter_controlled_anchor_raw, decode_parameter_rotation_anchor_raw,
    decode_point_constraint_anchor, decode_point_on_ray_anchor_raw,
    decode_point_pair_translation_anchor_raw, decode_ratio_scale_anchor_raw,
    decode_reflection_anchor_raw, decode_regular_polygon_vertex_anchor_raw,
    decode_translated_point_anchor_raw, resolve_circle_like_raw, resolve_line_like_points_raw,
    translation_point_pair_group_indices,
};
pub(super) use bindings::{
    RawPointIterationFamily, TransformBindingKind, collect_point_iteration_points,
    collect_standalone_parameter_points, collect_visible_points_checked, remap_circle_bindings,
    remap_label_bindings, remap_line_bindings, remap_polygon_bindings,
    try_decode_angle_rotation_binding, try_decode_parameter_rotation_binding,
    try_decode_transform_binding,
};
pub(crate) use constraints::decode_coordinate_point;
pub(super) use constraints::{
    RawPointConstraint, decode_translated_point_constraint, regular_polygon_angle_expr,
    regular_polygon_angle_expr_for_calc_group, regular_polygon_iteration_step,
    try_decode_parameter_controlled_point, try_decode_point_constraint,
};

pub(crate) fn collect_point_objects(
    file: &GspFile,
    groups: &[ObjectGroup],
) -> Vec<Option<PointRecord>> {
    groups
        .iter()
        .map(|group| {
            if (group.header.kind()) != crate::format::GroupKind::Point {
                return None;
            }
            group.records.iter().find_map(|record| {
                (record.record_type == 0x0899)
                    .then(|| decode_point_record(record.payload(&file.data)))
                    .flatten()
            })
        })
        .collect()
}

pub(super) fn collect_non_graph_parameters(
    file: &GspFile,
    groups: &[ObjectGroup],
    labels: &mut [TextLabel],
) -> Vec<SceneParameter> {
    let allow_orphan_parameter_controls = groups.iter().all(has_parameter_control_payload);
    groups
        .iter()
        .enumerate()
        .filter_map(|(group_index, group)| {
            decode_non_graph_parameter(
                file,
                groups,
                group_index,
                group,
                labels,
                allow_orphan_parameter_controls,
            )
        })
        .collect()
}

fn decode_non_graph_parameter(
    file: &GspFile,
    groups: &[ObjectGroup],
    group_index: usize,
    group: &ObjectGroup,
    labels: &mut [TextLabel],
    allow_orphan_parameter_controls: bool,
) -> Option<SceneParameter> {
    let name = if allow_orphan_parameter_controls && has_parameter_control_payload(group) {
        decode_label_name(file, group)?
    } else {
        editable_non_graph_parameter_name_for_group(file, groups, group)?
    };
    let unit = parameter_unit_for_group(
        file,
        groups,
        group_index,
        group,
        allow_orphan_parameter_controls,
    );
    let value = if allow_orphan_parameter_controls && has_parameter_control_payload(group) {
        decode_orphan_parameter_control_value_for_group(file, group)?
    } else if is_angle_parameter_group(file, groups, group_index) {
        decode_angle_parameter_value_for_group(file, group)?
    } else {
        try_decode_parameter_control_value_for_group(file, groups, group).ok()?
    };
    let label_index = labels.iter().position(|label| label.text == name);
    if let Some(index) = label_index {
        labels[index].text = format_parameter_label(&name, unit.as_deref(), value);
        if allow_orphan_parameter_controls {
            labels[index].visible = true;
        }
    }
    Some(SceneParameter {
        name,
        value,
        unit,
        label_index,
    })
}

fn is_slider_parameter_name(name: &str) -> bool {
    name.contains('₁') || name.contains('₂') || name.contains('₃') || name.contains('₄')
}

fn format_parameter_label(name: &str, unit: Option<&str>, value: f64) -> String {
    format!("{name} = {:.2}{}", value, parameter_unit_suffix(unit))
}

fn parameter_unit_suffix(unit: Option<&str>) -> &'static str {
    match unit {
        Some("degree") => "\u{00b0}",
        Some("cm") => " cm",
        _ => "",
    }
}

fn has_parameter_control_payload(group: &ObjectGroup) -> bool {
    is_parameter_control_group(group)
}

fn is_angle_parameter_group(file: &GspFile, groups: &[ObjectGroup], target_index: usize) -> bool {
    let target_ordinal = target_index + 1;
    groups.iter().any(|group| {
        (group.header.kind()) == crate::format::GroupKind::ParameterRotation
            && find_indexed_path(file, group)
                .is_some_and(|path| path.refs.get(2).copied() == Some(target_ordinal))
    })
}

fn is_function_plot_definition_group(
    file: &GspFile,
    groups: &[ObjectGroup],
    target_ordinal: usize,
) -> bool {
    groups.iter().any(|group| {
        (group.header.kind()) == crate::format::GroupKind::FunctionPlot
            && find_indexed_path(file, group)
                .is_some_and(|path| path.refs.first().copied() == Some(target_ordinal))
    })
}

fn is_function_plot_parameter_group(
    file: &GspFile,
    groups: &[ObjectGroup],
    target_ordinal: usize,
) -> bool {
    groups
        .iter()
        .filter(|group| (group.header.kind()) == crate::format::GroupKind::FunctionPlot)
        .filter_map(|group| find_indexed_path(file, group))
        .filter_map(|path| groups.get(path.refs.first()?.checked_sub(1)?))
        .any(|definition_group| {
            find_indexed_path(file, definition_group)
                .is_some_and(|path| path.refs.contains(&target_ordinal))
        })
}

fn has_external_indexed_path_reference(
    file: &GspFile,
    groups: &[ObjectGroup],
    target_ordinal: usize,
) -> bool {
    groups.iter().any(|group| {
        group.ordinal != target_ordinal
            && find_indexed_path(file, group)
                .is_some_and(|path| path.refs.contains(&target_ordinal))
    })
}

pub(super) fn is_non_graph_parameter_group(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
) -> bool {
    has_parameter_control_payload(group)
        && !is_function_plot_definition_group(file, groups, group.ordinal)
        && !is_function_plot_parameter_group(file, groups, group.ordinal)
        && has_external_indexed_path_reference(file, groups, group.ordinal)
}

pub(super) fn is_editable_non_graph_parameter_name(name: &str) -> bool {
    is_slider_parameter_name(name)
        || (name.chars().count() == 1
            && name
                .chars()
                .next()
                .is_some_and(|ch| ch.is_ascii_alphabetic()))
}

pub(super) fn editable_non_graph_parameter_name_for_group(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
) -> Option<String> {
    is_non_graph_parameter_group(file, groups, group)
        .then(|| decode_label_name(file, group))
        .flatten()
        .filter(|name| is_editable_non_graph_parameter_name(name))
}

fn decode_non_graph_parameter_value(payload: &[u8]) -> Option<f64> {
    decode_discrete_parameter_value(payload)
}

pub(super) fn decode_non_graph_parameter_value_for_group(
    file: &GspFile,
    group: &ObjectGroup,
) -> Option<f64> {
    let payload = group
        .records
        .iter()
        .find(|record| record.record_type == 0x0907)
        .map(|record| record.payload(&file.data))?;
    decode_non_graph_parameter_value(payload)
}

fn decode_orphan_parameter_control_value_for_group(
    file: &GspFile,
    group: &ObjectGroup,
) -> Option<f64> {
    let payload = group
        .records
        .iter()
        .find(|record| record.record_type == 0x0907)
        .map(|record| record.payload(&file.data))?;
    let control_value_offset = match payload
        .len()
        .checked_sub(2)
        .map(|offset| read_u16(payload, offset))
    {
        Some(0x0101 | 0x0201) => payload.len().checked_sub(4)?,
        _ => payload.len().checked_sub(2)?,
    };
    Some(f64::from(read_u16(payload, control_value_offset)))
}

fn decode_parameter_unit_from_payload(payload: &[u8]) -> Option<&'static str> {
    let offset = payload.len().checked_sub(2)?;
    match read_u16(payload, offset) {
        0x0101 => Some("degree"),
        0x0201 => Some("cm"),
        _ => None,
    }
}

fn parameter_unit_for_group(
    file: &GspFile,
    groups: &[ObjectGroup],
    group_index: usize,
    group: &ObjectGroup,
    allow_orphan_parameter_controls: bool,
) -> Option<String> {
    let payload = group
        .records
        .iter()
        .find(|record| record.record_type == 0x0907)
        .map(|record| record.payload(&file.data));
    if let Some(unit) = payload.and_then(decode_parameter_unit_from_payload) {
        return Some(unit.to_string());
    }
    if !allow_orphan_parameter_controls && is_angle_parameter_group(file, groups, group_index) {
        return Some("degree".to_string());
    }
    None
}

pub(super) fn decode_angle_parameter_value_for_group(
    file: &GspFile,
    group: &ObjectGroup,
) -> Option<f64> {
    let payload = group
        .records
        .iter()
        .find(|record| record.record_type == 0x0907)
        .map(|record| record.payload(&file.data))?;
    let current = decode_non_graph_parameter_value(payload)?;
    let max = (payload.len() >= 76)
        .then(|| read_f64(payload, 68))
        .filter(|value| value.is_finite())?;
    let step = (payload.len() >= 84)
        .then(|| read_f64(payload, 76))
        .filter(|value| value.is_finite() && *value > 0.0)?;

    // Legacy copies of some angle sliders keep range metadata but lose the current
    // snapped value. Recover the intended quarter-turn from the preserved tick step.
    if payload.len() >= 98
        && (max - std::f64::consts::TAU).abs() < 1e-6
        && (step - std::f64::consts::FRAC_PI_4).abs() < 1e-6
        && current < step * 0.5
    {
        return Some((step * 2.0).to_degrees());
    }

    Some(current.to_degrees())
}
