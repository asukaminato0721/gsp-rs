use std::collections::{BTreeMap, BTreeSet};

mod buttons;
mod decode;
mod graph;
mod labels;
mod points;
mod shapes;
#[cfg(test)]
mod tests;
mod world;

use self::buttons::collect_buttons;
use crate::format::{GspFile, ObjectGroup, PointRecord, read_f64, read_u32};

use self::graph::{
    bounds_within, collect_bounds, collect_saved_viewport, dedupe_line_shapes,
    detect_graph_transform, expand_bounds, has_graph_classes,
};
use self::labels::{
    PendingLabelHotspot, collect_circle_parameter_labels, collect_coordinate_labels,
    collect_label_iterations, collect_labels, collect_polygon_parameter_labels,
    collect_segment_parameter_labels, compute_iteration_labels, resolve_label_hotspots,
};
use self::points::{
    RawPointIterationFamily, TransformBindingKind, collect_non_graph_parameters,
    collect_point_iteration_points, collect_point_objects, collect_visible_points,
    decode_line_midpoint_anchor_raw, decode_offset_anchor_raw,
    decode_parameter_controlled_anchor_raw, decode_parameter_rotation_anchor_raw,
    decode_parameter_rotation_binding, decode_point_constraint_anchor,
    decode_point_on_ray_anchor_raw, decode_point_pair_translation_anchor_raw,
    decode_reflection_anchor_raw, decode_regular_polygon_vertex_anchor_raw,
    decode_transform_binding, decode_translated_point_anchor_raw, reflection_line_group_indices,
    regular_polygon_iteration_step, remap_circle_bindings, remap_label_bindings,
    remap_line_bindings, remap_polygon_bindings, translation_point_pair_group_indices,
};
use self::shapes::{
    collect_bound_line_shapes, collect_carried_iteration_lines, collect_carried_iteration_polygons,
    collect_carried_line_iteration_families, collect_carried_polygon_edge_segment_groups,
    collect_carried_polygon_iteration_families, collect_circle_shapes, collect_coordinate_traces,
    collect_derived_segments, collect_iteration_shapes, collect_line_shapes,
    collect_polygon_shapes, collect_raw_object_anchors, collect_reflected_circle_shapes,
    collect_reflected_line_shapes, collect_reflected_polygon_shapes, collect_rotated_circle_shapes,
    collect_rotated_line_shapes, collect_rotated_polygon_shapes,
    collect_rotational_iteration_lines, collect_scaled_line_shapes, collect_segment_marker_shapes,
    collect_three_point_arc_shapes, collect_transformed_circle_shapes,
    collect_transformed_polygon_shapes, collect_translated_line_shapes,
    collect_translated_polygon_shapes,
};

use self::world::{world_line_iteration_family, world_line_shape, world_polygon_iteration_family};
use super::functions::{
    collect_function_plot_domain, collect_function_plots, collect_scene_functions,
    collect_scene_parameters, decode_function_plot_descriptor, function_uses_pi_scale,
    synthesize_function_axes, synthesize_function_labels,
};
use super::geometry::{
    Bounds, GraphTransform, distance_world, include_line_bounds, lerp_point, point_on_circle_arc,
    point_on_three_point_arc, reflect_across_line, rotate_around, scale_around, to_world,
};
use super::scene::{
    LabelIterationFamily, LineBinding, LineIterationFamily, LineShape, PointIterationFamily,
    PolygonIterationFamily, PolygonShape, Scene, SceneArc, SceneCircle, ScenePoint,
    ScenePointBinding, ScenePointConstraint, TextLabel,
};

pub(crate) use self::decode::{find_indexed_path, is_circle_group_kind};

#[derive(Debug, Clone)]
struct CircleShape {
    center: PointRecord,
    radius_point: PointRecord,
    color: [u8; 4],
    dashed: bool,
    binding: Option<super::scene::ShapeBinding>,
}

#[derive(Debug, Clone)]
struct ArcShape {
    points: [PointRecord; 3],
    color: [u8; 4],
    center: Option<PointRecord>,
    counterclockwise: bool,
}

struct SceneAnalysis {
    graph_mode: bool,
    graph_ref: Option<GraphTransform>,
    saved_viewport: Option<Bounds>,
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
    let pi_mode = if graph_mode {
        saved_viewport.is_some() || function_uses_pi_scale(file, groups)
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
    let has_coordinate_objects = groups.iter().any(|group| {
        matches!(
            group.header.kind(),
            crate::format::GroupKind::CoordinatePoint | crate::format::GroupKind::CoordinateTrace
        )
    });
    let has_iteration_helpers = groups.iter().any(|group| {
        matches!(
            group.header.kind(),
            crate::format::GroupKind::AffineIteration
                | crate::format::GroupKind::IterationBinding
                | crate::format::GroupKind::RegularPolygonIteration
        )
    });
    let large_non_graph = !graph_mode && file.records.len() > 10_000;

    SceneAnalysis {
        graph_mode,
        graph_ref,
        saved_viewport,
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
    let suppressed_carried_polygon_segments =
        collect_carried_polygon_edge_segment_groups(file, groups);
    let polylines = collect_line_shapes(
        file,
        groups,
        &analysis.raw_anchors,
        &[
            crate::format::GroupKind::Segment,
            crate::format::GroupKind::AngleMarker,
        ],
        !analysis.graph_mode && !analysis.large_non_graph,
        &suppressed_carried_polygon_segments,
    );
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
        &suppressed_carried_polygon_segments,
    );
    let carried_iteration_polygons =
        collect_carried_iteration_polygons(file, groups, &analysis.raw_anchors);
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
        collect_coordinate_traces(file, groups, &analysis.graph_ref)
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
        polylines,
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
                color: [30, 30, 30, 255],
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
        matches!(
            group.header.kind(),
            crate::format::GroupKind::Segment
                | crate::format::GroupKind::AngleMarker
                | crate::format::GroupKind::LineKind5
                | crate::format::GroupKind::LineKind6
                | crate::format::GroupKind::LineKind7
        )
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

fn build_world_data(
    analysis: &SceneAnalysis,
    visible_points: &[ScenePoint],
    derived_iteration_points: &[ScenePoint],
    raw_point_iterations: Vec<RawPointIterationFamily>,
) -> WorldData {
    let world_points = visible_points
        .iter()
        .chain(derived_iteration_points.iter())
        .map(|point| ScenePoint {
            position: to_world(&point.position, &analysis.graph_ref),
            visible: point.visible,
            constraint: match &point.constraint {
                ScenePointConstraint::Free => ScenePointConstraint::Free,
                ScenePointConstraint::Offset {
                    origin_index,
                    dx,
                    dy,
                } => {
                    let (dx, dy) = if let Some(transform) = &analysis.graph_ref {
                        (dx / transform.raw_per_unit, -dy / transform.raw_per_unit)
                    } else {
                        (*dx, *dy)
                    };
                    ScenePointConstraint::Offset {
                        origin_index: *origin_index,
                        dx,
                        dy,
                    }
                }
                ScenePointConstraint::OnSegment {
                    start_index,
                    end_index,
                    t,
                } => ScenePointConstraint::OnSegment {
                    start_index: *start_index,
                    end_index: *end_index,
                    t: *t,
                },
                ScenePointConstraint::OnPolyline {
                    function_key,
                    points,
                    segment_index,
                    t,
                } => ScenePointConstraint::OnPolyline {
                    function_key: *function_key,
                    points: points
                        .iter()
                        .map(|point| to_world(point, &analysis.graph_ref))
                        .collect(),
                    segment_index: *segment_index,
                    t: *t,
                },
                ScenePointConstraint::OnPolygonBoundary {
                    vertex_indices,
                    edge_index,
                    t,
                } => ScenePointConstraint::OnPolygonBoundary {
                    vertex_indices: vertex_indices.clone(),
                    edge_index: *edge_index,
                    t: *t,
                },
                ScenePointConstraint::OnCircle {
                    center_index,
                    radius_index,
                    unit_x,
                    unit_y,
                } => ScenePointConstraint::OnCircle {
                    center_index: *center_index,
                    radius_index: *radius_index,
                    unit_x: *unit_x,
                    unit_y: if analysis.graph_ref.is_some() {
                        *unit_y
                    } else {
                        -*unit_y
                    },
                },
                ScenePointConstraint::OnCircleArc {
                    center_index,
                    start_index,
                    end_index,
                    t,
                } => ScenePointConstraint::OnCircleArc {
                    center_index: *center_index,
                    start_index: *start_index,
                    end_index: *end_index,
                    t: *t,
                },
                ScenePointConstraint::OnArc {
                    start_index,
                    mid_index,
                    end_index,
                    t,
                } => ScenePointConstraint::OnArc {
                    start_index: *start_index,
                    mid_index: *mid_index,
                    end_index: *end_index,
                    t: *t,
                },
                ScenePointConstraint::LineIntersection {
                    left_kind,
                    left_start_index,
                    left_end_index,
                    right_kind,
                    right_start_index,
                    right_end_index,
                } => ScenePointConstraint::LineIntersection {
                    left_kind: *left_kind,
                    left_start_index: *left_start_index,
                    left_end_index: *left_end_index,
                    right_kind: *right_kind,
                    right_start_index: *right_start_index,
                    right_end_index: *right_end_index,
                },
                ScenePointConstraint::LineCircleIntersection {
                    line_kind,
                    line_start_index,
                    line_end_index,
                    center_index,
                    radius_index,
                    variant,
                } => ScenePointConstraint::LineCircleIntersection {
                    line_kind: *line_kind,
                    line_start_index: *line_start_index,
                    line_end_index: *line_end_index,
                    center_index: *center_index,
                    radius_index: *radius_index,
                    variant: *variant,
                },
                ScenePointConstraint::CircleCircleIntersection {
                    left_center_index,
                    left_radius_index,
                    right_center_index,
                    right_radius_index,
                    variant,
                } => ScenePointConstraint::CircleCircleIntersection {
                    left_center_index: *left_center_index,
                    left_radius_index: *left_radius_index,
                    right_center_index: *right_center_index,
                    right_radius_index: *right_radius_index,
                    variant: *variant,
                },
            },
            binding: point.binding.clone(),
        })
        .collect::<Vec<_>>();

    let world_point_positions = world_points
        .iter()
        .filter(|point| point.visible)
        .map(|point| point.position.clone())
        .collect::<Vec<_>>();

    let point_iterations = raw_point_iterations
        .into_iter()
        .map(|family| match family {
            RawPointIterationFamily::Offset {
                seed_index,
                dx,
                dy,
                depth,
                parameter_name,
            } => {
                let (dx, dy) = if let Some(transform) = &analysis.graph_ref {
                    (dx / transform.raw_per_unit, -dy / transform.raw_per_unit)
                } else {
                    (dx, dy)
                };
                PointIterationFamily::Offset {
                    seed_index,
                    dx,
                    dy,
                    depth,
                    parameter_name,
                }
            }
            RawPointIterationFamily::RotateChain {
                seed_index,
                center_index,
                angle_degrees,
                depth,
            } => PointIterationFamily::RotateChain {
                seed_index,
                center_index,
                angle_degrees,
                depth,
            },
            RawPointIterationFamily::Rotate {
                source_index,
                center_index,
                angle_expr,
                depth,
                parameter_name,
            } => PointIterationFamily::Rotate {
                source_index,
                center_index,
                angle_expr,
                depth,
                parameter_name,
            },
        })
        .collect::<Vec<_>>();

    WorldData {
        world_points,
        world_point_positions,
        point_iterations,
    }
}

fn point_accepts_trace_parameter(point: &ScenePoint) -> bool {
    if matches!(point.binding, Some(ScenePointBinding::Midpoint { .. })) {
        return false;
    }
    matches!(
        point.constraint,
        ScenePointConstraint::OnSegment { .. }
            | ScenePointConstraint::OnPolygonBoundary { .. }
            | ScenePointConstraint::OnCircle { .. }
            | ScenePointConstraint::OnCircleArc { .. }
            | ScenePointConstraint::OnArc { .. }
    )
}

fn apply_trace_parameter(point: &mut ScenePoint, value: f64) {
    let clamped = value.clamp(0.0, 1.0);
    match &mut point.constraint {
        ScenePointConstraint::OnSegment { t, .. } => {
            *t = clamped;
        }
        ScenePointConstraint::OnPolygonBoundary {
            vertex_indices,
            edge_index,
            t,
        } => {
            if vertex_indices.len() < 2 {
                return;
            }
            let scaled = clamped * vertex_indices.len() as f64;
            let next_edge = scaled.floor() as usize;
            *edge_index = next_edge.min(vertex_indices.len() - 1);
            *t = scaled.fract();
        }
        ScenePointConstraint::OnCircle { unit_x, unit_y, .. } => {
            let angle = std::f64::consts::TAU * clamped;
            *unit_x = angle.cos();
            *unit_y = -angle.sin();
        }
        ScenePointConstraint::OnCircleArc { t, .. } => {
            *t = clamped;
        }
        ScenePointConstraint::OnArc { t, .. } => {
            *t = clamped;
        }
        _ => {}
    }
}

fn resolve_trace_point(
    points: &[ScenePoint],
    index: usize,
    visiting: &mut BTreeSet<usize>,
) -> Option<PointRecord> {
    if !visiting.insert(index) {
        return None;
    }

    let point = points.get(index)?;
    let resolved = match &point.binding {
        Some(ScenePointBinding::Translate {
            source_index,
            vector_start_index,
            vector_end_index,
        }) => {
            let source = resolve_trace_point(points, *source_index, visiting)?;
            let vector_start = resolve_trace_point(points, *vector_start_index, visiting)?;
            let vector_end = resolve_trace_point(points, *vector_end_index, visiting)?;
            Some(PointRecord {
                x: source.x + (vector_end.x - vector_start.x),
                y: source.y + (vector_end.y - vector_start.y),
            })
        }
        Some(ScenePointBinding::Reflect {
            source_index,
            line_start_index,
            line_end_index,
        }) => {
            let source = resolve_trace_point(points, *source_index, visiting)?;
            let line_start = resolve_trace_point(points, *line_start_index, visiting)?;
            let line_end = resolve_trace_point(points, *line_end_index, visiting)?;
            reflect_across_line(&source, &line_start, &line_end)
        }
        Some(ScenePointBinding::Rotate {
            source_index,
            center_index,
            angle_degrees,
            ..
        }) => {
            let source = resolve_trace_point(points, *source_index, visiting)?;
            let center = resolve_trace_point(points, *center_index, visiting)?;
            Some(rotate_around(&source, &center, angle_degrees.to_radians()))
        }
        Some(ScenePointBinding::Scale {
            source_index,
            center_index,
            factor,
        }) => {
            let source = resolve_trace_point(points, *source_index, visiting)?;
            let center = resolve_trace_point(points, *center_index, visiting)?;
            Some(scale_around(&source, &center, *factor))
        }
        Some(ScenePointBinding::Midpoint {
            start_index,
            end_index,
        }) => {
            let start = resolve_trace_point(points, *start_index, visiting)?;
            let end = resolve_trace_point(points, *end_index, visiting)?;
            Some(lerp_point(&start, &end, 0.5))
        }
        _ => match &point.constraint {
            ScenePointConstraint::Free => Some(point.position.clone()),
            ScenePointConstraint::Offset {
                origin_index,
                dx,
                dy,
            } => {
                let origin = resolve_trace_point(points, *origin_index, visiting)?;
                Some(PointRecord {
                    x: origin.x + dx,
                    y: origin.y + dy,
                })
            }
            ScenePointConstraint::OnSegment {
                start_index,
                end_index,
                t,
            } => {
                let start = resolve_trace_point(points, *start_index, visiting)?;
                let end = resolve_trace_point(points, *end_index, visiting)?;
                Some(lerp_point(&start, &end, *t))
            }
            ScenePointConstraint::OnPolyline {
                points,
                segment_index,
                t,
                ..
            } => {
                if points.len() < 2 {
                    None
                } else {
                    let start = &points[(*segment_index).min(points.len() - 2)];
                    let end = &points[(*segment_index).min(points.len() - 2) + 1];
                    Some(lerp_point(start, end, *t))
                }
            }
            ScenePointConstraint::OnPolygonBoundary {
                vertex_indices,
                edge_index,
                t,
            } => {
                if vertex_indices.len() < 2 {
                    None
                } else {
                    let start = resolve_trace_point(
                        points,
                        vertex_indices[*edge_index % vertex_indices.len()],
                        visiting,
                    )?;
                    let end = resolve_trace_point(
                        points,
                        vertex_indices[(*edge_index + 1) % vertex_indices.len()],
                        visiting,
                    )?;
                    Some(lerp_point(&start, &end, *t))
                }
            }
            ScenePointConstraint::OnCircle {
                center_index,
                radius_index,
                unit_x,
                unit_y,
            } => {
                let center = resolve_trace_point(points, *center_index, visiting)?;
                let radius_point = resolve_trace_point(points, *radius_index, visiting)?;
                let radius = ((radius_point.x - center.x).powi(2)
                    + (radius_point.y - center.y).powi(2))
                .sqrt();
                Some(PointRecord {
                    x: center.x + radius * unit_x,
                    y: center.y + radius * unit_y,
                })
            }
            ScenePointConstraint::OnCircleArc {
                center_index,
                start_index,
                end_index,
                t,
            } => {
                let center = resolve_trace_point(points, *center_index, visiting)?;
                let start = resolve_trace_point(points, *start_index, visiting)?;
                let end = resolve_trace_point(points, *end_index, visiting)?;
                point_on_circle_arc(&center, &start, &end, *t)
            }
            ScenePointConstraint::OnArc {
                start_index,
                mid_index,
                end_index,
                t,
            } => {
                let start = resolve_trace_point(points, *start_index, visiting)?;
                let mid = resolve_trace_point(points, *mid_index, visiting)?;
                let end = resolve_trace_point(points, *end_index, visiting)?;
                point_on_three_point_arc(&start, &mid, &end, *t)
            }
            ScenePointConstraint::LineIntersection {
                left_kind,
                left_start_index,
                left_end_index,
                right_kind,
                right_start_index,
                right_end_index,
            } => {
                let left_start = resolve_trace_point(points, *left_start_index, visiting)?;
                let left_end = resolve_trace_point(points, *left_end_index, visiting)?;
                let right_start = resolve_trace_point(points, *right_start_index, visiting)?;
                let right_end = resolve_trace_point(points, *right_end_index, visiting)?;
                trace_line_line_intersection(
                    &left_start,
                    &left_end,
                    *left_kind,
                    &right_start,
                    &right_end,
                    *right_kind,
                )
            }
            ScenePointConstraint::LineCircleIntersection {
                line_kind,
                line_start_index,
                line_end_index,
                center_index,
                radius_index,
                variant,
            } => {
                let line_start = resolve_trace_point(points, *line_start_index, visiting)?;
                let line_end = resolve_trace_point(points, *line_end_index, visiting)?;
                let center = resolve_trace_point(points, *center_index, visiting)?;
                let radius_point = resolve_trace_point(points, *radius_index, visiting)?;
                trace_line_circle_intersection(
                    &line_start,
                    &line_end,
                    *line_kind,
                    &center,
                    &radius_point,
                    *variant,
                )
            }
            ScenePointConstraint::CircleCircleIntersection {
                left_center_index,
                left_radius_index,
                right_center_index,
                right_radius_index,
                variant,
            } => {
                let left_center = resolve_trace_point(points, *left_center_index, visiting)?;
                let left_radius = resolve_trace_point(points, *left_radius_index, visiting)?;
                let right_center = resolve_trace_point(points, *right_center_index, visiting)?;
                let right_radius = resolve_trace_point(points, *right_radius_index, visiting)?;
                trace_circle_circle_intersection(
                    &left_center,
                    &left_radius,
                    &right_center,
                    &right_radius,
                    *variant,
                )
            }
        },
    };

    visiting.remove(&index);
    resolved
}

fn trace_line_line_intersection(
    left_start: &PointRecord,
    left_end: &PointRecord,
    left_kind: crate::runtime::scene::LineLikeKind,
    right_start: &PointRecord,
    right_end: &PointRecord,
    right_kind: crate::runtime::scene::LineLikeKind,
) -> Option<PointRecord> {
    let left_dx = left_end.x - left_start.x;
    let left_dy = left_end.y - left_start.y;
    let right_dx = right_end.x - right_start.x;
    let right_dy = right_end.y - right_start.y;
    let determinant = left_dx * right_dy - left_dy * right_dx;
    if determinant.abs() <= 1e-9 {
        return None;
    }
    let delta_x = right_start.x - left_start.x;
    let delta_y = right_start.y - left_start.y;
    let t = (delta_x * right_dy - delta_y * right_dx) / determinant;
    let point = PointRecord {
        x: left_start.x + t * left_dx,
        y: left_start.y + t * left_dy,
    };
    (trace_line_like_contains(left_start, left_end, left_kind, &point)
        && trace_line_like_contains(right_start, right_end, right_kind, &point))
    .then_some(point)
}

fn trace_line_circle_intersection(
    line_start: &PointRecord,
    line_end: &PointRecord,
    line_kind: crate::runtime::scene::LineLikeKind,
    center: &PointRecord,
    radius_point: &PointRecord,
    variant: usize,
) -> Option<PointRecord> {
    let dx = line_end.x - line_start.x;
    let dy = line_end.y - line_start.y;
    let a = dx * dx + dy * dy;
    if a <= 1e-9 {
        return None;
    }
    let radius = ((radius_point.x - center.x).powi(2) + (radius_point.y - center.y).powi(2)).sqrt();
    if radius <= 1e-9 {
        return None;
    }
    let fx = line_start.x - center.x;
    let fy = line_start.y - center.y;
    let b = 2.0 * (fx * dx + fy * dy);
    let c = fx * fx + fy * fy - radius * radius;
    let discriminant = b * b - 4.0 * a * c;
    if discriminant < -1e-9 {
        return None;
    }
    let root = discriminant.max(0.0).sqrt();
    let mut ts = [(-b - root) / (2.0 * a), (-b + root) / (2.0 * a)]
        .into_iter()
        .filter(|t| trace_param_in_line_like(*t, line_kind))
        .collect::<Vec<_>>();
    if ts.is_empty() {
        return None;
    }
    ts.sort_by(|left, right| left.total_cmp(right));
    let t = ts[variant.min(ts.len() - 1)];
    Some(PointRecord {
        x: line_start.x + dx * t,
        y: line_start.y + dy * t,
    })
}

fn trace_line_like_contains(
    start: &PointRecord,
    end: &PointRecord,
    kind: crate::runtime::scene::LineLikeKind,
    point: &PointRecord,
) -> bool {
    let dx = end.x - start.x;
    let dy = end.y - start.y;
    let len_sq = dx * dx + dy * dy;
    if len_sq <= 1e-9 {
        return false;
    }
    let t = ((point.x - start.x) * dx + (point.y - start.y) * dy) / len_sq;
    trace_param_in_line_like(t, kind)
}

fn trace_param_in_line_like(t: f64, kind: crate::runtime::scene::LineLikeKind) -> bool {
    match kind {
        crate::runtime::scene::LineLikeKind::Line => true,
        crate::runtime::scene::LineLikeKind::Ray => t >= -1e-9,
        crate::runtime::scene::LineLikeKind::Segment => (-1e-9..=1.0 + 1e-9).contains(&t),
    }
}

fn trace_circle_circle_intersection(
    left_center: &PointRecord,
    left_radius_point: &PointRecord,
    right_center: &PointRecord,
    right_radius_point: &PointRecord,
    variant: usize,
) -> Option<PointRecord> {
    let left_radius = ((left_radius_point.x - left_center.x).powi(2)
        + (left_radius_point.y - left_center.y).powi(2))
    .sqrt();
    let right_radius = ((right_radius_point.x - right_center.x).powi(2)
        + (right_radius_point.y - right_center.y).powi(2))
    .sqrt();
    if left_radius <= 1e-9 || right_radius <= 1e-9 {
        return None;
    }
    let dx = right_center.x - left_center.x;
    let dy = right_center.y - left_center.y;
    let distance = (dx * dx + dy * dy).sqrt();
    if distance <= 1e-9
        || distance > left_radius + right_radius + 1e-9
        || distance < (left_radius - right_radius).abs() - 1e-9
    {
        return None;
    }
    let along = (left_radius * left_radius - right_radius * right_radius + distance * distance)
        / (2.0 * distance);
    let height_sq = left_radius * left_radius - along * along;
    if height_sq < -1e-9 {
        return None;
    }
    let height = height_sq.max(0.0).sqrt();
    let ux = dx / distance;
    let uy = dy / distance;
    let base = PointRecord {
        x: left_center.x + along * ux,
        y: left_center.y + along * uy,
    };
    let mut ordered = [
        PointRecord {
            x: base.x - height * uy,
            y: base.y + height * ux,
        },
        PointRecord {
            x: base.x + height * uy,
            y: base.y - height * ux,
        },
    ];
    ordered.sort_by(|left, right| {
        left.y
            .total_cmp(&right.y)
            .then_with(|| left.x.total_cmp(&right.x))
    });
    Some(ordered[variant.min(1)].clone())
}

fn collect_point_traces(
    file: &GspFile,
    groups: &[ObjectGroup],
    visible_points: &[ScenePoint],
    group_to_point_index: &[Option<usize>],
) -> Vec<LineShape> {
    groups
        .iter()
        .filter(|group| (group.header.kind()) == crate::format::GroupKind::PointTrace)
        .filter_map(|group| {
            let path = find_indexed_path(file, group)?;
            let target_group_index = path.refs.first()?.checked_sub(1)?;
            let target_point_index = (*group_to_point_index.get(target_group_index)?)?;
            let driver_point_index = path.refs.iter().find_map(|ordinal| {
                let group_index = ordinal.checked_sub(1)?;
                let point_index = (*group_to_point_index.get(group_index)?)?;
                let point = visible_points.get(point_index)?;
                point_accepts_trace_parameter(point).then_some(point_index)
            })?;
            let payload = group
                .records
                .iter()
                .find(|record| record.record_type == 0x0902)
                .map(|record| record.payload(&file.data))?;
            let descriptor = decode_function_plot_descriptor(payload)?;

            let mut points = Vec::with_capacity(descriptor.sample_count);
            let last = descriptor.sample_count.saturating_sub(1).max(1) as f64;
            for sample_index in 0..descriptor.sample_count {
                let t = sample_index as f64 / last;
                let parameter = descriptor.x_min + (descriptor.x_max - descriptor.x_min) * t;
                let mut sampled_points = visible_points.to_vec();
                let driver_point = sampled_points.get_mut(driver_point_index)?;
                apply_trace_parameter(driver_point, parameter);
                points.push(resolve_trace_point(
                    &sampled_points,
                    target_point_index,
                    &mut BTreeSet::new(),
                )?);
            }

            (points.len() >= 2).then_some(LineShape {
                points,
                color: crate::runtime::geometry::color_from_style(group.header.style_b),
                dashed: false,
                binding: None,
            })
        })
        .collect()
}

fn compute_scene_bounds(
    analysis: &SceneAnalysis,
    shapes: &CollectedShapes,
    labels: &[TextLabel],
    world_point_positions: &[PointRecord],
) -> BoundsData {
    let bounds_lines = shapes
        .rotational_iteration_lines
        .iter()
        .chain(shapes.polylines.iter())
        .chain(shapes.direct_lines.iter())
        .chain(shapes.rays.iter())
        .chain(shapes.translated_lines.iter())
        .chain(shapes.segment_markers.iter())
        .chain(shapes.rotated_lines.iter())
        .chain(shapes.scaled_lines.iter())
        .chain(shapes.reflected_lines.iter())
        .chain(shapes.derived_segments.iter())
        .chain(shapes.measurements.iter())
        .chain(shapes.coordinate_traces.iter())
        .chain(shapes.axes.iter())
        .chain(shapes.iteration_lines.iter())
        .chain(shapes.carried_iteration_lines.iter())
        .cloned()
        .collect::<Vec<_>>();
    let bounds_polygons = shapes
        .polygons
        .iter()
        .chain(shapes.translated_polygons.iter())
        .chain(shapes.rotated_polygons.iter())
        .chain(shapes.transformed_polygons.iter())
        .chain(shapes.reflected_polygons.iter())
        .chain(shapes.iteration_polygons.iter())
        .chain(shapes.carried_iteration_polygons.iter())
        .cloned()
        .collect::<Vec<_>>();
    let bounds_circles = shapes
        .circles
        .iter()
        .chain(shapes.rotated_circles.iter())
        .chain(shapes.transformed_circles.iter())
        .chain(shapes.reflected_circles.iter())
        .cloned()
        .collect::<Vec<_>>();
    let bounds_arcs = shapes.arcs.clone();

    let mut bounds = collect_bounds(
        &analysis.graph_ref,
        &bounds_lines,
        &[],
        &[],
        &bounds_polygons,
        &bounds_circles,
        &bounds_arcs,
        labels,
        world_point_positions,
    );
    include_line_bounds(&mut bounds, &analysis.function_plots, &analysis.graph_ref);
    include_line_bounds(&mut bounds, &shapes.synthetic_axes, &analysis.graph_ref);
    let use_saved_viewport = analysis
        .saved_viewport
        .filter(|viewport| bounds_within(viewport, &bounds))
        .is_some();
    if let Some(viewport) = analysis.saved_viewport.filter(|_| use_saved_viewport) {
        bounds = viewport;
    } else {
        if let Some((domain_min_x, domain_max_x)) = analysis.function_plot_domain {
            bounds.min_x = bounds.min_x.min(domain_min_x);
            bounds.max_x = bounds.max_x.max(domain_max_x);
            bounds.min_y = bounds.min_y.min(0.0);
            bounds.max_y = bounds.max_y.max(0.0);
        }
        expand_bounds(&mut bounds);
    }

    BoundsData {
        bounds,
        use_saved_viewport,
    }
}

fn assemble_scene(
    analysis: SceneAnalysis,
    shapes: CollectedShapes,
    labels: Vec<TextLabel>,
    world_data: WorldData,
    bounds_data: BoundsData,
    line_iterations: Vec<LineIterationFamily>,
    polygon_iterations: Vec<PolygonIterationFamily>,
    label_iterations: Vec<LabelIterationFamily>,
    buttons: Vec<super::scene::SceneButton>,
    parameters: Vec<super::scene::SceneParameter>,
    functions: Vec<super::scene::SceneFunction>,
) -> Scene {
    let CollectedShapes {
        polylines,
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
        measurements,
        coordinate_traces,
        axes,
        iteration_polygon_indices: _,
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
    } = shapes;

    let raw_polygons = polygons
        .into_iter()
        .chain(translated_polygons)
        .chain(rotated_polygons)
        .chain(transformed_polygons)
        .chain(reflected_polygons)
        .chain(iteration_polygons)
        .chain(carried_iteration_polygons)
        .collect::<Vec<_>>();

    let raw_lines = suppress_polygon_edge_segments(
        dedupe_line_shapes(
            rotational_iteration_lines
                .into_iter()
                .chain(polylines)
                .chain(direct_lines)
                .chain(rays)
                .chain(translated_lines)
                .chain(segment_markers)
                .chain(rotated_lines)
                .chain(scaled_lines)
                .chain(reflected_lines)
                .chain(derived_segments)
                .chain(measurements)
                .chain(coordinate_traces)
                .chain(axes)
                .chain(analysis.function_plots)
                .chain(synthetic_axes)
                .chain(iteration_lines)
                .chain(carried_iteration_lines)
                .collect(),
        ),
        &raw_polygons,
    );

    Scene {
        graph_mode: analysis.graph_mode,
        pi_mode: analysis.pi_mode,
        saved_viewport: bounds_data.use_saved_viewport,
        y_up: analysis.graph_mode,
        origin: analysis
            .graph_ref
            .as_ref()
            .map(|transform| to_world(&transform.origin_raw, &analysis.graph_ref)),
        bounds: bounds_data.bounds,
        lines: raw_lines
            .into_iter()
            .map(|line| world_line_shape(line, &analysis.graph_ref, &bounds_data.bounds))
            .collect(),
        polygons: raw_polygons
            .into_iter()
            .map(|polygon| PolygonShape {
                points: polygon
                    .points
                    .into_iter()
                    .map(|point| to_world(&point, &analysis.graph_ref))
                    .collect(),
                color: polygon.color,
                binding: polygon.binding,
            })
            .collect(),
        circles: circles
            .into_iter()
            .chain(rotated_circles)
            .chain(transformed_circles)
            .chain(reflected_circles)
            .map(|circle| SceneCircle {
                center: to_world(&circle.center, &analysis.graph_ref),
                radius_point: to_world(&circle.radius_point, &analysis.graph_ref),
                color: circle.color,
                dashed: circle.dashed,
                binding: circle.binding,
            })
            .collect(),
        arcs: arcs
            .into_iter()
            .map(|arc| SceneArc {
                points: arc
                    .points
                    .map(|point| to_world(&point, &analysis.graph_ref)),
                color: arc.color,
                center: arc
                    .center
                    .map(|center| to_world(&center, &analysis.graph_ref)),
                counterclockwise: arc.counterclockwise,
            })
            .collect(),
        labels: labels
            .into_iter()
            .map(|label| TextLabel {
                anchor: if label.screen_space {
                    label.anchor
                } else {
                    to_world(&label.anchor, &analysis.graph_ref)
                },
                text: label.text,
                color: label.color,
                binding: label.binding,
                screen_space: label.screen_space,
                hotspots: label.hotspots,
            })
            .collect(),
        points: world_data.world_points,
        point_iterations: world_data.point_iterations,
        line_iterations: line_iterations
            .into_iter()
            .map(|family| world_line_iteration_family(family, &analysis.graph_ref))
            .collect(),
        polygon_iterations: polygon_iterations
            .into_iter()
            .map(|family| world_polygon_iteration_family(family, &analysis.graph_ref))
            .collect(),
        label_iterations,
        buttons,
        parameters,
        functions,
    }
}
fn suppress_polygon_edge_segments(
    lines: Vec<LineShape>,
    polygons: &[PolygonShape],
) -> Vec<LineShape> {
    lines
        .into_iter()
        .filter(|line| {
            matches!(line.binding, Some(LineBinding::Segment { .. }))
                .then(|| {
                    !polygons
                        .iter()
                        .any(|polygon| polygon_has_matching_edge(polygon, line))
                })
                .unwrap_or(true)
        })
        .collect()
}

fn polygon_has_matching_edge(polygon: &PolygonShape, line: &LineShape) -> bool {
    if polygon.points.len() < 3 || line.points.len() != 2 {
        return false;
    }
    polygon
        .points
        .iter()
        .zip(polygon.points.iter().cycle().skip(1))
        .take(polygon.points.len())
        .any(|(start, end)| points_match_segment(start, end, &line.points[0], &line.points[1]))
}

fn points_match_segment(
    left_start: &crate::format::PointRecord,
    left_end: &crate::format::PointRecord,
    right_start: &crate::format::PointRecord,
    right_end: &crate::format::PointRecord,
) -> bool {
    points_equal(left_start, right_start) && points_equal(left_end, right_end)
        || points_equal(left_start, right_end) && points_equal(left_end, right_start)
}

fn points_equal(left: &crate::format::PointRecord, right: &crate::format::PointRecord) -> bool {
    (left.x - right.x).abs() < 1e-6 && (left.y - right.y).abs() < 1e-6
}

pub(crate) fn build_scene(file: &GspFile) -> Scene {
    let groups = file.object_groups();
    let point_map = collect_point_objects(file, &groups);
    let analysis = analyze_scene(file, &groups, &point_map);
    let mut shapes = collect_scene_shapes(file, &groups, &point_map, &analysis);
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
    assemble_scene(
        analysis,
        shapes,
        labels,
        world_data,
        bounds_data,
        line_iterations,
        polygon_iterations,
        label_iterations,
        buttons,
        parameters,
        functions,
    )
}
