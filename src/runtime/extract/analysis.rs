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
    pub(super) large_non_graph: bool,
    pub(super) raw_anchors: Vec<Option<PointRecord>>,
}

pub(super) struct CollectedShapes {
    pub(super) segments: Vec<LineShape>,
    pub(super) lines: Vec<LineShape>,
    pub(super) rays: Vec<LineShape>,
    pub(super) translated_lines: Vec<LineShape>,
    pub(super) segment_markers: Vec<LineShape>,
    pub(super) derived_segments: Vec<LineShape>,
    pub(super) rotated_lines: Vec<LineShape>,
    pub(super) scaled_lines: Vec<LineShape>,
    pub(super) reflected_lines: Vec<LineShape>,
    pub(super) carried_iteration_lines: Vec<LineShape>,
    pub(super) carried_iteration_polygons: Vec<PolygonShape>,
    pub(super) carried_iteration_circles: Vec<CircleShape>,
    pub(super) measurements: Vec<LineShape>,
    pub(super) coordinate_traces: Vec<LineShape>,
    pub(super) axes: Vec<LineShape>,
    pub(super) polygons: Vec<PolygonShape>,
    pub(super) circles: Vec<CircleShape>,
    pub(super) arcs: Vec<ArcShape>,
    pub(super) translated_circles: Vec<CircleShape>,
    pub(super) rotated_circles: Vec<CircleShape>,
    pub(super) transformed_circles: Vec<CircleShape>,
    pub(super) reflected_circles: Vec<CircleShape>,
    pub(super) translated_polygons: Vec<PolygonShape>,
    pub(super) rotated_polygons: Vec<PolygonShape>,
    pub(super) transformed_polygons: Vec<PolygonShape>,
    pub(super) reflected_polygons: Vec<PolygonShape>,
    pub(super) iteration_lines: Vec<LineShape>,
    pub(super) iteration_polygons: Vec<PolygonShape>,
    pub(super) synthetic_axes: Vec<LineShape>,
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
    context: &SceneContext<'_>,
    point_map: &[Option<PointRecord>],
) -> SceneAnalysis {
    let raw_anchors_for_graph = collect_raw_object_anchors(file, groups, point_map, None);
    let graph = detect_graph_transform(file, groups, &raw_anchors_for_graph);
    let graph_mode = graph.is_some() && has_graph_classes(groups);
    let hidden_graph_transform = !graph_mode
        && graph
            .as_ref()
            .is_some_and(|graph| has_hidden_graph_panel(context, &raw_anchors_for_graph, graph))
        && count_function_coordinate_points(context) >= 10
        && count_polygon_payload_color_bindings(context) >= 10;
    let graph_ref =
        if graph_mode || hidden_graph_transform || has_coordinate_transform_consumers(groups) {
            graph.clone()
        } else {
            None
        };
    let raw_anchors = collect_raw_object_anchors(file, groups, point_map, graph.as_ref());
    let saved_viewport = if graph_mode || hidden_graph_transform {
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
    let large_non_graph = !graph_mode && file.records.len() > 10_000;

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
        large_non_graph,
        raw_anchors,
    }
}

fn count_function_coordinate_points(context: &SceneContext<'_>) -> usize {
    context
        .groups
        .iter()
        .filter(|group| {
            if !group.header.kind().is_coordinate_object() {
                return false;
            }
            context.indexed_path(group).is_some_and(|path| {
                path.refs
                    .iter()
                    .filter_map(|ordinal| context.group_by_ordinal(*ordinal))
                    .any(|source_group| source_group.header.kind() == GroupKind::FunctionExpr)
            })
        })
        .count()
}

pub(super) fn count_polygon_payload_color_bindings(context: &SceneContext<'_>) -> usize {
    context
        .group_indices_by_kind(GroupKind::DerivedSegment24)
        .iter()
        .chain(
            context
                .group_indices_by_kind(GroupKind::DerivedSegment75)
                .iter(),
        )
        .filter_map(|index| context.group(*index))
        .filter(|group| {
            context.indexed_path(group).is_some_and(|path| {
                if path.refs.len() < 4 {
                    return false;
                }
                path.refs
                    .first()
                    .and_then(|ordinal| context.group_by_ordinal(*ordinal))
                    .is_some_and(|host_group| host_group.header.kind() == GroupKind::Polygon)
            })
        })
        .count()
}

fn has_hidden_graph_panel(
    context: &SceneContext<'_>,
    anchors: &[Option<PointRecord>],
    graph: &GraphTransform,
) -> bool {
    context
        .group_indices_by_kind(GroupKind::Polygon)
        .iter()
        .filter_map(|index| context.group(*index))
        .any(|group| {
            !group.header.is_hidden()
                && color_from_style(group.header.style_b) == [0, 0, 0, 255]
                && context.indexed_path(group).is_some_and(|path| {
                    if path.refs.len() != 4 {
                        return false;
                    }
                    let points = path
                        .refs
                        .iter()
                        .filter_map(|ordinal| {
                            anchors.get(ordinal.saturating_sub(1)).cloned().flatten()
                        })
                        .map(|point| to_world(&point, &Some(graph.clone())))
                        .collect::<Vec<_>>();
                    points.len() == 4
                        && points.iter().all(|point| {
                            ((point.x - 0.0).abs() < 1e-9 || (point.x - 1.0).abs() < 1e-9)
                                && ((point.y - 0.0).abs() < 1e-9 || (point.y - 1.0).abs() < 1e-9)
                        })
                })
        })
}
