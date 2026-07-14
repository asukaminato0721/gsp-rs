use crate::format::PointRecord;
use crate::runtime::geometry::{from_core_point, lerp_point, to_core_point};
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
    pub(crate) fn center(&self) -> PointRecord {
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

pub(super) fn select_line_circle_intersection(
    line_start: PointRecord,
    line_end: PointRecord,
    line_kind: LineLikeKind,
    center: PointRecord,
    radius: f64,
    variant: usize,
) -> Option<PointRecord> {
    let intersections = gsp_runtime_core::line_circle_intersections(
        to_core_point(&line_start),
        to_core_point(&line_end),
        gsp_runtime_core::LineKind::Line,
        to_core_point(&center),
        radius,
    );
    let selected = intersections.get(variant).copied().map(from_core_point)?;
    point_lies_on_line_kind(&selected, &line_start, &line_end, line_kind).then_some(selected)
}

fn point_lies_on_line_kind(
    point: &PointRecord,
    start: &PointRecord,
    end: &PointRecord,
    kind: LineLikeKind,
) -> bool {
    if matches!(kind, LineLikeKind::Line) {
        return true;
    }
    let dx = end.x - start.x;
    let dy = end.y - start.y;
    let length_squared = dx * dx + dy * dy;
    if length_squared <= 1e-18 {
        return false;
    }
    let t = ((point.x - start.x) * dx + (point.y - start.y) * dy) / length_squared;
    match kind {
        LineLikeKind::Line => true,
        LineLikeKind::Ray => t >= -1e-9,
        LineLikeKind::Segment => (-1e-9..=1.0 + 1e-9).contains(&t),
    }
}

pub(super) fn line_line_intersection(
    left_start: &PointRecord,
    left_end: &PointRecord,
    left_kind: LineLikeKind,
    right_start: &PointRecord,
    right_end: &PointRecord,
    right_kind: LineLikeKind,
) -> Option<PointRecord> {
    gsp_runtime_core::line_line_intersection(
        to_core_point(left_start),
        to_core_point(left_end),
        to_core_line_kind(left_kind),
        to_core_point(right_start),
        to_core_point(right_end),
        to_core_line_kind(right_kind),
    )
    .map(from_core_point)
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
    on_both.get(variant).cloned()
}

pub(super) fn select_point_circle_tangent(
    point: &PointRecord,
    circle: &CircularConstraintRaw,
    variant: usize,
) -> Option<PointRecord> {
    let center = circle.center();
    let radius = circle.radius();
    gsp_runtime_core::point_circle_tangents(to_core_point(point), to_core_point(&center), radius)
        .into_iter()
        .map(from_core_point)
        .filter(|candidate| point_lies_on_circular_constraint(candidate, circle))
        .nth(variant)
}

fn circle_circle_intersections(
    left_center: &PointRecord,
    left_radius: f64,
    right_center: &PointRecord,
    right_radius: f64,
) -> Option<Vec<PointRecord>> {
    let intersections = gsp_runtime_core::circle_circle_intersections(
        to_core_point(left_center),
        left_radius,
        to_core_point(right_center),
        right_radius,
    )
    .into_iter()
    .map(from_core_point)
    .collect::<Vec<_>>();
    (!intersections.is_empty()).then_some(intersections)
}

fn to_core_line_kind(kind: LineLikeKind) -> gsp_runtime_core::LineKind {
    match kind {
        LineLikeKind::Segment => gsp_runtime_core::LineKind::Segment,
        LineLikeKind::Line => gsp_runtime_core::LineKind::Line,
        LineLikeKind::Ray => gsp_runtime_core::LineKind::Ray,
    }
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
    use super::{
        CircularConstraintRaw, normalize_angle_delta_raw, select_circular_intersection,
        select_line_circle_intersection, select_point_circle_tangent,
    };
    use crate::format::PointRecord;
    use crate::runtime::geometry::three_point_arc_geometry;
    use crate::runtime::scene::LineLikeKind;

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

    #[test]
    fn intersection_variants_outside_the_payload_defined_candidates_are_rejected() {
        let left = CircularConstraintRaw::Circle {
            center: PointRecord { x: 0.0, y: 0.0 },
            radius: 2.0,
        };
        let right = CircularConstraintRaw::Circle {
            center: PointRecord { x: 2.0, y: 0.0 },
            radius: 2.0,
        };
        assert!(select_circular_intersection(&left, &right, 2).is_none());

        assert!(
            select_line_circle_intersection(
                PointRecord { x: -3.0, y: 0.0 },
                PointRecord { x: 3.0, y: 0.0 },
                LineLikeKind::Line,
                PointRecord { x: 0.0, y: 0.0 },
                2.0,
                2,
            )
            .is_none()
        );

        assert!(select_point_circle_tangent(&PointRecord { x: 3.0, y: 0.0 }, &left, 2,).is_none());
    }
}
