use std::collections::BTreeSet;

use crate::format::{GspFile, ObjectGroup, PointRecord};
use crate::runtime::geometry::{
    lerp_point, point_on_circle_arc, point_on_three_point_arc, reflect_across_line, rotate_around,
    scale_around,
};
use crate::runtime::scene::{
    LineConstraint, LineLikeKind, ScenePoint, ScenePointBinding, ScenePointConstraint,
};

use super::find_indexed_path;
use super::points::{
    custom_transform_expression_parameter_map, custom_transform_trace_parameter,
};

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
            let driver_point_index = path.refs.iter().find_map(|ordinal| {
                let group_index = ordinal.checked_sub(1)?;
                let point_index = (*group_to_point_index.get(group_index)?)?;
                let point = visible_points.get(point_index)?;
                point_accepts_trace_parameter(point).then_some(point_index)
            })?;
            let payload = group
                .records
                .iter()
                .find(|record| record.record_type == 0x0902)
                .map(|record| record.payload(&file.data))?;
            let descriptor = crate::runtime::functions::decode_function_plot_descriptor(payload)?;

            let mut points = Vec::with_capacity(descriptor.sample_count);
            let last = descriptor.sample_count.saturating_sub(1).max(1) as f64;
            for sample_index in 0..descriptor.sample_count {
                let t = sample_index as f64 / last;
                let parameter = descriptor.x_min + (descriptor.x_max - descriptor.x_min) * t;
                let mut sampled_points = visible_points.to_vec();
                let driver_point = sampled_points.get_mut(driver_point_index)?;
                apply_trace_parameter(driver_point, parameter);
                points.push(resolve_trace_point(
                    &sampled_points,
                    target_point_index,
                    &mut BTreeSet::new(),
                )?);
            }

            (points.len() >= 2).then_some(crate::runtime::scene::LineShape {
                points,
                color: crate::runtime::geometry::color_from_style(group.header.style_b),
                dashed: false,
                binding: if (group.header.kind()) == crate::format::GroupKind::CustomTransformTrace {
                    Some(crate::runtime::scene::LineBinding::CustomTransformTrace {
                        point_index: target_group_index,
                        x_min: descriptor.x_min,
                        x_max: descriptor.x_max,
                        sample_count: descriptor.sample_count,
                    })
                } else {
                    None
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
            | ScenePointConstraint::OnPolygonBoundary { .. }
            | ScenePointConstraint::OnCircle { .. }
            | ScenePointConstraint::OnCircleArc { .. }
            | ScenePointConstraint::OnArc { .. }
    )
}

fn apply_trace_parameter(point: &mut ScenePoint, value: f64) {
    let clamped = value.clamp(0.0, 1.0);
    match &mut point.constraint {
        ScenePointConstraint::OnSegment { t, .. } => {
            *t = clamped;
        }
        ScenePointConstraint::OnPolygonBoundary {
            vertex_indices,
            edge_index,
            t,
        } => {
            if vertex_indices.len() < 2 {
                return;
            }
            let scaled = clamped * vertex_indices.len() as f64;
            let next_edge = scaled.floor() as usize;
            *edge_index = next_edge.min(vertex_indices.len() - 1);
            *t = scaled.fract();
        }
        ScenePointConstraint::OnCircle { unit_x, unit_y, .. } => {
            let angle = std::f64::consts::TAU * clamped;
            *unit_x = angle.cos();
            *unit_y = -angle.sin();
        }
        ScenePointConstraint::OnCircleArc { t, .. } => {
            *t = clamped;
        }
        ScenePointConstraint::OnArc { t, .. } => {
            *t = clamped;
        }
        _ => {}
    }
}

fn resolve_trace_point(
    points: &[ScenePoint],
    index: usize,
    visiting: &mut BTreeSet<usize>,
) -> Option<PointRecord> {
    if !visiting.insert(index) {
        return None;
    }

    let point = points.get(index)?;
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
            let base_angle = (-(axis_end.y - origin.y)).atan2(axis_end.x - origin.x).to_degrees();
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
                )
            }
        },
    };

    visiting.remove(&index);
    resolved
}

fn resolve_trace_line_constraint(
    points: &[ScenePoint],
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
    let t = ts[variant.min(ts.len() - 1)];
    Some(PointRecord {
        x: line_start.x + dx * t,
        y: line_start.y + dy * t,
    })
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
    Some(ordered[variant.min(1)].clone())
}
