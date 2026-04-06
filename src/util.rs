use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

pub const KNOWN_REFERENCE_TERMS: &[&str] = &[
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

pub fn analyze_reference_exe(path: &Path) -> Result<BTreeSet<String>, String> {
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

pub fn collect_ascii_strings(data: &[u8]) -> Vec<String> {
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

pub fn truncate_text(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }

    let mut result = text.chars().take(max_chars).collect::<String>();
    result.push_str("...");
    result
}

pub fn hex_bytes(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn base64_encode(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0];
        let b1 = *chunk.get(1).unwrap_or(&0);
        let b2 = *chunk.get(2).unwrap_or(&0);
        let triple = ((b0 as u32) << 16) | ((b1 as u32) << 8) | (b2 as u32);

        out.push(TABLE[((triple >> 18) & 0x3f) as usize] as char);
        out.push(TABLE[((triple >> 12) & 0x3f) as usize] as char);
        if chunk.len() > 1 {
            out.push(TABLE[((triple >> 6) & 0x3f) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(TABLE[(triple & 0x3f) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}
