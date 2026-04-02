use std::collections::BTreeSet;

mod decode;
mod graph;
mod labels;
mod points;
mod shapes;

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
    collect_bound_line_shapes, collect_circle_shapes, collect_coordinate_traces,
    collect_derived_segments, collect_iteration_shapes, collect_line_shapes,
    collect_polygon_shapes, collect_raw_object_anchors, collect_reflected_circle_shapes,
    collect_reflected_line_shapes, collect_reflected_polygon_shapes, collect_rotated_circle_shapes,
    collect_rotated_line_shapes, collect_rotated_polygon_shapes,
    collect_rotational_iteration_lines, collect_scaled_line_shapes,
    collect_transformed_circle_shapes, collect_transformed_polygon_shapes,
    collect_translated_polygon_shapes,
};

use super::functions::{
    collect_function_plot_domain, collect_function_plots, collect_scene_functions,
    collect_scene_parameters, function_uses_pi_scale, synthesize_function_axes,
    synthesize_function_labels,
};
use super::geometry::{
    Bounds, GraphTransform, clip_line_to_bounds, clip_ray_to_bounds, distance_world,
    include_line_bounds, to_world,
};
use super::scene::{
    ButtonAction, LabelIterationFamily, LineBinding, LineShape, PointIterationFamily, PolygonShape,
    Scene, SceneButton, SceneCircle, SceneParameter, ScenePoint, ScenePointBinding,
    ScenePointConstraint, ScreenPoint, ScreenRect, TextLabel, TextLabelBinding,
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
        .cloned()
        .collect::<Vec<_>>();
    let bounds_polygons = polygons
        .iter()
        .chain(translated_polygons.iter())
        .chain(rotated_polygons.iter())
        .chain(transformed_polygons.iter())
        .chain(reflected_polygons.iter())
        .chain(iteration_polygons.iter())
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
        label_iterations,
        buttons,
        parameters,
        functions,
    }
}

fn world_line_shape(
    line: LineShape,
    graph_ref: &Option<GraphTransform>,
    bounds: &Bounds,
) -> LineShape {
    let mut world_points = line
        .points
        .into_iter()
        .map(|point| to_world(&point, graph_ref))
        .collect::<Vec<_>>();

    if let Some(binding) = &line.binding {
        let clipped = match binding {
            LineBinding::Line { .. } if world_points.len() >= 2 => {
                clip_line_to_bounds(&world_points[0], &world_points[1], bounds)
            }
            LineBinding::Ray { .. } if world_points.len() >= 2 => {
                clip_ray_to_bounds(&world_points[0], &world_points[1], bounds)
            }
            _ => None,
        };
        if let Some([start, end]) = clipped {
            world_points = vec![start, end];
        }
    }

    LineShape {
        points: world_points,
        color: line.color,
        dashed: line.dashed,
        binding: line.binding,
    }
}

fn collect_buttons(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
    group_to_point_index: &[Option<usize>],
    line_group_to_index: &[Option<usize>],
    circle_group_to_index: &[Option<usize>],
    polygon_group_to_index: &[Option<usize>],
) -> Vec<SceneButton> {
    #[derive(Clone)]
    enum RawButtonAction {
        Link {
            href: String,
        },
        ToggleVisibility {
            refs: Vec<usize>,
        },
        SetVisibility {
            refs: Vec<usize>,
            visible: bool,
        },
        MovePoint {
            point_group_ordinal: usize,
            target_group_ordinal: Option<usize>,
        },
        AnimatePoint {
            point_group_ordinal: usize,
        },
        ScrollPoint {
            point_group_ordinal: usize,
        },
        Sequence {
            button_group_ordinals: Vec<usize>,
            interval_ms: u32,
        },
    }

    #[derive(Clone)]
    struct RawButton {
        group_ordinal: usize,
        text: String,
        anchor: ScreenPoint,
        rect: Option<ScreenRect>,
        action: RawButtonAction,
    }

    let button_label_groups = groups
        .iter()
        .filter(|group| (group.header.class_id & 0xffff) == 73)
        .filter_map(|group| {
            let path = find_indexed_path(file, group)?;
            let button_ordinal = *path.refs.first()?;
            let anchor = decode::decode_label_anchor(file, group, anchors)?;
            Some((button_ordinal, anchor))
        })
        .collect::<std::collections::BTreeMap<usize, PointRecord>>();

    let mut raw_buttons = Vec::<RawButton>::new();
    for group in groups {
        let kind = group.header.class_id & 0xffff;
        if kind == 0
            && !group
                .records
                .iter()
                .any(|record| matches!(record.record_type, 0x0899 | 0x0907))
            && let Some(href) = decode::decode_link_button_url(file, group)
            && let Some((x, y, width, height)) = decode::decode_bbox_rect_raw(file, group)
        {
            raw_buttons.push(RawButton {
                group_ordinal: group.ordinal,
                text: decode::decode_label_name_raw(file, group)
                    .filter(|label| !label.trim().is_empty())
                    .unwrap_or_else(|| href.clone()),
                anchor: ScreenPoint { x, y },
                rect: Some(ScreenRect { width, height }),
                action: RawButtonAction::Link { href },
            });
            continue;
        }

        if !decode::is_action_button_group(group) {
            continue;
        }

        let payload = group
            .records
            .iter()
            .find(|record| record.record_type == 0x0906)
            .map(|record| record.payload(&file.data));
        let action_payload = if let Some(payload) = payload {
            payload
        } else {
            continue;
        };
        if action_payload.len() < 16 {
            continue;
        }

        let refs = find_indexed_path(file, group)
            .map(|path| path.refs)
            .unwrap_or_default();
        let action_kind_lo = read_u16(action_payload, 12);
        let action_kind_hi = read_u16(action_payload, 14);
        let action = match (action_kind_lo, action_kind_hi) {
            (2, 0) => {
                refs.first()
                    .copied()
                    .map(|point_group_ordinal| RawButtonAction::AnimatePoint {
                        point_group_ordinal,
                    })
            }
            (4, 0) => {
                refs.first()
                    .copied()
                    .map(|point_group_ordinal| RawButtonAction::ScrollPoint {
                        point_group_ordinal,
                    })
            }
            (7, 0) => Some(RawButtonAction::Sequence {
                button_group_ordinals: refs,
                interval_ms: read_u32(action_payload, 16),
            }),
            (3, 1) => refs
                .first()
                .copied()
                .map(|point_group_ordinal| RawButtonAction::MovePoint {
                    point_group_ordinal,
                    target_group_ordinal: refs.get(1).copied(),
                }),
            (0, 7) => Some(RawButtonAction::ToggleVisibility { refs }),
            (1, 3) => Some(RawButtonAction::SetVisibility {
                refs,
                visible: true,
            }),
            (0, 3) => Some(RawButtonAction::SetVisibility {
                refs,
                visible: false,
            }),
            _ => None,
        };
        let Some(action) = action else {
            continue;
        };

        let anchor = button_label_groups
            .get(&group.ordinal)
            .cloned()
            .or_else(|| decode::decode_button_screen_anchor(file, group))
            .unwrap_or(PointRecord { x: 24.0, y: 24.0 });
        let text = decode::decode_label_name_raw(file, group)
            .filter(|label| !label.trim().is_empty())
            .unwrap_or_else(|| "按钮".to_string());

        raw_buttons.push(RawButton {
            group_ordinal: group.ordinal,
            text,
            anchor: ScreenPoint {
                x: anchor.x,
                y: anchor.y,
            },
            rect: None,
            action,
        });
    }

    let button_index_by_ordinal = raw_buttons
        .iter()
        .enumerate()
        .map(|(button_index, button)| (button.group_ordinal, button_index))
        .collect::<std::collections::BTreeMap<usize, usize>>();

    raw_buttons
        .into_iter()
        .filter_map(|button| {
            let action = match button.action {
                RawButtonAction::Link { href } => ButtonAction::Link { href },
                RawButtonAction::ToggleVisibility { refs } => {
                    let (point_indices, line_indices, circle_indices, polygon_indices) =
                        resolve_visibility_targets(
                            &refs,
                            group_to_point_index,
                            line_group_to_index,
                            circle_group_to_index,
                            polygon_group_to_index,
                        );
                    ButtonAction::ToggleVisibility {
                        point_indices,
                        line_indices,
                        circle_indices,
                        polygon_indices,
                    }
                }
                RawButtonAction::SetVisibility { refs, visible } => {
                    let (point_indices, line_indices, circle_indices, polygon_indices) =
                        resolve_visibility_targets(
                            &refs,
                            group_to_point_index,
                            line_group_to_index,
                            circle_group_to_index,
                            polygon_group_to_index,
                        );
                    ButtonAction::SetVisibility {
                        visible,
                        point_indices,
                        line_indices,
                        circle_indices,
                        polygon_indices,
                    }
                }
                RawButtonAction::MovePoint {
                    point_group_ordinal,
                    target_group_ordinal,
                } => ButtonAction::MovePoint {
                    point_index: group_to_point_index
                        .get(point_group_ordinal.checked_sub(1)?)
                        .copied()
                        .flatten()?,
                    target_point_index: target_group_ordinal.and_then(|ordinal| {
                        group_to_point_index
                            .get(ordinal.checked_sub(1)?)
                            .copied()
                            .flatten()
                    }),
                },
                RawButtonAction::AnimatePoint {
                    point_group_ordinal,
                } => ButtonAction::AnimatePoint {
                    point_index: group_to_point_index
                        .get(point_group_ordinal.checked_sub(1)?)
                        .copied()
                        .flatten()?,
                },
                RawButtonAction::ScrollPoint {
                    point_group_ordinal,
                } => ButtonAction::ScrollPoint {
                    point_index: group_to_point_index
                        .get(point_group_ordinal.checked_sub(1)?)
                        .copied()
                        .flatten()?,
                },
                RawButtonAction::Sequence {
                    button_group_ordinals,
                    interval_ms,
                } => ButtonAction::Sequence {
                    button_indices: button_group_ordinals
                        .into_iter()
                        .filter_map(|ordinal| button_index_by_ordinal.get(&ordinal).copied())
                        .collect(),
                    interval_ms,
                },
            };

            Some(SceneButton {
                text: button.text,
                anchor: button.anchor,
                rect: button.rect,
                action,
            })
        })
        .collect()
}

fn resolve_visibility_targets(
    refs: &[usize],
    group_to_point_index: &[Option<usize>],
    line_group_to_index: &[Option<usize>],
    circle_group_to_index: &[Option<usize>],
    polygon_group_to_index: &[Option<usize>],
) -> (Vec<usize>, Vec<usize>, Vec<usize>, Vec<usize>) {
    let point_indices = refs
        .iter()
        .filter_map(|ordinal| {
            group_to_point_index
                .get(ordinal.checked_sub(1)?)
                .copied()
                .flatten()
        })
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let line_indices = refs
        .iter()
        .filter_map(|ordinal| {
            line_group_to_index
                .get(ordinal.checked_sub(1)?)
                .copied()
                .flatten()
        })
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let circle_indices = refs
        .iter()
        .filter_map(|ordinal| {
            circle_group_to_index
                .get(ordinal.checked_sub(1)?)
                .copied()
                .flatten()
        })
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let polygon_indices = refs
        .iter()
        .filter_map(|ordinal| {
            polygon_group_to_index
                .get(ordinal.checked_sub(1)?)
                .copied()
                .flatten()
        })
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    (point_indices, line_indices, circle_indices, polygon_indices)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::format::GspFile;

    #[test]
    fn builds_function_plot_for_f_gsp() {
        let data = include_bytes!("../../../f.gsp");
        let file = GspFile::parse(data).expect("fixture parses");
        let scene = build_scene(&file);

        assert!(scene.graph_mode);
        assert!(
            scene.lines.iter().any(|line| {
                let min_x = line
                    .points
                    .iter()
                    .map(|point| point.x)
                    .fold(f64::INFINITY, f64::min);
                let max_x = line
                    .points
                    .iter()
                    .map(|point| point.x)
                    .fold(f64::NEG_INFINITY, f64::max);
                min_x <= 0.1 && max_x > 30.0
            }),
            "expected a non-degenerate function plot spanning the graph domain"
        );
        assert!(scene.bounds.min_x < -30.0);
        assert!(scene.bounds.max_y > 100.0);
        assert_eq!(scene.labels.len(), 1);
        assert_eq!(
            scene.labels[0].text,
            "f(x) = |x| + √x + ln(x) + log(x) + sgn(x) + round(x) + trunc(x)"
        );
    }

    #[test]
    fn preserves_constrained_points_in_edge_gsp() {
        let data = include_bytes!("../../../edge.gsp");
        let file = GspFile::parse(data).expect("fixture parses");
        let scene = build_scene(&file);

        assert_eq!(scene.circles.len(), 2);
        assert_eq!(scene.points.len(), 11);
        assert!(scene.points.iter().any(|point| {
            matches!(
                point.constraint,
                ScenePointConstraint::OnCircle {
                    center_index: 0,
                    radius_index: 1,
                    ..
                }
            )
        }));
        assert!(scene.points.iter().any(|point| {
            matches!(
                point.constraint,
                ScenePointConstraint::OnPolygonBoundary {
                    ref vertex_indices,
                    ..
                } if vertex_indices == &vec![2, 6, 3, 4]
            )
        }));
        assert!(scene.points.iter().any(|point| {
            (point.position.x + 9.17159).abs() < 0.01 && (point.position.y - 5.598877).abs() < 0.01
        }));
        assert!(scene.points.iter().any(|point| {
            (point.position.x + 4.956433).abs() < 0.01 && (point.position.y - 1.163518).abs() < 0.01
        }));
        assert!(
            scene.points.iter().any(|point| {
                matches!(point.constraint, ScenePointConstraint::OnPolyline { .. })
            })
        );
        assert_eq!(
            scene
                .labels
                .iter()
                .map(|label| label.text.as_str())
                .collect::<Vec<_>>(),
            vec![
                "a = 3.00",
                "b = 1.00",
                "f(x) = x + a*sin(x) + b",
                "f'(x) = 1 + a*cos(x)",
            ]
        );
    }

    #[test]
    fn preserves_translated_points_in_point_translation_gsp() {
        let data = include_bytes!("../../tests/fixtures/gsp/static/point_translation.gsp");
        let file = GspFile::parse(data).expect("fixture parses");
        let scene = build_scene(&file);

        assert_eq!(
            scene.points.len(),
            2,
            "expected base point and translated point"
        );
        let origin = scene
            .points
            .iter()
            .find(|point| matches!(point.constraint, ScenePointConstraint::Free))
            .expect("expected free origin point");
        let translated = scene
            .points
            .iter()
            .find(|point| matches!(point.constraint, ScenePointConstraint::Offset { .. }))
            .expect("expected translated offset point");

        match translated.constraint {
            ScenePointConstraint::Offset {
                origin_index,
                dx,
                dy,
            } => {
                assert_eq!(origin_index, 0);
                assert!(
                    dx.abs() < 0.001,
                    "expected 90-degree translation to keep x constant, got dx={dx}"
                );
                assert!(
                    dy < 0.0,
                    "expected upward translation in raw coordinates, got dy={dy}"
                );
                assert!(
                    (translated.position.x - (origin.position.x + dx)).abs() < 0.001
                        && (translated.position.y - (origin.position.y + dy)).abs() < 0.001,
                    "expected translated point to preserve offset from origin: origin={:?}, translated={:?}",
                    origin.position,
                    translated.position
                );
            }
            _ => panic!("expected offset constraint"),
        }
    }

    #[test]
    fn preserves_polygon_in_poly_gsp() {
        let data = include_bytes!("../../tests/fixtures/gsp/static/poly.gsp");
        let file = GspFile::parse(data).expect("fixture parses");
        let scene = build_scene(&file);

        assert_eq!(scene.polygons.len(), 1, "expected a single polygon");
        assert_eq!(
            scene.polygons[0].points.len(),
            4,
            "expected polygon to keep its four vertices"
        );
        assert_eq!(
            scene.polygons[0].color,
            [255, 128, 0, 127],
            "expected polygon fill opacity from source style metadata"
        );
        assert_eq!(scene.points.len(), 4, "expected four visible points");
        assert!(
            scene
                .points
                .iter()
                .all(|point| matches!(point.constraint, ScenePointConstraint::Free)),
            "expected polygon vertices to stay free points"
        );
    }

    #[test]
    fn preserves_polygon_boundary_point_in_poly_point_gsp() {
        let data = include_bytes!("../../tests/fixtures/gsp/static/poly_point.gsp");
        let file = GspFile::parse(data).expect("fixture parses");
        let scene = build_scene(&file);

        assert_eq!(scene.polygons.len(), 1, "expected a single polygon");
        assert_eq!(
            scene.points.len(),
            5,
            "expected four vertices and one constrained point"
        );
        assert_eq!(
            scene
                .points
                .iter()
                .filter(|point| matches!(point.constraint, ScenePointConstraint::Free))
                .count(),
            4,
            "expected four free polygon vertices"
        );
        assert!(scene.points.iter().any(|point| {
            matches!(
                point.constraint,
                ScenePointConstraint::OnPolygonBoundary {
                    ref vertex_indices,
                    edge_index: 2,
                    t,
                } if vertex_indices == &vec![0, 1, 2, 3] && (t - 0.4450450665338869).abs() < 0.001
            )
        }));
        assert!(scene.points.iter().any(|point| {
            (point.position.x - 487.23).abs() < 0.05 && (point.position.y - 262.28).abs() < 0.05
        }));
    }

    #[test]
    fn preserves_polygon_labels_in_poly_point_with_val_gsp() {
        let data = include_bytes!("../../tests/fixtures/gsp/static/poly_point_with_val.gsp");
        let file = GspFile::parse(data).expect("fixture parses");
        let scene = build_scene(&file);

        assert_eq!(scene.polygons.len(), 1, "expected a single polygon");
        assert_eq!(
            scene.points.len(),
            5,
            "expected four vertices and one constrained point"
        );
        let texts = scene
            .labels
            .iter()
            .map(|label| label.text.as_str())
            .collect::<Vec<_>>();
        assert!(
            texts.contains(&"A"),
            "expected point label A, got {texts:?}"
        );
        assert!(
            texts.contains(&"B"),
            "expected point label B, got {texts:?}"
        );
        assert!(
            texts.contains(&"C"),
            "expected point label C, got {texts:?}"
        );
        assert!(
            texts.contains(&"D"),
            "expected point label D, got {texts:?}"
        );
        assert!(
            texts.contains(&"E"),
            "expected constrained point label E, got {texts:?}"
        );
        assert!(
            texts.contains(&"E在ABCD上的t值 = 0.58"),
            "expected polygon parameter label, got {texts:?}"
        );
    }

    #[test]
    fn preserves_segment_parameter_label_in_segment_point_value_gsp() {
        let data = include_bytes!("../../tests/fixtures/gsp/static/segment_point_value.gsp");
        let file = GspFile::parse(data).expect("fixture parses");
        let scene = build_scene(&file);

        assert_eq!(scene.lines.len(), 1, "expected one segment");
        assert_eq!(
            scene.points.len(),
            3,
            "expected two endpoints and one constrained point"
        );
        let texts = scene
            .labels
            .iter()
            .map(|label| label.text.as_str())
            .collect::<Vec<_>>();
        assert!(
            texts.contains(&"A"),
            "expected point label A, got {texts:?}"
        );
        assert!(
            texts.contains(&"B"),
            "expected point label B, got {texts:?}"
        );
        assert!(
            texts.contains(&"C"),
            "expected point label C, got {texts:?}"
        );
        assert!(
            texts.contains(&"C在AB上的t值 = 0.51"),
            "expected segment parameter label, got {texts:?}"
        );
    }

    #[test]
    fn preserves_line_gsp() {
        let data = include_bytes!("../../tests/fixtures/gsp/static/line.gsp");
        let file = GspFile::parse(data).expect("fixture parses");
        let scene = build_scene(&file);

        assert_eq!(scene.lines.len(), 1, "expected one line");
        assert_eq!(scene.points.len(), 2, "expected two defining points");
        let line = &scene.lines[0];
        assert!(matches!(line.binding, Some(LineBinding::Line { .. })));
        let min_x = line
            .points
            .iter()
            .map(|point| point.x)
            .fold(f64::INFINITY, f64::min);
        let max_x = line
            .points
            .iter()
            .map(|point| point.x)
            .fold(f64::NEG_INFINITY, f64::max);
        assert!((min_x - scene.bounds.min_x).abs() < 1e-3);
        assert!((max_x - scene.bounds.max_x).abs() < 1e-3);
    }

    #[test]
    fn preserves_ray_gsp() {
        let data = include_bytes!("../../tests/fixtures/gsp/static/ray.gsp");
        let file = GspFile::parse(data).expect("fixture parses");
        let scene = build_scene(&file);

        assert_eq!(scene.lines.len(), 1, "expected one ray");
        assert_eq!(scene.points.len(), 2, "expected two defining points");
        let line = &scene.lines[0];
        assert!(matches!(line.binding, Some(LineBinding::Ray { .. })));
        let max_x = line
            .points
            .iter()
            .map(|point| point.x)
            .fold(f64::NEG_INFINITY, f64::max);
        assert!((max_x - scene.bounds.max_x).abs() < 1e-3);
        assert!(
            line.points
                .iter()
                .any(|point| (point.x - scene.points[0].position.x).abs() < 1e-3),
            "expected ray to include its start point"
        );
    }

    #[test]
    fn preserves_point_segment_value_segment_point_gsp() {
        let data =
            include_bytes!("../../tests/fixtures/gsp/static/point_segment_value_segment_point.gsp");
        let file = GspFile::parse(data).expect("fixture parses");
        let scene = build_scene(&file);

        assert_eq!(scene.lines.len(), 2, "expected two segments");
        let texts = scene
            .labels
            .iter()
            .map(|label| label.text.as_str())
            .collect::<Vec<_>>();
        assert!(
            texts.contains(&"C在AB上的t值 = 0.72"),
            "expected measured segment parameter label, got {texts:?}"
        );
        assert_eq!(
            scene.parameters.len(),
            0,
            "expected derived value, not slider parameter"
        );
        assert_eq!(
            scene
                .points
                .iter()
                .filter(|point| matches!(point.constraint, ScenePointConstraint::OnSegment { .. }))
                .count(),
            2,
            "expected measured point plus derived segment point"
        );
        assert!(
            scene
                .points
                .iter()
                .any(|point| matches!(point.constraint, ScenePointConstraint::OnCircle { .. })),
            "expected derived circle point"
        );
        assert!(
            scene.points.iter().any(|point| matches!(
                point.constraint,
                ScenePointConstraint::OnPolygonBoundary { .. }
            )),
            "expected derived polygon point"
        );
    }

    #[test]
    fn preserves_circle_parameter_label_in_circle_point_value_gsp() {
        let data = include_bytes!("../../tests/fixtures/gsp/static/circle_point_value.gsp");
        let file = GspFile::parse(data).expect("fixture parses");
        let scene = build_scene(&file);

        assert_eq!(scene.circles.len(), 1, "expected one circle");
        assert_eq!(
            scene.points.len(),
            3,
            "expected center, radius point, and constrained point"
        );
        let texts = scene
            .labels
            .iter()
            .map(|label| label.text.as_str())
            .collect::<Vec<_>>();
        assert!(
            texts.contains(&"A"),
            "expected point label A, got {texts:?}"
        );
        assert!(
            texts.contains(&"B"),
            "expected point label B, got {texts:?}"
        );
        assert!(
            texts.contains(&"C"),
            "expected point label C, got {texts:?}"
        );
        assert!(
            texts.contains(&"C在⊙AB上的值 = 0.38"),
            "expected circle parameter label, got {texts:?}"
        );
    }

    #[test]
    fn preserves_parameter_controlled_point_on_segment_gsp() {
        let data = include_bytes!("../../tests/fixtures/gsp/static/point_on_segment.gsp");
        let file = GspFile::parse(data).expect("fixture parses");
        let scene = build_scene(&file);

        assert_eq!(scene.lines.len(), 1, "expected one segment");
        assert_eq!(
            scene.points.len(),
            3,
            "expected endpoints plus controlled point"
        );
        assert_eq!(scene.parameters.len(), 1, "expected t parameter");
        assert_eq!(scene.parameters[0].name, "t₁");
        assert!((scene.parameters[0].value - 0.01).abs() < 0.001);
        assert!(scene.points.iter().any(|point| matches!(
            point.constraint,
            ScenePointConstraint::OnSegment { t, .. } if (t - 0.01).abs() < 0.001
        )));
    }

    #[test]
    fn preserves_parameter_controlled_point_on_poly_gsp() {
        let data = include_bytes!("../../tests/fixtures/gsp/static/point_on_poly.gsp");
        let file = GspFile::parse(data).expect("fixture parses");
        let scene = build_scene(&file);

        assert_eq!(scene.polygons.len(), 1, "expected one polygon");
        assert_eq!(
            scene.points.len(),
            4,
            "expected polygon vertices plus controlled point"
        );
        assert_eq!(scene.parameters.len(), 1, "expected t parameter");
        assert_eq!(scene.parameters[0].name, "t₁");
        assert!(scene.points.iter().any(|point| matches!(
            point.constraint,
            ScenePointConstraint::OnPolygonBoundary { .. }
        )));
    }

    #[test]
    fn preserves_parameter_controlled_point_on_circle_gsp() {
        let data = include_bytes!("../../tests/fixtures/gsp/static/point_on_circle.gsp");
        let file = GspFile::parse(data).expect("fixture parses");
        let scene = build_scene(&file);

        assert_eq!(scene.circles.len(), 1, "expected one circle");
        assert_eq!(
            scene.points.len(),
            3,
            "expected circle points plus controlled point"
        );
        assert_eq!(scene.parameters.len(), 1, "expected t parameter");
        assert_eq!(scene.parameters[0].name, "t₁");
        assert!(
            scene
                .points
                .iter()
                .any(|point| matches!(point.constraint, ScenePointConstraint::OnCircle { .. }))
        );
    }

    #[test]
    fn preserves_coordinate_point_in_cood_gsp() {
        let data = include_bytes!("../../tests/fixtures/gsp/static/cood.gsp");
        let file = GspFile::parse(data).expect("fixture parses");
        let scene = build_scene(&file);

        assert!(scene.graph_mode, "expected graph scene");
        assert_eq!(scene.parameters.len(), 1, "expected t parameter");
        assert_eq!(scene.parameters[0].name, "t₁");
        assert!((scene.parameters[0].value - 0.01).abs() < 0.001);
        assert!(
            scene.points.iter().any(|point| {
                point.binding.as_ref().is_some_and(|binding| {
                    matches!(
                        binding,
                        ScenePointBinding::Coordinate { name, .. } if name == "t₁"
                    )
                })
            }),
            "expected coordinate-controlled point"
        );
        assert!(scene.points.iter().any(|point| {
            (point.position.x - 0.01).abs() < 0.001 && (point.position.y - 1.01).abs() < 0.001
        }));
    }

    #[test]
    fn preserves_coordinate_trace_in_cood_trace_gsp() {
        let data = include_bytes!("../../tests/fixtures/gsp/static/cood-trace.gsp");
        let file = GspFile::parse(data).expect("fixture parses");
        let scene = build_scene(&file);

        assert!(scene.graph_mode, "expected graph scene");
        assert_eq!(scene.parameters.len(), 1, "expected t parameter");
        assert_eq!(scene.parameters[0].name, "t₁");
        assert!(
            scene.lines.iter().any(|line| {
                line.points.len() > 100
                    && line
                        .points
                        .first()
                        .is_some_and(|point| point.x.abs() < 0.001)
                    && line
                        .points
                        .first()
                        .is_some_and(|point| (point.y - 1.0).abs() < 0.001)
                    && line
                        .points
                        .last()
                        .is_some_and(|point| (point.x - 1.0).abs() < 0.001)
                    && line
                        .points
                        .last()
                        .is_some_and(|point| (point.y - 2.0).abs() < 0.001)
            }),
            "expected sampled coordinate trace line"
        );
    }

    #[test]
    fn preserves_parameter_driven_point_iteration_family() {
        let data =
            include_bytes!("../../tests/fixtures/gsp/static/简单迭代/原象点初象点深度5迭代.gsp");
        let file = GspFile::parse(data).expect("fixture parses");
        let scene = build_scene(&file);

        assert_eq!(scene.parameters.len(), 1, "expected n parameter");
        assert_eq!(scene.parameters[0].name, "n");
        assert_eq!(
            scene.point_iterations.len(),
            1,
            "expected one point iteration family"
        );
        match &scene.point_iterations[0] {
            PointIterationFamily::Offset {
                seed_index,
                depth,
                parameter_name,
                ..
            } => {
                assert_eq!(
                    *seed_index, 1,
                    "expected initial image point as iteration seed"
                );
                assert_eq!(*depth, 5, "expected exported depth");
                assert_eq!(parameter_name.as_deref(), Some("n"));
            }
            family => panic!("expected offset iteration family, got {family:?}"),
        }
        assert_eq!(
            scene.points.len(),
            7,
            "expected original point, initial point, and 5 iterates"
        );
    }

    #[test]
    fn preserves_non_graph_parameter_and_expression_labels_in_iteration_fixture() {
        let data = include_bytes!(
            "../../tests/fixtures/gsp/static/简单迭代/原象点和参数初象点和数值深度5迭代.gsp"
        );
        let file = GspFile::parse(data).expect("fixture parses");
        let scene = build_scene(&file);

        let parameter_names = scene
            .parameters
            .iter()
            .map(|parameter| parameter.name.as_str())
            .collect::<Vec<_>>();
        assert!(
            parameter_names.contains(&"n"),
            "expected n parameter, got {parameter_names:?}"
        );
        assert!(
            parameter_names.contains(&"a"),
            "expected a parameter, got {parameter_names:?}"
        );
        assert!(scene.labels.iter().any(|label| {
            matches!(
                label.binding,
                Some(TextLabelBinding::ParameterValue { ref name }) if name == "a"
            )
        }));
        assert!(scene.labels.iter().any(|label| {
            matches!(
                label.binding,
                Some(TextLabelBinding::PointExpressionValue {
                    ref parameter_name,
                    ..
                }) if parameter_name == "a"
            )
        }));
        assert!(scene.labels.iter().any(|label| {
            matches!(
                label.binding,
                Some(TextLabelBinding::ExpressionValue {
                    ref parameter_name,
                    ref expr_label,
                    ..
                }) if parameter_name == "a" && expr_label == "a + 1"
            )
        }));
        assert!(scene.point_iterations.iter().any(|family| {
            matches!(
                family,
                PointIterationFamily::Offset {
                    parameter_name,
                    ..
                } if parameter_name.as_deref() == Some("n")
            )
        }));
        assert!(scene.label_iterations.iter().any(|family| {
            matches!(
                family,
                LabelIterationFamily::PointExpression {
                    parameter_name,
                    depth_parameter_name,
                    ..
                } if parameter_name == "a" && depth_parameter_name.as_deref() == Some("n")
            )
        }));
    }

    #[test]
    fn preserves_default_depth_point_iteration_family() {
        let data = include_bytes!(
            "../../tests/fixtures/gsp/static/简单迭代/原象点初象点默认深度3迭代.gsp"
        );
        let file = GspFile::parse(data).expect("fixture parses");
        let scene = build_scene(&file);

        assert!(
            scene.parameters.is_empty(),
            "expected default-depth fixture without editable parameters"
        );
        assert_eq!(
            scene.point_iterations.len(),
            1,
            "expected one default-depth point iteration family"
        );
        match &scene.point_iterations[0] {
            PointIterationFamily::Offset {
                seed_index,
                depth,
                parameter_name,
                ..
            } => {
                assert_eq!(
                    *seed_index, 1,
                    "expected initial image point as iteration seed"
                );
                assert_eq!(*depth, 3, "expected default depth of three");
                assert_eq!(parameter_name, &None);
            }
            family => panic!("expected offset iteration family, got {family:?}"),
        }
        assert_eq!(
            scene.points.len(),
            5,
            "expected original point, initial point, and three default iterates"
        );
        assert!(
            matches!(
                scene.points[1].constraint,
                ScenePointConstraint::Offset {
                    origin_index: 0,
                    dx,
                    dy
                } if (dx - 37.79527559055118).abs() < 1e-6
                    && (dy + 37.79527559055118).abs() < 1e-6
            ),
            "expected legacy initial image point to preserve its 1cm horizontal and vertical offsets"
        );
    }

    #[test]
    fn preserves_scaled_point_and_single_parameter_label_in_scale_gsp() {
        let data = include_bytes!("../../tests/fixtures/gsp/static/scale.gsp");
        let file = GspFile::parse(data).expect("fixture parses");
        let scene = build_scene(&file);

        assert_eq!(
            scene.circles.len(),
            2,
            "expected original and scaled circle"
        );
        assert_eq!(
            scene.polygons.len(),
            2,
            "expected original and scaled polygon"
        );
        let texts = scene
            .labels
            .iter()
            .map(|label| label.text.as_str())
            .collect::<Vec<_>>();
        assert!(
            texts.contains(&"A"),
            "expected point label A, got {texts:?}"
        );
        assert!(
            texts.contains(&"B"),
            "expected point label B, got {texts:?}"
        );
        assert!(
            texts.contains(&"C"),
            "expected point label C, got {texts:?}"
        );
        assert!(
            scene.points.len() >= 3,
            "expected source point, center point, and transformed point"
        );
        assert!(scene.points.iter().any(|point| matches!(
            point.binding,
            Some(ScenePointBinding::Scale { factor, .. }) if (factor - 1.0 / 3.0).abs() < 0.0001
        )));
    }

    #[test]
    fn preserves_reflection_point_circle_and_polygon_gsp() {
        let data = include_bytes!("../../tests/fixtures/gsp/static/reflection.gsp");
        let file = GspFile::parse(data).expect("fixture parses");
        let scene = build_scene(&file);

        assert_eq!(
            scene.circles.len(),
            2,
            "expected original and reflected circle"
        );
        assert_eq!(
            scene.polygons.len(),
            2,
            "expected original and reflected polygon"
        );
        assert!(
            scene
                .points
                .iter()
                .any(|point| matches!(point.binding, Some(ScenePointBinding::Reflect { .. })))
        );
        assert!(scene.circles.iter().any(|circle| matches!(
            circle.binding,
            Some(crate::runtime::scene::ShapeBinding::ReflectCircle { .. })
        )));
        assert!(scene.polygons.iter().any(|polygon| matches!(
            polygon.binding,
            Some(crate::runtime::scene::ShapeBinding::ReflectPolygon { .. })
        )));
    }

    #[test]
    fn preserves_translation_and_right_angle_rotation_in_transform_fixture() {
        let data = include_bytes!("../../tests/fixtures/gsp/static/平移旋转缩放轴对称.gsp");
        let file = GspFile::parse(data).expect("fixture parses");
        let scene = build_scene(&file);

        assert!(
            scene
                .points
                .iter()
                .any(|point| matches!(point.binding, Some(ScenePointBinding::Translate { .. })))
        );
        assert!(scene.polygons.iter().any(|polygon| matches!(
            polygon.binding,
            Some(crate::runtime::scene::ShapeBinding::TranslatePolygon { .. })
        )));
        assert!(scene.polygons.iter().any(|polygon| matches!(
            polygon.binding,
            Some(crate::runtime::scene::ShapeBinding::RotatePolygon { angle_degrees, .. })
                if (angle_degrees - 90.0).abs() < 1e-3
        )));
    }

    #[test]
    fn preserves_reflect_then_translate_in_translation_fixture() {
        let data = include_bytes!("../../tests/fixtures/gsp/static/平移.gsp");
        let file = GspFile::parse(data).expect("fixture parses");
        let scene = build_scene(&file);

        let reflected_points = scene
            .points
            .iter()
            .enumerate()
            .filter_map(|(index, point)| match point.binding {
                Some(ScenePointBinding::Reflect { .. }) => Some(index),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(reflected_points.len(), 2, "expected reflected A' and B'");

        assert!(scene.polygons.iter().any(|polygon| matches!(
            polygon.binding,
            Some(crate::runtime::scene::ShapeBinding::TranslatePolygon {
                vector_start_index,
                vector_end_index,
                ..
            }) if reflected_points.contains(&vector_start_index)
                && reflected_points.contains(&vector_end_index)
        )));
    }

    #[test]
    fn preserves_point_label_in_point_label_gsp() {
        let data = include_bytes!("../../tests/fixtures/gsp/static/point_label.gsp");
        let file = GspFile::parse(data).expect("fixture parses");
        let scene = build_scene(&file);

        assert!(
            scene.labels.iter().any(|label| label.text == "A"),
            "expected point label A, got {:?}",
            scene
                .labels
                .iter()
                .map(|label| &label.text)
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn preserves_point_and_segment_labels_in_segment_label_gsp() {
        let data = include_bytes!("../../tests/fixtures/gsp/static/segment_label.gsp");
        let file = GspFile::parse(data).expect("fixture parses");
        let scene = build_scene(&file);

        let texts = scene
            .labels
            .iter()
            .map(|label| label.text.as_str())
            .collect::<Vec<_>>();
        assert!(
            texts.contains(&"A"),
            "expected point label A, got {texts:?}"
        );
        assert!(
            texts.contains(&"B"),
            "expected point label B, got {texts:?}"
        );
        assert!(
            texts.contains(&"j"),
            "expected segment label j, got {texts:?}"
        );
    }

    #[test]
    fn keeps_control_labels_in_non_graph_sample() {
        let data = include_bytes!("../../../Samples/个人专栏/潘建平作品/加油潘建平老师.gsp");
        let file = GspFile::parse(data).expect("fixture parses");
        let scene = build_scene(&file);

        assert!(
            scene.labels.iter().any(|label| label.text.contains("单价")),
            "expected UI text label from rich text payload, got {:?}",
            scene
                .labels
                .iter()
                .map(|label| label.text.as_str())
                .collect::<Vec<_>>()
        );
    }
}
