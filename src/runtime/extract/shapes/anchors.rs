use super::{
    GraphTransform, GspFile, ObjectGroup, PointRecord, decode_angle_rotation_anchor_raw,
    decode_bbox_anchor_raw, decode_line_midpoint_anchor_raw, decode_offset_anchor_raw,
    decode_parameter_controlled_anchor_raw, decode_parameter_rotation_anchor_raw,
    decode_point_constraint_anchor, decode_point_on_ray_anchor_raw,
    decode_point_pair_translation_anchor_raw, decode_reflection_anchor_raw,
    decode_regular_polygon_vertex_anchor_raw, decode_transform_anchor_raw,
    decode_translated_point_anchor_raw, find_indexed_path, try_decode_payload_anchor_point,
};
use crate::runtime::extract::decode::is_parameter_control_group;
use crate::runtime::extract::points::{
    decode_coordinate_expression_anchor_raw, decode_coordinate_point,
    decode_custom_transform_anchor_raw, decode_expression_offset_anchor_raw,
    decode_expression_rotation_anchor_raw, decode_graph_calibration_anchor_raw,
    decode_intersection_anchor_raw, decode_iteration_binding_point_alias_raw,
    decode_ratio_scale_anchor_raw,
};

pub(crate) fn collect_raw_object_anchors(
    file: &GspFile,
    groups: &[ObjectGroup],
    point_map: &[Option<PointRecord>],
    graph: Option<&GraphTransform>,
) -> Vec<Option<PointRecord>> {
    let mut anchors = Vec::with_capacity(groups.len());
    for (index, group) in groups.iter().enumerate() {
        let anchor = if let Some(point) = point_map.get(index).cloned().flatten() {
            Some(point)
        } else if is_parameter_control_group(group) {
            try_decode_payload_anchor_point(file, group).ok().flatten()
        } else if let Some(anchor) = decode_graph_calibration_anchor_raw(group, graph) {
            Some(anchor)
        } else if let Some(anchor) =
            decode_coordinate_expression_anchor_raw(file, groups, group, &anchors, graph)
        {
            Some(anchor)
        } else if matches!(
            group.header.kind(),
            crate::format::GroupKind::GraphFunctionPoint
                | crate::format::GroupKind::GraphValuePoint
        ) {
            decode_coordinate_point(file, groups, group, &anchors, &graph.cloned())
                .map(|point| point.position)
        } else if let Some(anchor) =
            decode_point_constraint_anchor(file, groups, group, &anchors, graph)
        {
            Some(anchor)
        } else if let Some(anchor) =
            decode_intersection_anchor_raw(file, groups, group, &anchors, graph)
        {
            Some(anchor)
        } else if let Some(anchor) = decode_point_on_ray_anchor_raw(file, groups, group, &anchors) {
            Some(anchor)
        } else if let Some(anchor) = decode_translated_point_anchor_raw(file, group, &anchors) {
            Some(anchor)
        } else if let Some(anchor) = decode_line_midpoint_anchor_raw(file, groups, group, &anchors)
        {
            Some(anchor)
        } else if let Some(anchor) =
            decode_parameter_rotation_anchor_raw(file, groups, group, &anchors)
        {
            Some(anchor)
        } else if let Some(anchor) = decode_angle_rotation_anchor_raw(file, group, &anchors) {
            Some(anchor)
        } else if let Some(anchor) =
            decode_expression_rotation_anchor_raw(file, groups, group, &anchors)
        {
            Some(anchor)
        } else if let Some(anchor) = decode_ratio_scale_anchor_raw(file, groups, group, &anchors) {
            Some(anchor)
        } else if let Some(anchor) = decode_transform_anchor_raw(file, group, &anchors) {
            Some(anchor)
        } else if let Some(anchor) =
            decode_point_pair_translation_anchor_raw(file, groups, group, &anchors)
        {
            Some(anchor)
        } else if let Some(anchor) =
            decode_custom_transform_anchor_raw(file, groups, group, &anchors)
        {
            Some(anchor)
        } else if let Some(anchor) = decode_reflection_anchor_raw(file, groups, group, &anchors) {
            Some(anchor)
        } else if let Some(anchor) =
            decode_regular_polygon_vertex_anchor_raw(file, groups, group, &anchors)
        {
            Some(anchor)
        } else if let Some(anchor) = decode_offset_anchor_raw(file, group, &anchors) {
            Some(anchor)
        } else if let Some(anchor) =
            decode_expression_offset_anchor_raw(file, groups, group, &anchors)
        {
            Some(anchor)
        } else if let Some(anchor) =
            decode_parameter_controlled_anchor_raw(file, groups, group, &anchors)
        {
            Some(anchor)
        } else if let Some(alias) =
            decode_iteration_binding_point_alias_raw(file, groups, group, &anchors)
        {
            Some(alias.position)
        } else if let Some(anchor) = decode_bbox_anchor_raw(file, group) {
            Some(anchor)
        } else {
            if let Some(path) = find_indexed_path(file, group) {
                path.refs.iter().rev().find_map(|object_ref| {
                    anchors.get(object_ref.saturating_sub(1)).cloned().flatten()
                })
            } else {
                None
            }
        };
        anchors.push(anchor);
    }
    anchors
}
