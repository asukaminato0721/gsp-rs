use std::collections::{BTreeMap, BTreeSet};

use anyhow::Result;

use super::analysis::{CollectedShapes, SceneAnalysis, analyze_scene};
use super::assemble::{
    SceneAssemblyArtifacts, assemble_scene, build_world_data, compute_scene_bounds,
};
use super::bindings::{BindingMaps, apply_payload_color_bindings, remap_scene_bindings};
use super::buttons::{ButtonIndexLookups, collect_buttons};
use super::context::SceneContext;
use super::images::collect_scene_images;
use super::labels::{
    HotspotIndexLookups, PendingLabelHotspot, bind_button_seed_expression_labels,
    bind_label_iteration_seed_anchors, bind_point_label_anchors, collect_iteration_tables,
    collect_label_iterations, collect_scene_labels, resolve_label_hotspots,
};
use super::payload_report::validate_scene_payloads;
use super::points::{
    RawPointIterationFamily, collect_non_graph_parameters, collect_point_iteration_points,
    collect_point_objects, collect_standalone_parameter_points,
    collect_visible_points_checked_with_context, remap_label_bindings,
};
use super::shapes::{collect_carried_circle_iteration_families, collect_scene_shapes};
use super::trace::{
    bind_points_to_point_traces, collect_colorized_spectrum_lines, collect_point_traces,
    collect_segment_traces,
};
use crate::format::{GspFile, ObjectGroup, PointRecord, record_name};
use crate::runtime::functions::{
    collect_scene_functions, collect_scene_parameters, collect_standalone_function_definitions,
    with_numeric_helper_cache,
};
use crate::runtime::scene::{
    LabelIterationFamily, LineBinding, LineIterationFamily, LineShape, PayloadDebugSource,
    PolygonIterationFamily, Scene, SceneParameter, ScenePoint, TextLabel,
};

struct PointStage {
    visible_points: Vec<ScenePoint>,
    group_to_point_index: Vec<Option<usize>>,
    derived_iteration_points: Vec<ScenePoint>,
    standalone_parameter_points: Vec<ScenePoint>,
    raw_point_iterations: Vec<RawPointIterationFamily>,
}

struct LabelStage {
    labels: Vec<TextLabel>,
    label_group_to_index: BTreeMap<usize, usize>,
    label_iterations: Vec<LabelIterationFamily>,
    pending_hotspots: Vec<PendingLabelHotspot>,
}

struct BindingStage {
    maps: BindingMaps,
    line_iterations: Vec<LineIterationFamily>,
    polygon_iterations: Vec<PolygonIterationFamily>,
}

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
    with_numeric_helper_cache(|| build_scene_checked_inner(file))
}

fn build_scene_checked_inner(file: &GspFile) -> Result<Scene> {
    let groups = file.object_groups();
    validate_scene_payloads(file, &groups)?;
    let context = SceneContext::new(file, &groups);
    let point_map = collect_point_objects(file, &groups);
    let analysis = analyze_scene(file, &groups, &point_map);
    let mut shapes = collect_scene_shapes(file, &groups, &context, &analysis);
    let (images, image_group_to_index) = collect_scene_images(file, &groups, &analysis.graph_ref);
    let mut label_stage = collect_label_stage(file, &groups, &context, &analysis, &shapes);
    let point_stage =
        collect_point_stage(file, &groups, &context, &point_map, &analysis, &mut shapes)?;
    complete_label_stage(
        file,
        &groups,
        &context,
        &analysis,
        &point_stage,
        &mut label_stage,
    );
    let iteration_tables = collect_iteration_tables(
        file,
        &groups,
        &context,
        &analysis.raw_anchors,
        &point_stage.group_to_point_index,
    );
    let binding_stage = remap_binding_stage(
        file,
        &groups,
        &analysis,
        &point_stage.group_to_point_index,
        &mut shapes,
    );
    apply_payload_color_bindings(
        file,
        &groups,
        &analysis.raw_anchors,
        &point_stage.group_to_point_index,
        &binding_stage.maps.circle_group_to_index,
        &binding_stage.maps.polygon_group_to_index,
        &mut shapes,
    );
    let circle_iterations = collect_carried_circle_iteration_families(
        file,
        &groups,
        &analysis.raw_anchors,
        &point_stage.group_to_point_index,
        &binding_stage.maps.circle_group_to_index,
    );
    let world_data = build_world_data(
        &analysis,
        &point_stage.visible_points,
        &point_stage.derived_iteration_points,
        &point_stage.standalone_parameter_points,
        point_stage.raw_point_iterations,
    );
    let bounds_data = compute_scene_bounds(
        &analysis,
        &shapes,
        &label_stage.labels,
        &world_data.world_point_positions,
    );

    let parameters = collect_parameters(file, &groups, &analysis, &mut label_stage.labels);
    let button_label_group_to_index = label_group_index_with_debug_ordinals(
        &label_stage.labels,
        &label_stage.label_group_to_index,
    );
    let line_iteration_group_to_index = binding_stage
        .line_iterations
        .iter()
        .enumerate()
        .map(|(index, family)| (family.binding_group_ordinal(), index))
        .collect::<BTreeMap<_, _>>();
    let polygon_iteration_group_to_index = binding_stage
        .polygon_iterations
        .iter()
        .enumerate()
        .map(|(index, family)| (family.binding_group_ordinal(), index))
        .collect::<BTreeMap<_, _>>();
    let (buttons, button_group_to_index) = collect_buttons(
        file,
        &groups,
        &analysis.raw_anchors,
        ButtonIndexLookups {
            label_group_to_index: &button_label_group_to_index,
            image_group_to_index: &image_group_to_index,
            group_to_point_index: &point_stage.group_to_point_index,
            line_group_to_index: &binding_stage.maps.line_group_to_index,
            circle_group_to_index: &binding_stage.maps.circle_group_to_index,
            polygon_group_to_index: &binding_stage.maps.polygon_group_to_index,
            line_iteration_group_to_index: &line_iteration_group_to_index,
            polygon_iteration_group_to_index: &polygon_iteration_group_to_index,
        },
    );
    resolve_label_hotspots(
        file,
        &groups,
        &mut label_stage.labels,
        &label_stage.pending_hotspots,
        HotspotIndexLookups {
            group_to_point_index: &point_stage.group_to_point_index,
            circle_group_to_index: &binding_stage.maps.circle_group_to_index,
            polygon_group_to_index: &binding_stage.maps.polygon_group_to_index,
            button_group_to_index: &button_group_to_index,
        },
    );
    let functions = if analysis.graph_mode {
        collect_scene_functions(
            file,
            &groups,
            &label_stage.labels,
            &world_data.world_points,
            function_shape_layer_count(&shapes),
        )
    } else {
        Vec::new()
    };
    let function_definitions =
        collect_standalone_function_definitions(file, &groups, &label_stage.labels);
    Ok(assemble_scene(
        analysis,
        shapes,
        label_stage.labels,
        world_data,
        bounds_data,
        SceneAssemblyArtifacts {
            circle_iterations,
            line_iterations: binding_stage.line_iterations,
            polygon_iterations: binding_stage.polygon_iterations,
            label_iterations: label_stage.label_iterations,
            iteration_tables,
            buttons,
            images,
            parameters,
            functions,
            function_definitions,
        },
    ))
}

fn collect_label_stage(
    file: &GspFile,
    groups: &[ObjectGroup],
    context: &SceneContext<'_>,
    analysis: &SceneAnalysis,
    shapes: &CollectedShapes,
) -> LabelStage {
    let (labels, label_group_to_index, pending_hotspots) =
        collect_scene_labels(file, groups, context, analysis, shapes);
    LabelStage {
        labels,
        label_group_to_index,
        label_iterations: Vec::new(),
        pending_hotspots,
    }
}

fn collect_point_stage(
    file: &GspFile,
    groups: &[ObjectGroup],
    context: &SceneContext<'_>,
    point_map: &[Option<PointRecord>],
    analysis: &SceneAnalysis,
    shapes: &mut CollectedShapes,
) -> Result<PointStage> {
    let (visible_points, group_to_point_index) =
        collect_visible_points_and_traces(file, groups, context, point_map, analysis, shapes)?;
    let (derived_iteration_points, raw_point_iterations) =
        collect_point_iteration_points(file, groups, &analysis.raw_anchors, &group_to_point_index);
    let standalone_parameter_points = collect_standalone_parameter_points(file, groups);
    Ok(PointStage {
        visible_points,
        group_to_point_index,
        derived_iteration_points,
        standalone_parameter_points,
        raw_point_iterations,
    })
}

fn complete_label_stage(
    file: &GspFile,
    groups: &[ObjectGroup],
    context: &SceneContext<'_>,
    analysis: &SceneAnalysis,
    point_stage: &PointStage,
    label_stage: &mut LabelStage,
) {
    label_stage.label_iterations = collect_label_iterations(
        file,
        groups,
        &label_stage.label_group_to_index,
        &point_stage.group_to_point_index,
        &analysis.raw_anchors,
    );
    bind_button_seed_expression_labels(
        file,
        groups,
        context,
        &analysis.raw_anchors,
        &mut label_stage.labels,
        &label_stage.label_group_to_index,
        &point_stage.group_to_point_index,
    );
    remap_label_bindings(&mut label_stage.labels, &point_stage.group_to_point_index);
    bind_point_label_anchors(
        file,
        groups,
        &analysis.raw_anchors,
        &point_stage.group_to_point_index,
        &mut label_stage.labels,
        &label_stage.label_group_to_index,
    );
    bind_label_iteration_seed_anchors(
        file,
        groups,
        &mut label_stage.labels,
        &label_stage.label_group_to_index,
        &label_stage.label_iterations,
        &point_stage.visible_points,
        &point_stage.group_to_point_index,
    );
}

fn remap_binding_stage(
    file: &GspFile,
    groups: &[ObjectGroup],
    analysis: &SceneAnalysis,
    group_to_point_index: &[Option<usize>],
    shapes: &mut CollectedShapes,
) -> BindingStage {
    let (maps, line_iterations, polygon_iterations) = remap_scene_bindings(
        file,
        groups,
        &analysis.raw_anchors,
        group_to_point_index,
        analysis.function_plots.len(),
        shapes,
    );
    BindingStage {
        maps,
        line_iterations,
        polygon_iterations,
    }
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
    shapes.trace_lines.extend(collect_point_traces(
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
        &shapes.trace_lines,
    );
    let existing_point_trace_ordinals = shapes
        .trace_lines
        .iter()
        .filter_map(|trace| trace.debug.as_ref().map(|debug| debug.group_ordinal))
        .collect::<BTreeSet<_>>();
    shapes.trace_lines.extend(
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
    bind_points_to_point_traces(
        file,
        groups,
        &mut visible_points,
        &mut group_to_point_index,
        &shapes.trace_lines,
    );
    shapes.trace_lines.extend(collect_segment_traces(
        file,
        groups,
        &visible_points,
        &group_to_point_index,
        &analysis.graph_ref,
    ));
    move_point_trace_overlays_to_end(&mut shapes.trace_lines);
    shapes.trace_lines.extend(collect_colorized_spectrum_lines(
        file,
        groups,
        &analysis.raw_anchors,
        &visible_points,
        &group_to_point_index,
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
    analysis: &SceneAnalysis,
    labels: &mut [TextLabel],
) -> Vec<SceneParameter> {
    let mut parameters = if analysis.graph_mode {
        collect_scene_parameters(file, groups, labels)
    } else {
        Vec::new()
    };
    parameters.extend(collect_non_graph_parameters(file, groups, labels));
    parameters
}

fn function_shape_layer_count(shapes: &CollectedShapes) -> usize {
    shapes.lines.len() + shapes.trace_lines.len() + shapes.axes.len()
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
