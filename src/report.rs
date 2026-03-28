use crate::config::Config;
use crate::format::{
    collect_strings, decode_c_string, decode_header_record, decode_indexed_path,
    decode_object_group_header, decode_palette_entry, decode_point_record, decode_symbol_slot,
    record_name, GspFile, Record,
};
use crate::util::{hex_bytes, truncate_text};
use std::collections::BTreeSet;
use std::fmt::Write as _;

pub fn render_report(
    config: &Config,
    file: &GspFile,
    exe_terms: Option<&BTreeSet<String>>,
) -> String {
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
    let _ = writeln!(output, "  indexed_paths: {}", file.indexed_paths().len());
    if let Some(render_path) = &config.render_path {
        let _ = writeln!(output, "  rendered_png: {}", render_path.display());
    }

    if let Some(header_record) = file
        .records
        .first()
        .filter(|record| record.record_type == 0x0384)
    {
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
        0x07d0 => {
            if let Some(header) = decode_object_group_header(payload) {
                details.push(format!(
                    "object class_id={} flags=0x{:08x} style_a=0x{:08x} style_b=0x{:08x} style_c=0x{:08x}",
                    header.class_id, header.flags, header.style_a, header.style_b, header.style_c
                ));
            }
        }
        0x07d2 | 0x07d3 => {
            if let Some(path) = decode_indexed_path(record.record_type, payload) {
                details.push(format!("point_refs={:?}", path.refs));
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
