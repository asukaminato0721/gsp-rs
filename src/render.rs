use crate::format::{
    GspFile, IndexedPathRecord, ObjectGroup, PointRecord, decode_indexed_path, decode_point_record,
    read_f64, read_u16,
};
use crate::png::encode_png_rgba;
use ab_glyph::{Font, FontArc, Glyph, PxScale, ScaleFont, point};
use std::collections::BTreeSet;
use std::fs;
use std::io::Write as _;
use std::path::Path;

const FONT_CANDIDATES: &[&str] = &[
    "/usr/share/fonts/noto/NotoSans-Regular.ttf",
    "/usr/share/fonts/Adwaita/AdwaitaSans-Regular.ttf",
    "/usr/share/fonts/gnu-free/FreeSans.otf",
];

#[derive(Debug, Clone)]
struct GraphTransform {
    origin_raw: PointRecord,
    raw_per_unit: f64,
}

#[derive(Debug, Clone, Copy)]
struct Bounds {
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

#[derive(Debug, Clone)]
struct LineShape {
    points: Vec<PointRecord>,
    color: [u8; 4],
    dashed: bool,
}

#[derive(Debug, Clone)]
struct PolygonShape {
    points: Vec<PointRecord>,
    color: [u8; 4],
}

#[derive(Debug, Clone)]
struct CircleShape {
    center: PointRecord,
    radius_point: PointRecord,
    color: [u8; 4],
}

#[derive(Debug, Clone)]
struct TextLabel {
    anchor: PointRecord,
    text: String,
    color: [u8; 4],
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

    let groups = file.object_groups();
    let point_map = collect_point_objects(file, &groups);
    let raw_anchors = collect_raw_object_anchors(file, &groups, &point_map);
    let graph = detect_graph_transform(file, &groups, &raw_anchors);
    let graph_mode = graph.is_some() && has_graph_classes(&groups);
    let graph_ref = if graph_mode { graph.clone() } else { None };
    let large_non_graph = !graph_mode && file.records.len() > 10_000;

    let polyline_kinds: &[u32] = if large_non_graph {
        &[2]
    } else {
        &[2]
    };
    let polygon_kinds: &[u32] = &[8];

    let polylines =
        collect_line_shapes(file, &groups, &raw_anchors, polyline_kinds, !graph_mode && !large_non_graph);
    let derived_segments = if large_non_graph {
        collect_derived_segments(file, &groups, &point_map, &[24])
    } else {
        Vec::new()
    };
    let measurements = if graph_mode {
        collect_line_shapes(file, &groups, &raw_anchors, &[58], false)
    } else {
        Vec::new()
    };
    let axes = if graph_mode {
        collect_line_shapes(file, &groups, &raw_anchors, &[61], false)
    } else {
        Vec::new()
    };
    let polygons =
        collect_polygon_shapes(file, &groups, &raw_anchors, polygon_kinds, !graph_mode && !large_non_graph);
    let circles = collect_circle_shapes(file, &groups, &point_map);
    let mut labels = if graph_mode {
        collect_labels(file, &groups, &raw_anchors)
    } else {
        Vec::new()
    };
    if graph_mode && let (Some(circle), Some(formula_index)) = (
        circles.first(),
        labels.iter().position(|label| label.text.contains("AB:")),
    ) {
        let circumference = 2.0
            * std::f64::consts::PI
            * distance_world(&circle.center, &circle.radius_point, &graph);
        let anchor = PointRecord {
            x: labels[formula_index].anchor.x,
            y: labels[formula_index].anchor.y
                - 0.9 * graph.as_ref().map(|g| g.raw_per_unit).unwrap_or(1.0),
        };
        labels.insert(
            formula_index,
            TextLabel {
                anchor,
                text: format!("AB perimeter = {:.2} cm", circumference),
                color: [30, 30, 30, 255],
            },
        );
    }

    let mut bounds = collect_bounds(
        &graph_ref,
        &polylines,
        &measurements,
        &axes,
        &polygons,
        &circles,
        &labels,
    );
    expand_bounds(&mut bounds);

    let mut canvas = Canvas::new(width, height, [250, 250, 248, 255]);
    let font_renderer = FontRenderer::load();
    let margin = 32.0;
    if graph_mode {
        draw_grid(&mut canvas, width, height, margin, &bounds, &graph_ref);
    }

    for polygon in &polygons {
        let screen_points = polygon
            .points
            .iter()
            .map(|point| to_screen(point, width, height, margin, &bounds, &graph_ref))
            .collect::<Vec<_>>();
        canvas.fill_polygon(&screen_points, polygon.color);
        canvas.draw_polyline(&screen_points, true, darken(polygon.color, 80));
    }

    for line in polylines
        .iter()
        .chain(derived_segments.iter())
        .chain(measurements.iter())
        .chain(axes.iter())
    {
        let screen_points = line
            .points
            .iter()
            .map(|point| to_screen(point, width, height, margin, &bounds, &graph_ref))
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

    for circle in &circles {
        let center = to_screen(&circle.center, width, height, margin, &bounds, &graph_ref);
        let radius_world = distance_world(&circle.center, &circle.radius_point, &graph_ref);
        let radius_pixels =
            (radius_world * screen_scale(width, height, margin, &bounds)).round() as i32;
        if radius_pixels >= 4 {
            canvas.draw_circle_outline(center.0, center.1, radius_pixels, circle.color);
        }
    }

    for point in point_map.iter().filter_map(|point| point.as_ref()) {
        let (x, y) = to_screen(point, width, height, margin, &bounds, &graph_ref);
        canvas.draw_circle_filled(x, y, 4, [255, 60, 40, 255]);
    }

    for label in &labels {
        let (x, y) = to_screen(&label.anchor, width, height, margin, &bounds, &graph_ref);
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

fn collect_point_objects(file: &GspFile, groups: &[ObjectGroup]) -> Vec<Option<PointRecord>> {
    groups
        .iter()
        .map(|group| {
            if (group.header.class_id & 0xffff) != 0 {
                return None;
            }
            group.records.iter().find_map(|record| {
                (record.record_type == 0x0899)
                    .then(|| decode_point_record(record.payload(&file.data)))
                    .flatten()
            })
        })
        .collect()
}

fn collect_raw_object_anchors(
    file: &GspFile,
    groups: &[ObjectGroup],
    point_map: &[Option<PointRecord>],
) -> Vec<Option<PointRecord>> {
    let mut anchors = Vec::with_capacity(groups.len());
    for (index, group) in groups.iter().enumerate() {
        let anchor = if let Some(point) = point_map.get(index).and_then(|point| point.clone()) {
            Some(point)
        } else if let Some(anchor) = decode_bbox_anchor_raw(file, group) {
            Some(anchor)
        } else {
            find_indexed_path(file, group).and_then(|path| {
                path.refs.iter().rev().find_map(|object_ref| {
                    anchors.get(object_ref.saturating_sub(1)).cloned().flatten()
                })
            })
        };
        anchors.push(anchor);
    }
    anchors
}

fn collect_line_shapes(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
    kinds: &[u32],
    fallback_generic: bool,
) -> Vec<LineShape> {
    groups
        .iter()
        .filter(|group| {
            let kind = group.header.class_id & 0xffff;
            kinds.contains(&kind)
                || (fallback_generic
                    && !matches!(kind, 0 | 3 | 8)
                    && find_indexed_path(file, group)
                        .map(|path| path.refs.len() == 2)
                        .unwrap_or(false))
        })
        .filter_map(|group| {
            let path = find_indexed_path(file, group)?;
            let points = path
                .refs
                .iter()
                .filter_map(|object_ref| {
                    anchors.get(object_ref.saturating_sub(1)).cloned().flatten()
                })
                .collect::<Vec<_>>();
            (points.len() >= 2).then_some(LineShape {
                points,
                color: if fallback_generic && !kinds.contains(&(group.header.class_id & 0xffff)) {
                    [40, 40, 40, 255]
                } else {
                    color_from_style(group.header.style_b)
                },
                dashed: (group.header.class_id & 0xffff) == 58,
            })
        })
        .collect()
}

fn collect_polygon_shapes(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
    kinds: &[u32],
    fallback_generic: bool,
) -> Vec<PolygonShape> {
    groups
        .iter()
        .filter(|group| {
            let kind = group.header.class_id & 0xffff;
            kinds.contains(&kind)
                || (fallback_generic
                    && !matches!(kind, 0 | 2 | 3)
                    && find_indexed_path(file, group)
                        .map(|path| path.refs.len() >= 3)
                        .unwrap_or(false))
        })
        .filter_map(|group| {
            let path = find_indexed_path(file, group)?;
            let points = path
                .refs
                .iter()
                .filter_map(|object_ref| {
                    anchors.get(object_ref.saturating_sub(1)).cloned().flatten()
                })
                .collect::<Vec<_>>();
            (points.len() >= 3).then_some(PolygonShape {
                points,
                color: if fallback_generic && (group.header.class_id & 0xffff) != 8 {
                    [170, 220, 170, 255]
                } else {
                    color_from_style(group.header.style_b)
                },
            })
        })
        .collect()
}

fn collect_circle_shapes(
    file: &GspFile,
    groups: &[ObjectGroup],
    point_map: &[Option<PointRecord>],
) -> Vec<CircleShape> {
    groups
        .iter()
        .filter(|group| (group.header.class_id & 0xffff) == 3)
        .filter_map(|group| {
            let path = find_indexed_path(file, group)?;
            if path.refs.len() != 2 {
                return None;
            }
            let center = point_map.get(path.refs[0].saturating_sub(1))?.clone()?;
            let radius_point = point_map.get(path.refs[1].saturating_sub(1))?.clone()?;
            Some(CircleShape {
                center,
                radius_point,
                color: color_from_style(group.header.style_b),
            })
        })
        .collect()
}

fn collect_derived_segments(
    file: &GspFile,
    groups: &[ObjectGroup],
    point_map: &[Option<PointRecord>],
    kinds: &[u32],
) -> Vec<LineShape> {
    let refs = groups
        .iter()
        .map(|group| find_indexed_path(file, group).map(|path| path.refs).unwrap_or_default())
        .collect::<Vec<_>>();
    let class_ids = groups
        .iter()
        .map(|group| group.header.class_id & 0xffff)
        .collect::<Vec<_>>();

    fn descend_points(
        ordinal: usize,
        refs: &[Vec<usize>],
        point_map: &[Option<PointRecord>],
        memo: &mut Vec<Option<Vec<PointRecord>>>,
        visiting: &mut BTreeSet<usize>,
    ) -> Vec<PointRecord> {
        if let Some(cached) = &memo[ordinal - 1] {
            return cached.clone();
        }
        if !visiting.insert(ordinal) {
            return Vec::new();
        }

        let mut points = Vec::new();
        if let Some(point) = point_map.get(ordinal - 1).and_then(|point| point.clone()) {
            points.push(point);
        } else {
            for child in &refs[ordinal - 1] {
                if *child > 0 && *child <= refs.len() {
                    points.extend(descend_points(*child, refs, point_map, memo, visiting));
                }
            }
        }

        visiting.remove(&ordinal);
        points.sort_by(|a, b| {
            a.x.partial_cmp(&b.x)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.y.partial_cmp(&b.y).unwrap_or(std::cmp::Ordering::Equal))
        });
        points.dedup_by(|a, b| (a.x - b.x).abs() < 0.001 && (a.y - b.y).abs() < 0.001);
        memo[ordinal - 1] = Some(points.clone());
        points
    }

    let mut memo = vec![None; groups.len()];
    let mut seen = BTreeSet::<((i32, i32), (i32, i32))>::new();
    let mut segments = Vec::new();

    for (index, class_id) in class_ids.iter().enumerate() {
        if !kinds.contains(class_id) {
            continue;
        }
        let points =
            descend_points(index + 1, &refs, point_map, &mut memo, &mut BTreeSet::new());
        if points.len() < 2 || points.len() > 12 {
            continue;
        }

        let mut best = None;
        let mut best_dist = -1.0_f64;
        for i in 0..points.len() {
            for j in i + 1..points.len() {
                let dx = points[i].x - points[j].x;
                let dy = points[i].y - points[j].y;
                let dist = dx * dx + dy * dy;
                if dist > best_dist {
                    best_dist = dist;
                    best = Some((points[i].clone(), points[j].clone()));
                }
            }
        }

        let Some((a, b)) = best else { continue };
        let a_key = (a.x.round() as i32, a.y.round() as i32);
        let b_key = (b.x.round() as i32, b.y.round() as i32);
        let key = if a_key <= b_key {
            (a_key, b_key)
        } else {
            (b_key, a_key)
        };
        if !seen.insert(key) {
            continue;
        }

        let color = match *class_id {
            24 => [20, 20, 20, 255],
            48 => [70, 70, 70, 255],
            75 => [120, 120, 120, 255],
            _ => [60, 60, 60, 255],
        };
        segments.push(LineShape {
            points: vec![a, b],
            color,
            dashed: false,
        });
    }

    segments
}

fn collect_labels(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
) -> Vec<TextLabel> {
    let mut labels = Vec::new();
    for group in groups {
        let kind = group.header.class_id & 0xffff;
        match kind {
            0 | 40 | 51 => {
                if let Some(text) = group
                    .records
                    .iter()
                    .find(|record| record.record_type == 0x08fc)
                    .and_then(|record| extract_rich_text(record.payload(&file.data)))
                {
                    let anchor = group
                        .records
                        .iter()
                        .find(|record| record.record_type == 0x08fc)
                        .and_then(|record| decode_text_anchor(record.payload(&file.data)))
                        .or_else(|| {
                            anchors
                                .get(group.ordinal.saturating_sub(1))
                                .cloned()
                                .flatten()
                        })
                        .or_else(|| {
                            find_indexed_path(file, group).and_then(|path| {
                                path.refs.iter().rev().find_map(|object_ref| {
                                    anchors.get(object_ref.saturating_sub(1)).cloned().flatten()
                                })
                            })
                        });
                    if let Some(anchor) = anchor {
                        labels.push(TextLabel {
                            anchor,
                            text,
                            color: [30, 30, 30, 255],
                        });
                    }
                }
            }
            52 | 54 => {
                if let Some(value) = group
                    .records
                    .iter()
                    .find(|record| record.record_type == 0x07d3 && record.length == 12)
                    .and_then(|record| decode_measurement_value(record.payload(&file.data)))
                {
                    let anchor = anchors
                        .get(group.ordinal.saturating_sub(1))
                        .cloned()
                        .flatten()
                        .or_else(|| {
                            find_indexed_path(file, group).and_then(|path| {
                                path.refs.iter().find_map(|object_ref| {
                                    anchors.get(object_ref.saturating_sub(1)).cloned().flatten()
                                })
                            })
                        });
                    if let Some(anchor) = anchor {
                        labels.push(TextLabel {
                            anchor,
                            text: format_number(value),
                            color: [60, 60, 60, 255],
                        });
                    }
                }
            }
            _ => {}
        }
    }
    labels
}

fn find_indexed_path(file: &GspFile, group: &ObjectGroup) -> Option<IndexedPathRecord> {
    group
        .records
        .iter()
        .find_map(|record| match record.record_type {
            0x07d2 | 0x07d3 => decode_indexed_path(record.record_type, record.payload(&file.data)),
            _ => None,
        })
}

fn decode_bbox_anchor_raw(file: &GspFile, group: &ObjectGroup) -> Option<PointRecord> {
    let payload = group
        .records
        .iter()
        .find(|record| matches!(record.record_type, 0x0898 | 0x0903))
        .map(|record| record.payload(&file.data))?;
    if payload.len() < 8 {
        return None;
    }
    let x0 = read_u16(payload, payload.len() - 8) as f64;
    let y0 = read_u16(payload, payload.len() - 6) as f64;
    let x1 = read_u16(payload, payload.len() - 4) as f64;
    let y1 = read_u16(payload, payload.len() - 2) as f64;
    Some(PointRecord {
        x: (x0 + x1) / 2.0,
        y: (y0 + y1) / 2.0,
    })
}

fn detect_graph_transform(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
) -> Option<GraphTransform> {
    let raw_per_unit = groups
        .iter()
        .filter(|group| matches!(group.header.class_id & 0xffff, 52 | 54))
        .find_map(|group| {
            group
                .records
                .iter()
                .find(|record| record.record_type == 0x07d3 && record.length == 12)
                .and_then(|record| decode_measurement_value(record.payload(&file.data)))
        })?;

    let origin_raw = groups
        .iter()
        .find(|group| matches!(group.header.class_id & 0xffff, 52 | 54))
        .and_then(|group| {
            find_indexed_path(file, group).and_then(|path| {
                path.refs.iter().find_map(|object_ref| {
                    anchors.get(object_ref.saturating_sub(1)).cloned().flatten()
                })
            })
        })?;

    Some(GraphTransform {
        origin_raw,
        raw_per_unit,
    })
}

fn has_graph_classes(groups: &[ObjectGroup]) -> bool {
    groups
        .iter()
        .any(|group| matches!(group.header.class_id & 0xffff, 40 | 52 | 54 | 58 | 61))
}

fn collect_bounds(
    graph: &Option<GraphTransform>,
    polylines: &[LineShape],
    measurements: &[LineShape],
    axes: &[LineShape],
    polygons: &[PolygonShape],
    circles: &[CircleShape],
    labels: &[TextLabel],
) -> Bounds {
    let mut points = Vec::<PointRecord>::new();
    for shape in polylines
        .iter()
        .chain(measurements.iter())
        .chain(axes.iter())
    {
        points.extend(shape.points.iter().cloned());
    }
    for shape in polygons {
        points.extend(shape.points.iter().cloned());
    }
    for circle in circles {
        points.push(circle.center.clone());
        points.push(circle.radius_point.clone());
    }
    for label in labels {
        points.push(label.anchor.clone());
    }
    if points.is_empty() {
        points.push(PointRecord { x: 0.0, y: 0.0 });
        points.push(PointRecord { x: 1.0, y: 1.0 });
    }

    let world_points = points
        .iter()
        .map(|point| to_world(point, graph))
        .collect::<Vec<_>>();
    let mut bounds = Bounds {
        min_x: world_points[0].x,
        max_x: world_points[0].x,
        min_y: world_points[0].y,
        max_y: world_points[0].y,
    };
    for point in &world_points {
        bounds.min_x = bounds.min_x.min(point.x);
        bounds.max_x = bounds.max_x.max(point.x);
        bounds.min_y = bounds.min_y.min(point.y);
        bounds.max_y = bounds.max_y.max(point.y);
    }
    bounds
}

fn expand_bounds(bounds: &mut Bounds) {
    if (bounds.max_x - bounds.min_x).abs() < f64::EPSILON {
        bounds.max_x += 1.0;
        bounds.min_x -= 1.0;
    }
    if (bounds.max_y - bounds.min_y).abs() < f64::EPSILON {
        bounds.max_y += 1.0;
        bounds.min_y -= 1.0;
    }
    let margin_x = (bounds.max_x - bounds.min_x) * 0.1 + 1.0;
    let margin_y = (bounds.max_y - bounds.min_y) * 0.1 + 1.0;
    bounds.min_x -= margin_x;
    bounds.max_x += margin_x;
    bounds.min_y -= margin_y;
    bounds.max_y += margin_y;
}

fn draw_grid(
    canvas: &mut Canvas,
    width: u32,
    height: u32,
    margin: f64,
    bounds: &Bounds,
    graph: &Option<GraphTransform>,
) {
    let step = 1.0;
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
            &None,
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
            &None,
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

    if let Some(graph) = graph {
        let origin = to_world(&graph.origin_raw, &Some(graph.clone()));
        let origin_screen = to_screen(&origin, width, height, margin, bounds, &None);
        canvas.draw_circle_filled(origin_screen.0, origin_screen.1, 3, [255, 60, 40, 255]);
    }

    let _ = step;
}

fn to_world(point: &PointRecord, graph: &Option<GraphTransform>) -> PointRecord {
    if let Some(graph) = graph {
        PointRecord {
            x: (point.x - graph.origin_raw.x) / graph.raw_per_unit,
            y: (graph.origin_raw.y - point.y) / graph.raw_per_unit,
        }
    } else {
        point.clone()
    }
}

fn to_screen(
    point: &PointRecord,
    width: u32,
    height: u32,
    margin: f64,
    bounds: &Bounds,
    graph: &Option<GraphTransform>,
) -> (i32, i32) {
    let world = to_world(point, graph);
    let scale = screen_scale(width, height, margin, bounds);
    let x = margin + (world.x - bounds.min_x) * scale;
    let y = height as f64 - margin - (world.y - bounds.min_y) * scale;
    (x.round() as i32, y.round() as i32)
}

fn screen_scale(width: u32, height: u32, margin: f64, bounds: &Bounds) -> f64 {
    let usable_width = (width as f64 - margin * 2.0).max(1.0);
    let usable_height = (height as f64 - margin * 2.0).max(1.0);
    let span_x = (bounds.max_x - bounds.min_x).max(1.0);
    let span_y = (bounds.max_y - bounds.min_y).max(1.0);
    f64::min(usable_width / span_x, usable_height / span_y)
}

fn distance_world(a: &PointRecord, b: &PointRecord, graph: &Option<GraphTransform>) -> f64 {
    let aw = to_world(a, graph);
    let bw = to_world(b, graph);
    ((aw.x - bw.x).powi(2) + (aw.y - bw.y).powi(2)).sqrt()
}

fn extract_rich_text(payload: &[u8]) -> Option<String> {
    let text = String::from_utf8_lossy(payload);
    let start = text.find('<')?;
    let markup = text[start..].trim_end_matches('\0');

    if markup.starts_with("<VL") {
        return extract_simple_text(markup);
    }

    let parsed = parse_markup(markup);
    let mut cleaned = parsed
        .replace(['\u{2013}', '\u{2014}'], "-")
        .replace("厘米", "cm");

    if let Some(first) = cleaned.find("AB:")
        && let Some(second_rel) = cleaned[first + 3..].find("AB:") {
            cleaned.truncate(first + 3 + second_rel);
        }

    cleaned = cleaned
        .replace("  ", " ")
        .replace("( ", "(")
        .replace(" )", ")")
        .replace(" + -", " -")
        .trim()
        .to_string();

    (!cleaned.is_empty()).then_some(cleaned)
}

fn decode_measurement_value(payload: &[u8]) -> Option<f64> {
    (payload.len() == 12).then(|| read_f64(payload, 4))
}

fn decode_text_anchor(payload: &[u8]) -> Option<PointRecord> {
    if payload.len() < 16 {
        return None;
    }
    Some(PointRecord {
        x: read_u16(payload, 12) as f64,
        y: read_u16(payload, 14) as f64,
    })
}

fn extract_simple_text(markup: &str) -> Option<String> {
    let start = markup.find("<T")?;
    let tail = &markup[start + 2..];
    let x_index = tail.find('x')?;
    let end = tail[x_index + 1..].find('>')?;
    Some(tail[x_index + 1..x_index + 1 + end].to_string())
}

fn parse_markup(markup: &str) -> String {
    fn parse_seq(s: &str, mut index: usize, stop_on_gt: bool) -> (Vec<String>, usize) {
        let bytes = s.as_bytes();
        let mut parts = Vec::new();

        while index < bytes.len() {
            if stop_on_gt && bytes[index] == b'>' {
                return (parts, index + 1);
            }
            if bytes[index] != b'<' {
                index += 1;
                continue;
            }
            if index + 1 >= bytes.len() {
                break;
            }

            match bytes[index + 1] as char {
                'T' => {
                    let mut end = index + 2;
                    while end < bytes.len() && bytes[end] != b'>' {
                        end += 1;
                    }
                    let token = &s[index + 2..end];
                    if let Some(x_index) = token.find('x') {
                        parts.push(token[x_index + 1..].to_string());
                    }
                    index = end.saturating_add(1);
                }
                '!' => {
                    let mut end = index + 2;
                    while end < bytes.len() && bytes[end] != b'>' {
                        end += 1;
                    }
                    index = end.saturating_add(1);
                }
                _ => {
                    let mut name_end = index + 1;
                    while name_end < bytes.len()
                        && bytes[name_end] != b'<'
                        && bytes[name_end] != b'>'
                    {
                        name_end += 1;
                    }
                    let name = &s[index + 1..name_end];
                    let (inner_parts, next_index) =
                        if name_end < bytes.len() && bytes[name_end] == b'<' {
                            parse_seq(s, name_end, true)
                        } else {
                            (Vec::new(), name_end.saturating_add(1))
                        };
                    index = next_index;

                    let mut inner = inner_parts.join("");
                    if name.starts_with('+') && !inner.is_empty() {
                        let split = inner
                            .char_indices()
                            .rev()
                            .find(|(_, ch)| !ch.is_ascii_digit())
                            .map(|(i, _)| i + 1)
                            .unwrap_or(0);
                        if split < inner.len() {
                            let exp = inner.split_off(split);
                            inner.push('^');
                            inner.push_str(&exp);
                        }
                    }
                    if !inner.is_empty() {
                        parts.push(inner);
                    }
                }
            }
        }

        (parts, index)
    }

    let (parts, _) = parse_seq(markup, 0, false);
    parts.join("")
}

fn format_number(value: f64) -> String {
    if (value.fract()).abs() < 0.005 {
        format!("{value:.0}")
    } else {
        format!("{value:.2}")
    }
}

fn color_from_style(style: u32) -> [u8; 4] {
    [
        (style & 0xff) as u8,
        ((style >> 8) & 0xff) as u8,
        ((style >> 16) & 0xff) as u8,
        255,
    ]
}

fn darken(rgba: [u8; 4], amount: u8) -> [u8; 4] {
    [
        rgba[0].saturating_sub(amount),
        rgba[1].saturating_sub(amount),
        rgba[2].saturating_sub(amount),
        rgba[3],
    ]
}

impl FontRenderer {
    fn load() -> Option<Self> {
        for path in FONT_CANDIDATES {
            if let Ok(bytes) = fs::read(path)
                && let Ok(font) = FontArc::try_from_vec(bytes) {
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
