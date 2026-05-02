use crate::format::PointRecord;
use crate::runtime::geometry::lerp_point;
use crate::runtime::scene::LineLikeKind;

#[derive(Clone)]
pub(crate) enum CircularConstraintRaw {
    Circle {
        center: PointRecord,
        radius: f64,
    },
    ThreePointArc {
        start: PointRecord,
        mid: PointRecord,
        end: PointRecord,
        center: PointRecord,
        radius: f64,
        ccw_span: f64,
        ccw_mid: f64,
    },
}

impl CircularConstraintRaw {
    pub(super) fn center(&self) -> PointRecord {
        match self {
            Self::Circle { center, .. } | Self::ThreePointArc { center, .. } => center.clone(),
        }
    }

    pub(crate) fn radius(&self) -> f64 {
        match self {
            Self::Circle { radius, .. } | Self::ThreePointArc { radius, .. } => *radius,
        }
    }
}

pub(super) fn line_polyline_intersection(
    line_start: PointRecord,
    line_end: PointRecord,
    line_kind: LineLikeKind,
    polyline: &[PointRecord],
) -> Option<PointRecord> {
    polyline.windows(2).find_map(|segment| {
        let start = segment.first()?;
        let end = segment.get(1)?;
        line_line_intersection(
            &line_start,
            &line_end,
            line_kind,
            start,
            end,
            LineLikeKind::Segment,
        )
    })
}

pub(super) fn distinct_pair(
    start: PointRecord,
    end: PointRecord,
) -> Option<(PointRecord, PointRecord)> {
    (((end.x - start.x).powi(2) + (end.y - start.y).powi(2)).sqrt() > 1e-9).then_some((start, end))
}

pub(super) fn select_line_circle_intersection(
    line_start: PointRecord,
    line_end: PointRecord,
    line_kind: LineLikeKind,
    center: PointRecord,
    radius: f64,
    variant: usize,
) -> Option<PointRecord> {
    let dx = line_end.x - line_start.x;
    let dy = line_end.y - line_start.y;
    let a = dx * dx + dy * dy;
    if a <= 1e-9 {
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
    let ts = [(-b - root) / (2.0 * a), (-b + root) / (2.0 * a)]
        .into_iter()
        .filter(|t| line_like_allows_param(*t, line_kind))
        .collect::<Vec<_>>();
    if ts.is_empty() {
        return None;
    }
    let t = ts[variant.min(ts.len().saturating_sub(1))];
    Some(PointRecord {
        x: line_start.x + dx * t,
        y: line_start.y + dy * t,
    })
}

pub(super) fn line_line_intersection(
    left_start: &PointRecord,
    left_end: &PointRecord,
    left_kind: LineLikeKind,
    right_start: &PointRecord,
    right_end: &PointRecord,
    right_kind: LineLikeKind,
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
    (line_like_contains(left_start, left_end, left_kind, &point)
        && line_like_contains(right_start, right_end, right_kind, &point))
    .then_some(point)
}

fn line_like_contains(
    start: &PointRecord,
    end: &PointRecord,
    kind: LineLikeKind,
    point: &PointRecord,
) -> bool {
    let dx = end.x - start.x;
    let dy = end.y - start.y;
    let len_sq = dx * dx + dy * dy;
    if len_sq <= 1e-9 {
        return false;
    }
    let t = ((point.x - start.x) * dx + (point.y - start.y) * dy) / len_sq;
    line_like_allows_param(t, kind)
}

fn line_like_allows_param(t: f64, kind: LineLikeKind) -> bool {
    match kind {
        LineLikeKind::Line => true,
        LineLikeKind::Ray => t >= -1e-9,
        LineLikeKind::Segment => (-1e-9..=1.0 + 1e-9).contains(&t),
    }
}

pub(super) fn select_circular_intersection(
    left: &CircularConstraintRaw,
    right: &CircularConstraintRaw,
    variant: usize,
) -> Option<PointRecord> {
    let intersections = circle_circle_intersections(
        &left.center(),
        left.radius(),
        &right.center(),
        right.radius(),
    )?;
    let on_both = intersections
        .iter()
        .filter(|point| point_lies_on_circular_constraint(point, left))
        .filter(|point| point_lies_on_circular_constraint(point, right))
        .cloned()
        .collect::<Vec<_>>();
    on_both
        .get(variant.min(on_both.len().saturating_sub(1)))
        .cloned()
}

pub(super) fn select_point_circle_tangent(
    point: &PointRecord,
    circle: &CircularConstraintRaw,
    variant: usize,
) -> Option<PointRecord> {
    let center = circle.center();
    let radius = circle.radius();
    let dx = point.x - center.x;
    let dy = point.y - center.y;
    let distance_sq = dx * dx + dy * dy;
    if distance_sq <= radius * radius + 1e-9 {
        return None;
    }
    let distance = distance_sq.sqrt();
    let base_angle = dy.atan2(dx);
    let offset = (radius / distance).acos();
    let mut tangents = vec![
        PointRecord {
            x: center.x + radius * (base_angle - offset).cos(),
            y: center.y + radius * (base_angle - offset).sin(),
        },
        PointRecord {
            x: center.x + radius * (base_angle + offset).cos(),
            y: center.y + radius * (base_angle + offset).sin(),
        },
    ];
    tangents.sort_by(|left, right| {
        left.y
            .total_cmp(&right.y)
            .then_with(|| left.x.total_cmp(&right.x))
    });
    tangents
        .into_iter()
        .filter(|candidate| point_lies_on_circular_constraint(candidate, circle))
        .nth(variant.min(1))
}

fn circle_circle_intersections(
    left_center: &PointRecord,
    left_radius: f64,
    right_center: &PointRecord,
    right_radius: f64,
) -> Option<Vec<PointRecord>> {
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
    let mut intersections = vec![
        PointRecord {
            x: base.x - height * uy,
            y: base.y + height * ux,
        },
        PointRecord {
            x: base.x + height * uy,
            y: base.y - height * ux,
        },
    ];
    intersections.sort_by(|left, right| {
        left.y
            .total_cmp(&right.y)
            .then_with(|| left.x.total_cmp(&right.x))
    });
    Some(intersections)
}

fn point_lies_on_circular_constraint(
    point: &PointRecord,
    constraint: &CircularConstraintRaw,
) -> bool {
    match constraint {
        CircularConstraintRaw::Circle { .. } => true,
        CircularConstraintRaw::ThreePointArc {
            start,
            mid,
            end,
            center,
            radius,
            ccw_span,
            ccw_mid,
        } => {
            let radial = ((point.x - center.x).powi(2) + (point.y - center.y).powi(2)).sqrt();
            if (radial - radius).abs() > 1e-6 {
                return false;
            }
            let angle = (point.y - center.y).atan2(point.x - center.x);
            let on_arc = if *ccw_mid <= *ccw_span + 1e-9 {
                normalize_angle_delta_raw((start.y - center.y).atan2(start.x - center.x), angle)
                    <= *ccw_span + 1e-9
            } else {
                normalize_angle_delta_raw(angle, (start.y - center.y).atan2(start.x - center.x))
                    <= normalize_angle_delta_raw(
                        (end.y - center.y).atan2(end.x - center.x),
                        (start.y - center.y).atan2(start.x - center.x),
                    ) + 1e-9
            };
            on_arc
                || ((point.x - start.x).abs() < 1e-6 && (point.y - start.y).abs() < 1e-6)
                || ((point.x - mid.x).abs() < 1e-6 && (point.y - mid.y).abs() < 1e-6)
                || ((point.x - end.x).abs() < 1e-6 && (point.y - end.y).abs() < 1e-6)
        }
    }
}

pub(super) fn normalize_angle_delta_raw(from: f64, to: f64) -> f64 {
    let tau = std::f64::consts::TAU;
    (to - from).rem_euclid(tau)
}

pub(super) fn resolve_polyline_point(
    points: &[PointRecord],
    segment_index: usize,
    t: f64,
) -> Option<PointRecord> {
    if points.len() < 2 {
        return None;
    }

    let start = &points[segment_index.min(points.len() - 2)];
    let end = &points[(segment_index.min(points.len() - 2)) + 1];
    Some(lerp_point(start, end, t))
}

#[cfg(test)]
mod tests {
    use super::{CircularConstraintRaw, normalize_angle_delta_raw, select_circular_intersection};
    use crate::format::PointRecord;
    use crate::runtime::geometry::three_point_arc_geometry;

    fn arc(start: PointRecord, mid: PointRecord, end: PointRecord) -> CircularConstraintRaw {
        let geometry = three_point_arc_geometry(&start, &mid, &end).expect("valid arc");
        CircularConstraintRaw::ThreePointArc {
            start,
            mid: mid.clone(),
            end,
            center: geometry.center.clone(),
            radius: geometry.radius,
            ccw_span: normalize_angle_delta_raw(geometry.start_angle, geometry.end_angle),
            ccw_mid: normalize_angle_delta_raw(
                geometry.start_angle,
                (mid.y - geometry.center.y).atan2(mid.x - geometry.center.x),
            ),
        }
    }

    #[test]
    fn arc_intersection_returns_none_when_only_parent_circles_intersect() {
        let left = arc(
            PointRecord { x: -1.0, y: 0.0 },
            PointRecord { x: 0.0, y: 1.0 },
            PointRecord { x: 1.0, y: 0.0 },
        );
        let right = arc(
            PointRecord { x: 2.0, y: 0.0 },
            PointRecord { x: 1.0, y: -1.0 },
            PointRecord { x: 0.0, y: 0.0 },
        );

        assert!(
            select_circular_intersection(&left, &right, 0).is_none(),
            "expected no intersection when arc spans do not overlap"
        );
    }
}
