use crate::format::GspFile;
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

    fn draw_crosshair(&mut self, cx: i32, cy: i32, size: i32, rgba: [u8; 4]) {
        for delta in -size..=size {
            self.set_pixel(cx + delta, cy, rgba);
            self.set_pixel(cx, cy + delta, rgba);
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

    for point in &points {
        let px = ((point.x - bounds.min_x) * scale + offset_x).round() as i32;
        let py = ((point.y - bounds.min_y) * scale + offset_y).round() as i32;
        canvas.draw_circle(px, py, 6, [30, 90, 180, 255]);
        canvas.draw_circle(px, py, 3, [255, 255, 255, 255]);
        canvas.draw_crosshair(px, py, 9, [20, 20, 20, 255]);
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

fn point_bounds(points: &[crate::format::PointRecord]) -> RenderBounds {
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
