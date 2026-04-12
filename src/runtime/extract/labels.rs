use std::collections::{BTreeMap, BTreeSet};

use super::CircleShape;
use super::decode::{
    RichTextHotspotRef, decode_label_anchor, decode_label_name, decode_label_name_raw,
    decode_label_visible, decode_text_anchor, find_indexed_path, is_action_button_group,
    try_decode_0907_anchor, try_decode_group_label_text, try_decode_group_rich_text,
    try_decode_link_button_url, try_decode_parameter_control_value_for_group,
};
use super::points::{
    RawPointConstraint, editable_non_graph_parameter_name_for_group,
    is_editable_non_graph_parameter_name, is_non_graph_parameter_group, try_decode_point_constraint,
};
use crate::format::{GspFile, ObjectGroup, PointRecord, read_f64, read_u32};
use crate::runtime::functions::{
    evaluate_expr_with_parameters, function_expr_label, try_decode_function_expr,
};
use crate::runtime::geometry::{color_from_style, format_number};
use crate::runtime::payload_consts::RECORD_POINT_F64_PAIR;
use crate::runtime::scene::{
    IterationTable, LabelIterationFamily, ScreenPoint, TextLabel, TextLabelBinding,
    TextLabelHotspot, TextLabelHotspotAction,
};

#[derive(Debug, Clone)]
pub(super) struct PendingLabelHotspot {
    pub(super) label_index: usize,
    pub(super) line: usize,
    pub(super) start: usize,
    pub(super) end: usize,
    pub(super) text: String,
    pub(super) group_ordinal: usize,
}

struct ResolvedLabelText {
    text: String,
    rich_markup: Option<String>,
    hotspots: Vec<RichTextHotspotRef>,
}

fn supports_payload_label(kind: crate::format::GroupKind) -> bool {
    matches!(
        kind,
        crate::format::GroupKind::Point
            | crate::format::GroupKind::CustomTransformPoint
            | crate::format::GroupKind::Translation
            | crate::format::GroupKind::Reflection
            | crate::format::GroupKind::Rotation
            | crate::format::GroupKind::ParameterRotation
            | crate::format::GroupKind::Scale
            | crate::format::GroupKind::PointConstraint
            | crate::format::GroupKind::PathPoint
            | crate::format::GroupKind::LinearIntersectionPoint
            | crate::format::GroupKind::IntersectionPoint1
            | crate::format::GroupKind::IntersectionPoint2
            | crate::format::GroupKind::CircleCircleIntersectionPoint1
            | crate::format::GroupKind::CircleCircleIntersectionPoint2
            | crate::format::GroupKind::Segment
            | crate::format::GroupKind::Ray
            | crate::format::GroupKind::GraphObject40
            | crate::format::GroupKind::Kind51
            | crate::format::GroupKind::AngleMarker
            | crate::format::GroupKind::ActionButton
            | crate::format::GroupKind::ButtonLabel
            | crate::format::GroupKind::LabelIterationSeed
    )
}

fn midpoint_has_explicit_label(group: &ObjectGroup) -> bool {
    (group.header.style_c >> 16) != 0
}

fn point_label_uses_group_color(group: &ObjectGroup) -> bool {
    matches!(
        group.header.kind(),
        crate::format::GroupKind::CustomTransformPoint
            | crate::format::GroupKind::Translation
            | crate::format::GroupKind::Reflection
            | crate::format::GroupKind::Rotation
            | crate::format::GroupKind::ParameterRotation
            | crate::format::GroupKind::Scale
            | crate::format::GroupKind::PointConstraint
            | crate::format::GroupKind::PathPoint
            | crate::format::GroupKind::LinearIntersectionPoint
            | crate::format::GroupKind::IntersectionPoint1
            | crate::format::GroupKind::IntersectionPoint2
            | crate::format::GroupKind::CircleCircleIntersectionPoint1
            | crate::format::GroupKind::CircleCircleIntersectionPoint2
    )
}

fn label_color_for_group(group: &ObjectGroup) -> [u8; 4] {
    if group.header.kind() == crate::format::GroupKind::PointConstraint {
        return match ((group.header.style_a >> 24) & 0xff) as u8 {
            0x02 => [0, 0, 255, 255],
            0x03 => color_from_style(group.header.style_b),
            _ => [30, 30, 30, 255],
        };
    }

    if point_label_uses_group_color(group)
        || (group.header.kind() == crate::format::GroupKind::Point
            && !group
                .records
                .iter()
                .any(|record| record.record_type == RECORD_POINT_F64_PAIR))
    {
        color_from_style(group.header.style_b)
    } else {
        [30, 30, 30, 255]
    }
}

fn resolve_label_text(
    file: &GspFile,
    group: &ObjectGroup,
    fallback_text: Option<String>,
) -> Option<ResolvedLabelText> {
    let rich_text = try_decode_group_rich_text(file, group).ok().flatten();
    let text = rich_text
        .as_ref()
        .map(|content| content.text.clone())
        .or_else(|| try_decode_group_label_text(file, group).ok().flatten())
        .or(fallback_text)?;
    Some(ResolvedLabelText {
        text,
        rich_markup: rich_text
            .as_ref()
            .and_then(|content| content.markup.clone()),
        hotspots: rich_text
            .map(|content| content.hotspots)
            .unwrap_or_default(),
    })
}

fn push_pending_label_hotspots(
    file: &GspFile,
    group: &ObjectGroup,
    label_index: usize,
    hotspots: &[RichTextHotspotRef],
    pending_hotspots: &mut Vec<PendingLabelHotspot>,
) {
    if hotspots.is_empty() {
        return;
    }
    let Some(path) = find_indexed_path(file, group) else {
        return;
    };
    for hotspot in hotspots {
        if let Some(group_ordinal) = path.refs.get(hotspot.path_slot.saturating_sub(1)).copied() {
            pending_hotspots.push(PendingLabelHotspot {
                label_index,
                line: hotspot.line,
                start: hotspot.start,
                end: hotspot.end,
                text: hotspot.text.clone(),
                group_ordinal,
            });
        }
    }
}

fn label_visible_for_group(file: &GspFile, group: &ObjectGroup) -> bool {
    !group.header.is_hidden() && decode_label_visible(file, group).unwrap_or(true)
}

fn resolve_function_expr_parameter(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
    anchors: &[Option<PointRecord>],
    visiting: &mut BTreeSet<usize>,
) -> Option<(String, f64)> {
    if !visiting.insert(group.ordinal) {
        return None;
    }
    let result = (|| {
        let path = find_indexed_path(file, group)?;
        let parameter_group = groups.get(path.refs.first()?.checked_sub(1)?)?;
        if parameter_group.header.kind() == crate::format::GroupKind::FunctionExpr {
            return resolve_function_expr_parameter(file, groups, parameter_group, anchors, visiting);
        }
        let name = if parameter_group.header.kind() == crate::format::GroupKind::ParameterAnchor {
            let anchor_path = find_indexed_path(file, parameter_group)?;
            let point_group = groups.get(anchor_path.refs.first()?.checked_sub(1)?)?;
            decode_label_name(file, parameter_group).or_else(|| decode_label_name(file, point_group))?
        } else {
            editable_non_graph_parameter_name_for_group(file, groups, parameter_group)?
        };
        let value = if parameter_group.header.kind() == crate::format::GroupKind::ParameterAnchor {
            parameter_anchor_value(file, groups, parameter_group, anchors)?
        } else {
            try_decode_parameter_control_value_for_group(file, groups, parameter_group).ok()?
        };
        Some((name, value))
    })();
    visiting.remove(&group.ordinal);
    result
}

fn parameter_anchor_value(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
    anchors: &[Option<PointRecord>],
) -> Option<f64> {
    if group.header.kind() != crate::format::GroupKind::ParameterAnchor {
        return None;
    }
    let path = find_indexed_path(file, group)?;
    let point_group_index = path.refs.first()?.checked_sub(1)?;
    let point_group = groups.get(point_group_index)?;
    match try_decode_point_constraint(file, groups, point_group, None, &None).ok()? {
        RawPointConstraint::Segment(constraint) => Some(constraint.t),
        RawPointConstraint::PolygonBoundary {
            edge_index,
            t,
            vertex_group_indices,
        } => polygon_boundary_parameter(anchors, &vertex_group_indices, edge_index, t),
        RawPointConstraint::Circle(constraint) => circle_parameter(
            anchors,
            constraint.center_group_index,
            constraint.radius_group_index,
            constraint.unit_x,
            constraint.unit_y,
        ),
        RawPointConstraint::CircleArc(_) => None,
        RawPointConstraint::Arc(_) => None,
        RawPointConstraint::Polyline { .. } => None,
    }
}

fn iteration_group_value(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
    anchors: &[Option<PointRecord>],
    visiting: &mut BTreeSet<usize>,
) -> Option<f64> {
    if !visiting.insert(group.ordinal) {
        return None;
    }
    let result = (|| match group.header.kind() {
        crate::format::GroupKind::Point => {
            try_decode_parameter_control_value_for_group(file, groups, group).ok()
        }
        crate::format::GroupKind::ParameterAnchor => {
            parameter_anchor_value(file, groups, group, anchors)
        }
        crate::format::GroupKind::FunctionExpr => {
            let expr = try_decode_function_expr(file, groups, group).ok()?;
            let path = find_indexed_path(file, group)?;
            let mut parameters = BTreeMap::new();
            for obj_ref in &path.refs {
                let ref_group = groups.get(obj_ref.checked_sub(1)?)?;
                let name = decode_label_name(file, ref_group)
                    .or_else(|| decode_label_name_raw(file, ref_group))?;
                let value = iteration_group_value(file, groups, ref_group, anchors, visiting)?;
                parameters.insert(name, value);
            }
            evaluate_expr_with_parameters(&expr, 0.0, &parameters)
        }
        _ => None,
    })();
    visiting.remove(&group.ordinal);
    result
}

fn payload_function_expr_label(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
    group: &ObjectGroup,
    fallback_expr_label: &str,
    visiting: &mut BTreeSet<usize>,
) -> String {
    if !visiting.insert(group.ordinal) {
        return fallback_expr_label.to_string();
    }
    let result = (|| {
        if let Some(label) = decode_label_name(file, group) {
            return Some(label);
        }
        let path = find_indexed_path(file, group)?;
        let source_group = groups.get(path.refs.first()?.checked_sub(1)?)?;
        let source_label = if source_group.header.kind() == crate::format::GroupKind::FunctionExpr {
            payload_function_expr_label(file, groups, anchors, source_group, fallback_expr_label, visiting)
        } else {
            decode_label_name(file, source_group).or_else(|| {
                if source_group.header.kind() == crate::format::GroupKind::ParameterAnchor {
                    let anchor_path = find_indexed_path(file, source_group)?;
                    let point_group = groups.get(anchor_path.refs.first()?.checked_sub(1)?)?;
                    decode_label_name(file, point_group)
                } else {
                    None
                }
            })?
        };
        let (parameter_name, _) = resolve_function_expr_parameter(
            file,
            groups,
            group,
            anchors,
            &mut BTreeSet::new(),
        )?;
        Some(fallback_expr_label.replacen(&parameter_name, &source_label, 1))
    })()
    .unwrap_or_else(|| fallback_expr_label.to_string());
    visiting.remove(&group.ordinal);
    result
}

fn decode_iteration_table_anchor(file: &GspFile, group: &ObjectGroup) -> Option<ScreenPoint> {
    let payload = group
        .records
        .iter()
        .find(|record| record.record_type == 0x090d)
        .map(|record| record.payload(&file.data))?;
    (payload.len() >= 16).then(|| ScreenPoint {
        x: crate::format::read_u16(payload, 12) as f64,
        y: crate::format::read_u16(payload, 14) as f64,
    })
}

fn mapped_point_index(group_to_point_index: &[Option<usize>], group_index: usize) -> Option<usize> {
    group_to_point_index.get(group_index).copied().flatten()
}

pub(super) fn collect_labels(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
    graph_mode: bool,
    include_measurements: bool,
) -> (
    Vec<TextLabel>,
    BTreeMap<usize, usize>,
    Vec<PendingLabelHotspot>,
) {
    let mut labels = Vec::new();
    let mut label_group_to_index = BTreeMap::new();
    let mut pending_hotspots = Vec::new();
    for group in groups {
        let kind = group.header.kind();
        match kind {
            crate::format::GroupKind::Midpoint if midpoint_has_explicit_label(group) => {
                if let Some(label_text) =
                    resolve_label_text(file, group, decode_label_name(file, group))
                    && let Some(anchor) = decode_label_anchor(file, group, anchors)
                {
                    let ResolvedLabelText {
                        text,
                        rich_markup,
                        hotspots,
                    } = label_text;
                    let binding = angle_marker_measurement_binding(file, group, &text);
                    let visible = label_visible_for_group(file, group);
                    let label_index = labels.len();
                    label_group_to_index.insert(group.ordinal, label_index);
                    labels.push(TextLabel {
                        anchor,
                        text,
                        rich_markup,
                        color: label_color_for_group(group),
                        visible,
                        binding,
                        screen_space: false,
                        hotspots: Vec::new(),
                    });
                    push_pending_label_hotspots(
                        file,
                        group,
                        label_index,
                        &hotspots,
                        &mut pending_hotspots,
                    );
                }
            }
            kind if supports_payload_label(kind) => {
                if kind == crate::format::GroupKind::Point
                    && try_decode_link_button_url(file, group)
                        .ok()
                        .flatten()
                        .is_some()
                {
                    continue;
                }
                if kind == crate::format::GroupKind::ActionButton && is_action_button_group(group) {
                    continue;
                }
                if kind == crate::format::GroupKind::LabelIterationSeed {
                    if let Some(label) =
                        collect_point_expression_label(file, groups, group, anchors)
                    {
                        label_group_to_index.insert(group.ordinal, labels.len());
                        labels.push(label);
                    }
                    continue;
                }
                let fallback_text = (!graph_mode
                    && matches!(
                        kind,
                        crate::format::GroupKind::Point
                            | crate::format::GroupKind::CustomTransformPoint
                            | crate::format::GroupKind::Translation
                            | crate::format::GroupKind::Reflection
                            | crate::format::GroupKind::Rotation
                            | crate::format::GroupKind::ParameterRotation
                            | crate::format::GroupKind::Scale
                            | crate::format::GroupKind::Segment
                            | crate::format::GroupKind::Ray
                            | crate::format::GroupKind::AngleMarker
                            | crate::format::GroupKind::PointConstraint
                            | crate::format::GroupKind::PathPoint
                            | crate::format::GroupKind::LinearIntersectionPoint
                            | crate::format::GroupKind::IntersectionPoint1
                            | crate::format::GroupKind::IntersectionPoint2
                            | crate::format::GroupKind::CircleCircleIntersectionPoint1
                            | crate::format::GroupKind::CircleCircleIntersectionPoint2
                    )
                    && !is_non_graph_parameter_group(file, groups, group))
                .then(|| decode_label_name(file, group))
                .flatten();
                if let Some(label_text) = resolve_label_text(file, group, fallback_text)
                    && let Some(anchor) = decode_label_anchor(file, group, anchors)
                {
                    let ResolvedLabelText {
                        text,
                        rich_markup,
                        hotspots,
                    } = label_text;
                    let binding = angle_marker_measurement_binding(file, group, &text);
                    let visible = label_visible_for_group(file, group);
                    let label_index = labels.len();
                    label_group_to_index.insert(group.ordinal, label_index);
                    labels.push(TextLabel {
                        anchor,
                        text,
                        rich_markup,
                        color: label_color_for_group(group),
                        visible,
                        binding,
                        screen_space: false,
                        hotspots: Vec::new(),
                    });
                    push_pending_label_hotspots(
                        file,
                        group,
                        label_index,
                        &hotspots,
                        &mut pending_hotspots,
                    );
                }
            }
            crate::format::GroupKind::FunctionExpr => {}
            crate::format::GroupKind::GraphCalibrationX
            | crate::format::GroupKind::GraphCalibrationY => {
                if !include_measurements {
                    continue;
                }
                let anchor = anchors
                    .get(group.ordinal.saturating_sub(1))
                    .cloned()
                    .flatten()
                    .or_else(|| {
                        let path = find_indexed_path(file, group)?;
                        path.refs.iter().find_map(|object_ref| {
                            anchors.get(object_ref.saturating_sub(1)).cloned().flatten()
                        })
                    });
                if anchor
                    .as_ref()
                    .is_some_and(|anchor| anchor.x.abs() < 1e-6 && anchor.y.abs() < 1e-6)
                {
                    continue;
                }
                // Graph calibration groups may carry measurement payloads for the axis scale
                // without any user-visible label text. Only emit a label when the payload
                // explicitly includes text instead of synthesizing one from the scale value.
                let text = try_decode_group_label_text(file, group)
                    .ok()
                    .flatten()
                    .or_else(|| decode_label_name(file, group));
                if let (Some(anchor), Some(text)) = (anchor, text) {
                    labels.push(TextLabel {
                        anchor,
                        text,
                        rich_markup: None,
                        color: [60, 60, 60, 255],
                        visible: label_visible_for_group(file, group),
                        binding: None,
                        screen_space: false,
                        hotspots: Vec::new(),
                    });
                }
            }
            _ => {}
        }
    }
    (labels, label_group_to_index, pending_hotspots)
}

fn angle_marker_measurement_binding(
    file: &GspFile,
    group: &ObjectGroup,
    text: &str,
) -> Option<TextLabelBinding> {
    if group.header.kind() != crate::format::GroupKind::AngleMarker {
        return None;
    }
    let decimals = measurement_label_decimals(text)?;
    let path = find_indexed_path(file, group)?;
    if path.refs.len() != 3 {
        return None;
    }
    Some(TextLabelBinding::AngleMarkerValue {
        start_index: path.refs[0].checked_sub(1)?,
        vertex_index: path.refs[1].checked_sub(1)?,
        end_index: path.refs[2].checked_sub(1)?,
        decimals,
    })
}

fn measurement_label_decimals(text: &str) -> Option<usize> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }
    let mut dot_index = None;
    for (index, ch) in trimmed.char_indices() {
        if ch == '.' {
            if dot_index.is_some() {
                return None;
            }
            dot_index = Some(index);
        } else if !(ch.is_ascii_digit() || (index == 0 && (ch == '+' || ch == '-'))) {
            return None;
        }
    }
    trimmed.parse::<f64>().ok()?;
    Some(
        dot_index
            .map(|index| trimmed[index + 1..].chars().count())
            .unwrap_or(0),
    )
}

fn build_expression_rich_markup(expr_label: &str, value_text: &str) -> Option<String> {
    let render_part = |text: &str| text.replace('*', "\u{00b7}");
    let slash_count = expr_label.matches(" / ").count();
    if slash_count > 1 || (slash_count == 1 && expr_label.contains('(')) {
        return None;
    }
    if let Some((numerator, denominator)) = split_top_level(expr_label, " / ") {
        return Some(format!(
            "<H</<Tx{}><Tx{}>><Tx = {}>>",
            render_part(numerator),
            render_part(denominator),
            value_text,
        ));
    }
    Some(format!(
        "<H<Tx{} = {}>>",
        render_part(expr_label),
        value_text,
    ))
}

fn split_top_level<'a>(text: &'a str, needle: &str) -> Option<(&'a str, &'a str)> {
    let mut depth = 0usize;
    let bytes = text.as_bytes();
    let needle_bytes = needle.as_bytes();
    let mut index = 0usize;
    while index + needle_bytes.len() <= bytes.len() {
        match bytes[index] as char {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            _ => {}
        }
        if depth == 0 && &bytes[index..index + needle_bytes.len()] == needle_bytes {
            let left = text[..index].trim();
            let right = text[index + needle.len()..].trim();
            return Some((left, right));
        }
        index += 1;
    }
    None
}

pub(super) fn collect_coordinate_labels(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
) -> Vec<TextLabel> {
    let mut labels = Vec::new();
    for group in groups {
        let kind = group.header.kind();
        let helper_visible = !group.header.is_hidden();
        if kind == crate::format::GroupKind::Point
            && is_non_graph_parameter_group(file, groups, group)
            && let Some(name) = decode_label_name(file, group)
            && let Some(value) =
                try_decode_parameter_control_value_for_group(file, groups, group).ok()
            && let Some(anchor) = try_decode_0907_anchor(file, group).ok().flatten()
        {
            let binding = is_editable_non_graph_parameter_name(&name)
                .then(|| TextLabelBinding::ParameterValue { name: name.clone() });
            labels.push(TextLabel {
                anchor,
                text: format!("{name} = {:.2}", value),
                rich_markup: None,
                color: [30, 30, 30, 255],
                visible: helper_visible,
                binding,
                screen_space: true,
                hotspots: Vec::new(),
            });
        } else if kind == crate::format::GroupKind::FunctionExpr
            && let Some(expr) = try_decode_function_expr(file, groups, group).ok()
            && let Some((parameter_name, parameter_value)) =
                resolve_function_expr_parameter(file, groups, group, anchors, &mut BTreeSet::new())
            && let Some(anchor) = try_decode_0907_anchor(file, group).ok().flatten()
        {
            let value = evaluate_expr_with_parameters(
                &expr,
                0.0,
                &BTreeMap::from([(parameter_name.clone(), parameter_value)]),
            );
            let expr_label = payload_function_expr_label(
                file,
                groups,
                anchors,
                group,
                &function_expr_label(expr.clone()),
                &mut BTreeSet::new(),
            );
            let binding = is_editable_non_graph_parameter_name(&parameter_name).then(|| {
                TextLabelBinding::ExpressionValue {
                    parameter_name: parameter_name.clone(),
                    result_name: decode_label_name(file, group),
                    expr_label: expr_label.clone(),
                    expr: expr.clone(),
                }
            });
            let value_text = value
                .map(format_number)
                .unwrap_or_else(|| "未定义".to_string());
            let text = format!("{expr_label} = {value_text}");
            labels.push(TextLabel {
                anchor,
                text,
                rich_markup: build_expression_rich_markup(&expr_label, &value_text),
                color: [30, 30, 30, 255],
                visible: helper_visible,
                binding,
                screen_space: true,
                hotspots: Vec::new(),
            });
        }
    }
    labels
}

pub(super) fn resolve_label_hotspots(
    file: &GspFile,
    groups: &[ObjectGroup],
    labels: &mut [TextLabel],
    pending_hotspots: &[PendingLabelHotspot],
    group_to_point_index: &[Option<usize>],
    circle_group_to_index: &[Option<usize>],
    polygon_group_to_index: &[Option<usize>],
    button_group_to_index: &BTreeMap<usize, usize>,
) {
    for pending in pending_hotspots {
        let Some(label) = labels.get_mut(pending.label_index) else {
            continue;
        };

        let Some(group) = groups.get(pending.group_ordinal.saturating_sub(1)) else {
            continue;
        };
        let action = match group.header.kind() {
            crate::format::GroupKind::ActionButton => button_group_to_index
                .get(&pending.group_ordinal)
                .copied()
                .map(|button_index| TextLabelHotspotAction::Button { button_index }),
            crate::format::GroupKind::ButtonLabel => (|| {
                let path = find_indexed_path(file, group)?;
                let ordinal = path.refs.first().copied()?;
                button_group_to_index
                    .get(&ordinal)
                    .copied()
                    .map(|button_index| TextLabelHotspotAction::Button { button_index })
            })(),
            crate::format::GroupKind::Point => group_to_point_index
                .get(pending.group_ordinal.saturating_sub(1))
                .copied()
                .flatten()
                .map(|point_index| TextLabelHotspotAction::Point { point_index }),
            crate::format::GroupKind::Segment => (|| {
                let path = find_indexed_path(file, group)?;
                let start_point_index =
                    mapped_point_index(group_to_point_index, path.refs.first()?.saturating_sub(1))?;
                let end_point_index =
                    mapped_point_index(group_to_point_index, path.refs.get(1)?.saturating_sub(1))?;
                Some(TextLabelHotspotAction::Segment {
                    start_point_index,
                    end_point_index,
                })
            })(),
            crate::format::GroupKind::AngleMarker => (|| {
                let path = find_indexed_path(file, group)?;
                let start_point_index =
                    mapped_point_index(group_to_point_index, path.refs.first()?.saturating_sub(1))?;
                let vertex_point_index =
                    mapped_point_index(group_to_point_index, path.refs.get(1)?.saturating_sub(1))?;
                let end_point_index =
                    mapped_point_index(group_to_point_index, path.refs.get(2)?.saturating_sub(1))?;
                Some(TextLabelHotspotAction::AngleMarker {
                    start_point_index,
                    vertex_point_index,
                    end_point_index,
                })
            })(),
            kind if super::decode::is_circle_group_kind(kind) => circle_group_to_index
                .get(pending.group_ordinal.saturating_sub(1))
                .copied()
                .flatten()
                .map(|circle_index| TextLabelHotspotAction::Circle { circle_index }),
            crate::format::GroupKind::Polygon => polygon_group_to_index
                .get(pending.group_ordinal.saturating_sub(1))
                .copied()
                .flatten()
                .map(|polygon_index| TextLabelHotspotAction::Polygon { polygon_index }),
            _ => None,
        };
        let Some(action) = action else {
            continue;
        };
        label.hotspots.push(TextLabelHotspot {
            line: pending.line,
            start: pending.start,
            end: pending.end,
            text: pending.text.clone(),
            action,
        });
    }
}

fn collect_point_expression_label(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
    anchors: &[Option<PointRecord>],
) -> Option<TextLabel> {
    let path = find_indexed_path(file, group)?;
    if path.refs.len() < 2 {
        return None;
    }
    let point_group_index = path.refs.first()?.checked_sub(1)?;
    let expr_group = groups.get(path.refs[1].checked_sub(1)?)?;
    if (expr_group.header.kind()) != crate::format::GroupKind::FunctionExpr {
        return None;
    }
    let expr = try_decode_function_expr(file, groups, expr_group).ok()?;
    let expr_path = find_indexed_path(file, expr_group)?;
    let parameter_group = groups.get(expr_path.refs.first()?.checked_sub(1)?)?;
    let parameter_name =
        editable_non_graph_parameter_name_for_group(file, groups, parameter_group)?;
    let parameter_value =
        try_decode_parameter_control_value_for_group(file, groups, parameter_group).ok()?;
    let value = evaluate_expr_with_parameters(
        &expr,
        0.0,
        &BTreeMap::from([(parameter_name.clone(), parameter_value)]),
    )?;
    let anchor = decode_label_anchor(file, group, anchors)?;
    Some(TextLabel {
        anchor,
        text: format_number(value),
        rich_markup: None,
        color: [30, 30, 30, 255],
        visible: label_visible_for_group(file, group),
        binding: Some(TextLabelBinding::PointExpressionValue {
            point_index: point_group_index,
            parameter_name,
            expr,
        }),
        screen_space: false,
        hotspots: Vec::new(),
    })
}

pub(super) fn collect_polygon_parameter_labels(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
) -> Vec<TextLabel> {
    groups
        .iter()
        .filter(|group| (group.header.kind()) == crate::format::GroupKind::ParameterAnchor)
        .filter_map(|group| {
            let path = find_indexed_path(file, group)?;
            if path.refs.len() < 2 {
                return None;
            }

            let point_group = groups.get(path.refs[0].checked_sub(1)?)?;
            let polygon_group = groups.get(path.refs[1].checked_sub(1)?)?;
            if !point_group.header.kind().is_point_constraint()
                || (polygon_group.header.kind()) != crate::format::GroupKind::Polygon
            {
                return None;
            }

            let point_name =
                decode_label_name(file, group).or_else(|| decode_label_name(file, point_group))?;
            let polygon_name = polygon_vertex_name(file, groups, polygon_group)?;
            let anchor_record = group
                .records
                .iter()
                .find(|record| record.record_type == 0x0903)?;
            let anchor = decode_text_anchor(anchor_record.payload(&file.data))?;
            let RawPointConstraint::PolygonBoundary {
                vertex_group_indices,
                edge_index,
                t,
            } = try_decode_point_constraint(file, groups, point_group, None, &None).ok()?
            else {
                return None;
            };
            let global_t =
                polygon_boundary_parameter(anchors, &vertex_group_indices, edge_index, t)?;

            Some(TextLabel {
                anchor,
                text: if decode_label_name(file, group).is_some() {
                    format!("{point_name} = {:.2}", global_t)
                } else {
                    format!("{point_name}在{polygon_name}上的t值 = {:.2}", global_t)
                },
                rich_markup: None,
                color: [30, 30, 30, 255],
                visible: decode_label_name(file, group).is_some()
                    || label_visible_for_group(file, group),
                binding: Some(TextLabelBinding::PolygonBoundaryParameter {
                    point_index: path.refs[0].checked_sub(1)?,
                    point_name,
                    polygon_name,
                }),
                screen_space: false,
                hotspots: Vec::new(),
            })
        })
        .collect()
}

pub(super) fn collect_segment_parameter_labels(
    file: &GspFile,
    groups: &[ObjectGroup],
) -> Vec<TextLabel> {
    groups
        .iter()
        .filter(|group| (group.header.kind()) == crate::format::GroupKind::ParameterAnchor)
        .filter_map(|group| {
            let path = find_indexed_path(file, group)?;
            if path.refs.len() < 2 {
                return None;
            }

            let point_group = groups.get(path.refs[0].checked_sub(1)?)?;
            let segment_group = groups.get(path.refs[1].checked_sub(1)?)?;
            if !point_group.header.kind().is_point_constraint()
                || !segment_group.header.kind().is_line_like()
            {
                return None;
            }

            let point_name =
                decode_label_name(file, group).or_else(|| decode_label_name(file, point_group))?;
            let segment_name = segment_name(file, groups, segment_group)?;
            let anchor_record = group
                .records
                .iter()
                .find(|record| record.record_type == 0x0903)?;
            let anchor = decode_text_anchor(anchor_record.payload(&file.data))?;
            let RawPointConstraint::Segment(constraint) =
                try_decode_point_constraint(file, groups, point_group, None, &None).ok()?
            else {
                return None;
            };

            Some(TextLabel {
                anchor,
                text: if decode_label_name(file, group).is_some() {
                    format!("{point_name} = {:.2}", constraint.t)
                } else {
                    format!("{point_name}在{segment_name}上的t值 = {:.2}", constraint.t)
                },
                rich_markup: None,
                color: [30, 30, 30, 255],
                visible: decode_label_name(file, group).is_some()
                    || label_visible_for_group(file, group),
                binding: Some(TextLabelBinding::SegmentParameter {
                    point_index: path.refs[0].checked_sub(1)?,
                    point_name,
                    segment_name,
                }),
                screen_space: false,
                hotspots: Vec::new(),
            })
        })
        .collect()
}

pub(super) fn collect_circle_parameter_labels(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
) -> Vec<TextLabel> {
    groups
        .iter()
        .filter(|group| (group.header.kind()) == crate::format::GroupKind::ParameterAnchor)
        .filter_map(|group| {
            let path = find_indexed_path(file, group)?;
            if path.refs.len() < 2 {
                return None;
            }

            let point_group = groups.get(path.refs[0].checked_sub(1)?)?;
            let circle_group = groups.get(path.refs[1].checked_sub(1)?)?;
            if !point_group.header.kind().is_point_constraint()
                || (circle_group.header.kind()) != crate::format::GroupKind::Circle
            {
                return None;
            }

            let point_name =
                decode_label_name(file, group).or_else(|| decode_label_name(file, point_group))?;
            let circle_name = circle_name(file, groups, circle_group)?;
            let anchor_record = group
                .records
                .iter()
                .find(|record| record.record_type == 0x0903)?;
            let anchor = decode_text_anchor(anchor_record.payload(&file.data))?;
            let RawPointConstraint::Circle(constraint) =
                try_decode_point_constraint(file, groups, point_group, None, &None).ok()?
            else {
                return None;
            };
            let value = circle_parameter(
                anchors,
                constraint.center_group_index,
                constraint.radius_group_index,
                constraint.unit_x,
                constraint.unit_y,
            )?;

            Some(TextLabel {
                anchor,
                text: if decode_label_name(file, group).is_some() {
                    format!("{point_name} = {:.2}", value)
                } else {
                    format!("{point_name}在⊙{circle_name}上的值 = {:.2}", value)
                },
                rich_markup: None,
                color: [30, 30, 30, 255],
                visible: decode_label_name(file, group).is_some()
                    || label_visible_for_group(file, group),
                binding: Some(TextLabelBinding::CircleParameter {
                    point_index: path.refs[0].checked_sub(1)?,
                    point_name,
                    circle_name,
                }),
                screen_space: false,
                hotspots: Vec::new(),
            })
        })
        .collect()
}

pub(super) fn collect_custom_transform_expression_labels(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
) -> Vec<TextLabel> {
    groups
        .iter()
        .filter(|group| (group.header.kind()) == crate::format::GroupKind::CustomTransformTrace)
        .filter_map(|group| {
            let path = find_indexed_path(file, group)?;
            if path.refs.len() < 6 {
                return None;
            }
            let source_group = groups.get(path.refs.get(2)?.checked_sub(1)?)?;
            let parameter_anchor_group = groups.get(path.refs.get(3)?.checked_sub(1)?)?;
            let distance_expr_group = groups.get(path.refs.get(4)?.checked_sub(1)?)?;
            let angle_expr_group = groups.get(path.refs.get(5)?.checked_sub(1)?)?;
            let base_label =
                custom_transform_parameter_anchor_label(file, groups, parameter_anchor_group)?;
            let t =
                match try_decode_point_constraint(file, groups, source_group, Some(anchors), &None)
                    .ok()?
                {
                    RawPointConstraint::Segment(constraint) => constraint.t,
                    _ => return None,
                };

            let mut labels = Vec::new();
            for (expr_group, suffix, multiplier_text, display_scale, value_suffix, decimals) in [
                (
                    distance_expr_group,
                    custom_transform_expr_suffix(file, distance_expr_group),
                    "1厘米",
                    1.0,
                    " 厘米",
                    4usize,
                ),
                (
                    angle_expr_group,
                    custom_transform_expr_suffix(file, angle_expr_group),
                    "100°",
                    100.0,
                    "°",
                    5usize,
                ),
            ] {
                let suffix_code = suffix?;
                if !matches!(suffix_code, 0x0201 | 0x0101) {
                    continue;
                }
                let anchor = try_decode_0907_anchor(file, expr_group).ok().flatten()?;
                let expr = try_decode_function_expr(file, groups, expr_group).ok()?;
                let value = evaluate_expr_with_parameters(
                    &expr,
                    t,
                    &BTreeMap::from([(
                        format!("__param_anchor_{}", parameter_anchor_group.ordinal),
                        t,
                    )]),
                )? * display_scale;
                labels.push(TextLabel {
                    anchor,
                    text: format!(
                        "{base_label}·{multiplier_text} = {value:.decimals$}{value_suffix}"
                    ),
                    rich_markup: None,
                    color: [30, 30, 30, 255],
                    visible: label_visible_for_group(file, group),
                    binding: Some(TextLabelBinding::CustomTransformValue {
                        point_index: path.refs.get(2)?.checked_sub(1)?,
                        expr_label: format!("{base_label}·{multiplier_text}"),
                        expr,
                        value_scale: display_scale,
                        value_suffix: value_suffix.to_string(),
                    }),
                    screen_space: false,
                    hotspots: Vec::new(),
                });
            }
            Some(labels)
        })
        .flatten()
        .collect()
}

fn custom_transform_parameter_anchor_label(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
) -> Option<String> {
    let path = find_indexed_path(file, group)?;
    if path.refs.len() < 2 {
        return None;
    }
    let point_group = groups.get(path.refs[0].checked_sub(1)?)?;
    let segment_group = groups.get(path.refs[1].checked_sub(1)?)?;
    if !point_group.header.kind().is_point_constraint() || !segment_group.header.kind().is_line_like() {
        return None;
    }
    Some(format!(
        "{}在{}上的t值",
        decode_label_name(file, point_group)?,
        segment_name(file, groups, segment_group)?
    ))
}

fn custom_transform_expr_suffix(file: &GspFile, expr_group: &ObjectGroup) -> Option<u16> {
    let payload = expr_group
        .records
        .iter()
        .find(|record| record.record_type == 0x0907)
        .map(|record| record.payload(&file.data))?;
    payload
        .chunks_exact(2)
        .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
        .next_back()
}

pub(super) fn collect_label_iterations(
    file: &GspFile,
    groups: &[ObjectGroup],
    label_group_to_index: &BTreeMap<usize, usize>,
    group_to_point_index: &[Option<usize>],
) -> Vec<LabelIterationFamily> {
    groups
        .iter()
        .filter(|group| (group.header.kind()) == crate::format::GroupKind::IterationBinding)
        .filter_map(|group| {
            let path = find_indexed_path(file, group)?;
            if path.refs.len() < 2 {
                return None;
            }
            let seed_group = groups.get(path.refs[0].checked_sub(1)?)?;
            if (seed_group.header.kind()) != crate::format::GroupKind::LabelIterationSeed {
                return None;
            }
            let seed_path = find_indexed_path(file, seed_group)?;
            let point_group_index = seed_path.refs.first()?.checked_sub(1)?;
            let point_seed_index = mapped_point_index(group_to_point_index, point_group_index)?;
            let expr_group = groups.get(seed_path.refs.get(1)?.checked_sub(1)?)?;
            if (expr_group.header.kind()) != crate::format::GroupKind::FunctionExpr {
                return None;
            }
            let expr = try_decode_function_expr(file, groups, expr_group).ok()?;
            let expr_path = find_indexed_path(file, expr_group)?;
            let parameter_group = groups.get(expr_path.refs.first()?.checked_sub(1)?)?;
            let parameter_name = decode_label_name(file, parameter_group)?;
            let seed_label_index = *label_group_to_index.get(&seed_group.ordinal)?;

            let iter_group = groups.get(path.refs[1].checked_sub(1)?)?;
            let depth = iter_group
                .records
                .iter()
                .find(|record| record.record_type == 0x090a)
                .map(|record| record.payload(&file.data))
                .filter(|payload| payload.len() >= 20)
                .map(|payload| read_u32(payload, 16) as usize)
                .unwrap_or(3);
            let depth_parameter_name = if let Some(iter_path) = find_indexed_path(file, iter_group)
            {
                if let Some(ordinal) = iter_path.refs.first().copied() {
                    if let Some(parameter_group_index) = ordinal.checked_sub(1) {
                        if let Some(parameter_group) = groups.get(parameter_group_index) {
                            editable_non_graph_parameter_name_for_group(
                                file,
                                groups,
                                parameter_group,
                            )
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            };

            Some(LabelIterationFamily::PointExpression {
                seed_label_index,
                point_seed_index,
                parameter_name,
                expr,
                depth,
                depth_parameter_name,
            })
        })
        .collect()
}

pub(super) fn bind_button_seed_expression_labels(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
    labels: &mut [TextLabel],
    label_group_to_index: &BTreeMap<usize, usize>,
    group_to_point_index: &[Option<usize>],
) {
    for seed_group in groups
        .iter()
        .filter(|group| (group.header.kind()) == crate::format::GroupKind::LabelIterationSeed)
    {
        let Some(seed_path) = find_indexed_path(file, seed_group) else {
            continue;
        };
        if seed_path.refs.len() < 2 {
            continue;
        }
        let Some(point_group_index) = seed_path.refs.first().and_then(|ordinal| ordinal.checked_sub(1)) else {
            continue;
        };
        let Some(point_index) = mapped_point_index(group_to_point_index, point_group_index) else {
            continue;
        };
        let Some(point_anchor) = anchors.get(point_group_index).cloned().flatten() else {
            continue;
        };
        let Some(button_group) = seed_path
            .refs
            .get(1)
            .and_then(|ordinal| groups.get(ordinal.saturating_sub(1)))
        else {
            continue;
        };
        if (button_group.header.kind()) != crate::format::GroupKind::ButtonLabel {
            continue;
        }
        let Some(label_index) = label_group_to_index.get(&button_group.ordinal).copied() else {
            continue;
        };
        let Some(expr_group) = find_indexed_path(file, button_group)
            .and_then(|path| path.refs.first().copied())
            .and_then(|ordinal| groups.get(ordinal.saturating_sub(1)))
        else {
            continue;
        };
        if (expr_group.header.kind()) != crate::format::GroupKind::FunctionExpr {
            continue;
        }
        let Some(expr) = try_decode_function_expr(file, groups, expr_group).ok() else {
            continue;
        };
        let Some((parameter_name, parameter_value)) = resolve_function_expr_parameter(
            file,
            groups,
            expr_group,
            anchors,
            &mut BTreeSet::new(),
        ) else {
            continue;
        };
        let value = evaluate_expr_with_parameters(
            &expr,
            0.0,
            &BTreeMap::from([(parameter_name.clone(), parameter_value)]),
        );
        let expr_label = payload_function_expr_label(
            file,
            groups,
            anchors,
            expr_group,
            &function_expr_label(expr.clone()),
            &mut BTreeSet::new(),
        );
        let Some(label) = labels.get_mut(label_index) else {
            continue;
        };
        let anchor_dx = 0.0;
        let anchor_dy = 0.0;
        let value_text = value
            .map(format_number)
            .unwrap_or_else(|| "未定义".to_string());
        label.anchor = point_anchor;
        label.text = format!("{expr_label} = {value_text}");
        label.rich_markup = build_expression_rich_markup(&expr_label, &value_text);
        label.binding = Some(TextLabelBinding::PointBoundExpressionValue {
            point_index,
            anchor_dx,
            anchor_dy,
            parameter_name,
            result_name: decode_label_name(file, expr_group),
            expr_label,
            expr,
        });
    }
}

pub(super) fn collect_iteration_tables(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
) -> Vec<IterationTable> {
    groups
        .iter()
        .filter(|group| {
            (group.header.kind()) == crate::format::GroupKind::IterationExpressionHelper
        })
        .filter_map(|group| {
            let path = find_indexed_path(file, group)?;
            if path.refs.len() < 2 {
                return None;
            }
            let iter_group = groups.get(path.refs[0].checked_sub(1)?)?;
            let expr_group = groups.get(path.refs[1].checked_sub(1)?)?;
            if expr_group.header.kind() != crate::format::GroupKind::FunctionExpr {
                return None;
            }
            let expr = try_decode_function_expr(file, groups, expr_group).ok()?;
            let (parameter_name, _) = resolve_function_expr_parameter(
                file,
                groups,
                expr_group,
                anchors,
                &mut BTreeSet::new(),
            )?;
            let expr_label = payload_function_expr_label(
                file,
                groups,
                anchors,
                expr_group,
                &function_expr_label(expr.clone()),
                &mut BTreeSet::new(),
            );
            let depth = iter_group
                .records
                .iter()
                .find(|record| record.record_type == 0x090a)
                .map(|record| record.payload(&file.data))
                .filter(|payload| payload.len() >= 20)
                .map(|payload| read_u32(payload, 16) as usize)
                .unwrap_or(3);
            let depth_parameter_name = if (iter_group.header.kind())
                == crate::format::GroupKind::RegularPolygonIteration
            {
                super::points::regular_polygon_iteration_step(file, groups, iter_group)
                    .map(|(_, _, parameter_name, _)| parameter_name)
            } else {
                None
            };
            Some(IterationTable {
                anchor: decode_iteration_table_anchor(file, group)?,
                expr_label,
                parameter_name,
                expr,
                depth,
                depth_parameter_name,
                visible: true,
            })
        })
        .collect()
}

fn segment_name(
    file: &GspFile,
    groups: &[ObjectGroup],
    segment_group: &ObjectGroup,
) -> Option<String> {
    let path = find_indexed_path(file, segment_group)?;
    let names = path
        .refs
        .iter()
        .map(|&object_ref| {
            let group = groups.get(object_ref.checked_sub(1)?)?;
            decode_label_name(file, group)
        })
        .collect::<Option<Vec<_>>>()?;
    (names.len() >= 2).then(|| names.join(""))
}

fn circle_name(
    file: &GspFile,
    groups: &[ObjectGroup],
    circle_group: &ObjectGroup,
) -> Option<String> {
    let path = find_indexed_path(file, circle_group)?;
    let names = path
        .refs
        .iter()
        .map(|&object_ref| {
            let group = groups.get(object_ref.checked_sub(1)?)?;
            decode_label_name(file, group)
        })
        .collect::<Option<Vec<_>>>()?;
    (names.len() >= 2).then(|| names.join(""))
}

pub(super) fn circle_parameter(
    anchors: &[Option<PointRecord>],
    center_group_index: usize,
    _radius_group_index: usize,
    unit_x: f64,
    unit_y: f64,
) -> Option<f64> {
    let _center = anchors.get(center_group_index)?.clone()?;
    let point_angle = unit_y.atan2(unit_x);
    let tau = std::f64::consts::TAU;
    Some(point_angle.rem_euclid(tau) / tau)
}

fn polygon_vertex_name(
    file: &GspFile,
    groups: &[ObjectGroup],
    polygon_group: &ObjectGroup,
) -> Option<String> {
    let path = find_indexed_path(file, polygon_group)?;
    let names = path
        .refs
        .iter()
        .map(|&object_ref| {
            let group = groups.get(object_ref.checked_sub(1)?)?;
            decode_label_name(file, group)
        })
        .collect::<Option<Vec<_>>>()?;
    (!names.is_empty()).then(|| names.join(""))
}

pub(super) fn polygon_boundary_parameter(
    anchors: &[Option<PointRecord>],
    vertex_group_indices: &[usize],
    edge_index: usize,
    t: f64,
) -> Option<f64> {
    if vertex_group_indices.len() < 2 {
        return None;
    }

    let vertices = vertex_group_indices
        .iter()
        .map(|group_index| anchors.get(*group_index)?.clone())
        .collect::<Option<Vec<_>>>()?;

    let mut perimeter = 0.0;
    let mut traveled = 0.0;
    for index in 0..vertices.len() {
        let start = &vertices[index];
        let end = &vertices[(index + 1) % vertices.len()];
        let length = ((end.x - start.x).powi(2) + (end.y - start.y).powi(2)).sqrt();
        perimeter += length;
        if index < edge_index % vertices.len() {
            traveled += length;
        } else if index == edge_index % vertices.len() {
            traveled += length * t.clamp(0.0, 1.0);
        }
    }

    (perimeter > 1e-9).then_some(traveled / perimeter)
}

pub(super) fn compute_iteration_labels(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
    circles: &[CircleShape],
) -> Vec<TextLabel> {
    let mut labels = Vec::new();

    let has_iteration = groups
        .iter()
        .any(|group| (group.header.kind()) == crate::format::GroupKind::RegularPolygonIteration);
    if !has_iteration {
        return labels;
    }

    let Some(circle) = circles.first() else {
        return labels;
    };
    let cx = circle.center.x;
    let cy = circle.center.y;
    let radius =
        ((circle.radius_point.x - cx).powi(2) + (circle.radius_point.y - cy).powi(2)).sqrt();

    let px_per_cm = groups
        .iter()
        .filter(|group| (group.header.kind()) == crate::format::GroupKind::PolarOffsetPoint)
        .find_map(|group| {
            let payload = group
                .records
                .iter()
                .find(|record| record.record_type == 0x07d3)
                .map(|record| record.payload(&file.data))?;
            (payload.len() >= 40).then(|| read_f64(payload, 32))
        })
        .filter(|v| v.is_finite() && *v > 1.0)
        .unwrap_or(37.79527559055118);

    let param_value = groups
        .iter()
        .filter(|group| (group.header.kind()) == crate::format::GroupKind::PolarOffsetPoint)
        .find_map(|group| {
            let payload = group
                .records
                .iter()
                .find(|record| record.record_type == 0x07d3)
                .map(|record| record.payload(&file.data))?;
            (payload.len() >= 20).then(|| read_f64(payload, 12))
        })
        .filter(|v| v.is_finite() && *v > 0.0)
        .unwrap_or(1.0);

    let t1 = param_value;
    let side = t1 / 2.0 * px_per_cm;
    let sqrt3 = 3.0_f64.sqrt();
    let diameter = 2.0 * radius;
    let m1 = diameter / (2.0 * side) + 0.5;
    let l_val = m1.floor() + 1.0;
    let m2 = diameter / (sqrt3 * side);
    let h_val = m2.ceil();
    let m3 = m2 - m1;
    let m4 = m3 - m3.floor();

    fn format_sub(raw: &str) -> String {
        raw.replace("[1]", "\u{2081}")
            .replace("[2]", "\u{2082}")
            .replace("[3]", "\u{2083}")
            .replace("[4]", "\u{2084}")
    }

    let mut computed_values = BTreeMap::<String, f64>::new();
    computed_values.insert("m\u{2081}".to_string(), m1);
    computed_values.insert("m\u{2082}".to_string(), m2);
    computed_values.insert("m\u{2083}".to_string(), m3);
    computed_values.insert("m\u{2084}".to_string(), m4);
    computed_values.insert("L".to_string(), l_val);
    computed_values.insert("H".to_string(), h_val);
    computed_values.insert("H\u{00b7}L".to_string(), h_val * l_val);

    for group in groups {
        if let Some(raw_name) = decode_label_name_raw(file, group) {
            let name = format_sub(&raw_name);
            if group
                .records
                .iter()
                .any(|record| record.record_type == 0x0907)
                && (group.header.kind()) == crate::format::GroupKind::Point
                && !computed_values.contains_key(&name)
            {
                computed_values.insert(name, t1);
            }
        }
    }

    for group in groups {
        let kind = group.header.kind();
        let has_0907 = group
            .records
            .iter()
            .any(|record| record.record_type == 0x0907);
        if !has_0907
            || !matches!(
                kind,
                crate::format::GroupKind::Point
                    | crate::format::GroupKind::ParameterAnchor
                    | crate::format::GroupKind::FunctionExpr
            )
        {
            continue;
        }
        if group
            .records
            .iter()
            .any(|record| record.record_type == 0x08fc)
        {
            continue;
        }
        let Some(anchor) = try_decode_0907_anchor(file, group).ok().flatten() else {
            continue;
        };

        let own_label = decode_label_name_raw(file, group).map(|s| format_sub(&s));
        let ref_labels: Vec<String> = find_indexed_path(file, group)
            .map(|path| {
                path.refs
                    .iter()
                    .filter_map(|&obj_ref| {
                        let ref_group = groups.get(obj_ref.checked_sub(1)?)?;
                        decode_label_name_raw(file, ref_group).map(|s| format_sub(&s))
                    })
                    .collect()
            })
            .unwrap_or_default();

        let mut lines = Vec::new();

        if let Some(name) = &own_label
            && let Some(value) =
                iteration_group_value(file, groups, group, anchors, &mut BTreeSet::new())
        {
            computed_values.entry(name.clone()).or_insert(value);
        }

        if kind == crate::format::GroupKind::FunctionExpr
            && let Some(expr) = try_decode_function_expr(file, groups, group).ok()
            && let Some(anchor_group) = find_indexed_path(file, group)
                .and_then(|path| groups.get(path.refs.first()?.checked_sub(1)?))
            && anchor_group.header.kind() == crate::format::GroupKind::ParameterAnchor
            && let Some(parameter_name) = decode_label_name(file, anchor_group)
            && let Some(path) = find_indexed_path(file, anchor_group)
            && let Some(point_index) = path.refs.first().and_then(|ordinal| ordinal.checked_sub(1))
            && let Some(value) =
                iteration_group_value(file, groups, group, anchors, &mut BTreeSet::new())
        {
            let expr_label = payload_function_expr_label(
                file,
                groups,
                anchors,
                group,
                &function_expr_label(expr.clone()),
                &mut BTreeSet::new(),
            );
            lines.push(format!("{expr_label} = {value:.2}"));
            labels.push(TextLabel {
                anchor,
                text: lines.join("\n"),
                rich_markup: None,
                color: [30, 30, 30, 255],
                visible: label_visible_for_group(file, group),
                binding: Some(TextLabelBinding::PolygonBoundaryExpression {
                    point_index,
                    parameter_name,
                    expr_label,
                    expr,
                }),
                screen_space: false,
                hotspots: Vec::new(),
            });
            continue;
        } else if kind == crate::format::GroupKind::Point {
            if let Some(name) = &own_label
                && let Some(&val) = computed_values.get(name.as_str())
            {
                let unit = "\u{5398}\u{7c73}";
                lines.push(format!("{name} = {val:.0} {unit}"));
                lines.push(format!("{name}/2 = {:.2} {unit}", val / 2.0));
            }
        } else {
            let has_h = ref_labels.iter().any(|n| n == "H");
            let has_l = ref_labels.iter().any(|n| n == "L");
            if own_label.is_none() && has_h && has_l {
                if let Some(val) = computed_values.get("H\u{00b7}L") {
                    lines.push(format!("H\u{00b7}L = {val:.2}"));
                }
            } else {
                let mut seen = BTreeSet::new();
                let mut try_add = |name: &str, lines: &mut Vec<String>| {
                    if seen.contains(name) {
                        return;
                    }
                    seen.insert(name.to_string());
                    if let Some(val) = computed_values.get(name) {
                        lines.push(format!("{name} = {val:.2}"));
                    }
                };

                if let Some(ol) = &own_label {
                    try_add(ol, &mut lines);
                }
                for rl in &ref_labels {
                    try_add(rl, &mut lines);
                }
            }

            if lines.is_empty()
                && let Some(ol) = &own_label
            {
                lines.push(ol.clone());
            }
        }

        if !lines.is_empty() {
            labels.push(TextLabel {
                anchor,
                text: lines.join("\n"),
                rich_markup: None,
                color: [30, 30, 30, 255],
                visible: label_visible_for_group(file, group),
                binding: None,
                screen_space: false,
                hotspots: Vec::new(),
            });
        }
    }

    labels
}
