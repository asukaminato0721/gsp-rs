use super::points::{TransformBindingKind, decode_transform_binding};
use crate::format::{
    GroupKind, GspFile, IndexedPathRecord, ObjectGroup, PointRecord, collect_strings,
    decode_indexed_path, read_f64, read_i16, read_u16, read_u32,
};

pub(crate) fn is_circle_group_kind(kind: GroupKind) -> bool {
    matches!(kind, GroupKind::Circle | GroupKind::CircleCenterRadius)
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

pub(crate) fn is_action_button_group(group: &ObjectGroup) -> bool {
    (group.header.kind()) == crate::format::GroupKind::ActionButton
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
    Some(
        String::from_utf8_lossy(name_bytes)
            .replace("[1]", "₁")
            .replace("[2]", "₂")
            .replace("[3]", "₃")
            .replace("[4]", "₄"),
    )
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

pub(crate) fn decode_label_visible(file: &GspFile, group: &ObjectGroup) -> Option<bool> {
    let payload = group
        .records
        .iter()
        .find(|record| record.record_type == 0x07d5)
        .map(|record| record.payload(&file.data))?;
    (payload.len() >= 4).then(|| read_u16(payload, 2) != 0)
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
            0x08fc => decode_rich_text(record.payload(&file.data)).map(|content| content.text),
            0x07d5 if matches!(group.header.kind(), crate::format::GroupKind::ActionButton) => {
                collect_strings(record.payload(&file.data))
                    .into_iter()
                    .map(|entry| entry.text.trim().to_string())
                    .find(|text| !text.is_empty())
            }
            _ => None,
        })
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

pub(crate) fn decode_group_rich_text(
    file: &GspFile,
    group: &ObjectGroup,
) -> Option<RichTextContent> {
    let record = group
        .records
        .iter()
        .find(|record| record.record_type == 0x08fc)?;
    decode_rich_text(record.payload(&file.data))
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
        .find(|record| record.record_type == 0x08fc)
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
        .or_else(|| decode_0907_anchor(file, group))
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
            let binding = decode_transform_binding(file, group)?;
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

fn decode_rich_text(payload: &[u8]) -> Option<RichTextContent> {
    let text = String::from_utf8_lossy(payload);
    let start = text.find('<')?;
    let markup = text[start..].trim_end_matches('\0');

    if markup.starts_with("<VL") {
        return extract_visual_rich_text(markup).map(|mut content| {
            content.markup = Some(markup.to_string());
            content
        });
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

    (!cleaned.is_empty()).then_some(RichTextContent {
        text: cleaned,
        hotspots: Vec::new(),
        markup: None,
    })
}

#[derive(Debug, Clone)]
struct RichMarkupNode {
    name: String,
    children: Vec<RichMarkupNode>,
}

#[derive(Debug, Clone)]
struct RichMarkupRun {
    text: String,
    path_slot: Option<usize>,
}

fn extract_visual_rich_text(markup: &str) -> Option<RichTextContent> {
    let nodes = parse_markup_nodes(markup);
    let lines = render_markup_nodes(&nodes, None);
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

fn parse_markup_nodes(markup: &str) -> Vec<RichMarkupNode> {
    fn parse_seq(s: &str, mut index: usize, stop_on_gt: bool) -> (Vec<RichMarkupNode>, usize) {
        let bytes = s.as_bytes();
        let mut nodes = Vec::new();
        while index < bytes.len() {
            if stop_on_gt && bytes[index] == b'>' {
                return (nodes, index + 1);
            }
            if bytes[index] != b'<' {
                index += 1;
                continue;
            }
            index += 1;
            let name_start = index;
            while index < bytes.len() && bytes[index] != b'<' && bytes[index] != b'>' {
                index += 1;
            }
            let name = s[name_start..index].to_string();
            let children = if index < bytes.len() && bytes[index] == b'<' {
                let (children, next_index) = parse_seq(s, index, true);
                index = next_index;
                children
            } else {
                if index < bytes.len() && bytes[index] == b'>' {
                    index += 1;
                }
                Vec::new()
            };
            nodes.push(RichMarkupNode { name, children });
        }
        (nodes, index)
    }

    let (nodes, _) = parse_seq(markup, 0, false);
    nodes
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
    if let Some(text) = decode_markup_text(&node.name) {
        return vec![vec![RichMarkupRun {
            text,
            path_slot: active_slot,
        }]];
    }
    if node.name.starts_with('!') {
        return vec![Vec::new()];
    }
    if node.name == "VL" {
        return node
            .children
            .iter()
            .flat_map(|child| render_markup_node(child, active_slot))
            .filter(|line| !line.is_empty())
            .collect::<Vec<_>>();
    }
    if let Some(path_slot) = decode_markup_path_slot(&node.name) {
        return render_markup_nodes(&node.children, Some(path_slot));
    }
    if node.name == "/" {
        let Some((numerator_node, denominator_node)) = node.children.split_first() else {
            return vec![Vec::new()];
        };
        let numerator = render_markup_inline(std::slice::from_ref(numerator_node), active_slot);
        let denominator = render_markup_inline(denominator_node, active_slot);
        if numerator.is_empty() || denominator.is_empty() {
            return vec![numerator.into_iter().chain(denominator).collect()];
        }
        let mut runs = numerator;
        runs.push(RichMarkupRun {
            text: "/".to_string(),
            path_slot: active_slot,
        });
        runs.extend(denominator);
        return vec![runs];
    }
    if node.name == "R" {
        let child_runs = render_markup_inline(&node.children, active_slot);
        if child_runs.is_empty() {
            return vec![Vec::new()];
        }
        let mut runs = vec![RichMarkupRun {
            text: "√".to_string(),
            path_slot: active_slot,
        }];
        runs.extend(child_runs);
        return vec![runs];
    }
    if node.name.starts_with('+') {
        let lines = render_markup_nodes(&node.children, active_slot);
        let joined = lines
            .into_iter()
            .flatten()
            .map(|run| run.text)
            .collect::<String>();
        if joined.is_empty() {
            return vec![Vec::new()];
        }
        let chars = joined.chars().collect::<Vec<_>>();
        let split = chars
            .iter()
            .rposition(|ch| !ch.is_ascii_digit())
            .map(|index| index + 1)
            .unwrap_or(0);
        let text = if split < chars.len() {
            let exponent = chars[split..].iter().collect::<String>();
            let mut base = chars[..split].iter().collect::<String>();
            base.push('^');
            base.push_str(&exponent);
            base
        } else {
            joined
        };
        return vec![vec![RichMarkupRun {
            text,
            path_slot: active_slot,
        }]];
    }
    render_markup_nodes(&node.children, active_slot)
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
                    if name == "/" && inner_parts.len() >= 2 {
                        inner = format!("{}/{}", inner_parts[0], inner_parts[1]);
                    } else if name == "R" && !inner.is_empty() {
                        inner = format!("√{inner}");
                    }
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
