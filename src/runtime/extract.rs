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
pub(crate) mod points;
pub(crate) mod shapes;
#[cfg(test)]
mod tests;
mod trace;
mod world;

use self::assemble::{
    SceneAssemblyArtifacts, assemble_scene, build_world_data, compute_scene_bounds,
};
use self::buttons::{ButtonIndexLookups, collect_buttons};
pub(crate) use self::decode::decode_measurement_value;
use crate::format::{
    GroupKind, GspFile, ObjectGroup, PointRecord, Record, collect_strings, decode_c_string,
    decode_indexed_path, decode_point_record, read_f64, read_u16, read_u32, record_name,
};
use crate::runtime::payload_consts::{
    RECORD_FUNCTION_EXPR_PAYLOAD, RECORD_FUNCTION_PLOT_DESCRIPTOR, RECORD_POINT_F64_PAIR,
};
use crate::util::{hex_bytes, truncate_text};

use self::graph::{
    collect_document_canvas_bounds, collect_saved_viewport, detect_graph_transform,
    has_graph_classes,
};
use self::images::collect_scene_images;
use self::labels::{
    HotspotIndexLookups, PendingLabelHotspot, bind_button_seed_expression_labels, circle_parameter,
    collect_circle_parameter_labels, collect_coordinate_labels,
    collect_custom_transform_expression_labels, collect_iteration_tables, collect_label_iterations,
    collect_labels, collect_polygon_parameter_labels, collect_segment_parameter_labels,
    polygon_boundary_parameter, resolve_label_hotspots,
};
use self::points::{
    RawPointConstraint, TransformBindingKind, collect_non_graph_parameters,
    collect_point_iteration_points, collect_point_objects, collect_standalone_parameter_points,
    collect_visible_points_checked, decode_angle_rotation_anchor_raw,
    decode_line_midpoint_anchor_raw, decode_offset_anchor_raw,
    decode_parameter_controlled_anchor_raw, decode_parameter_rotation_anchor_raw,
    decode_point_constraint_anchor, decode_point_on_ray_anchor_raw,
    decode_point_pair_translation_anchor_raw, decode_reflection_anchor_raw,
    decode_regular_polygon_vertex_anchor_raw, decode_translated_point_anchor_raw,
    decode_translated_point_constraint, regular_polygon_iteration_step, remap_circle_bindings,
    remap_label_bindings, remap_line_bindings, remap_polygon_bindings,
    translation_point_pair_group_indices, try_decode_parameter_controlled_point,
    try_decode_parameter_rotation_binding, try_decode_point_constraint,
    try_decode_transform_binding,
};
use self::shapes::{
    collect_arc_boundary_fill_polygons, collect_arc_boundary_shapes, collect_bound_line_shapes,
    collect_carried_circle_iteration_families, collect_carried_iteration_circles,
    collect_carried_iteration_lines, collect_carried_iteration_polygons,
    collect_carried_line_iteration_families, collect_carried_polygon_edge_segment_groups,
    collect_carried_polygon_iteration_families, collect_circle_shapes, collect_coordinate_traces,
    collect_derived_segments, collect_iteration_shapes, collect_line_shapes,
    collect_materialized_ray_groups, collect_polygon_shapes, collect_raw_object_anchors,
    collect_reflected_circle_shapes, collect_reflected_line_shapes,
    collect_reflected_polygon_shapes, collect_rotated_circle_shapes, collect_rotated_line_shapes,
    collect_rotated_polygon_shapes, collect_rotational_line_iteration_families,
    collect_scaled_line_shapes, collect_segment_marker_shapes, collect_three_point_arc_shapes,
    collect_transformed_circle_shapes, collect_transformed_polygon_shapes,
    collect_translated_circle_shapes, collect_translated_line_shapes,
    collect_translated_polygon_shapes,
};
use self::trace::collect_point_traces;
use super::functions::{
    collect_function_plot_domain, collect_function_plots, collect_scene_functions,
    collect_scene_parameters, function_uses_pi_scale, synthesize_function_axes,
    synthesize_function_labels, try_decode_function_expr, try_decode_function_plot_descriptor,
};
use super::geometry::{Bounds, GraphTransform, distance_world};
use super::scene::{
    ColorBinding, LabelIterationFamily, LineIterationFamily, LineShape, PayloadDebugSource,
    PointIterationFamily, PolygonIterationFamily, PolygonShape, Scene, ScenePoint, TextLabel,
};

pub(crate) use self::decode::{
    find_indexed_path, is_circle_group_kind, try_decode_bbox_rect_raw, try_decode_group_label_text,
    try_decode_group_rich_text, try_decode_link_button_url,
    try_decode_parameter_control_value_for_group, try_decode_payload_anchor_point,
    try_find_indexed_path,
};

#[derive(Debug, Clone)]
struct CircleShape {
    center: PointRecord,
    radius_point: PointRecord,
    color: [u8; 4],
    fill_color: Option<[u8; 4]>,
    fill_color_binding: Option<super::scene::ColorBinding>,
    dashed: bool,
    visible: bool,
    binding: Option<super::scene::ShapeBinding>,
    debug: Option<PayloadDebugSource>,
}

#[derive(Debug, Clone)]
struct ArcShape {
    points: [PointRecord; 3],
    color: [u8; 4],
    center: Option<PointRecord>,
    counterclockwise: bool,
    visible: bool,
    debug: Option<PayloadDebugSource>,
}

pub(super) fn payload_debug_source(group: &ObjectGroup) -> PayloadDebugSource {
    PayloadDebugSource {
        group_ordinal: group.ordinal,
        group_kind: format!("{:?}", group.header.kind()),
        record_types: group
            .records
            .iter()
            .map(|record| record.record_type)
            .collect(),
        record_names: group
            .records
            .iter()
            .map(|record| {
                format!(
                    "0x{:04x} {}",
                    record.record_type,
                    record_name(record.record_type)
                )
            })
            .collect(),
    }
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
    translated_circles: Vec<CircleShape>,
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

fn collect_scene_shapes(
    file: &GspFile,
    groups: &[ObjectGroup],
    point_map: &[Option<PointRecord>],
    analysis: &SceneAnalysis,
) -> CollectedShapes {
    let suppressed_segment_groups = collect_carried_polygon_edge_segment_groups(file, groups);
    let suppressed_ray_groups = collect_materialized_ray_groups(file, groups);
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
        polylines: polylines.into_iter().chain(boundary_lines).collect(),
        direct_lines,
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
        iteration_polygon_indices,
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
    labels.extend(collect_coordinate_labels(
        file,
        groups,
        &analysis.raw_anchors,
    ));
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
                color: [30, 30, 30, 255],
                visible: true,
                screen_space: false,
                ..Default::default()
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

fn circle_group_to_index_map(
    groups: &[ObjectGroup],
    shapes: &CollectedShapes,
) -> Vec<Option<usize>> {
    let mut mapping = vec![None; groups.len()];
    let mut next_index = 0usize;
    for circle in shapes
        .circles
        .iter()
        .chain(shapes.carried_iteration_circles.iter())
        .chain(shapes.translated_circles.iter())
        .chain(shapes.rotated_circles.iter())
        .chain(shapes.transformed_circles.iter())
        .chain(shapes.reflected_circles.iter())
    {
        let Some(group_ordinal) = circle.debug.as_ref().map(|debug| debug.group_ordinal) else {
            next_index += 1;
            continue;
        };
        if let Some(group_index) = group_ordinal.checked_sub(1)
            && group_index < mapping.len()
        {
            mapping[group_index] = Some(next_index);
        }
        next_index += 1;
    }
    mapping
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
    let line_group_to_index = group_shape_index_map(groups, |_, group| {
        group.header.kind().is_rendered_line_group()
    });
    let circle_group_to_index = circle_group_to_index_map(groups, shapes);
    remap_circle_bindings(
        &mut shapes.circles,
        group_to_point_index,
        &circle_group_to_index,
        &line_group_to_index,
    );
    let polygon_group_to_index = group_shape_index_map(groups, |index, group| {
        (group.header.kind()) == crate::format::GroupKind::Polygon
            && !shapes.iteration_polygon_indices.contains(&index)
    });
    remap_polygon_bindings(
        &mut shapes.polygons,
        group_to_point_index,
        &polygon_group_to_index,
        &line_group_to_index,
    );
    remap_circle_bindings(
        &mut shapes.translated_circles,
        group_to_point_index,
        &circle_group_to_index,
        &line_group_to_index,
    );
    remap_circle_bindings(
        &mut shapes.rotated_circles,
        group_to_point_index,
        &circle_group_to_index,
        &line_group_to_index,
    );
    remap_circle_bindings(
        &mut shapes.transformed_circles,
        group_to_point_index,
        &circle_group_to_index,
        &line_group_to_index,
    );
    remap_circle_bindings(
        &mut shapes.reflected_circles,
        group_to_point_index,
        &circle_group_to_index,
        &line_group_to_index,
    );
    remap_polygon_bindings(
        &mut shapes.translated_polygons,
        group_to_point_index,
        &polygon_group_to_index,
        &line_group_to_index,
    );
    remap_polygon_bindings(
        &mut shapes.rotated_polygons,
        group_to_point_index,
        &polygon_group_to_index,
        &line_group_to_index,
    );
    remap_polygon_bindings(
        &mut shapes.transformed_polygons,
        group_to_point_index,
        &polygon_group_to_index,
        &line_group_to_index,
    );
    remap_polygon_bindings(
        &mut shapes.reflected_polygons,
        group_to_point_index,
        &polygon_group_to_index,
        &line_group_to_index,
    );
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
        &mut shapes.coordinate_traces,
        group_to_point_index,
        &line_group_to_index,
    );
    let carried_line_iterations = collect_carried_line_iteration_families(
        file,
        groups,
        raw_anchors,
        group_to_point_index,
        &line_group_to_index,
        &suppressed_carried_polygon_segments,
    );
    let rotational_line_iterations = collect_rotational_line_iteration_families(
        file,
        groups,
        group_to_point_index,
        &line_group_to_index,
    );
    let polygon_iterations =
        collect_carried_polygon_iteration_families(file, groups, raw_anchors, group_to_point_index);
    let mut line_iterations = rotational_line_iterations;
    line_iterations.extend(carried_line_iterations);

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

fn apply_payload_color_bindings(
    file: &GspFile,
    groups: &[ObjectGroup],
    raw_anchors: &[Option<PointRecord>],
    group_to_point_index: &[Option<usize>],
    circle_group_to_index: &[Option<usize>],
    shapes: &mut CollectedShapes,
) {
    for group in groups.iter().filter(|group| {
        matches!(
            group.header.kind(),
            GroupKind::DerivedSegment24 | GroupKind::DerivedSegment75
        )
    }) {
        let Some(path) = find_indexed_path(file, group) else {
            continue;
        };
        if path.refs.len() < 4 {
            continue;
        }

        let Some(host_group_index) = path.refs[0].checked_sub(1) else {
            continue;
        };
        let Some(host_group) = groups.get(host_group_index) else {
            continue;
        };
        if host_group.header.kind() != GroupKind::CircleInterior {
            continue;
        }

        let Some(circle_path) = find_indexed_path(file, host_group) else {
            continue;
        };
        let Some(circle_group_index) = circle_path
            .refs
            .first()
            .and_then(|value| value.checked_sub(1))
        else {
            continue;
        };
        let Some(circle_index) = circle_group_to_index
            .get(circle_group_index)
            .copied()
            .flatten()
        else {
            continue;
        };

        let resolve_parameter_point = |ordinal: usize| -> Option<usize> {
            let anchor_group = groups.get(ordinal.checked_sub(1)?)?;
            let anchor_path = find_indexed_path(file, anchor_group)?;
            let point_group_index = anchor_path
                .refs
                .first()
                .and_then(|value| value.checked_sub(1))?;
            group_to_point_index
                .get(point_group_index)
                .copied()
                .flatten()
        };

        let Some(first_point_index) = resolve_parameter_point(path.refs[3]) else {
            continue;
        };
        let Some(second_point_index) = resolve_parameter_point(path.refs[2]) else {
            continue;
        };
        let Some(third_point_index) = resolve_parameter_point(path.refs[1]) else {
            continue;
        };
        let Some(first_value) =
            resolve_payload_color_parameter_value(file, groups, raw_anchors, path.refs[3])
        else {
            continue;
        };
        let Some(second_value) =
            resolve_payload_color_parameter_value(file, groups, raw_anchors, path.refs[2])
        else {
            continue;
        };
        let Some(third_value) =
            resolve_payload_color_parameter_value(file, groups, raw_anchors, path.refs[1])
        else {
            continue;
        };

        let alpha = shapes
            .circles
            .get(circle_index)
            .and_then(|circle| circle.fill_color.map(|color| color[3]))
            .unwrap_or(255);
        let expected = {
            let color = crate::runtime::geometry::color_from_style(group.header.style_b);
            [color[0], color[1], color[2]]
        };
        let rgb_candidate = normalized_rgb(first_value, second_value, third_value);
        let hsb_candidate = normalized_hsb(first_value, second_value, third_value);
        let (binding, resolved_fill) =
            if color_distance(expected, rgb_candidate) <= color_distance(expected, hsb_candidate) {
                (
                    ColorBinding::Rgb {
                        red_point_index: first_point_index,
                        green_point_index: second_point_index,
                        blue_point_index: third_point_index,
                        alpha,
                    },
                    [rgb_candidate[0], rgb_candidate[1], rgb_candidate[2], alpha],
                )
            } else {
                (
                    ColorBinding::Hsb {
                        hue_point_index: first_point_index,
                        saturation_point_index: second_point_index,
                        brightness_point_index: third_point_index,
                        alpha,
                    },
                    [hsb_candidate[0], hsb_candidate[1], hsb_candidate[2], alpha],
                )
            };

        if let Some(circle) = shapes.circles.get_mut(circle_index) {
            circle.fill_color = Some(resolved_fill);
            circle.fill_color_binding = Some(binding);
        }
    }
}

fn resolve_payload_color_parameter_value(
    file: &GspFile,
    groups: &[ObjectGroup],
    raw_anchors: &[Option<PointRecord>],
    anchor_ordinal: usize,
) -> Option<f64> {
    let anchor_group = groups.get(anchor_ordinal.checked_sub(1)?)?;
    let anchor_path = find_indexed_path(file, anchor_group)?;
    let point_group_index = anchor_path
        .refs
        .first()
        .and_then(|value| value.checked_sub(1))?;
    let point_group = groups.get(point_group_index)?;
    match try_decode_point_constraint(file, groups, point_group, None, &None).ok()? {
        RawPointConstraint::Segment(constraint) => Some(constraint.t),
        RawPointConstraint::ConstructedLine { t, .. } => Some(t),
        RawPointConstraint::PolygonBoundary {
            edge_index,
            t,
            vertex_group_indices,
        } => polygon_boundary_parameter(raw_anchors, &vertex_group_indices, edge_index, t),
        RawPointConstraint::Circle(constraint) => circle_parameter(
            raw_anchors,
            constraint.center_group_index,
            constraint.radius_group_index,
            constraint.unit_x,
            constraint.unit_y,
        ),
        RawPointConstraint::Circular(_) => None,
        RawPointConstraint::CircleArc(constraint) => Some(constraint.t),
        RawPointConstraint::Arc(constraint) => Some(constraint.t),
        RawPointConstraint::Polyline { t, .. } => Some(t),
    }
}

fn color_distance(expected: [u8; 3], candidate: [u8; 3]) -> u32 {
    u32::from(expected[0].abs_diff(candidate[0]))
        + u32::from(expected[1].abs_diff(candidate[1]))
        + u32::from(expected[2].abs_diff(candidate[2]))
}

fn normalized_channel(value: f64) -> u8 {
    (value.clamp(0.0, 1.0) * 255.0).floor() as u8
}

fn normalized_rgb(first: f64, second: f64, third: f64) -> [u8; 3] {
    [
        normalized_channel(first),
        normalized_channel(second),
        normalized_channel(third),
    ]
}

fn normalized_hsb(hue: f64, saturation: f64, brightness: f64) -> [u8; 3] {
    let hue = hue.rem_euclid(1.0);
    let saturation = saturation.clamp(0.0, 1.0);
    let brightness = brightness.clamp(0.0, 1.0);
    if saturation <= 1e-9 {
        let channel = normalized_channel(brightness);
        return [channel, channel, channel];
    }
    let scaled = hue * 6.0;
    let sector = scaled.floor() as usize % 6;
    let fraction = scaled - scaled.floor();
    let p = brightness * (1.0 - saturation);
    let q = brightness * (1.0 - saturation * fraction);
    let t = brightness * (1.0 - saturation * (1.0 - fraction));
    let (red, green, blue) = match sector {
        0 => (brightness, t, p),
        1 => (q, brightness, p),
        2 => (p, brightness, t),
        3 => (p, q, brightness),
        4 => (t, p, brightness),
        _ => (brightness, p, q),
    };
    [
        normalized_channel(red),
        normalized_channel(green),
        normalized_channel(blue),
    ]
}

pub(crate) fn build_scene_checked(file: &GspFile) -> Result<Scene> {
    let groups = file.object_groups();
    validate_scene_payloads(file, &groups)?;
    let point_map = collect_point_objects(file, &groups);
    let analysis = analyze_scene(file, &groups, &point_map);
    let mut shapes = collect_scene_shapes(file, &groups, &point_map, &analysis);
    let (images, image_group_to_index) = collect_scene_images(file, &groups, &analysis.graph_ref);
    let (mut labels, label_group_to_index, pending_hotspots) =
        collect_scene_labels(file, &groups, &analysis, &shapes);

    let (visible_points, group_to_point_index) = collect_visible_points_checked(
        file,
        &groups,
        &point_map,
        &analysis.raw_anchors,
        &analysis.graph_ref,
    )?;
    shapes.coordinate_traces.extend(collect_point_traces(
        file,
        &groups,
        &visible_points,
        &group_to_point_index,
        &analysis.graph_ref,
    ));
    let (derived_iteration_points, raw_point_iterations) =
        collect_point_iteration_points(file, &groups, &analysis.raw_anchors, &group_to_point_index);
    let standalone_parameter_points = collect_standalone_parameter_points(file, &groups);
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
    bind_button_seed_expression_labels(
        file,
        &groups,
        &analysis.raw_anchors,
        &mut labels,
        &label_group_to_index,
        &group_to_point_index,
    );
    let iteration_tables = collect_iteration_tables(file, &groups, &analysis.raw_anchors);
    remap_label_bindings(&mut labels, &group_to_point_index);
    let (binding_maps, line_iterations, polygon_iterations) = remap_scene_bindings(
        file,
        &groups,
        &analysis.raw_anchors,
        &group_to_point_index,
        &mut shapes,
    );
    apply_payload_color_bindings(
        file,
        &groups,
        &analysis.raw_anchors,
        &group_to_point_index,
        &binding_maps.circle_group_to_index,
        &mut shapes,
    );
    let circle_iterations = collect_carried_circle_iteration_families(
        file,
        &groups,
        &analysis.raw_anchors,
        &group_to_point_index,
        &binding_maps.circle_group_to_index,
    );
    let world_data = build_world_data(
        &analysis,
        &visible_points,
        &derived_iteration_points,
        &standalone_parameter_points,
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
        ButtonIndexLookups {
            label_group_to_index: &label_group_to_index,
            image_group_to_index: &image_group_to_index,
            group_to_point_index: &group_to_point_index,
            line_group_to_index: &binding_maps.line_group_to_index,
            circle_group_to_index: &binding_maps.circle_group_to_index,
            polygon_group_to_index: &binding_maps.polygon_group_to_index,
        },
    );
    resolve_label_hotspots(
        file,
        &groups,
        &mut labels,
        &pending_hotspots,
        HotspotIndexLookups {
            group_to_point_index: &group_to_point_index,
            circle_group_to_index: &binding_maps.circle_group_to_index,
            polygon_group_to_index: &binding_maps.polygon_group_to_index,
            button_group_to_index: &button_group_to_index,
        },
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
        SceneAssemblyArtifacts {
            circle_iterations,
            line_iterations,
            polygon_iterations,
            label_iterations,
            iteration_tables,
            buttons,
            images,
            parameters,
            functions,
        },
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

pub(crate) fn render_payload_log(source_path: &Path, file: &GspFile) -> String {
    let groups = file.object_groups();
    let issues = collect_unsupported_payload_issues(file, &groups);

    let mut output = String::new();
    let _ = writeln!(output, "载荷说明");
    let _ = writeln!(output, "文件: {}", source_path.display());
    let _ = writeln!(output, "问题数量: {}", issues.len());
    let _ = writeln!(output, "对象组数量: {}", groups.len());
    let _ = writeln!(output);
    let _ = writeln!(output, "问题列表");

    if issues.is_empty() {
        let _ = writeln!(output, "未发现不支持的载荷。");
    } else {
        for (index, issue) in issues.iter().enumerate() {
            let _ = writeln!(
                output,
                "{}. {}",
                index + 1,
                describe_issue_in_chinese(&issue.summary, &issue.group_ordinals)
            );
            let related_ordinals =
                collect_related_group_ordinals(file, &groups, &issue.group_ordinals);
            if !related_ordinals.is_empty() {
                let _ = writeln!(output, "   相关对象：");
                for (related_index, ordinal) in related_ordinals.iter().enumerate() {
                    if let Some(group) = groups.get(ordinal.saturating_sub(1)) {
                        let _ = writeln!(
                            output,
                            "   {}. {}",
                            related_index + 1,
                            describe_group_in_chinese(file, &groups, group)
                        );
                    }
                }
            }
            let _ = writeln!(output, "   原始载荷：");
            for ordinal in &issue.group_ordinals {
                if let Some(group) = groups.get(ordinal.saturating_sub(1)) {
                    write_group_detail(&mut output, file, group, "   ");
                }
            }
        }
    }

    let _ = writeln!(output);
    let _ = writeln!(output, "构造步骤");
    for (index, group) in groups.iter().enumerate() {
        let _ = writeln!(
            output,
            "{}. {}",
            index + 1,
            describe_group_in_chinese(file, &groups, group)
        );
    }

    output
}

fn collect_related_group_ordinals(
    file: &GspFile,
    groups: &[ObjectGroup],
    root_ordinals: &[usize],
) -> Vec<usize> {
    let mut visited = BTreeSet::new();
    let mut ordered = Vec::new();
    for ordinal in root_ordinals {
        visit_group_dependencies(file, groups, *ordinal, &mut visited, &mut ordered);
    }
    ordered
}

fn visit_group_dependencies(
    file: &GspFile,
    groups: &[ObjectGroup],
    ordinal: usize,
    visited: &mut BTreeSet<usize>,
    ordered: &mut Vec<usize>,
) {
    if ordinal == 0 || !visited.insert(ordinal) {
        return;
    }
    if let Some(group) = groups.get(ordinal.saturating_sub(1)) {
        ordered.push(ordinal);
        if let Some(path) = find_indexed_path(file, group) {
            for ref_ordinal in path.refs {
                visit_group_dependencies(file, groups, ref_ordinal, visited, ordered);
            }
        }
    }
}

fn describe_issue_in_chinese(summary: &str, group_ordinals: &[usize]) -> String {
    let target = group_ordinals
        .first()
        .map(|ordinal| format!("对象 #{}", ordinal))
        .unwrap_or_else(|| "当前对象".to_string());

    if let Some(rest) = summary.strip_prefix("unsupported payload: unknown object kind ")
        && let Some((raw, _)) = rest.split_once(" in ")
    {
        return format!("{target} 暂时无法导出，因为对象类型 {raw} 还没有实现。");
    }
    if let Some(rest) =
        summary.strip_prefix("unsupported payload: action button payload too short (")
        && let Some((bytes, _)) = rest.split_once(" bytes) in ")
    {
        return format!("{target} 暂时无法导出，因为按钮载荷只有 {bytes} 字节，长度不足。");
    }
    if let Some(rest) =
        summary.strip_prefix("unsupported payload: action button uses unsupported action kind (")
        && let Some((action_kind, _)) = rest.split_once(") in ")
    {
        return format!("{target} 暂时无法导出，因为按钮动作类型 ({action_kind}) 目前还不支持。");
    }
    if let Some(rest) = summary.strip_prefix("unsupported payload: malformed image payload in ")
        && let Some((_, sizes)) = rest.split_once(" (")
    {
        let sizes = sizes.trim_end_matches(')');
        return format!("{target} 暂时无法导出，因为图片载荷结构不完整（{sizes}）。");
    }
    if let Some(rest) = summary.strip_prefix("unsupported payload: non-positive image dimensions (")
        && let Some((dimensions, _)) = rest.split_once(") in ")
    {
        return format!("{target} 暂时无法导出，因为图片尺寸 {dimensions} 无效。");
    }
    if summary.starts_with("unsupported payload: non-finite image transform in ") {
        return format!("{target} 暂时无法导出，因为图片变换参数不是有限数值。");
    }
    if summary.starts_with("unsupported payload: non-axis-aligned image transform in ") {
        return format!("{target} 暂时无法导出，因为图片变换不是轴对齐矩形。");
    }
    if summary.starts_with("unsupported payload: function plot is missing indexed path in ") {
        return format!("{target} 暂时无法导出，因为函数图像缺少索引路径。");
    }
    if let Some(rest) = summary.strip_prefix("unsupported payload: function plot path has ")
        && let Some((refs, _)) = rest.split_once(" refs in ")
    {
        return format!("{target} 暂时无法导出，因为函数图像路径只有 {refs} 个引用。");
    }
    if let Some(rest) = summary
        .strip_prefix("unsupported payload: function plot references missing definition group #")
        && let Some((definition_ordinal, _)) = rest.split_once(" from ")
    {
        return format!(
            "{target} 暂时无法导出，因为它引用的函数定义对象组 #{definition_ordinal} 不存在。"
        );
    }
    if summary.starts_with("unsupported payload: invalid function plot descriptor in ") {
        return format!("{target} 暂时无法导出，因为函数图像描述符无效。");
    }
    if summary.starts_with("unsupported payload: invalid function expression in ") {
        return format!("{target} 暂时无法导出，因为关联的函数表达式无法解析。");
    }
    if let Some(rest) = summary.strip_prefix("unsupported payload: missing ")
        && let Some((record_label, _)) = rest.split_once(" (record ")
    {
        return format!("{target} 暂时无法导出，因为缺少“{record_label}”记录。");
    }

    format!("{target} 暂时无法导出。原始诊断：{summary}")
}

fn describe_group_in_chinese(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
) -> String {
    let refs = find_indexed_path(file, group)
        .map(|path| path.refs)
        .unwrap_or_default();
    let mut detail = match group.header.kind() {
        GroupKind::Point => describe_point_group_in_chinese(file, &refs, group),
        GroupKind::Midpoint => refs
            .first()
            .map(|host| format!("{} 的中点", format_ref(*host)))
            .unwrap_or_else(|| "中点对象".to_string()),
        GroupKind::Segment => describe_pair_relation(&refs, "线段", "连接"),
        GroupKind::Circle => {
            if refs.len() == 2 {
                format!(
                    "圆，圆心是 {}，并且经过 {}",
                    format_ref(refs[0]),
                    format_ref(refs[1])
                )
            } else {
                describe_generic_group(group, &refs)
            }
        }
        GroupKind::CircleCenterRadius => {
            if refs.len() == 2 {
                format!(
                    "圆，圆心是 {}，半径取自 {}",
                    format_ref(refs[0]),
                    format_ref_with_kind(groups, refs[1])
                )
            } else {
                describe_generic_group(group, &refs)
            }
        }
        GroupKind::Line => describe_pair_relation(&refs, "直线", "经过"),
        GroupKind::Ray => {
            if refs.len() == 2 {
                format!(
                    "射线，起点是 {}，方向经过 {}",
                    format_ref(refs[0]),
                    format_ref(refs[1])
                )
            } else {
                describe_generic_group(group, &refs)
            }
        }
        GroupKind::LineKind5 => {
            if refs.len() == 2 {
                format!(
                    "过 {} 且垂直于 {} 的直线",
                    format_ref(refs[0]),
                    format_ref_with_kind(groups, refs[1])
                )
            } else {
                describe_generic_group(group, &refs)
            }
        }
        GroupKind::LineKind6 => {
            if refs.len() == 2 {
                format!(
                    "过 {} 且平行于 {} 的直线",
                    format_ref(refs[0]),
                    format_ref_with_kind(groups, refs[1])
                )
            } else {
                describe_generic_group(group, &refs)
            }
        }
        GroupKind::LineKind7 => {
            if refs.len() == 3 {
                format!(
                    "以 {} 为顶点、夹在 {} 和 {} 之间的角平分线",
                    format_ref(refs[1]),
                    format_ref(refs[0]),
                    format_ref(refs[2])
                )
            } else {
                describe_generic_group(group, &refs)
            }
        }
        GroupKind::Polygon => {
            if refs.is_empty() {
                "多边形".to_string()
            } else {
                format!("多边形，顶点顺序是 {}", format_ref_list(&refs))
            }
        }
        GroupKind::LinearIntersectionPoint => describe_intersection_point(&refs, None),
        GroupKind::IntersectionPoint1 => describe_intersection_point(&refs, Some("第一个")),
        GroupKind::IntersectionPoint2 => describe_intersection_point(&refs, Some("第二个")),
        GroupKind::CircleCircleIntersectionPoint1 => {
            describe_circle_intersection_point(&refs, Some("第一个"))
        }
        GroupKind::CircleCircleIntersectionPoint2 => {
            describe_circle_intersection_point(&refs, Some("第二个"))
        }
        GroupKind::PointConstraint | GroupKind::PathPoint => refs
            .first()
            .map(|host| format!("位于 {} 上的动点", format_ref_with_kind(groups, *host)))
            .unwrap_or_else(|| "受约束的动点".to_string()),
        GroupKind::Translation => describe_translation_group_in_chinese(groups, &refs),
        GroupKind::CartesianOffsetPoint | GroupKind::PolarOffsetPoint => {
            describe_offset_point_in_chinese(file, group, &refs)
        }
        GroupKind::ExpressionOffsetPoint => {
            if refs.len() >= 2 {
                format!(
                    "以 {} 为基准、按 {} 做水平偏移得到的点",
                    format_ref_with_kind(groups, refs[0]),
                    format_ref_with_kind(groups, refs[1])
                )
            } else {
                describe_generic_group(group, &refs)
            }
        }
        GroupKind::Rotation => describe_rotation_group_in_chinese(file, groups, group),
        GroupKind::AngleRotation => describe_angle_rotation_group_in_chinese(file, groups, group),
        GroupKind::ParameterRotation => {
            describe_parameter_rotation_group_in_chinese(file, groups, group)
        }
        GroupKind::ExpressionRotation => {
            if refs.len() >= 3 {
                format!(
                    "将 {} 围绕 {} 按 {} 旋转得到的点",
                    format_ref_with_kind(groups, refs[0]),
                    format_ref(refs[1]),
                    format_ref_with_kind(groups, refs[2])
                )
            } else {
                describe_generic_group(group, &refs)
            }
        }
        GroupKind::Scale => describe_scale_group_in_chinese(file, groups, group),
        GroupKind::RatioScale => {
            if refs.len() >= 5 {
                format!(
                    "将 {} 以 {} 为中心，按 {} 到 {} 与 {} 到 {} 的长度比缩放得到的对象",
                    format_ref_with_kind(groups, refs[0]),
                    format_ref(refs[1]),
                    format_ref(refs[2]),
                    format_ref(refs[4]),
                    format_ref(refs[2]),
                    format_ref(refs[3])
                )
            } else {
                describe_generic_group(group, &refs)
            }
        }
        GroupKind::DistanceValue => {
            if refs.len() >= 2 {
                format!(
                    "{} 与 {} 的距离值",
                    format_ref(refs[0]),
                    format_ref(refs[1])
                )
            } else {
                describe_generic_group(group, &refs)
            }
        }
        GroupKind::PointLineDistanceValue => {
            if refs.len() >= 2 {
                format!(
                    "{} 到 {} 的距离值",
                    format_ref(refs[0]),
                    format_ref_with_kind(groups, refs[1])
                )
            } else {
                describe_generic_group(group, &refs)
            }
        }
        GroupKind::Reflection => {
            if refs.len() >= 2 {
                format!(
                    "把 {} 关于 {} 镜像得到的对象",
                    format_ref_with_kind(groups, refs[0]),
                    format_ref_with_kind(groups, refs[1])
                )
            } else {
                describe_generic_group(group, &refs)
            }
        }
        GroupKind::CircleInterior => refs
            .first()
            .map(|host| format!("以 {} 为边界的圆面", format_ref_with_kind(groups, *host)))
            .unwrap_or_else(|| "圆面".to_string()),
        GroupKind::CoordinateXValue => refs
            .first()
            .map(|host| format!("{} 的图像 x 坐标值", format_ref(*host)))
            .unwrap_or_else(|| "图像 x 坐标值".to_string()),
        GroupKind::CoordinateYValue => refs
            .first()
            .map(|host| format!("{} 的图像 y 坐标值", format_ref(*host)))
            .unwrap_or_else(|| "图像 y 坐标值".to_string()),
        GroupKind::ActionButton => describe_action_button_group_in_chinese(file, group, &refs),
        GroupKind::FunctionPlot => describe_function_plot_group_in_chinese(groups, &refs),
        GroupKind::ArcOnCircle => {
            if refs.len() == 3 {
                format!(
                    "在 {} 上，从 {} 到 {} 的圆弧",
                    format_ref_with_kind(groups, refs[0]),
                    format_ref(refs[1]),
                    format_ref(refs[2])
                )
            } else {
                describe_generic_group(group, &refs)
            }
        }
        GroupKind::CenterArc => {
            if refs.len() == 3 {
                format!(
                    "以 {} 为圆心、从 {} 到 {} 的圆弧",
                    format_ref(refs[0]),
                    format_ref(refs[1]),
                    format_ref(refs[2])
                )
            } else {
                describe_generic_group(group, &refs)
            }
        }
        GroupKind::ThreePointArc => {
            if refs.len() == 3 {
                format!(
                    "经过 {}、{}、{} 的三点圆弧",
                    format_ref(refs[0]),
                    format_ref(refs[1]),
                    format_ref(refs[2])
                )
            } else {
                describe_generic_group(group, &refs)
            }
        }
        GroupKind::SectorBoundary => {
            if refs.len() == 3 {
                format!(
                    "由 {}、{}、{} 定义的扇形边界",
                    format_ref(refs[0]),
                    format_ref(refs[1]),
                    format_ref(refs[2])
                )
            } else {
                describe_generic_group(group, &refs)
            }
        }
        GroupKind::CircularSegmentBoundary => {
            if refs.len() == 3 {
                format!(
                    "由 {}、{}、{} 定义的弓形边界",
                    format_ref(refs[0]),
                    format_ref(refs[1]),
                    format_ref(refs[2])
                )
            } else {
                describe_generic_group(group, &refs)
            }
        }
        GroupKind::CoordinatePoint
        | GroupKind::CoordinateExpressionPoint
        | GroupKind::CoordinateExpressionPointAlt
        | GroupKind::Unknown(20) => {
            if refs.is_empty() {
                "坐标点".to_string()
            } else {
                format!("坐标点，依赖 {}", format_ref_list(&refs))
            }
        }
        GroupKind::PointTrace => refs
            .first()
            .map(|host| format!("{} 的轨迹", format_ref_with_kind(groups, *host)))
            .unwrap_or_else(|| "点轨迹".to_string()),
        GroupKind::CoordinateTrace => refs
            .first()
            .map(|host| format!("{} 的坐标轨迹", format_ref_with_kind(groups, *host)))
            .unwrap_or_else(|| "坐标轨迹".to_string()),
        GroupKind::CoordinateTraceIntersectionPoint => {
            if refs.len() >= 2 {
                format!(
                    "{} 和 {} 的交点",
                    format_ref_with_kind(groups, refs[0]),
                    format_ref_with_kind(groups, refs[1])
                )
            } else {
                "轨迹交点".to_string()
            }
        }
        GroupKind::AngleMarker => {
            if refs.len() == 3 {
                format!(
                    "角标记，顶点是 {}，两边经过 {} 和 {}",
                    format_ref(refs[1]),
                    format_ref(refs[0]),
                    format_ref(refs[2])
                )
            } else {
                describe_generic_group(group, &refs)
            }
        }
        GroupKind::SegmentMarker => refs
            .first()
            .map(|host| {
                format!(
                    "用于标记 {} 的线段记号",
                    format_ref_with_kind(groups, *host)
                )
            })
            .unwrap_or_else(|| "线段记号".to_string()),
        _ => describe_generic_group(group, &refs),
    };

    let mut annotations = Vec::new();
    if let Some(name) = self::decode::decode_label_name_raw(file, group) {
        annotations.push(format!("名称“{}”", truncate_text(name.trim(), 48)));
    }
    match try_decode_group_label_text(file, group) {
        Ok(Some(text)) => {
            let text = text.trim();
            if !text.is_empty() {
                annotations.push(format!("文字“{}”", truncate_text(text, 48)));
            }
        }
        Ok(None) => {}
        Err(error) => annotations.push(format!("文字解析失败（{}）", error)),
    }
    match try_decode_link_button_url(file, group) {
        Ok(Some(url)) => annotations.push(format!("链接“{}”", truncate_text(url.trim(), 64))),
        Ok(None) => {}
        Err(error) => annotations.push(format!("链接解析失败（{}）", error)),
    }
    if !annotations.is_empty() {
        detail.push_str(&format!("，{}", annotations.join("，")));
    }

    format!("#{} = {}。", group.ordinal, detail)
}

fn describe_point_group_in_chinese(file: &GspFile, refs: &[usize], group: &ObjectGroup) -> String {
    let has_explicit_point = group
        .records
        .iter()
        .any(|record| record.record_type == RECORD_POINT_F64_PAIR);
    let has_image_payload = [0x090c, 0x08a8, 0x1f44].into_iter().all(|record_type| {
        group
            .records
            .iter()
            .any(|record| record.record_type == record_type)
    });
    if has_image_payload {
        return "图片锚点".to_string();
    }
    if self::decode::is_parameter_control_group(group) {
        return "参数控制点".to_string();
    }
    if has_explicit_point && refs.is_empty() {
        return "自由点".to_string();
    }
    if refs.is_empty() {
        return "点".to_string();
    }
    let point = group
        .records
        .iter()
        .find(|record| record.record_type == RECORD_POINT_F64_PAIR)
        .and_then(|record| decode_point_record(record.payload(&file.data)));
    if let Some(point) = point {
        return format!(
            "点，当前坐标是 ({}, {})，并且依赖 {}",
            format_number(point.x),
            format_number(point.y),
            format_ref_list(refs)
        );
    }
    format!("点，依赖 {}", format_ref_list(refs))
}

fn describe_pair_relation(refs: &[usize], noun: &str, verb: &str) -> String {
    if refs.len() == 2 {
        format!(
            "{noun}，{verb} {} 和 {}",
            format_ref(refs[0]),
            format_ref(refs[1])
        )
    } else {
        format!("{noun}，按载荷顺序引用 {}", format_ref_list(refs))
    }
}

fn describe_intersection_point(refs: &[usize], variant: Option<&str>) -> String {
    if refs.len() >= 2 {
        let prefix = variant.unwrap_or("");
        format!(
            "{prefix}交点，来自 {} 和 {}",
            format_ref(refs[0]),
            format_ref(refs[1])
        )
    } else {
        "交点".to_string()
    }
}

fn describe_circle_intersection_point(refs: &[usize], variant: Option<&str>) -> String {
    if refs.len() >= 2 {
        let prefix = variant.unwrap_or("");
        format!(
            "{prefix}圆交点，来自 {} 和 {}",
            format_ref(refs[0]),
            format_ref(refs[1])
        )
    } else {
        "圆交点".to_string()
    }
}

fn describe_translation_group_in_chinese(groups: &[ObjectGroup], refs: &[usize]) -> String {
    if refs.len() >= 3 {
        return format!(
            "将 {} 按向量 {} -> {} 平移得到的对象",
            format_ref_with_kind(groups, refs[0]),
            format_ref(refs[1]),
            format_ref(refs[2])
        );
    }
    "平移对象".to_string()
}

fn describe_offset_point_in_chinese(file: &GspFile, group: &ObjectGroup, refs: &[usize]) -> String {
    if let Some(constraint) = decode_translated_point_constraint(file, group)
        && let Some(origin) = refs.first()
    {
        return format!(
            "从 {} 平移 ({}, {}) 得到的点",
            format_ref(*origin),
            format_number(constraint.dx),
            format_number(constraint.dy)
        );
    }
    if let Some(origin) = refs.first() {
        return format!("从 {} 偏移得到的点", format_ref(*origin));
    }
    "偏移点".to_string()
}

fn describe_rotation_group_in_chinese(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
) -> String {
    if let Ok(binding) = try_decode_transform_binding(file, group) {
        let source_ordinal = binding.source_group_index + 1;
        let center_ordinal = binding.center_group_index + 1;
        if let TransformBindingKind::Rotate { angle_degrees, .. } = binding.kind {
            return format!(
                "将 {} 围绕 {} 旋转 {} 度得到的对象",
                format_ref_with_kind(groups, source_ordinal),
                format_ref(center_ordinal),
                format_number(angle_degrees)
            );
        }
    }
    describe_generic_group(
        group,
        &find_indexed_path(file, group)
            .map(|path| path.refs)
            .unwrap_or_default(),
    )
}

fn describe_parameter_rotation_group_in_chinese(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
) -> String {
    if let Ok(binding) = try_decode_parameter_rotation_binding(file, groups, group) {
        let source_ordinal = binding.source_group_index + 1;
        let center_ordinal = binding.center_group_index + 1;
        if let TransformBindingKind::Rotate {
            angle_degrees,
            parameter_name,
        } = binding.kind
        {
            if let Some(parameter_name) = parameter_name {
                return format!(
                    "将 {} 围绕 {} 按参数 {} 旋转得到的对象（当前角度 {} 度）",
                    format_ref_with_kind(groups, source_ordinal),
                    format_ref(center_ordinal),
                    parameter_name,
                    format_number(angle_degrees)
                );
            }
            return format!(
                "将 {} 围绕 {} 旋转 {} 度得到的对象",
                format_ref_with_kind(groups, source_ordinal),
                format_ref(center_ordinal),
                format_number(angle_degrees)
            );
        }
    }
    describe_generic_group(
        group,
        &find_indexed_path(file, group)
            .map(|path| path.refs)
            .unwrap_or_default(),
    )
}

fn describe_angle_rotation_group_in_chinese(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
) -> String {
    let refs = find_indexed_path(file, group)
        .map(|path| path.refs)
        .unwrap_or_default();
    if refs.len() >= 5 {
        return format!(
            "将 {} 围绕 {} 按 {}、{}、{} 所成角旋转得到的点",
            format_ref_with_kind(groups, refs[0]),
            format_ref(refs[1]),
            format_ref(refs[2]),
            format_ref(refs[3]),
            format_ref(refs[4])
        );
    }
    describe_generic_group(group, &refs)
}

fn describe_scale_group_in_chinese(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
) -> String {
    if let Ok(binding) = try_decode_transform_binding(file, group) {
        let source_ordinal = binding.source_group_index + 1;
        let center_ordinal = binding.center_group_index + 1;
        if let TransformBindingKind::Scale { factor } = binding.kind {
            return format!(
                "将 {} 以 {} 为中心缩放 {} 倍得到的对象",
                format_ref_with_kind(groups, source_ordinal),
                format_ref(center_ordinal),
                format_number(factor)
            );
        }
    }
    describe_generic_group(
        group,
        &find_indexed_path(file, group)
            .map(|path| path.refs)
            .unwrap_or_default(),
    )
}

fn describe_action_button_group_in_chinese(
    file: &GspFile,
    group: &ObjectGroup,
    refs: &[usize],
) -> String {
    let action_kind = group
        .records
        .iter()
        .find(|record| record.record_type == 0x0906)
        .map(|record| record.payload(&file.data))
        .filter(|payload| payload.len() >= 16)
        .map(|payload| (read_u16(payload, 12), read_u16(payload, 14)));
    let placement = if refs.is_empty() {
        "按钮".to_string()
    } else {
        format!("按钮，关联 {}", format_ref_list(refs))
    };
    if let Some((primary, secondary)) = action_kind {
        return format!("{placement}，动作类型是 ({primary}, {secondary})");
    }
    placement
}

fn describe_function_plot_group_in_chinese(groups: &[ObjectGroup], refs: &[usize]) -> String {
    if refs.len() >= 2 {
        return format!(
            "函数图像，定义来自 {}，并且依赖 {}",
            format_ref_with_kind(groups, refs[0]),
            format_ref_list(&refs[1..])
        );
    }
    if refs.len() == 1 {
        return format!(
            "函数图像，定义来自 {}",
            format_ref_with_kind(groups, refs[0])
        );
    }
    "函数图像".to_string()
}

fn describe_generic_group(group: &ObjectGroup, refs: &[usize]) -> String {
    match group.header.kind() {
        GroupKind::Unknown(raw) => {
            if refs.is_empty() {
                format!("未知对象，类型是 {raw}")
            } else {
                format!(
                    "未知对象，类型是 {raw}，按载荷顺序引用 {}",
                    format_ref_list(refs)
                )
            }
        }
        kind => {
            let kind_name = group_kind_name_in_chinese(kind);
            if refs.is_empty() {
                kind_name.to_string()
            } else {
                format!("{kind_name}，按载荷顺序引用 {}", format_ref_list(refs))
            }
        }
    }
}

fn group_kind_name_in_chinese(kind: GroupKind) -> &'static str {
    match kind {
        GroupKind::Point => "点",
        GroupKind::Midpoint => "中点",
        GroupKind::Segment => "线段",
        GroupKind::Circle => "圆",
        GroupKind::CircleCenterRadius => "定圆心定半径圆",
        GroupKind::LineKind5 => "垂线",
        GroupKind::LineKind6 => "平行线",
        GroupKind::LineKind7 => "角平分线",
        GroupKind::Polygon => "多边形",
        GroupKind::LinearIntersectionPoint => "交点",
        GroupKind::CircleInterior => "圆面",
        GroupKind::IntersectionPoint1 => "第一个交点",
        GroupKind::IntersectionPoint2 => "第二个交点",
        GroupKind::CircleCircleIntersectionPoint1 => "第一个圆交点",
        GroupKind::CircleCircleIntersectionPoint2 => "第二个圆交点",
        GroupKind::PointConstraint => "路径动点",
        GroupKind::Translation => "平移对象",
        GroupKind::CartesianOffsetPoint => "直角坐标偏移点",
        GroupKind::CoordinateExpressionPoint => "坐标表达式点",
        GroupKind::CoordinateExpressionPointAlt => "坐标表达式点",
        GroupKind::PolarOffsetPoint => "极坐标偏移点",
        GroupKind::ExpressionOffsetPoint => "表达式偏移点",
        GroupKind::DerivedSegment24 => "派生线段",
        GroupKind::CustomTransformPoint => "自定义变换点",
        GroupKind::Rotation => "旋转对象",
        GroupKind::AngleRotation => "角度旋转点",
        GroupKind::ParameterRotation => "参数旋转对象",
        GroupKind::ExpressionRotation => "表达式旋转点",
        GroupKind::Scale => "缩放对象",
        GroupKind::RatioScale => "比例缩放对象",
        GroupKind::Reflection => "镜像对象",
        GroupKind::DistanceValue => "两点距离值",
        GroupKind::PointLineDistanceValue => "点到直线距离值",
        GroupKind::PointTrace => "点轨迹",
        GroupKind::MeasuredValue => "度量值",
        GroupKind::GraphObject40 => "图像对象",
        GroupKind::CoordinateReadoutLabel => "坐标读数标签",
        GroupKind::FunctionExpr => "函数表达式",
        GroupKind::Kind51 => "对象类型 51",
        GroupKind::GraphCalibrationX => "图像校准点 X",
        GroupKind::GraphCalibrationY | GroupKind::GraphCalibrationYAlt => "图像校准点 Y",
        GroupKind::MeasurementLine => "测量线",
        GroupKind::AxisLine => "坐标轴",
        GroupKind::ActionButton => "动作按钮",
        GroupKind::Line => "直线",
        GroupKind::Ray => "射线",
        GroupKind::CoordinateXValue => "图像 x 坐标值",
        GroupKind::CoordinateYValue => "图像 y 坐标值",
        GroupKind::OffsetAnchor => "偏移锚点",
        GroupKind::CoordinatePoint => "坐标点",
        GroupKind::FunctionPlot => "函数图像",
        GroupKind::ButtonLabel => "按钮标签",
        GroupKind::DerivedSegment75 => "派生线段",
        GroupKind::AffineIteration => "仿射迭代",
        GroupKind::IterationBinding => "迭代绑定",
        GroupKind::DerivativeFunction => "导函数",
        GroupKind::ArcOnCircle => "圆上弧",
        GroupKind::CenterArc => "圆心弧",
        GroupKind::ThreePointArc => "过三点弧",
        GroupKind::SectorBoundary => "扇形边界",
        GroupKind::CircularSegmentBoundary => "弓形边界",
        GroupKind::RegularPolygonIteration => "正多边形迭代",
        GroupKind::LabelIterationSeed => "标签迭代种子",
        GroupKind::IterationExpressionHelper => "迭代表达式辅助对象",
        GroupKind::ParameterAnchor => "参数锚点",
        GroupKind::ParameterControlledPoint => "参数控制点",
        GroupKind::CoordinateTrace => "坐标轨迹",
        GroupKind::CoordinateTraceIntersectionPoint => "坐标轨迹交点",
        GroupKind::CustomTransformTrace => "自定义变换轨迹",
        GroupKind::LegacyCoordinateParameterHelper => "旧版坐标参数辅助对象",
        GroupKind::LegacyCoordinatePointHelper => "旧版坐标点辅助对象",
        GroupKind::AngleMarker => "角标记",
        GroupKind::PathPoint => "路径点",
        GroupKind::SegmentMarker => "线段记号",
        GroupKind::Unknown(_) => "未知对象",
    }
}

fn group_kind_noun_in_chinese(kind: GroupKind) -> &'static str {
    match kind {
        GroupKind::Point
        | GroupKind::Midpoint
        | GroupKind::LinearIntersectionPoint
        | GroupKind::IntersectionPoint1
        | GroupKind::IntersectionPoint2
        | GroupKind::CircleCircleIntersectionPoint1
        | GroupKind::CircleCircleIntersectionPoint2
        | GroupKind::PointConstraint
        | GroupKind::CartesianOffsetPoint
        | GroupKind::CoordinateExpressionPoint
        | GroupKind::CoordinateExpressionPointAlt
        | GroupKind::PolarOffsetPoint
        | GroupKind::ExpressionOffsetPoint
        | GroupKind::CustomTransformPoint
        | GroupKind::AngleRotation
        | GroupKind::ExpressionRotation
        | GroupKind::OffsetAnchor
        | GroupKind::CoordinatePoint
        | GroupKind::LegacyCoordinateParameterHelper
        | GroupKind::LegacyCoordinatePointHelper
        | GroupKind::ParameterAnchor
        | GroupKind::ParameterControlledPoint
        | GroupKind::CoordinateTraceIntersectionPoint
        | GroupKind::PathPoint
        | GroupKind::Unknown(20) => "点",
        GroupKind::DistanceValue
        | GroupKind::PointLineDistanceValue
        | GroupKind::MeasuredValue
        | GroupKind::CoordinateXValue
        | GroupKind::CoordinateYValue => "数值对象",
        GroupKind::Segment | GroupKind::DerivedSegment75 => "线段",
        GroupKind::Line | GroupKind::LineKind5 | GroupKind::LineKind6 | GroupKind::LineKind7 => {
            "直线"
        }
        GroupKind::Ray => "射线",
        GroupKind::Circle | GroupKind::CircleCenterRadius => "圆",
        GroupKind::Polygon => "多边形",
        GroupKind::ArcOnCircle | GroupKind::CenterArc | GroupKind::ThreePointArc => "圆弧",
        GroupKind::CoordinateReadoutLabel => "标签",
        GroupKind::ActionButton => "按钮",
        GroupKind::FunctionPlot => "函数图像",
        GroupKind::AngleMarker => "角标记",
        _ => "对象",
    }
}

fn format_ref(ordinal: usize) -> String {
    format!("#{ordinal}")
}

fn format_ref_with_kind(groups: &[ObjectGroup], ordinal: usize) -> String {
    groups
        .get(ordinal.saturating_sub(1))
        .map(|group| {
            format!(
                "{} #{}",
                group_kind_noun_in_chinese(group.header.kind()),
                ordinal
            )
        })
        .unwrap_or_else(|| format_ref(ordinal))
}

fn format_ref_list(refs: &[usize]) -> String {
    if refs.is_empty() {
        "无引用".to_string()
    } else {
        refs.iter()
            .map(|ordinal| format_ref(*ordinal))
            .collect::<Vec<_>>()
            .join("、")
    }
}

fn format_number(value: f64) -> String {
    let rounded = if value.abs() < 1e-9 { 0.0 } else { value };
    let text = format!("{rounded:.3}");
    text.trim_end_matches('0').trim_end_matches('.').to_string()
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
    if matches!(
        kind,
        GroupKind::Unknown(20)
            | GroupKind::DistanceValue
            | GroupKind::PointLineDistanceValue
            | GroupKind::CoordinateXValue
            | GroupKind::CoordinateYValue
            | GroupKind::Unknown(71)
            | GroupKind::Unknown(122)
            | GroupKind::Unknown(39)
            | GroupKind::Unknown(41)
            | GroupKind::Unknown(47)
            | GroupKind::Unknown(42)
            | GroupKind::Unknown(46)
            | GroupKind::Unknown(59)
            | GroupKind::Unknown(91)
            | GroupKind::Unknown(93)
            | GroupKind::Unknown(99)
            | GroupKind::Unknown(100)
            | GroupKind::Unknown(101)
            | GroupKind::Unknown(108)
            | GroupKind::Unknown(115)
            | GroupKind::Unknown(116)
            | GroupKind::Unknown(120)
            | GroupKind::Unknown(85)
            | GroupKind::Unknown(88)
            | GroupKind::LegacyCoordinateParameterHelper
            | GroupKind::LegacyCoordinatePointHelper
    ) || is_supported_group_kind(kind)
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
    match action_kind {
        (2, 0)
        | (4, 0)
        | (4, 1)
        | (8, 0)
        | (7, _)
        | (3, 0)
        | (3, 1)
        | (3, 2)
        | (3, 3)
        | (0, 0)
        | (0, 1)
        | (0, 7)
        | (0, 2)
        | (0, 3)
        | (0, 4)
        | (0, 5)
        | (0, 6)
        | (1, 7)
        | (1, 0)
        | (1, 1)
        | (1, 3)
        | (1, 2)
        | (1, 4)
        | (1, 5)
        | (1, 6) => return Ok(()),
        _ => {}
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

    let path = try_find_indexed_path(file, group)
        .map_err(anyhow::Error::msg)?
        .with_context(|| {
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
    let descriptor_payload = group_record_payload(
        file,
        group,
        RECORD_FUNCTION_PLOT_DESCRIPTOR,
        "function plot descriptor",
    )?;
    try_decode_function_plot_descriptor(descriptor_payload)
        .map_err(anyhow::Error::msg)
        .with_context(|| {
            format!(
                "unsupported payload: invalid function plot descriptor in {}",
                describe_group(group)
            )
        })?;
    let definition_kind = definition_group.header.kind();
    let definition_is_expression_bearing = matches!(
        definition_kind,
        GroupKind::FunctionExpr
            | GroupKind::DistanceValue
            | GroupKind::PointLineDistanceValue
            | GroupKind::CoordinateXValue
            | GroupKind::CoordinateYValue
            | GroupKind::Unknown(71)
    ) || definition_group
        .records
        .iter()
        .any(|record| record.record_type == RECORD_FUNCTION_EXPR_PAYLOAD);
    if definition_is_expression_bearing {
        try_decode_function_expr(file, groups, definition_group)
            .map_err(anyhow::Error::msg)
            .with_context(|| {
                format!(
                    "unsupported payload: invalid function expression in {} referenced by {}",
                    describe_group(definition_group),
                    describe_group(group)
                )
            })?;
    }

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

fn write_group_detail(output: &mut String, file: &GspFile, group: &ObjectGroup, indent: &str) {
    let _ = writeln!(output, "{indent}对象 #{}：", group.ordinal);
    let _ = writeln!(
        output,
        "{indent}  类型: {:?} (raw=0x{:04x}, class_id=0x{:08x})",
        group.header.kind(),
        group.header.kind_id(),
        group.header.class_id
    );
    let _ = writeln!(
        output,
        "{indent}  几何属性: hidden={} flags=0x{:08x} style=[0x{:08x}, 0x{:08x}, 0x{:08x}]",
        group.header.is_hidden(),
        group.header.flags,
        group.header.style_a,
        group.header.style_b,
        group.header.style_c
    );
    let _ = writeln!(
        output,
        "{indent}  偏移: start=0x{:x} end=0x{:x}",
        group.start_offset, group.end_offset
    );

    if let Some(name) = self::decode::decode_label_name_raw(file, group) {
        let _ = writeln!(output, "{indent}  名称: {:?}", name);
    }
    match try_decode_group_label_text(file, group) {
        Ok(Some(text)) => {
            let _ = writeln!(output, "{indent}  标签文字: {:?}", text);
        }
        Ok(None) => {}
        Err(error) => {
            let _ = writeln!(output, "{indent}  标签文字解析错误: {}", error);
        }
    }
    match try_decode_group_rich_text(file, group) {
        Ok(Some(content)) if !content.hotspots.is_empty() => {
            let _ = writeln!(
                output,
                "{indent}  富文本热点数量: {}",
                content.hotspots.len()
            );
        }
        Ok(_) => {}
        Err(error) => {
            let _ = writeln!(output, "{indent}  富文本解析错误: {}", error);
        }
    }
    match try_decode_link_button_url(file, group) {
        Ok(Some(url)) => {
            let _ = writeln!(output, "{indent}  动作链接: {:?}", url);
        }
        Ok(None) => {}
        Err(error) => {
            let _ = writeln!(output, "{indent}  动作链接解析错误: {}", error);
        }
    }
    match try_find_indexed_path(file, group) {
        Ok(Some(path)) => {
            let _ = writeln!(output, "{indent}  引用: {:?}", path.refs);
        }
        Ok(None) => {}
        Err(error) => {
            let _ = writeln!(output, "{indent}  引用解析错误: {}", error);
        }
    }
    if group.header.kind().is_point_constraint() {
        match try_decode_point_constraint(file, &file.object_groups(), group, None, &None) {
            Ok(constraint) => {
                let summary = match constraint {
                    self::points::RawPointConstraint::Segment(constraint) => format!(
                        "segment start=#{} end=#{} t={:.6}",
                        constraint.start_group_index + 1,
                        constraint.end_group_index + 1,
                        constraint.t
                    ),
                    self::points::RawPointConstraint::ConstructedLine {
                        host_group_index,
                        t,
                        line_like_kind,
                    } => format!(
                        "constructed-line host=#{} kind={:?} t={:.6}",
                        host_group_index + 1,
                        line_like_kind,
                        t
                    ),
                    self::points::RawPointConstraint::PolygonBoundary { edge_index, t, .. } => {
                        format!("polygon edge={} t={:.6}", edge_index, t)
                    }
                    self::points::RawPointConstraint::Circle(constraint) => format!(
                        "circle center=#{} radius=#{} unit=({:.6}, {:.6})",
                        constraint.center_group_index + 1,
                        constraint.radius_group_index + 1,
                        constraint.unit_x,
                        constraint.unit_y
                    ),
                    self::points::RawPointConstraint::Circular(constraint) => format!(
                        "circle-like host=#{} unit=({:.6}, {:.6})",
                        constraint.circle_group_index + 1,
                        constraint.unit_x,
                        constraint.unit_y
                    ),
                    self::points::RawPointConstraint::CircleArc(constraint) => format!(
                        "circle-arc center=#{} start=#{} end=#{} t={:.6}",
                        constraint.center_group_index + 1,
                        constraint.start_group_index + 1,
                        constraint.end_group_index + 1,
                        constraint.t
                    ),
                    self::points::RawPointConstraint::Arc(constraint) => format!(
                        "arc start=#{} mid=#{} end=#{} t={:.6}",
                        constraint.start_group_index + 1,
                        constraint.mid_group_index + 1,
                        constraint.end_group_index + 1,
                        constraint.t
                    ),
                    self::points::RawPointConstraint::Polyline {
                        function_key,
                        segment_index,
                        t,
                        ..
                    } => format!(
                        "polyline function_key={} segment={} t={:.6}",
                        function_key, segment_index, t
                    ),
                };
                let _ = writeln!(output, "{indent}  点约束: {}", summary);
            }
            Err(error) => {
                let _ = writeln!(output, "{indent}  点约束解析错误: {}", error);
            }
        }
    }
    match try_decode_transform_binding(file, group) {
        Ok(binding) => match binding.kind {
            TransformBindingKind::Rotate {
                angle_degrees,
                ref parameter_name,
            } => {
                let _ = writeln!(
                    output,
                    "{indent}  变换绑定: rotate source=#{} center=#{} angle={:.3} param={:?}",
                    binding.source_group_index + 1,
                    binding.center_group_index + 1,
                    angle_degrees,
                    parameter_name
                );
            }
            TransformBindingKind::Scale { factor } => {
                let _ = writeln!(
                    output,
                    "{indent}  变换绑定: scale source=#{} center=#{} factor={:.3}",
                    binding.source_group_index + 1,
                    binding.center_group_index + 1,
                    factor
                );
            }
        },
        Err(error) => {
            if matches!(
                group.header.kind(),
                GroupKind::Rotation
                    | GroupKind::AngleRotation
                    | GroupKind::Scale
                    | GroupKind::ParameterRotation
            ) {
                let _ = writeln!(output, "{indent}  变换绑定解析错误: {}", error);
            }
        }
    }
    if self::decode::is_parameter_control_group(group) {
        match try_decode_parameter_control_value_for_group(file, &[], group) {
            Ok(value) => {
                let _ = writeln!(output, "{indent}  参数值: {:.6}", value);
            }
            Err(error) => {
                let _ = writeln!(output, "{indent}  参数值解析错误: {}", error);
            }
        }
    }
    match try_decode_payload_anchor_point(file, group) {
        Ok(Some(anchor)) => {
            let _ = writeln!(output, "{indent}  锚点: ({:.3}, {:.3})", anchor.x, anchor.y);
        }
        Ok(None) => {}
        Err(error) => {
            let _ = writeln!(output, "{indent}  锚点解析错误: {}", error);
        }
    }
    match try_decode_bbox_rect_raw(file, group) {
        Ok(Some((x, y, width, height))) => {
            let _ = writeln!(
                output,
                "{indent}  包围框: ({:.3}, {:.3}, {:.3}, {:.3})",
                x, y, width, height
            );
        }
        Ok(None) => {}
        Err(error) => {
            let _ = writeln!(output, "{indent}  包围框解析错误: {}", error);
        }
    }

    let points = group
        .records
        .iter()
        .filter(|record| record.record_type == RECORD_POINT_F64_PAIR)
        .filter_map(|record| decode_point_record(record.payload(&file.data)))
        .take(3)
        .map(|point| format!("({:.3}, {:.3})", point.x, point.y))
        .collect::<Vec<_>>();
    if !points.is_empty() {
        let _ = writeln!(output, "{indent}  点坐标: {}", points.join(", "));
    }

    let strings = collect_group_strings(file, group);
    if !strings.is_empty() {
        let _ = writeln!(output, "{indent}  字符串: {}", strings.join(" | "));
    }

    let _ = writeln!(output, "{indent}  记录:");
    for record in &group.records {
        let _ = writeln!(
            output,
            "{indent}    - 0x{:04x} {} @0x{:x} payload=0x{:x}..0x{:x} len={}{}",
            record.record_type,
            record_name(record.record_type),
            record.offset,
            record.payload_range.start,
            record.payload_range.end,
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
        RECORD_POINT_F64_PAIR => {
            decode_point_record(payload).map(|point| format!("点=({:.3}, {:.3})", point.x, point.y))
        }
        crate::runtime::payload_consts::RECORD_INDEXED_PATH_A
        | crate::runtime::payload_consts::RECORD_INDEXED_PATH_B => {
            decode_indexed_path(record.record_type, payload)
                .map(|path| format!("引用={:?}", path.refs))
                .or_else(|| Some("引用解析失败".to_string()))
        }
        RECORD_FUNCTION_PLOT_DESCRIPTOR => {
            Some(match try_decode_function_plot_descriptor(payload) {
                Ok(descriptor) => format!(
                    "plot=[{:.3}, {:.3}] samples={} mode={:?}",
                    descriptor.x_min, descriptor.x_max, descriptor.sample_count, descriptor.mode
                ),
                Err(error) => format!("plot 解析失败: {error}"),
            })
        }
        _ => {
            let strings = collect_strings(payload)
                .into_iter()
                .map(|entry| truncate_text(entry.text.trim(), 48))
                .filter(|text| !text.is_empty())
                .take(2)
                .collect::<Vec<_>>();
            if !strings.is_empty() {
                return Some(format!("字符串={strings:?}"));
            }
            decode_c_string(payload)
                .map(|text| format!("文本={:?}", truncate_text(text.trim(), 48)))
                .or_else(|| {
                    (payload.len() <= 16 && !payload.is_empty())
                        .then(|| format!("载荷={}", hex_bytes(payload)))
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
            | GroupKind::ExpressionOffsetPoint
            | GroupKind::DerivedSegment24
            | GroupKind::CustomTransformPoint
            | GroupKind::Rotation
            | GroupKind::AngleRotation
            | GroupKind::ParameterRotation
            | GroupKind::ExpressionRotation
            | GroupKind::Scale
            | GroupKind::RatioScale
            | GroupKind::Reflection
            | GroupKind::PointTrace
            | GroupKind::MeasuredValue
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
            | GroupKind::Unknown(39)
            | GroupKind::Unknown(41)
            | GroupKind::Unknown(47)
            | GroupKind::Unknown(42)
            | GroupKind::Unknown(46)
            | GroupKind::Unknown(59)
            | GroupKind::Unknown(91)
            | GroupKind::Unknown(93)
            | GroupKind::Unknown(99)
            | GroupKind::Unknown(100)
            | GroupKind::Unknown(101)
            | GroupKind::Unknown(108)
            | GroupKind::Unknown(115)
            | GroupKind::Unknown(116)
            | GroupKind::Unknown(120)
            | GroupKind::Unknown(85)
            | GroupKind::Unknown(88)
            | GroupKind::LegacyCoordinateParameterHelper
            | GroupKind::LegacyCoordinatePointHelper
    )
}
