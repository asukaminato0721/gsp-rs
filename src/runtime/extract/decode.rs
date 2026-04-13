use super::points::{TransformBindingKind, try_decode_transform_binding};
use crate::format::{
    GroupKind, GspFile, IndexedPathRecord, ObjectGroup, PointRecord, collect_strings,
    decode_indexed_path, read_f64, read_i16, read_u16, read_u32,
};
use crate::runtime::payload_consts::{EXPR_OP_ADD, EXPR_OP_SUB};
use crate::runtime::payload_consts::{
    RECORD_ACTION_BUTTON_PAYLOAD, RECORD_FUNCTION_EXPR_PAYLOAD, RECORD_INDEXED_PATH_A,
    RECORD_INDEXED_PATH_B, RECORD_LABEL_AUX, RECORD_LABEL_VISIBILITY, RECORD_POINT_F64_PAIR,
    RECORD_RICH_TEXT, RECORD_RICH_TEXT_MAGIC,
};
use thiserror::Error;

pub(crate) fn is_circle_group_kind(kind: GroupKind) -> bool {
    matches!(kind, GroupKind::Circle | GroupKind::CircleCenterRadius)
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub(crate) enum IndexedPathDecodeError {
    #[error("malformed indexed path record 0x{record_type:04x} at 0x{offset:x} (len={length})")]
    MalformedPathRecord {
        record_type: u32,
        offset: usize,
        length: u32,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub(crate) enum LinkButtonDecodeError {
    #[error("action button payload at 0x{offset:x} is too short ({byte_len} bytes)")]
    PayloadTooShort { offset: usize, byte_len: usize },
    #[error("unsupported action button kind {action_kind} at payload offset 0x{offset:x}")]
    UnsupportedActionKind { offset: usize, action_kind: u32 },
    #[error("no URL found in action button payload at 0x{offset:x}")]
    MissingUrl { offset: usize },
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub(crate) enum Anchor0907DecodeError {
    #[error("0x0907 anchor payload at 0x{offset:x} is too short ({byte_len} bytes)")]
    PayloadTooShort { offset: usize, byte_len: usize },
    #[error("0x0907 anchor payload at 0x{offset:x} has unexpected magic 0x{magic:08x}")]
    UnexpectedMagic { offset: usize, magic: u32 },
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub(crate) enum BboxDecodeError {
    #[error("bbox record 0x{record_type:04x} at 0x{offset:x} is too short ({byte_len} bytes)")]
    PayloadTooShort {
        record_type: u32,
        offset: usize,
        byte_len: usize,
    },
}

pub(crate) fn resolve_circle_points_raw(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
    group: &ObjectGroup,
) -> Option<(PointRecord, PointRecord)> {
    let path = find_indexed_path(file, group)?;
    match group.header.kind() {
        GroupKind::Circle => {
            if path.refs.len() != 2 {
                return None;
            }
            let center = anchors.get(path.refs[0].checked_sub(1)?)?.clone()?;
            let radius_point = anchors.get(path.refs[1].checked_sub(1)?)?.clone()?;
            Some((center, radius_point))
        }
        GroupKind::CircleCenterRadius => {
            if path.refs.len() != 2 {
                return None;
            }
            let center = anchors.get(path.refs[0].checked_sub(1)?)?.clone()?;
            let segment_group = groups.get(path.refs[1].checked_sub(1)?)?;
            let segment_path = find_indexed_path(file, segment_group)?;
            if segment_path.refs.len() != 2 {
                return None;
            }
            let start = anchors.get(segment_path.refs[0].checked_sub(1)?)?.clone()?;
            let end = anchors.get(segment_path.refs[1].checked_sub(1)?)?.clone()?;
            let radius = ((end.x - start.x).powi(2) + (end.y - start.y).powi(2)).sqrt();
            radius.is_finite().then(|| {
                (
                    center.clone(),
                    PointRecord {
                        x: center.x + radius,
                        y: center.y,
                    },
                )
            })
        }
        _ => None,
    }
}

#[derive(Debug, Clone, PartialEq, Error)]
pub(crate) enum ParameterControlDecodeError {
    #[error("missing 0x0907 parameter payload record")]
    MissingPayloadRecord,
    #[error("discrete parameter payload has invalid fraction at word range [92..96]")]
    InvalidDiscreteFraction,
    #[error("continuous parameter payload contains non-finite value")]
    NonFiniteContinuousValue,
}

pub(crate) fn is_action_button_group(group: &ObjectGroup) -> bool {
    (group.header.kind()) == crate::format::GroupKind::ActionButton
        && group
            .records
            .iter()
            .any(|record| record.record_type == RECORD_ACTION_BUTTON_PAYLOAD)
}

pub(crate) fn is_parameter_control_group(group: &ObjectGroup) -> bool {
    (group.header.kind()) == crate::format::GroupKind::Point
        && group
            .records
            .iter()
            .any(|record| record.record_type == RECORD_FUNCTION_EXPR_PAYLOAD)
        && group
            .records
            .iter()
            .any(|record| record.record_type == RECORD_LABEL_AUX)
        && group
            .records
            .iter()
            .any(|record| record.record_type == 0x07d8)
        && group
            .records
            .iter()
            .any(|record| record.record_type == 0x08a3)
        && !group
            .records
            .iter()
            .any(|record| record.record_type == RECORD_POINT_F64_PAIR)
}

fn try_decode_continuous_parameter_value(
    payload: &[u8],
) -> Result<Option<f64>, ParameterControlDecodeError> {
    if payload.len() < 60 {
        return Ok(None);
    }
    let value = read_f64(payload, 52);
    if !value.is_finite() {
        return Err(ParameterControlDecodeError::NonFiniteContinuousValue);
    }
    Ok(Some(value))
}

pub(crate) fn decode_discrete_parameter_value(payload: &[u8]) -> Option<f64> {
    try_decode_discrete_parameter_value(payload).ok().flatten()
}

fn try_decode_discrete_parameter_value(
    payload: &[u8],
) -> Result<Option<f64>, ParameterControlDecodeError> {
    if payload.len() >= 98 {
        let whole = f64::from(read_u16(payload, 92));
        let denominator = f64::from(read_u16(payload, 94));
        let fractional = f64::from(read_u16(payload, 96));
        if denominator.is_finite()
            && denominator > 0.0
            && denominator <= 10_000.0
            && fractional >= 0.0
            && fractional < denominator
        {
            return Ok(Some(whole + fractional / denominator));
        }
        return Err(ParameterControlDecodeError::InvalidDiscreteFraction);
    }

    if payload.len() >= 94 {
        return Ok(Some(f64::from(read_u16(payload, 92))));
    }

    try_decode_continuous_parameter_value(payload)
}

fn parameter_group_drives_coordinate_value(file: &GspFile, target_ordinal: usize) -> bool {
    file.object_groups().into_iter().any(|group| {
        group.header.kind() == GroupKind::CoordinatePoint
            && find_indexed_path(file, &group).and_then(|path| path.refs.first().copied())
                == Some(target_ordinal)
    })
}

fn decode_signed_parameter_tail_value(payload: &[u8]) -> Option<f64> {
    let words = payload
        .chunks_exact(2)
        .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
        .collect::<Vec<_>>();
    let words = match words.last().copied() {
        Some(0x0101 | 0x0201) => &words[..words.len().saturating_sub(1)],
        _ => words.as_slice(),
    };
    match words {
        [
            ..,
            sign @ (EXPR_OP_ADD | EXPR_OP_SUB),
            whole,
            denominator,
            fractional,
        ] if *denominator > 0 && fractional < denominator => {
            let value = f64::from(*whole) + f64::from(*fractional) / f64::from(*denominator);
            Some(if *sign == EXPR_OP_SUB { -value } else { value })
        }
        [.., sign @ (EXPR_OP_ADD | EXPR_OP_SUB), whole] => {
            let value = f64::from(*whole);
            Some(if *sign == EXPR_OP_SUB { -value } else { value })
        }
        _ => None,
    }
}

pub(crate) fn try_decode_parameter_control_value_for_group(
    file: &GspFile,
    _groups: &[ObjectGroup],
    group: &ObjectGroup,
) -> Result<f64, ParameterControlDecodeError> {
    let payload = group
        .records
        .iter()
        .find(|record| record.record_type == RECORD_FUNCTION_EXPR_PAYLOAD)
        .map(|record| record.payload(&file.data))
        .ok_or(ParameterControlDecodeError::MissingPayloadRecord)?;

    if let Some(value) = decode_signed_parameter_tail_value(payload) {
        return Ok(value);
    }

    let continuous = try_decode_continuous_parameter_value(payload)?;
    let discrete = try_decode_discrete_parameter_value(payload);

    if parameter_group_drives_coordinate_value(file, group.ordinal) {
        return continuous
            .or_else(|| discrete.ok().flatten())
            .ok_or(ParameterControlDecodeError::InvalidDiscreteFraction);
    }

    match discrete {
        Ok(Some(value)) if value < 4096.0 => Ok(value),
        Ok(Some(_)) | Err(ParameterControlDecodeError::InvalidDiscreteFraction) => {
            continuous.ok_or(ParameterControlDecodeError::InvalidDiscreteFraction)
        }
        Ok(None) => continuous.ok_or(ParameterControlDecodeError::InvalidDiscreteFraction),
        Err(error) => Err(error),
    }
}

pub(crate) fn try_decode_link_button_url(
    file: &GspFile,
    group: &ObjectGroup,
) -> Result<Option<String>, LinkButtonDecodeError> {
    let record = group
        .records
        .iter()
        .find(|record| record.record_type == RECORD_ACTION_BUTTON_PAYLOAD);
    let Some(record) = record else {
        return Ok(None);
    };
    let payload = record.payload(&file.data);
    if payload.len() < 16 {
        return Err(LinkButtonDecodeError::PayloadTooShort {
            offset: record.offset,
            byte_len: payload.len(),
        });
    }
    let action_kind = read_u32(payload, 12);
    if action_kind != 6 {
        return Err(LinkButtonDecodeError::UnsupportedActionKind {
            offset: record.offset,
            action_kind,
        });
    }
    let url = collect_strings(payload)
        .into_iter()
        .map(|entry| entry.text.trim().to_string())
        .find(|text| text.starts_with("http://") || text.starts_with("https://"));
    match url {
        Some(url) => Ok(Some(url)),
        None => Err(LinkButtonDecodeError::MissingUrl {
            offset: record.offset,
        }),
    }
}

pub(crate) fn decode_label_name_raw(file: &GspFile, group: &ObjectGroup) -> Option<String> {
    let payload = group
        .records
        .iter()
        .find(|record| record.record_type == RECORD_LABEL_AUX)
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

pub(crate) fn try_decode_0907_anchor(
    file: &GspFile,
    group: &ObjectGroup,
) -> Result<Option<PointRecord>, Anchor0907DecodeError> {
    let record = group
        .records
        .iter()
        .find(|record| record.record_type == RECORD_FUNCTION_EXPR_PAYLOAD);
    let Some(record) = record else {
        return Ok(None);
    };
    let payload = record.payload(&file.data);
    if payload.len() < 16 {
        return Err(Anchor0907DecodeError::PayloadTooShort {
            offset: record.offset,
            byte_len: payload.len(),
        });
    }
    let magic = read_u32(payload, 0);
    if magic != RECORD_RICH_TEXT_MAGIC {
        return Err(Anchor0907DecodeError::UnexpectedMagic {
            offset: record.offset,
            magic,
        });
    }
    Ok(Some(PointRecord {
        x: read_i16(payload, 12) as f64,
        y: read_i16(payload, 14) as f64,
    }))
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
        .find(|record| record.record_type == RECORD_LABEL_AUX)
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

pub(crate) fn decode_label_visible(file: &GspFile, group: &ObjectGroup) -> Option<bool> {
    let payload = group
        .records
        .iter()
        .find(|record| record.record_type == RECORD_LABEL_VISIBILITY)
        .map(|record| record.payload(&file.data))?;
    (payload.len() >= 4).then(|| read_u16(payload, 2) != 0)
}

pub(crate) fn find_indexed_path(file: &GspFile, group: &ObjectGroup) -> Option<IndexedPathRecord> {
    try_find_indexed_path(file, group).ok().flatten()
}

pub(crate) fn try_find_indexed_path(
    file: &GspFile,
    group: &ObjectGroup,
) -> Result<Option<IndexedPathRecord>, IndexedPathDecodeError> {
    let record = group.records.iter().find(|record| {
        matches!(
            record.record_type,
            RECORD_INDEXED_PATH_A | RECORD_INDEXED_PATH_B
        )
    });
    let Some(record) = record else {
        return Ok(None);
    };
    decode_indexed_path(record.record_type, record.payload(&file.data))
        .map(Some)
        .ok_or(IndexedPathDecodeError::MalformedPathRecord {
            record_type: record.record_type,
            offset: record.offset,
            length: record.length,
        })
}

pub(crate) fn try_decode_group_label_text(
    file: &GspFile,
    group: &ObjectGroup,
) -> Result<Option<String>, RichTextDecodeError> {
    group
        .records
        .iter()
        .find_map(|record| match record.record_type {
            RECORD_RICH_TEXT => Some(
                try_decode_rich_text(record.payload(&file.data))
                    .map(|content| content.map(|content| content.text)),
            ),
            RECORD_LABEL_AUX
                if matches!(group.header.kind(), crate::format::GroupKind::ActionButton) =>
            {
                Some(Ok(collect_strings(record.payload(&file.data))
                    .into_iter()
                    .map(|entry| entry.text.trim().to_string())
                    .find(|text| !text.is_empty())))
            }
            _ => None,
        })
        .transpose()
        .map(|value| value.flatten())
}

#[derive(Debug, Clone)]
pub(crate) struct RichTextContent {
    pub(crate) text: String,
    pub(crate) hotspots: Vec<RichTextHotspotRef>,
    pub(crate) markup: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct RichTextHotspotRef {
    pub(crate) line: usize,
    pub(crate) start: usize,
    pub(crate) end: usize,
    pub(crate) text: String,
    pub(crate) path_slot: usize,
}

pub(crate) fn try_decode_group_rich_text(
    file: &GspFile,
    group: &ObjectGroup,
) -> Result<Option<RichTextContent>, RichTextDecodeError> {
    let record = group
        .records
        .iter()
        .find(|record| record.record_type == RECORD_RICH_TEXT);
    record
        .map(|record| try_decode_rich_text(record.payload(&file.data)))
        .transpose()
        .map(|value| value.flatten())
}

pub(crate) fn decode_label_anchor(
    file: &GspFile,
    group: &ObjectGroup,
    anchors: &[Option<PointRecord>],
) -> Option<PointRecord> {
    let kind = group.header.kind();
    let offset = decode_label_offset(file, group).unwrap_or((0.0, 0.0));
    let text_anchor = if let Some(record) = group
        .records
        .iter()
        .find(|record| record.record_type == RECORD_RICH_TEXT)
    {
        decode_text_anchor(record.payload(&file.data))
    } else {
        None
    };
    let indexed_anchor = || {
        let path = find_indexed_path(file, group)?;
        path.refs
            .iter()
            .rev()
            .find_map(|object_ref| anchors.get(object_ref.saturating_sub(1)).cloned().flatten())
    };
    let base = text_anchor
        .or_else(|| try_decode_0907_anchor(file, group).ok().flatten())
        .or_else(|| match kind {
            crate::format::GroupKind::Point => anchors
                .get(group.ordinal.saturating_sub(1))
                .cloned()
                .flatten(),
            crate::format::GroupKind::Segment
            | crate::format::GroupKind::Line
            | crate::format::GroupKind::Ray => decode_line_like_label_anchor(file, group, anchors),
            crate::format::GroupKind::AngleMarker => {
                decode_angle_marker_label_anchor(file, group, anchors)
            }
            _ => None,
        })
        .or_else(|| {
            anchors
                .get(group.ordinal.saturating_sub(1))
                .cloned()
                .flatten()
        })
        .or_else(indexed_anchor)?;
    Some(PointRecord {
        x: base.x + offset.0,
        y: base.y + offset.1,
    })
}

fn decode_line_like_label_anchor(
    file: &GspFile,
    group: &ObjectGroup,
    anchors: &[Option<PointRecord>],
) -> Option<PointRecord> {
    let path = find_indexed_path(file, group)?;
    let points = path
        .refs
        .iter()
        .filter_map(|object_ref| anchors.get(object_ref.saturating_sub(1)).cloned().flatten())
        .collect::<Vec<_>>();
    if points.len() < 2 {
        return None;
    }
    let start = points.first()?;
    let end = points.last()?;
    Some(PointRecord {
        x: (start.x + end.x) / 2.0,
        y: (start.y + end.y) / 2.0,
    })
}

fn decode_angle_marker_label_anchor(
    file: &GspFile,
    group: &ObjectGroup,
    anchors: &[Option<PointRecord>],
) -> Option<PointRecord> {
    let path = find_indexed_path(file, group)?;
    if path.refs.len() != 3 {
        return None;
    }
    let start = anchors.get(path.refs[0].checked_sub(1)?)?.clone()?;
    let vertex = anchors.get(path.refs[1].checked_sub(1)?)?.clone()?;
    let end = anchors.get(path.refs[2].checked_sub(1)?)?.clone()?;

    let first = normalize_direction(&vertex, &start)?;
    let second = normalize_direction(&vertex, &end)?;
    let first_len = ((start.x - vertex.x).powi(2) + (start.y - vertex.y).powi(2)).sqrt();
    let second_len = ((end.x - vertex.x).powi(2) + (end.y - vertex.y).powi(2)).sqrt();
    let shortest_len = first_len.min(second_len);
    if shortest_len <= 1e-9 {
        return None;
    }

    let marker_class = decode_angle_marker_class(file, group).max(1);
    let points =
        resolve_angle_marker_label_points(&vertex, first, second, shortest_len, marker_class)?;
    points.get(points.len() / 2).cloned()
}

fn decode_angle_marker_class(file: &GspFile, group: &ObjectGroup) -> u32 {
    group
        .records
        .iter()
        .find(|record| record.record_type == 0x090e)
        .map(|record| record.payload(&file.data))
        .filter(|payload| payload.len() >= 4)
        .map(|payload| read_u32(payload, 0))
        .unwrap_or(1)
}

fn normalize_direction(origin: &PointRecord, point: &PointRecord) -> Option<(f64, f64)> {
    let dx = point.x - origin.x;
    let dy = point.y - origin.y;
    let len = (dx * dx + dy * dy).sqrt();
    (len > 1e-9).then_some((dx / len, dy / len))
}

fn resolve_angle_marker_label_points(
    vertex: &PointRecord,
    first: (f64, f64),
    second: (f64, f64),
    shortest_len: f64,
    marker_class: u32,
) -> Option<Vec<PointRecord>> {
    let dot = (first.0 * second.0 + first.1 * second.1).clamp(-1.0, 1.0);
    let cross = first.0 * second.1 - first.1 * second.0;
    if dot.abs() <= 0.12 {
        return resolve_right_angle_marker_label_points(vertex, first, second, shortest_len);
    }
    resolve_arc_angle_marker_label_points(vertex, first, shortest_len, cross, dot, marker_class)
}

fn resolve_right_angle_marker_label_points(
    vertex: &PointRecord,
    first: (f64, f64),
    second: (f64, f64),
    shortest_len: f64,
) -> Option<Vec<PointRecord>> {
    let side = (shortest_len * 0.125)
        .clamp(10.0, 28.0)
        .min(shortest_len * 0.5);
    if side <= 1e-9 {
        return None;
    }

    let start_on_first = PointRecord {
        x: vertex.x + first.0 * side,
        y: vertex.y + first.1 * side,
    };
    let corner = PointRecord {
        x: vertex.x + (first.0 + second.0) * side,
        y: vertex.y + (first.1 + second.1) * side,
    };
    let end_on_second = PointRecord {
        x: vertex.x + second.0 * side,
        y: vertex.y + second.1 * side,
    };

    Some(vec![start_on_first, corner, end_on_second])
}

fn resolve_arc_angle_marker_label_points(
    vertex: &PointRecord,
    first: (f64, f64),
    shortest_len: f64,
    cross: f64,
    dot: f64,
    marker_class: u32,
) -> Option<Vec<PointRecord>> {
    let class_scale = 1.0 + 0.18 * (marker_class.saturating_sub(1) as f64);
    let radius = ((shortest_len * 0.12).clamp(10.0, 28.0) * class_scale).min(shortest_len * 0.42);
    if radius <= 1e-9 {
        return None;
    }
    let delta = cross.atan2(dot);
    if delta.abs() <= 1e-6 {
        return None;
    }
    let start_angle = first.1.atan2(first.0);
    let samples = 9usize;
    Some(
        (0..samples)
            .map(|index| {
                let t = index as f64 / (samples - 1) as f64;
                let angle = start_angle + delta * t;
                PointRecord {
                    x: vertex.x + radius * angle.cos(),
                    y: vertex.y + radius * angle.sin(),
                }
            })
            .collect(),
    )
}

pub(crate) fn decode_label_offset(file: &GspFile, group: &ObjectGroup) -> Option<(f64, f64)> {
    let payload = group
        .records
        .iter()
        .find(|record| record.record_type == RECORD_LABEL_AUX)
        .map(|record| record.payload(&file.data))?;
    (payload.len() >= 10).then(|| (read_i16(payload, 6) as f64, read_i16(payload, 8) as f64))
}

pub(crate) fn decode_bbox_anchor_raw(file: &GspFile, group: &ObjectGroup) -> Option<PointRecord> {
    let (x, y, width, height) = try_decode_bbox_rect_raw(file, group).ok().flatten()?;
    Some(PointRecord {
        x: x + width / 2.0,
        y: y + height / 2.0,
    })
}

pub(crate) fn try_decode_bbox_rect_raw(
    file: &GspFile,
    group: &ObjectGroup,
) -> Result<Option<(f64, f64, f64, f64)>, BboxDecodeError> {
    let record = group
        .records
        .iter()
        .find(|record| matches!(record.record_type, 0x0898 | 0x08a2 | 0x08a3 | 0x0903));
    let Some(record) = record else {
        return Ok(None);
    };
    let payload = record.payload(&file.data);
    if payload.len() < 8 {
        return Err(BboxDecodeError::PayloadTooShort {
            record_type: record.record_type,
            offset: record.offset,
            byte_len: payload.len(),
        });
    }
    let x0 = read_i16(payload, payload.len() - 8) as f64;
    let y0 = read_i16(payload, payload.len() - 6) as f64;
    let x1 = read_i16(payload, payload.len() - 4) as f64;
    let y1 = read_i16(payload, payload.len() - 2) as f64;
    let left = x0.min(x1);
    let top = y0.min(y1);
    Ok(Some((left, top, (x1 - x0).abs(), (y1 - y0).abs())))
}

pub(crate) fn decode_button_screen_anchor(
    file: &GspFile,
    group: &ObjectGroup,
) -> Option<PointRecord> {
    let payload = group
        .records
        .iter()
        .find(|record| record.record_type == RECORD_ACTION_BUTTON_PAYLOAD)
        .map(|record| record.payload(&file.data))?;
    (payload.len() >= 24).then(|| PointRecord {
        x: read_i16(payload, payload.len() - 4) as f64,
        y: read_i16(payload, payload.len() - 2) as f64,
    })
}

pub(crate) fn decode_transform_anchor_raw(
    file: &GspFile,
    group: &ObjectGroup,
    anchors: &[Option<PointRecord>],
) -> Option<PointRecord> {
    let kind = group.header.kind();
    match kind {
        crate::format::GroupKind::Rotation => {
            let binding = try_decode_transform_binding(file, group).ok()?;
            let source = anchors.get(binding.source_group_index)?.clone()?;
            let center = anchors.get(binding.center_group_index)?.clone()?;
            let TransformBindingKind::Rotate { angle_degrees, .. } = binding.kind else {
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
        crate::format::GroupKind::Scale => {
            let binding = try_decode_transform_binding(file, group).ok()?;
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

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub(crate) enum RichTextDecodeError {
    #[error("{0}")]
    MarkupParse(MarkupParseError),
}

impl From<MarkupParseError> for RichTextDecodeError {
    fn from(value: MarkupParseError) -> Self {
        Self::MarkupParse(value)
    }
}

fn try_decode_rich_text(payload: &[u8]) -> Result<Option<RichTextContent>, RichTextDecodeError> {
    let text = String::from_utf8_lossy(payload);
    let Some(start) = text.find('<') else {
        return Ok(None);
    };
    let markup = text[start..].trim_end_matches('\0');
    let nodes = parse_markup_nodes(markup)?;

    if markup.starts_with("<VL") {
        return Ok(extract_visual_rich_text(&nodes).map(|mut content| {
            content.markup = Some(markup.to_string());
            content
        }));
    }

    let parsed = render_markup_plain(&nodes);
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

    Ok((!cleaned.is_empty()).then_some(RichTextContent {
        text: cleaned,
        hotspots: Vec::new(),
        markup: None,
    }))
}

#[derive(Debug, Clone, PartialEq)]
enum RichMarkupNode {
    Text(String),
    Ignore,
    VerticalLines(Vec<RichMarkupNode>),
    PathRef {
        slot: usize,
        children: Vec<RichMarkupNode>,
    },
    Fraction {
        numerator: Vec<RichMarkupNode>,
        denominator: Vec<RichMarkupNode>,
    },
    Root(Vec<RichMarkupNode>),
    Superscript(Vec<RichMarkupNode>),
    Group(Vec<RichMarkupNode>),
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub(crate) enum MarkupParseError {
    #[error("empty markup tag at byte offset {index}")]
    EmptyTagName { index: usize },
    #[error("unterminated markup tag at byte offset {index}")]
    UnterminatedTag { index: usize },
}

#[derive(Debug, Clone)]
struct RichMarkupRun {
    text: String,
    path_slot: Option<usize>,
}

fn extract_visual_rich_text(nodes: &[RichMarkupNode]) -> Option<RichTextContent> {
    let lines = render_markup_nodes(nodes, None);
    let mut text_lines = Vec::new();
    let mut hotspots = Vec::new();

    for (line_index, line_runs) in lines.iter().enumerate() {
        let mut line_text = String::new();
        for run in line_runs {
            let cleaned = run
                .text
                .replace(['\u{2013}', '\u{2014}'], "-")
                .replace("厘米", "cm");
            if cleaned.is_empty() {
                continue;
            }
            let start = line_text.chars().count();
            line_text.push_str(&cleaned);
            let end = line_text.chars().count();
            if let Some(path_slot) = run.path_slot {
                hotspots.push(RichTextHotspotRef {
                    line: line_index,
                    start,
                    end,
                    text: cleaned.clone(),
                    path_slot,
                });
            }
        }
        text_lines.push(line_text);
    }

    let text = text_lines.join("\n");
    text.chars()
        .any(|ch| !ch.is_whitespace())
        .then_some(RichTextContent {
            text,
            hotspots,
            markup: None,
        })
}

struct MarkupParser<'a> {
    source: &'a str,
    bytes: &'a [u8],
    index: usize,
}

impl<'a> MarkupParser<'a> {
    fn new(source: &'a str) -> Self {
        Self {
            source,
            bytes: source.as_bytes(),
            index: 0,
        }
    }

    fn parse(mut self) -> Result<Vec<RichMarkupNode>, MarkupParseError> {
        self.parse_children(false)
    }

    fn parse_children(
        &mut self,
        stop_on_gt: bool,
    ) -> Result<Vec<RichMarkupNode>, MarkupParseError> {
        let mut nodes = Vec::new();
        while let Some(byte) = self.peek() {
            if stop_on_gt && byte == b'>' {
                self.bump();
                return Ok(nodes);
            }
            if byte != b'<' {
                self.bump();
                continue;
            }
            nodes.push(self.parse_node()?);
        }
        if stop_on_gt {
            Err(MarkupParseError::UnterminatedTag {
                index: self.source.len().saturating_sub(1),
            })
        } else {
            Ok(nodes)
        }
    }

    fn parse_node(&mut self) -> Result<RichMarkupNode, MarkupParseError> {
        let tag_index = self.index;
        self.expect_open_tag()?;
        let name_start = self.index;
        while let Some(byte) = self.peek() {
            if byte == b'<' || byte == b'>' {
                break;
            }
            self.bump();
        }
        if self.index == name_start {
            return Err(MarkupParseError::EmptyTagName { index: tag_index });
        }
        if self.peek().is_none() {
            return Err(MarkupParseError::UnterminatedTag { index: tag_index });
        }
        let name = self.source[name_start..self.index].to_string();
        let children = if self.peek() == Some(b'<') {
            self.parse_children(true)?
        } else {
            self.bump();
            Vec::new()
        };
        Ok(classify_markup_node(name, children))
    }

    fn expect_open_tag(&mut self) -> Result<(), MarkupParseError> {
        match self.peek() {
            Some(b'<') => {
                self.bump();
                Ok(())
            }
            _ => Err(MarkupParseError::UnterminatedTag { index: self.index }),
        }
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.index).copied()
    }

    fn bump(&mut self) {
        self.index += 1;
    }
}

fn parse_markup_nodes(markup: &str) -> Result<Vec<RichMarkupNode>, MarkupParseError> {
    MarkupParser::new(markup).parse()
}

fn classify_markup_node(name: String, children: Vec<RichMarkupNode>) -> RichMarkupNode {
    if let Some(text) = decode_markup_text(&name) {
        return RichMarkupNode::Text(text);
    }
    if name.starts_with('!') {
        return RichMarkupNode::Ignore;
    }
    if name == "VL" {
        return RichMarkupNode::VerticalLines(children);
    }
    if let Some(slot) = decode_markup_path_slot(&name) {
        return RichMarkupNode::PathRef { slot, children };
    }
    if name == "/" {
        let mut iter = children.into_iter();
        let numerator = iter.next().into_iter().collect::<Vec<_>>();
        let denominator = iter.collect::<Vec<_>>();
        return RichMarkupNode::Fraction {
            numerator,
            denominator,
        };
    }
    if name == "R" {
        return RichMarkupNode::Root(children);
    }
    if name.starts_with('+') {
        return RichMarkupNode::Superscript(children);
    }
    RichMarkupNode::Group(children)
}

fn render_markup_nodes(
    nodes: &[RichMarkupNode],
    active_slot: Option<usize>,
) -> Vec<Vec<RichMarkupRun>> {
    let mut lines = vec![Vec::new()];
    for node in nodes {
        let node_lines = render_markup_node(node, active_slot);
        append_markup_lines(&mut lines, node_lines);
    }
    lines
        .into_iter()
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
}

fn render_markup_node(
    node: &RichMarkupNode,
    active_slot: Option<usize>,
) -> Vec<Vec<RichMarkupRun>> {
    match node {
        RichMarkupNode::Text(text) => vec![vec![RichMarkupRun {
            text: text.clone(),
            path_slot: active_slot,
        }]],
        RichMarkupNode::Ignore => vec![Vec::new()],
        RichMarkupNode::VerticalLines(children) => children
            .iter()
            .flat_map(|child| render_markup_node(child, active_slot))
            .filter(|line| !line.is_empty())
            .collect::<Vec<_>>(),
        RichMarkupNode::PathRef { slot, children } => render_markup_nodes(children, Some(*slot)),
        RichMarkupNode::Fraction {
            numerator,
            denominator,
        } => {
            let numerator = render_markup_inline(numerator, active_slot);
            let denominator = render_markup_inline(denominator, active_slot);
            if numerator.is_empty() || denominator.is_empty() {
                return vec![numerator.into_iter().chain(denominator).collect()];
            }
            let mut runs = numerator;
            runs.push(RichMarkupRun {
                text: "/".to_string(),
                path_slot: active_slot,
            });
            runs.extend(denominator);
            vec![runs]
        }
        RichMarkupNode::Root(children) => {
            let child_runs = render_markup_inline(children, active_slot);
            if child_runs.is_empty() {
                return vec![Vec::new()];
            }
            let mut runs = vec![RichMarkupRun {
                text: "√".to_string(),
                path_slot: active_slot,
            }];
            runs.extend(child_runs);
            vec![runs]
        }
        RichMarkupNode::Superscript(children) => {
            let lines = render_markup_nodes(children, active_slot);
            let joined = lines
                .into_iter()
                .flatten()
                .map(|run| run.text)
                .collect::<String>();
            let text = collapse_markup_superscript(joined);
            if text.is_empty() {
                return vec![Vec::new()];
            }
            vec![vec![RichMarkupRun {
                text,
                path_slot: active_slot,
            }]]
        }
        RichMarkupNode::Group(children) => render_markup_nodes(children, active_slot),
    }
}

fn render_markup_inline(
    nodes: &[RichMarkupNode],
    active_slot: Option<usize>,
) -> Vec<RichMarkupRun> {
    render_markup_nodes(nodes, active_slot)
        .into_iter()
        .enumerate()
        .flat_map(|(line_index, line)| {
            let mut runs = Vec::new();
            if line_index > 0 {
                runs.push(RichMarkupRun {
                    text: " ".to_string(),
                    path_slot: active_slot,
                });
            }
            runs.extend(line);
            runs
        })
        .collect()
}

fn append_markup_lines(target: &mut Vec<Vec<RichMarkupRun>>, lines: Vec<Vec<RichMarkupRun>>) {
    if lines.is_empty() {
        return;
    }
    if target.is_empty() {
        target.extend(lines);
        return;
    }
    let mut iter = lines.into_iter();
    if let Some(first_line) = iter.next() {
        target
            .last_mut()
            .expect("target has at least one line")
            .extend(first_line);
    }
    target.extend(iter);
}

fn decode_markup_text(token: &str) -> Option<String> {
    let stripped = token.strip_prefix('T')?;
    let x_index = stripped.find('x')?;
    Some(stripped[x_index + 1..].to_string())
}

fn decode_markup_path_slot(token: &str) -> Option<usize> {
    let reference = token.strip_prefix("?1x")?;
    if reference.chars().all(|ch| ch.is_ascii_digit()) {
        return reference.parse::<usize>().ok().filter(|slot| *slot > 0);
    }
    if let Some(slot) = reference
        .strip_prefix('B')
        .filter(|suffix| !suffix.is_empty() && suffix.chars().all(|ch| ch.is_ascii_digit()))
        .and_then(|suffix| suffix.parse::<usize>().ok())
    {
        return (slot > 0).then_some(slot);
    }
    None
}

fn render_markup_plain(nodes: &[RichMarkupNode]) -> String {
    nodes
        .iter()
        .map(render_markup_plain_node)
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>()
        .join("")
}

fn render_markup_plain_node(node: &RichMarkupNode) -> String {
    match node {
        RichMarkupNode::Text(text) => text.clone(),
        RichMarkupNode::Ignore => String::new(),
        RichMarkupNode::VerticalLines(children)
        | RichMarkupNode::PathRef { children, .. }
        | RichMarkupNode::Group(children) => render_markup_plain_inline(children),
        RichMarkupNode::Fraction {
            numerator,
            denominator,
        } => {
            let numerator = render_markup_plain_inline(numerator);
            let denominator = render_markup_plain_inline(denominator);
            if numerator.is_empty() || denominator.is_empty() {
                return numerator + &denominator;
            }
            format!("{numerator}/{denominator}")
        }
        RichMarkupNode::Root(children) => {
            let inner = render_markup_plain_inline(children);
            if inner.is_empty() {
                String::new()
            } else {
                format!("√{inner}")
            }
        }
        RichMarkupNode::Superscript(children) => {
            collapse_markup_superscript(render_markup_plain_inline(children))
        }
    }
}

#[cfg(test)]
mod markup_tests {
    use super::{
        MarkupParseError, RichMarkupNode, classify_markup_node, extract_visual_rich_text,
        parse_markup_nodes, render_markup_plain,
    };

    #[test]
    fn parses_markup_into_semantic_nodes() {
        let nodes = parse_markup_nodes("<VL<?1x2<TxAB>></<Txx><Txy>><R<Txz>><+<Txn2>>>")
            .expect("markup parses");
        assert_eq!(
            nodes,
            vec![RichMarkupNode::VerticalLines(vec![
                RichMarkupNode::PathRef {
                    slot: 2,
                    children: vec![RichMarkupNode::Text("AB".to_string())],
                },
                RichMarkupNode::Fraction {
                    numerator: vec![RichMarkupNode::Text("x".to_string())],
                    denominator: vec![RichMarkupNode::Text("y".to_string())],
                },
                RichMarkupNode::Root(vec![RichMarkupNode::Text("z".to_string())]),
                RichMarkupNode::Superscript(vec![RichMarkupNode::Text("n2".to_string())]),
            ])]
        );
    }

    #[test]
    fn renders_plain_markup_from_semantic_ast() {
        let nodes =
            parse_markup_nodes("<Txf></<Txx><Txy>><R<Txz>><+<Txn2>>>").expect("markup parses");
        assert_eq!(render_markup_plain(&nodes), "fx/y√zn^2");
    }

    #[test]
    fn preserves_visual_hotspots_from_semantic_ast() {
        let nodes = parse_markup_nodes("<VL<?1x2<TxAB>><Tx=><?1x3<TxCD>>>").expect("markup parses");
        let rich = extract_visual_rich_text(&nodes).expect("visual rich text");
        assert_eq!(rich.text, "AB\n=\nCD");
        assert_eq!(
            rich.hotspots
                .iter()
                .map(|hotspot| (hotspot.text.as_str(), hotspot.path_slot))
                .collect::<Vec<_>>(),
            vec![("AB", 2), ("CD", 3)]
        );
    }

    #[test]
    fn classifies_unknown_wrapper_as_group() {
        assert_eq!(
            classify_markup_node("X".to_string(), vec![RichMarkupNode::Text("A".to_string())]),
            RichMarkupNode::Group(vec![RichMarkupNode::Text("A".to_string())])
        );
    }

    #[test]
    fn reports_unterminated_markup_tag_with_offset() {
        assert_eq!(
            parse_markup_nodes("<VL<TxA"),
            Err(MarkupParseError::UnterminatedTag { index: 3 })
        );
    }

    #[test]
    fn reports_empty_markup_tag_with_offset() {
        assert_eq!(
            parse_markup_nodes("<>"),
            Err(MarkupParseError::EmptyTagName { index: 0 })
        );
    }
}

fn render_markup_plain_inline(nodes: &[RichMarkupNode]) -> String {
    render_markup_plain(nodes)
}

fn collapse_markup_superscript(text: String) -> String {
    if text.is_empty() {
        return text;
    }
    let chars = text.chars().collect::<Vec<_>>();
    let split = chars
        .iter()
        .rposition(|ch| !ch.is_ascii_digit())
        .map(|index| index + 1)
        .unwrap_or(0);
    if split >= chars.len() {
        return text;
    }
    let exponent = chars[split..].iter().collect::<String>();
    let mut base = chars[..split].iter().collect::<String>();
    base.push('^');
    base.push_str(&exponent);
    base
}
