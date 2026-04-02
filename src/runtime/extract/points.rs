use super::decode::decode_label_name;
use crate::format::{GspFile, ObjectGroup, PointRecord, decode_point_record, read_f64, read_u16};
use crate::runtime::scene::{SceneParameter, TextLabel};

mod anchors;
mod bindings;
mod constraints;

pub(super) use anchors::{
    decode_offset_anchor_raw, decode_parameter_controlled_anchor_raw,
    decode_parameter_rotation_anchor_raw, decode_point_constraint_anchor,
    decode_point_on_ray_anchor_raw, decode_point_pair_translation_anchor_raw,
    decode_reflection_anchor_raw, decode_regular_polygon_vertex_anchor_raw,
    decode_translated_point_anchor_raw, reflection_line_group_indices,
    translation_point_pair_group_indices,
};
pub(super) use bindings::{
    RawPointIterationFamily, TransformBindingKind, collect_point_iteration_points,
    collect_visible_points, decode_parameter_rotation_binding, decode_transform_binding,
    remap_circle_bindings, remap_label_bindings, remap_line_bindings, remap_polygon_bindings,
};
pub(super) use constraints::{
    RawPointConstraint, decode_point_constraint, decode_translated_point_constraint,
    regular_polygon_angle_expr, regular_polygon_iteration_step,
};

pub(super) fn collect_point_objects(
    file: &GspFile,
    groups: &[ObjectGroup],
) -> Vec<Option<PointRecord>> {
    groups
        .iter()
        .map(|group| {
            if (group.header.class_id & 0xffff) != 0 {
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
    groups
        .iter()
        .filter_map(|group| decode_non_graph_parameter(file, group, labels))
        .collect()
}

fn decode_non_graph_parameter(
    file: &GspFile,
    group: &ObjectGroup,
    labels: &mut [TextLabel],
) -> Option<SceneParameter> {
    if (group.header.class_id & 0xffff) != 0 {
        return None;
    }
    if group
        .records
        .iter()
        .any(|record| record.record_type == 0x0899)
    {
        return None;
    }
    let _payload = group
        .records
        .iter()
        .find(|record| record.record_type == 0x0907)
        .map(|record| record.payload(&file.data))?;
    let name = decode_label_name(file, group)?;
    if !is_editable_non_graph_parameter_name(&name) {
        return None;
    }
    let value = decode_non_graph_parameter_value_for_group(file, group)?;
    let label_index = labels.iter().position(|label| label.text == name);
    if let Some(index) = label_index {
        labels[index].text = format!("{name} = {:.2}", value);
    }
    Some(SceneParameter {
        name,
        value,
        label_index,
    })
}

fn is_slider_parameter_name(name: &str) -> bool {
    name.contains('₁') || name.contains('₂') || name.contains('₃') || name.contains('₄')
}

pub(super) fn is_editable_non_graph_parameter_name(name: &str) -> bool {
    is_slider_parameter_name(name)
        || (name.chars().count() == 1
            && name
                .chars()
                .next()
                .is_some_and(|ch| ch.is_ascii_alphabetic()))
}

fn decode_non_graph_parameter_value(payload: &[u8]) -> Option<f64> {
    (payload.len() >= 60)
        .then(|| read_f64(payload, 52))
        .filter(|value| value.is_finite())
}

pub(super) fn decode_non_graph_parameter_value_for_group(
    file: &GspFile,
    group: &ObjectGroup,
) -> Option<f64> {
    let name = decode_label_name(file, group)?;
    let payload = group
        .records
        .iter()
        .find(|record| record.record_type == 0x0907)
        .map(|record| record.payload(&file.data))?;
    if is_slider_parameter_name(&name) {
        decode_non_graph_parameter_value(payload)
    } else {
        let value_code = read_u16(payload, payload.len().checked_sub(2)?);
        Some(f64::from(value_code))
    }
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
        return Some(step * 2.0);
    }

    Some(current)
}
