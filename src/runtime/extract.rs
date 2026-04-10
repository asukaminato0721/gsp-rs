use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::Path;

use anyhow::{Context, Result, bail};

mod assemble;
mod buttons;
mod decode;
mod graph;
mod images;
mod labels;
mod points;
mod shapes;
#[cfg(test)]
mod tests;
mod trace;
mod world;

use self::assemble::{assemble_scene, build_world_data, compute_scene_bounds};
use self::buttons::collect_buttons;
use crate::format::{
    GroupKind, GspFile, ObjectGroup, PointRecord, Record, collect_strings, decode_c_string,
    decode_indexed_path, decode_point_record, read_f64, read_u16, read_u32, record_name,
};
use crate::util::{hex_bytes, truncate_text};

use self::graph::{
    collect_document_canvas_bounds, collect_saved_viewport, detect_graph_transform,
    has_graph_classes,
};
use self::images::collect_scene_images;
use self::labels::{
    PendingLabelHotspot, collect_circle_parameter_labels, collect_coordinate_labels,
    collect_custom_transform_expression_labels, collect_iteration_tables, collect_label_iterations,
    collect_labels, collect_polygon_parameter_labels, collect_segment_parameter_labels,
    compute_iteration_labels, resolve_label_hotspots,
};
use self::points::{
    TransformBindingKind, collect_non_graph_parameters, collect_point_iteration_points,
    collect_point_objects, collect_visible_points, decode_line_midpoint_anchor_raw,
    decode_offset_anchor_raw, decode_parameter_controlled_anchor_raw,
    decode_parameter_controlled_point, decode_point_constraint,
    decode_parameter_rotation_anchor_raw, decode_parameter_rotation_binding,
    decode_point_constraint_anchor, decode_point_on_ray_anchor_raw,
    decode_point_pair_translation_anchor_raw, decode_reflection_anchor_raw,
    decode_regular_polygon_vertex_anchor_raw, decode_transform_binding,
    decode_translated_point_anchor_raw, reflection_line_group_indices,
    regular_polygon_iteration_step, remap_circle_bindings, remap_label_bindings,
    remap_line_bindings, remap_polygon_bindings, translation_point_pair_group_indices,
};
use self::shapes::{
    collect_arc_boundary_shapes, collect_bound_line_shapes, collect_carried_iteration_lines,
    collect_carried_iteration_circles, collect_carried_iteration_polygons,
    collect_carried_line_iteration_families,
    collect_carried_polygon_edge_segment_groups, collect_carried_polygon_iteration_families,
    collect_circle_shapes, collect_coordinate_traces, collect_derived_segments,
    collect_iteration_shapes, collect_line_shapes, collect_polygon_shapes,
    collect_raw_object_anchors, collect_reflected_circle_shapes, collect_reflected_line_shapes,
    collect_reflected_polygon_shapes, collect_rotated_circle_shapes, collect_rotated_line_shapes,
    collect_rotated_polygon_shapes, collect_rotational_iteration_lines,
    collect_rotational_iteration_segment_groups, collect_scaled_line_shapes,
    collect_segment_marker_shapes, collect_three_point_arc_shapes,
    collect_transformed_circle_shapes, collect_transformed_polygon_shapes,
    collect_translated_line_shapes, collect_translated_polygon_shapes,
};
use self::trace::collect_point_traces;
use super::functions::{
    collect_function_plot_domain, collect_function_plots, collect_scene_functions,
    collect_scene_parameters, function_uses_pi_scale, synthesize_function_axes,
    synthesize_function_labels,
};
use super::geometry::{Bounds, GraphTransform, distance_world};
use super::scene::{
    LabelIterationFamily, LineIterationFamily, LineShape, PointIterationFamily,
    PolygonIterationFamily, PolygonShape, Scene, ScenePoint, TextLabel,
};

pub(crate) use self::decode::{
    decode_parameter_control_value_for_group, find_indexed_path, is_circle_group_kind,
};

#[derive(Debug, Clone)]
struct CircleShape {
    center: PointRecord,
    radius_point: PointRecord,
    color: [u8; 4],
    fill_color: Option<[u8; 4]>,
    dashed: bool,
    visible: bool,
    binding: Option<super::scene::ShapeBinding>,
}

#[derive(Debug, Clone)]
struct ArcShape {
    points: [PointRecord; 3],
    color: [u8; 4],
    center: Option<PointRecord>,
    counterclockwise: bool,
    visible: bool,
}

struct SceneAnalysis {
    graph_mode: bool,
    graph_ref: Option<GraphTransform>,
    saved_viewport: Option<Bounds>,
    document_viewport: Option<Bounds>,
    pi_mode: bool,
    function_plot_domain: Option<(f64, f64)>,
    function_plots: Vec<LineShape>,
    has_function_plots: bool,
    has_coordinate_objects: bool,
    has_iteration_helpers: bool,
    large_non_graph: bool,
    raw_anchors: Vec<Option<PointRecord>>,
}

struct CollectedShapes {
    polylines: Vec<LineShape>,
    direct_lines: Vec<LineShape>,
    rays: Vec<LineShape>,
    translated_lines: Vec<LineShape>,
    segment_markers: Vec<LineShape>,
    derived_segments: Vec<LineShape>,
    rotated_lines: Vec<LineShape>,
    scaled_lines: Vec<LineShape>,
    reflected_lines: Vec<LineShape>,
    rotational_iteration_lines: Vec<LineShape>,
    carried_iteration_lines: Vec<LineShape>,
    carried_iteration_polygons: Vec<PolygonShape>,
    carried_iteration_circles: Vec<CircleShape>,
    measurements: Vec<LineShape>,
    coordinate_traces: Vec<LineShape>,
    axes: Vec<LineShape>,
    iteration_polygon_indices: BTreeSet<usize>,
    polygons: Vec<PolygonShape>,
    circles: Vec<CircleShape>,
    arcs: Vec<ArcShape>,
    rotated_circles: Vec<CircleShape>,
    transformed_circles: Vec<CircleShape>,
    reflected_circles: Vec<CircleShape>,
    translated_polygons: Vec<PolygonShape>,
    rotated_polygons: Vec<PolygonShape>,
    transformed_polygons: Vec<PolygonShape>,
    reflected_polygons: Vec<PolygonShape>,
    iteration_lines: Vec<LineShape>,
    iteration_polygons: Vec<PolygonShape>,
    synthetic_axes: Vec<LineShape>,
}

struct BindingMaps {
    circle_group_to_index: Vec<Option<usize>>,
    polygon_group_to_index: Vec<Option<usize>>,
    line_group_to_index: Vec<Option<usize>>,
}

#[derive(Debug, Clone)]
struct UnsupportedPayloadIssue {
    summary: String,
    group_ordinals: Vec<usize>,
}

struct WorldData {
    world_points: Vec<ScenePoint>,
    world_point_positions: Vec<PointRecord>,
    point_iterations: Vec<PointIterationFamily>,
}

struct BoundsData {
    bounds: Bounds,
    use_saved_viewport: bool,
}

fn analyze_scene(
    file: &GspFile,
    groups: &[ObjectGroup],
    point_map: &[Option<PointRecord>],
) -> SceneAnalysis {
    let raw_anchors_for_graph = collect_raw_object_anchors(file, groups, point_map, None);
    let graph = detect_graph_transform(file, groups, &raw_anchors_for_graph);
    let graph_mode = graph.is_some() && has_graph_classes(groups);
    let graph_ref = if graph_mode { graph.clone() } else { None };
    let raw_anchors = collect_raw_object_anchors(file, groups, point_map, graph_ref.as_ref());
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
    let document_viewport = if !graph_mode && has_rich_text_layout {
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
    let has_iteration_helpers = groups
        .iter()
        .any(|group| group.header.kind().is_iteration_helper());
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
        has_iteration_helpers,
        large_non_graph,
        raw_anchors,
    }
}

fn collect_scene_shapes(
    file: &GspFile,
    groups: &[ObjectGroup],
    point_map: &[Option<PointRecord>],
    analysis: &SceneAnalysis,
) -> CollectedShapes {
    let mut suppressed_segment_groups = collect_carried_polygon_edge_segment_groups(file, groups);
    suppressed_segment_groups.extend(collect_rotational_iteration_segment_groups(file, groups));
    let polylines = collect_line_shapes(
        file,
        groups,
        &analysis.raw_anchors,
        &[
            crate::format::GroupKind::Segment,
            crate::format::GroupKind::AngleMarker,
        ],
        !analysis.graph_mode && !analysis.large_non_graph,
        &suppressed_segment_groups,
    );
    let boundary_lines = collect_arc_boundary_shapes(file, groups, &analysis.raw_anchors);
    let direct_lines = collect_bound_line_shapes(
        file,
        groups,
        &analysis.raw_anchors,
        crate::format::GroupKind::Line,
    );
    let rays = collect_bound_line_shapes(
        file,
        groups,
        &analysis.raw_anchors,
        crate::format::GroupKind::Ray,
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
    let rotational_iteration_lines =
        collect_rotational_iteration_lines(file, groups, &analysis.raw_anchors);
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
    let iteration_polygon_indices = collect_iteration_polygon_indices(file, groups);
    let polygons = collect_polygon_shapes(
        file,
        groups,
        &analysis.raw_anchors,
        &[crate::format::GroupKind::Polygon],
    )
    .into_iter()
    .enumerate()
    .filter_map(|(ordinal, polygon)| {
        let group_index = groups
            .iter()
            .enumerate()
            .filter(|(_, group)| (group.header.kind()) == crate::format::GroupKind::Polygon)
            .nth(ordinal)
            .map(|(index, _)| index)?;
        (!iteration_polygon_indices.contains(&group_index)).then_some(polygon)
    })
    .collect::<Vec<_>>();
    let circles = collect_circle_shapes(file, groups, &analysis.raw_anchors);
    let arcs = collect_three_point_arc_shapes(file, groups, &analysis.raw_anchors);
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
        polylines: polylines.into_iter().chain(boundary_lines).collect(),
        direct_lines,
        rays,
        translated_lines,
        segment_markers,
        derived_segments,
        rotated_lines,
        scaled_lines,
        reflected_lines,
        rotational_iteration_lines,
        carried_iteration_lines,
        carried_iteration_polygons,
        carried_iteration_circles,
        measurements,
        coordinate_traces,
        axes,
        iteration_polygon_indices,
        polygons,
        circles,
        arcs,
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

fn collect_iteration_polygon_indices(file: &GspFile, groups: &[ObjectGroup]) -> BTreeSet<usize> {
    groups
        .iter()
        .filter(|group| (group.header.kind()) == crate::format::GroupKind::RegularPolygonIteration)
        .filter_map(|group| find_indexed_path(file, group))
        .flat_map(|path| path.refs)
        .filter_map(|obj_ref| {
            let index = obj_ref.checked_sub(1)?;
            let group = groups.get(index)?;
            ((group.header.kind()) == crate::format::GroupKind::Polygon).then_some(index)
        })
        .collect()
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

fn collect_scene_labels(
    file: &GspFile,
    groups: &[ObjectGroup],
    analysis: &SceneAnalysis,
    shapes: &CollectedShapes,
) -> (
    Vec<TextLabel>,
    BTreeMap<usize, usize>,
    Vec<PendingLabelHotspot>,
) {
    let (mut labels, label_group_to_index, mut pending_hotspots) = collect_labels(
        file,
        groups,
        &analysis.raw_anchors,
        analysis.graph_mode,
        !analysis.has_function_plots && !analysis.has_coordinate_objects,
    );
    if analysis.has_coordinate_objects || analysis.has_iteration_helpers {
        labels.extend(collect_coordinate_labels(file, groups));
    }
    labels.extend(collect_polygon_parameter_labels(
        file,
        groups,
        &analysis.raw_anchors,
    ));
    labels.extend(collect_segment_parameter_labels(file, groups));
    labels.extend(collect_custom_transform_expression_labels(
        file,
        groups,
        &analysis.raw_anchors,
    ));
    labels.extend(collect_circle_parameter_labels(
        file,
        groups,
        &analysis.raw_anchors,
    ));
    labels.extend(compute_iteration_labels(file, groups, &shapes.circles));
    if analysis.graph_mode && analysis.has_function_plots {
        labels.extend(synthesize_function_labels(
            file,
            groups,
            &analysis.function_plots,
            analysis.saved_viewport,
            &analysis.graph_ref,
        ));
    }
    append_circle_perimeter_label(
        &mut labels,
        &mut pending_hotspots,
        &shapes.circles,
        analysis,
    );
    (labels, label_group_to_index, pending_hotspots)
}

fn append_circle_perimeter_label(
    labels: &mut Vec<TextLabel>,
    pending_hotspots: &mut [PendingLabelHotspot],
    circles: &[CircleShape],
    analysis: &SceneAnalysis,
) {
    if analysis.graph_mode
        && let (Some(circle), Some(formula_index), Some(transform)) = (
            circles.first(),
            labels.iter().position(|label| label.text.contains("AB:")),
            analysis.graph_ref.as_ref(),
        )
    {
        let circumference = 2.0
            * std::f64::consts::PI
            * distance_world(&circle.center, &circle.radius_point, &analysis.graph_ref);
        let anchor = PointRecord {
            x: labels[formula_index].anchor.x,
            y: labels[formula_index].anchor.y - 0.9 * transform.raw_per_unit,
        };
        for hotspot in pending_hotspots.iter_mut() {
            if hotspot.label_index >= formula_index {
                hotspot.label_index += 1;
            }
        }
        labels.insert(
            formula_index,
            TextLabel {
                anchor,
                text: format!("AB perimeter = {:.2} cm", circumference),
                rich_markup: None,
                color: [30, 30, 30, 255],
                visible: true,
                binding: None,
                screen_space: false,
                hotspots: Vec::new(),
            },
        );
    }
}

fn group_shape_index_map<F>(groups: &[ObjectGroup], predicate: F) -> Vec<Option<usize>>
where
    F: Fn(usize, &ObjectGroup) -> bool,
{
    groups
        .iter()
        .enumerate()
        .filter(|(index, group)| predicate(*index, group))
        .enumerate()
        .fold(
            vec![None; groups.len()],
            |mut acc, (shape_index, (group_index, _))| {
                acc[group_index] = Some(shape_index);
                acc
            },
        )
}

fn remap_scene_bindings(
    file: &GspFile,
    groups: &[ObjectGroup],
    raw_anchors: &[Option<PointRecord>],
    group_to_point_index: &[Option<usize>],
    shapes: &mut CollectedShapes,
) -> (
    BindingMaps,
    Vec<LineIterationFamily>,
    Vec<PolygonIterationFamily>,
) {
    let suppressed_carried_polygon_segments =
        collect_carried_polygon_edge_segment_groups(file, groups);
    let circle_group_to_index =
        group_shape_index_map(groups, |_, group| is_circle_group_kind(group.header.kind()));
    remap_circle_bindings(
        &mut shapes.circles,
        group_to_point_index,
        &circle_group_to_index,
    );
    let polygon_group_to_index = group_shape_index_map(groups, |index, group| {
        (group.header.kind()) == crate::format::GroupKind::Polygon
            && !shapes.iteration_polygon_indices.contains(&index)
    });
    remap_polygon_bindings(
        &mut shapes.polygons,
        group_to_point_index,
        &polygon_group_to_index,
    );
    remap_circle_bindings(
        &mut shapes.rotated_circles,
        group_to_point_index,
        &circle_group_to_index,
    );
    remap_circle_bindings(
        &mut shapes.transformed_circles,
        group_to_point_index,
        &circle_group_to_index,
    );
    remap_circle_bindings(
        &mut shapes.reflected_circles,
        group_to_point_index,
        &circle_group_to_index,
    );
    remap_polygon_bindings(
        &mut shapes.translated_polygons,
        group_to_point_index,
        &polygon_group_to_index,
    );
    remap_polygon_bindings(
        &mut shapes.rotated_polygons,
        group_to_point_index,
        &polygon_group_to_index,
    );
    remap_polygon_bindings(
        &mut shapes.transformed_polygons,
        group_to_point_index,
        &polygon_group_to_index,
    );
    remap_polygon_bindings(
        &mut shapes.reflected_polygons,
        group_to_point_index,
        &polygon_group_to_index,
    );
    let line_group_to_index = group_shape_index_map(groups, |_, group| {
        group.header.kind().is_rendered_line_group()
    });
    remap_line_bindings(
        &mut shapes.polylines,
        group_to_point_index,
        &line_group_to_index,
    );
    remap_line_bindings(
        &mut shapes.direct_lines,
        group_to_point_index,
        &line_group_to_index,
    );
    remap_line_bindings(&mut shapes.rays, group_to_point_index, &line_group_to_index);
    remap_line_bindings(
        &mut shapes.translated_lines,
        group_to_point_index,
        &line_group_to_index,
    );
    remap_line_bindings(
        &mut shapes.segment_markers,
        group_to_point_index,
        &line_group_to_index,
    );
    remap_line_bindings(
        &mut shapes.rotated_lines,
        group_to_point_index,
        &line_group_to_index,
    );
    remap_line_bindings(
        &mut shapes.scaled_lines,
        group_to_point_index,
        &line_group_to_index,
    );
    remap_line_bindings(
        &mut shapes.reflected_lines,
        group_to_point_index,
        &line_group_to_index,
    );
    remap_line_bindings(
        &mut shapes.rotational_iteration_lines,
        group_to_point_index,
        &line_group_to_index,
    );
    remap_line_bindings(
        &mut shapes.coordinate_traces,
        group_to_point_index,
        &line_group_to_index,
    );
    let line_iterations = collect_carried_line_iteration_families(
        file,
        groups,
        raw_anchors,
        group_to_point_index,
        &line_group_to_index,
        &suppressed_carried_polygon_segments,
    );
    let polygon_iterations =
        collect_carried_polygon_iteration_families(file, groups, raw_anchors, group_to_point_index);

    (
        BindingMaps {
            circle_group_to_index,
            polygon_group_to_index,
            line_group_to_index,
        },
        line_iterations,
        polygon_iterations,
    )
}

pub(crate) fn build_scene(file: &GspFile) -> Scene {
    build_scene_checked(file).unwrap_or_else(|error| panic!("{error:#}"))
}

pub(crate) fn build_scene_checked(file: &GspFile) -> Result<Scene> {
    let groups = file.object_groups();
    validate_scene_payloads(file, &groups)?;
    let point_map = collect_point_objects(file, &groups);
    let analysis = analyze_scene(file, &groups, &point_map);
    let mut shapes = collect_scene_shapes(file, &groups, &point_map, &analysis);
    let images = collect_scene_images(file, &groups, &analysis.graph_ref);
    let (mut labels, label_group_to_index, pending_hotspots) =
        collect_scene_labels(file, &groups, &analysis, &shapes);

    let (visible_points, group_to_point_index) = collect_visible_points(
        file,
        &groups,
        &point_map,
        &analysis.raw_anchors,
        &analysis.graph_ref,
    );
    shapes.coordinate_traces.extend(collect_point_traces(
        file,
        &groups,
        &visible_points,
        &group_to_point_index,
    ));
    let (derived_iteration_points, raw_point_iterations) =
        collect_point_iteration_points(file, &groups, &analysis.raw_anchors, &group_to_point_index);
    let label_iterations =
        collect_label_iterations(file, &groups, &label_group_to_index, &group_to_point_index)
            .into_iter()
            .map(|family| match family {
                LabelIterationFamily::PointExpression {
                    seed_label_index,
                    point_seed_index,
                    parameter_name,
                    expr,
                    depth,
                    depth_parameter_name,
                } => LabelIterationFamily::PointExpression {
                    seed_label_index,
                    point_seed_index,
                    parameter_name,
                    expr,
                    depth,
                    depth_parameter_name,
                },
            })
            .collect::<Vec<_>>();
    let iteration_tables = collect_iteration_tables(file, &groups);
    remap_label_bindings(&mut labels, &group_to_point_index);
    let (binding_maps, line_iterations, polygon_iterations) = remap_scene_bindings(
        file,
        &groups,
        &analysis.raw_anchors,
        &group_to_point_index,
        &mut shapes,
    );
    let world_data = build_world_data(
        &analysis,
        &visible_points,
        &derived_iteration_points,
        raw_point_iterations,
    );
    let bounds_data = compute_scene_bounds(
        &analysis,
        &shapes,
        &labels,
        &world_data.world_point_positions,
    );

    let mut parameters = if analysis.graph_mode {
        collect_scene_parameters(file, &groups, &labels)
    } else {
        Vec::new()
    };
    parameters.extend(collect_non_graph_parameters(file, &groups, &mut labels));
    let (buttons, button_group_to_index) = collect_buttons(
        file,
        &groups,
        &analysis.raw_anchors,
        &group_to_point_index,
        &binding_maps.line_group_to_index,
        &binding_maps.circle_group_to_index,
        &binding_maps.polygon_group_to_index,
    );
    resolve_label_hotspots(
        file,
        &groups,
        &mut labels,
        &pending_hotspots,
        &group_to_point_index,
        &binding_maps.circle_group_to_index,
        &binding_maps.polygon_group_to_index,
        &button_group_to_index,
    );
    let functions = if analysis.graph_mode {
        collect_scene_functions(
            file,
            &groups,
            &labels,
            &world_data.world_points,
            shapes.polylines.len()
                + shapes.derived_segments.len()
                + shapes.measurements.len()
                + shapes.axes.len(),
        )
    } else {
        Vec::new()
    };
    Ok(assemble_scene(
        analysis,
        shapes,
        labels,
        world_data,
        bounds_data,
        line_iterations,
        polygon_iterations,
        label_iterations,
        iteration_tables,
        buttons,
        images,
        parameters,
        functions,
    ))
}

fn validate_scene_payloads(file: &GspFile, groups: &[ObjectGroup]) -> Result<()> {
    let issues = collect_unsupported_payload_issues(file, groups);
    if issues.is_empty() {
        return Ok(());
    }
    bail!(
        "unsupported payloads:\n- {}",
        issues
            .iter()
            .map(|issue| issue.summary.as_str())
            .collect::<Vec<_>>()
            .join("\n- ")
    )
}

pub(crate) fn render_unsupported_payload_log(source_path: &Path, file: &GspFile) -> Option<String> {
    let groups = file.object_groups();
    let issues = collect_unsupported_payload_issues(file, &groups);
    if issues.is_empty() {
        return None;
    }

    let mut output = String::new();
    let _ = writeln!(output, "Unsupported payload log");
    let _ = writeln!(output, "file: {}", source_path.display());
    let _ = writeln!(output, "issues: {}", issues.len());
    let _ = writeln!(output, "object_groups: {}", groups.len());
    let _ = writeln!(output);
    let _ = writeln!(output, "Issues");

    for (index, issue) in issues.iter().enumerate() {
        let _ = writeln!(output, "{}. {}", index + 1, issue.summary);
        for ordinal in &issue.group_ordinals {
            if let Some(group) = groups.get(ordinal.saturating_sub(1)) {
                write_group_detail(&mut output, file, group);
            }
        }
    }

    Some(output)
}

fn collect_unsupported_payload_issues(
    file: &GspFile,
    groups: &[ObjectGroup],
) -> Vec<UnsupportedPayloadIssue> {
    let mut issues = Vec::new();
    for group in groups {
        collect_validation_issue(&mut issues, &[group.ordinal], validate_group_kind(group));
        collect_validation_issue(
            &mut issues,
            &[group.ordinal],
            validate_action_button_payload(file, group),
        );
        collect_validation_issue(
            &mut issues,
            &[group.ordinal],
            validate_image_payload(file, group),
        );
        collect_validation_issue(
            &mut issues,
            &function_issue_group_ordinals(file, groups, group),
            validate_function_payload(file, groups, group),
        );
    }
    issues
}

fn function_issue_group_ordinals(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
) -> Vec<usize> {
    let mut ordinals = vec![group.ordinal];
    if group.header.kind() != GroupKind::FunctionPlot {
        return ordinals;
    }
    if let Some(path) = find_indexed_path(file, group)
        && let Some(definition_ordinal) = path.refs.first().copied()
        && definition_ordinal != group.ordinal
        && groups.get(definition_ordinal.saturating_sub(1)).is_some()
    {
        ordinals.push(definition_ordinal);
    }
    ordinals
}

fn collect_validation_issue(
    issues: &mut Vec<UnsupportedPayloadIssue>,
    group_ordinals: &[usize],
    result: Result<()>,
) {
    if let Err(error) = result {
        issues.push(UnsupportedPayloadIssue {
            summary: format!("{error:#}"),
            group_ordinals: group_ordinals.to_vec(),
        });
    }
}

fn validate_group_kind(group: &ObjectGroup) -> Result<()> {
    let kind = group.header.kind();
    if matches!(kind, GroupKind::Unknown(20) | GroupKind::Unknown(71) | GroupKind::Unknown(122))
        || is_supported_group_kind(kind)
    {
        return Ok(());
    }
    if let GroupKind::Unknown(raw) = kind {
        bail!(
            "unsupported payload: unknown object kind {raw} in {}",
            describe_group(group)
        );
    }
    Ok(())
}

fn validate_action_button_payload(file: &GspFile, group: &ObjectGroup) -> Result<()> {
    if !decode::is_action_button_group(group) {
        return Ok(());
    }

    let payload = group_record_payload(file, group, 0x0906, "action button payload")?;
    if payload.len() < 16 {
        bail!(
            "unsupported payload: action button payload too short ({} bytes) in {}",
            payload.len(),
            describe_group(group)
        );
    }

    let action_kind = (read_u16(payload, 12), read_u16(payload, 14));
    if matches!(
        action_kind,
        (2, 0)
            | (4, 0)
            | (7, 0)
            | (3, 1)
            | (3, 3)
            | (0, 7)
            | (1, 7)
            | (1, 3)
            | (0, 3)
    ) {
        return Ok(());
    }

    bail!(
        "unsupported payload: action button uses unsupported action kind ({}, {}) in {}",
        action_kind.0,
        action_kind.1,
        describe_group(group)
    )
}

fn validate_image_payload(file: &GspFile, group: &ObjectGroup) -> Result<()> {
    if group.header.kind() != GroupKind::Point {
        return Ok(());
    }

    let has_image_records = [0x090c, 0x08a8, 0x1f44].into_iter().any(|record_type| {
        group
            .records
            .iter()
            .any(|record| record.record_type == record_type)
    });
    if !has_image_records {
        return Ok(());
    }

    let size_payload = group_record_payload(file, group, 0x090c, "image size payload")?;
    let transform_payload = group_record_payload(file, group, 0x08a8, "image transform payload")?;
    let resource_payload = group_record_payload(file, group, 0x1f44, "image resource payload")?;
    if size_payload.len() < 8 || transform_payload.len() < 48 || resource_payload.len() < 2 {
        bail!(
            "unsupported payload: malformed image payload in {} (size={}, transform={}, resource={})",
            describe_group(group),
            size_payload.len(),
            transform_payload.len(),
            resource_payload.len()
        );
    }

    let width = read_u32(size_payload, 0) as f64;
    let height = read_u32(size_payload, 4) as f64;
    if width <= 0.0 || height <= 0.0 {
        bail!(
            "unsupported payload: non-positive image dimensions ({width}x{height}) in {}",
            describe_group(group)
        );
    }

    let shear_x = read_f64(transform_payload, 8);
    let shear_y = read_f64(transform_payload, 24);
    if !shear_x.is_finite() || !shear_y.is_finite() {
        bail!(
            "unsupported payload: non-finite image transform in {}",
            describe_group(group)
        );
    }
    if shear_x.abs() > 1e-6 || shear_y.abs() > 1e-6 {
        bail!(
            "unsupported payload: non-axis-aligned image transform in {}",
            describe_group(group)
        );
    }

    Ok(())
}

fn validate_function_payload(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
) -> Result<()> {
    if group.header.kind() != GroupKind::FunctionPlot {
        return Ok(());
    }

    let path = find_indexed_path(file, group).with_context(|| {
        format!(
            "unsupported payload: function plot is missing indexed path in {}",
            describe_group(group)
        )
    })?;
    if path.refs.len() < 2 {
        bail!(
            "unsupported payload: function plot path has {} refs in {}",
            path.refs.len(),
            describe_group(group)
        );
    }

    let definition_ordinal = path.refs[0];
    let definition_group = groups
        .get(definition_ordinal.checked_sub(1).context("function plot definition ordinal underflow")?)
        .with_context(|| {
            format!(
                "unsupported payload: function plot references missing definition group #{definition_ordinal} from {}",
                describe_group(group)
            )
        })?;
    let descriptor_payload = group_record_payload(file, group, 0x0902, "function plot descriptor")?;
    super::functions::decode_function_plot_descriptor(descriptor_payload).with_context(|| {
        format!(
            "unsupported payload: invalid function plot descriptor in {}",
            describe_group(group)
        )
    })?;
    super::functions::decode_function_expr(file, groups, definition_group).with_context(|| {
        format!(
            "unsupported payload: invalid function expression in {} referenced by {}",
            describe_group(definition_group),
            describe_group(group)
        )
    })?;

    Ok(())
}

fn group_record_payload<'a>(
    file: &'a GspFile,
    group: &'a ObjectGroup,
    record_type: u32,
    record_label: &str,
) -> Result<&'a [u8]> {
    group
        .records
        .iter()
        .find(|record| record.record_type == record_type)
        .map(|record| record.payload(&file.data))
        .with_context(|| {
            format!(
                "unsupported payload: missing {record_label} (record 0x{record_type:04x}) in {}",
                describe_group(group)
            )
        })
}

fn write_group_detail(output: &mut String, file: &GspFile, group: &ObjectGroup) {
    let _ = writeln!(output, "  group #{}:", group.ordinal);
    let _ = writeln!(
        output,
        "    type: {:?} (raw=0x{:04x}, class_id=0x{:08x})",
        group.header.kind(),
        group.header.kind_id(),
        group.header.class_id
    );
    let _ = writeln!(
        output,
        "    geometry: hidden={} flags=0x{:08x} style=[0x{:08x}, 0x{:08x}, 0x{:08x}]",
        group.header.is_hidden(),
        group.header.flags,
        group.header.style_a,
        group.header.style_b,
        group.header.style_c
    );
    let _ = writeln!(
        output,
        "    offsets: start=0x{:x} end=0x{:x}",
        group.start_offset, group.end_offset
    );

    if let Some(name) = self::decode::decode_label_name_raw(file, group) {
        let _ = writeln!(output, "    name: {:?}", name);
    }
    if let Some(text) = self::decode::decode_group_label_text(file, group) {
        let _ = writeln!(output, "    label_text: {:?}", text);
    }
    if let Some(url) = self::decode::decode_link_button_url(file, group) {
        let _ = writeln!(output, "    action_url: {:?}", url);
    }
    if let Some(path) = find_indexed_path(file, group) {
        let _ = writeln!(output, "    indexed_refs: {:?}", path.refs);
    }
    if let Some(anchor) = self::decode::decode_0907_anchor(file, group) {
        let _ = writeln!(
            output,
            "    anchor_point: ({:.3}, {:.3})",
            anchor.x, anchor.y
        );
    }

    let points = group
        .records
        .iter()
        .filter(|record| record.record_type == 0x0899)
        .filter_map(|record| decode_point_record(record.payload(&file.data)))
        .take(3)
        .map(|point| format!("({:.3}, {:.3})", point.x, point.y))
        .collect::<Vec<_>>();
    if !points.is_empty() {
        let _ = writeln!(output, "    points: {}", points.join(", "));
    }

    let strings = collect_group_strings(file, group);
    if !strings.is_empty() {
        let _ = writeln!(output, "    strings: {}", strings.join(" | "));
    }

    let _ = writeln!(output, "    records:");
    for record in &group.records {
        let _ = writeln!(
            output,
            "      - 0x{:04x} {} len={}{}",
            record.record_type,
            record_name(record.record_type),
            record.length,
            format_record_summary(file, record)
                .map(|summary| format!(" {summary}"))
                .unwrap_or_default()
        );
    }
}

fn collect_group_strings(file: &GspFile, group: &ObjectGroup) -> Vec<String> {
    let mut strings = BTreeSet::new();
    for record in &group.records {
        let payload = record.payload(&file.data);
        if let Some(text) = decode_c_string(payload) {
            let text = text.trim();
            if !text.is_empty() {
                strings.insert(format!("{:?}", truncate_text(text, 80)));
            }
        }
        for entry in collect_strings(payload) {
            let text = entry.text.trim();
            if !text.is_empty() {
                strings.insert(format!("{:?}", truncate_text(text, 80)));
            }
        }
    }
    strings.into_iter().take(6).collect()
}

fn format_record_summary(file: &GspFile, record: &Record) -> Option<String> {
    let payload = record.payload(&file.data);
    match record.record_type {
        0x0899 => decode_point_record(payload)
            .map(|point| format!("point=({:.3}, {:.3})", point.x, point.y)),
        0x07d2 | 0x07d3 => decode_indexed_path(record.record_type, payload)
            .map(|path| format!("refs={:?}", path.refs)),
        _ => {
            let strings = collect_strings(payload)
                .into_iter()
                .map(|entry| truncate_text(entry.text.trim(), 48))
                .filter(|text| !text.is_empty())
                .take(2)
                .collect::<Vec<_>>();
            if !strings.is_empty() {
                return Some(format!("strings={strings:?}"));
            }
            decode_c_string(payload)
                .map(|text| format!("text={:?}", truncate_text(text.trim(), 48)))
                .or_else(|| {
                    (payload.len() <= 16 && !payload.is_empty())
                        .then(|| format!("payload={}", hex_bytes(payload)))
                })
        }
    }
}

fn describe_group(group: &ObjectGroup) -> String {
    let record_types = group
        .records
        .iter()
        .map(|record| format!("0x{:04x}", record.record_type))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "group #{} {:?} @ 0x{:x} [{}]",
        group.ordinal,
        group.header.kind(),
        group.start_offset,
        record_types
    )
}

fn is_supported_group_kind(kind: GroupKind) -> bool {
    matches!(
        kind,
        GroupKind::Point
            | GroupKind::Midpoint
            | GroupKind::Segment
            | GroupKind::Circle
            | GroupKind::CircleCenterRadius
            | GroupKind::LineKind5
            | GroupKind::LineKind6
            | GroupKind::LineKind7
            | GroupKind::Polygon
            | GroupKind::LinearIntersectionPoint
            | GroupKind::CircleInterior
            | GroupKind::IntersectionPoint1
            | GroupKind::IntersectionPoint2
            | GroupKind::CircleCircleIntersectionPoint1
            | GroupKind::CircleCircleIntersectionPoint2
            | GroupKind::PointConstraint
            | GroupKind::Translation
            | GroupKind::CartesianOffsetPoint
            | GroupKind::CoordinateExpressionPoint
            | GroupKind::CoordinateExpressionPointAlt
            | GroupKind::PolarOffsetPoint
            | GroupKind::DerivedSegment24
            | GroupKind::CustomTransformPoint
            | GroupKind::Rotation
            | GroupKind::ParameterRotation
            | GroupKind::Scale
            | GroupKind::Reflection
            | GroupKind::PointTrace
            | GroupKind::GraphObject40
            | GroupKind::FunctionExpr
            | GroupKind::Kind51
            | GroupKind::GraphCalibrationX
            | GroupKind::GraphCalibrationY
            | GroupKind::MeasurementLine
            | GroupKind::AxisLine
            | GroupKind::ActionButton
            | GroupKind::Line
            | GroupKind::Ray
            | GroupKind::OffsetAnchor
            | GroupKind::CoordinatePoint
            | GroupKind::FunctionPlot
            | GroupKind::ButtonLabel
            | GroupKind::DerivedSegment75
            | GroupKind::AffineIteration
            | GroupKind::IterationBinding
            | GroupKind::DerivativeFunction
            | GroupKind::ArcOnCircle
            | GroupKind::CenterArc
            | GroupKind::ThreePointArc
            | GroupKind::SectorBoundary
            | GroupKind::CircularSegmentBoundary
            | GroupKind::RegularPolygonIteration
            | GroupKind::LabelIterationSeed
            | GroupKind::IterationExpressionHelper
            | GroupKind::ParameterAnchor
            | GroupKind::ParameterControlledPoint
            | GroupKind::CoordinateTrace
            | GroupKind::CoordinateTraceIntersectionPoint
            | GroupKind::CustomTransformTrace
            | GroupKind::AngleMarker
            | GroupKind::PathPoint
            | GroupKind::SegmentMarker
            | GroupKind::Unknown(71)
            | GroupKind::Unknown(122)
    )
}
