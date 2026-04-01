use super::decode::{decode_measurement_value, find_indexed_path};
use super::*;
use crate::runtime::functions::{FunctionExpr, decode_function_expr};
use crate::runtime::geometry::read_f32_unaligned;

pub(super) fn collect_saved_viewport(file: &GspFile, groups: &[ObjectGroup]) -> Option<Bounds> {
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

pub(super) fn detect_graph_transform(
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

pub(super) fn has_graph_classes(groups: &[ObjectGroup]) -> bool {
    groups
        .iter()
        .any(|group| matches!(group.header.class_id & 0xffff, 40 | 52 | 54 | 58 | 61))
}

pub(super) fn collect_bounds(
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
        if matches!(
            label.binding,
            Some(
                TextLabelBinding::ParameterValue { .. } | TextLabelBinding::ExpressionValue { .. }
            )
        ) {
            continue;
        }
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

pub(super) fn dedupe_line_shapes(lines: Vec<LineShape>) -> Vec<LineShape> {
    let mut deduped: Vec<LineShape> = Vec::new();
    'outer: for line in lines {
        for existing in &deduped {
            if line.points.len() != existing.points.len() {
                continue;
            }
            if line
                .points
                .iter()
                .zip(existing.points.iter())
                .all(|(left, right)| {
                    (left.x - right.x).abs() < 1e-6 && (left.y - right.y).abs() < 1e-6
                })
                && line.color == existing.color
                && line.dashed == existing.dashed
            {
                continue 'outer;
            }
        }
        deduped.push(line);
    }
    deduped
}

pub(super) fn bounds_within(container: &Bounds, content: &Bounds) -> bool {
    const TOLERANCE: f64 = 1e-3;
    container.min_x <= content.min_x + TOLERANCE
        && container.max_x >= content.max_x - TOLERANCE
        && container.min_y <= content.min_y + TOLERANCE
        && container.max_y >= content.max_y - TOLERANCE
}

pub(super) fn expand_bounds(bounds: &mut Bounds) {
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
