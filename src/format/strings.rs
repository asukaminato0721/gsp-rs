use super::{ExtractedString, read_u16, read_u32};

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
