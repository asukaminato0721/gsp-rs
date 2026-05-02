use super::decode::{decode_label_name, decode_measurement_value, find_indexed_path};
use super::shapes::{ArcShape, CircleShape};
use crate::format::{GroupKind, GspFile, ObjectGroup, PointRecord, read_u16};
use crate::runtime::functions::{evaluate_expr_with_parameters, try_decode_function_expr};
use crate::runtime::geometry::{
    Bounds, GraphTransform, arc_sample_points, read_f32_unaligned, to_world,
};
use crate::runtime::scene::{LineShape, PolygonShape, TextLabel, TextLabelBinding};

pub(super) fn collect_saved_viewport(file: &GspFile, groups: &[ObjectGroup]) -> Option<Bounds> {
    let (min_x, max_x) = collect_graph_window_hint(file, groups, "x")?;
    let (min_y, max_y) = collect_graph_window_hint(file, groups, "y")
        .or_else(|| collect_graph_window_hint(file, groups, "x"))?;
    Some(Bounds {
        min_x,
        max_x,
        min_y,
        max_y,
    })
}

pub(super) fn collect_document_canvas_bounds(file: &GspFile) -> Option<Bounds> {
    let header = file
        .records
        .first()
        .filter(|record| record.record_type == 0x0384)?;
    let payload = header.payload(&file.data);
    if payload.len() < 22 {
        return None;
    }

    let width = f64::from(read_u16(payload, 18));
    let height = f64::from(read_u16(payload, 20));
    if width <= 0.0 || height <= 0.0 {
        return None;
    }

    Some(Bounds {
        min_x: 0.0,
        max_x: width,
        min_y: 0.0,
        max_y: height,
    })
}

fn collect_graph_window_hint(
    file: &GspFile,
    groups: &[ObjectGroup],
    axis_name: &str,
) -> Option<(f64, f64)> {
    groups
        .iter()
        .filter(|group| {
            matches!(
                group.header.kind(),
                crate::format::GroupKind::MeasurementLine
                    | crate::format::GroupKind::GraphMeasurementSegment
            )
        })
        .find_map(|group| {
            if decode_label_name(file, group)
                .as_deref()
                .map(str::to_ascii_lowercase)
                .as_deref()
                != Some(axis_name)
            {
                return None;
            }
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

pub(super) fn detect_graph_transform(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
) -> Option<GraphTransform> {
    detect_explicit_axis_graph_transform(file, groups, anchors)
        .or_else(|| detect_calibration_graph_transform(file, groups, anchors))
}

fn detect_calibration_graph_transform(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
) -> Option<GraphTransform> {
    let raw_per_unit = groups
        .iter()
        .filter(|group| group.header.kind().is_graph_calibration())
        .find_map(|group| {
            let record = group
                .records
                .iter()
                .find(|record| record.record_type == 0x07d3 && record.length == 12)?;
            decode_measurement_value(record.payload(&file.data))
        })?;

    let origin_raw = groups.iter().find_map(|group| {
        if !group.header.kind().is_graph_calibration() {
            return None;
        }
        let path = find_indexed_path(file, group)?;
        path.refs
            .iter()
            .find_map(|object_ref| anchors.get(object_ref.saturating_sub(1)).cloned().flatten())
    })?;

    Some(GraphTransform {
        origin_raw,
        raw_per_unit,
    })
}

fn detect_explicit_axis_graph_transform(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
) -> Option<GraphTransform> {
    groups
        .iter()
        .filter(|group| group.header.kind() == crate::format::GroupKind::AxisLine)
        .find_map(|axis_group| {
            let axis_path = find_indexed_path(file, axis_group)?;
            let x_measure_group = groups.get(axis_path.refs.first()?.checked_sub(1)?)?;
            let x_measure_path = find_indexed_path(file, x_measure_group)?;
            let origin_raw = anchors
                .get(x_measure_path.refs.first()?.checked_sub(1)?)?
                .clone()?;
            let unit_expr_group = groups.get(x_measure_path.refs.get(1)?.checked_sub(1)?)?;
            let raw_per_unit = try_decode_function_expr(file, groups, unit_expr_group)
                .ok()
                .and_then(|unit_expr| {
                    evaluate_expr_with_parameters(
                        &unit_expr,
                        0.0,
                        &std::collections::BTreeMap::new(),
                    )
                })
                .map(f64::abs)
                .filter(|value| *value > 1e-9)
                .or_else(|| {
                    unit_expr_group
                        .records
                        .iter()
                        .find(|record| record.record_type == 0x07d3 && record.length == 12)
                        .and_then(|record| decode_measurement_value(record.payload(&file.data)))
                })
                .or_else(|| {
                    let unit_raw = anchors
                        .get(x_measure_path.refs.get(1)?.checked_sub(1)?)?
                        .clone()?;
                    let distance = (unit_raw.x - origin_raw.x).hypot(unit_raw.y - origin_raw.y);
                    (distance > 1e-9).then_some(distance)
                })?;
            (raw_per_unit > 1e-9).then_some(GraphTransform {
                origin_raw,
                raw_per_unit,
            })
        })
}

pub(super) fn has_coordinate_transform_consumers(groups: &[ObjectGroup]) -> bool {
    groups.iter().any(|group| {
        matches!(
            group.header.kind(),
            GroupKind::CoordinateXValue
                | GroupKind::CoordinateYValue
                | GroupKind::CoordinateReadoutLabel
        ) || group.header.kind().is_coordinate_object()
    })
}

pub(super) fn has_graph_classes(groups: &[ObjectGroup]) -> bool {
    let mut has_calibration = false;
    let mut has_graph_expression = false;

    for group in groups {
        match group.header.kind() {
            GroupKind::FunctionPlot
            | GroupKind::LegacyFunctionPlot
            | GroupKind::ParametricFunctionPlot => return true,
            GroupKind::GraphObject40
            | GroupKind::MeasurementLine
            | GroupKind::GraphMeasurementSegment
            | GroupKind::AxisLine
                if !group.header.is_hidden() =>
            {
                return true;
            }
            kind if kind.is_coordinate_object() => {}
            kind if kind.is_graph_calibration() => has_calibration = true,
            GroupKind::CoordinateXValue
            | GroupKind::CoordinateYValue
            | GroupKind::GraphViewHelper
                if !group.header.is_hidden() =>
            {
                has_graph_expression = true;
            }
            _ => {}
        }
    }

    has_calibration && has_graph_expression
}

pub(super) struct BoundsInputs<'a> {
    pub(super) segments: &'a [LineShape],
    pub(super) measurements: &'a [LineShape],
    pub(super) axes: &'a [LineShape],
    pub(super) polygons: &'a [PolygonShape],
    pub(super) circles: &'a [CircleShape],
    pub(super) arcs: &'a [ArcShape],
    pub(super) labels: &'a [TextLabel],
    pub(super) points_only: &'a [PointRecord],
}

pub(super) fn collect_bounds(graph: &Option<GraphTransform>, inputs: BoundsInputs<'_>) -> Bounds {
    let mut points = Vec::<PointRecord>::new();
    for shape in inputs
        .segments
        .iter()
        .chain(inputs.measurements.iter())
        .chain(inputs.axes.iter())
    {
        points.extend(shape.points.iter().cloned());
    }
    for shape in inputs.polygons {
        points.extend(shape.points.iter().cloned());
    }
    for circle in inputs.circles {
        points.push(circle.center.clone());
        points.push(circle.radius_point.clone());
        let radius = ((circle.radius_point.x - circle.center.x).powi(2)
            + (circle.radius_point.y - circle.center.y).powi(2))
        .sqrt();
        if radius.is_finite() && radius > 1e-9 {
            points.push(PointRecord {
                x: circle.center.x - radius,
                y: circle.center.y,
            });
            points.push(PointRecord {
                x: circle.center.x + radius,
                y: circle.center.y,
            });
            points.push(PointRecord {
                x: circle.center.x,
                y: circle.center.y - radius,
            });
            points.push(PointRecord {
                x: circle.center.x,
                y: circle.center.y + radius,
            });
        }
    }
    for arc in inputs.arcs {
        if let Some(samples) = arc_sample_points(&arc.points[0], &arc.points[1], &arc.points[2]) {
            points.extend(samples);
        } else {
            points.extend(arc.points.iter().cloned());
        }
    }
    for label in inputs.labels {
        if !label.visible || label.screen_space {
            continue;
        }
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
    points.extend(inputs.points_only.iter().cloned());
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
