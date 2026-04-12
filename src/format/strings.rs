use super::ExtractedString;

pub fn decode_c_string(payload: &[u8]) -> Option<String> {
    let nul = payload.iter().position(|byte| *byte == 0)?;
    let bytes = &payload[..nul];
    if bytes.len() < 4 {
        return None;
    }
    std::str::from_utf8(bytes).ok().map(ToString::to_string)
}

pub fn collect_strings(data: &[u8]) -> Vec<ExtractedString> {
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
            let _ = (data, offset);
            strings.push(ExtractedString { text });
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
