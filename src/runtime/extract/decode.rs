use super::points::{TransformBindingKind, decode_transform_binding};
use crate::format::{
    GspFile, IndexedPathRecord, ObjectGroup, PointRecord, collect_strings, decode_indexed_path,
    read_f64, read_i16, read_u16, read_u32,
};

pub(crate) fn is_action_button_group(group: &ObjectGroup) -> bool {
    (group.header.class_id & 0xffff) == 62
        && group
            .records
            .iter()
            .any(|record| record.record_type == 0x0906)
}

pub(crate) fn decode_link_button_url(file: &GspFile, group: &ObjectGroup) -> Option<String> {
    let payload = group
        .records
        .iter()
        .find(|record| record.record_type == 0x0906)
        .map(|record| record.payload(&file.data))?;
    if payload.len() < 16 || read_u32(payload, 12) != 6 {
        return None;
    }
    collect_strings(payload)
        .into_iter()
        .map(|entry| entry.text.trim().to_string())
        .find(|text| text.starts_with("http://") || text.starts_with("https://"))
}

pub(crate) fn decode_label_name_raw(file: &GspFile, group: &ObjectGroup) -> Option<String> {
    let payload = group
        .records
        .iter()
        .find(|record| record.record_type == 0x07d5)
        .map(|record| record.payload(&file.data))?;
    if payload.len() < 24 {
        return None;
    }
    let name_len = read_u16(payload, 22) as usize;
    if name_len == 0 || 24 + name_len > payload.len() {
        return None;
    }
    let name_bytes = &payload[24..24 + name_len];
    Some(String::from_utf8_lossy(name_bytes).to_string())
}

pub(crate) fn decode_0907_anchor(file: &GspFile, group: &ObjectGroup) -> Option<PointRecord> {
    let payload = group
        .records
        .iter()
        .find(|record| record.record_type == 0x0907)
        .map(|record| record.payload(&file.data))?;
    (payload.len() >= 16 && read_u32(payload, 0) == 0x08fc).then(|| PointRecord {
        x: read_i16(payload, 12) as f64,
        y: read_i16(payload, 14) as f64,
    })
}

#[allow(dead_code)]
pub(crate) fn decode_caption_text(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
) -> Option<String> {
    let path = find_indexed_path(file, group)?;
    let mut parts = Vec::new();
    for &obj_ref in &path.refs {
        let ref_group = groups.get(obj_ref.checked_sub(1)?)?;
        if let Some(name) = decode_label_name(file, ref_group) {
            parts.push(name);
        }
    }
    (!parts.is_empty()).then(|| parts.join(", "))
}

pub(crate) fn decode_label_name(file: &GspFile, group: &ObjectGroup) -> Option<String> {
    let payload = group
        .records
        .iter()
        .find(|record| record.record_type == 0x07d5)
        .map(|record| record.payload(&file.data))?;
    if payload.len() < 24 {
        return None;
    }
    let name_len = read_u16(payload, 22) as usize;
    if name_len == 0 || 24 + name_len > payload.len() {
        return None;
    }
    let name_bytes = &payload[24..24 + name_len];
    Some(
        String::from_utf8_lossy(name_bytes)
            .replace("[1]", "₁")
            .replace("[2]", "₂")
            .replace("[3]", "₃")
            .replace("[4]", "₄"),
    )
}

pub(crate) fn find_indexed_path(file: &GspFile, group: &ObjectGroup) -> Option<IndexedPathRecord> {
    group
        .records
        .iter()
        .find_map(|record| match record.record_type {
            0x07d2 | 0x07d3 => decode_indexed_path(record.record_type, record.payload(&file.data)),
            _ => None,
        })
}

pub(crate) fn decode_group_label_text(file: &GspFile, group: &ObjectGroup) -> Option<String> {
    group
        .records
        .iter()
        .find_map(|record| match record.record_type {
            0x08fc => extract_rich_text(record.payload(&file.data)),
            0x07d5 if matches!(group.header.class_id & 0xffff, 62) => {
                collect_strings(record.payload(&file.data))
                    .into_iter()
                    .map(|entry| entry.text.trim().to_string())
                    .find(|text| !text.is_empty())
            }
            _ => None,
        })
}

pub(crate) fn decode_label_anchor(
    file: &GspFile,
    group: &ObjectGroup,
    anchors: &[Option<PointRecord>],
) -> Option<PointRecord> {
    let kind = group.header.class_id & 0xffff;
    let offset = decode_label_offset(file, group).unwrap_or((0.0, 0.0));
    let base = group
        .records
        .iter()
        .find(|record| record.record_type == 0x08fc)
        .and_then(|record| decode_text_anchor(record.payload(&file.data)))
        .or_else(|| decode_0907_anchor(file, group))
        .or_else(|| match kind {
            0 => anchors
                .get(group.ordinal.saturating_sub(1))
                .cloned()
                .flatten(),
            2 => find_indexed_path(file, group).and_then(|path| {
                let points = path
                    .refs
                    .iter()
                    .filter_map(|object_ref| {
                        anchors.get(object_ref.saturating_sub(1)).cloned().flatten()
                    })
                    .collect::<Vec<_>>();
                if points.len() >= 2 {
                    let start = points.first()?;
                    let end = points.last()?;
                    Some(PointRecord {
                        x: (start.x + end.x) / 2.0,
                        y: (start.y + end.y) / 2.0,
                    })
                } else {
                    None
                }
            }),
            _ => None,
        })
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
        })?;
    Some(PointRecord {
        x: base.x + offset.0,
        y: base.y + offset.1,
    })
}

pub(crate) fn decode_label_offset(file: &GspFile, group: &ObjectGroup) -> Option<(f64, f64)> {
    let payload = group
        .records
        .iter()
        .find(|record| record.record_type == 0x07d5)
        .map(|record| record.payload(&file.data))?;
    (payload.len() >= 10).then(|| (read_i16(payload, 6) as f64, read_i16(payload, 8) as f64))
}

pub(crate) fn decode_bbox_anchor_raw(file: &GspFile, group: &ObjectGroup) -> Option<PointRecord> {
    let (x, y, width, height) = decode_bbox_rect_raw(file, group)?;
    Some(PointRecord {
        x: x + width / 2.0,
        y: y + height / 2.0,
    })
}

pub(crate) fn decode_bbox_rect_raw(
    file: &GspFile,
    group: &ObjectGroup,
) -> Option<(f64, f64, f64, f64)> {
    let payload = group
        .records
        .iter()
        .find(|record| matches!(record.record_type, 0x0898 | 0x08a2 | 0x08a3 | 0x0903))
        .map(|record| record.payload(&file.data))?;
    if payload.len() < 8 {
        return None;
    }
    let x0 = read_i16(payload, payload.len() - 8) as f64;
    let y0 = read_i16(payload, payload.len() - 6) as f64;
    let x1 = read_i16(payload, payload.len() - 4) as f64;
    let y1 = read_i16(payload, payload.len() - 2) as f64;
    let left = x0.min(x1);
    let top = y0.min(y1);
    Some((left, top, (x1 - x0).abs(), (y1 - y0).abs()))
}

pub(crate) fn decode_button_screen_anchor(
    file: &GspFile,
    group: &ObjectGroup,
) -> Option<PointRecord> {
    let payload = group
        .records
        .iter()
        .find(|record| record.record_type == 0x0906)
        .map(|record| record.payload(&file.data))?;
    (payload.len() >= 24).then(|| PointRecord {
        x: read_u16(payload, payload.len() - 4) as f64,
        y: read_u16(payload, payload.len() - 2) as f64,
    })
}

pub(crate) fn decode_transform_anchor_raw(
    file: &GspFile,
    group: &ObjectGroup,
    anchors: &[Option<PointRecord>],
) -> Option<PointRecord> {
    let kind = group.header.class_id & 0xffff;
    match kind {
        27 => {
            let binding = decode_transform_binding(file, group)?;
            let source = anchors.get(binding.source_group_index)?.clone()?;
            let center = anchors.get(binding.center_group_index)?.clone()?;
            let TransformBindingKind::Rotate { angle_degrees } = binding.kind else {
                return None;
            };
            let radians = angle_degrees.to_radians();
            let cos = radians.cos();
            let sin = radians.sin();
            let dx = source.x - center.x;
            let dy = source.y - center.y;
            Some(PointRecord {
                x: center.x + dx * cos + dy * sin,
                y: center.y - dx * sin + dy * cos,
            })
        }
        30 => {
            let binding = decode_transform_binding(file, group)?;
            let source = anchors.get(binding.source_group_index)?.clone()?;
            let center = anchors.get(binding.center_group_index)?.clone()?;
            let TransformBindingKind::Scale { factor: t } = binding.kind else {
                return None;
            };

            Some(PointRecord {
                x: center.x + (source.x - center.x) * t,
                y: center.y + (source.y - center.y) * t,
            })
        }
        _ => None,
    }
}

pub(crate) fn decode_measurement_value(payload: &[u8]) -> Option<f64> {
    (payload.len() == 12).then(|| read_f64(payload, 4))
}

pub(crate) fn decode_text_anchor(payload: &[u8]) -> Option<PointRecord> {
    if payload.len() < 16 {
        return None;
    }
    Some(PointRecord {
        x: read_i16(payload, 12) as f64,
        y: read_i16(payload, 14) as f64,
    })
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
        && let Some(second_rel) = cleaned[first + 3..].find("AB:")
    {
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
                        let chars = inner.chars().collect::<Vec<_>>();
                        let split = chars
                            .iter()
                            .rposition(|ch| !ch.is_ascii_digit())
                            .map(|index| index + 1)
                            .unwrap_or(0);
                        if split < chars.len() {
                            let exponent = chars[split..].iter().collect::<String>();
                            inner = chars[..split].iter().collect::<String>();
                            inner.push('^');
                            inner.push_str(&exponent);
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
