use super::function_expr_json::FunctionExprJson;
use super::scene_json::PointJson;
use crate::runtime::scene::{ButtonAction, SceneButton, TextLabelBinding, TextLabelHotspotAction};
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
    action: ButtonActionJson,
}

impl ButtonJson {
    pub(super) fn from_button(button: &SceneButton) -> Self {
        Self {
            text: button.text.clone(),
            x: button.anchor.x,
            y: button.anchor.y,
            width: button.rect.as_ref().map(|rect| rect.width),
            height: button.rect.as_ref().map(|rect| rect.height),
            action: ButtonActionJson::from_action(&button.action),
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
    AnimatePoint {
        #[serde(rename = "pointIndex")]
        point_index: usize,
    },
    ScrollPoint {
        #[serde(rename = "pointIndex")]
        point_index: usize,
    },
    Sequence {
        #[serde(rename = "buttonIndices")]
        button_indices: Vec<usize>,
        #[serde(rename = "intervalMs")]
        interval_ms: u32,
    },
}

impl ButtonActionJson {
    fn from_action(action: &ButtonAction) -> Self {
        match action {
            ButtonAction::Link { href } => Self::Link { href: href.clone() },
            ButtonAction::ToggleVisibility {
                point_indices,
                line_indices,
                circle_indices,
                polygon_indices,
            } => Self::ToggleVisibility {
                point_indices: point_indices.clone(),
                line_indices: line_indices.clone(),
                circle_indices: circle_indices.clone(),
                polygon_indices: polygon_indices.clone(),
            },
            ButtonAction::SetVisibility {
                visible,
                point_indices,
                line_indices,
                circle_indices,
                polygon_indices,
            } => Self::SetVisibility {
                visible: *visible,
                point_indices: point_indices.clone(),
                line_indices: line_indices.clone(),
                circle_indices: circle_indices.clone(),
                polygon_indices: polygon_indices.clone(),
            },
            ButtonAction::ShowHideVisibility {
                point_indices,
                line_indices,
                circle_indices,
                polygon_indices,
            } => Self::ShowHideVisibility {
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
            ButtonAction::AnimatePoint { point_index } => Self::AnimatePoint {
                point_index: *point_index,
            },
            ButtonAction::ScrollPoint { point_index } => Self::ScrollPoint {
                point_index: *point_index,
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
    #[serde(rename = "point-expression-value")]
    PointExpressionValue {
        #[serde(rename = "pointIndex")]
        point_index: usize,
        #[serde(rename = "parameterName")]
        parameter_name: String,
        expr: FunctionExprJson,
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
    #[serde(rename = "polygon-boundary-expression")]
    PolygonBoundaryExpression {
        #[serde(rename = "pointIndex")]
        point_index: usize,
        #[serde(rename = "parameterName")]
        parameter_name: String,
        #[serde(rename = "exprLabel")]
        expr_label: String,
        expr: FunctionExprJson,
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
            TextLabelBinding::PointExpressionValue {
                point_index,
                parameter_name,
                expr,
            } => Self::PointExpressionValue {
                point_index: *point_index,
                parameter_name: parameter_name.clone(),
                expr: FunctionExprJson::from_expr(expr),
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
            TextLabelBinding::PolygonBoundaryExpression {
                point_index,
                parameter_name,
                expr_label,
                expr,
            } => Self::PolygonBoundaryExpression {
                point_index: *point_index,
                parameter_name: parameter_name.clone(),
                expr_label: expr_label.clone(),
                expr: FunctionExprJson::from_expr(expr),
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
