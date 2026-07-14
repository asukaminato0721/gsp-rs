use std::collections::BTreeMap;

use crate::{
    FunctionExpr, LineKind, Point, evaluate_expr, line_circle_intersections,
    line_line_intersection, point_on_circle_arc, point_on_three_point_arc,
    point_on_three_point_arc_complement,
};

const POINT_EPSILON: f64 = 1e-6;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum PlotMode {
    Cartesian,
    Polar,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CoordinateTraceMode {
    Horizontal,
    Vertical,
    TwoDimensional,
}

pub fn segment_marker_points(
    start: Point,
    end: Point,
    t: f64,
    marker_class: u32,
) -> Option<Vec<Point>> {
    let dx = end.x - start.x;
    let dy = end.y - start.y;
    let length = dx.hypot(dy);
    if length <= 1e-9 {
        return None;
    }
    let tangent = Point {
        x: dx / length,
        y: dy / length,
    };
    let normal = Point {
        x: -tangent.y,
        y: tangent.x,
    };
    let center = Point {
        x: start.x + dx * t.clamp(0.0, 1.0),
        y: start.y + dy * t.clamp(0.0, 1.0),
    };
    let half_length = (length * 0.06).clamp(5.0, 10.0);
    let spacing = (length * 0.05).clamp(6.0, 11.0);
    let center_offset = marker_class.saturating_sub(1) as f64 * -0.5 * spacing;
    let slash_center = Point {
        x: center.x + tangent.x * center_offset,
        y: center.y + tangent.y * center_offset,
    };
    Some(vec![
        Point {
            x: slash_center.x - normal.x * half_length,
            y: slash_center.y - normal.y * half_length,
        },
        Point {
            x: slash_center.x + normal.x * half_length,
            y: slash_center.y + normal.y * half_length,
        },
    ])
}

/// Samples a scalar expression while preserving invalid samples as segment breaks.
pub fn sample_expression(
    expr: &FunctionExpr,
    parameters: &BTreeMap<String, f64>,
    x_min: f64,
    x_max: f64,
    sample_count: usize,
    plot_mode: PlotMode,
) -> Vec<Option<Point>> {
    if sample_count == 0 || !x_min.is_finite() || !x_max.is_finite() {
        return Vec::new();
    }
    let last = sample_count.saturating_sub(1).max(1) as f64;
    (0..sample_count)
        .map(|index| {
            let t = index as f64 / last;
            let x = x_min + (x_max - x_min) * t;
            let value = evaluate_expr(expr, x, parameters)?;
            Some(match plot_mode {
                PlotMode::Cartesian => Point { x, y: value },
                PlotMode::Polar => Point {
                    x: value * x.cos(),
                    y: value * x.sin(),
                },
            })
        })
        .collect()
}

pub fn sample_parametric_curve(
    x_expr: &FunctionExpr,
    y_expr: &FunctionExpr,
    x_parameters: &BTreeMap<String, f64>,
    y_parameters: &BTreeMap<String, f64>,
    value_min: f64,
    value_max: f64,
    sample_count: usize,
) -> Vec<Point> {
    if sample_count == 0 || !value_min.is_finite() || !value_max.is_finite() {
        return Vec::new();
    }
    let last = sample_count.saturating_sub(1).max(1) as f64;
    (0..sample_count)
        .filter_map(|index| {
            let t = index as f64 / last;
            let value = value_min + (value_max - value_min) * t;
            Some(Point {
                x: evaluate_expr(x_expr, value, x_parameters)?,
                y: evaluate_expr(y_expr, value, y_parameters)?,
            })
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
pub fn sample_coordinate_trace(
    x_expr: &FunctionExpr,
    y_expr: Option<&FunctionExpr>,
    x_parameters: &BTreeMap<String, f64>,
    y_parameters: Option<&BTreeMap<String, f64>>,
    x_parameter_name: Option<&str>,
    y_parameter_name: Option<&str>,
    source: Point,
    value_min: f64,
    value_max: f64,
    sample_count: usize,
    use_midpoints: bool,
    mode: CoordinateTraceMode,
) -> Vec<Point> {
    if sample_count == 0 {
        return Vec::new();
    }
    let last = sample_count.saturating_sub(1).max(1) as f64;
    (0..sample_count)
        .filter_map(|index| {
            let t = if use_midpoints {
                (index as f64 + 0.5) / sample_count as f64
            } else {
                index as f64 / last
            };
            let value = value_min + (value_max - value_min) * t;
            let mut x_parameters = x_parameters.clone();
            if let Some(name) = x_parameter_name {
                x_parameters.insert(name.into(), value);
            }
            let x = evaluate_expr(x_expr, 0.0, &x_parameters)?;
            Some(match mode {
                CoordinateTraceMode::Horizontal => Point {
                    x: source.x + x,
                    y: source.y,
                },
                CoordinateTraceMode::Vertical => Point {
                    x: source.x,
                    y: source.y + x,
                },
                CoordinateTraceMode::TwoDimensional => {
                    let y_expr = y_expr?;
                    let mut y_parameters = y_parameters?.clone();
                    if let Some(name) = y_parameter_name {
                        y_parameters.insert(name.into(), value);
                    }
                    Point {
                        x: source.x + x,
                        y: source.y + evaluate_expr(y_expr, 0.0, &y_parameters)?,
                    }
                }
            })
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
pub fn sample_custom_transform_trace(
    distance_expr: &FunctionExpr,
    angle_expr: &FunctionExpr,
    distance_parameters: &BTreeMap<String, f64>,
    angle_parameters: &BTreeMap<String, f64>,
    distance_parameter_names: &[String],
    angle_parameter_names: &[String],
    origin: Point,
    axis_end: Point,
    value_min: f64,
    value_max: f64,
    trace_max: f64,
    sample_count: usize,
    distance_scale: f64,
    angle_degrees_scale: f64,
) -> Vec<Point> {
    if sample_count == 0 {
        return Vec::new();
    }
    let last = sample_count.saturating_sub(1).max(1) as f64;
    let max_value = value_max.min(trace_max).max(value_min);
    let base_angle = (-(axis_end.y - origin.y))
        .atan2(axis_end.x - origin.x)
        .to_degrees();
    (0..sample_count)
        .filter_map(|index| {
            let value = value_min + (max_value - value_min) * (index as f64 / last);
            let mut distance_parameters = distance_parameters.clone();
            let mut angle_parameters = angle_parameters.clone();
            for name in distance_parameter_names {
                distance_parameters.insert(name.clone(), value);
            }
            for name in angle_parameter_names {
                angle_parameters.insert(name.clone(), value);
            }
            let distance =
                evaluate_expr(distance_expr, value, &distance_parameters)? * distance_scale;
            let angle = base_angle
                + evaluate_expr(angle_expr, value, &angle_parameters)? * angle_degrees_scale;
            let radians = angle.to_radians();
            Some(Point {
                x: origin.x + distance * radians.cos(),
                y: origin.y - distance * radians.sin(),
            })
        })
        .collect()
}

pub fn sample_circle_arc(
    center: Point,
    start: Point,
    end: Point,
    steps: usize,
    y_up: bool,
) -> Option<Vec<Point>> {
    (steps > 0).then_some(())?;
    (0..=steps)
        .map(|step| point_on_circle_arc(center, start, end, step as f64 / steps as f64, y_up))
        .collect()
}

pub fn sample_three_point_arc(
    start: Point,
    mid: Point,
    end: Point,
    steps: usize,
    complement: bool,
) -> Option<Vec<Point>> {
    (steps > 0).then_some(())?;
    (0..=steps)
        .map(|step| {
            let t = step as f64 / steps as f64;
            if complement {
                point_on_three_point_arc_complement(start, mid, end, t)
            } else {
                point_on_three_point_arc(start, mid, end, t)
            }
        })
        .collect()
}

pub fn translation_iteration_deltas(
    depth: usize,
    primary: Point,
    secondary: Option<Point>,
    bidirectional: bool,
    include_origin: bool,
) -> Vec<Point> {
    let mut deltas = Vec::new();
    match (bidirectional, secondary) {
        (true, Some(secondary)) => {
            let depth = depth as isize;
            for primary_step in -depth..=depth {
                for secondary_step in -depth..=depth {
                    if primary_step.unsigned_abs() + secondary_step.unsigned_abs() > depth as usize
                        || (!include_origin && primary_step == 0 && secondary_step == 0)
                    {
                        continue;
                    }
                    deltas.push(Point {
                        x: primary.x * primary_step as f64 + secondary.x * secondary_step as f64,
                        y: primary.y * primary_step as f64 + secondary.y * secondary_step as f64,
                    });
                }
            }
        }
        (true, None) => {
            if include_origin {
                deltas.push(Point::ZERO);
            }
            for step in 1..=depth {
                deltas.push(Point {
                    x: primary.x * step as f64,
                    y: primary.y * step as f64,
                });
                deltas.push(Point {
                    x: -primary.x * step as f64,
                    y: -primary.y * step as f64,
                });
            }
        }
        (false, Some(secondary)) => {
            for primary_step in 0..=depth {
                for secondary_step in 0..=depth - primary_step {
                    if !include_origin && primary_step == 0 && secondary_step == 0 {
                        continue;
                    }
                    deltas.push(Point {
                        x: primary.x * primary_step as f64 + secondary.x * secondary_step as f64,
                        y: primary.y * primary_step as f64 + secondary.y * secondary_step as f64,
                    });
                }
            }
        }
        (false, None) => {
            let first_step = usize::from(!include_origin);
            for step in first_step..=depth {
                deltas.push(Point {
                    x: primary.x * step as f64,
                    y: primary.y * step as f64,
                });
            }
        }
    }
    deltas
}

pub fn rotate_iteration_points(
    points: &[Point],
    center: Point,
    angle_radians: f64,
    depth: usize,
) -> Vec<Point> {
    (1..=depth)
        .flat_map(|step| {
            let radians = angle_radians * step as f64;
            points
                .iter()
                .map(move |point| crate::rotate_around(*point, center, radians))
        })
        .collect()
}

pub fn affine_iteration_segment(
    start: Point,
    end: Point,
    source_triangle: [Point; 3],
    target_triangle: [Point; 3],
    depth: usize,
) -> Option<Vec<Point>> {
    let source_origin = source_triangle[0];
    let source_u = subtract(source_triangle[1], source_origin);
    let source_v = subtract(source_triangle[2], source_origin);
    let determinant = source_u.x * source_v.y - source_u.y * source_v.x;
    if determinant.abs() <= 1e-9 {
        return None;
    }
    let target_origin = target_triangle[0];
    let target_u = subtract(target_triangle[1], target_origin);
    let target_v = subtract(target_triangle[2], target_origin);
    let map = |point: Point| {
        let relative = subtract(point, source_origin);
        let u = (relative.x * source_v.y - relative.y * source_v.x) / determinant;
        let v = (source_u.x * relative.y - source_u.y * relative.x) / determinant;
        Point {
            x: target_origin.x + target_u.x * u + target_v.x * v,
            y: target_origin.y + target_u.y * u + target_v.y * v,
        }
    };
    let mut output = Vec::with_capacity(depth * 2);
    let mut current_start = start;
    let mut current_end = end;
    for _ in 0..depth {
        current_start = map(current_start);
        current_end = map(current_end);
        output.extend([current_start, current_end]);
    }
    Some(output)
}

pub fn branching_iteration_segments(
    start: Point,
    end: Point,
    target_segments: &[[Point; 2]],
    depth: usize,
) -> Option<Vec<Point>> {
    let coefficients = target_segments
        .iter()
        .map(|segment| {
            Some((
                segment_point_coefficients(start, end, segment[0])?,
                segment_point_coefficients(start, end, segment[1])?,
            ))
        })
        .collect::<Option<Vec<_>>>()?;
    if coefficients.is_empty() {
        return None;
    }
    let mut output = Vec::new();
    let mut frontier = vec![[start, end]];
    for _ in 0..depth {
        let mut next = Vec::with_capacity(frontier.len() * coefficients.len());
        for segment in frontier {
            for &(start_coefficients, end_coefficients) in &coefficients {
                let child = [
                    apply_segment_coefficients(segment[0], segment[1], start_coefficients),
                    apply_segment_coefficients(segment[0], segment[1], end_coefficients),
                ];
                output.extend(child);
                next.push(child);
            }
        }
        frontier = next;
    }
    Some(output)
}

fn segment_point_coefficients(
    source_start: Point,
    source_end: Point,
    point: Point,
) -> Option<Point> {
    let delta = subtract(source_end, source_start);
    let length_squared = delta.x * delta.x + delta.y * delta.y;
    if length_squared <= 1e-9 {
        return None;
    }
    let relative = subtract(point, source_start);
    Some(Point {
        x: (relative.x * delta.x + relative.y * delta.y) / length_squared,
        y: (relative.x * -delta.y + relative.y * delta.x) / length_squared,
    })
}

fn apply_segment_coefficients(start: Point, end: Point, coefficients: Point) -> Point {
    let delta = subtract(end, start);
    Point {
        x: start.x + coefficients.x * delta.x - coefficients.y * delta.y,
        y: start.y + coefficients.x * delta.y + coefficients.y * delta.x,
    }
}

fn subtract(left: Point, right: Point) -> Point {
    Point {
        x: left.x - right.x,
        y: left.y - right.y,
    }
}

pub fn line_polyline_intersection(
    line_start: Point,
    line_end: Point,
    line_kind: LineKind,
    points: &[Point],
    sample_hint: Option<f64>,
    variant: usize,
) -> Option<Point> {
    if points.len() < 2 {
        return None;
    }
    if let Some(sample_hint) = sample_hint.filter(|value| value.is_finite()) {
        let mut best = None;
        let mut best_distance = f64::INFINITY;
        for (index, segment) in points.windows(2).enumerate() {
            let Some(hit) = line_line_intersection(
                line_start,
                line_end,
                line_kind,
                segment[0],
                segment[1],
                LineKind::Segment,
            ) else {
                continue;
            };
            let distance = (index as f64 - sample_hint).abs();
            if distance < best_distance {
                best = Some(hit);
                best_distance = distance;
            }
        }
        if best.is_some() {
            return best;
        }
    }

    let mut candidates = Vec::new();
    for segment in points.windows(2) {
        let Some(hit) = line_line_intersection(
            line_start,
            line_end,
            line_kind,
            segment[0],
            segment[1],
            LineKind::Segment,
        ) else {
            continue;
        };
        if !candidates.iter().any(|candidate: &Point| {
            (candidate.x - hit.x).hypot(candidate.y - hit.y) <= POINT_EPSILON
        }) {
            candidates.push(hit);
        }
    }
    candidates.get(variant).copied()
}

pub fn choose_point_candidate(
    candidates: &[Point],
    reference: Option<Point>,
    variant: usize,
) -> Option<Point> {
    candidates.get(variant)?;
    if let Some(reference) = reference.filter(|point| point.x.is_finite() && point.y.is_finite()) {
        return candidates.iter().copied().min_by(|left, right| {
            squared_distance(*left, reference).total_cmp(&squared_distance(*right, reference))
        });
    }
    candidates.get(variant).copied()
}

/// Preserves the payload's branch semantics: select an infinite-line candidate first,
/// then reject it when it is outside the requested line domain.
pub fn line_circle_intersection_candidate(
    start: Point,
    end: Point,
    line_kind: LineKind,
    center: Point,
    radius: f64,
    variant: usize,
) -> Option<Point> {
    let selected = line_circle_intersections(start, end, LineKind::Line, center, radius)
        .get(variant)
        .copied()?;
    point_lies_on_line_kind(selected, start, end, line_kind).then_some(selected)
}

fn point_lies_on_line_kind(point: Point, start: Point, end: Point, line_kind: LineKind) -> bool {
    if line_kind == LineKind::Line {
        return true;
    }
    let delta = subtract(end, start);
    let length_squared = delta.x * delta.x + delta.y * delta.y;
    if length_squared <= 1e-18 {
        return false;
    }
    let t = ((point.x - start.x) * delta.x + (point.y - start.y) * delta.y) / length_squared;
    match line_kind {
        LineKind::Line => true,
        LineKind::Ray => t >= -1e-9,
        LineKind::Segment => (-1e-9..=1.0 + 1e-9).contains(&t),
    }
}

fn squared_distance(left: Point, right: Point) -> f64 {
    (left.x - right.x).powi(2) + (left.y - right.y).powi(2)
}

pub fn point_distance(left: Point, right: Point, value_scale: f64) -> Option<f64> {
    finite((right.x - left.x).hypot(right.y - left.y) * value_scale)
}

pub fn point_distance_ratio(
    origin: Point,
    denominator: Point,
    numerator: Point,
    clamp_to_unit: bool,
) -> Option<f64> {
    let denominator_length = (denominator.x - origin.x).hypot(denominator.y - origin.y);
    if denominator_length <= 1e-9 {
        return None;
    }
    let ratio = (numerator.x - origin.x).hypot(numerator.y - origin.y) / denominator_length;
    finite(if clamp_to_unit { ratio.min(1.0) } else { ratio })
}

pub fn point_angle_degrees(start: Point, vertex: Point, end: Point) -> Option<f64> {
    let first = Point {
        x: start.x - vertex.x,
        y: start.y - vertex.y,
    };
    let second = Point {
        x: end.x - vertex.x,
        y: end.y - vertex.y,
    };
    let first_len = first.x.hypot(first.y);
    let second_len = second.x.hypot(second.y);
    if first_len <= 1e-9 || second_len <= 1e-9 {
        return None;
    }
    let cross = first.x / first_len * (second.y / second_len)
        - first.y / first_len * (second.x / second_len);
    let dot = first.x / first_len * (second.x / second_len)
        + first.y / first_len * (second.y / second_len);
    finite(cross.atan2(dot).abs().to_degrees())
}

pub fn angle_marker_points(
    start: Point,
    vertex: Point,
    end: Point,
    marker_class: u32,
) -> Option<Vec<Point>> {
    let first_dx = start.x - vertex.x;
    let first_dy = start.y - vertex.y;
    let second_dx = end.x - vertex.x;
    let second_dy = end.y - vertex.y;
    let first_len = first_dx.hypot(first_dy);
    let second_len = second_dx.hypot(second_dy);
    let shortest_len = first_len.min(second_len);
    if first_len <= 1e-9 || second_len <= 1e-9 || shortest_len <= 1e-9 {
        return None;
    }
    let first = Point {
        x: first_dx / first_len,
        y: first_dy / first_len,
    };
    let second = Point {
        x: second_dx / second_len,
        y: second_dy / second_len,
    };
    let dot = (first.x * second.x + first.y * second.y).clamp(-1.0, 1.0);
    let cross = first.x * second.y - first.y * second.x;
    if dot.abs() <= 0.12 {
        let side = (shortest_len * 0.125)
            .clamp(10.0, 28.0)
            .min(shortest_len * 0.5);
        if side <= 1e-9 {
            return None;
        }
        return Some(vec![
            Point {
                x: vertex.x + first.x * side,
                y: vertex.y + first.y * side,
            },
            Point {
                x: vertex.x + (first.x + second.x) * side,
                y: vertex.y + (first.y + second.y) * side,
            },
            Point {
                x: vertex.x + second.x * side,
                y: vertex.y + second.y * side,
            },
        ]);
    }

    let class_scale = 1.0 + 0.18 * marker_class.max(1).saturating_sub(1) as f64;
    let radius = ((shortest_len * 0.12).clamp(10.0, 28.0) * class_scale).min(shortest_len * 0.42);
    let delta = cross.atan2(dot);
    if radius <= 1e-9 || delta.abs() <= 1e-6 {
        return None;
    }
    let start_angle = first.y.atan2(first.x);
    let samples = 9usize;
    Some(
        (0..samples)
            .map(|index| {
                let t = index as f64 / (samples - 1) as f64;
                let angle = start_angle + delta * t;
                Point {
                    x: vertex.x + radius * angle.cos(),
                    y: vertex.y + radius * angle.sin(),
                }
            })
            .collect(),
    )
}

pub fn polygon_area(points: &[Point], value_scale: f64) -> Option<f64> {
    if points.len() < 3 {
        return None;
    }
    let twice_area = points
        .iter()
        .zip(points.iter().cycle().skip(1))
        .take(points.len())
        .map(|(left, right)| left.x * right.y - right.x * left.y)
        .sum::<f64>();
    finite(twice_area.abs() * 0.5 * value_scale)
}

fn finite(value: f64) -> Option<f64> {
    value.is_finite().then_some(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expression_sampling_preserves_discontinuities() {
        let expr = FunctionExpr::Parsed(crate::FunctionAst::Binary {
            lhs: Box::new(crate::FunctionAst::Constant(1.0)),
            op: crate::BinaryOp::Div,
            rhs: Box::new(crate::FunctionAst::Variable),
        });
        let sampled = sample_expression(&expr, &BTreeMap::new(), -1.0, 1.0, 3, PlotMode::Cartesian);
        assert_eq!(sampled[0], Some(Point { x: -1.0, y: -1.0 }));
        assert_eq!(sampled[1], None);
        assert_eq!(sampled[2], Some(Point { x: 1.0, y: 1.0 }));
    }

    #[test]
    fn coordinate_trace_varies_named_parameters_in_one_batch() {
        let expr = FunctionExpr::Parsed(crate::FunctionAst::Parameter("t".into(), 0.0));
        assert_eq!(
            sample_coordinate_trace(
                &expr,
                None,
                &BTreeMap::new(),
                None,
                Some("t"),
                None,
                Point { x: 1.0, y: 2.0 },
                0.0,
                2.0,
                3,
                false,
                CoordinateTraceMode::Horizontal,
            ),
            vec![
                Point { x: 1.0, y: 2.0 },
                Point { x: 2.0, y: 2.0 },
                Point { x: 3.0, y: 2.0 },
            ],
        );
    }

    #[test]
    fn polyline_variant_and_sample_hint_match_runtime_policy() {
        let points = [
            Point { x: -2.0, y: -1.0 },
            Point { x: -1.0, y: 1.0 },
            Point { x: 1.0, y: -1.0 },
            Point { x: 2.0, y: 1.0 },
        ];
        let start = Point { x: -3.0, y: 0.0 };
        let end = Point { x: 3.0, y: 0.0 };
        assert_eq!(
            line_polyline_intersection(start, end, LineKind::Line, &points, None, 1),
            Some(Point { x: 0.0, y: 0.0 }),
        );
        assert_eq!(
            line_polyline_intersection(start, end, LineKind::Line, &points, Some(2.0), 0),
            Some(Point { x: 1.5, y: 0.0 }),
        );
    }

    #[test]
    fn measurements_reject_degenerate_inputs() {
        let origin = Point::ZERO;
        assert_eq!(point_distance_ratio(origin, origin, origin, false), None);
        assert_eq!(point_angle_degrees(origin, origin, origin), None);
        assert_eq!(polygon_area(&[origin, origin], 1.0), None);
    }

    #[test]
    fn translation_iteration_deltas_preserve_line_and_polygon_domains() {
        let primary = Point { x: 1.0, y: 0.0 };
        let secondary = Point { x: 0.0, y: 1.0 };
        let line = translation_iteration_deltas(2, primary, Some(secondary), true, false);
        let polygon = translation_iteration_deltas(2, primary, Some(secondary), true, true);
        assert_eq!(line.len(), 12);
        assert_eq!(polygon.len(), 13);
        assert!(!line.contains(&Point::ZERO));
        assert!(polygon.contains(&Point::ZERO));
    }

    #[test]
    fn affine_and_branching_iterations_expand_complete_segments() {
        let start = Point::ZERO;
        let end = Point { x: 1.0, y: 0.0 };
        let affine = affine_iteration_segment(
            start,
            end,
            [start, end, Point { x: 0.0, y: 1.0 }],
            [start, Point { x: 2.0, y: 0.0 }, Point { x: 0.0, y: 2.0 }],
            2,
        )
        .unwrap();
        assert_eq!(
            affine,
            vec![
                start,
                Point { x: 2.0, y: 0.0 },
                start,
                Point { x: 4.0, y: 0.0 }
            ]
        );

        let branches = branching_iteration_segments(
            start,
            end,
            &[
                [start, Point { x: 0.5, y: 0.0 }],
                [Point { x: 0.5, y: 0.0 }, end],
            ],
            2,
        )
        .unwrap();
        assert_eq!(branches.len(), 12);
    }

    #[test]
    fn candidate_policy_validates_variant_and_line_domain() {
        let candidates = [Point { x: -1.0, y: 0.0 }, Point { x: 1.0, y: 0.0 }];
        assert_eq!(
            choose_point_candidate(&candidates, Some(Point { x: 0.9, y: 0.0 }), 0),
            Some(candidates[1]),
        );
        assert_eq!(
            choose_point_candidate(&candidates, Some(Point { x: 0.9, y: 0.0 }), 2),
            None,
        );
        assert_eq!(
            line_circle_intersection_candidate(
                Point::ZERO,
                Point { x: 0.5, y: 0.0 },
                LineKind::Segment,
                Point::ZERO,
                1.0,
                1,
            ),
            None,
        );
    }
}
