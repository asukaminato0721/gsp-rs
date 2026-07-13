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
    line_stroke_width_from_style, reflect_across_line, rotate_around, scale_around,
    three_point_arc_geometry, to_raw_from_world,
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
    collect_circle_fill_colors, collect_circle_shapes, collect_constructed_line_shapes,
    collect_coordinate_traces, collect_line_shapes, collect_materialized_ray_groups,
    collect_polygon_shapes, collect_segment_marker_shapes, collect_three_point_arc_shapes,
};
pub(super) use iterations::{
    collect_carried_circle_iteration_families, collect_carried_iteration_circles,
    collect_carried_iteration_lines, collect_carried_iteration_polygons,
    collect_carried_line_iteration_families, collect_carried_polygon_edge_segment_groups,
    collect_carried_polygon_iteration_families, collect_rotational_line_iteration_families,
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
    context: &SceneContext<'_>,
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
        &BTreeSet::new(),
    );
    let boundary_lines = collect_arc_boundary_shapes(file, groups, &analysis.raw_anchors);
    let lines: Vec<_> = collect_bound_line_shapes(
        file,
        groups,
        &analysis.raw_anchors,
        crate::format::GroupKind::Line,
        &BTreeSet::new(),
    )
    .into_iter()
    .chain(collect_constructed_line_shapes(
        file,
        groups,
        &analysis.raw_anchors,
    ))
    .collect();
    let rays = collect_bound_line_shapes(
        file,
        groups,
        &analysis.raw_anchors,
        crate::format::GroupKind::Ray,
        &suppressed_ray_groups,
    );
    let segment_markers = collect_segment_marker_shapes(file, groups, &analysis.raw_anchors);
    let measurements = if analysis.graph_mode {
        collect_line_shapes(
            file,
            groups,
            &analysis.raw_anchors,
            &[crate::format::GroupKind::MeasurementLine],
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
            &BTreeSet::new(),
        )
    } else {
        Vec::new()
    };
    let mut lines = segments
        .into_iter()
        .chain(boundary_lines)
        .chain(lines)
        .chain(rays)
        .chain(collect_translated_line_shapes(
            file,
            groups,
            context,
            &analysis.raw_anchors,
        ))
        .chain(segment_markers)
        .chain(collect_rotated_line_shapes(
            file,
            groups,
            context,
            &analysis.raw_anchors,
        ))
        .chain(collect_scaled_line_shapes(
            file,
            groups,
            context,
            &analysis.raw_anchors,
        ))
        .chain(collect_reflected_line_shapes(
            file,
            groups,
            context,
            &analysis.raw_anchors,
        ))
        .chain(measurements)
        .collect::<Vec<_>>();
    let trace_lines = coordinate_traces;
    let base_polygons = collect_polygon_shapes(
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
    let base_circles = collect_circle_shapes(file, groups, &analysis.raw_anchors);
    let arcs = collect_three_point_arc_shapes(file, groups, &analysis.raw_anchors);
    let iteration_lines = Vec::new();
    let iteration_polygons = Vec::new();
    let synthetic_axes = synthesize_axes_if_needed(analysis, &axes);
    let carried_iteration_lines = collect_carried_iteration_lines(
        file,
        groups,
        &analysis.raw_anchors,
        &suppressed_segment_groups,
    );

    lines.shrink_to_fit();
    let post_function_lines = synthetic_axes
        .into_iter()
        .chain(iteration_lines)
        .chain(carried_iteration_lines)
        .collect::<Vec<_>>();
    let polygons = base_polygons
        .into_iter()
        .chain(collect_translated_polygon_shapes(
            file,
            groups,
            context,
            &analysis.raw_anchors,
        ))
        .chain(collect_rotated_polygon_shapes(
            file,
            groups,
            context,
            &analysis.raw_anchors,
        ))
        .chain(collect_transformed_polygon_shapes(
            file,
            groups,
            context,
            &analysis.raw_anchors,
        ))
        .chain(collect_reflected_polygon_shapes(
            file,
            groups,
            context,
            &analysis.raw_anchors,
        ))
        .chain(iteration_polygons)
        .chain(collect_carried_iteration_polygons(
            file,
            groups,
            &analysis.raw_anchors,
        ))
        .collect::<Vec<_>>();
    let circles = base_circles
        .into_iter()
        .chain(collect_carried_iteration_circles(
            file,
            groups,
            &analysis.raw_anchors,
        ))
        .chain(collect_translated_circle_shapes(
            file,
            groups,
            context,
            &analysis.raw_anchors,
        ))
        .chain(collect_rotated_circle_shapes(
            file,
            groups,
            context,
            &analysis.raw_anchors,
        ))
        .chain(collect_transformed_circle_shapes(
            file,
            groups,
            context,
            &analysis.raw_anchors,
        ))
        .chain(collect_reflected_circle_shapes(
            file,
            groups,
            context,
            &analysis.raw_anchors,
        ))
        .collect::<Vec<_>>();

    CollectedShapes {
        lines,
        trace_lines,
        axes,
        polygons,
        circles,
        arcs,
        post_function_lines,
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
