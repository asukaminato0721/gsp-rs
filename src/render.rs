use crate::format::{decode_indexed_path, decode_point_record, GspFile, ObjectGroup, PointRecord};
use crate::png::encode_png_rgba;
use std::fs;
use std::io::Write as _;
use std::path::Path;

#[derive(Debug)]
struct RenderBounds {
    min_x: f64,
    max_x: f64,
    min_y: f64,
    max_y: f64,
}

#[derive(Debug, Clone)]
struct CircleShape {
    center: PointRecord,
    radius_point: PointRecord,
}

#[derive(Debug)]
struct Canvas {
    width: u32,
    height: u32,
    pixels: Vec<u8>,
}

impl Canvas {
    fn new(width: u32, height: u32, rgba: [u8; 4]) -> Self {
        let mut pixels = vec![0; (width as usize) * (height as usize) * 4];
        for chunk in pixels.chunks_exact_mut(4) {
            chunk.copy_from_slice(&rgba);
        }
        Self {
            width,
            height,
            pixels,
        }
    }

    fn set_pixel(&mut self, x: i32, y: i32, rgba: [u8; 4]) {
        if x < 0 || y < 0 || x >= self.width as i32 || y >= self.height as i32 {
            return;
        }

        let index = ((y as usize) * (self.width as usize) + (x as usize)) * 4;
        self.pixels[index..index + 4].copy_from_slice(&rgba);
    }

    fn draw_circle(&mut self, cx: i32, cy: i32, radius: i32, rgba: [u8; 4]) {
        let r2 = radius * radius;
        for dy in -radius..=radius {
            for dx in -radius..=radius {
                if dx * dx + dy * dy <= r2 {
                    self.set_pixel(cx + dx, cy + dy, rgba);
                }
            }
        }
    }

    fn draw_line(&mut self, mut x0: i32, mut y0: i32, x1: i32, y1: i32, rgba: [u8; 4]) {
        let dx = (x1 - x0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let dy = -(y1 - y0).abs();
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut error = dx + dy;

        loop {
            self.set_pixel(x0, y0, rgba);
            if x0 == x1 && y0 == y1 {
                break;
            }

            let doubled = error * 2;
            if doubled >= dy {
                error += dy;
                x0 += sx;
            }
            if doubled <= dx {
                error += dx;
                y0 += sy;
            }
        }
    }

    fn draw_polyline(&mut self, points: &[(i32, i32)], closed: bool, rgba: [u8; 4]) {
        if points.len() < 2 {
            return;
        }

        for segment in points.windows(2) {
            self.draw_line(segment[0].0, segment[0].1, segment[1].0, segment[1].1, rgba);
        }

        if closed {
            self.draw_line(
                points[points.len() - 1].0,
                points[points.len() - 1].1,
                points[0].0,
                points[0].1,
                rgba,
            );
        }
    }

    fn fill_polygon(&mut self, points: &[(i32, i32)], rgba: [u8; 4]) {
        if points.len() < 3 {
            return;
        }

        let min_y = points.iter().map(|point| point.1).min().unwrap_or(0);
        let max_y = points.iter().map(|point| point.1).max().unwrap_or(-1);

        for y in min_y..=max_y {
            let mut intersections = Vec::<i32>::new();
            for edge in 0..points.len() {
                let (x1, y1) = points[edge];
                let (x2, y2) = points[(edge + 1) % points.len()];
                if y1 == y2 {
                    continue;
                }

                let (sy, ey, sx, ex) = if y1 < y2 {
                    (y1, y2, x1, x2)
                } else {
                    (y2, y1, x2, x1)
                };

                if y < sy || y >= ey {
                    continue;
                }

                let t = (y - sy) as f64 / (ey - sy) as f64;
                let x = sx as f64 + t * (ex - sx) as f64;
                intersections.push(x.round() as i32);
            }

            intersections.sort_unstable();
            for pair in intersections.chunks_exact(2) {
                let start = pair[0];
                let end = pair[1];
                for x in start..=end {
                    self.set_pixel(x, y, rgba);
                }
            }
        }
    }

    fn draw_rect_outline(
        &mut self,
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
        rgba: [u8; 4],
    ) {
        self.draw_line(left, top, right, top, rgba);
        self.draw_line(right, top, right, bottom, rgba);
        self.draw_line(right, bottom, left, bottom, rgba);
        self.draw_line(left, bottom, left, top, rgba);
    }

    fn draw_circle_outline(&mut self, cx: i32, cy: i32, radius: i32, rgba: [u8; 4]) {
        let mut x = radius;
        let mut y = 0;
        let mut error = 1 - radius;

        while x >= y {
            let octants = [
                (cx + x, cy + y),
                (cx + y, cy + x),
                (cx - y, cy + x),
                (cx - x, cy + y),
                (cx - x, cy - y),
                (cx - y, cy - x),
                (cx + y, cy - x),
                (cx + x, cy - y),
            ];
            for (px, py) in octants {
                self.set_pixel(px, py, rgba);
            }

            y += 1;
            if error < 0 {
                error += 2 * y + 1;
            } else {
                x -= 1;
                error += 2 * (y - x) + 1;
            }
        }
    }
}

pub fn render_points_to_png(
    file: &GspFile,
    output_path: &Path,
    width: u32,
    height: u32,
) -> Result<(), String> {
    if !matches!(
        output_path.extension().and_then(|ext| ext.to_str()),
        Some("png") | Some("PNG")
    ) {
        return Err(format!(
            "only PNG output is implemented for now: {}",
            output_path.display()
        ));
    }

    let points = file.point_records();
    let groups = file.object_groups();
    if points.is_empty() {
        return Err("render target is empty: no 0x0899 point records found".to_string());
    }

    if width < 64 || height < 64 {
        return Err("render size must be at least 64x64".to_string());
    }

    let bounds = point_bounds(&points);
    let mut canvas = Canvas::new(width, height, [250, 250, 248, 255]);
    let margin = 32.0_f64;
    let usable_width = (width as f64 - margin * 2.0).max(1.0);
    let usable_height = (height as f64 - margin * 2.0).max(1.0);
    let span_x = (bounds.max_x - bounds.min_x).max(1.0);
    let span_y = (bounds.max_y - bounds.min_y).max(1.0);
    let scale = f64::min(usable_width / span_x, usable_height / span_y);
    let content_width = span_x * scale;
    let content_height = span_y * scale;
    let offset_x = (width as f64 - content_width) / 2.0;
    let offset_y = (height as f64 - content_height) / 2.0;

    let projected_points = points
        .iter()
        .map(|point| world_to_screen(point, &bounds, scale, offset_x, offset_y))
        .collect::<Vec<_>>();

    canvas.draw_rect_outline(
        margin as i32,
        margin as i32,
        (width as f64 - margin) as i32,
        (height as f64 - margin) as i32,
        [220, 220, 220, 255],
    );

    let point_map = collect_point_objects(file, &groups);
    let polylines = collect_shape_paths(file, &groups, 2, &point_map, &projected_points);
    let polygons = collect_shape_paths(file, &groups, 8, &point_map, &projected_points);
    let circles = collect_circles(file, &groups, &point_map);

    for polygon in &polygons {
        canvas.fill_polygon(polygon, [120, 245, 110, 255]);
        canvas.draw_polyline(polygon, true, [40, 150, 40, 255]);
    }

    for polyline in &polylines {
        canvas.draw_polyline(polyline, false, [20, 20, 180, 255]);
    }

    for circle in &circles {
        if let Some((cx, cy, radius_px)) =
            project_circle(circle, &bounds, scale, offset_x, offset_y)
        {
            canvas.draw_circle_outline(cx, cy, radius_px, [20, 120, 20, 255]);
        }
    }

    for (index, _point) in points.iter().enumerate() {
        let (px, py) = projected_points[index];
        canvas.draw_circle(px, py, 4, [255, 60, 40, 255]);
    }

    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "failed to create render output directory {}: {error}",
                parent.display()
            )
        })?;
    }

    let png = encode_png_rgba(width, height, &canvas.pixels)?;
    let mut file_handle = fs::File::create(output_path)
        .map_err(|error| format!("failed to create {}: {error}", output_path.display()))?;
    file_handle
        .write_all(&png)
        .map_err(|error| format!("failed to write {}: {error}", output_path.display()))?;

    Ok(())
}

fn point_bounds(points: &[PointRecord]) -> RenderBounds {
    let mut min_x = points[0].x;
    let mut max_x = points[0].x;
    let mut min_y = points[0].y;
    let mut max_y = points[0].y;

    for point in points {
        min_x = min_x.min(point.x);
        max_x = max_x.max(point.x);
        min_y = min_y.min(point.y);
        max_y = max_y.max(point.y);
    }

    if (max_x - min_x).abs() < f64::EPSILON {
        max_x += 1.0;
        min_x -= 1.0;
    }
    if (max_y - min_y).abs() < f64::EPSILON {
        max_y += 1.0;
        min_y -= 1.0;
    }

    RenderBounds {
        min_x,
        max_x,
        min_y,
        max_y,
    }
}

fn world_to_screen(
    point: &PointRecord,
    bounds: &RenderBounds,
    scale: f64,
    offset_x: f64,
    offset_y: f64,
) -> (i32, i32) {
    let px = ((point.x - bounds.min_x) * scale + offset_x).round() as i32;
    let py = ((point.y - bounds.min_y) * scale + offset_y).round() as i32;
    (px, py)
}

fn collect_point_objects(file: &GspFile, groups: &[ObjectGroup]) -> Vec<Option<PointRecord>> {
    groups
        .iter()
        .map(|group| {
            if group.header.class_id != 0 {
                return None;
            }
            group
                .records
                .iter()
                .find_map(|record| (record.record_type == 0x0899).then(|| decode_point_record(record.payload(&file.data))).flatten())
        })
        .collect()
}

fn collect_shape_paths(
    file: &GspFile,
    groups: &[ObjectGroup],
    class_id: u32,
    point_map: &[Option<PointRecord>],
    projected_points: &[(i32, i32)],
) -> Vec<Vec<(i32, i32)>> {
    groups
        .iter()
        .filter(|group| group.header.class_id == class_id)
        .filter_map(|group| {
            let path = group
                .records
                .iter()
                .find_map(|record| match record.record_type {
                    0x07d2 | 0x07d3 => decode_indexed_path(record.record_type, record.payload(&file.data)),
                    _ => None,
                });
            let path = path?;
            let mut vertices = Vec::new();
            for object_ref in path.refs {
                point_map.get(object_ref.saturating_sub(1))?.as_ref()?;
                let point_index = point_map
                    .iter()
                    .take(object_ref)
                    .filter(|entry| entry.is_some())
                    .count()
                    .saturating_sub(1);
                vertices.push(*projected_points.get(point_index)?);
            }
            (vertices.len() >= 2).then_some(vertices)
        })
        .collect()
}

fn collect_circles(
    file: &GspFile,
    groups: &[ObjectGroup],
    point_map: &[Option<PointRecord>],
) -> Vec<CircleShape> {
    groups
        .iter()
        .filter(|group| group.header.class_id == 3)
        .filter_map(|group| {
            let path = group.records.iter().find_map(|record| match record.record_type {
                0x07d2 | 0x07d3 => decode_indexed_path(record.record_type, record.payload(&file.data)),
                _ => None,
            })?;
            if path.refs.len() != 2 {
                return None;
            }
            let center = point_map.get(path.refs[0].saturating_sub(1))?.clone()?;
            let radius_point = point_map.get(path.refs[1].saturating_sub(1))?.clone()?;
            Some(CircleShape { center, radius_point })
        })
        .collect()
}

fn project_circle(
    circle: &CircleShape,
    bounds: &RenderBounds,
    scale: f64,
    offset_x: f64,
    offset_y: f64,
) -> Option<(i32, i32, i32)> {
    let (cx, cy) = world_to_screen(&circle.center, bounds, scale, offset_x, offset_y);
    let radius = ((circle.center.x - circle.radius_point.x).powi(2)
        + (circle.center.y - circle.radius_point.y).powi(2))
    .sqrt();
    let radius_px = (radius * scale).round() as i32;
    if radius_px < 4 {
        None
    } else {
        Some((cx, cy, radius_px))
    }
}
