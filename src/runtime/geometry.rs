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

pub(crate) fn lerp_point(start: &PointRecord, end: &PointRecord, t: f64) -> PointRecord {
    from_core_point(gsp_runtime_core::lerp_point(
        to_core_point(start),
        to_core_point(end),
        t,
    ))
}

pub(crate) fn rotate_around(
    point: &PointRecord,
    center: &PointRecord,
    radians: f64,
) -> PointRecord {
    from_core_point(gsp_runtime_core::rotate_around(
        to_core_point(point),
        to_core_point(center),
        radians,
    ))
}

pub(crate) fn angle_degrees_from_points(
    start: &PointRecord,
    vertex: &PointRecord,
    end: &PointRecord,
) -> Option<f64> {
    gsp_runtime_core::measured_rotation_radians(
        to_core_point(start),
        to_core_point(vertex),
        to_core_point(end),
    )
    .map(f64::to_degrees)
}

pub(crate) fn scale_around(point: &PointRecord, center: &PointRecord, factor: f64) -> PointRecord {
    from_core_point(gsp_runtime_core::scale_around(
        to_core_point(point),
        to_core_point(center),
        factor,
    ))
}

pub(crate) fn reflect_across_line(
    point: &PointRecord,
    line_start: &PointRecord,
    line_end: &PointRecord,
) -> Option<PointRecord> {
    gsp_runtime_core::reflect_across_line(
        to_core_point(point),
        to_core_point(line_start),
        to_core_point(line_end),
    )
    .map(from_core_point)
}

pub(crate) fn to_core_point(point: &PointRecord) -> gsp_runtime_core::Point {
    gsp_runtime_core::Point {
        x: point.x,
        y: point.y,
    }
}

pub(crate) fn from_core_point(point: gsp_runtime_core::Point) -> PointRecord {
    PointRecord {
        x: point.x,
        y: point.y,
    }
}

pub(super) fn read_f32_unaligned(data: &[u8], offset: usize) -> Option<f32> {
    let bytes = data.get(offset..offset + 4)?;
    Some(f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
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

pub(crate) fn clip_line_to_bounds(
    start: &PointRecord,
    end: &PointRecord,
    bounds: &Bounds,
) -> Option<[PointRecord; 2]> {
    gsp_runtime_core::clip_line_to_bounds(
        to_core_point(start),
        to_core_point(end),
        to_core_bounds(bounds),
    )
    .map(|points| points.map(from_core_point))
}

pub(crate) fn clip_ray_to_bounds(
    start: &PointRecord,
    end: &PointRecord,
    bounds: &Bounds,
) -> Option<[PointRecord; 2]> {
    gsp_runtime_core::clip_ray_to_bounds(
        to_core_point(start),
        to_core_point(end),
        to_core_bounds(bounds),
    )
    .map(|points| points.map(from_core_point))
}

fn to_core_bounds(bounds: &Bounds) -> gsp_runtime_core::Bounds {
    gsp_runtime_core::Bounds {
        min_x: bounds.min_x,
        max_x: bounds.max_x,
        min_y: bounds.min_y,
        max_y: bounds.max_y,
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

pub(super) fn line_is_dashed(style: u32) -> bool {
    // Sketchpad encodes the stroke pattern in the third byte of style_a.
    // Observed native payloads use:
    // - 0x12 for dashed segments
    // - 0x11 for dashed constructed lines (perpendicular / parallel / line-like)
    matches!(((style >> 16) & 0xff) as u8, 0x11 | 0x12)
}

pub(super) fn line_stroke_width_from_style(style: u32) -> f64 {
    if matches!((style >> 16) & 0xff, 0x12 | 0x22) {
        2.0
    } else {
        1.0
    }
}

pub(super) fn fill_color_from_styles(style_b: u32, style_c: u32) -> [u8; 4] {
    let mut color = color_from_style(style_b);
    let alpha = ((style_c >> 8) & 0xff) as u8;
    if alpha != 0 {
        color[3] = alpha;
    }
    color
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
    let geometry = gsp_runtime_core::three_point_arc_geometry(
        to_core_point(start),
        to_core_point(mid),
        to_core_point(end),
    )?;
    Some(ThreePointArcGeometry {
        center: from_core_point(geometry.center),
        radius: geometry.radius,
        start_angle: geometry.start_angle,
        end_angle: geometry.end_angle,
        counterclockwise: geometry.ccw_mid > geometry.ccw_span + 1e-9,
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
    gsp_runtime_core::point_on_three_point_arc(
        to_core_point(start),
        to_core_point(mid),
        to_core_point(end),
        t,
    )
    .map(from_core_point)
}

pub(crate) fn point_on_three_point_arc_complement(
    start: &PointRecord,
    mid: &PointRecord,
    end: &PointRecord,
    t: f64,
) -> Option<PointRecord> {
    gsp_runtime_core::point_on_three_point_arc_complement(
        to_core_point(start),
        to_core_point(mid),
        to_core_point(end),
        t,
    )
    .map(from_core_point)
}

pub(crate) fn point_on_circle_arc(
    center: &PointRecord,
    start: &PointRecord,
    end: &PointRecord,
    t: f64,
) -> Option<PointRecord> {
    gsp_runtime_core::point_on_circle_arc(
        to_core_point(center),
        to_core_point(start),
        to_core_point(end),
        t,
        false,
    )
    .map(from_core_point)
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

pub(crate) fn sample_three_point_arc_complement(
    start: &PointRecord,
    mid: &PointRecord,
    end: &PointRecord,
    subdivisions: usize,
) -> Option<Vec<PointRecord>> {
    let segment_count = subdivisions.max(2);
    (0..=segment_count)
        .map(|index| {
            point_on_three_point_arc_complement(
                start,
                mid,
                end,
                index as f64 / segment_count as f64,
            )
        })
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
    gsp_runtime_core::circle_arc_control_points(
        to_core_point(center),
        to_core_point(start),
        to_core_point(end),
        false,
    )
    .map(|points| points.map(from_core_point))
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
    gsp_runtime_core::normalize_angle_delta(from, to)
}

#[cfg(test)]
mod tests {
    use super::{line_is_dashed, line_stroke_width_from_style};

    #[test]
    fn detects_dashed_line_like_styles() {
        assert!(line_is_dashed(0x0112_000c), "expected dashed line style");
        assert!(line_is_dashed(0x0112_000d), "expected dashed ray style");
        assert!(
            line_is_dashed(0x0111_002f),
            "expected dashed perpendicular/parallel helper line style"
        );
        assert!(!line_is_dashed(0x0122_000c), "expected solid line style");
        assert!(!line_is_dashed(0x0122_000d), "expected solid ray style");
    }

    #[test]
    fn decodes_line_width_from_style_byte() {
        assert_eq!(line_stroke_width_from_style(0x0121_000c), 1.0);
        assert_eq!(line_stroke_width_from_style(0x0122_000c), 2.0);
        assert_eq!(line_stroke_width_from_style(0x0111_000c), 1.0);
        assert_eq!(line_stroke_width_from_style(0x0112_000c), 2.0);
    }
}
