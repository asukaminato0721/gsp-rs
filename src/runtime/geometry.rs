use crate::format::PointRecord;

use super::scene::LineShape;

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

pub(super) fn scale_around(
    point: &PointRecord,
    center: &PointRecord,
    factor: f64,
) -> PointRecord {
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
