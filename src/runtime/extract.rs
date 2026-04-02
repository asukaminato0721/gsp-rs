use std::collections::BTreeSet;

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
use crate::format::{
    GspFile, IndexedPathRecord, ObjectGroup, PointRecord, collect_strings, decode_indexed_path,
    decode_point_record, read_f64, read_i16, read_u16, read_u32,
};

use self::graph::{
    bounds_within, collect_bounds, collect_saved_viewport, dedupe_line_shapes,
    detect_graph_transform, expand_bounds, has_graph_classes,
};
use self::labels::{
    collect_circle_parameter_labels, collect_coordinate_labels, collect_label_iterations,
    collect_labels, collect_polygon_parameter_labels, collect_segment_parameter_labels,
    compute_iteration_labels,
};
use self::points::{
    RawPointConstraint, RawPointIterationFamily, TransformBindingKind,
    collect_non_graph_parameters, collect_point_iteration_points, collect_point_objects,
    collect_visible_points, decode_offset_anchor_raw, decode_parameter_controlled_anchor_raw,
    decode_parameter_rotation_anchor_raw, decode_parameter_rotation_binding,
    decode_point_constraint, decode_point_constraint_anchor, decode_point_on_ray_anchor_raw,
    decode_point_pair_translation_anchor_raw, decode_reflection_anchor_raw,
    decode_regular_polygon_vertex_anchor_raw, decode_transform_binding,
    decode_translated_point_anchor_raw, reflection_line_group_indices, regular_polygon_angle_expr,
    regular_polygon_iteration_step, remap_circle_bindings, remap_label_bindings,
    remap_line_bindings, remap_polygon_bindings, translation_point_pair_group_indices,
};
use self::shapes::{
    collect_bound_line_shapes, collect_carried_iteration_lines, collect_carried_iteration_polygons,
    collect_carried_line_iteration_families, collect_carried_polygon_iteration_families,
    collect_circle_shapes, collect_coordinate_traces, collect_derived_segments,
    collect_iteration_shapes, collect_line_shapes, collect_polygon_shapes,
    collect_raw_object_anchors, collect_reflected_circle_shapes, collect_reflected_line_shapes,
    collect_reflected_polygon_shapes, collect_rotated_circle_shapes, collect_rotated_line_shapes,
    collect_rotated_polygon_shapes, collect_rotational_iteration_lines, collect_scaled_line_shapes,
    collect_transformed_circle_shapes, collect_transformed_polygon_shapes,
    collect_translated_polygon_shapes,
};

use self::world::{world_line_iteration_family, world_line_shape, world_polygon_iteration_family};
use super::functions::{
    collect_function_plot_domain, collect_function_plots, collect_scene_functions,
    collect_scene_parameters, function_uses_pi_scale, synthesize_function_axes,
    synthesize_function_labels,
};
use super::geometry::{GraphTransform, distance_world, include_line_bounds, to_world};
use super::scene::{
    LabelIterationFamily, LineShape, PointIterationFamily, PolygonShape, Scene, SceneCircle,
    SceneParameter, ScenePoint, ScenePointBinding, ScenePointConstraint, TextLabel,
    TextLabelBinding,
};

pub(crate) use self::decode::find_indexed_path;

#[derive(Debug, Clone)]
struct CircleShape {
    center: PointRecord,
    radius_point: PointRecord,
    color: [u8; 4],
    binding: Option<super::scene::ShapeBinding>,
}

pub(crate) fn build_scene(file: &GspFile) -> Scene {
    let groups = file.object_groups();
    let point_map = collect_point_objects(file, &groups);
    let raw_anchors_for_graph = collect_raw_object_anchors(file, &groups, &point_map, None);
    let graph = detect_graph_transform(file, &groups, &raw_anchors_for_graph);
    let graph_mode = graph.is_some() && has_graph_classes(&groups);
    let graph_ref = if graph_mode { graph.clone() } else { None };
    let raw_anchors = collect_raw_object_anchors(file, &groups, &point_map, graph_ref.as_ref());
    let saved_viewport = if graph_mode {
        collect_saved_viewport(file, &groups)
    } else {
        None
    };
    let pi_mode = if graph_mode {
        saved_viewport.is_some() || function_uses_pi_scale(file, &groups)
    } else {
        false
    };
    let function_plot_domain = if graph_mode {
        collect_function_plot_domain(file, &groups)
    } else {
        None
    };
    let function_plots = if graph_mode {
        collect_function_plots(file, &groups, &graph_ref)
    } else {
        Vec::new()
    };
    let has_function_plots = !function_plots.is_empty();
    let has_coordinate_objects = groups
        .iter()
        .any(|group| matches!(group.header.class_id & 0xffff, 69 | 97));
    let has_iteration_helpers = groups
        .iter()
        .any(|group| matches!(group.header.class_id & 0xffff, 76 | 77 | 89));
    let large_non_graph = !graph_mode && file.records.len() > 10_000;

    let polylines = collect_line_shapes(
        file,
        &groups,
        &raw_anchors,
        &[2],
        !graph_mode && !large_non_graph,
    );
    let mut direct_lines = collect_bound_line_shapes(file, &groups, &raw_anchors, 63);
    let mut rays = collect_bound_line_shapes(file, &groups, &raw_anchors, 64);
    let derived_segments = if large_non_graph {
        collect_derived_segments(file, &groups, &point_map, &[24])
    } else {
        Vec::new()
    };
    let mut rotated_lines = collect_rotated_line_shapes(file, &groups, &raw_anchors);
    let mut scaled_lines = collect_scaled_line_shapes(file, &groups, &raw_anchors);
    let mut reflected_lines = collect_reflected_line_shapes(file, &groups, &raw_anchors);
    let mut rotational_iteration_lines =
        collect_rotational_iteration_lines(file, &groups, &raw_anchors);
    let carried_iteration_lines = collect_carried_iteration_lines(file, &groups, &raw_anchors);
    let carried_iteration_polygons =
        collect_carried_iteration_polygons(file, &groups, &raw_anchors);
    let measurements = if graph_mode {
        collect_line_shapes(file, &groups, &raw_anchors, &[58], false)
    } else {
        Vec::new()
    };
    let coordinate_traces = if graph_mode {
        collect_coordinate_traces(file, &groups, &graph_ref)
    } else {
        Vec::new()
    };
    let axes = if graph_mode {
        collect_line_shapes(file, &groups, &raw_anchors, &[61], false)
    } else {
        Vec::new()
    };
    let iteration_polygon_indices: BTreeSet<usize> = groups
        .iter()
        .filter(|group| (group.header.class_id & 0xffff) == 89)
        .filter_map(|group| find_indexed_path(file, group))
        .flat_map(|path| path.refs)
        .filter_map(|obj_ref| {
            let index = obj_ref.checked_sub(1)?;
            let group = groups.get(index)?;
            ((group.header.class_id & 0xffff) == 8).then_some(index)
        })
        .collect();
    let polygons = collect_polygon_shapes(file, &groups, &raw_anchors, &[8])
        .into_iter()
        .enumerate()
        .filter_map(|(ordinal, polygon)| {
            let group_index = groups
                .iter()
                .enumerate()
                .filter(|(_, group)| (group.header.class_id & 0xffff) == 8)
                .nth(ordinal)
                .map(|(index, _)| index)?;
            (!iteration_polygon_indices.contains(&group_index)).then_some(polygon)
        })
        .collect::<Vec<_>>();
    let circles = collect_circle_shapes(file, &groups, &raw_anchors);
    let mut rotated_circles = collect_rotated_circle_shapes(file, &groups, &raw_anchors);
    let mut transformed_circles = collect_transformed_circle_shapes(file, &groups, &raw_anchors);
    let mut reflected_circles = collect_reflected_circle_shapes(file, &groups, &raw_anchors);
    let mut translated_polygons = collect_translated_polygon_shapes(file, &groups, &raw_anchors);
    let mut rotated_polygons = collect_rotated_polygon_shapes(file, &groups, &raw_anchors);
    let mut transformed_polygons = collect_transformed_polygon_shapes(file, &groups, &raw_anchors);
    let mut reflected_polygons = collect_reflected_polygon_shapes(file, &groups, &raw_anchors);
    let (iteration_lines, iteration_polygons) = collect_iteration_shapes(file, &groups, &circles);
    let synthetic_axes = if graph_mode && has_function_plots && axes.is_empty() {
        synthesize_function_axes(
            &function_plots,
            function_plot_domain,
            saved_viewport,
            &graph_ref,
        )
    } else {
        Vec::new()
    };
    let (mut labels, label_group_to_index) = collect_labels(
        file,
        &groups,
        &raw_anchors,
        graph_mode,
        !has_function_plots && !has_coordinate_objects,
    );
    if has_coordinate_objects || has_iteration_helpers {
        labels.extend(collect_coordinate_labels(file, &groups));
    }
    labels.extend(collect_polygon_parameter_labels(
        file,
        &groups,
        &raw_anchors,
    ));
    labels.extend(collect_segment_parameter_labels(file, &groups));
    labels.extend(collect_circle_parameter_labels(file, &groups, &raw_anchors));
    labels.extend(compute_iteration_labels(file, &groups, &circles));
    if graph_mode && has_function_plots {
        labels.extend(synthesize_function_labels(
            file,
            &groups,
            &function_plots,
            saved_viewport,
            &graph_ref,
        ));
    }

    if graph_mode
        && let (Some(circle), Some(formula_index), Some(transform)) = (
            circles.first(),
            labels.iter().position(|label| label.text.contains("AB:")),
            graph_ref.as_ref(),
        )
    {
        let circumference = 2.0
            * std::f64::consts::PI
            * distance_world(&circle.center, &circle.radius_point, &graph_ref);
        let anchor = PointRecord {
            x: labels[formula_index].anchor.x,
            y: labels[formula_index].anchor.y - 0.9 * transform.raw_per_unit,
        };
        labels.insert(
            formula_index,
            TextLabel {
                anchor,
                text: format!("AB perimeter = {:.2} cm", circumference),
                color: [30, 30, 30, 255],
                binding: None,
                screen_space: false,
            },
        );
    }

    let (visible_points, group_to_point_index) =
        collect_visible_points(file, &groups, &point_map, &raw_anchors, &graph_ref);
    let (derived_iteration_points, raw_point_iterations) =
        collect_point_iteration_points(file, &groups, &raw_anchors, &group_to_point_index);
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
    let circle_group_to_index = groups
        .iter()
        .enumerate()
        .filter(|(_, group)| (group.header.class_id & 0xffff) == 3)
        .enumerate()
        .fold(
            vec![None; groups.len()],
            |mut acc, (shape_index, (group_index, _))| {
                acc[group_index] = Some(shape_index);
                acc
            },
        );
    let polygon_group_to_index = groups
        .iter()
        .enumerate()
        .filter(|(index, group)| {
            (group.header.class_id & 0xffff) == 8 && !iteration_polygon_indices.contains(index)
        })
        .enumerate()
        .fold(
            vec![None; groups.len()],
            |mut acc, (shape_index, (group_index, _))| {
                acc[group_index] = Some(shape_index);
                acc
            },
        );
    remap_circle_bindings(
        &mut rotated_circles,
        &group_to_point_index,
        &circle_group_to_index,
    );
    remap_circle_bindings(
        &mut transformed_circles,
        &group_to_point_index,
        &circle_group_to_index,
    );
    remap_circle_bindings(
        &mut reflected_circles,
        &group_to_point_index,
        &circle_group_to_index,
    );
    remap_polygon_bindings(
        &mut translated_polygons,
        &group_to_point_index,
        &polygon_group_to_index,
    );
    remap_polygon_bindings(
        &mut rotated_polygons,
        &group_to_point_index,
        &polygon_group_to_index,
    );
    remap_polygon_bindings(
        &mut transformed_polygons,
        &group_to_point_index,
        &polygon_group_to_index,
    );
    remap_polygon_bindings(
        &mut reflected_polygons,
        &group_to_point_index,
        &polygon_group_to_index,
    );
    let line_group_to_index = groups
        .iter()
        .enumerate()
        .filter(|(_, group)| (group.header.class_id & 0xffff) == 2)
        .enumerate()
        .fold(
            vec![None; groups.len()],
            |mut acc, (line_index, (group_index, _))| {
                acc[group_index] = Some(line_index);
                acc
            },
        );
    remap_line_bindings(
        &mut direct_lines,
        &group_to_point_index,
        &line_group_to_index,
    );
    remap_line_bindings(&mut rays, &group_to_point_index, &line_group_to_index);
    remap_line_bindings(
        &mut rotated_lines,
        &group_to_point_index,
        &line_group_to_index,
    );
    remap_line_bindings(
        &mut scaled_lines,
        &group_to_point_index,
        &line_group_to_index,
    );
    remap_line_bindings(
        &mut reflected_lines,
        &group_to_point_index,
        &line_group_to_index,
    );
    remap_line_bindings(
        &mut rotational_iteration_lines,
        &group_to_point_index,
        &line_group_to_index,
    );
    let line_iterations =
        collect_carried_line_iteration_families(file, &groups, &raw_anchors, &group_to_point_index);
    let polygon_iterations = collect_carried_polygon_iteration_families(
        file,
        &groups,
        &raw_anchors,
        &group_to_point_index,
    );

    let world_points = visible_points
        .iter()
        .chain(derived_iteration_points.iter())
        .map(|point| ScenePoint {
            position: to_world(&point.position, &graph_ref),
            constraint: match &point.constraint {
                ScenePointConstraint::Free => ScenePointConstraint::Free,
                ScenePointConstraint::Offset {
                    origin_index,
                    dx,
                    dy,
                } => {
                    let (dx, dy) = if let Some(transform) = &graph_ref {
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
                        .map(|point| to_world(point, &graph_ref))
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
                    unit_y: if graph_ref.is_some() {
                        *unit_y
                    } else {
                        -*unit_y
                    },
                },
            },
            binding: point.binding.clone(),
        })
        .collect::<Vec<_>>();

    let world_point_positions = world_points
        .iter()
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
                let (dx, dy) = if let Some(transform) = &graph_ref {
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

    let bounds_lines = rotational_iteration_lines
        .iter()
        .chain(polylines.iter())
        .chain(direct_lines.iter())
        .chain(rays.iter())
        .chain(rotated_lines.iter())
        .chain(scaled_lines.iter())
        .chain(reflected_lines.iter())
        .chain(derived_segments.iter())
        .chain(measurements.iter())
        .chain(coordinate_traces.iter())
        .chain(axes.iter())
        .chain(iteration_lines.iter())
        .chain(carried_iteration_lines.iter())
        .cloned()
        .collect::<Vec<_>>();
    let bounds_polygons = polygons
        .iter()
        .chain(translated_polygons.iter())
        .chain(rotated_polygons.iter())
        .chain(transformed_polygons.iter())
        .chain(reflected_polygons.iter())
        .chain(iteration_polygons.iter())
        .chain(carried_iteration_polygons.iter())
        .cloned()
        .collect::<Vec<_>>();
    let bounds_circles = circles
        .iter()
        .chain(rotated_circles.iter())
        .chain(transformed_circles.iter())
        .chain(reflected_circles.iter())
        .cloned()
        .collect::<Vec<_>>();

    let mut bounds = collect_bounds(
        &graph_ref,
        &bounds_lines,
        &[],
        &[],
        &bounds_polygons,
        &bounds_circles,
        &labels,
        &world_point_positions,
    );
    include_line_bounds(&mut bounds, &function_plots, &graph_ref);
    include_line_bounds(&mut bounds, &synthetic_axes, &graph_ref);
    let use_saved_viewport = saved_viewport
        .filter(|viewport| bounds_within(viewport, &bounds))
        .is_some();
    if let Some(viewport) = saved_viewport.filter(|_| use_saved_viewport) {
        bounds = viewport;
    } else {
        if let Some((domain_min_x, domain_max_x)) = function_plot_domain {
            bounds.min_x = bounds.min_x.min(domain_min_x);
            bounds.max_x = bounds.max_x.max(domain_max_x);
            bounds.min_y = bounds.min_y.min(0.0);
            bounds.max_y = bounds.max_y.max(0.0);
        }
        expand_bounds(&mut bounds);
    }

    let mut parameters = if graph_mode {
        collect_scene_parameters(file, &groups, &labels)
    } else {
        Vec::new()
    };
    parameters.extend(collect_non_graph_parameters(file, &groups, &mut labels));
    let buttons = collect_buttons(
        file,
        &groups,
        &raw_anchors,
        &group_to_point_index,
        &line_group_to_index,
        &circle_group_to_index,
        &polygon_group_to_index,
    );
    let functions = if graph_mode {
        collect_scene_functions(
            file,
            &groups,
            &labels,
            &world_points,
            polylines.len() + derived_segments.len() + measurements.len() + axes.len(),
        )
    } else {
        Vec::new()
    };

    let raw_lines = dedupe_line_shapes(
        polylines
            .into_iter()
            .chain(direct_lines)
            .chain(rays)
            .chain(rotated_lines)
            .chain(scaled_lines)
            .chain(reflected_lines)
            .chain(derived_segments)
            .chain(measurements)
            .chain(coordinate_traces)
            .chain(axes)
            .chain(function_plots)
            .chain(synthetic_axes)
            .chain(iteration_lines)
            .chain(rotational_iteration_lines)
            .chain(carried_iteration_lines)
            .collect(),
    );

    Scene {
        graph_mode,
        pi_mode,
        saved_viewport: use_saved_viewport,
        y_up: graph_mode,
        origin: graph_ref
            .as_ref()
            .map(|transform| to_world(&transform.origin_raw, &graph_ref)),
        bounds,
        lines: raw_lines
            .into_iter()
            .map(|line| world_line_shape(line, &graph_ref, &bounds))
            .collect(),
        polygons: polygons
            .into_iter()
            .chain(translated_polygons)
            .chain(rotated_polygons)
            .chain(transformed_polygons)
            .chain(reflected_polygons)
            .chain(iteration_polygons)
            .chain(carried_iteration_polygons)
            .map(|polygon| PolygonShape {
                points: polygon
                    .points
                    .into_iter()
                    .map(|point| to_world(&point, &graph_ref))
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
                center: to_world(&circle.center, &graph_ref),
                radius_point: to_world(&circle.radius_point, &graph_ref),
                color: circle.color,
                binding: circle.binding,
            })
            .collect(),
        labels: labels
            .into_iter()
            .map(|label| TextLabel {
                anchor: if label.screen_space {
                    label.anchor
                } else {
                    to_world(&label.anchor, &graph_ref)
                },
                text: label.text,
                color: label.color,
                binding: label.binding,
                screen_space: label.screen_space,
            })
            .collect(),
        points: world_points,
        point_iterations,
        line_iterations: line_iterations
            .into_iter()
            .map(|family| world_line_iteration_family(family, &graph_ref))
            .collect(),
        polygon_iterations: polygon_iterations
            .into_iter()
            .map(|family| world_polygon_iteration_family(family, &graph_ref))
            .collect(),
        label_iterations,
        buttons,
        parameters,
        functions,
    }
}
