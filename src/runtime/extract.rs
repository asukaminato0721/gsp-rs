use anyhow::Result;
use std::collections::{BTreeMap, BTreeSet};

mod analysis;
mod assemble;
mod bindings;
mod build;
mod buttons;
#[cfg(test)]
mod buttons_labels_images_tests;
mod context;
mod decode;
#[cfg(test)]
mod function_graph_tests;
mod graph;
#[cfg(test)]
mod htm_reference;
mod images;
mod iteration_depth;
mod labels;
#[cfg(test)]
mod payload_log_tests;
mod payload_report;
pub(crate) mod points;
pub(crate) mod shapes;
#[cfg(test)]
mod test_support;
#[cfg(test)]
mod tests_iterations;
#[cfg(test)]
mod tests_labels;
#[cfg(test)]
mod tests_points;
#[cfg(test)]
mod tests_shapes;
#[cfg(test)]
mod tests_trace;
mod trace;
mod world;

use self::context::SceneContext;
pub(crate) use self::decode::decode_measurement_value;
use crate::format::{
    GroupKind, GspFile, ObjectGroup, PointRecord, Record, collect_strings, decode_c_string,
    decode_indexed_path, decode_point_record, read_f64, read_u16, read_u32, record_name,
};
use crate::runtime::functions::{
    BinaryOp, FunctionAst, FunctionExpr, FunctionPlotMode, UnaryFunction,
    function_expr_label_with_variable,
};
use crate::runtime::payload_consts::{
    RECORD_BINDING_PAYLOAD, RECORD_FUNCTION_EXPR_PAYLOAD, RECORD_FUNCTION_PLOT_DESCRIPTOR,
    RECORD_LABEL_AUX, RECORD_POINT_F64_PAIR,
};
use crate::util::{hex_bytes, truncate_text};

use self::graph::{
    collect_document_canvas_bounds, collect_saved_viewport, detect_graph_transform,
    has_coordinate_transform_consumers, has_graph_classes,
};
use self::labels::{circle_parameter, polygon_boundary_parameter};
pub(crate) use self::payload_report::render_payload_log;
use self::points::{
    RawPointConstraint, TransformBindingKind, collect_point_objects,
    decode_angle_rotation_anchor_raw, decode_line_midpoint_anchor_raw, decode_offset_anchor_raw,
    decode_parameter_controlled_anchor_raw, decode_parameter_rotation_anchor_raw,
    decode_point_constraint_anchor, decode_point_on_ray_anchor_raw,
    decode_point_pair_translation_anchor_raw, decode_reflection_anchor_raw,
    decode_regular_polygon_vertex_anchor_raw, decode_translated_point_anchor_raw,
    decode_translated_point_constraint, regular_polygon_iteration_step, remap_arc_bindings,
    remap_circle_bindings, remap_line_bindings, remap_polygon_bindings,
    translation_point_pair_group_indices, try_decode_parameter_controlled_point,
    try_decode_parameter_rotation_binding, try_decode_point_constraint,
    try_decode_transform_binding,
};
use self::shapes::{
    collect_carried_line_iteration_families, collect_carried_polygon_iteration_families,
    collect_raw_object_anchors, collect_rotational_line_iteration_families,
};
use super::functions::{
    collect_function_plot_domain, collect_function_plots, function_uses_pi_scale,
    try_decode_function_expr, try_decode_function_plot_descriptor,
};
use super::geometry::{Bounds, GraphTransform, color_from_style, line_is_dashed};
use super::scene::{
    ColorBinding, LineIterationFamily, LineShape, PointIterationFamily, PolygonIterationFamily,
    PolygonShape, ScenePoint,
};

pub(crate) use self::build::build_scene_checked;
pub(super) use self::build::payload_debug_source;
pub(crate) use self::decode::{
    find_indexed_path, is_circle_group_kind, try_decode_bbox_rect_raw, try_decode_group_label_text,
    try_decode_group_rich_text, try_decode_link_button_url,
    try_decode_parameter_control_value_for_group, try_decode_payload_anchor_point,
    try_find_indexed_path,
};
