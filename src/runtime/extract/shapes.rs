use std::collections::BTreeSet;

use super::analysis::{CollectedShapes, SceneAnalysis};
use super::decode::{
    decode_bbox_anchor_raw, decode_label_name, decode_transform_anchor_raw, find_indexed_path,
};
use super::*;
use crate::runtime::extract::points::decode_translated_point_constraint;
use crate::runtime::functions::{
    evaluate_expr_with_parameters, synthesize_function_axes, try_decode_function_expr,
    try_decode_function_plot_descriptor,
};
use crate::runtime::geometry::{
    color_from_style, fill_color_from_styles, has_distinct_points, line_is_dashed,
    reflect_across_line, rotate_around, scale_around, three_point_arc_geometry, to_raw_from_world,
};
use crate::runtime::scene::{
    LineBinding, LineIterationFamily, PayloadDebugSource, PolygonIterationFamily, ShapeBinding,
};

#[derive(Debug, Clone)]
pub(super) struct CircleShape {
    pub(super) center: PointRecord,
    pub(super) radius_point: PointRecord,
    pub(super) color: [u8; 4],
    pub(super) fill_color: Option<[u8; 4]>,
    pub(super) fill_visible: bool,
    pub(super) fill_color_binding: Option<crate::runtime::scene::ColorBinding>,
    pub(super) dashed: bool,
    pub(super) visible: bool,
    pub(super) binding: Option<ShapeBinding>,
    pub(super) debug: Option<PayloadDebugSource>,
}

#[derive(Debug, Clone)]
pub(super) struct ArcShape {
    pub(super) points: [PointRecord; 3],
    pub(super) color: [u8; 4],
    pub(super) center: Option<PointRecord>,
    pub(super) counterclockwise: bool,
    pub(super) visible: bool,
    pub(super) debug: Option<PayloadDebugSource>,
}

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
    collect_rotational_line_iteration_families,
};
pub(super) use transforms::{
    collect_reflected_circle_shapes, collect_reflected_line_shapes,
    collect_reflected_polygon_shapes, collect_rotated_circle_shapes, collect_rotated_line_shapes,
    collect_rotated_polygon_shapes, collect_scaled_line_shapes, collect_transformed_circle_shapes,
    collect_transformed_polygon_shapes, collect_translated_circle_shapes,
    collect_translated_line_shapes, collect_translated_polygon_shapes,
};

pub(super) fn collect_scene_shapes(
    file: &GspFile,
    groups: &[ObjectGroup],
    point_map: &[Option<PointRecord>],
    analysis: &SceneAnalysis,
) -> CollectedShapes {
    let suppressed_segment_groups = collect_carried_polygon_edge_segment_groups(file, groups);
    let suppressed_ray_groups = collect_materialized_ray_groups(file, groups);
    let segments = collect_line_shapes(
        file,
        groups,
        &analysis.raw_anchors,
        &[
            crate::format::GroupKind::Segment,
            crate::format::GroupKind::AngleMarker,
        ],
        !analysis.graph_mode && !analysis.large_non_graph,
        &BTreeSet::new(),
    );
    let boundary_lines = collect_arc_boundary_shapes(file, groups, &analysis.raw_anchors);
    let lines = collect_bound_line_shapes(
        file,
        groups,
        &analysis.raw_anchors,
        crate::format::GroupKind::Line,
        &BTreeSet::new(),
    );
    let rays = collect_bound_line_shapes(
        file,
        groups,
        &analysis.raw_anchors,
        crate::format::GroupKind::Ray,
        &suppressed_ray_groups,
    );
    let translated_lines = collect_translated_line_shapes(file, groups, &analysis.raw_anchors);
    let segment_markers = collect_segment_marker_shapes(file, groups, &analysis.raw_anchors);
    let derived_segments = if analysis.large_non_graph {
        collect_derived_segments(
            file,
            groups,
            point_map,
            &[crate::format::GroupKind::DerivedSegment24],
        )
    } else {
        Vec::new()
    };
    let rotated_lines = collect_rotated_line_shapes(file, groups, &analysis.raw_anchors);
    let scaled_lines = collect_scaled_line_shapes(file, groups, &analysis.raw_anchors);
    let reflected_lines = collect_reflected_line_shapes(file, groups, &analysis.raw_anchors);
    let carried_iteration_lines = collect_carried_iteration_lines(
        file,
        groups,
        &analysis.raw_anchors,
        &suppressed_segment_groups,
    );
    let carried_iteration_polygons =
        collect_carried_iteration_polygons(file, groups, &analysis.raw_anchors);
    let carried_iteration_circles =
        collect_carried_iteration_circles(file, groups, &analysis.raw_anchors);
    let measurements = if analysis.graph_mode {
        collect_line_shapes(
            file,
            groups,
            &analysis.raw_anchors,
            &[crate::format::GroupKind::MeasurementLine],
            false,
            &BTreeSet::new(),
        )
    } else {
        Vec::new()
    };
    let coordinate_traces = if analysis.graph_mode {
        collect_coordinate_traces(file, groups, &analysis.raw_anchors, &analysis.graph_ref)
    } else {
        Vec::new()
    };
    let axes = if analysis.graph_mode {
        collect_line_shapes(
            file,
            groups,
            &analysis.raw_anchors,
            &[crate::format::GroupKind::AxisLine],
            false,
            &BTreeSet::new(),
        )
    } else {
        Vec::new()
    };
    let polygons = collect_polygon_shapes(
        file,
        groups,
        &analysis.raw_anchors,
        &[crate::format::GroupKind::Polygon],
    )
    .into_iter()
    .chain(collect_arc_boundary_fill_polygons(
        file,
        groups,
        &analysis.raw_anchors,
    ))
    .collect::<Vec<_>>();
    let circles = collect_circle_shapes(file, groups, &analysis.raw_anchors);
    let arcs = collect_three_point_arc_shapes(file, groups, &analysis.raw_anchors);
    let translated_circles = collect_translated_circle_shapes(file, groups, &analysis.raw_anchors);
    let rotated_circles = collect_rotated_circle_shapes(file, groups, &analysis.raw_anchors);
    let transformed_circles =
        collect_transformed_circle_shapes(file, groups, &analysis.raw_anchors);
    let reflected_circles = collect_reflected_circle_shapes(file, groups, &analysis.raw_anchors);
    let translated_polygons =
        collect_translated_polygon_shapes(file, groups, &analysis.raw_anchors);
    let rotated_polygons = collect_rotated_polygon_shapes(file, groups, &analysis.raw_anchors);
    let transformed_polygons =
        collect_transformed_polygon_shapes(file, groups, &analysis.raw_anchors);
    let reflected_polygons = collect_reflected_polygon_shapes(file, groups, &analysis.raw_anchors);
    let (iteration_lines, iteration_polygons) = collect_iteration_shapes(file, groups, &circles);
    let synthetic_axes = synthesize_axes_if_needed(analysis, &axes);

    CollectedShapes {
        segments: segments.into_iter().chain(boundary_lines).collect(),
        lines,
        rays,
        translated_lines,
        segment_markers,
        derived_segments,
        rotated_lines,
        scaled_lines,
        reflected_lines,
        carried_iteration_lines,
        carried_iteration_polygons,
        carried_iteration_circles,
        measurements,
        coordinate_traces,
        axes,
        polygons,
        circles,
        arcs,
        translated_circles,
        rotated_circles,
        transformed_circles,
        reflected_circles,
        translated_polygons,
        rotated_polygons,
        transformed_polygons,
        reflected_polygons,
        iteration_lines,
        iteration_polygons,
        synthetic_axes,
    }
}

fn synthesize_axes_if_needed(analysis: &SceneAnalysis, axes: &[LineShape]) -> Vec<LineShape> {
    if analysis.graph_mode && analysis.has_function_plots && axes.is_empty() {
        synthesize_function_axes(
            &analysis.function_plots,
            analysis.function_plot_domain,
            analysis.saved_viewport,
            &analysis.graph_ref,
        )
    } else {
        Vec::new()
    }
}
