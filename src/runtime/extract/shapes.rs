use super::decode::{
    decode_bbox_anchor_raw, decode_label_name, decode_transform_anchor_raw, find_indexed_path,
};
use super::*;
use crate::runtime::extract::points::decode_translated_point_constraint;
use crate::runtime::functions::{
    evaluate_expr_with_parameters, try_decode_function_expr, try_decode_function_plot_descriptor,
};
use crate::runtime::geometry::{
    color_from_style, fill_color_from_styles, has_distinct_points, line_is_dashed,
    reflect_across_line, rotate_around, scale_around, three_point_arc_geometry, to_raw_from_world,
};
use crate::runtime::scene::{
    LineBinding, LineIterationFamily, PolygonIterationFamily, ShapeBinding,
};

#[path = "shapes/anchors.rs"]
mod anchors;
#[path = "shapes/basic.rs"]
mod basic;
#[path = "shapes/iterations.rs"]
mod iterations;
#[path = "shapes/transforms.rs"]
mod transforms;

pub(crate) use anchors::collect_raw_object_anchors;
pub(super) use basic::{
    collect_arc_boundary_fill_polygons, collect_arc_boundary_shapes, collect_bound_line_shapes,
    collect_circle_fill_colors, collect_circle_shapes, collect_coordinate_traces,
    collect_derived_segments, collect_line_shapes, collect_materialized_ray_groups,
    collect_polygon_shapes, collect_segment_marker_shapes, collect_three_point_arc_shapes,
};
pub(super) use iterations::{
    collect_carried_circle_iteration_families, collect_carried_iteration_circles,
    collect_carried_iteration_lines, collect_carried_iteration_polygons,
    collect_carried_line_iteration_families, collect_carried_polygon_edge_segment_groups,
    collect_carried_polygon_iteration_families, collect_iteration_shapes,
    collect_rotational_iteration_lines, collect_rotational_iteration_segment_groups,
};
pub(super) use transforms::{
    collect_reflected_circle_shapes, collect_reflected_line_shapes,
    collect_reflected_polygon_shapes, collect_rotated_circle_shapes, collect_rotated_line_shapes,
    collect_rotated_polygon_shapes, collect_scaled_line_shapes, collect_transformed_circle_shapes,
    collect_transformed_polygon_shapes, collect_translated_line_shapes,
    collect_translated_polygon_shapes,
};
