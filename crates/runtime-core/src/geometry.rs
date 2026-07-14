const EPSILON: f64 = 1e-9;

#[derive(Debug, Clone, Copy, Default, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

impl Point {
    pub const ZERO: Self = Self { x: 0.0, y: 0.0 };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum LineKind {
    Segment,
    Line,
    Ray,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bounds {
    pub min_x: f64,
    pub max_x: f64,
    pub min_y: f64,
    pub max_y: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Projection {
    pub t: f64,
    pub projected: Point,
    pub distance_squared: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThreePointArcGeometry {
    pub center: Point,
    pub radius: f64,
    pub start_angle: f64,
    pub mid_angle: f64,
    pub end_angle: f64,
    pub ccw_span: f64,
    pub ccw_mid: f64,
}

pub fn normalize_angle_delta(from: f64, to: f64) -> f64 {
    (to - from).rem_euclid(std::f64::consts::TAU)
}

pub fn lerp_point(start: Point, end: Point, t: f64) -> Point {
    Point {
        x: start.x + (end.x - start.x) * t,
        y: start.y + (end.y - start.y) * t,
    }
}

pub fn rotate_around(point: Point, center: Point, radians: f64) -> Point {
    let cos = radians.cos();
    let sin = radians.sin();
    let dx = point.x - center.x;
    let dy = point.y - center.y;
    Point {
        x: center.x + dx * cos + dy * sin,
        y: center.y - dx * sin + dy * cos,
    }
}

pub fn scale_around(point: Point, center: Point, factor: f64) -> Point {
    Point {
        x: center.x + (point.x - center.x) * factor,
        y: center.y + (point.y - center.y) * factor,
    }
}

pub fn marked_angle_translation_point(
    target: Point,
    angle_start: Point,
    angle_vertex: Point,
    angle_end: Point,
    distance: f64,
) -> Option<Point> {
    if !distance.is_finite() {
        return None;
    }
    let angle = measured_rotation_radians(angle_start, angle_vertex, angle_end)?;
    Some(Point {
        x: target.x + distance * angle.cos(),
        y: target.y - distance * angle.sin(),
    })
}

pub fn reflect_across_line(point: Point, line_start: Point, line_end: Point) -> Option<Point> {
    let dx = line_end.x - line_start.x;
    let dy = line_end.y - line_start.y;
    let len_sq = dx * dx + dy * dy;
    if len_sq <= EPSILON {
        return None;
    }
    let t = ((point.x - line_start.x) * dx + (point.y - line_start.y) * dy) / len_sq;
    let projection = Point {
        x: line_start.x + t * dx,
        y: line_start.y + t * dy,
    };
    Some(Point {
        x: projection.x * 2.0 - point.x,
        y: projection.y * 2.0 - point.y,
    })
}

pub fn project_to_line_like(
    point: Point,
    start: Point,
    end: Point,
    kind: LineKind,
) -> Option<Projection> {
    let dx = end.x - start.x;
    let dy = end.y - start.y;
    let length_squared = dx * dx + dy * dy;
    if length_squared <= EPSILON {
        return None;
    }
    let raw_t = ((point.x - start.x) * dx + (point.y - start.y) * dy) / length_squared;
    let t = match kind {
        LineKind::Line => raw_t,
        LineKind::Ray => raw_t.max(0.0),
        LineKind::Segment => raw_t.clamp(0.0, 1.0),
    };
    let projected = lerp_point(start, end, t);
    Some(Projection {
        t,
        projected,
        distance_squared: (point.x - projected.x).powi(2) + (point.y - projected.y).powi(2),
    })
}

pub fn angle_bisector_direction(start: Point, vertex: Point, end: Point) -> Option<Point> {
    let first_dx = start.x - vertex.x;
    let first_dy = start.y - vertex.y;
    let second_dx = end.x - vertex.x;
    let second_dy = end.y - vertex.y;
    let first_len = first_dx.hypot(first_dy);
    let second_len = second_dx.hypot(second_dy);
    if first_len <= EPSILON || second_len <= EPSILON {
        return None;
    }
    let sum_x = first_dx / first_len + second_dx / second_len;
    let sum_y = first_dy / first_len + second_dy / second_len;
    let sum_len = sum_x.hypot(sum_y);
    Some(if sum_len > EPSILON {
        Point {
            x: sum_x / sum_len,
            y: sum_y / sum_len,
        }
    } else {
        Point {
            x: -first_dy / first_len,
            y: first_dx / first_len,
        }
    })
}

pub fn measured_rotation_radians(start: Point, vertex: Point, end: Point) -> Option<f64> {
    let first_x = start.x - vertex.x;
    let first_y = vertex.y - start.y;
    let second_x = end.x - vertex.x;
    let second_y = vertex.y - end.y;
    if first_x.hypot(first_y) <= EPSILON || second_x.hypot(second_y) <= EPSILON {
        return None;
    }
    Some((first_x * second_y - first_y * second_x).atan2(first_x * second_x + first_y * second_y))
}

pub fn scale_by_three_point_ratio(
    source: Point,
    center: Point,
    ratio_origin: Point,
    ratio_denominator: Point,
    ratio_numerator: Point,
    signed: bool,
    clamp_to_unit: bool,
) -> Option<Point> {
    let denominator_dx = ratio_denominator.x - ratio_origin.x;
    let denominator_dy = ratio_denominator.y - ratio_origin.y;
    let numerator_dx = ratio_numerator.x - ratio_origin.x;
    let numerator_dy = ratio_numerator.y - ratio_origin.y;
    let denominator = denominator_dx.hypot(denominator_dy);
    if denominator <= EPSILON {
        return None;
    }
    let raw_numerator = numerator_dx.hypot(numerator_dy);
    let numerator = if clamp_to_unit {
        raw_numerator.min(denominator)
    } else {
        raw_numerator
    };
    let direction = if signed && denominator_dx * numerator_dx + denominator_dy * numerator_dy < 0.0
    {
        -1.0
    } else {
        1.0
    };
    Some(scale_around(
        source,
        center,
        direction * numerator / denominator,
    ))
}

pub fn clip_line_to_bounds(start: Point, end: Point, bounds: Bounds) -> Option<[Point; 2]> {
    clip_parametric_line_to_bounds(start, end, bounds, false)
}

pub fn clip_ray_to_bounds(start: Point, end: Point, bounds: Bounds) -> Option<[Point; 2]> {
    clip_parametric_line_to_bounds(start, end, bounds, true)
}

fn clip_parametric_line_to_bounds(
    start: Point,
    end: Point,
    bounds: Bounds,
    ray_only: bool,
) -> Option<[Point; 2]> {
    let dx = end.x - start.x;
    let dy = end.y - start.y;
    if dx.abs() <= EPSILON && dy.abs() <= EPSILON {
        return None;
    }
    let mut hits = Vec::<(f64, Point)>::new();
    let mut push_hit = |t: f64, point: Point| {
        if !t.is_finite()
            || (ray_only && t < -EPSILON)
            || point.x < bounds.min_x - 1e-6
            || point.x > bounds.max_x + 1e-6
            || point.y < bounds.min_y - 1e-6
            || point.y > bounds.max_y + 1e-6
            || hits.iter().any(|(existing_t, existing)| {
                (existing_t - t).abs() < 1e-6
                    || ((existing.x - point.x).abs() < 1e-6 && (existing.y - point.y).abs() < 1e-6)
            })
        {
            return;
        }
        hits.push((t, point));
    };
    if dx.abs() > EPSILON {
        for x in [bounds.min_x, bounds.max_x] {
            let t = (x - start.x) / dx;
            push_hit(
                t,
                Point {
                    x,
                    y: start.y + dy * t,
                },
            );
        }
    }
    if dy.abs() > EPSILON {
        for y in [bounds.min_y, bounds.max_y] {
            let t = (y - start.y) / dy;
            push_hit(
                t,
                Point {
                    x: start.x + dx * t,
                    y,
                },
            );
        }
    }
    if ray_only
        && start.x >= bounds.min_x - 1e-6
        && start.x <= bounds.max_x + 1e-6
        && start.y >= bounds.min_y - 1e-6
        && start.y <= bounds.max_y + 1e-6
    {
        push_hit(0.0, start);
    }
    if hits.len() < 2 {
        return None;
    }
    hits.sort_by(|left, right| left.0.total_cmp(&right.0));
    let first = hits.first()?.1;
    let last = hits.last()?.1;
    ((first.x - last.x).abs() >= 1e-6 || (first.y - last.y).abs() >= 1e-6).then_some([first, last])
}

pub fn three_point_arc_geometry(
    start: Point,
    mid: Point,
    end: Point,
) -> Option<ThreePointArcGeometry> {
    let determinant =
        2.0 * (start.x * (mid.y - end.y) + mid.x * (end.y - start.y) + end.x * (start.y - mid.y));
    if determinant.abs() <= EPSILON {
        return None;
    }
    let start_sq = start.x * start.x + start.y * start.y;
    let mid_sq = mid.x * mid.x + mid.y * mid.y;
    let end_sq = end.x * end.x + end.y * end.y;
    let center = Point {
        x: (start_sq * (mid.y - end.y) + mid_sq * (end.y - start.y) + end_sq * (start.y - mid.y))
            / determinant,
        y: (start_sq * (end.x - mid.x) + mid_sq * (start.x - end.x) + end_sq * (mid.x - start.x))
            / determinant,
    };
    let radius = (start.x - center.x).hypot(start.y - center.y);
    if radius <= EPSILON {
        return None;
    }
    let start_angle = (start.y - center.y).atan2(start.x - center.x);
    let mid_angle = (mid.y - center.y).atan2(mid.x - center.x);
    let end_angle = (end.y - center.y).atan2(end.x - center.x);
    Some(ThreePointArcGeometry {
        center,
        radius,
        start_angle,
        mid_angle,
        end_angle,
        ccw_span: normalize_angle_delta(start_angle, end_angle),
        ccw_mid: normalize_angle_delta(start_angle, mid_angle),
    })
}

pub fn point_on_three_point_arc(start: Point, mid: Point, end: Point, t: f64) -> Option<Point> {
    point_on_three_point_arc_with_complement(start, mid, end, t, false)
}

pub fn point_on_three_point_arc_complement(
    start: Point,
    mid: Point,
    end: Point,
    t: f64,
) -> Option<Point> {
    point_on_three_point_arc_with_complement(start, mid, end, t, true)
}

fn point_on_three_point_arc_with_complement(
    start: Point,
    mid: Point,
    end: Point,
    t: f64,
    complement: bool,
) -> Option<Point> {
    let geometry = three_point_arc_geometry(start, mid, end)?;
    let use_ccw = if complement {
        geometry.ccw_mid > geometry.ccw_span + EPSILON
    } else {
        geometry.ccw_mid <= geometry.ccw_span + EPSILON
    };
    let angle = if use_ccw {
        geometry.start_angle + geometry.ccw_span * t.clamp(0.0, 1.0)
    } else {
        geometry.start_angle
            - normalize_angle_delta(geometry.end_angle, geometry.start_angle) * t.clamp(0.0, 1.0)
    };
    Some(Point {
        x: geometry.center.x + geometry.radius * angle.cos(),
        y: geometry.center.y + geometry.radius * angle.sin(),
    })
}

pub fn circle_arc_control_points(
    center: Point,
    start: Point,
    end: Point,
    y_up: bool,
) -> Option<[Point; 3]> {
    let start_dx = start.x - center.x;
    let start_dy = start.y - center.y;
    let end_dx = end.x - center.x;
    let end_dy = end.y - center.y;
    let radius = (start_dx.hypot(start_dy) + end_dx.hypot(end_dy)) * 0.5;
    if radius <= EPSILON {
        return None;
    }
    let y_sign = if y_up { 1.0 } else { -1.0 };
    let start_angle = (start_dy * y_sign).atan2(start_dx);
    let end_angle = (end_dy * y_sign).atan2(end_dx);
    let midpoint_angle = start_angle + normalize_angle_delta(start_angle, end_angle) * 0.5;
    Some([
        start,
        Point {
            x: center.x + radius * midpoint_angle.cos(),
            y: center.y + y_sign * radius * midpoint_angle.sin(),
        },
        end,
    ])
}

pub fn point_on_circle_arc(
    center: Point,
    start: Point,
    end: Point,
    t: f64,
    y_up: bool,
) -> Option<Point> {
    let [start, mid, end] = circle_arc_control_points(center, start, end, y_up)?;
    point_on_three_point_arc(start, mid, end, t)
}

pub fn project_to_three_point_arc(
    point: Point,
    start: Point,
    mid: Point,
    end: Point,
) -> Option<Projection> {
    let mut best: Option<Projection> = None;
    for step in 0..=256 {
        let t = step as f64 / 256.0;
        let projected = point_on_three_point_arc(start, mid, end, t)?;
        let distance_squared = (point.x - projected.x).powi(2) + (point.y - projected.y).powi(2);
        if best.is_none_or(|candidate| distance_squared < candidate.distance_squared) {
            best = Some(Projection {
                t,
                projected,
                distance_squared,
            });
        }
    }
    best
}

pub fn project_to_circle_arc(
    point: Point,
    center: Point,
    start: Point,
    end: Point,
    y_up: bool,
) -> Option<Projection> {
    let [start, mid, end] = circle_arc_control_points(center, start, end, y_up)?;
    project_to_three_point_arc(point, start, mid, end)
}

pub fn line_line_intersection(
    left_start: Point,
    left_end: Point,
    left_kind: LineKind,
    right_start: Point,
    right_end: Point,
    right_kind: LineKind,
) -> Option<Point> {
    let left_dx = left_end.x - left_start.x;
    let left_dy = left_end.y - left_start.y;
    let right_dx = right_end.x - right_start.x;
    let right_dy = right_end.y - right_start.y;
    let determinant = left_dx * right_dy - left_dy * right_dx;
    if determinant.abs() <= EPSILON {
        return None;
    }
    let delta_x = right_start.x - left_start.x;
    let delta_y = right_start.y - left_start.y;
    let left_t = (delta_x * right_dy - delta_y * right_dx) / determinant;
    let right_t = (delta_x * left_dy - delta_y * left_dx) / determinant;
    if !line_kind_allows_parameter(left_t, left_kind)
        || !line_kind_allows_parameter(right_t, right_kind)
    {
        return None;
    }
    Some(Point {
        x: left_start.x + left_t * left_dx,
        y: left_start.y + left_t * left_dy,
    })
}

pub fn line_circle_intersections(
    line_start: Point,
    line_end: Point,
    line_kind: LineKind,
    center: Point,
    radius: f64,
) -> Vec<Point> {
    if radius <= EPSILON {
        return Vec::new();
    }
    let dx = line_end.x - line_start.x;
    let dy = line_end.y - line_start.y;
    let a = dx * dx + dy * dy;
    if a <= EPSILON {
        return Vec::new();
    }
    let fx = line_start.x - center.x;
    let fy = line_start.y - center.y;
    let b = 2.0 * (fx * dx + fy * dy);
    let c = fx * fx + fy * fy - radius * radius;
    let discriminant = b * b - 4.0 * a * c;
    if discriminant < -EPSILON {
        return Vec::new();
    }
    let root = discriminant.max(0.0).sqrt();
    [(-b - root) / (2.0 * a), (-b + root) / (2.0 * a)]
        .into_iter()
        .filter(|t| line_kind_allows_parameter(*t, line_kind))
        .map(|t| Point {
            x: line_start.x + dx * t,
            y: line_start.y + dy * t,
        })
        .collect()
}

pub fn circle_circle_intersections(
    left_center: Point,
    left_radius: f64,
    right_center: Point,
    right_radius: f64,
) -> Vec<Point> {
    if left_radius <= EPSILON || right_radius <= EPSILON {
        return Vec::new();
    }
    let dx = right_center.x - left_center.x;
    let dy = right_center.y - left_center.y;
    let distance = dx.hypot(dy);
    if distance <= EPSILON
        || distance > left_radius + right_radius + EPSILON
        || distance < (left_radius - right_radius).abs() - EPSILON
    {
        return Vec::new();
    }
    let along = (left_radius * left_radius - right_radius * right_radius + distance * distance)
        / (2.0 * distance);
    let height_sq = left_radius * left_radius - along * along;
    if height_sq < -EPSILON {
        return Vec::new();
    }
    let height = height_sq.max(0.0).sqrt();
    let ux = dx / distance;
    let uy = dy / distance;
    let base = Point {
        x: left_center.x + along * ux,
        y: left_center.y + along * uy,
    };
    let mut points = vec![
        Point {
            x: base.x - height * uy,
            y: base.y + height * ux,
        },
        Point {
            x: base.x + height * uy,
            y: base.y - height * ux,
        },
    ];
    points.sort_by(|left, right| {
        left.y
            .total_cmp(&right.y)
            .then_with(|| left.x.total_cmp(&right.x))
    });
    points
}

pub fn point_circle_tangents(point: Point, center: Point, radius: f64) -> Vec<Point> {
    if radius <= EPSILON {
        return Vec::new();
    }
    let dx = point.x - center.x;
    let dy = point.y - center.y;
    let distance_sq = dx * dx + dy * dy;
    if distance_sq <= radius * radius + EPSILON {
        return Vec::new();
    }
    let distance = distance_sq.sqrt();
    let base_angle = dy.atan2(dx);
    let offset = (radius / distance).acos();
    let mut points = vec![
        Point {
            x: center.x + radius * (base_angle - offset).cos(),
            y: center.y + radius * (base_angle - offset).sin(),
        },
        Point {
            x: center.x + radius * (base_angle + offset).cos(),
            y: center.y + radius * (base_angle + offset).sin(),
        },
    ];
    points.sort_by(|left, right| {
        left.y
            .total_cmp(&right.y)
            .then_with(|| left.x.total_cmp(&right.x))
    });
    points
}

fn line_kind_allows_parameter(t: f64, kind: LineKind) -> bool {
    match kind {
        LineKind::Line => true,
        LineKind::Ray => t >= -EPSILON,
        LineKind::Segment => (-EPSILON..=1.0 + EPSILON).contains(&t),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_point_close(actual: Point, expected: Point) {
        assert!((actual.x - expected.x).abs() <= 1e-9, "x: {actual:?}");
        assert!((actual.y - expected.y).abs() <= 1e-9, "y: {actual:?}");
    }

    #[test]
    fn line_kinds_limit_intersections() {
        let segment_start = Point { x: 0.0, y: 0.0 };
        let segment_end = Point { x: 1.0, y: 0.0 };
        let vertical_start = Point { x: 2.0, y: -1.0 };
        let vertical_end = Point { x: 2.0, y: 1.0 };
        assert!(
            line_line_intersection(
                segment_start,
                segment_end,
                LineKind::Segment,
                vertical_start,
                vertical_end,
                LineKind::Line,
            )
            .is_none()
        );
        assert_eq!(
            line_line_intersection(
                segment_start,
                segment_end,
                LineKind::Line,
                vertical_start,
                vertical_end,
                LineKind::Line,
            ),
            Some(Point { x: 2.0, y: 0.0 })
        );
    }

    #[test]
    fn circle_intersections_are_sorted_and_reject_degenerate_circles() {
        let center = Point::ZERO;
        assert!(circle_circle_intersections(center, 0.0, Point { x: 1.0, y: 0.0 }, 1.0).is_empty());
        assert_eq!(
            circle_circle_intersections(center, 1.0, Point { x: 1.0, y: 0.0 }, 1.0),
            vec![
                Point {
                    x: 0.5,
                    y: -3.0_f64.sqrt() / 2.0
                },
                Point {
                    x: 0.5,
                    y: 3.0_f64.sqrt() / 2.0
                },
            ]
        );
    }

    #[test]
    fn constraint_projection_and_clipping_share_line_kind_rules() {
        let start = Point::ZERO;
        let end = Point { x: 2.0, y: 0.0 };
        let point = Point { x: 3.0, y: 4.0 };
        let segment = project_to_line_like(point, start, end, LineKind::Segment).unwrap();
        assert_eq!(segment.t, 1.0);
        assert_point_close(segment.projected, end);
        let line = project_to_line_like(point, start, end, LineKind::Line).unwrap();
        assert_eq!(line.t, 1.5);
        assert_point_close(line.projected, Point { x: 3.0, y: 0.0 });

        let bounds = Bounds {
            min_x: -1.0,
            max_x: 1.0,
            min_y: -1.0,
            max_y: 1.0,
        };
        assert_eq!(
            clip_ray_to_bounds(Point::ZERO, Point { x: 1.0, y: 0.0 }, bounds),
            Some([Point::ZERO, Point { x: 1.0, y: 0.0 }]),
        );
    }

    #[test]
    fn arc_geometry_sampling_and_projection_use_one_semantics() {
        let start = Point { x: -1.0, y: 0.0 };
        let mid = Point { x: 0.0, y: -1.0 };
        let end = Point { x: 1.0, y: 0.0 };
        let geometry = three_point_arc_geometry(start, mid, end).unwrap();
        assert_point_close(geometry.center, Point::ZERO);
        assert!((geometry.radius - 1.0).abs() <= 1e-9);
        assert_point_close(point_on_three_point_arc(start, mid, end, 0.5).unwrap(), mid);
        let projection = project_to_three_point_arc(mid, start, mid, end).unwrap();
        assert!((projection.t - 0.5).abs() <= 1.0 / 256.0);
        assert!(projection.distance_squared <= 1e-9);
    }

    #[test]
    fn bisector_rotation_and_ratio_transform_handle_degenerate_inputs() {
        let vertex = Point::ZERO;
        let direction =
            angle_bisector_direction(Point { x: 1.0, y: 0.0 }, vertex, Point { x: 0.0, y: 1.0 })
                .unwrap();
        let unit = 0.5_f64.sqrt();
        assert_point_close(direction, Point { x: unit, y: unit });
        assert!(measured_rotation_radians(vertex, vertex, Point { x: 1.0, y: 0.0 }).is_none());
        assert!(
            scale_by_three_point_ratio(
                Point { x: 2.0, y: 0.0 },
                vertex,
                vertex,
                vertex,
                Point { x: 1.0, y: 0.0 },
                true,
                false,
            )
            .is_none()
        );

        let translated = marked_angle_translation_point(
            Point { x: 10.0, y: 20.0 },
            Point { x: 1.0, y: 0.0 },
            Point::ZERO,
            Point { x: 0.0, y: -1.0 },
            3.0,
        )
        .unwrap();
        assert_point_close(translated, Point { x: 10.0, y: 17.0 });
        assert!(
            marked_angle_translation_point(
                Point::ZERO,
                Point::ZERO,
                Point::ZERO,
                Point { x: 1.0, y: 0.0 },
                1.0,
            )
            .is_none()
        );
    }
}
