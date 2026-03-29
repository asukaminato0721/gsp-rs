use crate::format::{
    GspFile, IndexedPathRecord, ObjectGroup, PointRecord, decode_indexed_path, decode_point_record,
    read_f64, read_u16, read_u32,
};
use std::collections::BTreeSet;

#[derive(Debug, Clone)]
struct GraphTransform {
    origin_raw: PointRecord,
    raw_per_unit: f64,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Bounds {
    pub(crate) min_x: f64,
    pub(crate) max_x: f64,
    pub(crate) min_y: f64,
    pub(crate) max_y: f64,
}

#[derive(Debug, Clone)]
pub(crate) struct Scene {
    pub(crate) graph_mode: bool,
    pub(crate) pi_mode: bool,
    pub(crate) y_up: bool,
    pub(crate) origin: Option<PointRecord>,
    pub(crate) bounds: Bounds,
    pub(crate) lines: Vec<LineShape>,
    pub(crate) polygons: Vec<PolygonShape>,
    pub(crate) circles: Vec<SceneCircle>,
    pub(crate) labels: Vec<TextLabel>,
    pub(crate) points: Vec<ScenePoint>,
}

#[derive(Debug, Clone)]
pub(crate) struct ScenePoint {
    pub(crate) position: PointRecord,
    pub(crate) constraint: ScenePointConstraint,
}

#[derive(Debug, Clone)]
pub(crate) enum ScenePointConstraint {
    Free,
    OnSegment {
        start_index: usize,
        end_index: usize,
        t: f64,
    },
}

#[derive(Debug, Clone)]
pub(crate) struct LineShape {
    pub(crate) points: Vec<PointRecord>,
    pub(crate) color: [u8; 4],
    pub(crate) dashed: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct PolygonShape {
    pub(crate) points: Vec<PointRecord>,
    pub(crate) color: [u8; 4],
}

#[derive(Debug, Clone)]
struct CircleShape {
    center: PointRecord,
    radius_point: PointRecord,
    color: [u8; 4],
}

#[derive(Debug, Clone)]
struct FunctionPlotDescriptor {
    x_min: f64,
    x_max: f64,
    sample_count: usize,
}

#[derive(Debug, Clone, PartialEq)]
enum FunctionExpr {
    Constant(f64),
    Identity,
    SinIdentity,
    CosIdentityPlus(f64),
    TanIdentityMinus(f64),
    Parsed(ParsedFunctionExpr),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BinaryOp {
    Add,
    Sub,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UnaryFunction {
    Sin,
    Cos,
    Tan,
    Abs,
    Sqrt,
    Ln,
    Log10,
    Sign,
    Round,
    Trunc,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum FunctionTerm {
    Variable,
    Constant(f64),
    UnaryX(UnaryFunction),
}

#[derive(Debug, Clone, PartialEq)]
struct ParsedFunctionExpr {
    head: FunctionTerm,
    tail: Vec<(BinaryOp, FunctionTerm)>,
}

#[derive(Debug, Clone)]
pub(crate) struct SceneCircle {
    pub(crate) center: PointRecord,
    pub(crate) radius_point: PointRecord,
    pub(crate) color: [u8; 4],
}

#[derive(Debug, Clone)]
pub(crate) struct TextLabel {
    pub(crate) anchor: PointRecord,
    pub(crate) text: String,
    pub(crate) color: [u8; 4],
}

pub(crate) fn build_scene(file: &GspFile) -> Scene {
    let groups = file.object_groups();
    let point_map = collect_point_objects(file, &groups);
    let raw_anchors = collect_raw_object_anchors(file, &groups, &point_map);
    let graph = detect_graph_transform(file, &groups, &raw_anchors);
    let graph_mode = graph.is_some() && has_graph_classes(&groups);
    let graph_ref = if graph_mode { graph.clone() } else { None };
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
    let polygons = collect_polygon_shapes(
        file,
        &groups,
        &raw_anchors,
        &[8],
        !graph_mode && !large_non_graph,
    );
    let circles = collect_circle_shapes(file, &groups, &point_map);
    let synthetic_axes = if graph_mode && has_function_plots && axes.is_empty() {
        synthesize_function_axes(&function_plots, function_plot_domain, &graph_ref)
    } else {
        Vec::new()
    };
    let mut labels = if graph_mode {
        collect_labels(file, &groups, &raw_anchors, !has_function_plots)
    } else {
        Vec::new()
    };
    if graph_mode && has_function_plots {
        labels.extend(synthesize_function_labels(
            file,
            &groups,
            &function_plots,
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

    let visible_points = collect_visible_points(file, &groups, &point_map, &raw_anchors);

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
    if let Some((domain_min_x, domain_max_x)) = function_plot_domain {
        bounds.min_x = bounds.min_x.min(domain_min_x);
        bounds.max_x = bounds.max_x.max(domain_max_x);
        bounds.min_y = bounds.min_y.min(0.0);
        bounds.max_y = bounds.max_y.max(0.0);
    }
    expand_bounds(&mut bounds);

    Scene {
        graph_mode,
        pi_mode: graph_mode && has_function_plots,
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
            15 => decode_point_on_segment_constraint(file, groups, group).and_then(|constraint| {
                let start_index = group_to_point_index
                    .get(constraint.start_group_index)
                    .and_then(|index| *index)?;
                let end_index = group_to_point_index
                    .get(constraint.end_group_index)
                    .and_then(|index| *index)?;
                let position = anchors.get(index).cloned().flatten()?;
                Some(ScenePoint {
                    position,
                    constraint: ScenePointConstraint::OnSegment {
                        start_index,
                        end_index,
                        t: constraint.t,
                    },
                })
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
) -> Vec<Option<PointRecord>> {
    let mut anchors = Vec::with_capacity(groups.len());
    for (index, group) in groups.iter().enumerate() {
        let anchor = if let Some(point) = point_map.get(index).and_then(|point| point.clone()) {
            Some(point)
        } else if let Some(anchor) = decode_point_on_segment_anchor(file, groups, group, &anchors) {
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
                    && !matches!(kind, 0 | 3 | 8)
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
    fallback_generic: bool,
) -> Vec<PolygonShape> {
    groups
        .iter()
        .filter(|group| {
            let kind = group.header.class_id & 0xffff;
            kinds.contains(&kind)
                || (fallback_generic
                    && !matches!(kind, 0 | 2 | 3)
                    && find_indexed_path(file, group)
                        .map(|path| path.refs.len() >= 3)
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
            (points.len() >= 3).then_some(PolygonShape {
                points,
                color: if fallback_generic && (group.header.class_id & 0xffff) != 8 {
                    [170, 220, 170, 255]
                } else {
                    color_from_style(group.header.style_b)
                },
            })
        })
        .collect()
}

fn collect_circle_shapes(
    file: &GspFile,
    groups: &[ObjectGroup],
    point_map: &[Option<PointRecord>],
) -> Vec<CircleShape> {
    groups
        .iter()
        .filter(|group| (group.header.class_id & 0xffff) == 3)
        .filter_map(|group| {
            let path = find_indexed_path(file, group)?;
            if path.refs.len() != 2 {
                return None;
            }
            let center = point_map.get(path.refs[0].saturating_sub(1))?.clone()?;
            let radius_point = point_map.get(path.refs[1].saturating_sub(1))?.clone()?;
            Some(CircleShape {
                center,
                radius_point,
                color: color_from_style(group.header.style_b),
            })
        })
        .collect()
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
            0 | 40 | 51 => {
                if let Some(text) = group
                    .records
                    .iter()
                    .find(|record| record.record_type == 0x08fc)
                    .and_then(|record| extract_rich_text(record.payload(&file.data)))
                {
                    let anchor = group
                        .records
                        .iter()
                        .find(|record| record.record_type == 0x08fc)
                        .and_then(|record| decode_text_anchor(record.payload(&file.data)))
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

fn collect_function_plots(
    file: &GspFile,
    groups: &[ObjectGroup],
    graph: &Option<GraphTransform>,
) -> Vec<LineShape> {
    let Some(transform) = graph.as_ref() else {
        return Vec::new();
    };

    let mut plots = Vec::new();
    for group in groups.iter().filter(|group| (group.header.class_id & 0xffff) == 72) {
        let Some(path) = find_indexed_path(file, group) else {
            continue;
        };
        if path.refs.len() < 2 {
            continue;
        }

        let Some(definition_index) = path.refs[0].checked_sub(1) else {
            continue;
        };
        let Some(definition_group) = groups.get(definition_index) else {
            continue;
        };
        let Some(descriptor) = group
            .records
            .iter()
            .find(|record| record.record_type == 0x0902)
            .and_then(|record| decode_function_plot_descriptor(record.payload(&file.data)))
        else {
            continue;
        };
        let Some(expr) = decode_function_expr(file, definition_group) else {
            continue;
        };

        for mut points in sample_function_points(&expr, &descriptor) {
            if !has_distinct_points(&points) {
                continue;
            }

            for point in &mut points {
                *point = to_raw_from_world(point, transform);
            }

            plots.push(LineShape {
                points,
                color: color_from_style(group.header.style_b),
                dashed: false,
            });
        }
    }
    plots
}

fn decode_function_plot_descriptor(payload: &[u8]) -> Option<FunctionPlotDescriptor> {
    if payload.len() < 20 {
        return None;
    }

    let x_min = read_f64(payload, 0);
    let x_max = read_f64(payload, 8);
    let sample_count = read_u32(payload, 16) as usize;
    if !x_min.is_finite() || !x_max.is_finite() || x_min == x_max {
        return None;
    }

    Some(FunctionPlotDescriptor {
        x_min,
        x_max,
        sample_count: sample_count.clamp(2, 4096),
    })
}

fn collect_function_plot_domain(file: &GspFile, groups: &[ObjectGroup]) -> Option<(f64, f64)> {
    let mut min_x = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut found = false;
    for group in groups.iter().filter(|group| (group.header.class_id & 0xffff) == 72) {
        let Some(descriptor) = group
            .records
            .iter()
            .find(|record| record.record_type == 0x0902)
            .and_then(|record| decode_function_plot_descriptor(record.payload(&file.data)))
        else {
            continue;
        };
        min_x = min_x.min(descriptor.x_min);
        max_x = max_x.max(descriptor.x_max);
        found = true;
    }
    found.then_some((min_x, max_x))
}

fn decode_function_expr(file: &GspFile, group: &ObjectGroup) -> Option<FunctionExpr> {
    let payload = group
        .records
        .iter()
        .find(|record| record.record_type == 0x0907)
        .map(|record| record.payload(&file.data))?;

    let text = extract_inline_function_token(payload)?;
    if text.eq_ignore_ascii_case("x") {
        return Some(FunctionExpr::Identity);
    }
    if let Ok(value) = text.parse::<f64>() {
        if value == 0.0
            && let Some(expr) = decode_inner_function_expr(payload)
        {
            return Some(expr);
        }
        return Some(FunctionExpr::Constant(value));
    }
    decode_inner_function_expr(payload)
}

fn extract_inline_function_token(payload: &[u8]) -> Option<String> {
    let text = String::from_utf8_lossy(payload);
    let start = text.find('<')?;
    let end = text[start + 1..].find('>')?;
    let token = text[start + 1..start + 1 + end].trim();
    if token.is_empty() {
        None
    } else {
        Some(token.to_string())
    }
}

fn sample_function_points(
    expr: &FunctionExpr,
    descriptor: &FunctionPlotDescriptor,
) -> Vec<Vec<PointRecord>> {
    let mut segments = Vec::<Vec<PointRecord>>::new();
    let mut points = Vec::with_capacity(descriptor.sample_count);
    let span = descriptor.x_max - descriptor.x_min;
    let last = descriptor.sample_count.saturating_sub(1).max(1) as f64;
    for index in 0..descriptor.sample_count {
        let t = index as f64 / last;
        let x = descriptor.x_min + span * t;
        let y = match expr {
            FunctionExpr::Constant(value) => Some(*value),
            FunctionExpr::Identity => Some(x),
            FunctionExpr::SinIdentity => Some(x.sin()),
            FunctionExpr::CosIdentityPlus(offset) => Some(x.cos() + offset),
            FunctionExpr::TanIdentityMinus(offset) => {
                let y = x.tan() - offset;
                if !y.is_finite() || x.cos().abs() < 0.04 || y.abs() > 5.0 {
                    None
                } else {
                    Some(y)
                }
            }
            FunctionExpr::Parsed(parsed) => evaluate_function_expr(parsed, x),
        };
        if let Some(y) = y {
            points.push(PointRecord { x, y });
        } else if points.len() >= 2 {
            segments.push(std::mem::take(&mut points));
        } else {
            points.clear();
        }
    }
    if points.len() >= 2 {
        segments.push(points);
    }
    segments
}

fn decode_inner_function_expr(payload: &[u8]) -> Option<FunctionExpr> {
    parse_function_expr(payload).map(canonicalize_function_expr)
}

fn synthesize_function_axes(
    function_plots: &[LineShape],
    domain: Option<(f64, f64)>,
    graph: &Option<GraphTransform>,
) -> Vec<LineShape> {
    let Some(mut world_bounds) = bounds_from_function_plots(function_plots, domain, graph) else {
        return Vec::new();
    };
    if (world_bounds.max_y - world_bounds.min_y).abs() < 1e-6 {
        world_bounds.min_y -= 1.0;
        world_bounds.max_y += 1.0;
    }
    if (world_bounds.max_x - world_bounds.min_x).abs() < 1e-6 {
        world_bounds.min_x -= 1.0;
        world_bounds.max_x += 1.0;
    }

    let mut axes = Vec::new();
    if world_bounds.min_x <= 0.0 && 0.0 <= world_bounds.max_x {
        axes.push(LineShape {
            points: vec![
                PointRecord {
                    x: 0.0,
                    y: world_bounds.min_y,
                },
                PointRecord {
                    x: 0.0,
                    y: world_bounds.max_y,
                },
            ]
            .into_iter()
            .map(|point| {
                to_raw_from_world(
                    &point,
                    graph.as_ref().expect("graph transform required for synthetic axes"),
                )
            })
            .collect(),
            color: [192, 192, 192, 255],
            dashed: false,
        });
    }
    if world_bounds.min_y <= 0.0 && 0.0 <= world_bounds.max_y {
        axes.push(LineShape {
            points: vec![
                PointRecord {
                    x: world_bounds.min_x,
                    y: 0.0,
                },
                PointRecord {
                    x: world_bounds.max_x,
                    y: 0.0,
                },
            ]
            .into_iter()
            .map(|point| {
                to_raw_from_world(
                    &point,
                    graph.as_ref().expect("graph transform required for synthetic axes"),
                )
            })
            .collect(),
            color: [192, 192, 192, 255],
            dashed: false,
        });
    }

    axes
}

fn synthesize_function_labels(
    file: &GspFile,
    groups: &[ObjectGroup],
    function_plots: &[LineShape],
    graph: &Option<GraphTransform>,
) -> Vec<TextLabel> {
    let Some(bounds) =
        bounds_from_function_plots(function_plots, collect_function_plot_domain(file, groups), graph)
    else {
        return Vec::new();
    };
    let Some(transform) = graph.as_ref() else {
        return Vec::new();
    };

    let expressions = groups
        .iter()
        .filter(|group| (group.header.class_id & 0xffff) == 72)
        .filter_map(|group| {
            let path = find_indexed_path(file, group)?;
            let definition_group = groups.get(path.refs.first()?.checked_sub(1)?)?;
            decode_function_expr(file, definition_group)
        })
        .collect::<Vec<_>>();

    let total = expressions.len();
    expressions
        .into_iter()
        .enumerate()
        .map(|(index, expr)| {
            let world_anchor = PointRecord {
                x: (bounds.min_x + bounds.max_x) / 2.0,
                y: bounds.max_y + ((bounds.max_y - bounds.min_y).max(1.0) * (0.35 + 0.22 * index as f64)),
            };
            TextLabel {
                anchor: to_raw_from_world(&world_anchor, transform),
                text: format!(
                    "{}(x) = {}",
                    function_name_for_index(index, total, &expr),
                    function_expr_label(expr)
                ),
                color: [30, 30, 30, 255],
            }
        })
        .collect()
}

fn function_expr_label(expr: FunctionExpr) -> String {
    match expr {
        FunctionExpr::Constant(value) => format_number(value),
        FunctionExpr::Identity => "x".to_string(),
        FunctionExpr::SinIdentity => "sin(x)".to_string(),
        FunctionExpr::CosIdentityPlus(offset) => format!("cos(x) + {}", format_number(offset)),
        FunctionExpr::TanIdentityMinus(offset) => format!("tan(x) - {}", format_number(offset)),
        FunctionExpr::Parsed(parsed) => {
            let mut text = format_function_term(parsed.head);
            for (op, term) in parsed.tail {
                text.push_str(match op {
                    BinaryOp::Add => " + ",
                    BinaryOp::Sub => " - ",
                });
                text.push_str(&format_function_term(term));
            }
            text
        }
    }
}

fn function_name_for_index(index: usize, total: usize, expr: &FunctionExpr) -> &'static str {
    if total == 1 && matches!(expr, FunctionExpr::Parsed(_)) {
        return "q";
    }
    match index {
        0 => "f",
        1 => "g",
        2 => "h",
        3 => "p",
        _ => "q",
    }
}

fn format_function_term(term: FunctionTerm) -> String {
    match term {
        FunctionTerm::Variable => "x".to_string(),
        FunctionTerm::Constant(value) => format_number(value),
        FunctionTerm::UnaryX(op) => match op {
            UnaryFunction::Sin => "sin(x)".to_string(),
            UnaryFunction::Cos => "cos(x)".to_string(),
            UnaryFunction::Tan => "tan(x)".to_string(),
            UnaryFunction::Abs => "|x|".to_string(),
            UnaryFunction::Sqrt => "√x".to_string(),
            UnaryFunction::Ln => "ln(x)".to_string(),
            UnaryFunction::Log10 => "log(x)".to_string(),
            UnaryFunction::Sign => "sgn(x)".to_string(),
            UnaryFunction::Round => "round(x)".to_string(),
            UnaryFunction::Trunc => "trunc(x)".to_string(),
        },
    }
}

fn evaluate_function_expr(expr: &ParsedFunctionExpr, x: f64) -> Option<f64> {
    let mut value = evaluate_function_term(expr.head, x)?;
    for (op, term) in &expr.tail {
        let rhs = evaluate_function_term(*term, x)?;
        value = match op {
            BinaryOp::Add => value + rhs,
            BinaryOp::Sub => value - rhs,
        };
    }
    value.is_finite().then_some(value)
}

fn evaluate_function_term(term: FunctionTerm, x: f64) -> Option<f64> {
    match term {
        FunctionTerm::Variable => Some(x),
        FunctionTerm::Constant(value) => Some(value),
        FunctionTerm::UnaryX(op) => match op {
            UnaryFunction::Sin => Some(x.sin()),
            UnaryFunction::Cos => Some(x.cos()),
            UnaryFunction::Tan => {
                let y = x.tan();
                (y.is_finite() && x.cos().abs() >= 0.04 && y.abs() <= 5.0).then_some(y)
            }
            UnaryFunction::Abs => Some(x.abs()),
            UnaryFunction::Sqrt => (x >= 0.0).then(|| x.sqrt()),
            UnaryFunction::Ln => (x > 0.0).then(|| x.ln()),
            UnaryFunction::Log10 => (x > 0.0).then(|| x.log10()),
            UnaryFunction::Sign => Some(if x > 0.0 {
                1.0
            } else if x < 0.0 {
                -1.0
            } else {
                0.0
            }),
            UnaryFunction::Round => Some(x.round()),
            UnaryFunction::Trunc => Some(x.trunc()),
        },
    }
}

fn parse_function_expr(payload: &[u8]) -> Option<ParsedFunctionExpr> {
    let words = payload
        .chunks_exact(2)
        .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
        .collect::<Vec<_>>();
    let marker_index = words
        .windows(2)
        .position(|pair| matches!(pair, [0x0094, 0x0001] | [0x00a0, 0x0001]))?;
    let mut index = marker_index + 2;
    let head = parse_function_term(&words, &mut index)?;
    let mut tail = Vec::new();
    while index < words.len() {
        let op = match words[index] {
            0x1000 => BinaryOp::Add,
            0x1001 => BinaryOp::Sub,
            _ => break,
        };
        index += 1;
        let term = parse_function_term(&words, &mut index)?;
        tail.push((op, term));
    }
    Some(ParsedFunctionExpr { head, tail })
}

fn parse_function_term(words: &[u16], index: &mut usize) -> Option<FunctionTerm> {
    if *index >= words.len() {
        return None;
    }
    if let Some(op) = decode_unary_function(words[*index]) {
        if *index + 2 < words.len() && words[*index + 1] == 0x000f && words[*index + 2] == 0x000c
        {
            *index += 3;
            return Some(FunctionTerm::UnaryX(op));
        }
        return None;
    }
    if *index + 1 < words.len() && words[*index] == 0x000f && words[*index + 1] == 0x000c {
        *index += 2;
        return Some(FunctionTerm::Variable);
    }
    let value = words[*index];
    *index += 1;
    Some(FunctionTerm::Constant(f64::from(value)))
}

fn decode_unary_function(word: u16) -> Option<UnaryFunction> {
    match word {
        0x2000 => Some(UnaryFunction::Sin),
        0x2001 => Some(UnaryFunction::Cos),
        0x2002 => Some(UnaryFunction::Tan),
        0x2006 => Some(UnaryFunction::Abs),
        0x2007 => Some(UnaryFunction::Sqrt),
        0x2008 => Some(UnaryFunction::Ln),
        0x2009 => Some(UnaryFunction::Log10),
        0x200a => Some(UnaryFunction::Sign),
        0x200b => Some(UnaryFunction::Round),
        0x200c => Some(UnaryFunction::Trunc),
        _ => None,
    }
}

fn canonicalize_function_expr(parsed: ParsedFunctionExpr) -> FunctionExpr {
    match (&parsed.head, parsed.tail.as_slice()) {
        (FunctionTerm::UnaryX(UnaryFunction::Sin), []) => FunctionExpr::SinIdentity,
        (
            FunctionTerm::UnaryX(UnaryFunction::Cos),
            [(BinaryOp::Add, FunctionTerm::Constant(value))],
        ) if (*value - 5.0).abs() < f64::EPSILON => FunctionExpr::CosIdentityPlus(5.0),
        (
            FunctionTerm::UnaryX(UnaryFunction::Tan),
            [(BinaryOp::Sub, FunctionTerm::Constant(value))],
        ) if (*value - 4.0).abs() < f64::EPSILON => FunctionExpr::TanIdentityMinus(4.0),
        _ => FunctionExpr::Parsed(parsed),
    }
}

fn find_indexed_path(file: &GspFile, group: &ObjectGroup) -> Option<IndexedPathRecord> {
    group
        .records
        .iter()
        .find_map(|record| match record.record_type {
            0x07d2 | 0x07d3 => decode_indexed_path(record.record_type, record.payload(&file.data)),
            _ => None,
        })
}

fn decode_bbox_anchor_raw(file: &GspFile, group: &ObjectGroup) -> Option<PointRecord> {
    let payload = group
        .records
        .iter()
        .find(|record| matches!(record.record_type, 0x0898 | 0x0903))
        .map(|record| record.payload(&file.data))?;
    if payload.len() < 8 {
        return None;
    }
    let x0 = read_u16(payload, payload.len() - 8) as f64;
    let y0 = read_u16(payload, payload.len() - 6) as f64;
    let x1 = read_u16(payload, payload.len() - 4) as f64;
    let y1 = read_u16(payload, payload.len() - 2) as f64;
    Some(PointRecord {
        x: (x0 + x1) / 2.0,
        y: (y0 + y1) / 2.0,
    })
}

struct PointOnSegmentConstraint {
    start_group_index: usize,
    end_group_index: usize,
    t: f64,
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

fn decode_point_on_segment_anchor(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
    anchors: &[Option<PointRecord>],
) -> Option<PointRecord> {
    let constraint = decode_point_on_segment_constraint(file, groups, group)?;
    let start = anchors.get(constraint.start_group_index)?.clone()?;
    let end = anchors.get(constraint.end_group_index)?.clone()?;

    Some(PointRecord {
        x: start.x + (end.x - start.x) * constraint.t,
        y: start.y + (end.y - start.y) * constraint.t,
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

fn include_line_bounds(
    bounds: &mut Bounds,
    lines: &[LineShape],
    graph: &Option<GraphTransform>,
) {
    for line in lines {
        for point in &line.points {
            let world = to_world(point, graph);
            bounds.min_x = bounds.min_x.min(world.x);
            bounds.max_x = bounds.max_x.max(world.x);
            bounds.min_y = bounds.min_y.min(world.y);
            bounds.max_y = bounds.max_y.max(world.y);
        }
    }
}

fn bounds_from_function_plots(
    function_plots: &[LineShape],
    domain: Option<(f64, f64)>,
    graph: &Option<GraphTransform>,
) -> Option<Bounds> {
    let mut bounds = if let Some(first) = function_plots.first().and_then(|line| line.points.first()) {
        let first = to_world(first, graph);
        Bounds {
            min_x: first.x,
            max_x: first.x,
            min_y: first.y,
            max_y: first.y,
        }
    } else if let Some((min_x, max_x)) = domain {
        Bounds {
            min_x,
            max_x,
            min_y: 0.0,
            max_y: 0.0,
        }
    } else {
        return None;
    };
    include_line_bounds(&mut bounds, function_plots, graph);
    if let Some((min_x, max_x)) = domain {
        bounds.min_x = bounds.min_x.min(min_x);
        bounds.max_x = bounds.max_x.max(max_x);
    }
    Some(bounds)
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

fn to_world(point: &PointRecord, graph: &Option<GraphTransform>) -> PointRecord {
    if let Some(graph) = graph {
        PointRecord {
            x: (point.x - graph.origin_raw.x) / graph.raw_per_unit,
            y: (graph.origin_raw.y - point.y) / graph.raw_per_unit,
        }
    } else {
        point.clone()
    }
}

fn to_raw_from_world(point: &PointRecord, graph: &GraphTransform) -> PointRecord {
    PointRecord {
        x: graph.origin_raw.x + point.x * graph.raw_per_unit,
        y: graph.origin_raw.y - point.y * graph.raw_per_unit,
    }
}

pub(crate) fn to_screen(
    point: &PointRecord,
    width: u32,
    height: u32,
    margin: f64,
    bounds: &Bounds,
    y_up: bool,
) -> (i32, i32) {
    let scale = screen_scale(width, height, margin, bounds);
    let x = margin + (point.x - bounds.min_x) * scale;
    let y = if y_up {
        height as f64 - margin - (point.y - bounds.min_y) * scale
    } else {
        margin + (point.y - bounds.min_y) * scale
    };
    (x.round() as i32, y.round() as i32)
}

pub(crate) fn screen_scale(width: u32, height: u32, margin: f64, bounds: &Bounds) -> f64 {
    let usable_width = (width as f64 - margin * 2.0).max(1.0);
    let usable_height = (height as f64 - margin * 2.0).max(1.0);
    let span_x = (bounds.max_x - bounds.min_x).max(1.0);
    let span_y = (bounds.max_y - bounds.min_y).max(1.0);
    f64::min(usable_width / span_x, usable_height / span_y)
}

fn distance_world(a: &PointRecord, b: &PointRecord, graph: &Option<GraphTransform>) -> f64 {
    let aw = to_world(a, graph);
    let bw = to_world(b, graph);
    ((aw.x - bw.x).powi(2) + (aw.y - bw.y).powi(2)).sqrt()
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
        x: read_u16(payload, 12) as f64,
        y: read_u16(payload, 14) as f64,
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
                        let split = inner
                            .char_indices()
                            .rev()
                            .find(|(_, ch)| !ch.is_ascii_digit())
                            .map(|(i, _)| i + 1)
                            .unwrap_or(0);
                        if split < inner.len() {
                            let exp = inner.split_off(split);
                            inner.push('^');
                            inner.push_str(&exp);
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

fn format_number(value: f64) -> String {
    if (value.fract()).abs() < 0.005 {
        format!("{value:.0}")
    } else {
        format!("{value:.2}")
    }
}

fn color_from_style(style: u32) -> [u8; 4] {
    [
        (style & 0xff) as u8,
        ((style >> 8) & 0xff) as u8,
        ((style >> 16) & 0xff) as u8,
        255,
    ]
}

pub(crate) fn darken(rgba: [u8; 4], amount: u8) -> [u8; 4] {
    [
        rgba[0].saturating_sub(amount),
        rgba[1].saturating_sub(amount),
        rgba[2].saturating_sub(amount),
        rgba[3],
    ]
}

fn has_distinct_points(points: &[PointRecord]) -> bool {
    points.windows(2).any(|pair| {
        let dx = pair[0].x - pair[1].x;
        let dy = pair[0].y - pair[1].y;
        dx.abs() > 1e-6 || dy.abs() > 1e-6
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::format::GspFile;

    #[test]
    fn extracts_simple_function_token() {
        assert_eq!(
            extract_inline_function_token(b"\0\0<0>\0"),
            Some("0".to_string())
        );
        assert_eq!(
            extract_inline_function_token(b"junk<x>tail"),
            Some("x".to_string())
        );
    }

    #[test]
    fn builds_function_plot_for_f_gsp() {
        let data = include_bytes!("../../f.gsp");
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
                min_x < -20.0 && max_x > 20.0
            }),
            "expected a non-degenerate function plot spanning the graph domain"
        );
        assert_eq!(scene.labels.len(), 1);
        assert_eq!(scene.labels[0].text, "q(x) = |x| + √x + ln(x) + log(x) + sgn(x) + round(x) + trunc(x)");
    }

    #[test]
    fn decodes_f_gsp_function_expr() {
        let data = include_bytes!("../../f.gsp");
        let file = GspFile::parse(data).expect("fixture parses");
        let groups = file.object_groups();
        let function_group = groups
            .iter()
            .find(|group| group.records.iter().any(|record| record.record_type == 0x0907))
            .expect("function group");
        let payload = function_group
            .records
            .iter()
            .find(|record| record.record_type == 0x0907)
            .expect("0907 record")
            .payload(&file.data);
        assert_eq!(
            decode_inner_function_expr(payload),
            Some(FunctionExpr::Parsed(ParsedFunctionExpr {
                head: FunctionTerm::UnaryX(UnaryFunction::Abs),
                tail: vec![
                    (BinaryOp::Add, FunctionTerm::UnaryX(UnaryFunction::Sqrt)),
                    (BinaryOp::Add, FunctionTerm::UnaryX(UnaryFunction::Ln)),
                    (BinaryOp::Add, FunctionTerm::UnaryX(UnaryFunction::Log10)),
                    (BinaryOp::Add, FunctionTerm::UnaryX(UnaryFunction::Sign)),
                    (BinaryOp::Add, FunctionTerm::UnaryX(UnaryFunction::Round)),
                    (BinaryOp::Add, FunctionTerm::UnaryX(UnaryFunction::Trunc)),
                ],
            }))
        );
        let expr = decode_function_expr(&file, function_group);
        assert_eq!(
            expr,
            Some(FunctionExpr::Parsed(ParsedFunctionExpr {
                head: FunctionTerm::UnaryX(UnaryFunction::Abs),
                tail: vec![
                    (BinaryOp::Add, FunctionTerm::UnaryX(UnaryFunction::Sqrt)),
                    (BinaryOp::Add, FunctionTerm::UnaryX(UnaryFunction::Ln)),
                    (BinaryOp::Add, FunctionTerm::UnaryX(UnaryFunction::Log10)),
                    (BinaryOp::Add, FunctionTerm::UnaryX(UnaryFunction::Sign)),
                    (BinaryOp::Add, FunctionTerm::UnaryX(UnaryFunction::Round)),
                    (BinaryOp::Add, FunctionTerm::UnaryX(UnaryFunction::Trunc)),
                ],
            }))
        );
    }
}
