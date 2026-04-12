use std::collections::BTreeSet;

use crate::format::{GspFile, ObjectGroup, PointRecord};
use crate::runtime::geometry::{
    lerp_point, point_on_circle_arc, point_on_three_point_arc, reflect_across_line, rotate_around,
    scale_around,
};
use crate::runtime::scene::{
    CircularConstraint, LineConstraint, LineLikeKind, ScenePoint, ScenePointBinding,
    ScenePointConstraint,
};

use super::find_indexed_path;
use super::points::{custom_transform_expression_parameter_map, custom_transform_trace_parameter};

pub(super) fn collect_point_traces(
    file: &GspFile,
    groups: &[ObjectGroup],
    visible_points: &[ScenePoint],
    group_to_point_index: &[Option<usize>],
) -> Vec<crate::runtime::scene::LineShape> {
    groups
        .iter()
        .filter(|group| {
            matches!(
                group.header.kind(),
                crate::format::GroupKind::PointTrace
                    | crate::format::GroupKind::CustomTransformTrace
            )
        })
        .filter_map(|group| {
            let path = find_indexed_path(file, group)?;
            let target_group_index = path.refs.first()?.checked_sub(1)?;
            let target_point_index = (*group_to_point_index.get(target_group_index)?)?;
            let (driver_point_index, driver_group_index) =
                path.refs.iter().find_map(|ordinal| {
                    let group_index = ordinal.checked_sub(1)?;
                    let point_index = (*group_to_point_index.get(group_index)?)?;
                    let point = visible_points.get(point_index)?;
                    point_accepts_trace_parameter(point).then_some((point_index, group_index))
                })?;
            let payload = group
                .records
                .iter()
                .find(|record| record.record_type == 0x0902)
                .map(|record| record.payload(&file.data))?;
            let descriptor =
                crate::runtime::functions::try_decode_function_plot_descriptor(payload).ok()?;
            let trace_max = if (group.header.kind())
                == crate::format::GroupKind::CustomTransformTrace
            {
                custom_transform_trace_parameter(visible_points.get(driver_point_index)?)?.clamp(
                    descriptor.x_min.min(descriptor.x_max),
                    descriptor.x_min.max(descriptor.x_max),
                )
            } else {
                descriptor.x_max
            };

            let mut points = Vec::with_capacity(descriptor.sample_count);
            let mut previous_points = visible_points.to_vec();
            let last = descriptor.sample_count.saturating_sub(1).max(1) as f64;
            for sample_index in 0..descriptor.sample_count {
                let t = sample_index as f64 / last;
                let parameter = descriptor.x_min + (trace_max - descriptor.x_min) * t;
                let mut sampled_points = previous_points.clone();
                let driver_point = sampled_points.get_mut(driver_point_index)?;
                apply_trace_parameter_with_mode(
                    driver_point,
                    parameter,
                    descriptor.x_min,
                    trace_max,
                    (group.header.kind()) == crate::format::GroupKind::CustomTransformTrace,
                );
                points.push(resolve_trace_point(
                    &mut sampled_points,
                    target_point_index,
                    &mut BTreeSet::new(),
                )?);
                previous_points = sampled_points;
            }

            (points.len() >= 2).then_some(crate::runtime::scene::LineShape {
                points,
                color: crate::runtime::geometry::color_from_style(group.header.style_b),
                dashed: false,
                visible: !group.header.is_hidden(),
                binding: match group.header.kind() {
                    crate::format::GroupKind::CustomTransformTrace => {
                        Some(crate::runtime::scene::LineBinding::CustomTransformTrace {
                            point_index: target_group_index,
                            x_min: descriptor.x_min,
                            x_max: descriptor.x_max,
                            sample_count: descriptor.sample_count,
                        })
                    }
                    crate::format::GroupKind::PointTrace => {
                        Some(crate::runtime::scene::LineBinding::PointTrace {
                            point_index: target_group_index,
                            driver_index: driver_group_index,
                            x_min: descriptor.x_min,
                            x_max: descriptor.x_max,
                            sample_count: descriptor.sample_count,
                        })
                    }
                    _ => None,
                },
            })
        })
        .collect()
}

fn point_accepts_trace_parameter(point: &ScenePoint) -> bool {
    if matches!(point.binding, Some(ScenePointBinding::Midpoint { .. })) {
        return false;
    }
    matches!(
        point.constraint,
        ScenePointConstraint::OnSegment { .. }
            | ScenePointConstraint::OnLine { .. }
            | ScenePointConstraint::OnRay { .. }
            | ScenePointConstraint::OnPolygonBoundary { .. }
            | ScenePointConstraint::OnCircle { .. }
            | ScenePointConstraint::OnCircleArc { .. }
            | ScenePointConstraint::OnArc { .. }
    )
}

fn apply_trace_parameter_with_mode(
    point: &mut ScenePoint,
    value: f64,
    x_min: f64,
    x_max: f64,
    use_raw_value: bool,
) {
    let normalized = if use_raw_value {
        value
    } else if (x_max - x_min).abs() <= 1e-9 {
        0.0
    } else {
        ((value - x_min) / (x_max - x_min)).clamp(0.0, 1.0)
    };
    match &mut point.constraint {
        ScenePointConstraint::OnSegment { t, .. } => {
            *t = normalized;
        }
        ScenePointConstraint::OnLine { t, .. } => {
            *t = value;
        }
        ScenePointConstraint::OnRay { t, .. } => {
            *t = value.max(0.0);
        }
        ScenePointConstraint::OnPolygonBoundary {
            vertex_indices,
            edge_index,
            t,
        } => {
            if vertex_indices.len() < 2 {
                return;
            }
            let scaled = normalized * vertex_indices.len() as f64;
            let next_edge = scaled.floor() as usize;
            *edge_index = next_edge.min(vertex_indices.len() - 1);
            *t = scaled.fract();
        }
        ScenePointConstraint::OnCircle { unit_x, unit_y, .. } => {
            let angle = value;
            *unit_x = angle.cos();
            *unit_y = -angle.sin();
        }
        ScenePointConstraint::OnCircleArc { t, .. } => {
            *t = normalized;
        }
        ScenePointConstraint::OnArc { t, .. } => {
            *t = normalized;
        }
        _ => {}
    }
}

fn resolve_trace_point(
    points: &mut [ScenePoint],
    index: usize,
    visiting: &mut BTreeSet<usize>,
) -> Option<PointRecord> {
    if !visiting.insert(index) {
        return None;
    }

    let point = points.get(index)?.clone();
    let resolved = match &point.binding {
        Some(ScenePointBinding::Translate {
            source_index,
            vector_start_index,
            vector_end_index,
        }) => {
            let source = resolve_trace_point(points, *source_index, visiting)?;
            let vector_start = resolve_trace_point(points, *vector_start_index, visiting)?;
            let vector_end = resolve_trace_point(points, *vector_end_index, visiting)?;
            Some(PointRecord {
                x: source.x + (vector_end.x - vector_start.x),
                y: source.y + (vector_end.y - vector_start.y),
            })
        }
        Some(ScenePointBinding::Reflect {
            source_index,
            line_start_index,
            line_end_index,
        }) => {
            let source = resolve_trace_point(points, *source_index, visiting)?;
            let line_start = resolve_trace_point(points, *line_start_index, visiting)?;
            let line_end = resolve_trace_point(points, *line_end_index, visiting)?;
            reflect_across_line(&source, &line_start, &line_end)
        }
        Some(ScenePointBinding::ReflectLineConstraint { source_index, line }) => {
            let source = resolve_trace_point(points, *source_index, visiting)?;
            let (line_start, line_end, _) = resolve_trace_line_constraint(points, line, visiting)?;
            reflect_across_line(&source, &line_start, &line_end)
        }
        Some(ScenePointBinding::Rotate {
            source_index,
            center_index,
            angle_degrees,
            ..
        }) => {
            let source = resolve_trace_point(points, *source_index, visiting)?;
            let center = resolve_trace_point(points, *center_index, visiting)?;
            Some(rotate_around(&source, &center, angle_degrees.to_radians()))
        }
        Some(ScenePointBinding::Scale {
            source_index,
            center_index,
            factor,
        }) => {
            let source = resolve_trace_point(points, *source_index, visiting)?;
            let center = resolve_trace_point(points, *center_index, visiting)?;
            Some(scale_around(&source, &center, *factor))
        }
        Some(ScenePointBinding::ScaleByRatio {
            source_index,
            center_index,
            ratio_origin_index,
            ratio_denominator_index,
            ratio_numerator_index,
        }) => {
            let source = resolve_trace_point(points, *source_index, visiting)?;
            let center = resolve_trace_point(points, *center_index, visiting)?;
            let ratio_origin = resolve_trace_point(points, *ratio_origin_index, visiting)?;
            let ratio_denominator =
                resolve_trace_point(points, *ratio_denominator_index, visiting)?;
            let ratio_numerator = resolve_trace_point(points, *ratio_numerator_index, visiting)?;
            let denominator =
                (ratio_denominator.x - ratio_origin.x).hypot(ratio_denominator.y - ratio_origin.y);
            if denominator <= 1e-9 {
                return None;
            }
            let numerator =
                (ratio_numerator.x - ratio_origin.x).hypot(ratio_numerator.y - ratio_origin.y);
            Some(scale_around(&source, &center, numerator / denominator))
        }
        Some(ScenePointBinding::Midpoint {
            start_index,
            end_index,
        }) => {
            let start = resolve_trace_point(points, *start_index, visiting)?;
            let end = resolve_trace_point(points, *end_index, visiting)?;
            Some(lerp_point(&start, &end, 0.5))
        }
        Some(ScenePointBinding::CustomTransform {
            source_index,
            origin_index,
            axis_end_index,
            distance_expr,
            angle_expr,
            distance_raw_scale,
            angle_degrees_scale,
        }) => {
            let source_point = points.get(*source_index)?;
            let t = custom_transform_trace_parameter(source_point)?;
            let origin = resolve_trace_point(points, *origin_index, visiting)?;
            let axis_end = resolve_trace_point(points, *axis_end_index, visiting)?;
            let parameters =
                custom_transform_expression_parameter_map(distance_expr, angle_expr, t);
            let distance = crate::runtime::functions::evaluate_expr_with_parameters(
                distance_expr,
                t,
                &parameters,
            )? * distance_raw_scale;
            let angle_degrees = crate::runtime::functions::evaluate_expr_with_parameters(
                angle_expr,
                t,
                &parameters,
            )? * angle_degrees_scale;
            let base_angle = (-(axis_end.y - origin.y))
                .atan2(axis_end.x - origin.x)
                .to_degrees();
            let radians = (base_angle + angle_degrees).to_radians();
            Some(PointRecord {
                x: origin.x + distance * radians.cos(),
                y: origin.y - distance * radians.sin(),
            })
        }
        _ => match &point.constraint {
            ScenePointConstraint::Free => Some(point.position.clone()),
            ScenePointConstraint::Offset {
                origin_index,
                dx,
                dy,
            } => {
                let origin = resolve_trace_point(points, *origin_index, visiting)?;
                Some(PointRecord {
                    x: origin.x + dx,
                    y: origin.y + dy,
                })
            }
            ScenePointConstraint::OnSegment {
                start_index,
                end_index,
                t,
            } => {
                let start = resolve_trace_point(points, *start_index, visiting)?;
                let end = resolve_trace_point(points, *end_index, visiting)?;
                Some(lerp_point(&start, &end, *t))
            }
            ScenePointConstraint::OnLine {
                start_index,
                end_index,
                t,
            } => {
                let start = resolve_trace_point(points, *start_index, visiting)?;
                let end = resolve_trace_point(points, *end_index, visiting)?;
                Some(lerp_point(&start, &end, *t))
            }
            ScenePointConstraint::OnRay {
                start_index,
                end_index,
                t,
            } => {
                let start = resolve_trace_point(points, *start_index, visiting)?;
                let end = resolve_trace_point(points, *end_index, visiting)?;
                Some(lerp_point(&start, &end, *t))
            }
            ScenePointConstraint::OnPolyline {
                points,
                segment_index,
                t,
                ..
            } => {
                if points.len() < 2 {
                    None
                } else {
                    let start = &points[(*segment_index).min(points.len() - 2)];
                    let end = &points[(*segment_index).min(points.len() - 2) + 1];
                    Some(lerp_point(start, end, *t))
                }
            }
            ScenePointConstraint::OnPolygonBoundary {
                vertex_indices,
                edge_index,
                t,
            } => {
                if vertex_indices.len() < 2 {
                    None
                } else {
                    let start = resolve_trace_point(
                        points,
                        vertex_indices[*edge_index % vertex_indices.len()],
                        visiting,
                    )?;
                    let end = resolve_trace_point(
                        points,
                        vertex_indices[(*edge_index + 1) % vertex_indices.len()],
                        visiting,
                    )?;
                    Some(lerp_point(&start, &end, *t))
                }
            }
            ScenePointConstraint::OnCircle {
                center_index,
                radius_index,
                unit_x,
                unit_y,
            } => {
                let center = resolve_trace_point(points, *center_index, visiting)?;
                let radius_point = resolve_trace_point(points, *radius_index, visiting)?;
                let radius = ((radius_point.x - center.x).powi(2)
                    + (radius_point.y - center.y).powi(2))
                .sqrt();
                Some(PointRecord {
                    x: center.x + radius * unit_x,
                    y: center.y + radius * unit_y,
                })
            }
            ScenePointConstraint::OnCircleArc {
                center_index,
                start_index,
                end_index,
                t,
            } => {
                let center = resolve_trace_point(points, *center_index, visiting)?;
                let start = resolve_trace_point(points, *start_index, visiting)?;
                let end = resolve_trace_point(points, *end_index, visiting)?;
                point_on_circle_arc(&center, &start, &end, *t)
            }
            ScenePointConstraint::OnArc {
                start_index,
                mid_index,
                end_index,
                t,
            } => {
                let start = resolve_trace_point(points, *start_index, visiting)?;
                let mid = resolve_trace_point(points, *mid_index, visiting)?;
                let end = resolve_trace_point(points, *end_index, visiting)?;
                point_on_three_point_arc(&start, &mid, &end, *t)
            }
            ScenePointConstraint::LineIntersection { left, right } => {
                let (left_start, left_end, left_kind) =
                    resolve_trace_line_constraint(points, left, visiting)?;
                let (right_start, right_end, right_kind) =
                    resolve_trace_line_constraint(points, right, visiting)?;
                trace_line_line_intersection(
                    &left_start,
                    &left_end,
                    left_kind,
                    &right_start,
                    &right_end,
                    right_kind,
                )
            }
            ScenePointConstraint::LineTraceIntersection { .. } => None,
            ScenePointConstraint::PointCircularTangent {
                point_index,
                circle,
                variant,
            } => {
                let point = resolve_trace_point(points, *point_index, visiting)?;
                let circle = resolve_trace_circular_constraint(points, circle, visiting)?;
                trace_point_circular_tangent(&point, &circle, *variant)
            }
            ScenePointConstraint::LineCircularIntersection {
                line,
                circle,
                variant,
            } => {
                let (line_start, line_end, line_kind) =
                    resolve_trace_line_constraint(points, line, visiting)?;
                let circle = resolve_trace_circular_constraint(points, circle, visiting)?;
                let (center, radius) = trace_circle_center_radius(&circle);
                if radius <= 1e-9 {
                    return None;
                }
                let radius_point = PointRecord {
                    x: center.x + radius,
                    y: center.y,
                };
                trace_line_circle_intersection(
                    &line_start,
                    &line_end,
                    line_kind,
                    &center,
                    &radius_point,
                    *variant,
                    Some(&point.position),
                )
            }
            ScenePointConstraint::LineCircleIntersection {
                line,
                center_index,
                radius_index,
                variant,
            } => {
                let (line_start, line_end, line_kind) =
                    resolve_trace_line_constraint(points, line, visiting)?;
                let center = resolve_trace_point(points, *center_index, visiting)?;
                let radius_point = resolve_trace_point(points, *radius_index, visiting)?;
                trace_line_circle_intersection(
                    &line_start,
                    &line_end,
                    line_kind,
                    &center,
                    &radius_point,
                    *variant,
                    Some(&point.position),
                )
            }
            ScenePointConstraint::CircleCircleIntersection {
                left_center_index,
                left_radius_index,
                right_center_index,
                right_radius_index,
                variant,
            } => {
                let left_center = resolve_trace_point(points, *left_center_index, visiting)?;
                let left_radius = resolve_trace_point(points, *left_radius_index, visiting)?;
                let right_center = resolve_trace_point(points, *right_center_index, visiting)?;
                let right_radius = resolve_trace_point(points, *right_radius_index, visiting)?;
                trace_circle_circle_intersection(
                    &left_center,
                    &left_radius,
                    &right_center,
                    &right_radius,
                    *variant,
                    Some(&point.position),
                )
            }
            ScenePointConstraint::CircularIntersection {
                left,
                right,
                variant,
            } => {
                let left = resolve_trace_circular_constraint(points, left, visiting)?;
                let right = resolve_trace_circular_constraint(points, right, visiting)?;
                trace_circular_intersection(&left, &right, *variant, Some(&point.position))
            }
        },
    };

    visiting.remove(&index);
    if let Some(resolved_point) = resolved.clone()
        && let Some(point) = points.get_mut(index)
    {
        point.position = resolved_point;
    }
    resolved
}

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
        LineConstraint::PerpendicularLine {
            through_index,
            line_start_index,
            line_end_index,
        } => {
            let through = resolve_trace_point(points, *through_index, visiting)?;
            let host_start = resolve_trace_point(points, *line_start_index, visiting)?;
            let host_end = resolve_trace_point(points, *line_end_index, visiting)?;
            let dx = host_end.x - host_start.x;
            let dy = host_end.y - host_start.y;
            let len = (dx * dx + dy * dy).sqrt();
            (len > 1e-9).then_some((
                through.clone(),
                PointRecord {
                    x: through.x - dy / len,
                    y: through.y + dx / len,
                },
                LineLikeKind::Line,
            ))
        }
        LineConstraint::ParallelLine {
            through_index,
            line_start_index,
            line_end_index,
        } => {
            let through = resolve_trace_point(points, *through_index, visiting)?;
            let host_start = resolve_trace_point(points, *line_start_index, visiting)?;
            let host_end = resolve_trace_point(points, *line_end_index, visiting)?;
            let dx = host_end.x - host_start.x;
            let dy = host_end.y - host_start.y;
            let len = (dx * dx + dy * dy).sqrt();
            (len > 1e-9).then_some((
                through.clone(),
                PointRecord {
                    x: through.x + dx / len,
                    y: through.y + dy / len,
                },
                LineLikeKind::Line,
            ))
        }
        LineConstraint::AngleBisectorRay {
            start_index,
            vertex_index,
            end_index,
        } => {
            let start = resolve_trace_point(points, *start_index, visiting)?;
            let vertex = resolve_trace_point(points, *vertex_index, visiting)?;
            let end = resolve_trace_point(points, *end_index, visiting)?;
            let first_dx = start.x - vertex.x;
            let first_dy = start.y - vertex.y;
            let second_dx = end.x - vertex.x;
            let second_dy = end.y - vertex.y;
            let first_len = (first_dx * first_dx + first_dy * first_dy).sqrt();
            let second_len = (second_dx * second_dx + second_dy * second_dy).sqrt();
            if first_len <= 1e-9 || second_len <= 1e-9 {
                return None;
            }
            let sum_x = first_dx / first_len + second_dx / second_len;
            let sum_y = first_dy / first_len + second_dy / second_len;
            let sum_len = (sum_x * sum_x + sum_y * sum_y).sqrt();
            let (dir_x, dir_y) = if sum_len > 1e-9 {
                (sum_x / sum_len, sum_y / sum_len)
            } else {
                (-first_dy / first_len, first_dx / first_len)
            };
            Some((
                vertex.clone(),
                PointRecord {
                    x: vertex.x + dir_x,
                    y: vertex.y + dir_y,
                },
                LineLikeKind::Ray,
            ))
        }
        LineConstraint::Translated {
            line,
            vector_start_index,
            vector_end_index,
        } => {
            let (start, end, kind) = resolve_trace_line_constraint(points, line, visiting)?;
            let vector_start = resolve_trace_point(points, *vector_start_index, visiting)?;
            let vector_end = resolve_trace_point(points, *vector_end_index, visiting)?;
            let dx = vector_end.x - vector_start.x;
            let dy = vector_end.y - vector_start.y;
            Some((
                PointRecord {
                    x: start.x + dx,
                    y: start.y + dy,
                },
                PointRecord {
                    x: end.x + dx,
                    y: end.y + dy,
                },
                kind,
            ))
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
    (trace_line_like_contains(left_start, left_end, left_kind, &point)
        && trace_line_like_contains(right_start, right_end, right_kind, &point))
    .then_some(point)
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
    let dx = line_end.x - line_start.x;
    let dy = line_end.y - line_start.y;
    let a = dx * dx + dy * dy;
    if a <= 1e-9 {
        return None;
    }
    let radius = ((radius_point.x - center.x).powi(2) + (radius_point.y - center.y).powi(2)).sqrt();
    if radius <= 1e-9 {
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
    let mut ts = [(-b - root) / (2.0 * a), (-b + root) / (2.0 * a)]
        .into_iter()
        .filter(|t| trace_param_in_line_like(*t, line_kind))
        .collect::<Vec<_>>();
    if ts.is_empty() {
        return None;
    }
    ts.sort_by(|left, right| left.total_cmp(right));
    let candidates = ts
        .into_iter()
        .map(|t| PointRecord {
            x: line_start.x + dx * t,
            y: line_start.y + dy * t,
        })
        .collect::<Vec<_>>();
    choose_trace_candidate(&candidates, reference, variant)
}

fn trace_line_like_contains(
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
    trace_param_in_line_like(t, kind)
}

fn trace_param_in_line_like(t: f64, kind: LineLikeKind) -> bool {
    match kind {
        LineLikeKind::Line => true,
        LineLikeKind::Ray => t >= -1e-9,
        LineLikeKind::Segment => (-1e-9..=1.0 + 1e-9).contains(&t),
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
    if left_radius <= 1e-9 || right_radius <= 1e-9 {
        return None;
    }
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
    let mut ordered = [
        PointRecord {
            x: base.x - height * uy,
            y: base.y + height * ux,
        },
        PointRecord {
            x: base.x + height * uy,
            y: base.y - height * ux,
        },
    ];
    ordered.sort_by(|left, right| {
        left.y
            .total_cmp(&right.y)
            .then_with(|| left.x.total_cmp(&right.x))
    });
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
    candidates
        .get(variant.min(candidates.len().saturating_sub(1)))
        .cloned()
}

fn trace_circle_circle_intersections(
    left: &TraceCircularConstraint,
    right: &TraceCircularConstraint,
) -> Option<Vec<PointRecord>> {
    let (left_center, left_radius) = trace_circle_center_radius(left);
    let (right_center, right_radius) = trace_circle_center_radius(right);
    if left_radius <= 1e-9 || right_radius <= 1e-9 {
        return None;
    }
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
    let dx = point.x - center.x;
    let dy = point.y - center.y;
    let distance_sq = dx * dx + dy * dy;
    if distance_sq <= radius * radius + 1e-9 {
        return None;
    }
    let distance = distance_sq.sqrt();
    let base_angle = dy.atan2(dx);
    let offset = (radius / distance).acos();
    let mut tangents = [
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
        .filter(|candidate| trace_point_on_circular_constraint(candidate, circle))
        .nth(variant.min(1))
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
