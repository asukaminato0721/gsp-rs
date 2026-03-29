use crate::format::{GspFile, PointRecord};
use crate::render::{Bounds, build_scene, darken, screen_scale, to_screen};
use ab_glyph::{Font, FontArc, Glyph, PxScale, ScaleFont, point};
use std::fs;
use std::io::Write as _;
use std::path::Path;

const FONT_CANDIDATES: &[&str] = &[
    "/usr/share/fonts/noto/NotoSans-Regular.ttf",
    "/usr/share/fonts/Adwaita/AdwaitaSans-Regular.ttf",
    "/usr/share/fonts/gnu-free/FreeSans.otf",
];

#[derive(Debug)]
struct Canvas {
    width: u32,
    height: u32,
    pixels: Vec<u8>,
}

#[derive(Clone)]
struct FontRenderer {
    font: FontArc,
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
        let index = ((y as usize) * self.width as usize + x as usize) * 4;
        self.pixels[index..index + 4].copy_from_slice(&rgba);
    }

    fn draw_circle_filled(&mut self, cx: i32, cy: i32, radius: i32, rgba: [u8; 4]) {
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
            let e2 = error * 2;
            if e2 >= dy {
                error += dy;
                x0 += sx;
            }
            if e2 <= dx {
                error += dx;
                y0 += sy;
            }
        }
    }

    fn draw_dashed_line(
        &mut self,
        mut x0: i32,
        mut y0: i32,
        x1: i32,
        y1: i32,
        rgba: [u8; 4],
        dash_len: i32,
    ) {
        let dx = (x1 - x0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let dy = -(y1 - y0).abs();
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut error = dx + dy;
        let mut step = 0;

        loop {
            if (step / dash_len) % 2 == 0 {
                self.set_pixel(x0, y0, rgba);
            }
            if x0 == x1 && y0 == y1 {
                break;
            }
            let e2 = error * 2;
            if e2 >= dy {
                error += dy;
                x0 += sx;
            }
            if e2 <= dx {
                error += dx;
                y0 += sy;
            }
            step += 1;
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
                intersections.push((sx as f64 + t * (ex - sx) as f64).round() as i32);
            }
            intersections.sort_unstable();
            for pair in intersections.chunks_exact(2) {
                for x in pair[0]..=pair[1] {
                    self.set_pixel(x, y, rgba);
                }
            }
        }
    }

    fn draw_circle_outline(&mut self, cx: i32, cy: i32, radius: i32, rgba: [u8; 4]) {
        let mut x = radius;
        let mut y = 0;
        let mut error = 1 - radius;
        while x >= y {
            for (px, py) in [
                (cx + x, cy + y),
                (cx + y, cy + x),
                (cx - y, cy + x),
                (cx - x, cy + y),
                (cx - x, cy - y),
                (cx - y, cy - x),
                (cx + y, cy - x),
                (cx + x, cy - y),
            ] {
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

    fn draw_text(&mut self, x: i32, y: i32, text: &str, rgba: [u8; 4]) {
        self.draw_text_bitmap(x, y, text, rgba);
    }

    fn draw_text_bitmap(&mut self, x: i32, y: i32, text: &str, rgba: [u8; 4]) {
        let mut cursor_x = x;
        let mut cursor_y = y;
        for ch in text.chars() {
            if ch == '\n' {
                cursor_x = x;
                cursor_y += 10;
                continue;
            }
            self.draw_glyph(cursor_x, cursor_y, ch, rgba);
            cursor_x += 6;
        }
    }

    fn draw_glyph(&mut self, x: i32, y: i32, ch: char, rgba: [u8; 4]) {
        let upper = ch.to_ascii_uppercase();
        let pattern = match upper {
            'A' => [
                "01110", "10001", "11111", "10001", "10001", "00000", "00000",
            ],
            'B' => [
                "11110", "10001", "11110", "10001", "11110", "00000", "00000",
            ],
            'C' => [
                "01111", "10000", "10000", "10000", "01111", "00000", "00000",
            ],
            'D' => [
                "11110", "10001", "10001", "10001", "11110", "00000", "00000",
            ],
            'E' => [
                "11111", "10000", "11110", "10000", "11111", "00000", "00000",
            ],
            'F' => [
                "11111", "10000", "11110", "10000", "10000", "00000", "00000",
            ],
            'G' => [
                "01111", "10000", "10111", "10001", "01111", "00000", "00000",
            ],
            'H' => [
                "10001", "10001", "11111", "10001", "10001", "00000", "00000",
            ],
            'I' => [
                "11111", "00100", "00100", "00100", "11111", "00000", "00000",
            ],
            'J' => [
                "00111", "00010", "00010", "10010", "01100", "00000", "00000",
            ],
            'K' => [
                "10001", "10010", "11100", "10010", "10001", "00000", "00000",
            ],
            'L' => [
                "10000", "10000", "10000", "10000", "11111", "00000", "00000",
            ],
            'M' => [
                "10001", "11011", "10101", "10001", "10001", "00000", "00000",
            ],
            'N' => [
                "10001", "11001", "10101", "10011", "10001", "00000", "00000",
            ],
            'O' => [
                "01110", "10001", "10001", "10001", "01110", "00000", "00000",
            ],
            'P' => [
                "11110", "10001", "11110", "10000", "10000", "00000", "00000",
            ],
            'Q' => [
                "01110", "10001", "10001", "10011", "01111", "00000", "00000",
            ],
            'R' => [
                "11110", "10001", "11110", "10010", "10001", "00000", "00000",
            ],
            'S' => [
                "01111", "10000", "01110", "00001", "11110", "00000", "00000",
            ],
            'T' => [
                "11111", "00100", "00100", "00100", "00100", "00000", "00000",
            ],
            'U' => [
                "10001", "10001", "10001", "10001", "01110", "00000", "00000",
            ],
            'V' => [
                "10001", "10001", "10001", "01010", "00100", "00000", "00000",
            ],
            'W' => [
                "10001", "10001", "10101", "11011", "10001", "00000", "00000",
            ],
            'X' => [
                "10001", "01010", "00100", "01010", "10001", "00000", "00000",
            ],
            'Y' => [
                "10001", "01010", "00100", "00100", "00100", "00000", "00000",
            ],
            'Z' => [
                "11111", "00010", "00100", "01000", "11111", "00000", "00000",
            ],
            '0' => [
                "01110", "10011", "10101", "11001", "01110", "00000", "00000",
            ],
            '1' => [
                "00100", "01100", "00100", "00100", "01110", "00000", "00000",
            ],
            '2' => [
                "01110", "10001", "00010", "00100", "11111", "00000", "00000",
            ],
            '3' => [
                "11110", "00001", "00110", "00001", "11110", "00000", "00000",
            ],
            '4' => [
                "10010", "10010", "11111", "00010", "00010", "00000", "00000",
            ],
            '5' => [
                "11111", "10000", "11110", "00001", "11110", "00000", "00000",
            ],
            '6' => [
                "01110", "10000", "11110", "10001", "01110", "00000", "00000",
            ],
            '7' => [
                "11111", "00010", "00100", "01000", "01000", "00000", "00000",
            ],
            '8' => [
                "01110", "10001", "01110", "10001", "01110", "00000", "00000",
            ],
            '9' => [
                "01110", "10001", "01111", "00001", "01110", "00000", "00000",
            ],
            '+' => [
                "00000", "00100", "11111", "00100", "00000", "00000", "00000",
            ],
            '-' => [
                "00000", "00000", "11111", "00000", "00000", "00000", "00000",
            ],
            '=' => [
                "00000", "11111", "00000", "11111", "00000", "00000", "00000",
            ],
            ':' => [
                "00000", "00100", "00000", "00100", "00000", "00000", "00000",
            ],
            '.' => [
                "00000", "00000", "00000", "00000", "00100", "00000", "00000",
            ],
            ',' => [
                "00000", "00000", "00000", "00100", "01000", "00000", "00000",
            ],
            '(' => [
                "00010", "00100", "00100", "00100", "00010", "00000", "00000",
            ],
            ')' => [
                "01000", "00100", "00100", "00100", "01000", "00000", "00000",
            ],
            '/' => [
                "00001", "00010", "00100", "01000", "10000", "00000", "00000",
            ],
            '^' => [
                "00100", "01010", "10001", "00000", "00000", "00000", "00000",
            ],
            ' ' => [
                "00000", "00000", "00000", "00000", "00000", "00000", "00000",
            ],
            _ => [
                "11111", "10001", "00100", "00000", "00100", "00000", "00000",
            ],
        };

        for (row, bits) in pattern.iter().enumerate() {
            for (col, bit) in bits.bytes().enumerate() {
                if bit == b'1' {
                    self.set_pixel(x + col as i32, y + row as i32, rgba);
                }
            }
        }
    }
}

impl FontRenderer {
    fn load() -> Option<Self> {
        for path in FONT_CANDIDATES {
            if let Ok(bytes) = fs::read(path)
                && let Ok(font) = FontArc::try_from_vec(bytes)
            {
                return Some(Self { font });
            }
        }
        None
    }

    fn draw_text(&self, canvas: &mut Canvas, x: i32, y: i32, text: &str, rgba: [u8; 4], size: f32) {
        let scale = PxScale::from(size);
        let scaled = self.font.as_scaled(scale);
        let mut pen_x = x as f32;
        let mut pen_y = y as f32 + scaled.ascent();

        for ch in text.chars() {
            if ch == '\n' {
                pen_x = x as f32;
                pen_y += scaled.height() + 4.0;
                continue;
            }

            let glyph_id = self.font.glyph_id(ch);
            let glyph = Glyph {
                id: glyph_id,
                scale,
                position: point(pen_x, pen_y),
            };

            if let Some(outlined) = self.font.outline_glyph(glyph.clone()) {
                let bounds = outlined.px_bounds();
                outlined.draw(|gx, gy, coverage| {
                    if coverage <= 0.0 {
                        return;
                    }
                    let px = gx as i32 + bounds.min.x.floor() as i32;
                    let py = gy as i32 + bounds.min.y.floor() as i32;
                    if px < 0 || py < 0 || px >= canvas.width as i32 || py >= canvas.height as i32 {
                        return;
                    }
                    let index = ((py as usize) * canvas.width as usize + px as usize) * 4;
                    let alpha = coverage.clamp(0.0, 1.0);
                    for channel in 0..3 {
                        let bg = canvas.pixels[index + channel] as f32;
                        let fg = rgba[channel] as f32;
                        let blended = bg * (1.0 - alpha) + fg * alpha;
                        canvas.pixels[index + channel] = blended.round().clamp(0.0, 255.0) as u8;
                    }
                    canvas.pixels[index + 3] = 255;
                });
            }

            pen_x += scaled.h_advance(glyph_id);
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
            "png output path must end with .png: {}",
            output_path.display()
        ));
    }

    let scene = build_scene(file);
    let margin = 32.0;
    let mut canvas = Canvas::new(width, height, [250, 250, 248, 255]);
    let font_renderer = FontRenderer::load();

    if scene.graph_mode {
        draw_grid(
            &mut canvas,
            width,
            height,
            margin,
            &scene.bounds,
            scene.origin.as_ref(),
        );
    }

    for polygon in &scene.polygons {
        let screen_points = polygon
            .points
            .iter()
            .map(|point| to_screen(point, width, height, margin, &scene.bounds, scene.y_up))
            .collect::<Vec<_>>();
        canvas.fill_polygon(&screen_points, polygon.color);
        canvas.draw_polyline(&screen_points, true, darken(polygon.color, 80));
    }

    for line in &scene.lines {
        let screen_points = line
            .points
            .iter()
            .map(|point| to_screen(point, width, height, margin, &scene.bounds, scene.y_up))
            .collect::<Vec<_>>();
        if screen_points.len() >= 2 {
            if line.dashed {
                canvas.draw_dashed_line(
                    screen_points[0].0,
                    screen_points[0].1,
                    screen_points[1].0,
                    screen_points[1].1,
                    line.color,
                    8,
                );
            } else {
                canvas.draw_polyline(&screen_points, false, line.color);
            }
        }
    }

    for circle in &scene.circles {
        let center = to_screen(
            &circle.center,
            width,
            height,
            margin,
            &scene.bounds,
            scene.y_up,
        );
        let radius_world = ((circle.radius_point.x - circle.center.x).powi(2)
            + (circle.radius_point.y - circle.center.y).powi(2))
        .sqrt();
        let radius_pixels =
            (radius_world * screen_scale(width, height, margin, &scene.bounds)).round() as i32;
        if radius_pixels >= 4 {
            canvas.draw_circle_outline(center.0, center.1, radius_pixels, circle.color);
        }
    }

    for point in &scene.points {
        let (x, y) = to_screen(
            &point.position,
            width,
            height,
            margin,
            &scene.bounds,
            scene.y_up,
        );
        canvas.draw_circle_filled(x, y, 4, [255, 60, 40, 255]);
    }

    for label in &scene.labels {
        let (x, y) = to_screen(
            &label.anchor,
            width,
            height,
            margin,
            &scene.bounds,
            scene.y_up,
        );
        if let Some(font) = &font_renderer {
            font.draw_text(&mut canvas, x + 6, y - 10, &label.text, label.color, 18.0);
        } else {
            canvas.draw_text(x + 6, y - 10, &label.text, label.color);
        }
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

fn draw_grid(
    canvas: &mut Canvas,
    width: u32,
    height: u32,
    margin: f64,
    bounds: &Bounds,
    origin: Option<&PointRecord>,
) {
    let x_label_step = if bounds.max_x - bounds.min_x > 20.0 {
        5.0
    } else {
        2.0
    };
    let y_label_step = if bounds.max_y - bounds.min_y > 12.0 {
        2.0
    } else {
        1.0
    };

    let min_x = bounds.min_x.floor() as i32;
    let max_x = bounds.max_x.ceil() as i32;
    let min_y = bounds.min_y.floor() as i32;
    let max_y = bounds.max_y.ceil() as i32;

    for x in min_x..=max_x {
        let screen = to_screen(
            &PointRecord {
                x: x as f64,
                y: bounds.min_y,
            },
            width,
            height,
            margin,
            bounds,
            true,
        );
        let color = if x == 0 {
            [40, 40, 40, 255]
        } else {
            [200, 200, 200, 255]
        };
        canvas.draw_line(
            screen.0,
            margin as i32,
            screen.0,
            (height as f64 - margin) as i32,
            color,
        );
        if (x as f64) % x_label_step == 0.0 && x != 0 {
            canvas.draw_text(
                screen.0 - 6,
                (height as f64 - margin + 8.0) as i32,
                &x.to_string(),
                [20, 20, 20, 255],
            );
        }
    }

    for y in min_y..=max_y {
        let screen = to_screen(
            &PointRecord {
                x: bounds.min_x,
                y: y as f64,
            },
            width,
            height,
            margin,
            bounds,
            true,
        );
        let color = if y == 0 {
            [40, 40, 40, 255]
        } else {
            [200, 200, 200, 255]
        };
        canvas.draw_line(
            margin as i32,
            screen.1,
            (width as f64 - margin) as i32,
            screen.1,
            color,
        );
        if (y as f64) % y_label_step == 0.0 && y != 0 {
            canvas.draw_text(
                (width as f64 / 2.0 - 12.0) as i32,
                screen.1 - 4,
                &y.to_string(),
                [20, 20, 20, 255],
            );
        }
    }

    if let Some(origin) = origin {
        let origin_screen = to_screen(origin, width, height, margin, bounds, true);
        canvas.draw_circle_filled(origin_screen.0, origin_screen.1, 3, [255, 60, 40, 255]);
    }
}

pub fn encode_png_rgba(width: u32, height: u32, rgba: &[u8]) -> Result<Vec<u8>, String> {
    let expected_len = (width as usize)
        .checked_mul(height as usize)
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or_else(|| "image dimensions overflow".to_string())?;
    if rgba.len() != expected_len {
        return Err(format!(
            "rgba buffer length mismatch: expected {}, got {}",
            expected_len,
            rgba.len()
        ));
    }

    let raw = build_png_scanlines(width, height, rgba);
    let compressed = zlib_store_blocks(&raw);
    let mut png = Vec::new();
    png.extend_from_slice(&[137, 80, 78, 71, 13, 10, 26, 10]);

    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.push(8);
    ihdr.push(6);
    ihdr.push(0);
    ihdr.push(0);
    ihdr.push(0);
    write_png_chunk(&mut png, *b"IHDR", &ihdr);
    write_png_chunk(&mut png, *b"IDAT", &compressed);
    write_png_chunk(&mut png, *b"IEND", &[]);
    Ok(png)
}

fn build_png_scanlines(width: u32, height: u32, rgba: &[u8]) -> Vec<u8> {
    let stride = width as usize * 4;
    let mut raw = Vec::with_capacity((stride + 1) * height as usize);
    for row in 0..height as usize {
        raw.push(0);
        let start = row * stride;
        raw.extend_from_slice(&rgba[start..start + stride]);
    }
    raw
}

fn zlib_store_blocks(raw: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(raw.len() + raw.len() / 65535 * 5 + 16);
    out.extend_from_slice(&[0x78, 0x01]);

    let mut offset = 0usize;
    while offset < raw.len() {
        let remaining = raw.len() - offset;
        let block_len = remaining.min(65_535);
        let final_block = offset + block_len == raw.len();
        out.push(if final_block { 0x01 } else { 0x00 });
        let len = block_len as u16;
        let nlen = !len;
        out.extend_from_slice(&len.to_le_bytes());
        out.extend_from_slice(&nlen.to_le_bytes());
        out.extend_from_slice(&raw[offset..offset + block_len]);
        offset += block_len;
    }

    out.extend_from_slice(&adler32(raw).to_be_bytes());
    out
}

fn write_png_chunk(out: &mut Vec<u8>, chunk_type: [u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    out.extend_from_slice(&chunk_type);
    out.extend_from_slice(data);

    let mut crc_input = Vec::with_capacity(4 + data.len());
    crc_input.extend_from_slice(&chunk_type);
    crc_input.extend_from_slice(data);
    out.extend_from_slice(&crc32(&crc_input).to_be_bytes());
}

fn adler32(data: &[u8]) -> u32 {
    const MOD_ADLER: u32 = 65_521;
    let mut a = 1u32;
    let mut b = 0u32;

    for byte in data {
        a = (a + u32::from(*byte)) % MOD_ADLER;
        b = (b + a) % MOD_ADLER;
    }

    (b << 16) | a
}

fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xffff_ffffu32;
    for byte in data {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg() & 0xedb8_8320;
            crc = (crc >> 1) ^ mask;
        }
    }
    !crc
}
