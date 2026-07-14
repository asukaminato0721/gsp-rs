use super::context::SceneContext;
use super::decode::{
    decode_discrete_parameter_value, decode_label_name, find_indexed_path,
    is_parameter_control_group, try_decode_parameter_control_value_for_group,
};
use crate::format::{GspFile, ObjectGroup, PointRecord, decode_point_record, read_f64, read_u16};
use crate::runtime::functions::{
    function_expr_contains_variable, try_decode_standalone_function_expr,
};
use crate::runtime::scene::{SceneParameter, TextLabel};

mod anchors;
mod bindings;
mod constraints;

#[allow(unused_imports)]
pub(crate) use anchors::{
    IterationBindingPointAliasKind, boundary_arc_length_raw,
    custom_transform_expression_parameter_map, custom_transform_trace_parameter,
    decode_angle_rotation_anchor_raw, decode_coordinate_expression_anchor_raw,
    decode_custom_transform_anchor_raw, decode_custom_transform_angle_scale,
    decode_custom_transform_binding, decode_custom_transform_distance_scale,
    decode_derived_polar_endpoint_anchor_raw, decode_derived_polar_endpoint_binding,
    decode_directed_angle_anchor_binding, decode_directed_angle_anchor_raw,
    decode_expression_offset_anchor_raw, decode_expression_offset_binding,
    decode_expression_ratio_scale_binding, decode_expression_rotation_anchor_raw,
    decode_expression_rotation_binding, decode_expression_scale_binding,
    decode_graph_calibration_anchor_raw, decode_intersection_anchor_raw,
    decode_iteration_binding_point_alias_raw, decode_legacy_angle_rotation_anchor_raw,
    decode_line_midpoint_anchor_raw, decode_offset_anchor_raw,
    decode_parameter_controlled_anchor_raw, decode_parameter_rotation_anchor_raw,
    decode_point_constraint_anchor, decode_point_on_ray_anchor_raw,
    decode_point_pair_translation_anchor_raw, decode_ratio_scale_anchor_raw,
    decode_reflection_anchor_raw, decode_regular_polygon_vertex_anchor_raw,
    decode_translated_point_anchor_raw, expression_runtime_context, resolve_circle_like_raw,
    resolve_line_like_points_raw, translation_point_pair_group_indices,
};
#[allow(unused_imports)]
pub(crate) use bindings::TransformBinding;
#[allow(unused_imports)]
pub(super) use bindings::{
    RawPointIterationFamily, TransformBindingKind, collect_point_iteration_points,
    collect_standalone_parameter_points, collect_visible_points_checked,
    collect_visible_points_checked_with_context, refresh_visible_points_checked_with_context,
    remap_arc_bindings, remap_circle_bindings, remap_label_bindings, remap_line_bindings,
    remap_polygon_bindings, scene_point_from_parameter_controlled,
    try_decode_angle_rotation_binding, try_decode_parameter_rotation_binding,
    try_decode_transform_binding,
};
pub(super) use constraints::{
    LegacyCoordinateConstructPoint, RawPointConstraint, decode_translated_point_constraint,
    regular_polygon_angle_expr_for_calc_group, regular_polygon_iteration_step,
    try_decode_parameter_controlled_point, try_decode_parameter_controlled_point_on_polyline,
    try_decode_point_constraint,
};
pub(crate) use constraints::{decode_coordinate_point, decode_legacy_coordinate_construct_point};

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
                (record.record_type == crate::runtime::payload_consts::RECORD_POINT_F64_PAIR)
                    .then(|| decode_point_record(record.payload(&file.data)))
                    .flatten()
                    .map(|point| file.document_display_point(point))
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
    if is_standalone_function_definition_group(file, groups, group) {
        return None;
    }
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
        visible: !group.header.is_hidden(),
    })
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
    let Some(target_group) = groups.get(target_index) else {
        return false;
    };
    if decode_angle_parameter_value_for_group(file, target_group).is_none() {
        return false;
    }
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
        (group.header.kind() == crate::format::GroupKind::FunctionPlot)
            && find_indexed_path(file, group)
                .is_some_and(|path| path.refs.first().copied() == Some(target_ordinal))
    })
}

fn is_function_plot_definition_group_with_context(
    context: &SceneContext<'_>,
    target_ordinal: usize,
) -> bool {
    context
        .group_indices_by_kind(crate::format::GroupKind::FunctionPlot)
        .iter()
        .filter_map(|index| context.group(*index))
        .any(|group| {
            context
                .indexed_path(group)
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
        .filter(|group| {
            matches!(
                group.header.kind(),
                crate::format::GroupKind::FunctionPlot
                    | crate::format::GroupKind::ParametricFunctionPlot
            )
        })
        .filter_map(|group| find_indexed_path(file, group))
        .filter_map(|path| groups.get(path.refs.first()?.checked_sub(1)?))
        .any(|definition_group| {
            find_indexed_path(file, definition_group)
                .is_some_and(|path| path.refs.contains(&target_ordinal))
        })
}

fn is_function_plot_parameter_group_with_context(
    context: &SceneContext<'_>,
    target_ordinal: usize,
) -> bool {
    context
        .group_indices_by_kind(crate::format::GroupKind::FunctionPlot)
        .iter()
        .chain(
            context
                .group_indices_by_kind(crate::format::GroupKind::ParametricFunctionPlot)
                .iter(),
        )
        .filter_map(|index| context.group(*index))
        .filter_map(|group| context.indexed_path(group))
        .filter_map(|path| {
            path.refs
                .first()
                .and_then(|ordinal| context.group_by_ordinal(*ordinal))
        })
        .any(|definition_group| {
            context
                .indexed_path(definition_group)
                .is_some_and(|path| path.refs.contains(&target_ordinal))
        })
}

pub(super) fn is_parametric_function_component_group(
    file: &GspFile,
    groups: &[ObjectGroup],
    target_ordinal: usize,
) -> bool {
    parametric_function_component_slot(file, groups, target_ordinal).is_some()
}

pub(super) fn is_parametric_function_component_group_with_context(
    context: &SceneContext<'_>,
    target_ordinal: usize,
) -> bool {
    parametric_function_component_slot_with_context(context, target_ordinal).is_some()
}

pub(crate) fn is_standalone_function_definition_group(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
) -> bool {
    has_parameter_control_payload(group)
        && !is_function_plot_definition_group(file, groups, group.ordinal)
        && !is_non_graph_parameter_group(file, groups, group)
        && try_decode_standalone_function_expr(file, groups, group)
            .ok()
            .is_some_and(|expr| function_expr_contains_variable(&expr))
}

pub(crate) fn is_standalone_function_definition_group_with_context(
    file: &GspFile,
    context: &SceneContext<'_>,
    group: &ObjectGroup,
) -> bool {
    has_parameter_control_payload(group)
        && !is_function_plot_definition_group_with_context(context, group.ordinal)
        && !is_non_graph_parameter_group_with_context(file, context, group)
        && context
            .standalone_function_expr(group)
            .ok()
            .is_some_and(|expr| function_expr_contains_variable(&expr))
}

pub(super) fn parametric_function_component_slot(
    file: &GspFile,
    groups: &[ObjectGroup],
    target_ordinal: usize,
) -> Option<usize> {
    groups.iter().find_map(|group| {
        ((group.header.kind()) == crate::format::GroupKind::ParametricFunctionPlot)
            .then(|| find_indexed_path(file, group))
            .flatten()
            .and_then(|path| {
                path.refs
                    .iter()
                    .take(2)
                    .position(|ordinal| *ordinal == target_ordinal)
            })
    })
}

pub(super) fn parametric_function_component_slot_with_context(
    context: &SceneContext<'_>,
    target_ordinal: usize,
) -> Option<usize> {
    context
        .group_indices_by_kind(crate::format::GroupKind::ParametricFunctionPlot)
        .iter()
        .filter_map(|index| context.group(*index))
        .find_map(|group| {
            context.indexed_path(group).and_then(|path| {
                path.refs
                    .iter()
                    .take(2)
                    .position(|ordinal| *ordinal == target_ordinal)
            })
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

fn has_external_indexed_path_reference_with_context(
    context: &SceneContext<'_>,
    target_ordinal: usize,
) -> bool {
    context.referrers(target_ordinal).iter().any(|index| {
        context
            .group(*index)
            .is_some_and(|group| group.ordinal != target_ordinal)
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
        && !is_parametric_function_component_group(file, groups, group.ordinal)
        && has_external_indexed_path_reference(file, groups, group.ordinal)
}

pub(super) fn is_non_graph_parameter_group_with_context(
    _file: &GspFile,
    context: &SceneContext<'_>,
    group: &ObjectGroup,
) -> bool {
    has_parameter_control_payload(group)
        && !is_function_plot_definition_group_with_context(context, group.ordinal)
        && !is_function_plot_parameter_group_with_context(context, group.ordinal)
        && !is_parametric_function_component_group_with_context(context, group.ordinal)
        && has_external_indexed_path_reference_with_context(context, group.ordinal)
}

pub(super) fn is_editable_non_graph_parameter_name(name: &str) -> bool {
    !name.trim().is_empty()
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
        .find(|record| {
            record.record_type == crate::runtime::payload_consts::RECORD_FUNCTION_EXPR_PAYLOAD
        })
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
        .find(|record| {
            record.record_type == crate::runtime::payload_consts::RECORD_FUNCTION_EXPR_PAYLOAD
        })
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
        .find(|record| {
            record.record_type == crate::runtime::payload_consts::RECORD_FUNCTION_EXPR_PAYLOAD
        })
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
        .find(|record| {
            record.record_type == crate::runtime::payload_consts::RECORD_FUNCTION_EXPR_PAYLOAD
        })
        .map(|record| record.payload(&file.data))?;
    if decode_parameter_unit_from_payload(payload) == Some("degree") {
        return try_decode_parameter_control_value_for_group(file, &[], group).ok();
    }
    let current = decode_non_graph_parameter_value(payload)?;
    let max = (payload.len() >= 76)
        .then(|| read_f64(payload, 68))
        .filter(|value| value.is_finite())?;
    if max.abs() > std::f64::consts::TAU * 2.0 {
        return None;
    }

    Some(current.to_degrees())
}
