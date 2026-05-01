use super::shapes::{ArcShape, CircleShape};
use super::*;

pub(super) struct SceneAnalysis {
    pub(super) graph_mode: bool,
    pub(super) graph_ref: Option<GraphTransform>,
    pub(super) saved_viewport: Option<Bounds>,
    pub(super) document_viewport: Option<Bounds>,
    pub(super) pi_mode: bool,
    pub(super) function_plot_domain: Option<(f64, f64)>,
    pub(super) function_plots: Vec<LineShape>,
    pub(super) has_function_plots: bool,
    pub(super) has_coordinate_objects: bool,
    pub(super) raw_anchors: Vec<Option<PointRecord>>,
}

pub(super) struct CollectedShapes {
    pub(super) lines: Vec<LineShape>,
    pub(super) trace_lines: Vec<LineShape>,
    pub(super) axes: Vec<LineShape>,
    pub(super) post_function_lines: Vec<LineShape>,
    pub(super) polygons: Vec<PolygonShape>,
    pub(super) circles: Vec<CircleShape>,
    pub(super) arcs: Vec<ArcShape>,
}

pub(super) struct WorldData {
    pub(super) world_points: Vec<ScenePoint>,
    pub(super) world_point_positions: Vec<PointRecord>,
    pub(super) point_iterations: Vec<PointIterationFamily>,
}

pub(super) struct BoundsData {
    pub(super) bounds: Bounds,
    pub(super) use_saved_viewport: bool,
}

pub(super) fn analyze_scene(
    file: &GspFile,
    groups: &[ObjectGroup],
    point_map: &[Option<PointRecord>],
) -> SceneAnalysis {
    let raw_anchors_for_graph = collect_raw_object_anchors(file, groups, point_map, None);
    let graph = detect_graph_transform(file, groups, &raw_anchors_for_graph);
    let graph_mode = graph.is_some() && has_graph_classes(groups);
    let graph_ref = if graph_mode || has_coordinate_transform_consumers(groups) {
        graph.clone()
    } else {
        None
    };
    let raw_anchors = collect_raw_object_anchors(file, groups, point_map, graph.as_ref());
    let saved_viewport = if graph_mode {
        collect_saved_viewport(file, groups)
    } else {
        None
    };
    let has_rich_text_layout = groups.iter().any(|group| {
        group
            .records
            .iter()
            .any(|record| record.record_type == 0x08fc)
    });
    let document_viewport = if !graph_mode && graph_ref.is_none() && has_rich_text_layout {
        collect_document_canvas_bounds(file)
    } else {
        None
    };
    let pi_mode = if graph_mode {
        function_uses_pi_scale(file, groups)
    } else {
        false
    };
    let function_plot_domain = if graph_mode {
        collect_function_plot_domain(file, groups)
    } else {
        None
    };
    let function_plots = if graph_mode {
        collect_function_plots(file, groups, &graph_ref)
    } else {
        Vec::new()
    };
    let has_function_plots = !function_plots.is_empty();
    let has_coordinate_objects = groups
        .iter()
        .any(|group| group.header.kind().is_coordinate_object());

    SceneAnalysis {
        graph_mode,
        graph_ref,
        saved_viewport,
        document_viewport,
        pi_mode,
        function_plot_domain,
        function_plots,
        has_function_plots,
        has_coordinate_objects,
        raw_anchors,
    }
}
