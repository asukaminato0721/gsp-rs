use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fmt::Write as _;
use std::fs;
use std::io::Write as _;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::process;

const KNOWN_REFERENCE_TERMS: &[&str] = &[
    "Action Buttons",
    "Angle",
    "Circle",
    "CoordinateDistance",
    "Coordinates",
    "Distance",
    "Function",
    "Graph",
    "Line",
    "Locus",
    "Measure",
    "Midpoint",
    "Parallel",
    "Parameter",
    "Perpendicular",
    "Point",
    "Polygon",
    "Segment",
];

fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error}");
        process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let config = Config::parse(env::args_os().skip(1))?;
    let data = fs::read(&config.gsp_path)
        .map_err(|error| format!("failed to read {}: {error}", config.gsp_path.display()))?;
    let file = GspFile::parse(&data)?;
    let exe_terms = config
        .reference_exe
        .as_ref()
        .map(|path| analyze_reference_exe(path))
        .transpose()?;

    if let Some(render_path) = &config.render_path {
        render_points_to_png(&file, render_path, config.render_width, config.render_height)?;
    }

    println!("{}", render_report(&config, &file, exe_terms.as_ref()));
    Ok(())
}

#[derive(Debug)]
struct Config {
    gsp_path: PathBuf,
    reference_exe: Option<PathBuf>,
    render_path: Option<PathBuf>,
    render_width: u32,
    render_height: u32,
}

impl Config {
    fn parse(args: impl Iterator<Item = impl Into<std::ffi::OsString>>) -> Result<Self, String> {
        let raw_args: Vec<_> = args.map(Into::into).collect();
        if raw_args.is_empty() {
            return Err(Self::usage());
        }

        let mut gsp_path = None;
        let mut reference_exe = None;
        let mut render_path = None;
        let mut render_width = 800_u32;
        let mut render_height = 600_u32;
        let mut index = 0usize;

        while index < raw_args.len() {
            let current = PathBuf::from(&raw_args[index]);
            let current_text = current.to_string_lossy();

            match current_text.as_ref() {
                "-h" | "--help" => return Err(Self::usage()),
                "--reference-exe" => {
                    index += 1;
                    let Some(path) = raw_args.get(index) else {
                        return Err("--reference-exe requires a path".to_string());
                    };
                    reference_exe = Some(PathBuf::from(path));
                }
                "--render" => {
                    index += 1;
                    let Some(path) = raw_args.get(index) else {
                        return Err("--render requires a path".to_string());
                    };
                    render_path = Some(PathBuf::from(path));
                }
                "--width" => {
                    index += 1;
                    let Some(value) = raw_args.get(index) else {
                        return Err("--width requires an integer".to_string());
                    };
                    render_width = parse_u32_arg(value, "--width")?;
                }
                "--height" => {
                    index += 1;
                    let Some(value) = raw_args.get(index) else {
                        return Err("--height requires an integer".to_string());
                    };
                    render_height = parse_u32_arg(value, "--height")?;
                }
                _ if current_text.starts_with("--") => {
                    return Err(format!("unknown option: {current_text}\n\n{}", Self::usage()));
                }
                _ if gsp_path.is_none() => gsp_path = Some(current),
                _ => {
                    return Err(format!(
                        "unexpected positional argument: {current_text}\n\n{}",
                        Self::usage()
                    ));
                }
            }

            index += 1;
        }

        let Some(gsp_path) = gsp_path else {
            return Err(Self::usage());
        };

        Ok(Self {
            gsp_path,
            reference_exe,
            render_path,
            render_width,
            render_height,
        })
    }

    fn usage() -> String {
        "usage: gsp-rs <path/to/file.gsp> [--reference-exe path/to/GSP5Chs.exe] [--render out.png] [--width 800] [--height 600]".to_string()
    }
}

#[derive(Debug)]
struct GspFile {
    magic: String,
    data: Vec<u8>,
    records: Vec<Record>,
}

impl GspFile {
    fn parse(data: &[u8]) -> Result<Self, String> {
        if data.len() < 12 {
            return Err(format!("file is too small to be a GSP file: {} bytes", data.len()));
        }

        let magic = String::from_utf8_lossy(&data[..4]).to_string();
        if magic != "GSP4" {
            return Err(format!("unexpected magic {magic:?}, expected \"GSP4\""));
        }

        let records = parse_records(data)?;
        Ok(Self {
            magic,
            data: data.to_vec(),
            records,
        })
    }

    fn record_type_counts(&self) -> Vec<RecordTypeCount> {
        let mut counts = BTreeMap::<u32, usize>::new();
        for record in &self.records {
            *counts.entry(record.record_type).or_default() += 1;
        }

        let mut entries = counts
            .into_iter()
            .map(|(record_type, count)| RecordTypeCount { record_type, count })
            .collect::<Vec<_>>();
        entries.sort_by(|left, right| {
            right
                .count
                .cmp(&left.count)
                .then_with(|| left.record_type.cmp(&right.record_type))
        });
        entries
    }

    fn point_records(&self) -> Vec<PointRecord> {
        self.records
            .iter()
            .filter_map(|record| {
                if record.record_type == 0x0899 {
                    decode_point_record(record.payload(&self.data))
                } else {
                    None
                }
            })
            .collect()
    }
}

#[derive(Debug, Clone)]
struct Record {
    offset: usize,
    length: u32,
    record_type: u32,
    payload_range: Range<usize>,
}

impl Record {
    fn payload<'a>(&self, data: &'a [u8]) -> &'a [u8] {
        &data[self.payload_range.clone()]
    }
}

#[derive(Debug)]
struct RecordTypeCount {
    record_type: u32,
    count: usize,
}

#[derive(Debug)]
struct HeaderRecord {
    words_u16: Vec<u16>,
    words_u32: Vec<u32>,
}

#[derive(Debug)]
struct SymbolSlotRecord {
    slot_index: u32,
    value: u16,
    flag: u16,
    reserved: u16,
}

#[derive(Debug)]
struct PaletteEntryRecord {
    slot_index: u16,
    rgba: [u8; 4],
}

#[derive(Debug)]
struct PointRecord {
    x: f64,
    y: f64,
}

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

#[derive(Debug, Clone)]
struct ExtractedString {
    offset: usize,
    byte_len: usize,
    text: String,
    prefix_len16: Option<u16>,
    prefix_len32: Option<u32>,
}

fn render_report(config: &Config, file: &GspFile, exe_terms: Option<&BTreeSet<String>>) -> String {
    let mut output = String::new();
    let zero_len_count = file.records.iter().filter(|record| record.length == 0).count();

    let _ = writeln!(output, "GSP analysis");
    let _ = writeln!(output, "  path: {}", config.gsp_path.display());
    let _ = writeln!(
        output,
        "  size: {} bytes (0x{:x})",
        file.data.len(),
        file.data.len()
    );
    let _ = writeln!(output, "  magic: {}", file.magic);
    let _ = writeln!(output, "  records: {}", file.records.len());
    let _ = writeln!(output, "  zero_length_records: {}", zero_len_count);
    let _ = writeln!(
        output,
        "  distinct_record_types: {}",
        file.record_type_counts().len()
    );
    let _ = writeln!(output, "  point_records: {}", file.point_records().len());
    if let Some(render_path) = &config.render_path {
        let _ = writeln!(output, "  rendered_png: {}", render_path.display());
    }

    if let Some(header_record) = file.records.first().filter(|record| record.record_type == 0x0384) {
        if let Some(header) = decode_header_record(header_record.payload(&file.data)) {
            let _ = writeln!(output);
            let _ = writeln!(output, "Header");
            let _ = writeln!(
                output,
                "  @0x{:04x} type=0x{:04x} {} len=0x{:x}",
                header_record.offset,
                header_record.record_type,
                record_name(header_record.record_type),
                header_record.length
            );
            let _ = writeln!(output, "  words_u16: {:?}", header.words_u16);
            let _ = writeln!(output, "  words_u32: {:?}", header.words_u32);
        }
    }

    let _ = writeln!(output);
    let _ = writeln!(output, "Record Type Counts");
    for entry in file.record_type_counts().iter().take(20) {
        let _ = writeln!(
            output,
            "  0x{:04x} {:<26} count={}",
            entry.record_type,
            record_name(entry.record_type),
            entry.count
        );
    }

    let _ = writeln!(output);
    let _ = writeln!(output, "Records");
    for record in &file.records {
        let _ = writeln!(
            output,
            "  @0x{:04x} len=0x{:04x} type=0x{:04x} {}",
            record.offset,
            record.length,
            record.record_type,
            record_name(record.record_type)
        );

        let payload = record.payload(&file.data);
        for detail in describe_record(record, payload) {
            let _ = writeln!(output, "    {detail}");
        }
    }

    if let Some(terms) = exe_terms {
        let _ = writeln!(output);
        let _ = writeln!(
            output,
            "Reference Terms Found In {}",
            config.reference_exe.as_ref().unwrap().display()
        );
        if terms.is_empty() {
            let _ = writeln!(output, "  none");
        } else {
            for term in terms {
                let _ = writeln!(output, "  {term}");
            }
        }
    }

    output
}

fn parse_records(data: &[u8]) -> Result<Vec<Record>, String> {
    let mut records = Vec::new();
    let mut offset = 4usize;

    while offset < data.len() {
        if offset + 8 > data.len() {
            return Err(format!(
                "truncated record header at 0x{offset:x}: {} trailing byte(s)",
                data.len() - offset
            ));
        }

        let length = read_u32(data, offset);
        let record_type = read_u32(data, offset + 4);
        let payload_start = offset + 8;
        let payload_end = payload_start
            .checked_add(length as usize)
            .ok_or_else(|| format!("record at 0x{offset:x} overflows usize"))?;

        if payload_end > data.len() {
            return Err(format!(
                "record at 0x{offset:x} extends past EOF: len=0x{length:x}, end=0x{payload_end:x}, file=0x{:x}",
                data.len()
            ));
        }

        records.push(Record {
            offset,
            length,
            record_type,
            payload_range: payload_start..payload_end,
        });

        offset = payload_end;
    }

    Ok(records)
}

fn describe_record(record: &Record, payload: &[u8]) -> Vec<String> {
    let mut details = Vec::new();

    match record.record_type {
        0x0384 => {
            if let Some(header) = decode_header_record(payload) {
                details.push(format!("header_words_u16={:?}", header.words_u16));
            }
        }
        0x0386 => {
            let strings = collect_strings(payload);
            if strings.is_empty() {
                details.push("compatibility bundle with no decoded strings".to_string());
            } else {
                for string in strings.iter().take(4) {
                    let prefix = match (string.prefix_len16, string.prefix_len32) {
                        (Some(len16), Some(len32)) => format!("prefix16={len16} prefix32={len32}"),
                        (Some(len16), None) => format!("prefix16={len16}"),
                        (None, Some(len32)) => format!("prefix32={len32}"),
                        (None, None) => "no-prefix".to_string(),
                    };
                    details.push(format!(
                        "string @+0x{:x} bytes={} {} text={:?}",
                        string.offset,
                        string.byte_len,
                        prefix,
                        truncate_text(&string.text, 72)
                    ));
                }
                if strings.len() > 4 {
                    details.push(format!("... {} more strings", strings.len() - 4));
                }
            }
        }
        0x0899 => {
            if let Some(point) = decode_point_record(payload) {
                details.push(format!("point x={:.6} y={:.6}", point.x, point.y));
            }
        }
        0x232c => {
            if let Some(text) = decode_c_string(payload) {
                details.push(format!("build_string={text:?}"));
            }
        }
        0x232f => {
            for string in collect_strings(payload).iter().take(2) {
                details.push(format!("text_blob={:?}", truncate_text(&string.text, 80)));
            }
        }
        0x2724 => {
            if let Some(entry) = decode_palette_entry(payload) {
                details.push(format!(
                    "palette slot={} rgba=[{}, {}, {}, {}]",
                    entry.slot_index,
                    entry.rgba[0],
                    entry.rgba[1],
                    entry.rgba[2],
                    entry.rgba[3]
                ));
            }
        }
        0x273c => {
            let strings = collect_strings(payload);
            if !strings.is_empty() {
                details.push(format!(
                    "font strings={}",
                    strings
                        .iter()
                        .map(|entry| truncate_text(&entry.text, 32))
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
            }
        }
        0x095f..=0x0973 => {
            if let Some(symbol) = decode_symbol_slot(record.record_type, payload) {
                details.push(format!(
                    "symbol slot={} value={} flag={} reserved={}",
                    symbol.slot_index, symbol.value, symbol.flag, symbol.reserved
                ));
            }
        }
        _ => {}
    }

    if details.is_empty() {
        if let Some(text) = decode_c_string(payload) {
            details.push(format!("string={:?}", truncate_text(&text, 80)));
        } else if payload.len() <= 24 && !payload.is_empty() {
            details.push(format!("payload_hex={}", hex_bytes(payload)));
        }
    }

    if details.is_empty() && payload.is_empty() {
        details.push("marker record (no payload)".to_string());
    }

    details
}

fn record_name(record_type: u32) -> &'static str {
    match record_type {
        0x0384 => "GSP_HEADER",
        0x0385 => "END_MARKER_0x385",
        0x0386 => "COMPATIBILITY_BUNDLE",
        0x0387 => "SECTION_MARKER_0x387",
        0x03e8 => "DOC_PREAMBLE_0x3e8",
        0x03e9 => "DOC_TRAILER_0x3e9",
        0x03ec => "OLE_OR_ITEM_REF_0x3ec",
        0x03ed => "OLE_OR_ITEM_REF_0x3ed",
        0x03ee => "RESOURCE_BLOCK_0x3ee",
        0x03ef => "RESOURCE_BLOCK_0x3ef",
        0x07d0 => "OBJECT_GROUP_BEGIN",
        0x07d2 => "OBJECT_INDEX_0x7d2",
        0x07d3 => "OBJECT_INDEX_0x7d3",
        0x07d5 => "OBJECT_AUX_0x7d5",
        0x07d6 => "OBJECT_AUX_0x7d6",
        0x07d7 => "OBJECT_MARKER_0x7d7",
        0x07d8 => "OBJECT_AUX_0x7d8",
        0x0899 => "POINT_F64_PAIR",
        0x2328 => "METADATA_0x2328",
        0x2329 => "METADATA_0x2329",
        0x232b => "DISPLAY_SLOT_0x232b",
        0x232c => "BUILD_INFO_STRING",
        0x232d => "METADATA_0x232d",
        0x232e => "METADATA_0x232e",
        0x232f => "TEXT_BLOB_0x232f",
        0x2330 => "BUILD_INFO_EXTRA",
        0x2331 => "METADATA_0x2331",
        0x2724 => "PALETTE_ENTRY",
        0x273c => "FONT_ENTRY",
        0x095f..=0x0973 => "SYMBOL_SLOT",
        _ => "UNKNOWN",
    }
}

fn decode_header_record(payload: &[u8]) -> Option<HeaderRecord> {
    if payload.len() != 0x1c {
        return None;
    }

    let words_u16 = (0..payload.len())
        .step_by(2)
        .map(|offset| read_u16(payload, offset))
        .collect::<Vec<_>>();
    let words_u32 = (0..payload.len())
        .step_by(4)
        .map(|offset| read_u32(payload, offset))
        .collect::<Vec<_>>();

    Some(HeaderRecord { words_u16, words_u32 })
}

fn decode_symbol_slot(record_type: u32, payload: &[u8]) -> Option<SymbolSlotRecord> {
    if payload.len() != 6 {
        return None;
    }

    Some(SymbolSlotRecord {
        slot_index: record_type - 0x095f,
        value: read_u16(payload, 0),
        flag: read_u16(payload, 2),
        reserved: read_u16(payload, 4),
    })
}

fn decode_palette_entry(payload: &[u8]) -> Option<PaletteEntryRecord> {
    if payload.len() != 6 {
        return None;
    }

    Some(PaletteEntryRecord {
        slot_index: read_u16(payload, 0),
        rgba: [payload[2], payload[3], payload[4], payload[5]],
    })
}

fn decode_point_record(payload: &[u8]) -> Option<PointRecord> {
    if payload.len() != 16 {
        return None;
    }

    Some(PointRecord {
        x: read_f64(payload, 0),
        y: read_f64(payload, 8),
    })
}

fn decode_c_string(payload: &[u8]) -> Option<String> {
    let nul = payload.iter().position(|byte| *byte == 0)?;
    let bytes = &payload[..nul];
    if bytes.len() < 4 {
        return None;
    }
    std::str::from_utf8(bytes).ok().map(ToString::to_string)
}

fn collect_strings(data: &[u8]) -> Vec<ExtractedString> {
    let mut strings = Vec::new();
    let mut offset = 0usize;

    while offset < data.len() {
        let Some(end) = data[offset..].iter().position(|byte| *byte == 0) else {
            break;
        };
        let end = offset + end;
        let bytes = &data[offset..end];

        if is_useful_string(bytes) {
            let text = String::from_utf8_lossy(bytes).to_string();
            strings.push(ExtractedString {
                offset,
                byte_len: bytes.len(),
                prefix_len16: prefix_len16(data, offset, bytes.len()),
                prefix_len32: prefix_len32(data, offset, bytes.len()),
                text,
            });
        }

        offset = end + 1;
    }

    strings
}

fn is_useful_string(bytes: &[u8]) -> bool {
    if bytes.len() < 4 {
        return false;
    }

    let Ok(text) = std::str::from_utf8(bytes) else {
        return false;
    };

    let mut useful = 0usize;
    for ch in text.chars() {
        if ch.is_control() && ch != '\n' && ch != '\r' && ch != '\t' {
            return false;
        }
        if ch.is_alphanumeric() || !ch.is_ascii() {
            useful += 1;
        }
    }

    useful >= 3
}

fn prefix_len16(data: &[u8], offset: usize, len: usize) -> Option<u16> {
    if offset < 2 {
        return None;
    }
    let value = read_u16(data, offset - 2);
    if usize::from(value) == len || usize::from(value) == len + 1 {
        Some(value)
    } else {
        None
    }
}

fn prefix_len32(data: &[u8], offset: usize, len: usize) -> Option<u32> {
    if offset < 4 {
        return None;
    }
    let value = read_u32(data, offset - 4);
    if value as usize == len || value as usize == len + 1 {
        Some(value)
    } else {
        None
    }
}

fn analyze_reference_exe(path: &Path) -> Result<BTreeSet<String>, String> {
    let data =
        fs::read(path).map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    let ascii = collect_ascii_strings(&data);
    let ascii_set = ascii.into_iter().collect::<BTreeSet<_>>();
    let matches = KNOWN_REFERENCE_TERMS
        .iter()
        .filter(|term| ascii_set.contains(**term))
        .map(|term| (*term).to_string())
        .collect();
    Ok(matches)
}

fn render_points_to_png(
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

fn encode_png_rgba(width: u32, height: u32, rgba: &[u8]) -> Result<Vec<u8>, String> {
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

fn parse_u32_arg(value: &std::ffi::OsString, flag: &str) -> Result<u32, String> {
    let text = value.to_string_lossy();
    text.parse::<u32>()
        .map_err(|error| format!("{flag} expects an unsigned integer, got {text:?}: {error}"))
}

fn collect_ascii_strings(data: &[u8]) -> Vec<String> {
    let mut strings = Vec::new();
    let mut start = None;

    for (index, byte) in data.iter().copied().enumerate() {
        let printable = byte.is_ascii_graphic() || byte == b' ';
        if printable {
            if start.is_none() {
                start = Some(index);
            }
            continue;
        }

        if let Some(start_index) = start.take() {
            let slice = &data[start_index..index];
            if slice.len() >= 4 {
                strings.push(String::from_utf8_lossy(slice).to_string());
            }
        }
    }

    if let Some(start_index) = start {
        let slice = &data[start_index..];
        if slice.len() >= 4 {
            strings.push(String::from_utf8_lossy(slice).to_string());
        }
    }

    strings
}

fn truncate_text(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }

    let mut result = text.chars().take(max_chars).collect::<String>();
    result.push_str("...");
    result
}

fn hex_bytes(bytes: &[u8]) -> String {
    bytes.iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<Vec<_>>()
        .join(" ")
}

fn read_u16(data: &[u8], offset: usize) -> u16 {
    let bytes = data
        .get(offset..offset + 2)
        .expect("read_u16 caller must validate bounds");
    u16::from_le_bytes([bytes[0], bytes[1]])
}

fn read_u32(data: &[u8], offset: usize) -> u32 {
    let bytes = data
        .get(offset..offset + 4)
        .expect("read_u32 caller must validate bounds");
    u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
}

fn read_f64(data: &[u8], offset: usize) -> f64 {
    let bytes = data
        .get(offset..offset + 8)
        .expect("read_f64 caller must validate bounds");
    f64::from_le_bytes([
        bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_record_stream() {
        let mut data = Vec::new();
        data.extend_from_slice(b"GSP4");
        data.extend_from_slice(&4_u32.to_le_bytes());
        data.extend_from_slice(&0x1111_u32.to_le_bytes());
        data.extend_from_slice(&[1, 2, 3, 4]);
        data.extend_from_slice(&0_u32.to_le_bytes());
        data.extend_from_slice(&0x2222_u32.to_le_bytes());

        let file = GspFile::parse(&data).expect("valid file");
        assert_eq!(file.records.len(), 2);
        assert_eq!(file.records[0].record_type, 0x1111);
        assert_eq!(file.records[0].payload(&file.data), &[1, 2, 3, 4]);
        assert_eq!(file.records[1].record_type, 0x2222);
        assert!(file.records[1].payload(&file.data).is_empty());
    }

    #[test]
    fn decodes_symbol_slot_record() {
        let payload = [0x00, 0x00, 0x01, 0x00, 0x00, 0x00];
        let slot = decode_symbol_slot(0x0960, &payload).expect("slot");
        assert_eq!(slot.slot_index, 1);
        assert_eq!(slot.value, 0);
        assert_eq!(slot.flag, 1);
        assert_eq!(slot.reserved, 0);
    }

    #[test]
    fn decodes_point_record() {
        let mut payload = Vec::new();
        payload.extend_from_slice(&239.0_f64.to_le_bytes());
        payload.extend_from_slice(&205.0_f64.to_le_bytes());
        let point = decode_point_record(&payload).expect("point");
        assert_eq!(point.x, 239.0);
        assert_eq!(point.y, 205.0);
    }
}
