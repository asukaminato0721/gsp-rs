use std::collections::{BTreeMap, BTreeSet};

use super::decode::{
    RichTextHotspotRef, decode_label_anchor, decode_label_name, decode_label_name_raw,
    decode_label_offset, decode_label_visible, decode_measurement_value, decode_text_anchor,
    find_indexed_path, is_action_button_group, try_decode_group_label_text,
    try_decode_group_rich_text, try_decode_link_button_url,
    try_decode_parameter_control_value_for_group, try_decode_payload_anchor_point,
};
use super::payload_debug_source;
use super::points::{
    RawPointConstraint, editable_non_graph_parameter_name_for_group,
    is_editable_non_graph_parameter_name, is_non_graph_parameter_group,
    is_non_graph_parameter_group_with_context, is_parametric_function_component_group_with_context,
    is_standalone_function_definition_group, is_standalone_function_definition_group_with_context,
    parametric_function_component_slot_with_context, regular_polygon_angle_expr_for_calc_group,
    try_decode_point_constraint,
};
use crate::format::{GspFile, ObjectGroup, PointRecord, read_f64, read_u32};
use crate::runtime::DEFAULT_GRAPH_RAW_PER_UNIT;
use crate::runtime::extract::context::SceneContext;
use crate::runtime::extract::iteration_depth::decode_iteration_depth_expr;
use crate::runtime::functions::{
    FunctionExpr, evaluate_expr_with_parameters, function_expr_label, try_decode_function_expr,
};
use crate::runtime::geometry::{color_from_style, format_number};
use crate::runtime::payload_consts::{
    EXPR_OP_ADD, EXPR_OP_DIV, EXPR_OP_MUL, EXPR_OP_POW, EXPR_OP_SUB, EXPR_PARAMETER_MASK,
    EXPR_PARAMETER_PREFIX, EXPR_PI_WORD, EXPR_VARIABLE_WORD, FUNCTION_EXPR_MARKER_A,
    FUNCTION_EXPR_MARKER_B, RECORD_BINDING_PAYLOAD, RECORD_FUNCTION_EXPR_PAYLOAD,
    RECORD_POINT_F64_PAIR,
};
use crate::runtime::scene::{
    IterationTable, LabelIterationFamily, RichTextExpressionRef, ScenePoint, ScreenPoint,
    TextLabel, TextLabelBinding, TextLabelHotspot, TextLabelHotspotAction,
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

#[derive(Clone, Copy)]
pub(super) struct HotspotIndexLookups<'a> {
    pub(super) group_to_point_index: &'a [Option<usize>],
    pub(super) circle_group_to_index: &'a [Option<usize>],
    pub(super) polygon_group_to_index: &'a [Option<usize>],
    pub(super) button_group_to_index: &'a BTreeMap<usize, usize>,
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
            | crate::format::GroupKind::LegacyCoordinateConstructPoint
            | crate::format::GroupKind::FixedCoordinatePoint
            | crate::format::GroupKind::CustomTransformPoint
            | crate::format::GroupKind::Translation
            | crate::format::GroupKind::Reflection
            | crate::format::GroupKind::Rotation
            | crate::format::GroupKind::ExpressionRotation
            | crate::format::GroupKind::ParameterRotation
            | crate::format::GroupKind::ParameterControlledPoint
            | crate::format::GroupKind::Scale
            | crate::format::GroupKind::PointConstraint
            | crate::format::GroupKind::PathPoint
            | crate::format::GroupKind::LinearIntersectionPoint
            | crate::format::GroupKind::IntersectionPoint1
            | crate::format::GroupKind::IntersectionPoint2
            | crate::format::GroupKind::CircleCircleIntersectionPoint1
            | crate::format::GroupKind::CircleCircleIntersectionPoint2
            | crate::format::GroupKind::BoundaryLengthValue
            | crate::format::GroupKind::ArcAngleValue
            | crate::format::GroupKind::BoundaryCurveLengthValue
            | crate::format::GroupKind::AngleValue
            | crate::format::GroupKind::GraphSlopeValue
            | crate::format::GroupKind::CoordinateReadoutLabel
            | crate::format::GroupKind::RatioValue
            | crate::format::GroupKind::IterationPointAlias
            | crate::format::GroupKind::NamedAlias
            | crate::format::GroupKind::Segment
            | crate::format::GroupKind::Ray
            | crate::format::GroupKind::GraphObject40
            | crate::format::GroupKind::Kind51
            | crate::format::GroupKind::AngleMarker
            | crate::format::GroupKind::LegacyAngleMarker
            | crate::format::GroupKind::ActionButton
            | crate::format::GroupKind::ButtonLabel
            | crate::format::GroupKind::RichTextLabel
            | crate::format::GroupKind::LabelIterationSeed
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

    if group.header.kind() == crate::format::GroupKind::Point
        && !group
            .records
            .iter()
            .any(|record| record.record_type == RECORD_POINT_F64_PAIR)
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
    let rich_text = try_decode_group_rich_text(file, group);
    let text = rich_text
        .as_ref()
        .map(|content| content.text.clone())
        .or_else(|| try_decode_group_label_text(file, group))
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

fn rich_text_expression_binding(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
    text: &str,
    rich_markup: &Option<String>,
    hotspots: &[RichTextHotspotRef],
) -> Option<TextLabelBinding> {
    let path = find_indexed_path(file, group)?;
    let mut seen_slots = BTreeSet::new();
    let refs = hotspots
        .iter()
        .filter_map(|hotspot| {
            if !seen_slots.insert(hotspot.path_slot) {
                return None;
            }
            let source_group_ordinal = path.refs.get(hotspot.path_slot.checked_sub(1)?)?;
            let source_group = groups.get(source_group_ordinal.checked_sub(1)?)?;
            if source_group.header.kind() != crate::format::GroupKind::FunctionExpr {
                return None;
            }
            Some(RichTextExpressionRef {
                source_group_ordinal: *source_group_ordinal,
                slot: hotspot.path_slot,
                line: hotspot.line,
                start: hotspot.start,
                end: hotspot.end,
                expr: try_decode_function_expr(file, groups, source_group).ok()?,
            })
        })
        .collect::<Vec<_>>();
    (!refs.is_empty()).then(|| TextLabelBinding::RichTextExpressionValues {
        template_text: text.to_string(),
        template_rich_markup: rich_markup.clone(),
        refs,
    })
}

fn label_visible_for_group(file: &GspFile, group: &ObjectGroup) -> bool {
    if group.header.is_hidden() && group.header.kind() == crate::format::GroupKind::Rotation {
        return false;
    }
    decode_label_visible(file, group).unwrap_or(!group.header.is_hidden())
}

fn hidden_measurement_is_button_controlled(
    file: &GspFile,
    groups: &[ObjectGroup],
    ordinal: usize,
) -> bool {
    groups.iter().any(|group| {
        group.header.kind() == crate::format::GroupKind::ActionButton
            && find_indexed_path(file, group)
                .is_some_and(|path| path.refs.iter().copied().any(|entry| entry == ordinal))
    })
}

fn ratio_value_label_visible(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
    helper_visible: bool,
) -> bool {
    if group.header.is_hidden()
        && hidden_measurement_is_button_controlled(file, groups, group.ordinal)
    {
        return false;
    }
    helper_visible || group.header.kind() == crate::format::GroupKind::RatioValue
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
            return resolve_function_expr_parameter(
                file,
                groups,
                parameter_group,
                anchors,
                visiting,
            );
        }
        if let Some((name, value)) =
            numeric_helper_function_parameter(file, groups, parameter_group)
            && !matches!(
                parameter_group.header.kind(),
                crate::format::GroupKind::ParameterAnchor | crate::format::GroupKind::RatioValue
            )
        {
            return Some((name, value));
        }
        let name = if parameter_group.header.kind() == crate::format::GroupKind::ParameterAnchor {
            let anchor_path = find_indexed_path(file, parameter_group)?;
            let point_group = groups.get(anchor_path.refs.first()?.checked_sub(1)?)?;
            decode_label_name(file, parameter_group)
                .or_else(|| decode_label_name(file, point_group))?
        } else if parameter_group.header.kind() == crate::format::GroupKind::RatioValue {
            decode_label_name(file, parameter_group)?
        } else {
            editable_non_graph_parameter_name_for_group(file, groups, parameter_group)?
        };
        let value = if parameter_group.header.kind() == crate::format::GroupKind::ParameterAnchor {
            parameter_anchor_value(file, groups, parameter_group, anchors)?
        } else if parameter_group.header.kind() == crate::format::GroupKind::RatioValue {
            ratio_value(file, parameter_group, anchors)?
        } else {
            try_decode_parameter_control_value_for_group(file, groups, parameter_group).ok()?
        };
        Some((name, value))
    })();
    visiting.remove(&group.ordinal);
    result
}

fn numeric_helper_function_parameter(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
) -> Option<(String, f64)> {
    let expr = try_decode_function_expr(file, groups, group).ok()?;
    let value = snap_near_integer(evaluate_expr_with_parameters(&expr, 0.0, &BTreeMap::new())?);
    if !value.is_finite() {
        return None;
    }
    let name = decode_label_name(file, group).unwrap_or_else(|| function_expr_label(expr));
    Some((name, value))
}

fn numeric_helper_axis_binding(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
    name: &str,
) -> Option<TextLabelBinding> {
    let path = find_indexed_path(file, group)?;
    let source_group = groups.get(path.refs.first()?.checked_sub(1)?)?;
    let source_path = find_indexed_path(file, source_group)?;
    let point_index = source_path.refs.first()?.checked_sub(1)?;
    let (origin_index, x_unit_index, y_unit_index) = source_path
        .refs
        .get(1)
        .and_then(|coord_sys_ordinal| {
            coordinate_system_point_group_indices(file, groups, *coord_sys_ordinal)
        })
        .unwrap_or((None, None, None));
    let axis = match source_group.header.kind() {
        crate::format::GroupKind::CoordinateXValue => {
            crate::runtime::scene::CoordinateAxis::Horizontal
        }
        crate::format::GroupKind::CoordinateYValue => {
            crate::runtime::scene::CoordinateAxis::Vertical
        }
        crate::format::GroupKind::FunctionExpr => {
            return numeric_helper_axis_binding(file, groups, source_group, name);
        }
        _ => return None,
    };
    Some(TextLabelBinding::PointAxisValue {
        point_index,
        name: name.to_string(),
        axis,
        origin_index,
        x_unit_index,
        y_unit_index,
    })
}

fn snap_near_integer(value: f64) -> f64 {
    let rounded = value.round();
    if (value - rounded).abs() < 1e-9 {
        rounded
    } else {
        value
    }
}

fn direct_function_expr_parameter_name(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
) -> Option<String> {
    let path = find_indexed_path(file, group)?;
    let parameter_group = groups.get(path.refs.first()?.checked_sub(1)?)?;
    if let Some(name) = decode_label_name(file, parameter_group)
        .or_else(|| decode_label_name_raw(file, parameter_group))
    {
        return Some(name);
    }
    if parameter_group.header.kind() == crate::format::GroupKind::ParameterAnchor {
        let anchor_path = find_indexed_path(file, parameter_group)?;
        let point_group = groups.get(anchor_path.refs.first()?.checked_sub(1)?)?;
        return decode_label_name(file, point_group)
            .or_else(|| decode_label_name_raw(file, point_group));
    }
    editable_non_graph_parameter_name_for_group(file, groups, parameter_group)
}

fn iteration_depth_driver_name(
    file: &GspFile,
    groups: &[ObjectGroup],
    iter_group: &ObjectGroup,
) -> Option<String> {
    let iter_path = find_indexed_path(file, iter_group)?;
    let depth_group = groups.get(iter_path.refs.first()?.checked_sub(1)?)?;
    if depth_group.header.kind() == crate::format::GroupKind::FunctionExpr {
        return decode_label_name(file, depth_group)
            .or_else(|| decode_label_name_raw(file, depth_group))
            .or_else(|| direct_function_expr_parameter_name(file, groups, depth_group));
    }
    if depth_group.header.kind() == crate::format::GroupKind::ParameterAnchor {
        let anchor_path = find_indexed_path(file, depth_group)?;
        let point_group = groups.get(anchor_path.refs.first()?.checked_sub(1)?)?;
        return decode_label_name(file, point_group)
            .or_else(|| decode_label_name_raw(file, point_group));
    }
    editable_non_graph_parameter_name_for_group(file, groups, depth_group)
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
        RawPointConstraint::ConstructedLine { t, .. } => Some(t),
        RawPointConstraint::PolygonBoundary {
            edge_index,
            t,
            vertex_group_indices,
        } => polygon_boundary_parameter(anchors, &vertex_group_indices, edge_index, t),
        RawPointConstraint::TranslatedPolygonBoundary {
            edge_index,
            t,
            vertex_group_indices,
            ..
        } => polygon_boundary_parameter(anchors, &vertex_group_indices, edge_index, t),
        RawPointConstraint::Circle(constraint) => circle_parameter(
            anchors,
            constraint.center_group_index,
            constraint.radius_group_index,
            constraint.unit_x,
            constraint.unit_y,
        ),
        RawPointConstraint::Circular(_) => None,
        RawPointConstraint::CircleArc(_) => None,
        RawPointConstraint::Arc(_) => None,
        RawPointConstraint::Polyline {
            points,
            segment_index,
            t,
            ..
        } => polyline_parameter(points.len(), segment_index, t),
    }
}

fn ratio_value(
    file: &GspFile,
    group: &ObjectGroup,
    anchors: &[Option<PointRecord>],
) -> Option<f64> {
    if group.header.kind() != crate::format::GroupKind::RatioValue {
        return None;
    }
    let path = find_indexed_path(file, group)?;
    let origin = anchors.get(path.refs.first()?.checked_sub(1)?)?.as_ref()?;
    let denominator = anchors.get(path.refs.get(1)?.checked_sub(1)?)?.as_ref()?;
    let numerator = anchors.get(path.refs.get(2)?.checked_sub(1)?)?.as_ref()?;
    let denominator_length = (denominator.x - origin.x).hypot(denominator.y - origin.y);
    if denominator_length <= 1e-9 {
        return None;
    }
    let numerator_length = (numerator.x - origin.x).hypot(numerator.y - origin.y);
    Some(numerator_length / denominator_length)
}

fn segment_projection_parameter(
    point: &PointRecord,
    start: &PointRecord,
    end: &PointRecord,
) -> Option<f64> {
    let dx = end.x - start.x;
    let dy = end.y - start.y;
    let len_sq = dx * dx + dy * dy;
    if len_sq <= 1e-9 {
        return None;
    }
    let t = ((point.x - start.x) * dx + (point.y - start.y) * dy) / len_sq;
    Some(t.clamp(0.0, 1.0))
}

fn detect_graph_context(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
) -> Option<(PointRecord, f64)> {
    let raw_per_unit = groups
        .iter()
        .filter(|group| group.header.kind().is_graph_calibration())
        .find_map(|group| {
            let record = group
                .records
                .iter()
                .find(|record| record.record_type == 0x07d3 && record.length == 12)?;
            decode_measurement_value(record.payload(&file.data))
        })?;
    let origin_raw = groups.iter().find_map(|group| {
        if !group.header.kind().is_graph_calibration() {
            return None;
        }
        let path = find_indexed_path(file, group)?;
        path.refs
            .iter()
            .find_map(|object_ref| anchors.get(object_ref.saturating_sub(1)).cloned().flatten())
    })?;
    Some((origin_raw, raw_per_unit))
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
        if let Some(display_label) = expanded_fallback_expr_label(
            file,
            groups,
            anchors,
            group,
            fallback_expr_label,
            visiting,
        ) {
            return Some(display_label);
        }
        if let Some(display_label) =
            payload_function_expr_display_label(file, groups, anchors, group, visiting)
        {
            return Some(display_label);
        }
        if let Some(label) = decode_label_name(file, group) {
            return Some(label);
        }
        let path = find_indexed_path(file, group)?;
        let source_group = groups.get(path.refs.first()?.checked_sub(1)?)?;
        let source_label = if source_group.header.kind() == crate::format::GroupKind::FunctionExpr {
            payload_function_expr_label(
                file,
                groups,
                anchors,
                source_group,
                fallback_expr_label,
                visiting,
            )
        } else {
            parameter_anchor_display_label(file, groups, source_group)
                .or_else(|| decode_label_name(file, source_group))
                .or_else(|| {
                    if source_group.header.kind() == crate::format::GroupKind::ParameterAnchor {
                        let anchor_path = find_indexed_path(file, source_group)?;
                        let point_group = groups.get(anchor_path.refs.first()?.checked_sub(1)?)?;
                        decode_label_name(file, point_group)
                    } else {
                        None
                    }
                })?
        };
        let (parameter_name, _) =
            resolve_function_expr_parameter(file, groups, group, anchors, &mut BTreeSet::new())?;
        Some(fallback_expr_label.replace(&parameter_name, &source_label))
    })()
    .unwrap_or_else(|| fallback_expr_label.to_string());
    visiting.remove(&group.ordinal);
    trim_decimal_literal_zeros(&result)
}

fn trim_decimal_literal_zeros(text: &str) -> String {
    let mut output = String::with_capacity(text.len());
    let mut chars = text.char_indices().peekable();
    while let Some((start, ch)) = chars.next() {
        if !ch.is_ascii_digit() {
            output.push(ch);
            continue;
        }
        let mut end = start + ch.len_utf8();
        let mut number = String::from(ch);
        while let Some((next_index, next_ch)) = chars.peek().copied() {
            if next_ch.is_ascii_digit() || next_ch == '.' {
                chars.next();
                end = next_index + next_ch.len_utf8();
                number.push(next_ch);
            } else {
                break;
            }
        }
        if number.contains('.') {
            while number.ends_with('0') {
                number.pop();
            }
            if number.ends_with('.') {
                number.pop();
            }
        }
        output.push_str(&number);
        if end >= text.len() {
            break;
        }
    }
    output
}

fn expanded_fallback_expr_label(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
    group: &ObjectGroup,
    fallback_expr_label: &str,
    visiting: &mut BTreeSet<usize>,
) -> Option<String> {
    let path = find_indexed_path(file, group)?;
    let mut text = fallback_expr_label.to_string();
    let mut changed = false;
    for ordinal in path.refs.iter().copied() {
        let source_group = groups.get(ordinal.checked_sub(1)?)?;
        let source_name = if source_group.header.kind() == crate::format::GroupKind::ParameterAnchor
        {
            let anchor_path = find_indexed_path(file, source_group)?;
            let point_group = groups.get(anchor_path.refs.first()?.checked_sub(1)?)?;
            decode_label_name(file, source_group).or_else(|| decode_label_name(file, point_group))
        } else {
            decode_label_name(file, source_group)
        };
        let Some(source_name) = source_name else {
            continue;
        };
        let Some(display_name) =
            display_group_reference_label(file, groups, anchors, source_group, visiting)
        else {
            continue;
        };
        if display_name == source_name {
            continue;
        }
        let next = replace_expr_identifier(&text, &source_name, &display_name);
        changed |= next != text;
        text = next;
    }
    changed.then_some(text)
}

fn replace_expr_identifier(text: &str, needle: &str, replacement: &str) -> String {
    if needle.is_empty() {
        return text.to_string();
    }
    let mut output = String::with_capacity(text.len());
    let mut cursor = 0;
    while let Some(relative_index) = text[cursor..].find(needle) {
        let start = cursor + relative_index;
        let end = start + needle.len();
        let before = text[..start].chars().next_back();
        let after = text[end..].chars().next();
        output.push_str(&text[cursor..start]);
        if expr_identifier_boundary(before) && expr_identifier_boundary(after) {
            output.push_str(replacement);
        } else {
            output.push_str(needle);
        }
        cursor = end;
    }
    output.push_str(&text[cursor..]);
    output
}

fn expr_identifier_boundary(value: Option<char>) -> bool {
    match value {
        Some(ch) => !(ch.is_alphanumeric() || matches!(ch, '_' | '.' | '[' | ']' | '₀'..='₉')),
        None => true,
    }
}

#[derive(Clone)]
struct DisplayExprLabel {
    text: String,
    precedence: u8,
}

impl DisplayExprLabel {
    fn atom(text: String) -> Self {
        Self {
            text,
            precedence: 4,
        }
    }

    fn parenthesized_for(&self, parent_precedence: u8) -> String {
        if self.precedence < parent_precedence {
            format!("({})", self.text)
        } else {
            self.text.clone()
        }
    }
}

fn payload_function_expr_display_label(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
    group: &ObjectGroup,
    visiting: &mut BTreeSet<usize>,
) -> Option<String> {
    if group.header.kind() != crate::format::GroupKind::FunctionExpr {
        return None;
    }
    let payload = group
        .records
        .iter()
        .find(|record| record.record_type == RECORD_FUNCTION_EXPR_PAYLOAD)
        .map(|record| record.payload(&file.data))?;
    let words = payload
        .chunks_exact(2)
        .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
        .collect::<Vec<_>>();
    let start = function_expr_word_start(&words)?;
    payload_function_application_label(file, groups, anchors, group, &words[start..], visiting)
        .or_else(|| {
            postfix_display_label_from_words(
                file,
                groups,
                anchors,
                group,
                &words[start..],
                visiting,
            )
            .map(|label| label.text)
        })
        .or_else(|| {
            linear_display_label_from_words(file, groups, anchors, group, &words[start..], visiting)
                .map(|label| label.text)
        })
}

fn payload_function_expr_application_display_label(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
    group: &ObjectGroup,
    visiting: &mut BTreeSet<usize>,
) -> Option<String> {
    if group.header.kind() != crate::format::GroupKind::FunctionExpr {
        return None;
    }
    let payload = group
        .records
        .iter()
        .find(|record| record.record_type == RECORD_FUNCTION_EXPR_PAYLOAD)
        .map(|record| record.payload(&file.data))?;
    let words = payload
        .chunks_exact(2)
        .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
        .collect::<Vec<_>>();
    let start = function_expr_word_start(&words)?;
    payload_function_application_label(file, groups, anchors, group, &words[start..], visiting)
}

fn function_expr_word_start(words: &[u16]) -> Option<usize> {
    if let Some(start) = words
        .windows(7)
        .enumerate()
        .find_map(|(index, window)| {
            (window[0] == RECORD_FUNCTION_EXPR_PAYLOAD as u16
                && window[1] == 0
                && window[4] == crate::format::GroupKind::FunctionExpr.raw()
                && window[5] == 0)
                .then_some(index + 6)
        })
        .and_then(|count_index| {
            let word_count = usize::from(*words.get(count_index)?);
            let start = words.len().checked_sub(word_count)?;
            (word_count > 0 && start > count_index && start < words.len()).then_some(start)
        })
    {
        return Some(start);
    }
    words
        .windows(2)
        .position(|pair| *pair == FUNCTION_EXPR_MARKER_A || *pair == FUNCTION_EXPR_MARKER_B)
        .map(|marker_index| marker_index + 2)
        .or_else(|| (!words.is_empty()).then_some(0))
}

const EXPR_FUNCTION_REF_MASK: u16 = 0xfff0;
const EXPR_FUNCTION_REF_PREFIX: u16 = 0x7000;

fn payload_function_application_label(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
    group: &ObjectGroup,
    words: &[u16],
    visiting: &mut BTreeSet<usize>,
) -> Option<String> {
    let (application_offset, application_word) = words
        .iter()
        .copied()
        .enumerate()
        .find(|(_, word)| (*word & EXPR_FUNCTION_REF_MASK) == EXPR_FUNCTION_REF_PREFIX)?;
    let helper_index = usize::from(application_word & 0x000f);
    let path = find_indexed_path(file, group)?;
    let helper_group = groups.get(path.refs.get(helper_index)?.checked_sub(1)?)?;
    let helper_name = decode_label_name(file, helper_group).unwrap_or_else(|| "f".to_string());
    let arg_text = path
        .refs
        .iter()
        .enumerate()
        .find_map(|(index, ordinal)| {
            (index != helper_index).then(|| {
                let arg_group = groups.get(ordinal.checked_sub(1)?)?;
                display_group_reference_label(file, groups, anchors, arg_group, visiting)
            })?
        })
        .or_else(|| {
            postfix_display_label_from_words(
                file,
                groups,
                anchors,
                group,
                words.get(application_offset + 1..)?,
                visiting,
            )
            .map(|arg| arg.text)
        })?;
    Some(format!("{helper_name}({arg_text})"))
}

fn postfix_display_label_from_words(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
    group: &ObjectGroup,
    words: &[u16],
    visiting: &mut BTreeSet<usize>,
) -> Option<DisplayExprLabel> {
    let mut stack = Vec::<DisplayExprLabel>::new();
    let mut index = 0;
    while index < words.len() {
        let word = words[index];
        if matches!(word, 0x000b | 0x000c) {
            index += 1;
            continue;
        }
        if word == 0
            && matches!(
                words.get(index + 1).copied(),
                Some(EXPR_OP_ADD | EXPR_OP_SUB | EXPR_OP_MUL | EXPR_OP_DIV | EXPR_OP_POW)
            )
        {
            index += 1;
            continue;
        }
        match word {
            EXPR_OP_ADD | EXPR_OP_SUB | EXPR_OP_MUL | EXPR_OP_DIV | EXPR_OP_POW => {
                let rhs = stack.pop()?;
                let lhs = stack.pop()?;
                let (symbol, precedence) = match word {
                    EXPR_OP_ADD => (" + ", 1),
                    EXPR_OP_SUB => (" - ", 1),
                    EXPR_OP_MUL => ("*", 2),
                    EXPR_OP_DIV => (" / ", 2),
                    EXPR_OP_POW => ("^", 3),
                    _ => unreachable!(),
                };
                stack.push(DisplayExprLabel {
                    text: format!(
                        "{}{}{}",
                        lhs.parenthesized_for(precedence),
                        symbol,
                        rhs.parenthesized_for(precedence + usize::from(word == EXPR_OP_SUB) as u8)
                    ),
                    precedence,
                });
                index += 1;
            }
            EXPR_VARIABLE_WORD => {
                stack.push(DisplayExprLabel::atom("x".to_string()));
                index += 1;
            }
            EXPR_PI_WORD => {
                stack.push(DisplayExprLabel::atom("π".to_string()));
                index += 1;
            }
            _ if (word & EXPR_PARAMETER_MASK) == EXPR_PARAMETER_PREFIX => {
                let parameter_index = usize::from(word & 0x000f);
                stack.push(DisplayExprLabel::atom(display_parameter_label(
                    file,
                    groups,
                    anchors,
                    group,
                    parameter_index,
                    visiting,
                )?));
                index += 1;
            }
            _ if display_unary_function(word).is_some() => {
                let arg = stack.pop()?;
                let op = display_unary_function(word)?;
                stack.push(DisplayExprLabel::atom(format!(
                    "{op}({})",
                    arg.parenthesized_for(0)
                )));
                index += 1;
            }
            _ => {
                stack.push(DisplayExprLabel::atom(format_number(f64::from(word))));
                index += 1;
            }
        }
    }
    (stack.len() == 1).then(|| stack.pop().unwrap())
}

fn linear_display_label_from_words(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
    group: &ObjectGroup,
    words: &[u16],
    visiting: &mut BTreeSet<usize>,
) -> Option<DisplayExprLabel> {
    let mut index = 0;
    let mut lhs =
        display_operand_label(file, groups, anchors, group, *words.get(index)?, visiting)?;
    index += 1;
    while index < words.len() {
        let op_word = words[index];
        let (symbol, precedence) = match op_word {
            EXPR_OP_ADD => (" + ", 1),
            EXPR_OP_SUB => (" - ", 1),
            EXPR_OP_MUL => ("*", 2),
            EXPR_OP_DIV => (" / ", 2),
            EXPR_OP_POW => ("^", 3),
            _ => return None,
        };
        let rhs = display_operand_label(
            file,
            groups,
            anchors,
            group,
            *words.get(index + 1)?,
            visiting,
        )?;
        lhs = DisplayExprLabel {
            text: format!(
                "{}{}{}",
                lhs.parenthesized_for(precedence),
                symbol,
                rhs.parenthesized_for(precedence)
            ),
            precedence,
        };
        index += 2;
    }
    Some(lhs)
}

fn display_operand_label(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
    group: &ObjectGroup,
    word: u16,
    visiting: &mut BTreeSet<usize>,
) -> Option<DisplayExprLabel> {
    if (word & EXPR_PARAMETER_MASK) == EXPR_PARAMETER_PREFIX {
        return Some(DisplayExprLabel::atom(display_parameter_label(
            file,
            groups,
            anchors,
            group,
            usize::from(word & 0x000f),
            visiting,
        )?));
    }
    match word {
        EXPR_VARIABLE_WORD => Some(DisplayExprLabel::atom("x".to_string())),
        EXPR_PI_WORD => Some(DisplayExprLabel::atom("π".to_string())),
        _ => Some(DisplayExprLabel::atom(format_number(f64::from(word)))),
    }
}

fn display_unary_function(word: u16) -> Option<&'static str> {
    match word {
        0x2000 => Some("sin"),
        0x2001 => Some("cos"),
        0x2002 => Some("tan"),
        0x2006 => Some("abs"),
        0x2007 => Some("sqrt"),
        0x2008 => Some("ln"),
        0x2009 => Some("log"),
        0x200a => Some("sgn"),
        0x200b => Some("round"),
        0x200c => Some("trunc"),
        _ => None,
    }
}

fn display_parameter_label(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
    group: &ObjectGroup,
    parameter_index: usize,
    visiting: &mut BTreeSet<usize>,
) -> Option<String> {
    let path = find_indexed_path(file, group)?;
    let parameter_group = groups.get(path.refs.get(parameter_index)?.checked_sub(1)?)?;
    if parameter_group.header.kind() == crate::format::GroupKind::FunctionExpr
        && !visiting.contains(&parameter_group.ordinal)
    {
        if let Some(label) = payload_function_expr_application_display_label(
            file,
            groups,
            anchors,
            parameter_group,
            visiting,
        ) {
            return Some(label);
        }
        if let Some(label) = decode_label_name(file, parameter_group) {
            return Some(label);
        }
        if let Some(label) =
            payload_function_expr_display_label(file, groups, anchors, parameter_group, visiting)
        {
            return Some(label);
        }
    }
    display_group_reference_label(file, groups, anchors, parameter_group, visiting)
}

fn display_group_reference_label(
    file: &GspFile,
    groups: &[ObjectGroup],
    _anchors: &[Option<PointRecord>],
    parameter_group: &ObjectGroup,
    _visiting: &mut BTreeSet<usize>,
) -> Option<String> {
    parameter_anchor_display_label(file, groups, parameter_group)
        .or_else(|| decode_label_name(file, parameter_group))
        .or_else(|| {
            if parameter_group.header.kind() == crate::format::GroupKind::DistanceValue {
                let path = find_indexed_path(file, parameter_group)?;
                return Some(distance_value_label_name(file, groups, &path));
            }
            None
        })
        .or_else(|| {
            numeric_helper_function_parameter(file, groups, parameter_group).map(|(name, _)| name)
        })
        .or_else(|| {
            if parameter_group.header.kind() == crate::format::GroupKind::ParameterAnchor {
                let anchor_path = find_indexed_path(file, parameter_group)?;
                let point_group = groups.get(anchor_path.refs.first()?.checked_sub(1)?)?;
                decode_label_name(file, point_group)
            } else {
                None
            }
        })
        .or_else(|| {
            let expr = try_decode_function_expr(file, groups, parameter_group).ok()?;
            Some(function_expr_label(expr))
        })
}

fn parameter_anchor_display_label(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
) -> Option<String> {
    if group.header.kind() != crate::format::GroupKind::ParameterAnchor {
        return None;
    }
    let path = find_indexed_path(file, group)?;
    if path.refs.len() < 2 {
        return None;
    }
    let point_group = groups.get(path.refs[0].checked_sub(1)?)?;
    let host_group = groups.get(path.refs[1].checked_sub(1)?)?;
    if !point_group.header.kind().is_point_constraint() {
        return None;
    }
    let point_name =
        decode_label_name(file, group).or_else(|| decode_label_name(file, point_group))?;
    match host_group.header.kind() {
        crate::format::GroupKind::Polygon => {
            let polygon_name = polygon_vertex_name(file, groups, host_group)?;
            Some(format!("{point_name}在{polygon_name}上的值"))
        }
        kind if kind.is_line_like() => None,
        crate::format::GroupKind::PointTrace
        | crate::format::GroupKind::CoordinateTrace
        | crate::format::GroupKind::CustomTransformTrace => {
            let object_name = trace_object_name(file, groups, host_group)?;
            Some(format!("{point_name}在{object_name}上的值"))
        }
        kind if super::decode::is_circle_group_kind(kind) => {
            let circle_name = circle_name(file, groups, host_group)?;
            Some(format!("{point_name}在⊙{circle_name}上的值"))
        }
        _ => None,
    }
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

fn function_expr_screen_anchor(file: &GspFile, group: &ObjectGroup) -> Option<PointRecord> {
    let payload = group
        .records
        .iter()
        .find(|record| record.record_type == 0x0907)
        .map(|record| record.payload(&file.data))?;
    let anchor = decode_text_anchor(payload)?;
    let offset = decode_label_offset(file, group).unwrap_or((0.0, 0.0));
    Some(PointRecord {
        x: anchor.x + offset.0,
        y: anchor.y + offset.1,
    })
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
        let is_standalone_function_definition = kind == crate::format::GroupKind::Point
            && is_standalone_function_definition_group(file, groups, group);
        match kind {
            crate::format::GroupKind::Midpoint => {
                if let Some(label_text) =
                    resolve_label_text(file, group, decode_label_name(file, group))
                    && let Some(anchor) = decode_label_anchor(file, group, anchors)
                {
                    let ResolvedLabelText {
                        text,
                        rich_markup,
                        hotspots,
                    } = label_text;
                    let (text, rich_markup) = if is_standalone_function_definition {
                        if let (Some(name), Ok(expr)) = (
                            decode_label_name(file, group),
                            crate::runtime::functions::try_decode_standalone_function_expr(
                                file, groups, group,
                            ),
                        ) {
                            let text = format!("{name}(x) = {}", function_expr_label(expr));
                            let rich_markup = build_plain_text_rich_markup(&text);
                            (text, rich_markup)
                        } else {
                            (text, rich_markup)
                        }
                    } else {
                        (text, rich_markup)
                    };
                    let binding = angle_marker_measurement_binding(file, group, &text)
                        .or_else(|| coordinate_readout_binding(file, groups, group))
                        .or_else(|| {
                            rich_text_expression_binding(
                                file,
                                groups,
                                group,
                                &text,
                                &rich_markup,
                                &hotspots,
                            )
                        });
                    let visible = if is_standalone_function_definition {
                        true
                    } else {
                        label_visible_for_group(file, group)
                    };
                    let label_index = labels.len();
                    label_group_to_index.insert(group.ordinal, label_index);
                    labels.push(TextLabel {
                        anchor,
                        text,
                        rich_markup,
                        color: label_color_for_group(group),
                        visible,
                        binding,
                        screen_space: kind == crate::format::GroupKind::ButtonLabel,
                        hotspots: Vec::new(),
                        debug: Some(payload_debug_source(group)),
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
                        collect_label_iteration_seed_label(file, groups, group, anchors)
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
                            | crate::format::GroupKind::LegacyCoordinateConstructPoint
                            | crate::format::GroupKind::FixedCoordinatePoint
                            | crate::format::GroupKind::CustomTransformPoint
                            | crate::format::GroupKind::Translation
                            | crate::format::GroupKind::Reflection
                            | crate::format::GroupKind::Rotation
                            | crate::format::GroupKind::ExpressionRotation
                            | crate::format::GroupKind::ParameterRotation
                            | crate::format::GroupKind::ParameterControlledPoint
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
                            | crate::format::GroupKind::BoundaryLengthValue
                            | crate::format::GroupKind::ArcAngleValue
                            | crate::format::GroupKind::BoundaryCurveLengthValue
                            | crate::format::GroupKind::AngleValue
                            | crate::format::GroupKind::GraphSlopeValue
                            | crate::format::GroupKind::RatioValue
                            | crate::format::GroupKind::IterationPointAlias
                            | crate::format::GroupKind::NamedAlias
                            | crate::format::GroupKind::RichTextLabel
                    )
                    && !is_non_graph_parameter_group(file, groups, group))
                .then(|| decode_label_name(file, group))
                .flatten();
                if let Some(label_text) = resolve_label_text(file, group, fallback_text)
                    && let Some(anchor) = decode_label_anchor(file, group, anchors).or_else(|| {
                        if kind == crate::format::GroupKind::IterationPointAlias {
                            find_indexed_path(file, group).and_then(|path| {
                                path.refs.iter().find_map(|ordinal| {
                                    anchors.get(ordinal.saturating_sub(1)).cloned().flatten()
                                })
                            })
                        } else {
                            None
                        }
                    })
                {
                    let ResolvedLabelText {
                        text,
                        rich_markup,
                        hotspots,
                    } = label_text;
                    let (text, rich_markup) = if is_standalone_function_definition {
                        if let (Some(name), Ok(expr)) = (
                            decode_label_name(file, group),
                            crate::runtime::functions::try_decode_standalone_function_expr(
                                file, groups, group,
                            ),
                        ) {
                            let text = format!("{name}(x) = {}", function_expr_label(expr));
                            (text.clone(), build_plain_text_rich_markup(&text))
                        } else {
                            (text, rich_markup)
                        }
                    } else {
                        (text, rich_markup)
                    };
                    let binding = angle_marker_measurement_binding(file, group, &text)
                        .or_else(|| coordinate_readout_binding(file, groups, group))
                        .or_else(|| {
                            rich_text_expression_binding(
                                file,
                                groups,
                                group,
                                &text,
                                &rich_markup,
                                &hotspots,
                            )
                        });
                    let visible = if is_standalone_function_definition {
                        true
                    } else {
                        label_visible_for_group(file, group)
                    };
                    let label_index = labels.len();
                    label_group_to_index.insert(group.ordinal, label_index);
                    labels.push(TextLabel {
                        anchor,
                        text,
                        rich_markup,
                        color: label_color_for_group(group),
                        visible,
                        binding,
                        screen_space: kind == crate::format::GroupKind::ButtonLabel,
                        hotspots: Vec::new(),
                        debug: Some(payload_debug_source(group)),
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
            | crate::format::GroupKind::GraphCalibrationY
            | crate::format::GroupKind::GraphCalibrationYAlt => {
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
                    .or_else(|| decode_label_name(file, group));
                if let (Some(anchor), Some(text)) = (anchor, text) {
                    labels.push(TextLabel {
                        anchor,
                        text,
                        color: [60, 60, 60, 255],
                        visible: label_visible_for_group(file, group),
                        screen_space: false,
                        debug: Some(payload_debug_source(group)),
                        ..Default::default()
                    });
                }
            }
            _ => {}
        }
    }
    collect_label_iteration_output_labels(
        file,
        groups,
        anchors,
        &mut labels,
        &mut label_group_to_index,
    );
    labels.iter_mut().for_each(apply_fallback_rich_markup);
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

fn coordinate_readout_binding(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
) -> Option<TextLabelBinding> {
    if group.header.kind() != crate::format::GroupKind::CoordinateReadoutLabel {
        return None;
    }
    let path = find_indexed_path(file, group)?;
    let point_group = groups.get(path.refs.first()?.checked_sub(1)?)?;
    let point_name = decode_label_name(file, point_group).unwrap_or_else(|| "点".to_string());
    let (origin_index, x_unit_index, y_unit_index) = path
        .refs
        .get(1)
        .and_then(|axis_ordinal| coordinate_system_point_group_indices(file, groups, *axis_ordinal))
        .unwrap_or((None, None, None));
    Some(TextLabelBinding::PointCoordinateValue {
        point_index: path.refs.first()?.checked_sub(1)?,
        point_name,
        origin_index,
        x_unit_index,
        y_unit_index,
    })
}

fn coordinate_system_point_group_indices(
    file: &GspFile,
    groups: &[ObjectGroup],
    axis_ordinal: usize,
) -> Option<(Option<usize>, Option<usize>, Option<usize>)> {
    let axis_group = groups.get(axis_ordinal.checked_sub(1)?)?;
    if axis_group.header.kind() != crate::format::GroupKind::AxisLine {
        return None;
    }
    let axis_path = find_indexed_path(file, axis_group)?;
    let horizontal_group = axis_path
        .refs
        .first()
        .and_then(|ordinal| groups.get(ordinal.checked_sub(1)?));
    let vertical_group = axis_path
        .refs
        .get(1)
        .and_then(|ordinal| groups.get(ordinal.checked_sub(1)?));
    let horizontal_path = horizontal_group.and_then(|group| find_indexed_path(file, group));
    let vertical_path = vertical_group.and_then(|group| find_indexed_path(file, group));
    let origin_index = horizontal_path
        .as_ref()
        .and_then(|path| path.refs.first().copied())
        .or_else(|| {
            vertical_path
                .as_ref()
                .and_then(|path| path.refs.first().copied())
        })
        .and_then(|ordinal| ordinal.checked_sub(1));
    let x_unit_index = horizontal_path
        .as_ref()
        .and_then(|path| path.refs.get(1).copied())
        .and_then(|ordinal| ordinal.checked_sub(1));
    let y_unit_index = vertical_path
        .as_ref()
        .and_then(|path| path.refs.get(1).copied())
        .and_then(|ordinal| ordinal.checked_sub(1));
    Some((origin_index, x_unit_index, y_unit_index))
}

fn distance_value_label_name(
    file: &GspFile,
    groups: &[ObjectGroup],
    path: &crate::format::IndexedPathRecord,
) -> String {
    if let Some(name) = decode_label_name(
        file,
        groups
            .get(
                path.refs
                    .first()
                    .copied()
                    .unwrap_or_default()
                    .saturating_sub(1),
            )
            .unwrap_or(&groups[0]),
    ) && path.refs.len() == 1
    {
        return name;
    }
    if path.refs.len() >= 2 {
        let left = groups
            .get(path.refs[0].saturating_sub(1))
            .and_then(|group| decode_label_name(file, group))
            .unwrap_or_else(|| "P".to_string());
        let right = groups
            .get(path.refs[1].saturating_sub(1))
            .and_then(|group| decode_label_name(file, group))
            .unwrap_or_else(|| "Q".to_string());
        return format!("{left}{right}");
    }
    "距离".to_string()
}

fn measurement_screen_anchor(file: &GspFile, group: &ObjectGroup) -> Option<PointRecord> {
    group.records.iter().find_map(|record| {
        matches!(record.record_type, 0x0903 | 0x0907)
            .then(|| decode_text_anchor(record.payload(&file.data)))
            .flatten()
    })
}

fn point_label_or_default(
    file: &GspFile,
    groups: &[ObjectGroup],
    index: usize,
    default: &str,
) -> String {
    groups
        .get(index)
        .and_then(|group| decode_label_name(file, group))
        .unwrap_or_else(|| default.to_string())
}

fn measured_segment_points_and_name(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
) -> Option<(usize, usize, String)> {
    let path = find_indexed_path(file, group)?;
    let host_group = groups.get(path.refs.first()?.checked_sub(1)?)?;
    let host_path = find_indexed_path(file, host_group)?;
    if host_path.refs.len() != 2 {
        return None;
    }
    let left_index = host_path.refs[0].checked_sub(1)?;
    let right_index = host_path.refs[1].checked_sub(1)?;
    let left_name = point_label_or_default(file, groups, left_index, "P");
    let right_name = point_label_or_default(file, groups, right_index, "Q");
    Some((left_index, right_index, format!("{left_name}{right_name}")))
}

fn angle_value_label_name(
    file: &GspFile,
    groups: &[ObjectGroup],
    path: &crate::format::IndexedPathRecord,
) -> String {
    let start = point_label_or_default(file, groups, path.refs[0].saturating_sub(1), "P");
    let vertex = point_label_or_default(file, groups, path.refs[1].saturating_sub(1), "Q");
    let end = point_label_or_default(file, groups, path.refs[2].saturating_sub(1), "R");
    format!("∠{start}{vertex}{end}")
}

fn polygon_area_label_name(
    file: &GspFile,
    groups: &[ObjectGroup],
    path: &crate::format::IndexedPathRecord,
) -> String {
    let names = path
        .refs
        .iter()
        .enumerate()
        .map(|(index, ordinal)| {
            point_label_or_default(
                file,
                groups,
                ordinal.saturating_sub(1),
                &format!("P{index}"),
            )
        })
        .collect::<String>();
    if path.refs.len() == 3 {
        format!("△{names}的面积")
    } else {
        format!("{names}的面积")
    }
}

fn polygon_area_for_points(points: &[PointRecord]) -> Option<f64> {
    if points.len() < 3 {
        return None;
    }
    let twice_area = points
        .iter()
        .zip(points.iter().cycle().skip(1))
        .map(|(left, right)| left.x * right.y - right.x * left.y)
        .sum::<f64>();
    Some(twice_area.abs() * 0.5)
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
    if expr_label.matches(" / ").count() > 1 {
        return None;
    }
    if let Some((numerator, denominator)) = split_top_level(expr_label, " / ") {
        let numerator = strip_wrapping_parens(numerator);
        return Some(format!(
            "<H</<H{}><H{}>><Tx = {}>>",
            render_expression_rich_part(numerator),
            render_expression_rich_part(denominator),
            value_text,
        ));
    }
    Some(format!(
        "<H{}<Tx = {}>>",
        render_expression_rich_part(expr_label),
        value_text,
    ))
}

fn render_expression_rich_part(text: &str) -> String {
    let mut output = String::new();
    let mut rest = text;
    while let Some(index) = rest.find("√(") {
        output.push_str(&rich_text_node(&rest[..index]));
        let open_index = index + "√".len();
        let Some(close_index) = matching_close_paren(rest, open_index) else {
            output.push_str(&rich_text_node(&rest[index..]));
            return output;
        };
        let inner = strip_wrapping_parens(&rest[open_index + 1..close_index]);
        output.push_str("<R");
        output.push_str(&render_expression_rich_part(inner));
        output.push('>');
        rest = &rest[close_index + 1..];
    }
    output.push_str(&rich_text_node(rest));
    output
}

fn rich_text_node(text: &str) -> String {
    if text.is_empty() {
        return String::new();
    }
    format!(
        "<Tx{}>",
        text.replace('&', "＆")
            .replace('<', "＜")
            .replace('>', "＞")
            .replace('*', "\u{00b7}")
    )
}

fn matching_close_paren(text: &str, open_index: usize) -> Option<usize> {
    let mut depth = 0usize;
    for (offset, ch) in text[open_index..].char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(open_index + offset);
                }
            }
            _ => {}
        }
    }
    None
}

fn strip_wrapping_parens(text: &str) -> &str {
    let trimmed = text.trim();
    if !trimmed.starts_with('(') || !trimmed.ends_with(')') {
        return trimmed;
    }
    if matching_close_paren(trimmed, 0) == Some(trimmed.len() - 1) {
        &trimmed[1..trimmed.len() - 1]
    } else {
        trimmed
    }
}

fn build_plain_text_rich_markup(text: &str) -> Option<String> {
    let escaped = text
        .replace('&', "＆")
        .replace('<', "＜")
        .replace('>', "＞")
        .replace('*', "\u{00b7}");
    (!escaped.is_empty()).then(|| format!("<H<Tx{}>>", escaped))
}

fn apply_fallback_rich_markup(label: &mut TextLabel) {
    if label.rich_markup.is_none() {
        label.rich_markup = build_plain_text_rich_markup(&label.text);
    }
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
    context: &SceneContext<'_>,
    anchors: &[Option<PointRecord>],
) -> Vec<TextLabel> {
    let mut labels = Vec::new();
    let graph = detect_graph_context(file, groups, anchors);
    for group in groups {
        let kind = group.header.kind();
        let helper_visible = label_visible_for_group(file, group);
        let is_standalone_function_definition = kind == crate::format::GroupKind::Point
            && is_standalone_function_definition_group_with_context(file, context, group);
        let is_non_graph_parameter = kind == crate::format::GroupKind::Point
            && is_non_graph_parameter_group_with_context(file, context, group);
        let is_parametric_function_component = kind == crate::format::GroupKind::Point
            && is_parametric_function_component_group_with_context(context, group.ordinal)
            && !is_standalone_function_definition;
        if (is_non_graph_parameter || is_parametric_function_component)
            && let Some(name) = decode_label_name(file, group)
            && let Some(value) =
                try_decode_parameter_control_value_for_group(file, groups, group).ok()
            && let Some(anchor) = try_decode_payload_anchor_point(file, group).ok().flatten()
        {
            let binding = if is_non_graph_parameter && is_editable_non_graph_parameter_name(&name) {
                Some(TextLabelBinding::ParameterValue { name: name.clone() })
            } else {
                None
            };
            labels.push(TextLabel {
                anchor,
                text: format!("{name} = {}", format_number(value)),
                color: [30, 30, 30, 255],
                visible: if is_parametric_function_component {
                    parametric_function_component_slot_with_context(context, group.ordinal)
                        == Some(1)
                } else {
                    helper_visible
                },
                binding,
                screen_space: true,
                debug: Some(payload_debug_source(group)),
                ..Default::default()
            });
        } else if kind == crate::format::GroupKind::DistanceValue
            && let Some(path) = find_indexed_path(file, group)
            && path.refs.len() >= 2
            && let (Some(left), Some(right)) = (
                anchors
                    .get(path.refs[0].saturating_sub(1))
                    .cloned()
                    .flatten(),
                anchors
                    .get(path.refs[1].saturating_sub(1))
                    .cloned()
                    .flatten(),
            )
        {
            let name = decode_label_name(file, group)
                .unwrap_or_else(|| distance_value_label_name(file, groups, &path));
            let raw_value = ((right.x - left.x).powi(2) + (right.y - left.y).powi(2)).sqrt();
            let value = if let Some((_, raw_per_unit)) = graph.as_ref() {
                raw_value / raw_per_unit
            } else {
                raw_value
            };
            labels.push(TextLabel {
                anchor: decode_label_anchor(file, group, anchors).unwrap_or(PointRecord {
                    x: (left.x + right.x) * 0.5,
                    y: (left.y + right.y) * 0.5,
                }),
                text: format!("{name} = {} 厘米", format_number(value)),
                color: [30, 30, 30, 255],
                visible: helper_visible,
                binding: Some(TextLabelBinding::PointDistanceValue {
                    left_index: path.refs[0].saturating_sub(1),
                    right_index: path.refs[1].saturating_sub(1),
                    name,
                    value_scale: graph
                        .as_ref()
                        .map(|(_, raw_per_unit)| 1.0 / raw_per_unit)
                        .unwrap_or(1.0),
                    value_suffix: " 厘米".to_string(),
                }),
                screen_space: true,
                debug: Some(payload_debug_source(group)),
                ..Default::default()
            });
        } else if kind == crate::format::GroupKind::MeasuredValue
            && let Some((left_index, right_index, name)) =
                measured_segment_points_and_name(file, groups, group)
            && let (Some(left), Some(right)) = (
                anchors.get(left_index).cloned().flatten(),
                anchors.get(right_index).cloned().flatten(),
            )
        {
            let value = ((right.x - left.x).powi(2) + (right.y - left.y).powi(2)).sqrt()
                / DEFAULT_GRAPH_RAW_PER_UNIT;
            labels.push(TextLabel {
                anchor: measurement_screen_anchor(file, group).unwrap_or(PointRecord {
                    x: (left.x + right.x) * 0.5,
                    y: (left.y + right.y) * 0.5,
                }),
                text: format!("{name} = {} 厘米", format_number(value)),
                color: [30, 30, 30, 255],
                visible: helper_visible,
                binding: Some(TextLabelBinding::PointDistanceValue {
                    left_index,
                    right_index,
                    name,
                    value_scale: 1.0 / DEFAULT_GRAPH_RAW_PER_UNIT,
                    value_suffix: " 厘米".to_string(),
                }),
                screen_space: true,
                debug: Some(payload_debug_source(group)),
                ..Default::default()
            });
        } else if kind == crate::format::GroupKind::AngleValue
            && let Some(path) = find_indexed_path(file, group)
            && path.refs.len() >= 3
            && let (Some(start), Some(vertex), Some(end)) = (
                anchors
                    .get(path.refs[0].saturating_sub(1))
                    .cloned()
                    .flatten(),
                anchors
                    .get(path.refs[1].saturating_sub(1))
                    .cloned()
                    .flatten(),
                anchors
                    .get(path.refs[2].saturating_sub(1))
                    .cloned()
                    .flatten(),
            )
        {
            let name = angle_value_label_name(file, groups, &path);
            let value = crate::runtime::geometry::angle_degrees_from_points(&start, &vertex, &end)
                .unwrap_or(0.0);
            labels.push(TextLabel {
                anchor: measurement_screen_anchor(file, group).unwrap_or(vertex),
                text: format!("{name} = {value:.2}°"),
                color: [30, 30, 30, 255],
                visible: helper_visible,
                binding: Some(TextLabelBinding::PointAngleValue {
                    start_index: path.refs[0].saturating_sub(1),
                    vertex_index: path.refs[1].saturating_sub(1),
                    end_index: path.refs[2].saturating_sub(1),
                    name,
                    value_suffix: "°".to_string(),
                }),
                screen_space: true,
                debug: Some(payload_debug_source(group)),
                ..Default::default()
            });
        } else if kind == crate::format::GroupKind::PolygonAreaValue
            && let Some(path) = find_indexed_path(file, group)
            && let Some(polygon_group) = path
                .refs
                .first()
                .and_then(|ordinal| groups.get(ordinal.saturating_sub(1)))
            && let Some(polygon_path) = find_indexed_path(file, polygon_group)
        {
            let point_indices = polygon_path
                .refs
                .iter()
                .map(|ordinal| ordinal.saturating_sub(1))
                .collect::<Vec<_>>();
            let points = point_indices
                .iter()
                .filter_map(|index| anchors.get(*index).cloned().flatten())
                .collect::<Vec<_>>();
            let name = polygon_area_label_name(file, groups, &polygon_path);
            let value = polygon_area_for_points(&points)
                .map(|area| area / (DEFAULT_GRAPH_RAW_PER_UNIT * DEFAULT_GRAPH_RAW_PER_UNIT))
                .unwrap_or(0.0);
            labels.push(TextLabel {
                anchor: measurement_screen_anchor(file, group)
                    .or_else(|| decode_label_anchor(file, group, anchors))
                    .unwrap_or(PointRecord { x: 0.0, y: 0.0 }),
                text: format!("{name} = {} 平方厘米", format_number(value)),
                color: [30, 30, 30, 255],
                visible: helper_visible,
                binding: Some(TextLabelBinding::PolygonAreaValue {
                    point_indices,
                    name,
                    value_scale: 1.0 / (DEFAULT_GRAPH_RAW_PER_UNIT * DEFAULT_GRAPH_RAW_PER_UNIT),
                    value_suffix: " 平方厘米".to_string(),
                }),
                screen_space: true,
                debug: Some(payload_debug_source(group)),
                ..Default::default()
            });
        } else if kind == crate::format::GroupKind::RatioValue
            && let Some(path) = find_indexed_path(file, group)
            && path.refs.len() >= 3
            && let Some(name) = decode_label_name(file, group)
            && let Some(value) = ratio_value(file, group, anchors)
            && let Some(anchor) = decode_label_anchor(file, group, anchors)
                .or_else(|| try_decode_payload_anchor_point(file, group).ok().flatten())
        {
            labels.push(TextLabel {
                anchor,
                text: format!("{name} = {}", format_number(value)),
                color: [30, 30, 30, 255],
                visible: ratio_value_label_visible(file, groups, group, helper_visible),
                binding: Some(TextLabelBinding::PointDistanceRatioValue {
                    origin_index: path.refs[0].saturating_sub(1),
                    denominator_index: path.refs[1].saturating_sub(1),
                    numerator_index: path.refs[2].saturating_sub(1),
                    name,
                }),
                screen_space: true,
                debug: Some(payload_debug_source(group)),
                ..Default::default()
            });
        } else if matches!(
            kind,
            crate::format::GroupKind::CoordinateXValue | crate::format::GroupKind::CoordinateYValue
        ) && let Some(path) = find_indexed_path(file, group)
            && let Some(point_index) = path
                .refs
                .first()
                .copied()
                .map(|value| value.saturating_sub(1))
            && let Some(point) = anchors.get(point_index).cloned().flatten()
        {
            let axis = if kind == crate::format::GroupKind::CoordinateYValue {
                crate::runtime::scene::CoordinateAxis::Vertical
            } else {
                crate::runtime::scene::CoordinateAxis::Horizontal
            };
            let name = decode_label_name(file, group).unwrap_or_else(|| {
                if axis == crate::runtime::scene::CoordinateAxis::Vertical {
                    "y".to_string()
                } else {
                    "x".to_string()
                }
            });
            let (origin_index, x_unit_index, y_unit_index) = path
                .refs
                .get(1)
                .and_then(|coord_sys_ordinal| {
                    coordinate_system_point_group_indices(file, groups, *coord_sys_ordinal)
                })
                .unwrap_or((None, None, None));
            let value = if axis == crate::runtime::scene::CoordinateAxis::Vertical {
                if let Some((origin, raw_per_unit)) = graph.as_ref() {
                    (origin.y - point.y) / raw_per_unit
                } else {
                    point.y
                }
            } else {
                if let Some((origin, raw_per_unit)) = graph.as_ref() {
                    (point.x - origin.x) / raw_per_unit
                } else {
                    point.x
                }
            };
            labels.push(TextLabel {
                anchor: decode_label_anchor(file, group, anchors).unwrap_or(point),
                text: format!("{name} = {}", format_number(value)),
                color: [30, 30, 30, 255],
                visible: helper_visible,
                binding: Some(TextLabelBinding::PointAxisValue {
                    point_index,
                    name,
                    axis,
                    origin_index,
                    x_unit_index,
                    y_unit_index,
                }),
                screen_space: false,
                debug: Some(payload_debug_source(group)),
                ..Default::default()
            });
        } else if kind == crate::format::GroupKind::FunctionExpr
            && let Some(override_expr) =
                regular_polygon_angle_expr_for_calc_group(file, groups, group)
                    .map(|(expr, parameter_name, parameter_value)| {
                        (
                            expr,
                            parameter_name,
                            parameter_value,
                            Some("regular-polygon-angle"),
                        )
                    })
                    .or_else(|| {
                        let expr = try_decode_function_expr(file, groups, group).ok()?;
                        let (parameter_name, parameter_value) = resolve_function_expr_parameter(
                            file,
                            groups,
                            group,
                            anchors,
                            &mut BTreeSet::new(),
                        )?;
                        let semantic_kind = parameter_name
                            .parse::<f64>()
                            .is_ok()
                            .then_some("numeric-helper");
                        Some((expr, parameter_name, parameter_value, semantic_kind))
                    })
                    .or_else(|| {
                        let expr = try_decode_function_expr(file, groups, group).ok()?;
                        Some((expr, String::new(), 0.0, Some("multi-parameter")))
                    })
            && let Some(anchor) = try_decode_payload_anchor_point(file, group)
                .ok()
                .flatten()
                .or_else(|| function_expr_screen_anchor(file, group))
        {
            let (expr, parameter_name, parameter_value, semantic_kind) = override_expr;
            let value = evaluate_expr_with_parameters(
                &expr,
                0.0,
                &BTreeMap::from([(parameter_name.clone(), parameter_value)]),
            );
            let expr_label = if semantic_kind == Some("regular-polygon-angle") {
                format!("360° / {parameter_name}")
            } else {
                payload_function_expr_label(
                    file,
                    groups,
                    anchors,
                    group,
                    &function_expr_label(expr.clone()),
                    &mut BTreeSet::new(),
                )
            };
            let binding = if semantic_kind == Some("numeric-helper") {
                numeric_helper_axis_binding(file, groups, group, &expr_label)
            } else {
                (matches!(
                    semantic_kind,
                    None | Some("regular-polygon-angle") | Some("multi-parameter")
                ) && (semantic_kind == Some("multi-parameter")
                    || is_editable_non_graph_parameter_name(&parameter_name)))
                .then(|| TextLabelBinding::ExpressionValue {
                    parameter_name: parameter_name.clone(),
                    result_name: decode_label_name(file, group),
                    expr_label: expr_label.clone(),
                    expr: expr.clone(),
                })
            };
            let value_text = value
                .map(|value| {
                    if semantic_kind == Some("regular-polygon-angle") {
                        format!("{value:.2}°")
                    } else {
                        format_number(value)
                    }
                })
                .unwrap_or_else(|| "未定义".to_string());
            let text = format!("{expr_label} = {value_text}");
            labels.push(TextLabel {
                anchor,
                text,
                rich_markup: build_expression_rich_markup(&expr_label, &value_text),
                color: [30, 30, 30, 255],
                visible: !group.header.is_hidden(),
                binding,
                screen_space: true,
                hotspots: Vec::new(),
                debug: Some(payload_debug_source(group)),
            });
        } else if matches!(
            kind,
            crate::format::GroupKind::RatioValue | crate::format::GroupKind::IterationPointAlias
        ) && let Some(label) =
            collect_legacy_expression_label(file, groups, anchors, group)
        {
            labels.push(label);
        }
    }
    labels.iter_mut().for_each(apply_fallback_rich_markup);
    labels
}

fn collect_legacy_expression_label(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
    group: &ObjectGroup,
) -> Option<TextLabel> {
    let path = find_indexed_path(file, group)?;
    let expr_group = path
        .refs
        .iter()
        .rev()
        .filter_map(|ordinal| groups.get(ordinal.checked_sub(1)?))
        .find(|candidate| {
            matches!(
                candidate.header.kind(),
                crate::format::GroupKind::FunctionExpr
                    | crate::format::GroupKind::DerivativeFunction
            )
        })?;
    let expr = try_decode_function_expr(file, groups, expr_group).ok()?;
    let (parameter_name, parameter_value) =
        resolve_function_expr_parameter(file, groups, expr_group, anchors, &mut BTreeSet::new())?;
    let expr_label = decode_label_name(file, group).unwrap_or_else(|| {
        payload_function_expr_label(
            file,
            groups,
            anchors,
            expr_group,
            &function_expr_label(expr.clone()),
            &mut BTreeSet::new(),
        )
    });
    let value_text = evaluate_expr_with_parameters(
        &expr,
        0.0,
        &BTreeMap::from([(parameter_name.clone(), parameter_value)]),
    )
    .map(format_number)
    .unwrap_or_else(|| "未定义".to_string());
    let anchor = decode_label_anchor(file, group, anchors)
        .or_else(|| {
            try_decode_payload_anchor_point(file, expr_group)
                .ok()
                .flatten()
        })
        .or_else(|| {
            path.refs
                .first()
                .and_then(|ordinal| anchors.get(ordinal.saturating_sub(1)))
                .cloned()
                .flatten()
        })?;
    Some(TextLabel {
        anchor,
        text: format!("{expr_label} = {value_text}"),
        rich_markup: build_expression_rich_markup(&expr_label, &value_text),
        color: [30, 30, 30, 255],
        visible: label_visible_for_group(file, group),
        binding: Some(TextLabelBinding::ExpressionValue {
            parameter_name,
            result_name: decode_label_name(file, group),
            expr_label,
            expr,
        }),
        screen_space: false,
        hotspots: Vec::new(),
        debug: Some(payload_debug_source(group)),
    })
}

pub(super) fn resolve_label_hotspots(
    file: &GspFile,
    groups: &[ObjectGroup],
    labels: &mut [TextLabel],
    pending_hotspots: &[PendingLabelHotspot],
    lookups: HotspotIndexLookups<'_>,
) {
    for pending in pending_hotspots {
        let Some(label) = labels.get_mut(pending.label_index) else {
            continue;
        };

        let Some(group) = groups.get(pending.group_ordinal.saturating_sub(1)) else {
            continue;
        };
        let action = match group.header.kind() {
            crate::format::GroupKind::ActionButton => lookups
                .button_group_to_index
                .get(&pending.group_ordinal)
                .copied()
                .map(|button_index| TextLabelHotspotAction::Button { button_index }),
            crate::format::GroupKind::ButtonLabel => (|| {
                let path = find_indexed_path(file, group)?;
                let ordinal = path.refs.first().copied()?;
                lookups
                    .button_group_to_index
                    .get(&ordinal)
                    .copied()
                    .map(|button_index| TextLabelHotspotAction::Button { button_index })
            })(),
            crate::format::GroupKind::Point => lookups
                .group_to_point_index
                .get(pending.group_ordinal.saturating_sub(1))
                .copied()
                .flatten()
                .map(|point_index| TextLabelHotspotAction::Point { point_index }),
            crate::format::GroupKind::Segment => (|| {
                let path = find_indexed_path(file, group)?;
                let start_point_index = mapped_point_index(
                    lookups.group_to_point_index,
                    path.refs.first()?.saturating_sub(1),
                )?;
                let end_point_index = mapped_point_index(
                    lookups.group_to_point_index,
                    path.refs.get(1)?.saturating_sub(1),
                )?;
                Some(TextLabelHotspotAction::Segment {
                    start_point_index,
                    end_point_index,
                })
            })(),
            crate::format::GroupKind::AngleMarker => (|| {
                let path = find_indexed_path(file, group)?;
                let start_point_index = mapped_point_index(
                    lookups.group_to_point_index,
                    path.refs.first()?.saturating_sub(1),
                )?;
                let vertex_point_index = mapped_point_index(
                    lookups.group_to_point_index,
                    path.refs.get(1)?.saturating_sub(1),
                )?;
                let end_point_index = mapped_point_index(
                    lookups.group_to_point_index,
                    path.refs.get(2)?.saturating_sub(1),
                )?;
                Some(TextLabelHotspotAction::AngleMarker {
                    start_point_index,
                    vertex_point_index,
                    end_point_index,
                })
            })(),
            kind if super::decode::is_circle_group_kind(kind) => lookups
                .circle_group_to_index
                .get(pending.group_ordinal.saturating_sub(1))
                .copied()
                .flatten()
                .map(|circle_index| TextLabelHotspotAction::Circle { circle_index }),
            crate::format::GroupKind::Polygon => lookups
                .polygon_group_to_index
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

fn collect_label_iteration_seed_label(
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
    let source_group = groups.get(path.refs[1].checked_sub(1)?)?;
    let anchor = decode_label_anchor(file, group, anchors)
        .or_else(|| label_iteration_seed_anchor(&path, anchors))?;
    if (source_group.header.kind()) == crate::format::GroupKind::FunctionExpr {
        return collect_point_expression_label_from_seed(
            file,
            groups,
            group,
            source_group,
            point_group_index,
            anchor,
            anchors,
        );
    }
    let ResolvedLabelText {
        text, rich_markup, ..
    } = resolve_label_text(
        file,
        source_group,
        try_decode_group_label_text(file, group)
            .or_else(|| decode_label_name(file, source_group))
            .or_else(|| decode_label_name(file, group)),
    )?;
    Some(TextLabel {
        anchor,
        text,
        rich_markup,
        color: label_color_for_group(group),
        visible: label_visible_for_group(file, group),
        screen_space: false,
        debug: Some(payload_debug_source(group)),
        ..Default::default()
    })
}

fn label_iteration_seed_anchor(
    path: &crate::format::IndexedPathRecord,
    anchors: &[Option<PointRecord>],
) -> Option<PointRecord> {
    let point_group_index = path.refs.first()?.checked_sub(1)?;
    anchors.get(point_group_index).cloned().flatten()
}

fn collect_point_expression_label_from_seed(
    file: &GspFile,
    groups: &[ObjectGroup],
    seed_group: &ObjectGroup,
    expr_group: &ObjectGroup,
    point_group_index: usize,
    anchor: PointRecord,
    anchors: &[Option<PointRecord>],
) -> Option<TextLabel> {
    let expr = try_decode_function_expr(file, groups, expr_group).ok()?;
    let (parameter_name, parameter_value) =
        resolve_function_expr_parameter(file, groups, expr_group, anchors, &mut BTreeSet::new())?;
    let value = evaluate_expr_with_parameters(
        &expr,
        0.0,
        &BTreeMap::from([(parameter_name.clone(), parameter_value)]),
    )?;
    let point_anchor = anchors
        .get(point_group_index)
        .cloned()
        .flatten()
        .unwrap_or(anchor.clone());
    Some(TextLabel {
        anchor: anchor.clone(),
        text: format_number(value),
        color: [30, 30, 30, 255],
        visible: label_visible_for_group(file, seed_group),
        binding: Some(TextLabelBinding::PointExpressionValue {
            point_index: point_group_index,
            anchor_dx: anchor.x - point_anchor.x,
            anchor_dy: anchor.y - point_anchor.y,
            anchor_y_point_index: None,
            anchor_y_dy: None,
            parameter_name,
            expr,
        }),
        screen_space: false,
        debug: Some(payload_debug_source(seed_group)),
        ..Default::default()
    })
}

fn collect_label_iteration_output_labels(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
    labels: &mut Vec<TextLabel>,
    label_group_to_index: &mut BTreeMap<usize, usize>,
) {
    for binding_group in groups
        .iter()
        .filter(|group| (group.header.kind()) == crate::format::GroupKind::IterationBinding)
    {
        let Some(path) = find_indexed_path(file, binding_group) else {
            continue;
        };
        let Some(seed_group) = path
            .refs
            .first()
            .and_then(|ordinal| groups.get(ordinal.saturating_sub(1)))
        else {
            continue;
        };
        if (seed_group.header.kind()) != crate::format::GroupKind::LabelIterationSeed {
            continue;
        }
        let Some(iter_group) = path
            .refs
            .get(1)
            .and_then(|ordinal| groups.get(ordinal.saturating_sub(1)))
        else {
            continue;
        };
        let Some(output_labels) = collect_label_iteration_output_labels_for_binding(
            file, groups, seed_group, iter_group, anchors,
        ) else {
            continue;
        };
        label_group_to_index.insert(binding_group.ordinal, labels.len());
        for label in output_labels {
            labels.push(TextLabel {
                visible: !binding_group.header.is_hidden(),
                debug: Some(payload_debug_source(binding_group)),
                ..label
            });
        }
    }
}

fn collect_label_iteration_output_labels_for_binding(
    file: &GspFile,
    groups: &[ObjectGroup],
    seed_group: &ObjectGroup,
    iter_group: &ObjectGroup,
    anchors: &[Option<PointRecord>],
) -> Option<Vec<TextLabel>> {
    let seed_path = find_indexed_path(file, seed_group)?;
    if seed_path.refs.len() < 2 {
        return None;
    }
    let source_group = groups.get(seed_path.refs[1].checked_sub(1)?)?;
    if (source_group.header.kind()) != crate::format::GroupKind::FunctionExpr {
        return None;
    }
    let anchor = decode_label_anchor(file, seed_group, anchors)
        .or_else(|| label_iteration_seed_anchor(&seed_path, anchors))?;
    let expr = try_decode_function_expr(file, groups, source_group).ok()?;
    let (parameter_name, parameter_value) = resolve_sequence_expression_state_parameter(
        file,
        groups,
        source_group,
        anchors,
    )
    .or_else(|| {
        resolve_function_expr_parameter(file, groups, source_group, anchors, &mut BTreeSet::new())
    })?;
    let depth = iter_group
        .records
        .iter()
        .find(|record| record.record_type == 0x090a)
        .map(|record| record.payload(&file.data))
        .filter(|payload| payload.len() >= 20)
        .map(|payload| read_u32(payload, 16) as usize)
        .unwrap_or(3);
    let depth_parameter_name = iteration_depth_driver_name(file, groups, iter_group);
    let Some(anchor_step) = label_iteration_anchor_step(file, groups, &seed_path, anchors) else {
        let text =
            evaluate_sequence_expression_value(&expr, &parameter_name, parameter_value, depth)
                .map(format_number)
                .or_else(|| try_decode_group_label_text(file, seed_group))
                .or_else(|| decode_label_name(file, seed_group))
                .unwrap_or_else(|| "未定义".to_string());
        return Some(vec![TextLabel {
            anchor,
            text,
            color: [30, 30, 30, 255],
            binding: Some(TextLabelBinding::SequenceExpressionValue {
                parameter_name,
                expr,
                depth,
                depth_parameter_name,
            }),
            screen_space: false,
            ..Default::default()
        }]);
    };

    let output_labels = (1..=depth)
        .filter_map(|step_index| {
            let text = evaluate_sequence_expression_value(
                &expr,
                &parameter_name,
                parameter_value,
                step_index,
            )
            .map(format_number)?;
            let delta = anchor_step.clone() * step_index as f64;
            Some(TextLabel {
                anchor: anchor.clone() + delta,
                text,
                color: [30, 30, 30, 255],
                binding: None,
                screen_space: false,
                ..Default::default()
            })
        })
        .collect::<Vec<_>>();
    (!output_labels.is_empty()).then_some(output_labels)
}

fn label_iteration_anchor_step(
    file: &GspFile,
    groups: &[ObjectGroup],
    seed_path: &crate::format::IndexedPathRecord,
    anchors: &[Option<PointRecord>],
) -> Option<PointRecord> {
    let point_group = groups.get(seed_path.refs.first()?.checked_sub(1)?)?;
    if point_group.header.kind() != crate::format::GroupKind::Translation {
        return None;
    }
    let path = find_indexed_path(file, point_group)?;
    if path.refs.len() < 3 {
        return None;
    }
    let start = anchors.get(path.refs[1].checked_sub(1)?)?.clone()?;
    let end = anchors.get(path.refs[2].checked_sub(1)?)?.clone()?;
    let step = end - start;
    ((step.x.abs() >= 1e-6) || (step.y.abs() >= 1e-6)).then_some(step)
}

fn resolve_sequence_expression_state_parameter(
    file: &GspFile,
    groups: &[ObjectGroup],
    source_group: &ObjectGroup,
    anchors: &[Option<PointRecord>],
) -> Option<(String, f64)> {
    let source_path = find_indexed_path(file, source_group)?;
    let calc_group = groups.get(source_path.refs.first()?.checked_sub(1)?)?;
    let calc_path = find_indexed_path(file, calc_group)?;
    let first_group = groups.get(calc_path.refs.first()?.checked_sub(1)?)?;
    if first_group.header.kind() != crate::format::GroupKind::FunctionExpr {
        return resolve_function_expr_parameter(
            file,
            groups,
            calc_group,
            anchors,
            &mut BTreeSet::new(),
        );
    }
    calc_path
        .refs
        .iter()
        .skip(1)
        .filter_map(|ordinal| groups.get(ordinal.saturating_sub(1)))
        .find(|group| group.header.kind() == crate::format::GroupKind::FunctionExpr)
        .and_then(|group| {
            resolve_function_expr_parameter(file, groups, group, anchors, &mut BTreeSet::new())
        })
}

fn evaluate_sequence_expression_value(
    expr: &crate::runtime::functions::FunctionExpr,
    parameter_name: &str,
    parameter_value: f64,
    depth: usize,
) -> Option<f64> {
    let mut current_value = parameter_value;
    let mut value = None;
    for _ in 0..=depth {
        let next_value = evaluate_expr_with_parameters(
            expr,
            0.0,
            &BTreeMap::from([(parameter_name.to_string(), current_value)]),
        )?;
        if !next_value.is_finite() {
            return None;
        }
        value = Some(next_value);
        current_value = next_value;
    }
    value
}

pub(super) fn collect_polygon_parameter_labels(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
) -> Vec<TextLabel> {
    let mut labels = groups
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
            let anchor = decode_label_anchor(file, group, anchors).or_else(|| {
                let anchor_record = group
                    .records
                    .iter()
                    .find(|record| record.record_type == 0x0903)?;
                decode_text_anchor(anchor_record.payload(&file.data))
            })?;
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
                    format!("{point_name}在{polygon_name}上的值 = {:.2}", global_t)
                },
                color: [30, 30, 30, 255],
                visible: decode_label_name(file, group).is_some()
                    || label_visible_for_group(file, group),
                binding: Some(TextLabelBinding::PolygonBoundaryParameter {
                    point_index: path.refs[0].checked_sub(1)?,
                    point_name,
                    polygon_name,
                }),
                screen_space: false,
                debug: Some(payload_debug_source(group)),
                ..Default::default()
            })
        })
        .collect::<Vec<_>>();
    labels.iter_mut().for_each(apply_fallback_rich_markup);
    labels
}

pub(super) fn collect_segment_parameter_labels(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
) -> Vec<TextLabel> {
    let mut labels = groups
        .iter()
        .filter(|group| (group.header.kind()) == crate::format::GroupKind::ParameterAnchor)
        .filter_map(|group| {
            let path = find_indexed_path(file, group)?;
            if path.refs.len() < 2 {
                return None;
            }

            let point_group = groups.get(path.refs[0].checked_sub(1)?)?;
            let segment_group = groups.get(path.refs[1].checked_sub(1)?)?;
            if !segment_group.header.kind().is_line_like() {
                return None;
            }
            let segment_path = find_indexed_path(file, segment_group)?;
            let start_group_index = segment_path.refs.first()?.checked_sub(1)?;
            let end_group_index = segment_path.refs.get(1)?.checked_sub(1)?;

            let point_name =
                decode_label_name(file, group).or_else(|| decode_label_name(file, point_group))?;
            let segment_name = segment_name(file, groups, segment_group)?;
            let anchor_record = group
                .records
                .iter()
                .find(|record| record.record_type == 0x0903)?;
            let anchor = decode_text_anchor(anchor_record.payload(&file.data))?;
            let constrained_segment_t = if point_group.header.kind().is_point_constraint() {
                match try_decode_point_constraint(file, groups, point_group, None, &None).ok()? {
                    RawPointConstraint::Segment(constraint) => Some(constraint.t),
                    _ => None,
                }
            } else {
                None
            };
            let projected_t = constrained_segment_t.or_else(|| {
                let point = anchors.get(path.refs[0].checked_sub(1)?)?.as_ref()?;
                let start = anchors.get(start_group_index)?.as_ref()?;
                let end = anchors.get(end_group_index)?.as_ref()?;
                segment_projection_parameter(point, start, end)
            })?;

            Some(TextLabel {
                anchor,
                text: if decode_label_name(file, group).is_some() {
                    format!("{point_name} = {:.2}", projected_t)
                } else if constrained_segment_t.is_some() {
                    format!("{point_name}在{segment_name}上的t值 = {:.2}", projected_t)
                } else {
                    format!("{point_name}在{segment_name}上的值 = {:.2}", projected_t)
                },
                color: [30, 30, 30, 255],
                visible: decode_label_name(file, group).is_some()
                    || label_visible_for_group(file, group),
                binding: if constrained_segment_t.is_some() {
                    Some(TextLabelBinding::SegmentParameter {
                        point_index: path.refs[0].checked_sub(1)?,
                        point_name,
                        segment_name,
                    })
                } else {
                    Some(TextLabelBinding::SegmentProjectionParameter {
                        point_index: path.refs[0].checked_sub(1)?,
                        start_index: start_group_index,
                        end_index: end_group_index,
                        point_name,
                        segment_name,
                    })
                },
                screen_space: false,
                debug: Some(payload_debug_source(group)),
                ..Default::default()
            })
        })
        .collect::<Vec<_>>();
    labels.iter_mut().for_each(apply_fallback_rich_markup);
    labels
}

pub(super) fn collect_polyline_parameter_labels(
    file: &GspFile,
    groups: &[ObjectGroup],
    _anchors: &[Option<PointRecord>],
) -> Vec<TextLabel> {
    let mut labels = groups
        .iter()
        .filter(|group| (group.header.kind()) == crate::format::GroupKind::ParameterAnchor)
        .filter_map(|group| {
            let path = find_indexed_path(file, group)?;
            if path.refs.len() < 2 {
                return None;
            }

            let point_group = groups.get(path.refs[0].checked_sub(1)?)?;
            let host_group = groups.get(path.refs[1].checked_sub(1)?)?;
            if !matches!(
                host_group.header.kind(),
                crate::format::GroupKind::PointTrace
                    | crate::format::GroupKind::CoordinateTrace
                    | crate::format::GroupKind::CustomTransformTrace
            ) {
                return None;
            }

            let value = point_on_trace_payload_parameter(file, point_group)?;
            let point_name =
                decode_label_name(file, group).or_else(|| decode_label_name(file, point_group))?;
            let object_name = trace_object_name(file, groups, host_group)?;
            let anchor_record = group
                .records
                .iter()
                .find(|record| record.record_type == 0x0903)?;
            let anchor = decode_text_anchor(anchor_record.payload(&file.data))?;

            Some(TextLabel {
                anchor,
                text: format!(
                    "{point_name}在{object_name}上的值 = {}",
                    format_number(value)
                ),
                color: [30, 30, 30, 255],
                visible: decode_label_name(file, group).is_some()
                    || label_visible_for_group(file, group),
                binding: Some(TextLabelBinding::PolylineParameter {
                    point_index: path.refs[0].checked_sub(1)?,
                    point_name,
                    object_name,
                }),
                screen_space: true,
                debug: Some(payload_debug_source(group)),
                ..Default::default()
            })
        })
        .collect::<Vec<_>>();
    labels.iter_mut().for_each(apply_fallback_rich_markup);
    labels
}

pub(super) fn collect_circle_parameter_labels(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
) -> Vec<TextLabel> {
    let mut labels = groups
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
                color: [30, 30, 30, 255],
                visible: decode_label_name(file, group).is_some()
                    || label_visible_for_group(file, group),
                binding: Some(TextLabelBinding::CircleParameter {
                    point_index: path.refs[0].checked_sub(1)?,
                    point_name,
                    circle_name,
                }),
                screen_space: false,
                debug: Some(payload_debug_source(group)),
                ..Default::default()
            })
        })
        .collect::<Vec<_>>();
    labels.iter_mut().for_each(apply_fallback_rich_markup);
    labels
}

pub(super) fn collect_custom_transform_expression_labels(
    file: &GspFile,
    groups: &[ObjectGroup],
    context: &SceneContext<'_>,
    anchors: &[Option<PointRecord>],
) -> Vec<TextLabel> {
    groups
        .iter()
        .filter(|group| (group.header.kind()) == crate::format::GroupKind::CustomTransformTrace)
        .filter_map(|group| {
            let path = context.indexed_path(group)?;
            if path.refs.len() < 6 {
                return None;
            }
            let source_group = context.group_by_ordinal(*path.refs.get(2)?)?;
            let parameter_anchor_group = context.group_by_ordinal(*path.refs.get(3)?)?;
            let distance_expr_group = context.group_by_ordinal(*path.refs.get(4)?)?;
            let angle_expr_group = context.group_by_ordinal(*path.refs.get(5)?)?;
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
                let anchor = try_decode_payload_anchor_point(file, expr_group)
                    .ok()
                    .flatten()?;
                let expr = context.function_expr(expr_group).ok()?;
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
                    debug: Some(payload_debug_source(group)),
                    ..Default::default()
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
    if !point_group.header.kind().is_point_constraint()
        || !segment_group.header.kind().is_line_like()
    {
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
    anchors: &[Option<PointRecord>],
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
            let depth_parameter_name = iteration_depth_driver_name(file, groups, iter_group);
            if let Some((vector_start_index, vector_end_index)) = label_iteration_vector_indices(
                file,
                groups,
                &seed_path,
                group_to_point_index,
                anchors,
            ) {
                return Some(LabelIterationFamily::TranslateExpression {
                    seed_label_index,
                    first_output_label_index: label_group_to_index.get(&group.ordinal).copied(),
                    output_label_count: depth,
                    vector_start_index,
                    vector_end_index,
                    parameter_name,
                    expr,
                    depth,
                    depth_expr: label_iteration_depth_expr(file, groups, iter_group),
                    depth_parameter_name,
                });
            }

            let point_seed_index = mapped_point_index(group_to_point_index, point_group_index)?;

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

fn label_iteration_vector_indices(
    file: &GspFile,
    groups: &[ObjectGroup],
    seed_path: &crate::format::IndexedPathRecord,
    group_to_point_index: &[Option<usize>],
    anchors: &[Option<PointRecord>],
) -> Option<(usize, usize)> {
    let point_group_index = seed_path.refs.first()?.checked_sub(1)?;
    let point_group = groups.get(point_group_index)?;
    if point_group.header.kind() != crate::format::GroupKind::Translation {
        return None;
    }
    let path = find_indexed_path(file, point_group)?;
    if path.refs.len() < 3 {
        return None;
    }
    Some((
        mapped_or_equivalent_point_index(
            group_to_point_index,
            anchors,
            path.refs[1].checked_sub(1)?,
        )?,
        mapped_or_equivalent_point_index(
            group_to_point_index,
            anchors,
            path.refs[2].checked_sub(1)?,
        )?,
    ))
}

fn mapped_or_equivalent_point_index(
    group_to_point_index: &[Option<usize>],
    anchors: &[Option<PointRecord>],
    group_index: usize,
) -> Option<usize> {
    if let Some(point_index) = mapped_point_index(group_to_point_index, group_index) {
        return Some(point_index);
    }
    let anchor = anchors.get(group_index).cloned().flatten()?;
    group_to_point_index
        .iter()
        .enumerate()
        .filter_map(|(candidate_index, point_index)| {
            let point_index = (*point_index)?;
            let candidate = anchors.get(candidate_index).cloned().flatten()?;
            ((candidate.x - anchor.x).abs() < 1e-6 && (candidate.y - anchor.y).abs() < 1e-6)
                .then_some(point_index)
        })
        .next()
}

fn label_iteration_depth_expr(
    file: &GspFile,
    groups: &[ObjectGroup],
    iter_group: &ObjectGroup,
) -> Option<FunctionExpr> {
    let path = find_indexed_path(file, iter_group)?;
    let depth_group = groups.get(path.refs.first()?.checked_sub(1)?)?;
    if depth_group.header.kind() != crate::format::GroupKind::FunctionExpr {
        return None;
    }
    decode_iteration_depth_expr(file, groups, depth_group)
}

pub(super) fn bind_button_seed_expression_labels(
    file: &GspFile,
    groups: &[ObjectGroup],
    context: &SceneContext<'_>,
    anchors: &[Option<PointRecord>],
    labels: &mut [TextLabel],
    label_group_to_index: &BTreeMap<usize, usize>,
    group_to_point_index: &[Option<usize>],
) {
    for seed_group in groups
        .iter()
        .filter(|group| (group.header.kind()) == crate::format::GroupKind::LabelIterationSeed)
    {
        let Some(seed_path) = context.indexed_path(seed_group) else {
            continue;
        };
        if seed_path.refs.len() < 2 {
            continue;
        }
        let Some(point_group_index) = seed_path
            .refs
            .first()
            .and_then(|ordinal| ordinal.checked_sub(1))
        else {
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
            .and_then(|ordinal| context.group_by_ordinal(*ordinal))
        else {
            continue;
        };
        if (button_group.header.kind()) != crate::format::GroupKind::ButtonLabel {
            continue;
        }
        let Some(label_index) = label_group_to_index.get(&button_group.ordinal).copied() else {
            continue;
        };
        let Some(expr_group) = context
            .indexed_path(button_group)
            .and_then(|path| path.refs.first().copied())
            .and_then(|ordinal| context.group_by_ordinal(ordinal))
        else {
            continue;
        };
        if (expr_group.header.kind()) != crate::format::GroupKind::FunctionExpr {
            continue;
        }
        let Some(expr) = context.function_expr(expr_group).ok() else {
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

pub(super) fn bind_point_label_anchors(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
    group_to_point_index: &[Option<usize>],
    labels: &mut [TextLabel],
    label_group_to_index: &BTreeMap<usize, usize>,
) {
    for group in groups {
        let Some(label_index) = label_group_to_index.get(&group.ordinal).copied() else {
            continue;
        };
        let Some(point_group_index) = point_label_anchor_group_index(file, group) else {
            continue;
        };
        let Some(point_index) =
            mapped_or_equivalent_point_index(group_to_point_index, anchors, point_group_index)
        else {
            continue;
        };
        let Some(point_anchor) = anchors.get(point_group_index).cloned().flatten() else {
            continue;
        };
        let Some(label) = labels.get_mut(label_index) else {
            continue;
        };
        if label.binding.is_some() {
            continue;
        }
        label.binding = Some(TextLabelBinding::PointAnchor {
            point_index,
            anchor_dx: label.anchor.x - point_anchor.x,
            anchor_dy: label.anchor.y - point_anchor.y,
            anchor_y_point_index: None,
            anchor_y_dy: None,
        });
    }
}

pub(super) fn bind_label_iteration_seed_anchors(
    file: &GspFile,
    groups: &[ObjectGroup],
    labels: &mut [TextLabel],
    label_group_to_index: &BTreeMap<usize, usize>,
    label_iterations: &[LabelIterationFamily],
    points: &[ScenePoint],
    group_to_point_index: &[Option<usize>],
) {
    let mut origin_by_parameter = BTreeMap::new();
    let label_y_control =
        label_iteration_vertical_control(file, groups, group_to_point_index, points);
    for family in label_iterations {
        let LabelIterationFamily::TranslateExpression {
            seed_label_index,
            vector_start_index,
            vector_end_index,
            parameter_name,
            expr,
            ..
        } = family
        else {
            continue;
        };
        origin_by_parameter.insert(parameter_name.clone(), *vector_start_index);
        let Some(label) = labels.get_mut(*seed_label_index) else {
            continue;
        };
        let Some(point) = points.get(*vector_end_index) else {
            continue;
        };
        label.binding = Some(TextLabelBinding::PointExpressionValue {
            point_index: *vector_end_index,
            anchor_dx: label.anchor.x - point.position.x,
            anchor_dy: label.anchor.y - point.position.y,
            anchor_y_point_index: label_y_control.as_ref().map(|control| control.point_index),
            anchor_y_dy: label_y_control
                .as_ref()
                .map(|control| label.anchor.y - control.base_y),
            parameter_name: parameter_name.clone(),
            expr: expr.clone(),
        });
    }
    for group in groups {
        if group.header.kind() != crate::format::GroupKind::LabelIterationSeed {
            continue;
        }
        let Some(label_index) = label_group_to_index.get(&group.ordinal).copied() else {
            continue;
        };
        let Some(label) = labels.get_mut(label_index) else {
            continue;
        };
        if label.binding.is_some() {
            continue;
        }
        let Some(path) = find_indexed_path(file, group) else {
            continue;
        };
        let Some(expr_group) = path
            .refs
            .get(1)
            .and_then(|ordinal| groups.get(ordinal.saturating_sub(1)))
        else {
            continue;
        };
        if expr_group.header.kind() != crate::format::GroupKind::FunctionExpr {
            if label.text == "0"
                && let Some(point_index) = origin_by_parameter.values().next().copied()
                && let Some(point) = points.get(point_index)
            {
                label.binding = Some(TextLabelBinding::PointAnchor {
                    point_index,
                    anchor_dx: label.anchor.x - point.position.x,
                    anchor_dy: label.anchor.y - point.position.y,
                    anchor_y_point_index: label_y_control
                        .as_ref()
                        .map(|control| control.point_index),
                    anchor_y_dy: label_y_control
                        .as_ref()
                        .map(|control| label.anchor.y - control.base_y),
                });
            }
            continue;
        }
        let Some(expr_path) = find_indexed_path(file, expr_group) else {
            continue;
        };
        let Some(parameter_name) = expr_path
            .refs
            .first()
            .and_then(|ordinal| groups.get(ordinal.saturating_sub(1)))
            .and_then(|parameter_group| decode_label_name(file, parameter_group))
        else {
            if label.text == "0"
                && let Some(point_index) = origin_by_parameter.values().next().copied()
                && let Some(point) = points.get(point_index)
            {
                label.binding = Some(TextLabelBinding::PointAnchor {
                    point_index,
                    anchor_dx: label.anchor.x - point.position.x,
                    anchor_dy: label.anchor.y - point.position.y,
                    anchor_y_point_index: label_y_control
                        .as_ref()
                        .map(|control| control.point_index),
                    anchor_y_dy: label_y_control
                        .as_ref()
                        .map(|control| label.anchor.y - control.base_y),
                });
            }
            continue;
        };
        let Some(point_index) = origin_by_parameter.get(&parameter_name).copied() else {
            if label.text != "0" {
                continue;
            }
            let Some(point_index) = origin_by_parameter.values().next().copied() else {
                continue;
            };
            let Some(point) = points.get(point_index) else {
                continue;
            };
            label.binding = Some(TextLabelBinding::PointAnchor {
                point_index,
                anchor_dx: label.anchor.x - point.position.x,
                anchor_dy: label.anchor.y - point.position.y,
                anchor_y_point_index: label_y_control.as_ref().map(|control| control.point_index),
                anchor_y_dy: label_y_control
                    .as_ref()
                    .map(|control| label.anchor.y - control.base_y),
            });
            continue;
        };
        let Some(point) = points.get(point_index) else {
            continue;
        };
        label.binding = Some(TextLabelBinding::PointAnchor {
            point_index,
            anchor_dx: label.anchor.x - point.position.x,
            anchor_dy: label.anchor.y - point.position.y,
            anchor_y_point_index: label_y_control.as_ref().map(|control| control.point_index),
            anchor_y_dy: label_y_control
                .as_ref()
                .map(|control| label.anchor.y - control.base_y),
        });
    }
}

struct LabelVerticalControl {
    point_index: usize,
    base_y: f64,
}

fn label_iteration_vertical_control(
    file: &GspFile,
    groups: &[ObjectGroup],
    group_to_point_index: &[Option<usize>],
    points: &[ScenePoint],
) -> Option<LabelVerticalControl> {
    let zero_label_point_group_index = groups.iter().find_map(|group| {
        if group.header.kind() != crate::format::GroupKind::LabelIterationSeed {
            return None;
        }
        let path = find_indexed_path(file, group)?;
        let expr_group = path
            .refs
            .get(1)
            .and_then(|ordinal| groups.get(ordinal.checked_sub(1)?))?;
        (expr_group.header.kind() != crate::format::GroupKind::FunctionExpr)
            .then(|| path.refs.first()?.checked_sub(1))
            .flatten()
    })?;
    groups.iter().enumerate().find_map(|(group_index, group)| {
        if group.header.kind() != crate::format::GroupKind::Translation || group.header.is_hidden()
        {
            return None;
        }
        let path = find_indexed_path(file, group)?;
        if path.refs.first()?.checked_sub(1)? != zero_label_point_group_index {
            return None;
        }
        let point_index = group_to_point_index.get(group_index).copied().flatten()?;
        let vector_start_index = path
            .refs
            .get(1)
            .and_then(|ordinal| group_to_point_index.get(ordinal.checked_sub(1)?))
            .copied()
            .flatten()?;
        let vector_end_index = path
            .refs
            .get(2)
            .and_then(|ordinal| group_to_point_index.get(ordinal.checked_sub(1)?))
            .copied()
            .flatten()?;
        let control = points.get(point_index)?;
        let vector_start = points.get(vector_start_index)?;
        let vector_end = points.get(vector_end_index)?;
        Some(LabelVerticalControl {
            point_index,
            base_y: control.position.y - (vector_end.position.y - vector_start.position.y),
        })
    })
}

fn point_label_anchor_group_index(file: &GspFile, group: &ObjectGroup) -> Option<usize> {
    if group.header.kind() == crate::format::GroupKind::LabelIterationSeed {
        let path = find_indexed_path(file, group)?;
        return path.refs.first()?.checked_sub(1);
    }
    if matches!(
        group.header.kind(),
        crate::format::GroupKind::Point
            | crate::format::GroupKind::LegacyCoordinateConstructPoint
            | crate::format::GroupKind::FixedCoordinatePoint
            | crate::format::GroupKind::CustomTransformPoint
            | crate::format::GroupKind::Translation
            | crate::format::GroupKind::Reflection
            | crate::format::GroupKind::Rotation
            | crate::format::GroupKind::ExpressionRotation
            | crate::format::GroupKind::ParameterRotation
            | crate::format::GroupKind::ParameterControlledPoint
            | crate::format::GroupKind::Scale
            | crate::format::GroupKind::PointConstraint
            | crate::format::GroupKind::PathPoint
            | crate::format::GroupKind::LinearIntersectionPoint
            | crate::format::GroupKind::IntersectionPoint1
            | crate::format::GroupKind::IntersectionPoint2
            | crate::format::GroupKind::CircleCircleIntersectionPoint1
            | crate::format::GroupKind::CircleCircleIntersectionPoint2
            | crate::format::GroupKind::Midpoint
            | crate::format::GroupKind::GraphCalibrationX
            | crate::format::GroupKind::GraphCalibrationY
            | crate::format::GroupKind::GraphCalibrationYAlt
    ) {
        return group.ordinal.checked_sub(1);
    }
    None
}

pub(super) fn collect_iteration_tables(
    file: &GspFile,
    groups: &[ObjectGroup],
    context: &SceneContext<'_>,
    anchors: &[Option<PointRecord>],
) -> Vec<IterationTable> {
    groups
        .iter()
        .filter(|group| {
            (group.header.kind()) == crate::format::GroupKind::IterationExpressionHelper
        })
        .filter_map(|group| {
            let path = context.indexed_path(group)?;
            if path.refs.len() < 2 {
                return None;
            }
            let iter_group = context.group_by_ordinal(path.refs[0])?;
            let expr_group = context.group_by_ordinal(path.refs[1])?;
            if expr_group.header.kind() != crate::format::GroupKind::FunctionExpr {
                return None;
            }
            let expr = context.function_expr(expr_group).ok()?;
            let parameter_name = direct_function_expr_parameter_name(file, groups, expr_group)
                .or_else(|| {
                    resolve_function_expr_parameter(
                        file,
                        groups,
                        expr_group,
                        anchors,
                        &mut BTreeSet::new(),
                    )
                    .map(|(name, _)| name)
                })?;
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
            let depth_parameter_name = iteration_depth_driver_name(file, groups, iter_group);
            Some(IterationTable {
                anchor: decode_iteration_table_anchor(file, group)?,
                expr_label,
                parameter_name,
                expr,
                depth,
                depth_parameter_name,
                visible: true,
                debug: Some(payload_debug_source(group)),
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

fn trace_object_name(
    file: &GspFile,
    groups: &[ObjectGroup],
    trace_group: &ObjectGroup,
) -> Option<String> {
    decode_label_name(file, trace_group).or_else(|| {
        let kind = trace_group.header.kind();
        let index = groups
            .iter()
            .filter(|group| group.header.kind() == kind)
            .take_while(|group| group.ordinal <= trace_group.ordinal)
            .count()
            .max(1);
        Some(format!("L{}", subscript_number(index)))
    })
}

fn subscript_number(value: usize) -> String {
    value
        .to_string()
        .chars()
        .map(|digit| match digit {
            '0' => '₀',
            '1' => '₁',
            '2' => '₂',
            '3' => '₃',
            '4' => '₄',
            '5' => '₅',
            '6' => '₆',
            '7' => '₇',
            '8' => '₈',
            '9' => '₉',
            other => other,
        })
        .collect()
}

fn polyline_parameter(point_count: usize, segment_index: usize, t: f64) -> Option<f64> {
    if point_count < 2 {
        return None;
    }
    let denominator = point_count.checked_sub(1)? as f64;
    Some((segment_index as f64 + t.clamp(0.0, 1.0)) / denominator)
}

fn point_on_trace_payload_parameter(file: &GspFile, point_group: &ObjectGroup) -> Option<f64> {
    let payload = point_group
        .records
        .iter()
        .find(|record| record.record_type == RECORD_BINDING_PAYLOAD)
        .map(|record| record.payload(&file.data))?;
    let value = read_f64(payload, 4);
    value.is_finite().then_some(value)
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
