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
use crate::format::{GspFile, IndexedPathRecord, ObjectGroup, PointRecord, read_f64, read_u32};
use crate::runtime::DEFAULT_GRAPH_RAW_PER_UNIT;
use crate::runtime::extract::context::SceneContext;
use crate::runtime::extract::iteration_depth::decode_iteration_depth_expr;
use crate::runtime::functions::{
    FunctionExpr, evaluate_expr_with_parameters, function_expr_label,
    function_parameter_group_ordinals, synthesize_standalone_function_definition_labels,
    try_decode_function_expr,
};
use crate::runtime::geometry::{
    angle_degrees_from_points, color_from_style, distance_world, format_number,
};
use crate::runtime::payload_consts::{
    EXPR_EULER_WORD, EXPR_OP_ADD, EXPR_OP_DIV, EXPR_OP_MUL, EXPR_OP_POW, EXPR_OP_SUB,
    EXPR_PARAMETER_MASK, EXPR_PARAMETER_PREFIX, EXPR_PI_WORD, EXPR_VARIABLE_WORD,
    FUNCTION_EXPR_MARKER_A, FUNCTION_EXPR_MARKER_B, RECORD_BINDING_PAYLOAD,
    RECORD_FUNCTION_EXPR_PAYLOAD, RECORD_ITERATION_DEFINITION, RECORD_POINT_F64_PAIR,
    RECORD_RICH_TEXT, RECORD_VALUE_TABLE_LAYOUT,
};
use crate::runtime::scene::{
    IterationTable, IterationTableColumn, IterationTableValueBinding, LabelIterationFamily,
    RichTextExpressionRef, RichTextExpressionValue, ScenePoint, ScreenPoint, TextLabel,
    TextLabelBinding, TextLabelHotspot, TextLabelHotspotAction,
};

use super::analysis::{CollectedShapes, SceneAnalysis};
use super::shapes::CircleShape;

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

pub(super) fn collect_scene_labels(
    file: &GspFile,
    groups: &[ObjectGroup],
    context: &SceneContext<'_>,
    analysis: &SceneAnalysis,
    shapes: &CollectedShapes,
) -> (
    Vec<TextLabel>,
    BTreeMap<usize, usize>,
    Vec<PendingLabelHotspot>,
) {
    let (mut labels, label_group_to_index, mut pending_hotspots) = collect_labels(
        file,
        groups,
        &analysis.raw_anchors,
        analysis.graph_mode,
        !analysis.has_function_plots && !analysis.has_coordinate_objects,
    );
    labels.extend(collect_coordinate_labels(
        file,
        groups,
        context,
        &analysis.raw_anchors,
    ));
    labels.extend(collect_polygon_parameter_labels(
        file,
        groups,
        &analysis.raw_anchors,
    ));
    labels.extend(collect_line_projection_parameter_labels(
        file,
        groups,
        &analysis.raw_anchors,
    ));
    labels.extend(collect_polyline_parameter_labels(
        file,
        groups,
        &analysis.raw_anchors,
    ));
    labels.extend(collect_custom_transform_expression_labels(
        file,
        groups,
        context,
        &analysis.raw_anchors,
    ));
    labels.extend(collect_circle_parameter_labels(
        file,
        groups,
        &analysis.raw_anchors,
    ));
    labels.extend(synthesize_standalone_function_definition_labels(
        file, groups, &labels,
    ));
    append_circle_perimeter_label(
        &mut labels,
        &mut pending_hotspots,
        &shapes.circles,
        analysis,
    );
    (labels, label_group_to_index, pending_hotspots)
}

fn append_circle_perimeter_label(
    labels: &mut Vec<TextLabel>,
    pending_hotspots: &mut [PendingLabelHotspot],
    circles: &[CircleShape],
    analysis: &SceneAnalysis,
) {
    if analysis.graph_mode
        && let (Some(circle), Some(formula_index), Some(transform)) = (
            circles.first(),
            labels.iter().position(|label| label.text.contains("AB:")),
            analysis.graph_ref.as_ref(),
        )
    {
        let circumference = 2.0
            * std::f64::consts::PI
            * distance_world(&circle.center, &circle.radius_point, &analysis.graph_ref);
        let anchor = PointRecord {
            x: labels[formula_index].anchor.x,
            y: labels[formula_index].anchor.y - 0.9 * transform.raw_per_unit,
        };
        for hotspot in pending_hotspots.iter_mut() {
            if hotspot.label_index >= formula_index {
                hotspot.label_index += 1;
            }
        }
        labels.insert(
            formula_index,
            TextLabel {
                anchor,
                text: format!("AB perimeter = {:.2} cm", circumference),
                color: [30, 30, 30, 255],
                visible: true,
                screen_space: false,
                ..Default::default()
            },
        );
    }
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

fn named_alias_binding(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
) -> Option<TextLabelBinding> {
    if group.header.kind() != crate::format::GroupKind::NamedAlias {
        return None;
    }
    let source_group_ordinal = *find_indexed_path(file, group)?.refs.first()?;
    let source_group = groups.get(source_group_ordinal.checked_sub(1)?)?;
    if matches!(
        source_group.header.kind(),
        crate::format::GroupKind::AngleMarker | crate::format::GroupKind::LegacyAngleMarker
    ) {
        let path = find_indexed_path(file, source_group)?;
        if path.refs.len() >= 3 {
            return Some(TextLabelBinding::PointAngleValue {
                start_index: path.refs[0].saturating_sub(1),
                vertex_index: path.refs[1].saturating_sub(1),
                end_index: path.refs[2].saturating_sub(1),
                name: decode_label_name(file, group).unwrap_or_default(),
                value_suffix: "°".to_string(),
            });
        }
    }
    Some(TextLabelBinding::ScalarAlias {
        source_group_ordinal,
        name: decode_label_name(file, group).unwrap_or_default(),
    })
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
        let color = color_from_style(group.header.style_b);
        if color[..3].iter().all(|component| *component <= 1) {
            [color[0] * 255, color[1] * 255, color[2] * 255, 255]
        } else {
            color
        }
    } else {
        [30, 30, 30, 255]
    }
}

fn rich_text_font(file: &GspFile, group: &ObjectGroup) -> (Option<f64>, Option<String>) {
    let Some(payload) = group
        .records
        .iter()
        .find(|record| record.record_type == RECORD_RICH_TEXT)
        .map(|record| record.payload(&file.data))
    else {
        return (None, None);
    };
    if payload.len() < 12 {
        return (None, None);
    }
    let index = crate::format::read_u32(payload, 8);
    file.document_font(index)
        .map(|(point_size, family)| (Some(f64::from(point_size) * 4.0 / 3.0), Some(family)))
        .unwrap_or((None, None))
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
            let value = rich_text_value_ref_for_group(file, groups, source_group)?;
            Some(RichTextExpressionRef {
                source_group_ordinal: *source_group_ordinal,
                slot: hotspot.path_slot,
                line: hotspot.line,
                start: hotspot.start,
                end: hotspot.end,
                value,
            })
        })
        .collect::<Vec<_>>();
    (!refs.is_empty()).then(|| TextLabelBinding::RichTextExpressionValues {
        template_text: text.to_string(),
        template_rich_markup: rich_markup.clone(),
        refs,
    })
}

fn rich_text_value_ref_for_group(
    file: &GspFile,
    groups: &[ObjectGroup],
    source_group: &ObjectGroup,
) -> Option<RichTextExpressionValue> {
    if source_group.header.kind() == crate::format::GroupKind::FunctionExpr {
        return Some(RichTextExpressionValue::Expr {
            expr: try_decode_function_expr(file, groups, source_group).ok()?,
        });
    }
    if let Some(name) = editable_non_graph_parameter_name_for_group(file, groups, source_group)
        .or_else(|| decode_label_name(file, source_group))
        .or_else(|| decode_label_name_raw(file, source_group))
    {
        return Some(RichTextExpressionValue::Parameter { name });
    }
    rich_text_iteration_coordinate_value_ref(file, groups, source_group)
}

fn rich_text_iteration_coordinate_value_ref(
    file: &GspFile,
    groups: &[ObjectGroup],
    coordinate_group: &ObjectGroup,
) -> Option<RichTextExpressionValue> {
    let axis = match coordinate_group.header.kind() {
        crate::format::GroupKind::CoordinateXValue => {
            crate::runtime::scene::CoordinateAxis::Horizontal
        }
        crate::format::GroupKind::CoordinateYValue => {
            crate::runtime::scene::CoordinateAxis::Vertical
        }
        _ => return None,
    };
    let coordinate_path = find_indexed_path(file, coordinate_group)?;
    let alias_group = groups.get(coordinate_path.refs.first()?.checked_sub(1)?)?;
    if alias_group.header.kind() != crate::format::GroupKind::IterationPointAlias {
        return None;
    }
    let alias_path = find_indexed_path(file, alias_group)?;
    let binding_group = groups.get(alias_path.refs.first()?.checked_sub(1)?)?;
    if binding_group.header.kind() != crate::format::GroupKind::IterationBinding {
        return None;
    }
    let binding_path = find_indexed_path(file, binding_group)?;
    let source_group = groups.get(binding_path.refs.first()?.checked_sub(1)?)?;
    let iter_group = groups.get(binding_path.refs.get(1)?.checked_sub(1)?)?;
    if !source_group.header.kind().is_coordinate_object()
        || iter_group.header.kind() != crate::format::GroupKind::RegularPolygonIteration
    {
        return None;
    }
    let source_path = find_indexed_path(file, source_group)?;
    let target_parameter_group_ordinal = match axis {
        crate::runtime::scene::CoordinateAxis::Horizontal => source_path.refs.first()?,
        crate::runtime::scene::CoordinateAxis::Vertical => source_path.refs.get(1)?,
    };
    let target_group = groups.get(target_parameter_group_ordinal.checked_sub(1)?)?;
    let target_parameter_name =
        editable_non_graph_parameter_name_for_group(file, groups, target_group)
            .or_else(|| decode_label_name(file, target_group))
            .or_else(|| decode_label_name_raw(file, target_group))?;
    let iter_path = find_indexed_path(file, iter_group)?;
    let mut seen_names = BTreeSet::new();
    let mut state_parameter_names = Vec::new();
    let mut state_exprs = Vec::new();
    for ordinal in iter_path.refs.iter().skip(1) {
        let expr_group = groups.get(ordinal.checked_sub(1)?)?;
        if expr_group.header.kind() != crate::format::GroupKind::FunctionExpr {
            continue;
        }
        let parameter_name = direct_function_expr_parameter_name(file, groups, expr_group)
            .or_else(|| {
                resolve_function_expr_parameter(file, groups, expr_group, &[], &mut BTreeSet::new())
                    .map(|(name, _)| name)
            })?;
        if !seen_names.insert(parameter_name.clone()) {
            continue;
        }
        state_parameter_names.push(parameter_name);
        state_exprs.push(try_decode_function_expr(file, groups, expr_group).ok()?);
    }
    if state_parameter_names.is_empty()
        || state_parameter_names.len() != state_exprs.len()
        || !state_parameter_names
            .iter()
            .any(|name| name == &target_parameter_name)
    {
        return None;
    }
    let depth = iter_group
        .records
        .iter()
        .find(|record| record.record_type == RECORD_ITERATION_DEFINITION)
        .map(|record| record.payload(&file.data))
        .filter(|payload| payload.len() >= 20)
        .map(|payload| read_u32(payload, 16) as usize)
        .unwrap_or(3);
    let depth_expr = iter_path
        .refs
        .first()
        .and_then(|ordinal| groups.get(ordinal.checked_sub(1)?))
        .filter(|group| group.header.kind() == crate::format::GroupKind::FunctionExpr)
        .and_then(|group| try_decode_function_expr(file, groups, group).ok());
    Some(RichTextExpressionValue::IterationState {
        state_parameter_names,
        state_exprs,
        target_parameter_name,
        depth,
        depth_expr,
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
    if let Some(segment_group) = path
        .refs
        .get(1)
        .and_then(|ordinal| groups.get(ordinal.checked_sub(1)?))
        .filter(|group| group.header.kind().is_line_like())
    {
        let segment_path = find_indexed_path(file, segment_group)?;
        let point = anchors.get(point_group_index)?.as_ref()?;
        let start = anchors
            .get(segment_path.refs.first()?.checked_sub(1)?)?
            .as_ref()?;
        let end = anchors
            .get(segment_path.refs.get(1)?.checked_sub(1)?)?
            .as_ref()?;
        return line_projection_parameter(
            point,
            start,
            end,
            line_like_kind(segment_group.header.kind())?,
        );
    }
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
        RawPointConstraint::HostedArc { t, .. } => Some(t),
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

fn ratio_value_label_name(
    file: &GspFile,
    groups: &[ObjectGroup],
    path: &IndexedPathRecord,
) -> Option<String> {
    let origin = point_label_or_default(file, groups, path.refs.first()?.saturating_sub(1), "P");
    let denominator =
        point_label_or_default(file, groups, path.refs.get(1)?.saturating_sub(1), "Q");
    let numerator = point_label_or_default(file, groups, path.refs.get(2)?.saturating_sub(1), "R");
    Some(format!("({origin}{numerator}/{origin}{denominator})"))
}

fn line_projection_parameter(
    point: &PointRecord,
    start: &PointRecord,
    end: &PointRecord,
    line_kind: crate::runtime::scene::LineLikeKind,
) -> Option<f64> {
    gsp_runtime_core::project_to_line_like(
        gsp_runtime_core::Point {
            x: point.x,
            y: point.y,
        },
        gsp_runtime_core::Point {
            x: start.x,
            y: start.y,
        },
        gsp_runtime_core::Point { x: end.x, y: end.y },
        match line_kind {
            crate::runtime::scene::LineLikeKind::Segment => gsp_runtime_core::LineKind::Segment,
            crate::runtime::scene::LineLikeKind::Line => gsp_runtime_core::LineKind::Line,
            crate::runtime::scene::LineLikeKind::Ray => gsp_runtime_core::LineKind::Ray,
        },
    )
    .map(|projection| projection.t)
}

fn line_like_kind(kind: crate::format::GroupKind) -> Option<crate::runtime::scene::LineLikeKind> {
    match kind {
        crate::format::GroupKind::Segment | crate::format::GroupKind::GraphMeasurementSegment => {
            Some(crate::runtime::scene::LineLikeKind::Segment)
        }
        crate::format::GroupKind::Line => Some(crate::runtime::scene::LineLikeKind::Line),
        crate::format::GroupKind::Ray => Some(crate::runtime::scene::LineLikeKind::Ray),
        _ => None,
    }
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
            let record = group.records.iter().find(|record| {
                record.record_type == crate::runtime::payload_consts::RECORD_BINDING_PAYLOAD
                    && record.length == 12
            })?;
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

include!("labels/expression_display.rs");
fn decode_iteration_table_anchor(file: &GspFile, group: &ObjectGroup) -> Option<ScreenPoint> {
    let payload = group
        .records
        .iter()
        .find(|record| record.record_type == RECORD_VALUE_TABLE_LAYOUT)
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
        .find(|record| {
            record.record_type == crate::runtime::payload_consts::RECORD_FUNCTION_EXPR_PAYLOAD
        })
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
                        .or_else(|| named_alias_binding(file, groups, group))
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
                    let (font_size, font_family) = rich_text_font(file, group);
                    labels.push(TextLabel {
                        anchor,
                        text,
                        rich_markup,
                        color: label_color_for_group(group),
                        font_size,
                        font_family,
                        visible,
                        binding,
                        screen_space: kind == crate::format::GroupKind::ButtonLabel
                            || group
                                .records
                                .iter()
                                .any(|record| record.record_type == RECORD_RICH_TEXT),
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
                        .or_else(|| named_alias_binding(file, groups, group))
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
                    let (font_size, font_family) = rich_text_font(file, group);
                    labels.push(TextLabel {
                        anchor,
                        text,
                        rich_markup,
                        color: label_color_for_group(group),
                        font_size,
                        font_family,
                        visible,
                        binding,
                        screen_space: kind == crate::format::GroupKind::ButtonLabel
                            || group
                                .records
                                .iter()
                                .any(|record| record.record_type == RECORD_RICH_TEXT),
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
        matches!(
            record.record_type,
            crate::runtime::payload_consts::RECORD_ACTION_AUX
                | crate::runtime::payload_consts::RECORD_FUNCTION_EXPR_PAYLOAD
        )
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

include!("labels/rich_markup.rs");
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
        } else if matches!(
            kind,
            crate::format::GroupKind::DistanceValue | crate::format::GroupKind::GraphDistanceValue
        ) && let Some(path) = find_indexed_path(file, group)
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
        } else if matches!(
            kind,
            crate::format::GroupKind::AngleValue | crate::format::GroupKind::VertexAngleValue
        ) && let Some(path) = find_indexed_path(file, group)
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
            && let Some(value) = ratio_value(file, group, anchors)
            && let Some(anchor) = decode_label_anchor(file, group, anchors)
                .or_else(|| try_decode_payload_anchor_point(file, group).ok().flatten())
        {
            let name = decode_label_name(file, group)
                .or_else(|| ratio_value_label_name(file, groups, &path))
                .unwrap_or_else(|| "ratio".to_string());
            let clamp_to_unit = true;
            let value = if clamp_to_unit { value.min(1.0) } else { value };
            let value_text = format_number(value);
            let text = format!("{name} = {value_text}");
            let rich_markup = build_ratio_value_rich_markup(&name, &value_text);
            labels.push(TextLabel {
                anchor,
                text,
                rich_markup,
                color: [30, 30, 30, 255],
                visible: ratio_value_label_visible(file, groups, group, helper_visible),
                binding: Some(TextLabelBinding::PointDistanceRatioValue {
                    origin_index: path.refs[0].saturating_sub(1),
                    denominator_index: path.refs[1].saturating_sub(1),
                    numerator_index: path.refs[2].saturating_sub(1),
                    name,
                    clamp_to_unit,
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
            let mut expr_label = payload_function_expr_label(
                file,
                groups,
                anchors,
                group,
                &function_expr_label(expr.clone()),
                &mut BTreeSet::new(),
            );
            if semantic_kind == Some("regular-polygon-angle")
                && expr_label == format!("360 / {parameter_name}")
            {
                expr_label = format!("360° / {parameter_name}");
            }
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
                    parameter_group_ordinals: function_parameter_group_ordinals(
                        file, groups, group,
                    ),
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
                font_size: None,
                font_family: None,
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
        font_size: None,
        font_family: None,
        visible: label_visible_for_group(file, group),
        binding: Some(TextLabelBinding::ExpressionValue {
            parameter_name,
            result_name: decode_label_name(file, group),
            expr_label,
            expr,
            parameter_group_ordinals: function_parameter_group_ordinals(file, groups, expr_group),
        }),
        screen_space: false,
        hotspots: Vec::new(),
        debug: Some(payload_debug_source(group)),
    })
}

include!("labels/hotspots.rs");
include!("labels/iterations.rs");
