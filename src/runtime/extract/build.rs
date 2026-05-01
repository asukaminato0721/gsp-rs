use std::collections::{BTreeMap, BTreeSet};

use super::analysis::{CollectedShapes, SceneAnalysis};
use super::*;
use crate::runtime::scene::SceneParameter;

pub(in crate::runtime) fn payload_debug_source(group: &ObjectGroup) -> PayloadDebugSource {
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

    let (visible_points, group_to_point_index) = collect_visible_points_and_traces(
        file,
        &groups,
        &context,
        &point_map,
        &analysis,
        &mut shapes,
    )?;
    let (derived_iteration_points, raw_point_iterations) =
        collect_point_iteration_points(file, &groups, &analysis.raw_anchors, &group_to_point_index);
    let standalone_parameter_points = collect_standalone_parameter_points(file, &groups);
    let label_iterations = collect_label_iterations(
        file,
        &groups,
        &label_group_to_index,
        &group_to_point_index,
        &analysis.raw_anchors,
    );
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

    let parameters = collect_parameters(file, &groups, &context, &analysis, &mut labels);
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
            function_shape_layer_count(&shapes),
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

fn collect_visible_points_and_traces(
    file: &GspFile,
    groups: &[ObjectGroup],
    context: &SceneContext<'_>,
    point_map: &[Option<PointRecord>],
    analysis: &SceneAnalysis,
    shapes: &mut CollectedShapes,
) -> Result<(Vec<ScenePoint>, Vec<Option<usize>>)> {
    let (mut visible_points, mut group_to_point_index) =
        collect_visible_points_checked_with_context(
            file,
            groups,
            context,
            point_map,
            &analysis.raw_anchors,
            &analysis.graph_ref,
        )?;
    shapes.coordinate_traces.extend(collect_point_traces(
        file,
        groups,
        &visible_points,
        &group_to_point_index,
        &analysis.graph_ref,
    ));
    bind_points_to_point_traces(
        file,
        groups,
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
            groups,
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
        groups,
        &visible_points,
        &group_to_point_index,
        &analysis.graph_ref,
    ));
    move_point_trace_overlays_to_end(&mut shapes.coordinate_traces);
    shapes
        .coordinate_traces
        .extend(collect_colorized_spectrum_lines(
            file,
            groups,
            &visible_points,
            &group_to_point_index,
            800.0,
        ));
    Ok((visible_points, group_to_point_index))
}

fn move_point_trace_overlays_to_end(traces: &mut Vec<LineShape>) {
    let (mut point_trace_overlays, mut base_traces): (Vec<_>, Vec<_>) =
        traces.drain(..).partition(|trace| {
            trace.visible && matches!(trace.binding, Some(LineBinding::PointTrace { .. }))
        });
    base_traces.append(&mut point_trace_overlays);
    *traces = base_traces;
}

fn collect_parameters(
    file: &GspFile,
    groups: &[ObjectGroup],
    context: &SceneContext<'_>,
    analysis: &SceneAnalysis,
    labels: &mut [TextLabel],
) -> Vec<SceneParameter> {
    let mut parameters = if analysis.graph_mode {
        collect_scene_parameters(file, groups, labels)
    } else {
        Vec::new()
    };
    let show_hidden_parameter_controls = !analysis.graph_mode
        && analysis.graph_ref.is_some()
        && count_polygon_payload_color_bindings(context) >= 10;
    parameters.extend(collect_non_graph_parameters(
        file,
        groups,
        labels,
        show_hidden_parameter_controls,
    ));
    parameters
}

fn function_shape_layer_count(shapes: &CollectedShapes) -> usize {
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
        + shapes.axes.len()
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
