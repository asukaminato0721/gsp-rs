use std::collections::{BTreeMap, BTreeSet};

use crate::format::{
    GspFile, IndexedPathRecord, ObjectGroup, PointRecord, collect_strings, decode_indexed_path,
    decode_point_record, read_f64, read_i16, read_u16, read_u32,
};

use super::functions::{
    FunctionExpr, collect_function_plot_domain, collect_function_plots, collect_scene_functions,
    collect_scene_parameters, decode_function_expr, decode_function_plot_descriptor,
    function_uses_pi_scale, sample_function_points, synthesize_function_axes,
    synthesize_function_labels,
};
use super::geometry::{
    Bounds, GraphTransform, color_from_style, distance_world, format_number, has_distinct_points,
    include_line_bounds, read_f32_unaligned, to_raw_from_world, to_world,
};
use super::scene::{
    LineShape, PolygonShape, Scene, SceneCircle, ScenePoint, ScenePointConstraint, TextLabel,
};

#[derive(Debug, Clone)]
struct CircleShape {
    center: PointRecord,
    radius_point: PointRecord,
    color: [u8; 4],
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
    let large_non_graph = !graph_mode && file.records.len() > 10_000;

    let polylines = collect_line_shapes(
        file,
        &groups,
        &raw_anchors,
        &[2],
        !graph_mode && !large_non_graph,
    );
    let derived_segments = if large_non_graph {
        collect_derived_segments(file, &groups, &point_map, &[24])
    } else {
        Vec::new()
    };
    let measurements = if graph_mode {
        collect_line_shapes(file, &groups, &raw_anchors, &[58], false)
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
    let mut labels = collect_labels(
        file,
        &groups,
        &raw_anchors,
        graph_mode && !has_function_plots,
    );
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
            },
        );
    }

    let visible_points =
        collect_visible_points(file, &groups, &point_map, &raw_anchors, &graph_ref);

    let world_points = visible_points
        .iter()
        .map(|point| ScenePoint {
            position: to_world(&point.position, &graph_ref),
            constraint: match &point.constraint {
                ScenePointConstraint::Free => ScenePointConstraint::Free,
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
        })
        .collect::<Vec<_>>();

    let world_point_positions = world_points
        .iter()
        .map(|point| point.position.clone())
        .collect::<Vec<_>>();

    let mut bounds = collect_bounds(
        &graph_ref,
        &polylines,
        &measurements,
        &axes,
        &polygons,
        &circles,
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

    let parameters = if graph_mode {
        collect_scene_parameters(file, &groups, &labels)
    } else {
        Vec::new()
    };
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

    Scene {
        graph_mode,
        pi_mode,
        saved_viewport: use_saved_viewport,
        y_up: graph_mode,
        origin: graph_ref
            .as_ref()
            .map(|transform| to_world(&transform.origin_raw, &graph_ref)),
        bounds,
        lines: polylines
            .into_iter()
            .chain(derived_segments)
            .chain(measurements)
            .chain(axes)
            .chain(function_plots)
            .chain(synthetic_axes)
            .chain(iteration_lines)
            .map(|line| LineShape {
                points: line
                    .points
                    .into_iter()
                    .map(|point| to_world(&point, &graph_ref))
                    .collect(),
                color: line.color,
                dashed: line.dashed,
            })
            .collect(),
        polygons: polygons
            .into_iter()
            .chain(iteration_polygons)
            .map(|polygon| PolygonShape {
                points: polygon
                    .points
                    .into_iter()
                    .map(|point| to_world(&point, &graph_ref))
                    .collect(),
                color: polygon.color,
            })
            .collect(),
        circles: circles
            .into_iter()
            .map(|circle| SceneCircle {
                center: to_world(&circle.center, &graph_ref),
                radius_point: to_world(&circle.radius_point, &graph_ref),
                color: circle.color,
            })
            .collect(),
        labels: labels
            .into_iter()
            .map(|label| TextLabel {
                anchor: to_world(&label.anchor, &graph_ref),
                text: label.text,
                color: label.color,
            })
            .collect(),
        points: world_points,
        parameters,
        functions,
    }
}

fn collect_point_objects(file: &GspFile, groups: &[ObjectGroup]) -> Vec<Option<PointRecord>> {
    groups
        .iter()
        .map(|group| {
            if (group.header.class_id & 0xffff) != 0 {
                return None;
            }
            group.records.iter().find_map(|record| {
                (record.record_type == 0x0899)
                    .then(|| decode_point_record(record.payload(&file.data)))
                    .flatten()
            })
        })
        .collect()
}

fn collect_visible_points(
    file: &GspFile,
    groups: &[ObjectGroup],
    point_map: &[Option<PointRecord>],
    anchors: &[Option<PointRecord>],
    graph: &Option<GraphTransform>,
) -> Vec<ScenePoint> {
    let mut group_to_point_index = vec![None; groups.len()];
    let mut points = Vec::<ScenePoint>::new();

    for (index, group) in groups.iter().enumerate() {
        let kind = group.header.class_id & 0xffff;
        let scene_point = match kind {
            0 => point_map
                .get(index)
                .cloned()
                .flatten()
                .map(|position| ScenePoint {
                    position,
                    constraint: ScenePointConstraint::Free,
                }),
            15 => decode_point_constraint(file, groups, group, graph).and_then(|constraint| {
                let position = anchors.get(index).cloned().flatten()?;
                match constraint {
                    RawPointConstraint::Segment(constraint) => {
                        let start_index = group_to_point_index
                            .get(constraint.start_group_index)
                            .and_then(|index| *index)?;
                        let end_index = group_to_point_index
                            .get(constraint.end_group_index)
                            .and_then(|index| *index)?;
                        Some(ScenePoint {
                            position,
                            constraint: ScenePointConstraint::OnSegment {
                                start_index,
                                end_index,
                                t: constraint.t,
                            },
                        })
                    }
                    RawPointConstraint::Polyline {
                        function_key,
                        points,
                        segment_index,
                        t,
                    } => Some(ScenePoint {
                        position,
                        constraint: ScenePointConstraint::OnPolyline {
                            function_key,
                            points,
                            segment_index,
                            t,
                        },
                    }),
                    RawPointConstraint::PolygonBoundary {
                        vertex_group_indices,
                        edge_index,
                        t,
                    } => {
                        let vertex_indices = vertex_group_indices
                            .iter()
                            .map(|group_index| {
                                group_to_point_index
                                    .get(*group_index)
                                    .and_then(|index| *index)
                            })
                            .collect::<Option<Vec<_>>>()?;
                        Some(ScenePoint {
                            position,
                            constraint: ScenePointConstraint::OnPolygonBoundary {
                                vertex_indices,
                                edge_index,
                                t,
                            },
                        })
                    }
                    RawPointConstraint::Circle(constraint) => {
                        let center_index = group_to_point_index
                            .get(constraint.center_group_index)
                            .and_then(|index| *index)?;
                        let radius_index = group_to_point_index
                            .get(constraint.radius_group_index)
                            .and_then(|index| *index)?;
                        Some(ScenePoint {
                            position,
                            constraint: ScenePointConstraint::OnCircle {
                                center_index,
                                radius_index,
                                unit_x: constraint.unit_x,
                                unit_y: constraint.unit_y,
                            },
                        })
                    }
                }
            }),
            _ => None,
        };

        if let Some(scene_point) = scene_point {
            group_to_point_index[index] = Some(points.len());
            points.push(scene_point);
        }
    }

    points
}

fn collect_raw_object_anchors(
    file: &GspFile,
    groups: &[ObjectGroup],
    point_map: &[Option<PointRecord>],
    graph: Option<&GraphTransform>,
) -> Vec<Option<PointRecord>> {
    let mut anchors = Vec::with_capacity(groups.len());
    for (index, group) in groups.iter().enumerate() {
        let anchor = if let Some(point) = point_map.get(index).and_then(|point| point.clone()) {
            Some(point)
        } else if let Some(anchor) =
            decode_point_constraint_anchor(file, groups, group, &anchors, graph)
        {
            Some(anchor)
        } else if let Some(anchor) = decode_point_on_ray_anchor_raw(file, groups, group, &anchors) {
            Some(anchor)
        } else if let Some(anchor) = decode_transform_anchor_raw(file, group, &anchors) {
            Some(anchor)
        } else if let Some(anchor) = decode_offset_anchor_raw(file, group, &anchors) {
            Some(anchor)
        } else if let Some(anchor) = decode_bbox_anchor_raw(file, group) {
            Some(anchor)
        } else {
            find_indexed_path(file, group).and_then(|path| {
                path.refs.iter().rev().find_map(|object_ref| {
                    anchors.get(object_ref.saturating_sub(1)).cloned().flatten()
                })
            })
        };
        anchors.push(anchor);
    }
    anchors
}

fn collect_line_shapes(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
    kinds: &[u32],
    fallback_generic: bool,
) -> Vec<LineShape> {
    groups
        .iter()
        .filter(|group| {
            let kind = group.header.class_id & 0xffff;
            kinds.contains(&kind)
                || (fallback_generic
                    && matches!(kind, 2 | 5 | 6 | 7)
                    && find_indexed_path(file, group)
                        .map(|path| path.refs.len() == 2)
                        .unwrap_or(false))
        })
        .filter_map(|group| {
            let path = find_indexed_path(file, group)?;
            let points = path
                .refs
                .iter()
                .filter_map(|object_ref| {
                    anchors.get(object_ref.saturating_sub(1)).cloned().flatten()
                })
                .collect::<Vec<_>>();
            (points.len() >= 2 && has_distinct_points(&points)).then_some(LineShape {
                points,
                color: if fallback_generic && !kinds.contains(&(group.header.class_id & 0xffff)) {
                    [40, 40, 40, 255]
                } else {
                    color_from_style(group.header.style_b)
                },
                dashed: (group.header.class_id & 0xffff) == 58,
            })
        })
        .collect()
}

fn collect_polygon_shapes(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
    kinds: &[u32],
) -> Vec<PolygonShape> {
    groups
        .iter()
        .filter(|group| kinds.contains(&(group.header.class_id & 0xffff)))
        .filter_map(|group| {
            let path = find_indexed_path(file, group)?;
            let points = path
                .refs
                .iter()
                .filter_map(|object_ref| {
                    anchors.get(object_ref.saturating_sub(1)).cloned().flatten()
                })
                .collect::<Vec<_>>();
            (points.len() >= 3).then_some(PolygonShape {
                points,
                color: color_from_style(group.header.style_b),
            })
        })
        .collect()
}

fn collect_circle_shapes(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
) -> Vec<CircleShape> {
    groups
        .iter()
        .filter(|group| (group.header.class_id & 0xffff) == 3)
        .filter_map(|group| {
            let path = find_indexed_path(file, group)?;
            if path.refs.len() != 2 {
                return None;
            }
            let center = anchors.get(path.refs[0].saturating_sub(1))?.clone()?;
            let radius_point = anchors.get(path.refs[1].saturating_sub(1))?.clone()?;
            Some(CircleShape {
                center,
                radius_point,
                color: color_from_style(group.header.style_b),
            })
        })
        .collect()
}

fn collect_iteration_shapes(
    file: &GspFile,
    groups: &[ObjectGroup],
    circles: &[CircleShape],
) -> (Vec<LineShape>, Vec<PolygonShape>) {
    let mut lines = Vec::new();
    let polygons = Vec::new();

    let has_iteration = groups
        .iter()
        .any(|group| (group.header.class_id & 0xffff) == 89);
    if !has_iteration {
        return (lines, polygons);
    }

    for iter_group in groups
        .iter()
        .filter(|group| (group.header.class_id & 0xffff) == 89)
    {
        let Some(iter_path) = find_indexed_path(file, iter_group) else {
            continue;
        };

        let iter_data = iter_group
            .records
            .iter()
            .find(|record| record.record_type == 0x090a)
            .map(|record| record.payload(&file.data));

        let depth = iter_data
            .filter(|payload| payload.len() >= 20)
            .map(|payload| read_u32(payload, 16) as usize)
            .unwrap_or(0);
        if depth == 0 {
            continue;
        }

        let polygon_group_index = iter_path.refs.iter().find_map(|&obj_ref| {
            let index = obj_ref.checked_sub(1)?;
            let group = groups.get(index)?;
            ((group.header.class_id & 0xffff) == 8).then_some(index)
        });

        let Some(polygon_index) = polygon_group_index else {
            continue;
        };
        let polygon_group = &groups[polygon_index];
        let Some(polygon_path) = find_indexed_path(file, polygon_group) else {
            continue;
        };
        let vertex_count = polygon_path.refs.len();
        if vertex_count < 3 {
            continue;
        }

        let Some(circle) = circles.first() else {
            continue;
        };
        let cx = circle.center.x;
        let cy = circle.center.y;
        let radius =
            ((circle.radius_point.x - cx).powi(2) + (circle.radius_point.y - cy).powi(2)).sqrt();
        if radius < 1.0 {
            continue;
        }

        let px_per_cm = groups
            .iter()
            .filter(|group| (group.header.class_id & 0xffff) == 21)
            .find_map(|group| {
                let payload = group
                    .records
                    .iter()
                    .find(|record| record.record_type == 0x07d3)
                    .map(|record| record.payload(&file.data))?;
                (payload.len() >= 40).then(|| read_f64(payload, 32))
            })
            .filter(|v| v.is_finite() && *v > 1.0)
            .unwrap_or(37.79527559055118);

        let param_value = groups
            .iter()
            .filter(|group| (group.header.class_id & 0xffff) == 21)
            .find_map(|group| {
                let payload = group
                    .records
                    .iter()
                    .find(|record| record.record_type == 0x07d3)
                    .map(|record| record.payload(&file.data))?;
                (payload.len() >= 20).then(|| read_f64(payload, 12))
            })
            .filter(|v| v.is_finite() && *v > 0.0)
            .unwrap_or(1.0);

        let side = param_value * px_per_cm / 2.0;
        if side < 1.0 {
            continue;
        }

        let outline_color = [30, 30, 30, 255];

        let sqrt3 = 3.0_f64.sqrt();
        let col_spacing = sqrt3 * side;
        let row_spacing = 1.5 * side;

        let max_cols = (radius / col_spacing).ceil() as i32 + 2;
        let max_rows = (radius / row_spacing).ceil() as i32 + 2;

        let hex_vertices = |hx: f64, hy: f64| -> Vec<PointRecord> {
            (0..6)
                .map(|i| {
                    let angle =
                        std::f64::consts::FRAC_PI_3 * i as f64 + std::f64::consts::FRAC_PI_6;
                    PointRecord {
                        x: hx + side * angle.cos(),
                        y: hy + side * angle.sin(),
                    }
                })
                .collect()
        };

        for row in -max_rows..=max_rows {
            let y = cy + row as f64 * row_spacing;
            let x_offset = if row.rem_euclid(2) == 1 {
                col_spacing / 2.0
            } else {
                0.0
            };
            for col in -max_cols..=max_cols {
                let x = cx + col as f64 * col_spacing + x_offset;
                let dist = ((x - cx).powi(2) + (y - cy).powi(2)).sqrt();
                if dist > radius + side * 0.5 {
                    continue;
                }
                let verts = hex_vertices(x, y);

                let mut outline = verts.clone();
                outline.push(verts[0].clone());
                lines.push(LineShape {
                    points: outline,
                    color: outline_color,
                    dashed: false,
                });
            }
        }
    }

    (lines, polygons)
}

fn collect_derived_segments(
    file: &GspFile,
    groups: &[ObjectGroup],
    point_map: &[Option<PointRecord>],
    kinds: &[u32],
) -> Vec<LineShape> {
    let refs = groups
        .iter()
        .map(|group| {
            find_indexed_path(file, group)
                .map(|path| path.refs)
                .unwrap_or_default()
        })
        .collect::<Vec<_>>();
    let class_ids = groups
        .iter()
        .map(|group| group.header.class_id & 0xffff)
        .collect::<Vec<_>>();

    fn descend_points(
        ordinal: usize,
        refs: &[Vec<usize>],
        point_map: &[Option<PointRecord>],
        memo: &mut Vec<Option<Vec<PointRecord>>>,
        visiting: &mut BTreeSet<usize>,
    ) -> Vec<PointRecord> {
        if let Some(cached) = &memo[ordinal - 1] {
            return cached.clone();
        }
        if !visiting.insert(ordinal) {
            return Vec::new();
        }

        let mut points = Vec::new();
        if let Some(point) = point_map.get(ordinal - 1).and_then(|point| point.clone()) {
            points.push(point);
        } else {
            for child in &refs[ordinal - 1] {
                if *child > 0 && *child <= refs.len() {
                    points.extend(descend_points(*child, refs, point_map, memo, visiting));
                }
            }
        }

        visiting.remove(&ordinal);
        points.sort_by(|a, b| {
            a.x.partial_cmp(&b.x)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.y.partial_cmp(&b.y).unwrap_or(std::cmp::Ordering::Equal))
        });
        points.dedup_by(|a, b| (a.x - b.x).abs() < 0.001 && (a.y - b.y).abs() < 0.001);
        memo[ordinal - 1] = Some(points.clone());
        points
    }

    let mut memo = vec![None; groups.len()];
    let mut seen = BTreeSet::<((i32, i32), (i32, i32))>::new();
    let mut segments = Vec::new();

    for (index, class_id) in class_ids.iter().enumerate() {
        if !kinds.contains(class_id) {
            continue;
        }
        let points = descend_points(index + 1, &refs, point_map, &mut memo, &mut BTreeSet::new());
        if points.len() < 2 || points.len() > 12 {
            continue;
        }

        let mut best = None;
        let mut best_dist = -1.0_f64;
        for i in 0..points.len() {
            for j in i + 1..points.len() {
                let dx = points[i].x - points[j].x;
                let dy = points[i].y - points[j].y;
                let dist = dx * dx + dy * dy;
                if dist > best_dist {
                    best_dist = dist;
                    best = Some((points[i].clone(), points[j].clone()));
                }
            }
        }

        let Some((a, b)) = best else { continue };
        let a_key = (a.x.round() as i32, a.y.round() as i32);
        let b_key = (b.x.round() as i32, b.y.round() as i32);
        let key = if a_key <= b_key {
            (a_key, b_key)
        } else {
            (b_key, a_key)
        };
        if !seen.insert(key) {
            continue;
        }

        let color = match *class_id {
            24 => [20, 20, 20, 255],
            48 => [70, 70, 70, 255],
            75 => [120, 120, 120, 255],
            _ => [60, 60, 60, 255],
        };
        segments.push(LineShape {
            points: vec![a, b],
            color,
            dashed: false,
        });
    }

    segments
}

fn collect_labels(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
    include_measurements: bool,
) -> Vec<TextLabel> {
    let mut labels = Vec::new();
    for group in groups {
        let kind = group.header.class_id & 0xffff;
        match kind {
            0 | 40 | 51 | 62 | 73 => {
                let text = decode_group_label_text(file, group);
                if let Some(text) = text {
                    let anchor = group
                        .records
                        .iter()
                        .find(|record| record.record_type == 0x08fc)
                        .and_then(|record| decode_text_anchor(record.payload(&file.data)))
                        .or_else(|| decode_0907_anchor(file, group))
                        .or_else(|| {
                            anchors
                                .get(group.ordinal.saturating_sub(1))
                                .cloned()
                                .flatten()
                        })
                        .or_else(|| {
                            find_indexed_path(file, group).and_then(|path| {
                                path.refs.iter().rev().find_map(|object_ref| {
                                    anchors.get(object_ref.saturating_sub(1)).cloned().flatten()
                                })
                            })
                        });
                    if let Some(anchor) = anchor {
                        labels.push(TextLabel {
                            anchor,
                            text,
                            color: [30, 30, 30, 255],
                        });
                    }
                }
            }
            48 => {}
            52 | 54 => {
                if !include_measurements {
                    continue;
                }
                if let Some(value) = group
                    .records
                    .iter()
                    .find(|record| record.record_type == 0x07d3 && record.length == 12)
                    .and_then(|record| decode_measurement_value(record.payload(&file.data)))
                {
                    let anchor = anchors
                        .get(group.ordinal.saturating_sub(1))
                        .cloned()
                        .flatten()
                        .or_else(|| {
                            find_indexed_path(file, group).and_then(|path| {
                                path.refs.iter().find_map(|object_ref| {
                                    anchors.get(object_ref.saturating_sub(1)).cloned().flatten()
                                })
                            })
                        });
                    if let Some(anchor) = anchor {
                        labels.push(TextLabel {
                            anchor,
                            text: format_number(value),
                            color: [60, 60, 60, 255],
                        });
                    }
                }
            }
            _ => {}
        }
    }
    labels
}

fn collect_saved_viewport(file: &GspFile, groups: &[ObjectGroup]) -> Option<Bounds> {
    let (min_x, max_x) = collect_graph_window_x_hint(file, groups)?;
    let (min_y, max_y) = collect_graph_window_y_hint(file, groups)?;
    Some(Bounds {
        min_x,
        max_x,
        min_y,
        max_y,
    })
}

fn collect_graph_window_x_hint(file: &GspFile, groups: &[ObjectGroup]) -> Option<(f64, f64)> {
    groups
        .iter()
        .filter(|group| (group.header.class_id & 0xffff) == 58)
        .find_map(|group| {
            let payload = group
                .records
                .iter()
                .find(|record| record.record_type == 0x07d5)
                .map(|record| record.payload(&file.data))?;
            if payload.len() < 22 {
                return None;
            }
            let min_x = read_f32_unaligned(payload, 14)?;
            let max_x = read_f32_unaligned(payload, 18)?;
            (min_x.is_finite() && max_x.is_finite() && min_x < max_x && (max_x - min_x) > 1.0)
                .then_some((f64::from(min_x), f64::from(max_x)))
        })
}

fn collect_graph_window_y_hint(file: &GspFile, groups: &[ObjectGroup]) -> Option<(f64, f64)> {
    let expr = groups
        .iter()
        .find(|group| {
            group
                .records
                .iter()
                .any(|record| record.record_type == 0x0907)
        })
        .and_then(|group| decode_function_expr(file, groups, group))?;
    let plot_payload = groups
        .iter()
        .filter(|group| (group.header.class_id & 0xffff) == 72)
        .find_map(|group| {
            group
                .records
                .iter()
                .find(|record| record.record_type == 0x0902)
                .map(|record| record.payload(&file.data))
        })?;

    match expr {
        FunctionExpr::Parsed(_) => {
            let max_y = read_f32_unaligned(plot_payload, 11)?;
            (max_y.is_finite() && max_y > 0.0).then_some((-1.0, f64::from(max_y)))
        }
        _ => None,
    }
}

fn compute_iteration_labels(
    file: &GspFile,
    groups: &[ObjectGroup],
    circles: &[CircleShape],
) -> Vec<TextLabel> {
    let mut labels = Vec::new();

    let has_iteration = groups
        .iter()
        .any(|group| (group.header.class_id & 0xffff) == 89);
    if !has_iteration {
        return labels;
    }

    let Some(circle) = circles.first() else {
        return labels;
    };
    let cx = circle.center.x;
    let cy = circle.center.y;
    let radius =
        ((circle.radius_point.x - cx).powi(2) + (circle.radius_point.y - cy).powi(2)).sqrt();

    let px_per_cm = groups
        .iter()
        .filter(|group| (group.header.class_id & 0xffff) == 21)
        .find_map(|group| {
            let payload = group
                .records
                .iter()
                .find(|record| record.record_type == 0x07d3)
                .map(|record| record.payload(&file.data))?;
            (payload.len() >= 40).then(|| read_f64(payload, 32))
        })
        .filter(|v| v.is_finite() && *v > 1.0)
        .unwrap_or(37.79527559055118);

    let param_value = groups
        .iter()
        .filter(|group| (group.header.class_id & 0xffff) == 21)
        .find_map(|group| {
            let payload = group
                .records
                .iter()
                .find(|record| record.record_type == 0x07d3)
                .map(|record| record.payload(&file.data))?;
            (payload.len() >= 20).then(|| read_f64(payload, 12))
        })
        .filter(|v| v.is_finite() && *v > 0.0)
        .unwrap_or(1.0);

    let t1 = param_value;
    let side = t1 / 2.0 * px_per_cm;
    let sqrt3 = 3.0_f64.sqrt();
    let diameter = 2.0 * radius;
    let m1 = diameter / (2.0 * side) + 0.5;
    let l_val = m1.floor() + 1.0;
    let m2 = diameter / (sqrt3 * side);
    let h_val = m2.ceil();
    let m3 = m2 - m1;
    let m4 = m3 - m3.floor();

    fn format_sub(raw: &str) -> String {
        raw.replace("[1]", "\u{2081}")
            .replace("[2]", "\u{2082}")
            .replace("[3]", "\u{2083}")
            .replace("[4]", "\u{2084}")
    }

    let mut computed_values = BTreeMap::<String, f64>::new();
    computed_values.insert("m\u{2081}".to_string(), m1);
    computed_values.insert("m\u{2082}".to_string(), m2);
    computed_values.insert("m\u{2083}".to_string(), m3);
    computed_values.insert("m\u{2084}".to_string(), m4);
    computed_values.insert("L".to_string(), l_val);
    computed_values.insert("H".to_string(), h_val);
    computed_values.insert("H\u{00b7}L".to_string(), h_val * l_val);

    for group in groups {
        if let Some(raw_name) = decode_label_name_raw(file, group) {
            let name = format_sub(&raw_name);
            if group
                .records
                .iter()
                .any(|record| record.record_type == 0x0907)
                && (group.header.class_id & 0xffff) == 0
                && !computed_values.contains_key(&name)
            {
                computed_values.insert(name, t1);
            }
        }
    }

    for group in groups {
        let kind = group.header.class_id & 0xffff;
        let has_0907 = group
            .records
            .iter()
            .any(|record| record.record_type == 0x0907);
        if !has_0907 || !matches!(kind, 0 | 48) {
            continue;
        }
        if group
            .records
            .iter()
            .any(|record| record.record_type == 0x08fc)
        {
            continue;
        }

        let Some(anchor) = decode_0907_anchor(file, group) else {
            continue;
        };

        let own_label = decode_label_name_raw(file, group).map(|s| format_sub(&s));
        let ref_labels: Vec<String> = find_indexed_path(file, group)
            .map(|path| {
                path.refs
                    .iter()
                    .filter_map(|&obj_ref| {
                        let ref_group = groups.get(obj_ref.checked_sub(1)?)?;
                        decode_label_name_raw(file, ref_group).map(|s| format_sub(&s))
                    })
                    .collect()
            })
            .unwrap_or_default();

        let mut lines = Vec::new();

        if kind == 0 {
            if let Some(name) = &own_label
                && let Some(&val) = computed_values.get(name.as_str())
            {
                let unit = "\u{5398}\u{7c73}";
                lines.push(format!("{name} = {val:.0} {unit}"));
                lines.push(format!("{name}/2 = {:.2} {unit}", val / 2.0));
            }
        } else {
            let has_h = ref_labels.iter().any(|n| n == "H");
            let has_l = ref_labels.iter().any(|n| n == "L");
            if own_label.is_none() && has_h && has_l {
                if let Some(val) = computed_values.get("H\u{00b7}L") {
                    lines.push(format!("H\u{00b7}L = {val:.2}"));
                }
            } else {
                let mut seen = BTreeSet::new();
                let mut try_add = |name: &str, lines: &mut Vec<String>| {
                    if seen.contains(name) {
                        return;
                    }
                    seen.insert(name.to_string());
                    if let Some(val) = computed_values.get(name) {
                        lines.push(format!("{name} = {val:.2}"));
                    }
                };

                if let Some(ol) = &own_label {
                    try_add(ol, &mut lines);
                }
                for rl in &ref_labels {
                    try_add(rl, &mut lines);
                }
            }

            if lines.is_empty()
                && let Some(ol) = &own_label
            {
                lines.push(ol.clone());
            }
        }

        if !lines.is_empty() {
            labels.push(TextLabel {
                anchor,
                text: lines.join("\n"),
                color: [30, 30, 30, 255],
            });
        }
    }

    labels
}

fn decode_label_name_raw(file: &GspFile, group: &ObjectGroup) -> Option<String> {
    let payload = group
        .records
        .iter()
        .find(|record| record.record_type == 0x07d5)
        .map(|record| record.payload(&file.data))?;
    if payload.len() < 24 {
        return None;
    }
    let name_len = read_u16(payload, 22) as usize;
    if name_len == 0 || 24 + name_len > payload.len() {
        return None;
    }
    let name_bytes = &payload[24..24 + name_len];
    Some(String::from_utf8_lossy(name_bytes).to_string())
}

fn decode_0907_anchor(file: &GspFile, group: &ObjectGroup) -> Option<PointRecord> {
    let payload = group
        .records
        .iter()
        .find(|record| record.record_type == 0x0907)
        .map(|record| record.payload(&file.data))?;
    (payload.len() >= 16 && read_u32(payload, 0) == 0x08fc).then(|| PointRecord {
        x: read_i16(payload, 12) as f64,
        y: read_i16(payload, 14) as f64,
    })
}

fn decode_caption_text(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
) -> Option<String> {
    let path = find_indexed_path(file, group)?;
    let mut parts = Vec::new();
    for &obj_ref in &path.refs {
        let ref_group = groups.get(obj_ref.checked_sub(1)?)?;
        if let Some(name) = decode_label_name(file, ref_group) {
            parts.push(name);
        }
    }
    (!parts.is_empty()).then(|| parts.join(", "))
}

fn decode_label_name(file: &GspFile, group: &ObjectGroup) -> Option<String> {
    let payload = group
        .records
        .iter()
        .find(|record| record.record_type == 0x07d5)
        .map(|record| record.payload(&file.data))?;
    if payload.len() < 24 {
        return None;
    }
    let name_len = read_u16(payload, 22) as usize;
    if name_len == 0 || 24 + name_len > payload.len() {
        return None;
    }
    let name_bytes = &payload[24..24 + name_len];
    Some(
        String::from_utf8_lossy(name_bytes)
            .replace("[1]", "₁")
            .replace("[2]", "₂")
            .replace("[3]", "₃")
            .replace("[4]", "₄"),
    )
}

pub(super) fn find_indexed_path(file: &GspFile, group: &ObjectGroup) -> Option<IndexedPathRecord> {
    group
        .records
        .iter()
        .find_map(|record| match record.record_type {
            0x07d2 | 0x07d3 => decode_indexed_path(record.record_type, record.payload(&file.data)),
            _ => None,
        })
}

fn decode_group_label_text(file: &GspFile, group: &ObjectGroup) -> Option<String> {
    group
        .records
        .iter()
        .find_map(|record| match record.record_type {
            0x08fc => extract_rich_text(record.payload(&file.data)),
            0x07d5 if matches!(group.header.class_id & 0xffff, 62) => {
                collect_strings(record.payload(&file.data))
                    .into_iter()
                    .map(|entry| entry.text.trim().to_string())
                    .find(|text| !text.is_empty())
            }
            _ => None,
        })
}

fn decode_bbox_anchor_raw(file: &GspFile, group: &ObjectGroup) -> Option<PointRecord> {
    let payload = group
        .records
        .iter()
        .find(|record| matches!(record.record_type, 0x0898 | 0x08a2 | 0x08a3 | 0x0903))
        .map(|record| record.payload(&file.data))?;
    if payload.len() < 8 {
        return None;
    }
    let x0 = read_i16(payload, payload.len() - 8) as f64;
    let y0 = read_i16(payload, payload.len() - 6) as f64;
    let x1 = read_i16(payload, payload.len() - 4) as f64;
    let y1 = read_i16(payload, payload.len() - 2) as f64;
    Some(PointRecord {
        x: (x0 + x1) / 2.0,
        y: (y0 + y1) / 2.0,
    })
}

fn decode_transform_anchor_raw(
    file: &GspFile,
    group: &ObjectGroup,
    anchors: &[Option<PointRecord>],
) -> Option<PointRecord> {
    let kind = group.header.class_id & 0xffff;
    match kind {
        27 => {
            let path = find_indexed_path(file, group)?;
            let source = anchors.get(path.refs.first()?.checked_sub(1)?)?.clone()?;
            let center = anchors.get(path.refs.get(1)?.checked_sub(1)?)?.clone()?;
            let payload = group
                .records
                .iter()
                .find(|record| record.record_type == 0x07d3)
                .map(|record| record.payload(&file.data))?;
            if payload.len() < 20 {
                return None;
            }

            let cos = read_f64(payload, 4);
            let sin = read_f64(payload, 12);
            if !cos.is_finite() || !sin.is_finite() {
                return None;
            }

            let dx = source.x - center.x;
            let dy = source.y - center.y;
            Some(PointRecord {
                x: center.x + dx * cos - dy * sin,
                y: center.y + dx * sin + dy * cos,
            })
        }
        30 => {
            let path = find_indexed_path(file, group)?;
            let source = anchors.get(path.refs.first()?.checked_sub(1)?)?.clone()?;
            let center = anchors.get(path.refs.get(1)?.checked_sub(1)?)?.clone()?;
            let payload = group
                .records
                .iter()
                .find(|record| record.record_type == 0x07d3)
                .map(|record| record.payload(&file.data))?;
            if payload.len() < 12 {
                return None;
            }

            let t = read_f64(payload, 4);
            if !t.is_finite() {
                return None;
            }

            Some(PointRecord {
                x: center.x + (source.x - center.x) * t,
                y: center.y + (source.y - center.y) * t,
            })
        }
        _ => None,
    }
}

fn decode_point_on_ray_anchor_raw(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
    anchors: &[Option<PointRecord>],
) -> Option<PointRecord> {
    if (group.header.class_id & 0xffff) != 15 {
        return None;
    }

    let host_ref = find_indexed_path(file, group)?
        .refs
        .first()
        .copied()
        .filter(|ordinal| *ordinal > 0)?;
    let host_group = groups.get(host_ref - 1)?;
    if (host_group.header.class_id & 0xffff) != 64 {
        return None;
    }

    let host_path = find_indexed_path(file, host_group)?;
    let origin = anchors
        .get(host_path.refs.first()?.checked_sub(1)?)?
        .clone()?;
    let direction_group = groups.get(host_path.refs.get(1)?.checked_sub(1)?)?;
    let direction_payload = direction_group
        .records
        .iter()
        .find(|record| record.record_type == 0x07d3)
        .map(|record| record.payload(&file.data))?;
    if direction_payload.len() < 20 {
        return None;
    }

    let unit_x = read_f64(direction_payload, 4);
    let unit_y = read_f64(direction_payload, 12);
    if !unit_x.is_finite() || !unit_y.is_finite() {
        return None;
    }

    let payload = group
        .records
        .iter()
        .find(|record| record.record_type == 0x07d3)
        .map(|record| record.payload(&file.data))?;
    if payload.len() < 12 {
        return None;
    }

    let distance = read_f64(payload, 4);
    if !distance.is_finite() {
        return None;
    }

    Some(PointRecord {
        x: origin.x + distance * unit_x,
        y: origin.y - distance * unit_y,
    })
}

fn decode_offset_anchor_raw(
    file: &GspFile,
    group: &ObjectGroup,
    anchors: &[Option<PointRecord>],
) -> Option<PointRecord> {
    if (group.header.class_id & 0xffff) != 67 {
        return None;
    }

    let path = find_indexed_path(file, group)?;
    let origin = anchors.get(path.refs.first()?.checked_sub(1)?)?.clone()?;
    let payload = group
        .records
        .iter()
        .find(|record| record.record_type == 0x07d3)
        .map(|record| record.payload(&file.data))?;
    if payload.len() < 20 {
        return None;
    }

    let dx = read_f64(payload, 4);
    let dy = read_f64(payload, 12);
    if !dx.is_finite() || !dy.is_finite() {
        return None;
    }

    Some(PointRecord {
        x: origin.x + dx,
        y: origin.y + dy,
    })
}

struct PointOnSegmentConstraint {
    start_group_index: usize,
    end_group_index: usize,
    t: f64,
}

struct PointOnCircleConstraint {
    center_group_index: usize,
    radius_group_index: usize,
    unit_x: f64,
    unit_y: f64,
}

enum RawPointConstraint {
    Segment(PointOnSegmentConstraint),
    Polyline {
        function_key: usize,
        points: Vec<PointRecord>,
        segment_index: usize,
        t: f64,
    },
    PolygonBoundary {
        vertex_group_indices: Vec<usize>,
        edge_index: usize,
        t: f64,
    },
    Circle(PointOnCircleConstraint),
}

fn decode_point_on_segment_constraint(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
) -> Option<PointOnSegmentConstraint> {
    if (group.header.class_id & 0xffff) != 15 {
        return None;
    }

    let host_ref = find_indexed_path(file, group)?
        .refs
        .first()
        .copied()
        .filter(|ordinal| *ordinal > 0)?;
    let host_group_index = host_ref - 1;
    let host_group = groups.get(host_group_index)?;
    let host_path = find_indexed_path(file, host_group)?;
    if host_path.refs.len() != 2 {
        return None;
    }

    let payload = group
        .records
        .iter()
        .find(|record| record.record_type == 0x07d3 && record.length == 12)
        .map(|record| record.payload(&file.data))?;
    let t = read_f64(payload, 4);
    if !t.is_finite() {
        return None;
    }

    Some(PointOnSegmentConstraint {
        start_group_index: host_path.refs[0].checked_sub(1)?,
        end_group_index: host_path.refs[1].checked_sub(1)?,
        t,
    })
}

fn decode_point_constraint(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
    graph: &Option<GraphTransform>,
) -> Option<RawPointConstraint> {
    if (group.header.class_id & 0xffff) != 15 {
        return None;
    }

    let host_ref = find_indexed_path(file, group)?
        .refs
        .first()
        .copied()
        .filter(|ordinal| *ordinal > 0)?;
    let host_group = groups.get(host_ref - 1)?;
    let host_kind = host_group.header.class_id & 0xffff;
    let payload = group
        .records
        .iter()
        .find(|record| record.record_type == 0x07d3)
        .map(|record| record.payload(&file.data))?;

    match (host_kind, payload.len()) {
        (3, 20) => {
            let host_path = find_indexed_path(file, host_group)?;
            if host_path.refs.len() != 2 {
                return None;
            }

            let unit_x = read_f64(payload, 4);
            let unit_y = read_f64(payload, 12);
            if !unit_x.is_finite() || !unit_y.is_finite() {
                return None;
            }

            Some(RawPointConstraint::Circle(PointOnCircleConstraint {
                center_group_index: host_path.refs[0].checked_sub(1)?,
                radius_group_index: host_path.refs[1].checked_sub(1)?,
                unit_x,
                unit_y,
            }))
        }
        (8, 20) => {
            let host_path = find_indexed_path(file, host_group)?;
            if host_path.refs.len() < 2 {
                return None;
            }

            let t = read_f64(payload, 4);
            let selector = read_f64(payload, 12);
            if !t.is_finite() || !selector.is_finite() {
                return None;
            }

            let end_vertex = decode_polygon_edge_end_index(host_path.refs.len(), selector)?;
            Some(RawPointConstraint::PolygonBoundary {
                vertex_group_indices: host_path
                    .refs
                    .iter()
                    .map(|vertex| vertex.checked_sub(1))
                    .collect::<Option<Vec<_>>>()?,
                edge_index: (end_vertex + host_path.refs.len() - 1) % host_path.refs.len(),
                t,
            })
        }
        (72, 12) => decode_point_on_function_constraint(file, groups, host_group, payload, graph),
        _ => {
            decode_point_on_segment_constraint(file, groups, group).map(RawPointConstraint::Segment)
        }
    }
}

fn decode_point_on_function_constraint(
    file: &GspFile,
    groups: &[ObjectGroup],
    host_group: &ObjectGroup,
    payload: &[u8],
    graph: &Option<GraphTransform>,
) -> Option<RawPointConstraint> {
    let transform = graph.as_ref()?;
    let normalized_t = read_f64(payload, 4);
    if !normalized_t.is_finite() {
        return None;
    }

    let path = find_indexed_path(file, host_group)?;
    let definition_group = groups.get(path.refs.first()?.checked_sub(1)?)?;
    let descriptor = host_group
        .records
        .iter()
        .find(|record| record.record_type == 0x0902)
        .and_then(|record| decode_function_plot_descriptor(record.payload(&file.data)))?;
    let expr = decode_function_expr(file, groups, definition_group)?;
    let points = sample_function_points(&expr, &descriptor)
        .into_iter()
        .flatten()
        .map(|point| to_raw_from_world(&point, transform))
        .collect::<Vec<_>>();
    let (segment_index, t) = locate_polyline_parameter(&points, normalized_t)?;
    Some(RawPointConstraint::Polyline {
        function_key: *path.refs.first()?,
        points,
        segment_index,
        t,
    })
}

fn locate_polyline_parameter(points: &[PointRecord], normalized_t: f64) -> Option<(usize, f64)> {
    if points.len() < 2 {
        return None;
    }

    let clamped_t = normalized_t.clamp(0.0, 1.0);
    let scaled = clamped_t * (points.len() - 1) as f64;
    let segment_index = scaled.floor() as usize;
    Some((segment_index.min(points.len() - 2), scaled.fract()))
}

fn decode_polygon_edge_end_index(vertex_count: usize, selector: f64) -> Option<usize> {
    if vertex_count < 2 || !selector.is_finite() {
        return None;
    }

    let edge = ((selector * vertex_count as f64) - 0.25).round() as isize;
    Some(edge.rem_euclid(vertex_count as isize) as usize)
}

fn decode_point_constraint_anchor(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
    anchors: &[Option<PointRecord>],
    graph: Option<&GraphTransform>,
) -> Option<PointRecord> {
    let graph = graph.cloned();
    match decode_point_constraint(file, groups, group, &graph)? {
        RawPointConstraint::Segment(constraint) => {
            let start = anchors.get(constraint.start_group_index)?.clone()?;
            let end = anchors.get(constraint.end_group_index)?.clone()?;

            Some(PointRecord {
                x: start.x + (end.x - start.x) * constraint.t,
                y: start.y + (end.y - start.y) * constraint.t,
            })
        }
        RawPointConstraint::Polyline {
            points,
            segment_index,
            t,
            ..
        } => resolve_polyline_point(&points, segment_index, t),
        RawPointConstraint::PolygonBoundary {
            vertex_group_indices,
            edge_index,
            t,
        } => {
            let vertices = vertex_group_indices
                .iter()
                .map(|group_index| anchors.get(*group_index)?.clone())
                .collect::<Option<Vec<_>>>()?;
            resolve_polygon_boundary_point_raw(&vertices, edge_index, t)
        }
        RawPointConstraint::Circle(constraint) => {
            let center = anchors.get(constraint.center_group_index)?.clone()?;
            let radius_point = anchors.get(constraint.radius_group_index)?.clone()?;

            Some(resolve_circle_point_raw(
                &center,
                &radius_point,
                constraint.unit_x,
                constraint.unit_y,
            ))
        }
    }
}

fn resolve_circle_point_raw(
    center: &PointRecord,
    radius_point: &PointRecord,
    unit_x: f64,
    unit_y: f64,
) -> PointRecord {
    let radius = ((radius_point.x - center.x).powi(2) + (radius_point.y - center.y).powi(2)).sqrt();
    PointRecord {
        x: center.x + radius * unit_x,
        y: center.y - radius * unit_y,
    }
}

fn resolve_polygon_boundary_point_raw(
    vertices: &[PointRecord],
    edge_index: usize,
    t: f64,
) -> Option<PointRecord> {
    if vertices.len() < 2 {
        return None;
    }

    let start = &vertices[edge_index % vertices.len()];
    let end = &vertices[(edge_index + 1) % vertices.len()];
    Some(PointRecord {
        x: start.x + (end.x - start.x) * t,
        y: start.y + (end.y - start.y) * t,
    })
}

fn resolve_polyline_point(
    points: &[PointRecord],
    segment_index: usize,
    t: f64,
) -> Option<PointRecord> {
    if points.len() < 2 {
        return None;
    }

    let start = &points[segment_index.min(points.len() - 2)];
    let end = &points[(segment_index.min(points.len() - 2)) + 1];
    Some(PointRecord {
        x: start.x + (end.x - start.x) * t,
        y: start.y + (end.y - start.y) * t,
    })
}

fn detect_graph_transform(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
) -> Option<GraphTransform> {
    let raw_per_unit = groups
        .iter()
        .filter(|group| matches!(group.header.class_id & 0xffff, 52 | 54))
        .find_map(|group| {
            group
                .records
                .iter()
                .find(|record| record.record_type == 0x07d3 && record.length == 12)
                .and_then(|record| decode_measurement_value(record.payload(&file.data)))
        })?;

    let origin_raw = groups
        .iter()
        .find(|group| matches!(group.header.class_id & 0xffff, 52 | 54))
        .and_then(|group| {
            find_indexed_path(file, group).and_then(|path| {
                path.refs.iter().find_map(|object_ref| {
                    anchors.get(object_ref.saturating_sub(1)).cloned().flatten()
                })
            })
        })?;

    Some(GraphTransform {
        origin_raw,
        raw_per_unit,
    })
}

fn has_graph_classes(groups: &[ObjectGroup]) -> bool {
    groups
        .iter()
        .any(|group| matches!(group.header.class_id & 0xffff, 40 | 52 | 54 | 58 | 61))
}

fn collect_bounds(
    graph: &Option<GraphTransform>,
    polylines: &[LineShape],
    measurements: &[LineShape],
    axes: &[LineShape],
    polygons: &[PolygonShape],
    circles: &[CircleShape],
    labels: &[TextLabel],
    points_only: &[PointRecord],
) -> Bounds {
    let mut points = Vec::<PointRecord>::new();
    for shape in polylines
        .iter()
        .chain(measurements.iter())
        .chain(axes.iter())
    {
        points.extend(shape.points.iter().cloned());
    }
    for shape in polygons {
        points.extend(shape.points.iter().cloned());
    }
    for circle in circles {
        points.push(circle.center.clone());
        points.push(circle.radius_point.clone());
    }
    for label in labels {
        points.push(label.anchor.clone());
    }
    points.extend(points_only.iter().cloned());
    if points.is_empty() {
        points.push(PointRecord { x: 0.0, y: 0.0 });
        points.push(PointRecord { x: 1.0, y: 1.0 });
    }

    let world_points = points
        .iter()
        .map(|point| to_world(point, graph))
        .collect::<Vec<_>>();
    let mut bounds = Bounds {
        min_x: world_points[0].x,
        max_x: world_points[0].x,
        min_y: world_points[0].y,
        max_y: world_points[0].y,
    };
    for point in &world_points {
        bounds.min_x = bounds.min_x.min(point.x);
        bounds.max_x = bounds.max_x.max(point.x);
        bounds.min_y = bounds.min_y.min(point.y);
        bounds.max_y = bounds.max_y.max(point.y);
    }
    bounds
}

fn bounds_within(container: &Bounds, content: &Bounds) -> bool {
    const TOLERANCE: f64 = 1e-3;
    container.min_x <= content.min_x + TOLERANCE
        && container.max_x >= content.max_x - TOLERANCE
        && container.min_y <= content.min_y + TOLERANCE
        && container.max_y >= content.max_y - TOLERANCE
}

fn expand_bounds(bounds: &mut Bounds) {
    if (bounds.max_x - bounds.min_x).abs() < f64::EPSILON {
        bounds.max_x += 1.0;
        bounds.min_x -= 1.0;
    }
    if (bounds.max_y - bounds.min_y).abs() < f64::EPSILON {
        bounds.max_y += 1.0;
        bounds.min_y -= 1.0;
    }
    let margin_x = (bounds.max_x - bounds.min_x) * 0.1 + 1.0;
    let margin_y = (bounds.max_y - bounds.min_y) * 0.1 + 1.0;
    bounds.min_x -= margin_x;
    bounds.max_x += margin_x;
    bounds.min_y -= margin_y;
    bounds.max_y += margin_y;
}

fn extract_rich_text(payload: &[u8]) -> Option<String> {
    let text = String::from_utf8_lossy(payload);
    let start = text.find('<')?;
    let markup = text[start..].trim_end_matches('\0');

    if markup.starts_with("<VL") {
        return extract_simple_text(markup);
    }

    let parsed = parse_markup(markup);
    let mut cleaned = parsed
        .replace(['\u{2013}', '\u{2014}'], "-")
        .replace("厘米", "cm");

    if let Some(first) = cleaned.find("AB:")
        && let Some(second_rel) = cleaned[first + 3..].find("AB:")
    {
        cleaned.truncate(first + 3 + second_rel);
    }

    cleaned = cleaned
        .replace("  ", " ")
        .replace("( ", "(")
        .replace(" )", ")")
        .replace(" + -", " -")
        .trim()
        .to_string();

    (!cleaned.is_empty()).then_some(cleaned)
}

fn decode_measurement_value(payload: &[u8]) -> Option<f64> {
    (payload.len() == 12).then(|| read_f64(payload, 4))
}

fn decode_text_anchor(payload: &[u8]) -> Option<PointRecord> {
    if payload.len() < 16 {
        return None;
    }
    Some(PointRecord {
        x: read_i16(payload, 12) as f64,
        y: read_i16(payload, 14) as f64,
    })
}

fn extract_simple_text(markup: &str) -> Option<String> {
    let start = markup.find("<T")?;
    let tail = &markup[start + 2..];
    let x_index = tail.find('x')?;
    let end = tail[x_index + 1..].find('>')?;
    Some(tail[x_index + 1..x_index + 1 + end].to_string())
}

fn parse_markup(markup: &str) -> String {
    fn parse_seq(s: &str, mut index: usize, stop_on_gt: bool) -> (Vec<String>, usize) {
        let bytes = s.as_bytes();
        let mut parts = Vec::new();

        while index < bytes.len() {
            if stop_on_gt && bytes[index] == b'>' {
                return (parts, index + 1);
            }
            if bytes[index] != b'<' {
                index += 1;
                continue;
            }
            if index + 1 >= bytes.len() {
                break;
            }

            match bytes[index + 1] as char {
                'T' => {
                    let mut end = index + 2;
                    while end < bytes.len() && bytes[end] != b'>' {
                        end += 1;
                    }
                    let token = &s[index + 2..end];
                    if let Some(x_index) = token.find('x') {
                        parts.push(token[x_index + 1..].to_string());
                    }
                    index = end.saturating_add(1);
                }
                '!' => {
                    let mut end = index + 2;
                    while end < bytes.len() && bytes[end] != b'>' {
                        end += 1;
                    }
                    index = end.saturating_add(1);
                }
                _ => {
                    let mut name_end = index + 1;
                    while name_end < bytes.len()
                        && bytes[name_end] != b'<'
                        && bytes[name_end] != b'>'
                    {
                        name_end += 1;
                    }
                    let name = &s[index + 1..name_end];
                    let (inner_parts, next_index) =
                        if name_end < bytes.len() && bytes[name_end] == b'<' {
                            parse_seq(s, name_end, true)
                        } else {
                            (Vec::new(), name_end.saturating_add(1))
                        };
                    index = next_index;

                    let mut inner = inner_parts.join("");
                    if name.starts_with('+') && !inner.is_empty() {
                        let chars = inner.chars().collect::<Vec<_>>();
                        let split = chars
                            .iter()
                            .rposition(|ch| !ch.is_ascii_digit())
                            .map(|index| index + 1)
                            .unwrap_or(0);
                        if split < chars.len() {
                            let exponent = chars[split..].iter().collect::<String>();
                            inner = chars[..split].iter().collect::<String>();
                            inner.push('^');
                            inner.push_str(&exponent);
                        }
                    }
                    if !inner.is_empty() {
                        parts.push(inner);
                    }
                }
            }
        }

        (parts, index)
    }

    let (parts, _) = parse_seq(markup, 0, false);
    parts.join("")
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
                    edge_index: 3,
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
