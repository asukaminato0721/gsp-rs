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
    decode_custom_transform_anchor_raw, decode_derived_polar_endpoint_anchor_raw,
    decode_expression_offset_anchor_raw, decode_expression_rotation_anchor_raw,
    decode_graph_calibration_anchor_raw, decode_intersection_anchor_raw,
    decode_iteration_binding_point_alias_raw, decode_legacy_coordinate_construct_point,
    decode_marked_angle_translation_anchor_raw, decode_ratio_scale_anchor_raw,
};
use crate::runtime::extract::points::{
    decode_directed_angle_anchor_raw, decode_legacy_angle_rotation_anchor_raw,
};
use crate::runtime::functions::{
    cached_raw_object_anchors, point_map_fingerprint, set_cached_raw_object_anchor,
    with_function_expr_cache,
};

pub(crate) fn collect_raw_object_anchors(
    file: &GspFile,
    groups: &[ObjectGroup],
    point_map: &[Option<PointRecord>],
    graph: Option<&GraphTransform>,
) -> Vec<Option<PointRecord>> {
    let point_map_fingerprint = point_map_fingerprint(point_map);
    with_function_expr_cache(|| {
        cached_raw_object_anchors(
            groups.len(),
            point_map_fingerprint,
            graph,
            point_map,
            || {
                collect_raw_object_anchors_inner(
                    file,
                    groups,
                    point_map,
                    point_map_fingerprint,
                    graph,
                )
            },
        )
    })
}

fn collect_raw_object_anchors_inner(
    file: &GspFile,
    groups: &[ObjectGroup],
    point_map: &[Option<PointRecord>],
    point_map_fingerprint: u64,
    graph: Option<&GraphTransform>,
) -> Vec<Option<PointRecord>> {
    let mut anchors = Vec::with_capacity(groups.len());
    for (index, group) in groups.iter().enumerate() {
        let anchor = if let Some(point) = point_map.get(index).cloned().flatten() {
            Some(point)
        } else if group.header.kind() == crate::format::GroupKind::FunctionExpr {
            inherited_anchor(file, group, &anchors)
        } else if is_parameter_control_group(group) {
            try_decode_payload_anchor_point(file, group).ok().flatten()
        } else if let Some(anchor) =
            decode_graph_calibration_anchor_raw(file, groups, group, &anchors, graph)
        {
            Some(anchor)
        } else if let Some(anchor) =
            decode_coordinate_expression_anchor_raw(file, groups, group, &anchors, graph)
        {
            Some(anchor)
        } else if let Some(anchor) = decode_directed_angle_anchor_raw(file, group, &anchors) {
            Some(anchor)
        } else if let Some(anchor) =
            decode_marked_angle_translation_anchor_raw(file, groups, group, &anchors)
        {
            Some(anchor)
        } else if matches!(
            group.header.kind(),
            crate::format::GroupKind::CoordinatePoint
                | crate::format::GroupKind::CoordinateExpressionPoint
                | crate::format::GroupKind::CoordinateExpressionPointAlt
                | crate::format::GroupKind::CoordinateExpressionPointPair
                | crate::format::GroupKind::GraphFunctionPoint
                | crate::format::GroupKind::GraphValuePoint
                | crate::format::GroupKind::LegacyCoordinateParameterHelper
                | crate::format::GroupKind::LegacyCoordinatePointHelper
                | crate::format::GroupKind::FixedCoordinatePoint
        ) {
            decode_coordinate_point(file, groups, group, &anchors, &graph.cloned())
                .map(|point| point.position)
        } else if group.header.kind() == crate::format::GroupKind::LegacyCoordinateConstructPoint {
            decode_legacy_coordinate_construct_point(file, groups, group, &anchors)
                .map(|point| point.position)
        } else if let Some(anchor) =
            decode_custom_transform_anchor_raw(file, groups, group, &anchors, graph)
        {
            Some(anchor)
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
        } else if let Some(anchor) =
            decode_derived_polar_endpoint_anchor_raw(file, groups, group, &anchors, graph)
        {
            Some(anchor)
        } else if let Some(anchor) = decode_angle_rotation_anchor_raw(file, group, &anchors) {
            Some(anchor)
        } else if let Some(anchor) =
            decode_legacy_angle_rotation_anchor_raw(file, groups, group, &anchors)
        {
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
            inherited_anchor(file, group, &anchors)
        };
        set_cached_raw_object_anchor(
            groups.len(),
            point_map_fingerprint,
            graph,
            index,
            anchor.clone(),
        );
        anchors.push(anchor);
    }
    anchors
}

fn inherited_anchor(
    file: &GspFile,
    group: &ObjectGroup,
    anchors: &[Option<PointRecord>],
) -> Option<PointRecord> {
    find_indexed_path(file, group)?
        .refs
        .iter()
        .rev()
        .find_map(|object_ref| anchors.get(object_ref.saturating_sub(1)).cloned().flatten())
}
