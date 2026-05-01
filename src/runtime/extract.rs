use anyhow::{Context, Result};
use std::collections::{BTreeMap, BTreeSet};

mod analysis;
mod assemble;
mod bindings;
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

use self::analysis::{
    BoundsData, CollectedShapes, SceneAnalysis, WorldData, analyze_scene,
    count_polygon_payload_color_bindings,
};
use self::assemble::{
    SceneAssemblyArtifacts, assemble_scene, build_world_data, compute_scene_bounds,
};
use self::bindings::{apply_payload_color_bindings, normalized_hsb, remap_scene_bindings};
use self::buttons::{ButtonIndexLookups, collect_buttons};
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
use self::images::collect_scene_images;
use self::labels::{
    HotspotIndexLookups, PendingLabelHotspot, bind_button_seed_expression_labels,
    bind_label_iteration_seed_anchors, bind_point_label_anchors, circle_parameter,
    collect_circle_parameter_labels, collect_coordinate_labels,
    collect_custom_transform_expression_labels, collect_iteration_tables, collect_label_iterations,
    collect_labels, collect_polygon_parameter_labels, collect_polyline_parameter_labels,
    collect_segment_parameter_labels, polygon_boundary_parameter, resolve_label_hotspots,
};
pub(crate) use self::payload_report::render_payload_log;
use self::payload_report::validate_scene_payloads;
#[cfg(test)]
use self::points::collect_visible_points_checked;
use self::points::{
    RawPointConstraint, TransformBindingKind, collect_non_graph_parameters,
    collect_point_iteration_points, collect_point_objects, collect_standalone_parameter_points,
    collect_visible_points_checked_with_context, decode_angle_rotation_anchor_raw,
    decode_line_midpoint_anchor_raw, decode_offset_anchor_raw,
    decode_parameter_controlled_anchor_raw, decode_parameter_rotation_anchor_raw,
    decode_point_constraint_anchor, decode_point_on_ray_anchor_raw,
    decode_point_pair_translation_anchor_raw, decode_reflection_anchor_raw,
    decode_regular_polygon_vertex_anchor_raw, decode_translated_point_anchor_raw,
    decode_translated_point_constraint, editable_non_graph_parameter_name_for_group,
    regular_polygon_iteration_step, remap_circle_bindings, remap_label_bindings,
    remap_line_bindings, remap_polygon_bindings, translation_point_pair_group_indices,
    try_decode_parameter_controlled_point, try_decode_parameter_rotation_binding,
    try_decode_point_constraint, try_decode_transform_binding,
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
use self::trace::{collect_point_traces, collect_segment_traces};
use super::functions::{
    collect_function_plot_domain, collect_function_plots, collect_scene_functions,
    collect_scene_parameters, collect_standalone_function_definitions, function_uses_pi_scale,
    synthesize_function_axes, synthesize_function_labels,
    synthesize_standalone_function_definition_labels, try_decode_function_expr,
    try_decode_function_plot_descriptor,
};
use super::geometry::{
    Bounds, GraphTransform, color_from_style, distance_world, line_is_dashed, to_world,
};
use super::scene::{
    ColorBinding, LabelIterationFamily, LineBinding, LineIterationFamily, LineShape,
    PayloadDebugSource, PointIterationFamily, PolygonIterationFamily, PolygonShape, Scene,
    ScenePoint, ScenePointConstraint, TextLabel,
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
    fill_visible: bool,
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

fn collect_scene_shapes(
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

fn collect_scene_labels(
    file: &GspFile,
    groups: &[ObjectGroup],
    context: &SceneContext<'_>,
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
        context,
        &analysis.raw_anchors,
    ));
    labels.extend(collect_polygon_parameter_labels(
        file,
        groups,
        &analysis.raw_anchors,
    ));
    labels.extend(collect_segment_parameter_labels(
        file,
        groups,
        &analysis.raw_anchors,
    ));
    labels.extend(collect_polyline_parameter_labels(
        file,
        groups,
        &analysis.raw_anchors,
    ));
    labels.extend(collect_custom_transform_expression_labels(
        file,
        groups,
        context,
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
    labels.extend(synthesize_standalone_function_definition_labels(
        file, groups, &labels,
    ));
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

pub(crate) fn build_scene_checked(file: &GspFile) -> Result<Scene> {
    let groups = file.object_groups();
    let context = SceneContext::new(file, &groups);
    validate_scene_payloads(file, &groups)?;
    let point_map = collect_point_objects(file, &groups);
    let analysis = analyze_scene(file, &groups, &context, &point_map);
    let mut shapes = collect_scene_shapes(file, &groups, &point_map, &analysis);
    let (images, image_group_to_index) = collect_scene_images(file, &groups, &analysis.graph_ref);
    let (mut labels, label_group_to_index, pending_hotspots) =
        collect_scene_labels(file, &groups, &context, &analysis, &shapes);

    let (mut visible_points, mut group_to_point_index) =
        collect_visible_points_checked_with_context(
            file,
            &groups,
            &context,
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
    bind_points_to_point_traces(
        file,
        &groups,
        &mut visible_points,
        &mut group_to_point_index,
        &shapes.coordinate_traces,
    );
    let existing_point_trace_ordinals = shapes
        .coordinate_traces
        .iter()
        .filter_map(|trace| trace.debug.as_ref().map(|debug| debug.group_ordinal))
        .collect::<BTreeSet<_>>();
    shapes.coordinate_traces.extend(
        collect_point_traces(
            file,
            &groups,
            &visible_points,
            &group_to_point_index,
            &analysis.graph_ref,
        )
        .into_iter()
        .filter(|trace| match trace.debug.as_ref() {
            Some(debug) => !existing_point_trace_ordinals.contains(&debug.group_ordinal),
            None => true,
        }),
    );
    shapes.coordinate_traces.extend(collect_segment_traces(
        file,
        &groups,
        &visible_points,
        &group_to_point_index,
        &analysis.graph_ref,
    ));
    let (mut point_trace_overlays, mut base_traces): (Vec<_>, Vec<_>) =
        shapes.coordinate_traces.drain(..).partition(|trace| {
            trace.visible && matches!(trace.binding, Some(LineBinding::PointTrace { .. }))
        });
    base_traces.append(&mut point_trace_overlays);
    shapes.coordinate_traces = base_traces;
    shapes
        .coordinate_traces
        .extend(collect_colorized_spectrum_lines(
            file,
            &groups,
            &visible_points,
            &group_to_point_index,
            800.0,
        ));
    let (derived_iteration_points, raw_point_iterations) =
        collect_point_iteration_points(file, &groups, &analysis.raw_anchors, &group_to_point_index);
    let standalone_parameter_points = collect_standalone_parameter_points(file, &groups);
    let label_iterations = collect_label_iterations(
        file,
        &groups,
        &label_group_to_index,
        &group_to_point_index,
        &analysis.raw_anchors,
    )
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
        LabelIterationFamily::TranslateExpression {
            seed_label_index,
            first_output_label_index,
            output_label_count,
            vector_start_index,
            vector_end_index,
            parameter_name,
            expr,
            depth,
            depth_expr,
            depth_parameter_name,
        } => LabelIterationFamily::TranslateExpression {
            seed_label_index,
            first_output_label_index,
            output_label_count,
            vector_start_index,
            vector_end_index,
            parameter_name,
            expr,
            depth,
            depth_expr,
            depth_parameter_name,
        },
    })
    .collect::<Vec<_>>();
    bind_button_seed_expression_labels(
        file,
        &groups,
        &context,
        &analysis.raw_anchors,
        &mut labels,
        &label_group_to_index,
        &group_to_point_index,
    );
    let iteration_tables = collect_iteration_tables(file, &groups, &context, &analysis.raw_anchors);
    remap_label_bindings(&mut labels, &group_to_point_index);
    bind_point_label_anchors(
        file,
        &groups,
        &analysis.raw_anchors,
        &group_to_point_index,
        &mut labels,
        &label_group_to_index,
    );
    bind_label_iteration_seed_anchors(
        file,
        &groups,
        &mut labels,
        &label_group_to_index,
        &label_iterations,
        &visible_points,
        &group_to_point_index,
    );
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
        &binding_maps.polygon_group_to_index,
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
    let show_hidden_parameter_controls = !analysis.graph_mode
        && analysis.graph_ref.is_some()
        && count_polygon_payload_color_bindings(&context) >= 10;
    parameters.extend(collect_non_graph_parameters(
        file,
        &groups,
        &mut labels,
        show_hidden_parameter_controls,
    ));
    let button_label_group_to_index =
        label_group_index_with_debug_ordinals(&labels, &label_group_to_index);
    let (buttons, button_group_to_index) = collect_buttons(
        file,
        &groups,
        &analysis.raw_anchors,
        ButtonIndexLookups {
            label_group_to_index: &button_label_group_to_index,
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
            shapes.segments.len()
                + shapes.lines.len()
                + shapes.rays.len()
                + shapes.translated_lines.len()
                + shapes.segment_markers.len()
                + shapes.rotated_lines.len()
                + shapes.scaled_lines.len()
                + shapes.reflected_lines.len()
                + shapes.derived_segments.len()
                + shapes.measurements.len()
                + shapes.coordinate_traces.len()
                + shapes.axes.len(),
        )
    } else {
        Vec::new()
    };
    let function_definitions = collect_standalone_function_definitions(file, &groups, &labels);
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
            function_definitions,
        },
    ))
}

fn bind_points_to_point_traces(
    file: &GspFile,
    groups: &[ObjectGroup],
    visible_points: &mut Vec<ScenePoint>,
    group_to_point_index: &mut [Option<usize>],
    point_trace_lines: &[LineShape],
) {
    for group in groups.iter().filter(|group| {
        matches!(
            group.header.kind(),
            GroupKind::PointConstraint | GroupKind::PathPoint | GroupKind::ParameterControlledPoint
        )
    }) {
        let Some(path) = find_indexed_path(file, group) else {
            continue;
        };
        let Some(host_ordinal) = path.refs.first().copied() else {
            continue;
        };
        let Some(host_group) = groups.get(host_ordinal.saturating_sub(1)) else {
            continue;
        };
        if host_group.header.kind() != GroupKind::PointTrace {
            continue;
        }
        let Some(group_index) = group.ordinal.checked_sub(1) else {
            continue;
        };
        let Some(slot) = group_to_point_index.get_mut(group_index) else {
            continue;
        };
        let existing_point_index = *slot;
        let Some(payload) = group
            .records
            .iter()
            .find(|record| record.record_type == RECORD_BINDING_PAYLOAD)
            .map(|record| record.payload(&file.data))
        else {
            continue;
        };
        if payload.len() < 12 {
            continue;
        }
        let normalized_t = read_f64(payload, 4);
        if !normalized_t.is_finite() {
            continue;
        }
        let Some(trace_line) = point_trace_lines.iter().find(|line| {
            line.debug
                .as_ref()
                .is_some_and(|debug| debug.group_ordinal == host_ordinal)
                && matches!(line.binding, Some(LineBinding::PointTrace { .. }))
        }) else {
            continue;
        };
        if trace_line.points.len() < 2 {
            continue;
        }
        let wrapped_t = normalized_t.rem_euclid(1.0);
        let scaled = wrapped_t * (trace_line.points.len() - 1) as f64;
        let segment_index = (scaled.floor() as usize).min(trace_line.points.len() - 2);
        let t = scaled.fract();
        let start = &trace_line.points[segment_index];
        let end = &trace_line.points[segment_index + 1];
        let position = PointRecord {
            x: start.x + (end.x - start.x) * t,
            y: start.y + (end.y - start.y) * t,
        };
        let point_index = existing_point_index.unwrap_or_else(|| {
            let next_index = visible_points.len();
            *slot = Some(next_index);
            visible_points.push(ScenePoint {
                position: position.clone(),
                color: color_from_style(group.header.style_b),
                visible: !group.header.is_hidden(),
                draggable: true,
                constraint: ScenePointConstraint::Free,
                binding: None,
                debug: Some(payload_debug_source(group)),
            });
            next_index
        });
        if let Some(point) = visible_points.get_mut(point_index) {
            point.position = position;
            point.constraint = ScenePointConstraint::OnPolyline {
                function_key: host_ordinal,
                points: trace_line.points.clone(),
                segment_index,
                t,
            };
            point.draggable = true;
        }
    }
}

fn collect_colorized_spectrum_lines(
    file: &GspFile,
    groups: &[ObjectGroup],
    visible_points: &[ScenePoint],
    group_to_point_index: &[Option<usize>],
    viewport_width: f64,
) -> Vec<LineShape> {
    let mut lines = Vec::new();
    let mut seen = BTreeSet::new();
    for binding_group in groups
        .iter()
        .filter(|group| group.header.kind() == GroupKind::IterationBinding)
    {
        let Some(binding_path) = find_indexed_path(file, binding_group) else {
            continue;
        };
        let Some(source_ordinal) = binding_path.refs.first().copied() else {
            continue;
        };
        let Some(iter_ordinal) = binding_path.refs.get(1).copied() else {
            continue;
        };
        let Some(source_group) = groups.get(source_ordinal.saturating_sub(1)) else {
            continue;
        };
        if source_group.header.kind() != GroupKind::DerivedSegment75 {
            continue;
        }
        if !seen.insert(source_ordinal) {
            continue;
        }
        let Some(source_path) = find_indexed_path(file, source_group) else {
            continue;
        };
        let Some(host_ordinal) = source_path.refs.first().copied() else {
            continue;
        };
        let Some(host_group) = groups.get(host_ordinal.saturating_sub(1)) else {
            continue;
        };
        if !matches!(
            host_group.header.kind(),
            GroupKind::Segment | GroupKind::Ray
        ) {
            continue;
        }
        let Some(host_path) = find_indexed_path(file, host_group) else {
            continue;
        };
        if host_path.refs.len() != 2 {
            continue;
        }
        let Some((trace_point_ordinal, trace_endpoint_index, other_ordinal)) = host_path
            .refs
            .iter()
            .copied()
            .enumerate()
            .find_map(|(endpoint_index, ordinal)| {
                let point_index = group_to_point_index
                    .get(ordinal.checked_sub(1)?)
                    .copied()
                    .flatten()?;
                let point = visible_points.get(point_index)?;
                matches!(point.constraint, ScenePointConstraint::OnPolyline { .. }).then_some((
                    ordinal,
                    endpoint_index,
                    *host_path
                        .refs
                        .iter()
                        .find(|candidate| **candidate != ordinal)?,
                ))
            })
        else {
            continue;
        };
        let Some(trace_point_group_index) = trace_point_ordinal.checked_sub(1) else {
            continue;
        };
        let Some(trace_point_index) = group_to_point_index
            .get(trace_point_group_index)
            .copied()
            .flatten()
        else {
            continue;
        };
        let Some(host_group_index) = host_ordinal.checked_sub(1) else {
            continue;
        };
        let Some(trace_point) = visible_points.get(trace_point_index) else {
            continue;
        };
        let ScenePointConstraint::OnPolyline {
            function_key,
            points,
            segment_index,
            t,
            ..
        } = &trace_point.constraint
        else {
            continue;
        };
        let Some(trace_line_group_index) = function_key.checked_sub(1) else {
            continue;
        };
        if points.len() < 2 {
            continue;
        }
        let Some(iter_group) = groups.get(iter_ordinal.saturating_sub(1)) else {
            continue;
        };
        let depth = iter_group
            .records
            .iter()
            .find(|record| record.record_type == 0x090a)
            .map(|record| record.payload(&file.data))
            .filter(|payload| payload.len() >= 20)
            .map(|payload| read_u32(payload, 16) as usize)
            .unwrap_or(0);
        if depth == 0 {
            continue;
        }
        let depth_parameter_name = iteration_depth_parameter_name(file, groups, iter_group);
        let base = (*segment_index as f64 + *t) / (points.len() - 1) as f64;
        let other_point = group_to_point_index
            .get(other_ordinal.saturating_sub(1))
            .copied()
            .flatten()
            .and_then(|point_index| visible_points.get(point_index))
            .map(|point| point.position.clone());
        let reflected_endpoint = groups
            .get(other_ordinal.saturating_sub(1))
            .filter(|group| group.header.kind() == GroupKind::Reflection)
            .and_then(|group| find_indexed_path(file, group))
            .and_then(|path| {
                Some((
                    path.refs.first()?.checked_sub(1)?,
                    path.refs.get(1)?.checked_sub(1)?,
                ))
            });
        let sampled_reflection_axis = reflected_endpoint.and_then(|(_, axis_line_group_index)| {
            sampled_reflection_axis_driver(
                file,
                groups,
                axis_line_group_index,
                trace_point_group_index,
            )
        });
        for step in 0..depth {
            let normalized = (base + step as f64 / depth as f64).rem_euclid(1.0);
            let Some(start) = interpolate_polyline(points, normalized) else {
                continue;
            };
            let end = if host_group.header.kind() == GroupKind::Ray {
                PointRecord {
                    x: viewport_width,
                    y: start.y,
                }
            } else {
                let Some(other) = other_point.clone() else {
                    continue;
                };
                other
            };
            let [red, green, blue] = normalized_hsb(step as f64 / depth as f64, 1.0, 1.0);
            lines.push(LineShape {
                points: vec![start, end],
                color: [red, green, blue, 255],
                dashed: false,
                visible: !iter_group.header.is_hidden(),
                binding: Some(LineBinding::ColorizedSpectrum {
                    line_index: host_group_index,
                    trace_line_index: trace_line_group_index,
                    point_index: trace_point_group_index,
                    trace_endpoint_index,
                    reflection_source_index: reflected_endpoint
                        .map(|(source_index, _)| source_index),
                    reflection_axis_line_index: reflected_endpoint
                        .map(|(_, line_index)| line_index),
                    reflection_focus_index: sampled_reflection_axis
                        .map(|(focus_group_index, _)| focus_group_index),
                    reflection_directrix_line_index: sampled_reflection_axis
                        .map(|(_, directrix_line_group_index)| directrix_line_group_index),
                    step_index: step,
                    depth,
                    depth_parameter_name: depth_parameter_name.clone(),
                    ray: host_group.header.kind() == GroupKind::Ray,
                }),
                debug: Some(payload_debug_source(source_group)),
            });
        }
    }
    lines
}

fn sampled_reflection_axis_driver(
    file: &GspFile,
    groups: &[ObjectGroup],
    axis_line_group_index: usize,
    trace_point_group_index: usize,
) -> Option<(usize, usize)> {
    let axis_group = groups.get(axis_line_group_index)?;
    if axis_group.header.kind() != GroupKind::LineKind5 {
        return None;
    }
    let axis_path = find_indexed_path(file, axis_group)?;
    let [through_ordinal, host_line_ordinal] = axis_path.refs.as_slice() else {
        return None;
    };
    if through_ordinal.checked_sub(1)? != trace_point_group_index {
        return None;
    }

    let host_line_group = groups.get(host_line_ordinal.checked_sub(1)?)?;
    let host_line_path = find_indexed_path(file, host_line_group)?;
    let intersection_group_index = host_line_path.refs.iter().find_map(|ordinal| {
        let index = ordinal.checked_sub(1)?;
        (index != trace_point_group_index).then_some(index)
    })?;

    let intersection_group = groups.get(intersection_group_index)?;
    let intersection_path = find_indexed_path(file, intersection_group)?;
    let mut directrix_line_group_index = None;
    let mut bisector_group_index = None;
    for ordinal in &intersection_path.refs {
        let index = ordinal.checked_sub(1)?;
        let group = groups.get(index)?;
        if group.header.kind() == GroupKind::LineKind7 {
            bisector_group_index = Some(index);
        } else {
            directrix_line_group_index = Some(index);
        }
    }
    let directrix_line_group_index = directrix_line_group_index?;
    let bisector_group = groups.get(bisector_group_index?)?;
    let bisector_path = find_indexed_path(file, bisector_group)?;
    let [focus_ordinal, vertex_ordinal, _] = bisector_path.refs.as_slice() else {
        return None;
    };
    if vertex_ordinal.checked_sub(1)? != trace_point_group_index {
        return None;
    }
    Some((focus_ordinal.checked_sub(1)?, directrix_line_group_index))
}

fn iteration_depth_parameter_name(
    file: &GspFile,
    groups: &[ObjectGroup],
    iter_group: &ObjectGroup,
) -> Option<String> {
    let iter_path = find_indexed_path(file, iter_group)?;
    let parameter_group = groups.get(iter_path.refs.first()?.checked_sub(1)?)?;
    self::decode::decode_label_name(file, parameter_group)
        .or_else(|| editable_non_graph_parameter_name_for_group(file, groups, parameter_group))
}

fn interpolate_polyline(points: &[PointRecord], normalized: f64) -> Option<PointRecord> {
    if points.len() < 2 {
        return None;
    }
    let scaled = normalized.rem_euclid(1.0) * (points.len() - 1) as f64;
    let segment_index = (scaled.floor() as usize).min(points.len() - 2);
    let t = scaled.fract();
    let start = points.get(segment_index)?;
    let end = points.get(segment_index + 1)?;
    Some(PointRecord {
        x: start.x + (end.x - start.x) * t,
        y: start.y + (end.y - start.y) * t,
    })
}

fn label_group_index_with_debug_ordinals(
    labels: &[TextLabel],
    base: &BTreeMap<usize, usize>,
) -> BTreeMap<usize, usize> {
    let mut expanded = base.clone();
    for (index, label) in labels.iter().enumerate() {
        if let Some(group_ordinal) = label.debug.as_ref().map(|debug| debug.group_ordinal) {
            expanded.entry(group_ordinal).or_insert(index);
        }
    }
    expanded
}
