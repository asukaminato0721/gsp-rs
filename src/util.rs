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
