use super::function_expr_json::FunctionExprJson;
use super::scene_json::{DebugSourceJson, PointJson};
use crate::runtime::scene::{
    ButtonAction, RichTextExpressionRef, SceneButton, TextLabelBinding, TextLabelHotspotAction,
};
use serde::Serialize;
use ts_rs::TS;

#[derive(Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub(super) struct ButtonJson {
    text: String,
    x: f64,
    y: f64,
    width: Option<f64>,
    height: Option<f64>,
    visible: bool,
    action: ButtonActionJson,
    #[serde(skip_serializing_if = "Option::is_none")]
    debug: Option<DebugSourceJson>,
}

impl ButtonJson {
    pub(super) fn from_button(button: &SceneButton) -> Self {
        Self {
            text: button.text.clone(),
            x: button.anchor.x,
            y: button.anchor.y,
            width: button.rect.as_ref().map(|rect| rect.width),
            height: button.rect.as_ref().map(|rect| rect.height),
            visible: button.visible,
            action: ButtonActionJson::from_action(&button.action),
            debug: button.debug.as_ref().map(DebugSourceJson::from_source),
        }
    }
}

#[derive(Serialize, TS)]
#[serde(tag = "kind", rename_all = "kebab-case")]
enum ButtonActionJson {
    Link {
        href: String,
    },
    ToggleVisibility {
        #[serde(rename = "buttonIndices")]
        button_indices: Vec<usize>,
        #[serde(rename = "labelIndices")]
        label_indices: Vec<usize>,
        #[serde(rename = "imageIndices")]
        image_indices: Vec<usize>,
        #[serde(rename = "pointIndices")]
        point_indices: Vec<usize>,
        #[serde(rename = "lineIndices")]
        line_indices: Vec<usize>,
        #[serde(rename = "circleIndices")]
        circle_indices: Vec<usize>,
        #[serde(rename = "polygonIndices")]
        polygon_indices: Vec<usize>,
    },
    SetVisibility {
        visible: bool,
        #[serde(rename = "buttonIndices")]
        button_indices: Vec<usize>,
        #[serde(rename = "labelIndices")]
        label_indices: Vec<usize>,
        #[serde(rename = "imageIndices")]
        image_indices: Vec<usize>,
        #[serde(rename = "pointIndices")]
        point_indices: Vec<usize>,
        #[serde(rename = "lineIndices")]
        line_indices: Vec<usize>,
        #[serde(rename = "circleIndices")]
        circle_indices: Vec<usize>,
        #[serde(rename = "polygonIndices")]
        polygon_indices: Vec<usize>,
    },
    ShowHideVisibility {
        #[serde(rename = "buttonIndices")]
        button_indices: Vec<usize>,
        #[serde(rename = "labelIndices")]
        label_indices: Vec<usize>,
        #[serde(rename = "imageIndices")]
        image_indices: Vec<usize>,
        #[serde(rename = "pointIndices")]
        point_indices: Vec<usize>,
        #[serde(rename = "lineIndices")]
        line_indices: Vec<usize>,
        #[serde(rename = "circleIndices")]
        circle_indices: Vec<usize>,
        #[serde(rename = "polygonIndices")]
        polygon_indices: Vec<usize>,
    },
    MovePoint {
        #[serde(rename = "pointIndex")]
        point_index: usize,
        #[serde(rename = "targetPointIndex")]
        target_point_index: Option<usize>,
    },
    MovePoints {
        targets: Vec<ButtonMoveTargetJson>,
    },
    SetParameter {
        #[serde(rename = "parameterName")]
        parameter_name: String,
        value: f64,
    },
    AnimateParameter {
        #[serde(rename = "parameterName")]
        parameter_name: String,
        #[serde(rename = "targetValue")]
        target_value: f64,
    },
    AnimatePoint {
        #[serde(rename = "pointIndex")]
        point_index: usize,
    },
    ScrollPoint {
        #[serde(rename = "pointIndex")]
        point_index: usize,
    },
    FocusPoint {
        #[serde(rename = "pointIndex")]
        point_index: usize,
    },
    PlayFunction {
        #[serde(rename = "functionKey")]
        function_key: usize,
    },
    Sequence {
        #[serde(rename = "buttonIndices")]
        button_indices: Vec<usize>,
        #[serde(rename = "intervalMs")]
        interval_ms: u32,
    },
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
struct ButtonMoveTargetJson {
    point_index: usize,
    target_point_index: Option<usize>,
}

impl ButtonActionJson {
    fn from_action(action: &ButtonAction) -> Self {
        match action {
            ButtonAction::Link { href } => Self::Link { href: href.clone() },
            ButtonAction::ToggleVisibility {
                button_indices,
                label_indices,
                image_indices,
                point_indices,
                line_indices,
                circle_indices,
                polygon_indices,
            } => Self::ToggleVisibility {
                button_indices: button_indices.clone(),
                label_indices: label_indices.clone(),
                image_indices: image_indices.clone(),
                point_indices: point_indices.clone(),
                line_indices: line_indices.clone(),
                circle_indices: circle_indices.clone(),
                polygon_indices: polygon_indices.clone(),
            },
            ButtonAction::SetVisibility {
                visible,
                button_indices,
                label_indices,
                image_indices,
                point_indices,
                line_indices,
                circle_indices,
                polygon_indices,
            } => Self::SetVisibility {
                visible: *visible,
                button_indices: button_indices.clone(),
                label_indices: label_indices.clone(),
                image_indices: image_indices.clone(),
                point_indices: point_indices.clone(),
                line_indices: line_indices.clone(),
                circle_indices: circle_indices.clone(),
                polygon_indices: polygon_indices.clone(),
            },
            ButtonAction::ShowHideVisibility {
                button_indices,
                label_indices,
                image_indices,
                point_indices,
                line_indices,
                circle_indices,
                polygon_indices,
            } => Self::ShowHideVisibility {
                button_indices: button_indices.clone(),
                label_indices: label_indices.clone(),
                image_indices: image_indices.clone(),
                point_indices: point_indices.clone(),
                line_indices: line_indices.clone(),
                circle_indices: circle_indices.clone(),
                polygon_indices: polygon_indices.clone(),
            },
            ButtonAction::MovePoint {
                point_index,
                target_point_index,
            } => Self::MovePoint {
                point_index: *point_index,
                target_point_index: *target_point_index,
            },
            ButtonAction::MovePoints { targets } => Self::MovePoints {
                targets: targets
                    .iter()
                    .map(|target| ButtonMoveTargetJson {
                        point_index: target.point_index,
                        target_point_index: target.target_point_index,
                    })
                    .collect(),
            },
            ButtonAction::SetParameter {
                parameter_name,
                value,
            } => Self::SetParameter {
                parameter_name: parameter_name.clone(),
                value: *value,
            },
            ButtonAction::AnimateParameter {
                parameter_name,
                target_value,
            } => Self::AnimateParameter {
                parameter_name: parameter_name.clone(),
                target_value: *target_value,
            },
            ButtonAction::AnimatePoint { point_index } => Self::AnimatePoint {
                point_index: *point_index,
            },
            ButtonAction::ScrollPoint { point_index } => Self::ScrollPoint {
                point_index: *point_index,
            },
            ButtonAction::FocusPoint { point_index } => Self::FocusPoint {
                point_index: *point_index,
            },
            ButtonAction::PlayFunction { function_key } => Self::PlayFunction {
                function_key: *function_key,
            },
            ButtonAction::Sequence {
                button_indices,
                interval_ms,
            } => Self::Sequence {
                button_indices: button_indices.clone(),
                interval_ms: *interval_ms,
            },
        }
    }
}

#[derive(Serialize, TS)]
pub(super) struct LabelJson {
    anchor: PointJson,
    text: String,
    #[serde(rename = "richMarkup")]
    #[serde(skip_serializing_if = "Option::is_none")]
    rich_markup: Option<String>,
    color: [u8; 4],
    visible: bool,
    binding: Option<LabelBindingJson>,
    hotspots: Vec<LabelHotspotJson>,
    #[serde(rename = "screenSpace")]
    screen_space: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    debug: Option<DebugSourceJson>,
}

impl LabelJson {
    pub(super) fn from_label(label: &crate::runtime::scene::TextLabel) -> Self {
        Self {
            anchor: PointJson::from_point(&label.anchor),
            text: label.text.clone(),
            rich_markup: label.rich_markup.clone(),
            color: label.color,
            visible: label.visible,
            binding: label.binding.as_ref().map(LabelBindingJson::from_binding),
            hotspots: label
                .hotspots
                .iter()
                .map(LabelHotspotJson::from_hotspot)
                .collect(),
            screen_space: label.screen_space,
            debug: label.debug.as_ref().map(DebugSourceJson::from_source),
        }
    }
}

#[derive(Serialize, TS)]
#[serde(rename_all = "camelCase")]
struct LabelHotspotJson {
    line: usize,
    start: usize,
    end: usize,
    text: String,
    action: LabelHotspotActionJson,
}

impl LabelHotspotJson {
    fn from_hotspot(hotspot: &crate::runtime::scene::TextLabelHotspot) -> Self {
        Self {
            line: hotspot.line,
            start: hotspot.start,
            end: hotspot.end,
            text: hotspot.text.clone(),
            action: LabelHotspotActionJson::from_action(&hotspot.action),
        }
    }
}

#[derive(Serialize, TS)]
#[serde(tag = "kind", rename_all = "kebab-case")]
enum LabelHotspotActionJson {
    Button {
        #[serde(rename = "buttonIndex")]
        button_index: usize,
    },
    Point {
        #[serde(rename = "pointIndex")]
        point_index: usize,
    },
    Segment {
        #[serde(rename = "startPointIndex")]
        start_point_index: usize,
        #[serde(rename = "endPointIndex")]
        end_point_index: usize,
    },
    AngleMarker {
        #[serde(rename = "startPointIndex")]
        start_point_index: usize,
        #[serde(rename = "vertexPointIndex")]
        vertex_point_index: usize,
        #[serde(rename = "endPointIndex")]
        end_point_index: usize,
    },
    Circle {
        #[serde(rename = "circleIndex")]
        circle_index: usize,
    },
    Polygon {
        #[serde(rename = "polygonIndex")]
        polygon_index: usize,
    },
}

impl LabelHotspotActionJson {
    fn from_action(action: &TextLabelHotspotAction) -> Self {
        match action {
            TextLabelHotspotAction::Button { button_index } => Self::Button {
                button_index: *button_index,
            },
            TextLabelHotspotAction::Point { point_index } => Self::Point {
                point_index: *point_index,
            },
            TextLabelHotspotAction::Segment {
                start_point_index,
                end_point_index,
            } => Self::Segment {
                start_point_index: *start_point_index,
                end_point_index: *end_point_index,
            },
            TextLabelHotspotAction::AngleMarker {
                start_point_index,
                vertex_point_index,
                end_point_index,
            } => Self::AngleMarker {
                start_point_index: *start_point_index,
                vertex_point_index: *vertex_point_index,
                end_point_index: *end_point_index,
            },
            TextLabelHotspotAction::Circle { circle_index } => Self::Circle {
                circle_index: *circle_index,
            },
            TextLabelHotspotAction::Polygon { polygon_index } => Self::Polygon {
                polygon_index: *polygon_index,
            },
        }
    }
}

#[derive(Serialize, TS)]
#[serde(tag = "kind")]
enum LabelBindingJson {
    #[serde(rename = "parameter-value")]
    ParameterValue { name: String },
    #[serde(rename = "function-label")]
    FunctionLabel {
        #[serde(rename = "functionKey")]
        function_key: usize,
        derivative: bool,
    },
    #[serde(rename = "expression-value")]
    ExpressionValue {
        #[serde(rename = "parameterName")]
        parameter_name: String,
        #[serde(rename = "resultName")]
        result_name: Option<String>,
        #[serde(rename = "exprLabel")]
        expr_label: String,
        expr: FunctionExprJson,
    },
    #[serde(rename = "point-bound-expression-value")]
    PointBoundExpressionValue {
        #[serde(rename = "pointIndex")]
        point_index: usize,
        #[serde(rename = "anchorDx")]
        anchor_dx: f64,
        #[serde(rename = "anchorDy")]
        anchor_dy: f64,
        #[serde(rename = "parameterName")]
        parameter_name: String,
        #[serde(rename = "resultName")]
        result_name: Option<String>,
        #[serde(rename = "exprLabel")]
        expr_label: String,
        expr: FunctionExprJson,
    },
    #[serde(rename = "point-anchor")]
    PointAnchor {
        #[serde(rename = "pointIndex")]
        point_index: usize,
        #[serde(rename = "anchorDx")]
        anchor_dx: f64,
        #[serde(rename = "anchorDy")]
        anchor_dy: f64,
        #[serde(rename = "anchorYPointIndex", skip_serializing_if = "Option::is_none")]
        anchor_y_point_index: Option<usize>,
        #[serde(rename = "anchorYDy", skip_serializing_if = "Option::is_none")]
        anchor_y_dy: Option<f64>,
    },
    #[serde(rename = "point-expression-value")]
    PointExpressionValue {
        #[serde(rename = "pointIndex")]
        point_index: usize,
        #[serde(rename = "anchorDx")]
        anchor_dx: f64,
        #[serde(rename = "anchorDy")]
        anchor_dy: f64,
        #[serde(rename = "anchorYPointIndex", skip_serializing_if = "Option::is_none")]
        anchor_y_point_index: Option<usize>,
        #[serde(rename = "anchorYDy", skip_serializing_if = "Option::is_none")]
        anchor_y_dy: Option<f64>,
        #[serde(rename = "parameterName")]
        parameter_name: String,
        expr: FunctionExprJson,
    },
    #[serde(rename = "sequence-expression-value")]
    SequenceExpressionValue {
        #[serde(rename = "parameterName")]
        parameter_name: String,
        expr: FunctionExprJson,
        depth: usize,
        #[serde(rename = "depthParameterName")]
        depth_parameter_name: Option<String>,
    },
    #[serde(rename = "rich-text-expression-values")]
    RichTextExpressionValues {
        #[serde(rename = "templateText")]
        template_text: String,
        #[serde(rename = "templateRichMarkup")]
        template_rich_markup: Option<String>,
        refs: Vec<RichTextExpressionRefJson>,
    },
    #[serde(rename = "point-coordinate-value")]
    PointCoordinateValue {
        #[serde(rename = "pointIndex")]
        point_index: usize,
        #[serde(rename = "pointName")]
        point_name: String,
        #[serde(rename = "originIndex", skip_serializing_if = "Option::is_none")]
        origin_index: Option<usize>,
        #[serde(rename = "xUnitIndex", skip_serializing_if = "Option::is_none")]
        x_unit_index: Option<usize>,
        #[serde(rename = "yUnitIndex", skip_serializing_if = "Option::is_none")]
        y_unit_index: Option<usize>,
    },
    #[serde(rename = "point-distance-value")]
    PointDistanceValue {
        #[serde(rename = "leftIndex")]
        left_index: usize,
        #[serde(rename = "rightIndex")]
        right_index: usize,
        name: String,
        #[serde(rename = "valueScale")]
        value_scale: f64,
        #[serde(rename = "valueSuffix")]
        value_suffix: String,
    },
    #[serde(rename = "point-angle-value")]
    PointAngleValue {
        #[serde(rename = "startIndex")]
        start_index: usize,
        #[serde(rename = "vertexIndex")]
        vertex_index: usize,
        #[serde(rename = "endIndex")]
        end_index: usize,
        name: String,
        #[serde(rename = "valueSuffix")]
        value_suffix: String,
    },
    #[serde(rename = "polygon-area-value")]
    PolygonAreaValue {
        #[serde(rename = "pointIndices")]
        point_indices: Vec<usize>,
        name: String,
        #[serde(rename = "valueScale")]
        value_scale: f64,
        #[serde(rename = "valueSuffix")]
        value_suffix: String,
    },
    #[serde(rename = "point-distance-ratio-value")]
    PointDistanceRatioValue {
        #[serde(rename = "originIndex")]
        origin_index: usize,
        #[serde(rename = "denominatorIndex")]
        denominator_index: usize,
        #[serde(rename = "numeratorIndex")]
        numerator_index: usize,
        name: String,
        #[serde(rename = "clampToUnit")]
        clamp_to_unit: bool,
    },
    #[serde(rename = "point-axis-value")]
    PointAxisValue {
        #[serde(rename = "pointIndex")]
        point_index: usize,
        name: String,
        axis: AxisJson,
        #[serde(rename = "originIndex", skip_serializing_if = "Option::is_none")]
        origin_index: Option<usize>,
        #[serde(rename = "xUnitIndex", skip_serializing_if = "Option::is_none")]
        x_unit_index: Option<usize>,
        #[serde(rename = "yUnitIndex", skip_serializing_if = "Option::is_none")]
        y_unit_index: Option<usize>,
    },
    #[serde(rename = "polygon-boundary-parameter")]
    PolygonBoundaryParameter {
        #[serde(rename = "pointIndex")]
        point_index: usize,
        #[serde(rename = "pointName")]
        point_name: String,
        #[serde(rename = "polygonName")]
        polygon_name: String,
    },
    #[serde(rename = "segment-parameter")]
    SegmentParameter {
        #[serde(rename = "pointIndex")]
        point_index: usize,
        #[serde(rename = "pointName")]
        point_name: String,
        #[serde(rename = "segmentName")]
        segment_name: String,
    },
    #[serde(rename = "segment-projection-parameter")]
    SegmentProjectionParameter {
        #[serde(rename = "pointIndex")]
        point_index: usize,
        #[serde(rename = "startIndex")]
        start_index: usize,
        #[serde(rename = "endIndex")]
        end_index: usize,
        #[serde(rename = "pointName")]
        point_name: String,
        #[serde(rename = "segmentName")]
        segment_name: String,
    },
    #[serde(rename = "polyline-parameter")]
    PolylineParameter {
        #[serde(rename = "pointIndex")]
        point_index: usize,
        #[serde(rename = "pointName")]
        point_name: String,
        #[serde(rename = "objectName")]
        object_name: String,
    },
    #[serde(rename = "circle-parameter")]
    CircleParameter {
        #[serde(rename = "pointIndex")]
        point_index: usize,
        #[serde(rename = "pointName")]
        point_name: String,
        #[serde(rename = "circleName")]
        circle_name: String,
    },
    #[serde(rename = "angle-marker-value")]
    AngleMarkerValue {
        #[serde(rename = "startIndex")]
        start_index: usize,
        #[serde(rename = "vertexIndex")]
        vertex_index: usize,
        #[serde(rename = "endIndex")]
        end_index: usize,
        decimals: usize,
    },
    #[serde(rename = "custom-transform-value")]
    CustomTransformValue {
        #[serde(rename = "pointIndex")]
        point_index: usize,
        #[serde(rename = "exprLabel")]
        expr_label: String,
        expr: FunctionExprJson,
        #[serde(rename = "valueScale")]
        value_scale: f64,
        #[serde(rename = "valueSuffix")]
        value_suffix: String,
    },
}

#[derive(Serialize, TS)]
#[serde(rename_all = "camelCase")]
struct RichTextExpressionRefJson {
    #[serde(rename = "sourceGroupOrdinal")]
    source_group_ordinal: usize,
    slot: usize,
    line: usize,
    start: usize,
    end: usize,
    expr: FunctionExprJson,
}

impl RichTextExpressionRefJson {
    fn from_ref(reference: &RichTextExpressionRef) -> Self {
        Self {
            source_group_ordinal: reference.source_group_ordinal,
            slot: reference.slot,
            line: reference.line,
            start: reference.start,
            end: reference.end,
            expr: FunctionExprJson::from_expr(&reference.expr),
        }
    }
}

impl LabelBindingJson {
    fn from_binding(binding: &TextLabelBinding) -> Self {
        match binding {
            TextLabelBinding::ParameterValue { name } => {
                Self::ParameterValue { name: name.clone() }
            }
            TextLabelBinding::FunctionLabel {
                function_key,
                derivative,
            } => Self::FunctionLabel {
                function_key: *function_key,
                derivative: *derivative,
            },
            TextLabelBinding::ExpressionValue {
                parameter_name,
                result_name,
                expr_label,
                expr,
            } => Self::ExpressionValue {
                parameter_name: parameter_name.clone(),
                result_name: result_name.clone(),
                expr_label: expr_label.clone(),
                expr: FunctionExprJson::from_expr(expr),
            },
            TextLabelBinding::PointBoundExpressionValue {
                point_index,
                anchor_dx,
                anchor_dy,
                parameter_name,
                result_name,
                expr_label,
                expr,
            } => Self::PointBoundExpressionValue {
                point_index: *point_index,
                anchor_dx: *anchor_dx,
                anchor_dy: *anchor_dy,
                parameter_name: parameter_name.clone(),
                result_name: result_name.clone(),
                expr_label: expr_label.clone(),
                expr: FunctionExprJson::from_expr(expr),
            },
            TextLabelBinding::PointAnchor {
                point_index,
                anchor_dx,
                anchor_dy,
                anchor_y_point_index,
                anchor_y_dy,
            } => Self::PointAnchor {
                point_index: *point_index,
                anchor_dx: *anchor_dx,
                anchor_dy: *anchor_dy,
                anchor_y_point_index: *anchor_y_point_index,
                anchor_y_dy: *anchor_y_dy,
            },
            TextLabelBinding::PointExpressionValue {
                point_index,
                anchor_dx,
                anchor_dy,
                anchor_y_point_index,
                anchor_y_dy,
                parameter_name,
                expr,
            } => Self::PointExpressionValue {
                point_index: *point_index,
                anchor_dx: *anchor_dx,
                anchor_dy: *anchor_dy,
                anchor_y_point_index: *anchor_y_point_index,
                anchor_y_dy: *anchor_y_dy,
                parameter_name: parameter_name.clone(),
                expr: FunctionExprJson::from_expr(expr),
            },
            TextLabelBinding::SequenceExpressionValue {
                parameter_name,
                expr,
                depth,
                depth_parameter_name,
            } => Self::SequenceExpressionValue {
                parameter_name: parameter_name.clone(),
                expr: FunctionExprJson::from_expr(expr),
                depth: *depth,
                depth_parameter_name: depth_parameter_name.clone(),
            },
            TextLabelBinding::RichTextExpressionValues {
                template_text,
                template_rich_markup,
                refs,
            } => Self::RichTextExpressionValues {
                template_text: template_text.clone(),
                template_rich_markup: template_rich_markup.clone(),
                refs: refs
                    .iter()
                    .map(RichTextExpressionRefJson::from_ref)
                    .collect(),
            },
            TextLabelBinding::PointCoordinateValue {
                point_index,
                point_name,
                origin_index,
                x_unit_index,
                y_unit_index,
            } => Self::PointCoordinateValue {
                point_index: *point_index,
                point_name: point_name.clone(),
                origin_index: *origin_index,
                x_unit_index: *x_unit_index,
                y_unit_index: *y_unit_index,
            },
            TextLabelBinding::PointDistanceValue {
                left_index,
                right_index,
                name,
                value_scale,
                value_suffix,
            } => Self::PointDistanceValue {
                left_index: *left_index,
                right_index: *right_index,
                name: name.clone(),
                value_scale: *value_scale,
                value_suffix: value_suffix.clone(),
            },
            TextLabelBinding::PointAngleValue {
                start_index,
                vertex_index,
                end_index,
                name,
                value_suffix,
            } => Self::PointAngleValue {
                start_index: *start_index,
                vertex_index: *vertex_index,
                end_index: *end_index,
                name: name.clone(),
                value_suffix: value_suffix.clone(),
            },
            TextLabelBinding::PolygonAreaValue {
                point_indices,
                name,
                value_scale,
                value_suffix,
            } => Self::PolygonAreaValue {
                point_indices: point_indices.clone(),
                name: name.clone(),
                value_scale: *value_scale,
                value_suffix: value_suffix.clone(),
            },
            TextLabelBinding::PointDistanceRatioValue {
                origin_index,
                denominator_index,
                numerator_index,
                name,
                clamp_to_unit,
            } => Self::PointDistanceRatioValue {
                origin_index: *origin_index,
                denominator_index: *denominator_index,
                numerator_index: *numerator_index,
                name: name.clone(),
                clamp_to_unit: *clamp_to_unit,
            },
            TextLabelBinding::PointAxisValue {
                point_index,
                name,
                axis,
                origin_index,
                x_unit_index,
                y_unit_index,
            } => Self::PointAxisValue {
                point_index: *point_index,
                name: name.clone(),
                axis: AxisJson::from_axis(*axis),
                origin_index: *origin_index,
                x_unit_index: *x_unit_index,
                y_unit_index: *y_unit_index,
            },
            TextLabelBinding::PolygonBoundaryParameter {
                point_index,
                point_name,
                polygon_name,
            } => Self::PolygonBoundaryParameter {
                point_index: *point_index,
                point_name: point_name.clone(),
                polygon_name: polygon_name.clone(),
            },
            TextLabelBinding::SegmentParameter {
                point_index,
                point_name,
                segment_name,
            } => Self::SegmentParameter {
                point_index: *point_index,
                point_name: point_name.clone(),
                segment_name: segment_name.clone(),
            },
            TextLabelBinding::SegmentProjectionParameter {
                point_index,
                start_index,
                end_index,
                point_name,
                segment_name,
            } => Self::SegmentProjectionParameter {
                point_index: *point_index,
                start_index: *start_index,
                end_index: *end_index,
                point_name: point_name.clone(),
                segment_name: segment_name.clone(),
            },
            TextLabelBinding::PolylineParameter {
                point_index,
                point_name,
                object_name,
            } => Self::PolylineParameter {
                point_index: *point_index,
                point_name: point_name.clone(),
                object_name: object_name.clone(),
            },
            TextLabelBinding::CircleParameter {
                point_index,
                point_name,
                circle_name,
            } => Self::CircleParameter {
                point_index: *point_index,
                point_name: point_name.clone(),
                circle_name: circle_name.clone(),
            },
            TextLabelBinding::AngleMarkerValue {
                start_index,
                vertex_index,
                end_index,
                decimals,
            } => Self::AngleMarkerValue {
                start_index: *start_index,
                vertex_index: *vertex_index,
                end_index: *end_index,
                decimals: *decimals,
            },
            TextLabelBinding::CustomTransformValue {
                point_index,
                expr_label,
                expr,
                value_scale,
                value_suffix,
            } => Self::CustomTransformValue {
                point_index: *point_index,
                expr_label: expr_label.clone(),
                expr: FunctionExprJson::from_expr(expr),
                value_scale: *value_scale,
                value_suffix: value_suffix.clone(),
            },
        }
    }
}

#[derive(Serialize, TS)]
#[serde(rename_all = "kebab-case")]
enum AxisJson {
    Horizontal,
    Vertical,
}

impl AxisJson {
    fn from_axis(axis: crate::runtime::scene::CoordinateAxis) -> Self {
        match axis {
            crate::runtime::scene::CoordinateAxis::Horizontal => Self::Horizontal,
            crate::runtime::scene::CoordinateAxis::Vertical => Self::Vertical,
        }
    }
}
