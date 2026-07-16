fn resolve_trace_line_constraint(
    points: &mut [ScenePoint],
    constraint: &LineConstraint,
    visiting: &mut BTreeSet<usize>,
) -> Option<(PointRecord, PointRecord, LineLikeKind)> {
    match constraint {
        LineConstraint::Segment {
            start_index,
            end_index,
        } => Some((
            resolve_trace_point(points, *start_index, visiting)?,
            resolve_trace_point(points, *end_index, visiting)?,
            LineLikeKind::Segment,
        )),
        LineConstraint::Line {
            start_index,
            end_index,
        } => Some((
            resolve_trace_point(points, *start_index, visiting)?,
            resolve_trace_point(points, *end_index, visiting)?,
            LineLikeKind::Line,
        )),
        LineConstraint::Ray {
            start_index,
            end_index,
        } => Some((
            resolve_trace_point(points, *start_index, visiting)?,
            resolve_trace_point(points, *end_index, visiting)?,
            LineLikeKind::Ray,
        )),
        LineConstraint::AngleBisectorRay {
            start_index,
            vertex_index,
            end_index,
        } => {
            let start = resolve_trace_point(points, *start_index, visiting)?;
            let vertex = resolve_trace_point(points, *vertex_index, visiting)?;
            let end = resolve_trace_point(points, *end_index, visiting)?;
            let direction = gsp_runtime_core::angle_bisector_direction(
                to_core_point(&start),
                to_core_point(&vertex),
                to_core_point(&end),
            )?;
            Some((
                vertex.clone(),
                PointRecord {
                    x: vertex.x + direction.x,
                    y: vertex.y + direction.y,
                },
                LineLikeKind::Ray,
            ))
        }
        LineConstraint::MatrixApply { source, matrices } => {
            let (start, end, mut kind) = resolve_trace_line_constraint(points, source, visiting)?;
            let mut start = to_core_point(&start);
            let mut end = to_core_point(&end);
            for matrix in matrices {
                let matrix = match matrix {
                    crate::runtime::scene::LineConstraintMatrix::TranslateVector {
                        vector_start_index,
                        vector_end_index,
                    } => {
                        let vector_start =
                            resolve_trace_point(points, *vector_start_index, visiting)?;
                        let vector_end = resolve_trace_point(points, *vector_end_index, visiting)?;
                        gsp_runtime_core::AffineMatrix::translation(
                            vector_end.x - vector_start.x,
                            vector_end.y - vector_start.y,
                        )
                    }
                    crate::runtime::scene::LineConstraintMatrix::TranslateDelta { dx, dy } => {
                        gsp_runtime_core::AffineMatrix::translation(*dx, *dy)
                    }
                    crate::runtime::scene::LineConstraintMatrix::Reflect { axis } => {
                        let (axis_start, axis_end, _) =
                            resolve_trace_line_constraint(points, axis, visiting)?;
                        gsp_runtime_core::AffineMatrix::reflection(
                            to_core_point(&axis_start),
                            to_core_point(&axis_end),
                        )?
                    }
                    crate::runtime::scene::LineConstraintMatrix::Rotate { rotation } => {
                        let center =
                            resolve_trace_point(points, rotation.center_index, visiting)?;
                        let angle_degrees = if let (
                            Some(start_index),
                            Some(vertex_index),
                            Some(end_index),
                        ) = (
                            rotation.angle_start_index,
                            rotation.angle_vertex_index,
                            rotation.angle_end_index,
                        ) {
                            let angle_start =
                                resolve_trace_point(points, start_index, visiting)?;
                            let angle_vertex =
                                resolve_trace_point(points, vertex_index, visiting)?;
                            let angle_end = resolve_trace_point(points, end_index, visiting)?;
                            crate::runtime::geometry::angle_degrees_from_points(
                                &angle_start,
                                &angle_vertex,
                                &angle_end,
                            )?
                        } else {
                            rotation.angle_degrees
                        };
                        gsp_runtime_core::AffineMatrix::rotation(
                            to_core_point(&center),
                            angle_degrees.to_radians(),
                        )
                    }
                    crate::runtime::scene::LineConstraintMatrix::RotateAroundSourcePoint {
                        source_point_index,
                        angle_degrees,
                    } => {
                        kind = LineLikeKind::Line;
                        let center = [start, end].get(*source_point_index).copied()?;
                        gsp_runtime_core::AffineMatrix::rotation(
                            center,
                            angle_degrees.to_radians(),
                        )
                    }
                    crate::runtime::scene::LineConstraintMatrix::TranslateSourcePointToPoint {
                        source_point_index,
                        target_index,
                    } => {
                        kind = LineLikeKind::Line;
                        let source = [start, end].get(*source_point_index).copied()?;
                        let target = resolve_trace_point(points, *target_index, visiting)?;
                        gsp_runtime_core::AffineMatrix::translation(
                            target.x - source.x,
                            target.y - source.y,
                        )
                    }
                };
                start = matrix.apply(start);
                end = matrix.apply(end);
            }
            Some((from_core_point(start), from_core_point(end), kind))
        }
    }
}

fn trace_line_line_intersection(
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
        trace_core_line_kind(left_kind),
        to_core_point(right_start),
        to_core_point(right_end),
        trace_core_line_kind(right_kind),
    )
    .map(from_core_point)
}

fn trace_line_circle_intersection(
    line_start: &PointRecord,
    line_end: &PointRecord,
    line_kind: LineLikeKind,
    center: &PointRecord,
    radius_point: &PointRecord,
    variant: usize,
    reference: Option<&PointRecord>,
) -> Option<PointRecord> {
    let radius = ((radius_point.x - center.x).powi(2) + (radius_point.y - center.y).powi(2)).sqrt();
    let candidates = gsp_runtime_core::line_circle_intersections(
        to_core_point(line_start),
        to_core_point(line_end),
        gsp_runtime_core::LineKind::Line,
        to_core_point(center),
        radius,
    )
        .into_iter()
        .map(from_core_point)
        .collect::<Vec<_>>();
    let _ = reference;
    let selected = choose_trace_candidate(&candidates, None, variant)?;
    trace_point_lies_on_line_kind(&selected, line_start, line_end, line_kind).then_some(selected)
}

fn trace_point_lies_on_line_kind(
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

fn trace_core_line_kind(kind: LineLikeKind) -> gsp_runtime_core::LineKind {
    match kind {
        LineLikeKind::Line => gsp_runtime_core::LineKind::Line,
        LineLikeKind::Ray => gsp_runtime_core::LineKind::Ray,
        LineLikeKind::Segment => gsp_runtime_core::LineKind::Segment,
    }
}

fn trace_circle_circle_intersection(
    left_center: &PointRecord,
    left_radius_point: &PointRecord,
    right_center: &PointRecord,
    right_radius_point: &PointRecord,
    variant: usize,
    reference: Option<&PointRecord>,
) -> Option<PointRecord> {
    let left_radius = ((left_radius_point.x - left_center.x).powi(2)
        + (left_radius_point.y - left_center.y).powi(2))
    .sqrt();
    let right_radius = ((right_radius_point.x - right_center.x).powi(2)
        + (right_radius_point.y - right_center.y).powi(2))
    .sqrt();
    let ordered = gsp_runtime_core::circle_circle_intersections(
        to_core_point(left_center),
        left_radius,
        to_core_point(right_center),
        right_radius,
    )
    .into_iter()
    .map(from_core_point)
    .collect::<Vec<_>>();
    choose_trace_candidate(&ordered, reference, variant)
}

#[derive(Clone)]
enum TraceCircularConstraint {
    Circle {
        center: PointRecord,
        radius: f64,
    },
    ThreePointArc {
        start: PointRecord,
        end: PointRecord,
        center: PointRecord,
        radius: f64,
        start_angle: f64,
        end_angle: f64,
        ccw_span: f64,
        ccw_mid: f64,
    },
}

fn resolve_trace_circular_constraint(
    points: &mut [ScenePoint],
    constraint: &CircularConstraint,
    visiting: &mut BTreeSet<usize>,
) -> Option<TraceCircularConstraint> {
    match constraint {
        CircularConstraint::Circle {
            center_index,
            radius_index,
        } => {
            let center = resolve_trace_point(points, *center_index, visiting)?;
            let radius_point = resolve_trace_point(points, *radius_index, visiting)?;
            let radius =
                ((radius_point.x - center.x).powi(2) + (radius_point.y - center.y).powi(2)).sqrt();
            (radius > 1e-9).then_some(TraceCircularConstraint::Circle { center, radius })
        }
        CircularConstraint::SegmentRadiusCircle {
            center_index,
            line_start_index,
            line_end_index,
        } => {
            let center = resolve_trace_point(points, *center_index, visiting)?;
            let line_start = resolve_trace_point(points, *line_start_index, visiting)?;
            let line_end = resolve_trace_point(points, *line_end_index, visiting)?;
            let radius =
                ((line_end.x - line_start.x).powi(2) + (line_end.y - line_start.y).powi(2)).sqrt();
            (radius > 1e-9).then_some(TraceCircularConstraint::Circle { center, radius })
        }
        CircularConstraint::ParameterRadiusCircle {
            center_index,
            parameter_value,
            raw_per_unit,
            ..
        } => {
            let center = resolve_trace_point(points, *center_index, visiting)?;
            let radius = parameter_value.abs() * raw_per_unit;
            (radius > 1e-9).then_some(TraceCircularConstraint::Circle { center, radius })
        }
        CircularConstraint::ExpressionRadiusCircle {
            center_index,
            initial_value,
            ..
        } => {
            let center = resolve_trace_point(points, *center_index, visiting)?;
            let radius = initial_value.abs();
            (radius > 1e-9).then_some(TraceCircularConstraint::Circle { center, radius })
        }
        CircularConstraint::MatrixApply { source, matrices } => {
            let mut resolved = resolve_trace_circular_constraint(points, source, visiting)?;
            for matrix in matrices {
                let matrix = trace_geometry_matrix(points, matrix, visiting)?;
                resolved = transform_trace_circular_constraint(resolved, matrix)?;
            }
            Some(resolved)
        }
        CircularConstraint::CircleArc {
            center_index,
            start_index,
            end_index,
        } => {
            let center = resolve_trace_point(points, *center_index, visiting)?;
            let start = resolve_trace_point(points, *start_index, visiting)?;
            let end = resolve_trace_point(points, *end_index, visiting)?;
            let controls =
                crate::runtime::geometry::arc_on_circle_control_points(&center, &start, &end)?;
            let start = controls[0].clone();
            let mid = controls[1].clone();
            let end = controls[2].clone();
            let radius = ((start.x - center.x).powi(2) + (start.y - center.y).powi(2)).sqrt();
            let start_angle = (start.y - center.y).atan2(start.x - center.x);
            let end_angle = (end.y - center.y).atan2(end.x - center.x);
            let ccw_mid = trace_normalized_angle_delta(
                start_angle,
                (mid.y - center.y).atan2(mid.x - center.x),
            );
            Some(TraceCircularConstraint::ThreePointArc {
                start,
                end,
                center,
                radius,
                start_angle,
                end_angle,
                ccw_span: trace_normalized_angle_delta(start_angle, end_angle),
                ccw_mid,
            })
        }
        CircularConstraint::ThreePointArc {
            start_index,
            mid_index,
            end_index,
        } => {
            let start = resolve_trace_point(points, *start_index, visiting)?;
            let mid = resolve_trace_point(points, *mid_index, visiting)?;
            let end = resolve_trace_point(points, *end_index, visiting)?;
            let geometry = crate::runtime::geometry::three_point_arc_geometry(&start, &mid, &end)?;
            let center = geometry.center.clone();
            Some(TraceCircularConstraint::ThreePointArc {
                start,
                end,
                center: center.clone(),
                radius: geometry.radius,
                start_angle: geometry.start_angle,
                end_angle: geometry.end_angle,
                ccw_span: trace_normalized_angle_delta(geometry.start_angle, geometry.end_angle),
                ccw_mid: trace_normalized_angle_delta(
                    geometry.start_angle,
                    (mid.y - center.y).atan2(mid.x - center.x),
                ),
            })
        }
    }
}

fn trace_geometry_matrix(
    points: &mut [ScenePoint],
    transform: &crate::runtime::scene::GeometryTransformBinding,
    visiting: &mut BTreeSet<usize>,
) -> Option<gsp_runtime_core::AffineMatrix> {
    use crate::runtime::scene::GeometryTransformBinding;
    Some(match transform {
        GeometryTransformBinding::TranslateDelta { dx, dy } => {
            gsp_runtime_core::AffineMatrix::translation(*dx, *dy)
        }
        GeometryTransformBinding::TranslateVector {
            vector_start_index,
            vector_end_index,
        } => {
            let start = resolve_trace_point(points, *vector_start_index, visiting)?;
            let end = resolve_trace_point(points, *vector_end_index, visiting)?;
            gsp_runtime_core::AffineMatrix::translation(end.x - start.x, end.y - start.y)
        }
        GeometryTransformBinding::Rotate(rotation) => {
            let center = resolve_trace_point(points, rotation.center_index, visiting)?;
            let angle_degrees = if let (Some(start), Some(vertex), Some(end)) = (
                rotation.angle_start_index,
                rotation.angle_vertex_index,
                rotation.angle_end_index,
            ) {
                crate::runtime::geometry::angle_degrees_from_points(
                    &resolve_trace_point(points, start, visiting)?,
                    &resolve_trace_point(points, vertex, visiting)?,
                    &resolve_trace_point(points, end, visiting)?,
                )?
            } else {
                rotation.angle_degrees
            };
            gsp_runtime_core::AffineMatrix::rotation(
                to_core_point(&center),
                angle_degrees.to_radians(),
            )
        }
        GeometryTransformBinding::Scale(scale) => gsp_runtime_core::AffineMatrix::scale(
            to_core_point(&resolve_trace_point(points, scale.center_index, visiting)?),
            scale.factor,
        ),
        GeometryTransformBinding::ScaleByRatio(scale) => {
            let center = resolve_trace_point(points, scale.center_index, visiting)?;
            let center = to_core_point(&center);
            let probe = gsp_runtime_core::Point {
                x: center.x + 1.0,
                y: center.y,
            };
            let scaled = gsp_runtime_core::scale_by_three_point_ratio(
                probe,
                center,
                to_core_point(&resolve_trace_point(points, scale.ratio_origin_index, visiting)?),
                to_core_point(&resolve_trace_point(
                    points,
                    scale.ratio_denominator_index,
                    visiting,
                )?),
                to_core_point(&resolve_trace_point(
                    points,
                    scale.ratio_numerator_index,
                    visiting,
                )?),
                scale.signed,
                scale.clamp_to_unit,
            )?;
            gsp_runtime_core::AffineMatrix::scale(center, scaled.x - center.x)
        }
        GeometryTransformBinding::Reflect(axis) => {
            let start = resolve_trace_point(points, axis.line_start_index?, visiting)?;
            let end = resolve_trace_point(points, axis.line_end_index?, visiting)?;
            gsp_runtime_core::AffineMatrix::reflection(
                to_core_point(&start),
                to_core_point(&end),
            )?
        }
        GeometryTransformBinding::RotateAroundSourcePoint { .. }
        | GeometryTransformBinding::TranslateSourcePointToPoint { .. } => return None,
    })
}

fn transform_trace_circular_constraint(
    source: TraceCircularConstraint,
    matrix: gsp_runtime_core::AffineMatrix,
) -> Option<TraceCircularConstraint> {
    match source {
        TraceCircularConstraint::Circle { center, radius } => {
            let center = to_core_point(&center);
            let radius_point = gsp_runtime_core::Point {
                x: center.x + radius,
                y: center.y,
            };
            let transformed_center = matrix.apply(center);
            let transformed_radius = matrix.apply(radius_point);
            Some(TraceCircularConstraint::Circle {
                center: from_core_point(transformed_center),
                radius: (transformed_radius.x - transformed_center.x)
                    .hypot(transformed_radius.y - transformed_center.y),
            })
        }
        TraceCircularConstraint::ThreePointArc {
            start,
            end,
            center,
            radius,
            start_angle,
            ccw_mid,
            ..
        } => {
            let mid = PointRecord {
                x: center.x + radius * (start_angle + ccw_mid).cos(),
                y: center.y + radius * (start_angle + ccw_mid).sin(),
            };
            let start = from_core_point(matrix.apply(to_core_point(&start)));
            let mid = from_core_point(matrix.apply(to_core_point(&mid)));
            let end = from_core_point(matrix.apply(to_core_point(&end)));
            let geometry = crate::runtime::geometry::three_point_arc_geometry(&start, &mid, &end)?;
            let center = geometry.center.clone();
            Some(TraceCircularConstraint::ThreePointArc {
                start,
                end,
                center: center.clone(),
                radius: geometry.radius,
                start_angle: geometry.start_angle,
                end_angle: geometry.end_angle,
                ccw_span: trace_normalized_angle_delta(
                    geometry.start_angle,
                    geometry.end_angle,
                ),
                ccw_mid: trace_normalized_angle_delta(
                    geometry.start_angle,
                    (mid.y - center.y).atan2(mid.x - center.x),
                ),
            })
        }
    }
}

fn trace_circular_intersection(
    left: &TraceCircularConstraint,
    right: &TraceCircularConstraint,
    variant: usize,
    reference: Option<&PointRecord>,
) -> Option<PointRecord> {
    let intersections = trace_circle_circle_intersections(left, right)?;
    let on_both = intersections
        .iter()
        .filter(|point| trace_point_on_circular_constraint(point, left))
        .filter(|point| trace_point_on_circular_constraint(point, right))
        .cloned()
        .collect::<Vec<_>>();
    choose_trace_candidate(&on_both, reference, variant)
}

fn choose_trace_candidate(
    candidates: &[PointRecord],
    reference: Option<&PointRecord>,
    variant: usize,
) -> Option<PointRecord> {
    if candidates.is_empty() {
        return None;
    }
    candidates.get(variant)?;
    if let Some(reference) = reference {
        return candidates
            .iter()
            .min_by(|left, right| {
                let left_distance = (left.x - reference.x).powi(2) + (left.y - reference.y).powi(2);
                let right_distance =
                    (right.x - reference.x).powi(2) + (right.y - reference.y).powi(2);
                left_distance.total_cmp(&right_distance)
            })
            .cloned();
    }
    candidates.get(variant).cloned()
}

fn trace_circle_circle_intersections(
    left: &TraceCircularConstraint,
    right: &TraceCircularConstraint,
) -> Option<Vec<PointRecord>> {
    let (left_center, left_radius) = trace_circle_center_radius(left);
    let (right_center, right_radius) = trace_circle_center_radius(right);
    let intersections = gsp_runtime_core::circle_circle_intersections(
        to_core_point(&left_center),
        left_radius,
        to_core_point(&right_center),
        right_radius,
    )
    .into_iter()
    .map(from_core_point)
    .collect::<Vec<_>>();
    (!intersections.is_empty()).then_some(intersections)
}

fn trace_circle_center_radius(constraint: &TraceCircularConstraint) -> (PointRecord, f64) {
    match constraint {
        TraceCircularConstraint::Circle { center, radius }
        | TraceCircularConstraint::ThreePointArc { center, radius, .. } => {
            (center.clone(), *radius)
        }
    }
}

fn trace_point_circular_tangent(
    point: &PointRecord,
    circle: &TraceCircularConstraint,
    variant: usize,
) -> Option<PointRecord> {
    let (center, radius) = trace_circle_center_radius(circle);
    gsp_runtime_core::point_circle_tangents(to_core_point(point), to_core_point(&center), radius)
        .into_iter()
        .map(from_core_point)
        .filter(|candidate| trace_point_on_circular_constraint(candidate, circle))
        .nth(variant)
}

fn trace_point_on_circular_constraint(
    point: &PointRecord,
    constraint: &TraceCircularConstraint,
) -> bool {
    match constraint {
        TraceCircularConstraint::Circle { .. } => true,
        TraceCircularConstraint::ThreePointArc {
            start,
            end,
            center,
            radius,
            start_angle,
            end_angle,
            ccw_span,
            ccw_mid,
        } => {
            let radial = ((point.x - center.x).powi(2) + (point.y - center.y).powi(2)).sqrt();
            if (radial - radius).abs() > 1e-6 {
                return false;
            }
            let angle = (point.y - center.y).atan2(point.x - center.x);
            if *ccw_mid <= *ccw_span + 1e-9 {
                return trace_normalized_angle_delta(*start_angle, angle) <= *ccw_span + 1e-9;
            }
            trace_normalized_angle_delta(angle, *start_angle)
                <= trace_normalized_angle_delta(*end_angle, *start_angle) + 1e-9
                || ((point.x - start.x).abs() < 1e-6 && (point.y - start.y).abs() < 1e-6)
                || ((point.x - end.x).abs() < 1e-6 && (point.y - end.y).abs() < 1e-6)
        }
    }
}

fn trace_normalized_angle_delta(from: f64, to: f64) -> f64 {
    (to - from).rem_euclid(std::f64::consts::TAU)
}

#[cfg(test)]
mod tests {
    use super::choose_trace_candidate;
    use crate::format::PointRecord;

    #[test]
    fn candidate_selection_uses_reference_or_rejects_an_invalid_variant() {
        let candidates = [
            PointRecord { x: -1.0, y: 0.0 },
            PointRecord { x: 1.0, y: 0.0 },
        ];
        assert!(choose_trace_candidate(&candidates, None, 2).is_none());
        let selected = choose_trace_candidate(
            &candidates,
            Some(&PointRecord { x: 0.8, y: 0.0 }),
            0,
        )
        .expect("reference should select the nearest branch");
        assert_eq!(selected.x, candidates[1].x);
        assert_eq!(selected.y, candidates[1].y);
    }
}
