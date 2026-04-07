use crate::format::PointRecord;

use super::scene::LineShape;

#[derive(Debug, Clone)]
pub(crate) struct ThreePointArcGeometry {
    pub(crate) center: PointRecord,
    pub(crate) radius: f64,
    pub(crate) start_angle: f64,
    pub(crate) end_angle: f64,
    pub(crate) counterclockwise: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct GraphTransform {
    pub(super) origin_raw: PointRecord,
    pub(super) raw_per_unit: f64,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Bounds {
    pub(crate) min_x: f64,
    pub(crate) max_x: f64,
    pub(crate) min_y: f64,
    pub(crate) max_y: f64,
}

pub(super) fn to_world(point: &PointRecord, graph: &Option<GraphTransform>) -> PointRecord {
    if let Some(graph) = graph {
        PointRecord {
            x: (point.x - graph.origin_raw.x) / graph.raw_per_unit,
            y: (graph.origin_raw.y - point.y) / graph.raw_per_unit,
        }
    } else {
        point.clone()
    }
}

pub(super) fn to_raw_from_world(point: &PointRecord, graph: &GraphTransform) -> PointRecord {
    PointRecord {
        x: graph.origin_raw.x + point.x * graph.raw_per_unit,
        y: graph.origin_raw.y - point.y * graph.raw_per_unit,
    }
}

pub(super) fn lerp_point(start: &PointRecord, end: &PointRecord, t: f64) -> PointRecord {
    start.clone() + (end.clone() - start.clone()) * t
}

pub(super) fn rotate_around(
    point: &PointRecord,
    center: &PointRecord,
    radians: f64,
) -> PointRecord {
    let cos = radians.cos();
    let sin = radians.sin();
    let delta = point.clone() - center.clone();
    PointRecord {
        x: center.x + delta.x * cos + delta.y * sin,
        y: center.y - delta.x * sin + delta.y * cos,
    }
}

pub(super) fn scale_around(point: &PointRecord, center: &PointRecord, factor: f64) -> PointRecord {
    center.clone() + (point.clone() - center.clone()) * factor
}

pub(super) fn reflect_across_line(
    point: &PointRecord,
    line_start: &PointRecord,
    line_end: &PointRecord,
) -> Option<PointRecord> {
    let line_delta = line_end.clone() - line_start.clone();
    let len_sq = line_delta.x * line_delta.x + line_delta.y * line_delta.y;
    if len_sq <= 1e-9 {
        return None;
    }
    let point_delta = point.clone() - line_start.clone();
    let t = (point_delta.x * line_delta.x + point_delta.y * line_delta.y) / len_sq;
    let projection = line_start.clone() + line_delta * t;
    Some(projection.clone() * 2.0 - point.clone())
}

pub(super) fn read_f32_unaligned(data: &[u8], offset: usize) -> Option<f32> {
    let bytes = data.get(offset..offset + 4)?;
    Some(f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
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

pub(super) fn distance_world(
    a: &PointRecord,
    b: &PointRecord,
    graph: &Option<GraphTransform>,
) -> f64 {
    let aw = to_world(a, graph);
    let bw = to_world(b, graph);
    ((aw.x - bw.x).powi(2) + (aw.y - bw.y).powi(2)).sqrt()
}

pub(super) fn include_line_bounds(
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

pub(super) fn clip_line_to_bounds(
    start: &PointRecord,
    end: &PointRecord,
    bounds: &Bounds,
) -> Option<[PointRecord; 2]> {
    clip_parametric_line_to_bounds(start, end, bounds, false)
}

pub(super) fn clip_ray_to_bounds(
    start: &PointRecord,
    end: &PointRecord,
    bounds: &Bounds,
) -> Option<[PointRecord; 2]> {
    clip_parametric_line_to_bounds(start, end, bounds, true)
}

fn clip_parametric_line_to_bounds(
    start: &PointRecord,
    end: &PointRecord,
    bounds: &Bounds,
    ray_only: bool,
) -> Option<[PointRecord; 2]> {
    let dx = end.x - start.x;
    let dy = end.y - start.y;
    if dx.abs() <= 1e-9 && dy.abs() <= 1e-9 {
        return None;
    }

    let mut hits = Vec::<(f64, PointRecord)>::new();
    let mut push_hit = |t: f64, point: PointRecord| {
        if !t.is_finite()
            || (ray_only && t < -1e-9)
            || point.x < bounds.min_x - 1e-6
            || point.x > bounds.max_x + 1e-6
            || point.y < bounds.min_y - 1e-6
            || point.y > bounds.max_y + 1e-6
        {
            return;
        }
        if hits.iter().any(|(existing_t, existing)| {
            (existing_t - t).abs() < 1e-6
                || ((existing.x - point.x).abs() < 1e-6 && (existing.y - point.y).abs() < 1e-6)
        }) {
            return;
        }
        hits.push((t, point));
    };

    if dx.abs() > 1e-9 {
        for x in [bounds.min_x, bounds.max_x] {
            let t = (x - start.x) / dx;
            push_hit(
                t,
                PointRecord {
                    x,
                    y: start.y + dy * t,
                },
            );
        }
    }

    if dy.abs() > 1e-9 {
        for y in [bounds.min_y, bounds.max_y] {
            let t = (y - start.y) / dy;
            push_hit(
                t,
                PointRecord {
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
        push_hit(0.0, start.clone());
    }

    if hits.len() < 2 {
        return None;
    }
    hits.sort_by(|left, right| left.0.total_cmp(&right.0));
    if ray_only {
        let first = hits.first()?.1.clone();
        let last = hits.last()?.1.clone();
        if (first.x - last.x).abs() < 1e-6 && (first.y - last.y).abs() < 1e-6 {
            return None;
        }
        Some([first, last])
    } else {
        let first = hits.first()?.1.clone();
        let last = hits.last()?.1.clone();
        if (first.x - last.x).abs() < 1e-6 && (first.y - last.y).abs() < 1e-6 {
            return None;
        }
        Some([first, last])
    }
}

pub(super) fn format_number(value: f64) -> String {
    if (value.fract()).abs() < 0.005 {
        format!("{value:.0}")
    } else {
        format!("{value:.2}")
    }
}

pub(super) fn color_from_style(style: u32) -> [u8; 4] {
    [
        (style & 0xff) as u8,
        ((style >> 8) & 0xff) as u8,
        ((style >> 16) & 0xff) as u8,
        255,
    ]
}

pub(super) fn fill_color_from_styles(style_b: u32, style_c: u32) -> [u8; 4] {
    let mut color = color_from_style(style_b);
    let alpha = ((style_c >> 8) & 0xff) as u8;
    if alpha != 0 {
        color[3] = alpha;
    }
    color
}

pub(crate) fn darken(rgba: [u8; 4], amount: u8) -> [u8; 4] {
    [
        rgba[0].saturating_sub(amount),
        rgba[1].saturating_sub(amount),
        rgba[2].saturating_sub(amount),
        rgba[3],
    ]
}

pub(super) fn has_distinct_points(points: &[PointRecord]) -> bool {
    points.windows(2).any(|pair| {
        let delta = pair[0].clone() - pair[1].clone();
        let dx = delta.x;
        let dy = delta.y;
        dx.abs() > 1e-6 || dy.abs() > 1e-6
    })
}

pub(crate) fn three_point_arc_geometry(
    start: &PointRecord,
    mid: &PointRecord,
    end: &PointRecord,
) -> Option<ThreePointArcGeometry> {
    let determinant =
        2.0 * (start.x * (mid.y - end.y) + mid.x * (end.y - start.y) + end.x * (start.y - mid.y));
    if determinant.abs() <= 1e-9 {
        return None;
    }

    let start_sq = start.x * start.x + start.y * start.y;
    let mid_sq = mid.x * mid.x + mid.y * mid.y;
    let end_sq = end.x * end.x + end.y * end.y;
    let center = PointRecord {
        x: (start_sq * (mid.y - end.y) + mid_sq * (end.y - start.y) + end_sq * (start.y - mid.y))
            / determinant,
        y: (start_sq * (end.x - mid.x) + mid_sq * (start.x - end.x) + end_sq * (mid.x - start.x))
            / determinant,
    };
    let radius = ((start.x - center.x).powi(2) + (start.y - center.y).powi(2)).sqrt();
    if radius <= 1e-9 {
        return None;
    }

    let start_angle = (start.y - center.y).atan2(start.x - center.x);
    let mid_angle = (mid.y - center.y).atan2(mid.x - center.x);
    let end_angle = (end.y - center.y).atan2(end.x - center.x);
    let ccw_span = normalized_angle_delta(start_angle, end_angle);
    let ccw_mid = normalized_angle_delta(start_angle, mid_angle);

    Some(ThreePointArcGeometry {
        center,
        radius,
        start_angle,
        end_angle,
        counterclockwise: ccw_mid > ccw_span + 1e-9,
    })
}

pub(crate) fn arc_sample_points(
    start: &PointRecord,
    mid: &PointRecord,
    end: &PointRecord,
) -> Option<Vec<PointRecord>> {
    let geometry = three_point_arc_geometry(start, mid, end)?;
    let mut points = vec![start.clone(), mid.clone(), end.clone()];
    for angle in [
        0.0,
        std::f64::consts::FRAC_PI_2,
        std::f64::consts::PI,
        std::f64::consts::PI * 1.5,
    ] {
        if angle_lies_on_arc(
            angle,
            geometry.start_angle,
            geometry.end_angle,
            geometry.counterclockwise,
        ) {
            points.push(PointRecord {
                x: geometry.center.x + geometry.radius * angle.cos(),
                y: geometry.center.y + geometry.radius * angle.sin(),
            });
        }
    }
    Some(points)
}

pub(crate) fn point_on_three_point_arc(
    start: &PointRecord,
    mid: &PointRecord,
    end: &PointRecord,
    t: f64,
) -> Option<PointRecord> {
    let geometry = three_point_arc_geometry(start, mid, end)?;
    let ccw_span = normalized_angle_delta(geometry.start_angle, geometry.end_angle);
    let ccw_mid = normalized_angle_delta(geometry.start_angle, mid_angle(&geometry.center, mid));
    let clamped_t = t.clamp(0.0, 1.0);
    let angle = if ccw_mid <= ccw_span + 1e-9 {
        geometry.start_angle + ccw_span * clamped_t
    } else {
        geometry.start_angle
            - normalized_angle_delta(geometry.end_angle, geometry.start_angle) * clamped_t
    };
    Some(PointRecord {
        x: geometry.center.x + geometry.radius * angle.cos(),
        y: geometry.center.y + geometry.radius * angle.sin(),
    })
}

pub(crate) fn point_on_circle_arc(
    center: &PointRecord,
    start: &PointRecord,
    end: &PointRecord,
    t: f64,
) -> Option<PointRecord> {
    let [start, mid, end] = arc_on_circle_control_points(center, start, end)?;
    point_on_three_point_arc(&start, &mid, &end, t)
}

pub(crate) fn sample_three_point_arc(
    start: &PointRecord,
    mid: &PointRecord,
    end: &PointRecord,
    subdivisions: usize,
) -> Option<Vec<PointRecord>> {
    let segment_count = subdivisions.max(2);
    (0..=segment_count)
        .map(|index| point_on_three_point_arc(start, mid, end, index as f64 / segment_count as f64))
        .collect()
}

pub(crate) fn locate_polyline_parameter_by_length(
    points: &[PointRecord],
    normalized_t: f64,
) -> Option<(usize, f64)> {
    if points.len() < 2 {
        return None;
    }

    let lengths = points
        .windows(2)
        .map(|segment| {
            let dx = segment[1].x - segment[0].x;
            let dy = segment[1].y - segment[0].y;
            (dx * dx + dy * dy).sqrt()
        })
        .collect::<Vec<_>>();
    let total_length: f64 = lengths.iter().sum();
    if total_length <= 1e-9 {
        return None;
    }

    let target = normalized_t.clamp(0.0, 1.0) * total_length;
    let mut traveled = 0.0;
    for (segment_index, length) in lengths.iter().enumerate() {
        if traveled + length >= target || segment_index == lengths.len() - 1 {
            let local_t = if *length <= 1e-9 {
                0.0
            } else {
                ((target - traveled) / length).clamp(0.0, 1.0)
            };
            return Some((segment_index, local_t));
        }
        traveled += length;
    }

    None
}

pub(crate) fn arc_on_circle_control_points(
    center: &PointRecord,
    start: &PointRecord,
    end: &PointRecord,
) -> Option<[PointRecord; 3]> {
    let start_dx = start.x - center.x;
    let start_dy = start.y - center.y;
    let end_dx = end.x - center.x;
    let end_dy = end.y - center.y;
    let start_radius = (start_dx * start_dx + start_dy * start_dy).sqrt();
    let end_radius = (end_dx * end_dx + end_dy * end_dy).sqrt();
    let radius = (start_radius + end_radius) * 0.5;
    if radius <= 1e-9 {
        return None;
    }

    let start_angle = (-start_dy).atan2(start_dx);
    let end_angle = (-end_dy).atan2(end_dx);
    let ccw_span = normalized_angle_delta(start_angle, end_angle);
    let midpoint_angle = start_angle + ccw_span * 0.5;
    let mid = PointRecord {
        x: center.x + radius * midpoint_angle.cos(),
        y: center.y - radius * midpoint_angle.sin(),
    };

    Some([start.clone(), mid, end.clone()])
}

fn angle_lies_on_arc(angle: f64, start_angle: f64, end_angle: f64, counterclockwise: bool) -> bool {
    if counterclockwise {
        normalized_angle_delta(angle, start_angle)
            <= normalized_angle_delta(end_angle, start_angle) + 1e-9
    } else {
        normalized_angle_delta(start_angle, angle)
            <= normalized_angle_delta(start_angle, end_angle) + 1e-9
    }
}

fn normalized_angle_delta(from: f64, to: f64) -> f64 {
    let tau = std::f64::consts::TAU;
    (to - from).rem_euclid(tau)
}

fn mid_angle(center: &PointRecord, point: &PointRecord) -> f64 {
    (point.y - center.y).atan2(point.x - center.x)
}
