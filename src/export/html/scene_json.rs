use crate::format::PointRecord;
use crate::runtime::functions::{BinaryOp, FunctionExpr, FunctionTerm, UnaryFunction};
use crate::runtime::geometry::darken;
use crate::runtime::scene::{
    ButtonAction, LineBinding, Scene, SceneButton, ScenePointBinding, ScenePointConstraint,
    ShapeBinding, TextLabelBinding,
};
use serde::Serialize;

pub(super) fn scene_to_json(scene: &Scene, width: u32, height: u32) -> String {
    serde_json::to_string(&SceneJson::from_scene(scene, width, height))
        .expect("scene JSON serialization should succeed")
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SceneJson {
    width: u32,
    height: u32,
    graph_mode: bool,
    pi_mode: bool,
    saved_viewport: bool,
    y_up: bool,
    bounds: BoundsJson,
    origin: Option<PointJson>,
    lines: Vec<LineJson>,
    polygons: Vec<PolygonJson>,
    circles: Vec<CircleJson>,
    labels: Vec<LabelJson>,
    points: Vec<ScenePointJson>,
    buttons: Vec<ButtonJson>,
    parameters: Vec<ParameterJson>,
    functions: Vec<FunctionJson>,
}

impl SceneJson {
    fn from_scene(scene: &Scene, width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            graph_mode: scene.graph_mode,
            pi_mode: scene.pi_mode,
            saved_viewport: scene.saved_viewport,
            y_up: scene.y_up,
            bounds: BoundsJson::from_scene(scene),
            origin: scene.origin.as_ref().map(PointJson::from_point),
            lines: scene.lines.iter().map(LineJson::from_line).collect(),
            polygons: scene
                .polygons
                .iter()
                .map(PolygonJson::from_polygon)
                .collect(),
            circles: scene.circles.iter().map(CircleJson::from_circle).collect(),
            labels: scene.labels.iter().map(LabelJson::from_label).collect(),
            points: scene
                .points
                .iter()
                .map(ScenePointJson::from_scene_point)
                .collect(),
            buttons: scene.buttons.iter().map(ButtonJson::from_button).collect(),
            parameters: scene
                .parameters
                .iter()
                .map(ParameterJson::from_parameter)
                .collect(),
            functions: scene
                .functions
                .iter()
                .map(FunctionJson::from_function)
                .collect(),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ButtonJson {
    text: String,
    x: f64,
    y: f64,
    width: Option<f64>,
    height: Option<f64>,
    action: ButtonActionJson,
}

impl ButtonJson {
    fn from_button(button: &SceneButton) -> Self {
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

#[derive(Serialize)]
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

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BoundsJson {
    min_x: f64,
    max_x: f64,
    min_y: f64,
    max_y: f64,
}

impl BoundsJson {
    fn from_scene(scene: &Scene) -> Self {
        Self {
            min_x: scene.bounds.min_x,
            max_x: scene.bounds.max_x,
            min_y: scene.bounds.min_y,
            max_y: scene.bounds.max_y,
        }
    }
}

#[derive(Serialize)]
struct PointJson {
    x: f64,
    y: f64,
}

impl PointJson {
    fn from_point(point: &PointRecord) -> Self {
        Self {
            x: point.x,
            y: point.y,
        }
    }
}

#[derive(Serialize)]
struct LineJson {
    points: Vec<PointJson>,
    color: [u8; 4],
    dashed: bool,
    binding: Option<LineBindingJson>,
}

impl LineJson {
    fn from_line(line: &crate::runtime::scene::LineShape) -> Self {
        Self {
            points: line.points.iter().map(PointJson::from_point).collect(),
            color: line.color,
            dashed: line.dashed,
            binding: line.binding.as_ref().map(LineBindingJson::from_binding),
        }
    }
}

#[derive(Serialize)]
#[serde(tag = "kind")]
enum LineBindingJson {
    #[serde(rename = "line")]
    Line {
        #[serde(rename = "startIndex")]
        start_index: usize,
        #[serde(rename = "endIndex")]
        end_index: usize,
    },
    #[serde(rename = "ray")]
    Ray {
        #[serde(rename = "startIndex")]
        start_index: usize,
        #[serde(rename = "endIndex")]
        end_index: usize,
    },
    #[serde(rename = "translate-line")]
    TranslateLine {
        #[serde(rename = "sourceIndex")]
        source_index: usize,
        #[serde(rename = "vectorStartIndex")]
        vector_start_index: usize,
        #[serde(rename = "vectorEndIndex")]
        vector_end_index: usize,
    },
    #[serde(rename = "rotate-line")]
    RotateLine {
        #[serde(rename = "sourceIndex")]
        source_index: usize,
        #[serde(rename = "centerIndex")]
        center_index: usize,
        #[serde(rename = "angleDegrees")]
        angle_degrees: f64,
    },
    #[serde(rename = "scale-line")]
    ScaleLine {
        #[serde(rename = "sourceIndex")]
        source_index: usize,
        #[serde(rename = "centerIndex")]
        center_index: usize,
        factor: f64,
    },
    #[serde(rename = "reflect-line")]
    ReflectLine {
        #[serde(rename = "sourceIndex")]
        source_index: usize,
        #[serde(rename = "lineStartIndex")]
        line_start_index: usize,
        #[serde(rename = "lineEndIndex")]
        line_end_index: usize,
    },
    #[serde(rename = "rotate-edge")]
    RotateEdge {
        #[serde(rename = "centerIndex")]
        center_index: usize,
        #[serde(rename = "vertexIndex")]
        vertex_index: usize,
        #[serde(rename = "parameterName")]
        parameter_name: String,
        #[serde(rename = "angleExpr")]
        angle_expr: FunctionExprJson,
        #[serde(rename = "startStep")]
        start_step: usize,
        #[serde(rename = "endStep")]
        end_step: usize,
    },
}

impl LineBindingJson {
    fn from_binding(binding: &LineBinding) -> Self {
        match binding {
            LineBinding::Line {
                start_index,
                end_index,
            } => Self::Line {
                start_index: *start_index,
                end_index: *end_index,
            },
            LineBinding::Ray {
                start_index,
                end_index,
            } => Self::Ray {
                start_index: *start_index,
                end_index: *end_index,
            },
            LineBinding::TranslateLine {
                source_index,
                vector_start_index,
                vector_end_index,
            } => Self::TranslateLine {
                source_index: *source_index,
                vector_start_index: *vector_start_index,
                vector_end_index: *vector_end_index,
            },
            LineBinding::RotateLine {
                source_index,
                center_index,
                angle_degrees,
            } => Self::RotateLine {
                source_index: *source_index,
                center_index: *center_index,
                angle_degrees: *angle_degrees,
            },
            LineBinding::ScaleLine {
                source_index,
                center_index,
                factor,
            } => Self::ScaleLine {
                source_index: *source_index,
                center_index: *center_index,
                factor: *factor,
            },
            LineBinding::ReflectLine {
                source_index,
                line_start_index,
                line_end_index,
            } => Self::ReflectLine {
                source_index: *source_index,
                line_start_index: *line_start_index,
                line_end_index: *line_end_index,
            },
            LineBinding::RotateEdge {
                center_index,
                vertex_index,
                parameter_name,
                angle_expr,
                start_step,
                end_step,
            } => Self::RotateEdge {
                center_index: *center_index,
                vertex_index: *vertex_index,
                parameter_name: parameter_name.clone(),
                angle_expr: FunctionExprJson::from_expr(angle_expr),
                start_step: *start_step,
                end_step: *end_step,
            },
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PolygonJson {
    points: Vec<PointJson>,
    color: [u8; 4],
    outline_color: [u8; 4],
    binding: Option<ShapeBindingJson>,
}

impl PolygonJson {
    fn from_polygon(polygon: &crate::runtime::scene::PolygonShape) -> Self {
        Self {
            points: polygon.points.iter().map(PointJson::from_point).collect(),
            color: polygon.color,
            outline_color: darken(polygon.color, 80),
            binding: polygon.binding.as_ref().map(ShapeBindingJson::from_binding),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CircleJson {
    center: PointJson,
    radius_point: PointJson,
    color: [u8; 4],
    binding: Option<ShapeBindingJson>,
}

impl CircleJson {
    fn from_circle(circle: &crate::runtime::scene::SceneCircle) -> Self {
        Self {
            center: PointJson::from_point(&circle.center),
            radius_point: PointJson::from_point(&circle.radius_point),
            color: circle.color,
            binding: circle.binding.as_ref().map(ShapeBindingJson::from_binding),
        }
    }
}

#[derive(Serialize)]
#[serde(tag = "kind")]
enum ShapeBindingJson {
    #[serde(rename = "translate-polygon")]
    TranslatePolygon {
        #[serde(rename = "sourceIndex")]
        source_index: usize,
        #[serde(rename = "vectorStartIndex")]
        vector_start_index: usize,
        #[serde(rename = "vectorEndIndex")]
        vector_end_index: usize,
    },
    #[serde(rename = "translate-circle")]
    TranslateCircle {
        #[serde(rename = "sourceIndex")]
        source_index: usize,
        #[serde(rename = "vectorStartIndex")]
        vector_start_index: usize,
        #[serde(rename = "vectorEndIndex")]
        vector_end_index: usize,
    },
    #[serde(rename = "rotate-polygon")]
    RotatePolygon {
        #[serde(rename = "sourceIndex")]
        source_index: usize,
        #[serde(rename = "centerIndex")]
        center_index: usize,
        #[serde(rename = "angleDegrees")]
        angle_degrees: f64,
    },
    #[serde(rename = "rotate-circle")]
    RotateCircle {
        #[serde(rename = "sourceIndex")]
        source_index: usize,
        #[serde(rename = "centerIndex")]
        center_index: usize,
        #[serde(rename = "angleDegrees")]
        angle_degrees: f64,
    },
    #[serde(rename = "scale-polygon")]
    ScalePolygon {
        #[serde(rename = "sourceIndex")]
        source_index: usize,
        #[serde(rename = "centerIndex")]
        center_index: usize,
        factor: f64,
    },
    #[serde(rename = "scale-circle")]
    ScaleCircle {
        #[serde(rename = "sourceIndex")]
        source_index: usize,
        #[serde(rename = "centerIndex")]
        center_index: usize,
        factor: f64,
    },
    #[serde(rename = "reflect-polygon")]
    ReflectPolygon {
        #[serde(rename = "sourceIndex")]
        source_index: usize,
        #[serde(rename = "lineStartIndex")]
        line_start_index: usize,
        #[serde(rename = "lineEndIndex")]
        line_end_index: usize,
    },
    #[serde(rename = "reflect-circle")]
    ReflectCircle {
        #[serde(rename = "sourceIndex")]
        source_index: usize,
        #[serde(rename = "lineStartIndex")]
        line_start_index: usize,
        #[serde(rename = "lineEndIndex")]
        line_end_index: usize,
    },
}

impl ShapeBindingJson {
    fn from_binding(binding: &ShapeBinding) -> Self {
        match binding {
            ShapeBinding::TranslatePolygon {
                source_index,
                vector_start_index,
                vector_end_index,
            } => Self::TranslatePolygon {
                source_index: *source_index,
                vector_start_index: *vector_start_index,
                vector_end_index: *vector_end_index,
            },
            ShapeBinding::TranslateCircle {
                source_index,
                vector_start_index,
                vector_end_index,
            } => Self::TranslateCircle {
                source_index: *source_index,
                vector_start_index: *vector_start_index,
                vector_end_index: *vector_end_index,
            },
            ShapeBinding::RotatePolygon {
                source_index,
                center_index,
                angle_degrees,
            } => Self::RotatePolygon {
                source_index: *source_index,
                center_index: *center_index,
                angle_degrees: *angle_degrees,
            },
            ShapeBinding::RotateCircle {
                source_index,
                center_index,
                angle_degrees,
            } => Self::RotateCircle {
                source_index: *source_index,
                center_index: *center_index,
                angle_degrees: *angle_degrees,
            },
            ShapeBinding::ScalePolygon {
                source_index,
                center_index,
                factor,
            } => Self::ScalePolygon {
                source_index: *source_index,
                center_index: *center_index,
                factor: *factor,
            },
            ShapeBinding::ScaleCircle {
                source_index,
                center_index,
                factor,
            } => Self::ScaleCircle {
                source_index: *source_index,
                center_index: *center_index,
                factor: *factor,
            },
            ShapeBinding::ReflectPolygon {
                source_index,
                line_start_index,
                line_end_index,
            } => Self::ReflectPolygon {
                source_index: *source_index,
                line_start_index: *line_start_index,
                line_end_index: *line_end_index,
            },
            ShapeBinding::ReflectCircle {
                source_index,
                line_start_index,
                line_end_index,
            } => Self::ReflectCircle {
                source_index: *source_index,
                line_start_index: *line_start_index,
                line_end_index: *line_end_index,
            },
        }
    }
}

#[derive(Serialize)]
struct LabelJson {
    anchor: PointJson,
    text: String,
    color: [u8; 4],
    binding: Option<LabelBindingJson>,
    #[serde(rename = "screenSpace")]
    screen_space: bool,
}

impl LabelJson {
    fn from_label(label: &crate::runtime::scene::TextLabel) -> Self {
        Self {
            anchor: PointJson::from_point(&label.anchor),
            text: label.text.clone(),
            color: label.color,
            binding: label.binding.as_ref().map(LabelBindingJson::from_binding),
            screen_space: label.screen_space,
        }
    }
}

#[derive(Serialize)]
#[serde(tag = "kind")]
enum LabelBindingJson {
    #[serde(rename = "parameter-value")]
    ParameterValue { name: String },
    #[serde(rename = "expression-value")]
    ExpressionValue {
        #[serde(rename = "parameterName")]
        parameter_name: String,
        #[serde(rename = "exprLabel")]
        expr_label: String,
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
}

impl LabelBindingJson {
    fn from_binding(binding: &TextLabelBinding) -> Self {
        match binding {
            TextLabelBinding::ParameterValue { name } => {
                Self::ParameterValue { name: name.clone() }
            }
            TextLabelBinding::ExpressionValue {
                parameter_name,
                expr_label,
                expr,
            } => Self::ExpressionValue {
                parameter_name: parameter_name.clone(),
                expr_label: expr_label.clone(),
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
        }
    }
}

#[derive(Serialize)]
struct ScenePointJson {
    x: f64,
    y: f64,
    constraint: Option<PointConstraintJson>,
    binding: Option<PointBindingJson>,
}

impl ScenePointJson {
    fn from_scene_point(point: &crate::runtime::scene::ScenePoint) -> Self {
        Self {
            x: point.position.x,
            y: point.position.y,
            constraint: PointConstraintJson::from_constraint(&point.constraint),
            binding: point.binding.as_ref().map(PointBindingJson::from_binding),
        }
    }
}

#[derive(Serialize)]
#[serde(tag = "kind")]
enum PointBindingJson {
    #[serde(rename = "parameter")]
    Parameter { name: String },
    #[serde(rename = "derived-parameter")]
    DerivedParameter {
        #[serde(rename = "sourceIndex")]
        source_index: usize,
    },
    #[serde(rename = "translate")]
    Translate {
        #[serde(rename = "sourceIndex")]
        source_index: usize,
        #[serde(rename = "vectorStartIndex")]
        vector_start_index: usize,
        #[serde(rename = "vectorEndIndex")]
        vector_end_index: usize,
    },
    #[serde(rename = "reflect")]
    Reflect {
        #[serde(rename = "sourceIndex")]
        source_index: usize,
        #[serde(rename = "lineStartIndex")]
        line_start_index: usize,
        #[serde(rename = "lineEndIndex")]
        line_end_index: usize,
    },
    #[serde(rename = "rotate")]
    Rotate {
        #[serde(rename = "sourceIndex")]
        source_index: usize,
        #[serde(rename = "centerIndex")]
        center_index: usize,
        #[serde(rename = "angleDegrees")]
        angle_degrees: f64,
    },
    #[serde(rename = "scale")]
    Scale {
        #[serde(rename = "sourceIndex")]
        source_index: usize,
        #[serde(rename = "centerIndex")]
        center_index: usize,
        factor: f64,
    },
    #[serde(rename = "coordinate")]
    Coordinate {
        name: String,
        expr: FunctionExprJson,
    },
}

impl PointBindingJson {
    fn from_binding(binding: &ScenePointBinding) -> Self {
        match binding {
            ScenePointBinding::Parameter { name } => Self::Parameter { name: name.clone() },
            ScenePointBinding::DerivedParameter { source_index } => Self::DerivedParameter {
                source_index: *source_index,
            },
            ScenePointBinding::Translate {
                source_index,
                vector_start_index,
                vector_end_index,
            } => Self::Translate {
                source_index: *source_index,
                vector_start_index: *vector_start_index,
                vector_end_index: *vector_end_index,
            },
            ScenePointBinding::Reflect {
                source_index,
                line_start_index,
                line_end_index,
            } => Self::Reflect {
                source_index: *source_index,
                line_start_index: *line_start_index,
                line_end_index: *line_end_index,
            },
            ScenePointBinding::Rotate {
                source_index,
                center_index,
                angle_degrees,
            } => Self::Rotate {
                source_index: *source_index,
                center_index: *center_index,
                angle_degrees: *angle_degrees,
            },
            ScenePointBinding::Scale {
                source_index,
                center_index,
                factor,
            } => Self::Scale {
                source_index: *source_index,
                center_index: *center_index,
                factor: *factor,
            },
            ScenePointBinding::Coordinate { name, expr } => Self::Coordinate {
                name: name.clone(),
                expr: FunctionExprJson::from_expr(expr),
            },
        }
    }
}

#[derive(Serialize)]
#[serde(tag = "kind")]
enum PointConstraintJson {
    #[serde(rename = "offset")]
    Offset {
        #[serde(rename = "originIndex")]
        origin_index: usize,
        dx: f64,
        dy: f64,
    },
    #[serde(rename = "segment")]
    Segment {
        #[serde(rename = "startIndex")]
        start_index: usize,
        #[serde(rename = "endIndex")]
        end_index: usize,
        t: f64,
    },
    #[serde(rename = "polyline")]
    Polyline {
        #[serde(rename = "functionKey")]
        function_key: usize,
        points: Vec<PointJson>,
        #[serde(rename = "segmentIndex")]
        segment_index: usize,
        t: f64,
    },
    #[serde(rename = "polygon-boundary")]
    PolygonBoundary {
        #[serde(rename = "vertexIndices")]
        vertex_indices: Vec<usize>,
        #[serde(rename = "edgeIndex")]
        edge_index: usize,
        t: f64,
    },
    #[serde(rename = "circle")]
    Circle {
        #[serde(rename = "centerIndex")]
        center_index: usize,
        #[serde(rename = "radiusIndex")]
        radius_index: usize,
        #[serde(rename = "unitX")]
        unit_x: f64,
        #[serde(rename = "unitY")]
        unit_y: f64,
    },
}

impl PointConstraintJson {
    fn from_constraint(constraint: &ScenePointConstraint) -> Option<Self> {
        match constraint {
            ScenePointConstraint::Free => None,
            ScenePointConstraint::Offset {
                origin_index,
                dx,
                dy,
            } => Some(Self::Offset {
                origin_index: *origin_index,
                dx: *dx,
                dy: *dy,
            }),
            ScenePointConstraint::OnSegment {
                start_index,
                end_index,
                t,
            } => Some(Self::Segment {
                start_index: *start_index,
                end_index: *end_index,
                t: *t,
            }),
            ScenePointConstraint::OnPolyline {
                function_key,
                points,
                segment_index,
                t,
            } => Some(Self::Polyline {
                function_key: *function_key,
                points: points.iter().map(PointJson::from_point).collect(),
                segment_index: *segment_index,
                t: *t,
            }),
            ScenePointConstraint::OnPolygonBoundary {
                vertex_indices,
                edge_index,
                t,
            } => Some(Self::PolygonBoundary {
                vertex_indices: vertex_indices.clone(),
                edge_index: *edge_index,
                t: *t,
            }),
            ScenePointConstraint::OnCircle {
                center_index,
                radius_index,
                unit_x,
                unit_y,
            } => Some(Self::Circle {
                center_index: *center_index,
                radius_index: *radius_index,
                unit_x: *unit_x,
                unit_y: *unit_y,
            }),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ParameterJson {
    name: String,
    value: f64,
    label_index: Option<usize>,
}

impl ParameterJson {
    fn from_parameter(parameter: &crate::runtime::scene::SceneParameter) -> Self {
        Self {
            name: parameter.name.clone(),
            value: parameter.value,
            label_index: parameter.label_index,
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FunctionJson {
    key: usize,
    name: String,
    derivative: bool,
    domain: DomainJson,
    line_index: Option<usize>,
    label_index: usize,
    constrained_point_indices: Vec<usize>,
    expr: FunctionExprJson,
}

impl FunctionJson {
    fn from_function(function_def: &crate::runtime::scene::SceneFunction) -> Self {
        Self {
            key: function_def.key,
            name: function_def.name.clone(),
            derivative: function_def.derivative,
            domain: DomainJson {
                x_min: function_def.domain.x_min,
                x_max: function_def.domain.x_max,
                sample_count: function_def.domain.sample_count,
            },
            line_index: function_def.line_index,
            label_index: function_def.label_index,
            constrained_point_indices: function_def.constrained_point_indices.clone(),
            expr: FunctionExprJson::from_expr(&function_def.expr),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DomainJson {
    x_min: f64,
    x_max: f64,
    sample_count: usize,
}

#[derive(Serialize)]
#[serde(tag = "kind")]
enum FunctionExprJson {
    #[serde(rename = "constant")]
    Constant { value: f64 },
    #[serde(rename = "identity")]
    Identity,
    #[serde(rename = "parsed")]
    Parsed {
        head: FunctionTermJson,
        tail: Vec<ExprTailJson>,
    },
}

impl FunctionExprJson {
    fn from_expr(expr: &FunctionExpr) -> Self {
        match expr {
            FunctionExpr::Constant(value) => Self::Constant { value: *value },
            FunctionExpr::Identity => Self::Identity,
            FunctionExpr::SinIdentity => Self::Parsed {
                head: FunctionTermJson::UnaryX { op: "sin" },
                tail: Vec::new(),
            },
            FunctionExpr::CosIdentityPlus(offset) => Self::Parsed {
                head: FunctionTermJson::UnaryX { op: "cos" },
                tail: vec![ExprTailJson {
                    op: "add",
                    term: FunctionTermJson::Constant { value: *offset },
                }],
            },
            FunctionExpr::TanIdentityMinus(offset) => Self::Parsed {
                head: FunctionTermJson::UnaryX { op: "tan" },
                tail: vec![ExprTailJson {
                    op: "sub",
                    term: FunctionTermJson::Constant { value: *offset },
                }],
            },
            FunctionExpr::Parsed(parsed) => Self::Parsed {
                head: FunctionTermJson::from_term(&parsed.head),
                tail: parsed
                    .tail
                    .iter()
                    .map(|(op, term)| ExprTailJson {
                        op: binary_op_name(*op),
                        term: FunctionTermJson::from_term(term),
                    })
                    .collect(),
            },
        }
    }
}

#[derive(Serialize)]
struct ExprTailJson {
    op: &'static str,
    term: FunctionTermJson,
}

#[derive(Serialize)]
#[serde(tag = "kind")]
enum FunctionTermJson {
    #[serde(rename = "variable")]
    Variable,
    #[serde(rename = "constant")]
    Constant { value: f64 },
    #[serde(rename = "parameter")]
    Parameter { name: String, value: f64 },
    #[serde(rename = "unary_x")]
    UnaryX { op: &'static str },
    #[serde(rename = "product")]
    Product {
        left: Box<FunctionTermJson>,
        right: Box<FunctionTermJson>,
    },
}

impl FunctionTermJson {
    fn from_term(term: &FunctionTerm) -> Self {
        match term {
            FunctionTerm::Variable => Self::Variable,
            FunctionTerm::Constant(value) => Self::Constant { value: *value },
            FunctionTerm::Parameter(name, value) => Self::Parameter {
                name: name.clone(),
                value: *value,
            },
            FunctionTerm::UnaryX(op) => Self::UnaryX {
                op: unary_function_name(*op),
            },
            FunctionTerm::Product(left, right) => Self::Product {
                left: Box::new(Self::from_term(left)),
                right: Box::new(Self::from_term(right)),
            },
        }
    }
}

fn binary_op_name(op: BinaryOp) -> &'static str {
    match op {
        BinaryOp::Add => "add",
        BinaryOp::Sub => "sub",
        BinaryOp::Mul => "mul",
        BinaryOp::Div => "div",
    }
}

fn unary_function_name(op: UnaryFunction) -> &'static str {
    match op {
        UnaryFunction::Sin => "sin",
        UnaryFunction::Cos => "cos",
        UnaryFunction::Tan => "tan",
        UnaryFunction::Abs => "abs",
        UnaryFunction::Sqrt => "sqrt",
        UnaryFunction::Ln => "ln",
        UnaryFunction::Log10 => "log10",
        UnaryFunction::Sign => "sign",
        UnaryFunction::Round => "round",
        UnaryFunction::Trunc => "trunc",
    }
}
