use crate::format::PointRecord;
use crate::runtime::functions::{
    BinaryOp, FunctionExpr, FunctionPlotMode, FunctionTerm, UnaryFunction,
};
use crate::runtime::geometry::darken;
use crate::runtime::scene::{
    ArcBoundaryKind, ButtonAction, CircularConstraint, IterationPointHandle, LabelIterationFamily,
    LineBinding, LineConstraint, LineIterationFamily, PointIterationFamily,
    PolygonIterationFamily, Scene, SceneButton, ScenePointBinding, ScenePointConstraint,
    ShapeBinding, TextLabelBinding,
    TextLabelHotspotAction,
};
use serde::Serialize;

pub(super) fn scene_to_json(scene: &Scene, width: u32, height: u32, pretty: bool) -> String {
    if pretty {
        serde_json::to_string_pretty(&SceneJson::from_scene(scene, width, height))
    } else {
        serde_json::to_string(&SceneJson::from_scene(scene, width, height))
    }
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
    images: Vec<ImageJson>,
    lines: Vec<LineJson>,
    polygons: Vec<PolygonJson>,
    circles: Vec<CircleJson>,
    arcs: Vec<ArcJson>,
    labels: Vec<LabelJson>,
    points: Vec<ScenePointJson>,
    point_iterations: Vec<PointIterationJson>,
    line_iterations: Vec<LineIterationJson>,
    polygon_iterations: Vec<PolygonIterationJson>,
    label_iterations: Vec<LabelIterationJson>,
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
            images: scene.images.iter().map(ImageJson::from_image).collect(),
            lines: scene.lines.iter().map(LineJson::from_line).collect(),
            polygons: scene
                .polygons
                .iter()
                .map(PolygonJson::from_polygon)
                .collect(),
            circles: scene.circles.iter().map(CircleJson::from_circle).collect(),
            arcs: scene.arcs.iter().map(ArcJson::from_arc).collect(),
            labels: scene.labels.iter().map(LabelJson::from_label).collect(),
            points: scene
                .points
                .iter()
                .map(ScenePointJson::from_scene_point)
                .collect(),
            point_iterations: scene
                .point_iterations
                .iter()
                .map(PointIterationJson::from_family)
                .collect(),
            line_iterations: scene
                .line_iterations
                .iter()
                .map(LineIterationJson::from_family)
                .collect(),
            polygon_iterations: scene
                .polygon_iterations
                .iter()
                .map(PolygonIterationJson::from_family)
                .collect(),
            label_iterations: scene
                .label_iterations
                .iter()
                .map(LabelIterationJson::from_family)
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
struct ImageJson {
    top_left: PointJson,
    bottom_right: PointJson,
    src: String,
    screen_space: bool,
}

impl ImageJson {
    fn from_image(image: &crate::runtime::scene::SceneImage) -> Self {
        Self {
            top_left: PointJson::from_point(&image.top_left),
            bottom_right: PointJson::from_point(&image.bottom_right),
            src: image.src.clone(),
            screen_space: image.screen_space,
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
    visible: bool,
    binding: Option<LineBindingJson>,
}

impl LineJson {
    fn from_line(line: &crate::runtime::scene::LineShape) -> Self {
        Self {
            points: line.points.iter().map(PointJson::from_point).collect(),
            color: line.color,
            dashed: line.dashed,
            visible: line.visible,
            binding: line.binding.as_ref().map(LineBindingJson::from_binding),
        }
    }
}

#[derive(Serialize)]
#[serde(tag = "kind")]
enum LineBindingJson {
    #[serde(rename = "graph-helper-line")]
    GraphHelperLine {
        #[serde(rename = "startIndex")]
        start_index: usize,
        #[serde(rename = "endIndex")]
        end_index: usize,
    },
    #[serde(rename = "segment")]
    Segment {
        #[serde(rename = "startIndex")]
        start_index: usize,
        #[serde(rename = "endIndex")]
        end_index: usize,
    },
    #[serde(rename = "angle-marker")]
    AngleMarker {
        #[serde(rename = "startIndex")]
        start_index: usize,
        #[serde(rename = "vertexIndex")]
        vertex_index: usize,
        #[serde(rename = "endIndex")]
        end_index: usize,
        #[serde(rename = "markerClass")]
        marker_class: u32,
    },
    #[serde(rename = "segment-marker")]
    SegmentMarker {
        #[serde(rename = "startIndex")]
        start_index: usize,
        #[serde(rename = "endIndex")]
        end_index: usize,
        t: f64,
        #[serde(rename = "markerClass")]
        marker_class: u32,
    },
    #[serde(rename = "angle-bisector-ray")]
    AngleBisectorRay {
        #[serde(rename = "startIndex")]
        start_index: usize,
        #[serde(rename = "vertexIndex")]
        vertex_index: usize,
        #[serde(rename = "endIndex")]
        end_index: usize,
    },
    #[serde(rename = "perpendicular-line")]
    PerpendicularLine {
        #[serde(rename = "throughIndex")]
        through_index: usize,
        #[serde(rename = "lineStartIndex", skip_serializing_if = "Option::is_none")]
        line_start_index: Option<usize>,
        #[serde(rename = "lineEndIndex", skip_serializing_if = "Option::is_none")]
        line_end_index: Option<usize>,
        #[serde(rename = "lineIndex", skip_serializing_if = "Option::is_none")]
        line_index: Option<usize>,
    },
    #[serde(rename = "parallel-line")]
    ParallelLine {
        #[serde(rename = "throughIndex")]
        through_index: usize,
        #[serde(rename = "lineStartIndex", skip_serializing_if = "Option::is_none")]
        line_start_index: Option<usize>,
        #[serde(rename = "lineEndIndex", skip_serializing_if = "Option::is_none")]
        line_end_index: Option<usize>,
        #[serde(rename = "lineIndex", skip_serializing_if = "Option::is_none")]
        line_index: Option<usize>,
    },
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
        #[serde(rename = "parameterName")]
        parameter_name: Option<String>,
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
    #[serde(rename = "custom-transform-trace")]
    CustomTransformTrace {
        #[serde(rename = "pointIndex")]
        point_index: usize,
        #[serde(rename = "xMin")]
        x_min: f64,
        #[serde(rename = "xMax")]
        x_max: f64,
        #[serde(rename = "sampleCount")]
        sample_count: usize,
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
    #[serde(rename = "arc-boundary")]
    ArcBoundary {
        #[serde(rename = "hostKey")]
        host_key: usize,
        #[serde(rename = "boundaryKind")]
        boundary_kind: ArcBoundaryKindJson,
        #[serde(rename = "centerIndex", skip_serializing_if = "Option::is_none")]
        center_index: Option<usize>,
        #[serde(rename = "startIndex")]
        start_index: usize,
        #[serde(rename = "midIndex", skip_serializing_if = "Option::is_none")]
        mid_index: Option<usize>,
        #[serde(rename = "endIndex")]
        end_index: usize,
        reversed: bool,
    },
}

impl LineBindingJson {
    fn from_binding(binding: &LineBinding) -> Self {
        match binding {
            LineBinding::GraphHelperLine {
                start_index,
                end_index,
            } => Self::GraphHelperLine {
                start_index: *start_index,
                end_index: *end_index,
            },
            LineBinding::Segment {
                start_index,
                end_index,
            } => Self::Segment {
                start_index: *start_index,
                end_index: *end_index,
            },
            LineBinding::AngleMarker {
                start_index,
                vertex_index,
                end_index,
                marker_class,
            } => Self::AngleMarker {
                start_index: *start_index,
                vertex_index: *vertex_index,
                end_index: *end_index,
                marker_class: *marker_class,
            },
            LineBinding::SegmentMarker {
                start_index,
                end_index,
                t,
                marker_class,
            } => Self::SegmentMarker {
                start_index: *start_index,
                end_index: *end_index,
                t: *t,
                marker_class: *marker_class,
            },
            LineBinding::AngleBisectorRay {
                start_index,
                vertex_index,
                end_index,
            } => Self::AngleBisectorRay {
                start_index: *start_index,
                vertex_index: *vertex_index,
                end_index: *end_index,
            },
            LineBinding::PerpendicularLine {
                through_index,
                line_start_index,
                line_end_index,
                line_index,
            } => Self::PerpendicularLine {
                through_index: *through_index,
                line_start_index: *line_start_index,
                line_end_index: *line_end_index,
                line_index: *line_index,
            },
            LineBinding::ParallelLine {
                through_index,
                line_start_index,
                line_end_index,
                line_index,
            } => Self::ParallelLine {
                through_index: *through_index,
                line_start_index: *line_start_index,
                line_end_index: *line_end_index,
                line_index: *line_index,
            },
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
                parameter_name,
            } => Self::RotateLine {
                source_index: *source_index,
                center_index: *center_index,
                angle_degrees: *angle_degrees,
                parameter_name: parameter_name.clone(),
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
            LineBinding::CustomTransformTrace {
                point_index,
                x_min,
                x_max,
                sample_count,
            } => Self::CustomTransformTrace {
                point_index: *point_index,
                x_min: *x_min,
                x_max: *x_max,
                sample_count: *sample_count,
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
            LineBinding::ArcBoundary {
                host_key,
                boundary_kind,
                center_index,
                start_index,
                mid_index,
                end_index,
                reversed,
            } => Self::ArcBoundary {
                host_key: *host_key,
                boundary_kind: ArcBoundaryKindJson::from_kind(*boundary_kind),
                center_index: *center_index,
                start_index: *start_index,
                mid_index: *mid_index,
                end_index: *end_index,
                reversed: *reversed,
            },
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "kebab-case")]
enum ArcBoundaryKindJson {
    Sector,
    CircularSegment,
}

impl ArcBoundaryKindJson {
    fn from_kind(kind: ArcBoundaryKind) -> Self {
        match kind {
            ArcBoundaryKind::Sector => Self::Sector,
            ArcBoundaryKind::CircularSegment => Self::CircularSegment,
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PolygonJson {
    points: Vec<PointJson>,
    color: [u8; 4],
    outline_color: [u8; 4],
    visible: bool,
    binding: Option<ShapeBindingJson>,
}

impl PolygonJson {
    fn from_polygon(polygon: &crate::runtime::scene::PolygonShape) -> Self {
        Self {
            points: polygon.points.iter().map(PointJson::from_point).collect(),
            color: polygon.color,
            outline_color: darken(polygon.color, 80),
            visible: polygon.visible,
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
    dashed: bool,
    visible: bool,
    binding: Option<ShapeBindingJson>,
}

impl CircleJson {
    fn from_circle(circle: &crate::runtime::scene::SceneCircle) -> Self {
        Self {
            center: PointJson::from_point(&circle.center),
            radius_point: PointJson::from_point(&circle.radius_point),
            color: circle.color,
            dashed: circle.dashed,
            visible: circle.visible,
            binding: circle.binding.as_ref().map(ShapeBindingJson::from_binding),
        }
    }
}

#[derive(Serialize)]
struct ArcJson {
    points: Vec<PointJson>,
    color: [u8; 4],
    center: Option<PointJson>,
    counterclockwise: bool,
    visible: bool,
}

impl ArcJson {
    fn from_arc(arc: &crate::runtime::scene::SceneArc) -> Self {
        Self {
            points: arc.points.iter().map(PointJson::from_point).collect(),
            color: arc.color,
            center: arc.center.as_ref().map(PointJson::from_point),
            counterclockwise: arc.counterclockwise,
            visible: arc.visible,
        }
    }
}

#[derive(Serialize)]
#[serde(tag = "kind")]
enum ShapeBindingJson {
    #[serde(rename = "point-radius-circle")]
    PointRadiusCircle {
        #[serde(rename = "centerIndex")]
        center_index: usize,
        #[serde(rename = "radiusIndex")]
        radius_index: usize,
    },
    #[serde(rename = "segment-radius-circle")]
    SegmentRadiusCircle {
        #[serde(rename = "centerIndex")]
        center_index: usize,
        #[serde(rename = "lineStartIndex")]
        line_start_index: usize,
        #[serde(rename = "lineEndIndex")]
        line_end_index: usize,
    },
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
        #[serde(rename = "parameterName")]
        parameter_name: Option<String>,
    },
    #[serde(rename = "rotate-circle")]
    RotateCircle {
        #[serde(rename = "sourceIndex")]
        source_index: usize,
        #[serde(rename = "centerIndex")]
        center_index: usize,
        #[serde(rename = "angleDegrees")]
        angle_degrees: f64,
        #[serde(rename = "parameterName")]
        parameter_name: Option<String>,
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
            ShapeBinding::PointRadiusCircle {
                center_index,
                radius_index,
            } => Self::PointRadiusCircle {
                center_index: *center_index,
                radius_index: *radius_index,
            },
            ShapeBinding::SegmentRadiusCircle {
                center_index,
                line_start_index,
                line_end_index,
            } => Self::SegmentRadiusCircle {
                center_index: *center_index,
                line_start_index: *line_start_index,
                line_end_index: *line_end_index,
            },
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
                parameter_name,
            } => Self::RotatePolygon {
                source_index: *source_index,
                center_index: *center_index,
                angle_degrees: *angle_degrees,
                parameter_name: parameter_name.clone(),
            },
            ShapeBinding::RotateCircle {
                source_index,
                center_index,
                angle_degrees,
                parameter_name,
            } => Self::RotateCircle {
                source_index: *source_index,
                center_index: *center_index,
                angle_degrees: *angle_degrees,
                parameter_name: parameter_name.clone(),
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
    hotspots: Vec<LabelHotspotJson>,
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
            hotspots: label
                .hotspots
                .iter()
                .map(LabelHotspotJson::from_hotspot)
                .collect(),
            screen_space: label.screen_space,
        }
    }
}

#[derive(Serialize)]
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

#[derive(Serialize)]
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

#[derive(Serialize)]
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
                expr_label,
                expr,
            } => Self::ExpressionValue {
                parameter_name: parameter_name.clone(),
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

#[derive(Serialize)]
struct ScenePointJson {
    x: f64,
    y: f64,
    color: [u8; 4],
    visible: bool,
    constraint: Option<PointConstraintJson>,
    binding: Option<PointBindingJson>,
}

impl ScenePointJson {
    fn from_scene_point(point: &crate::runtime::scene::ScenePoint) -> Self {
        Self {
            x: point.position.x,
            y: point.position.y,
            color: point.color,
            visible: point.visible,
            constraint: PointConstraintJson::from_constraint(&point.constraint),
            binding: point.binding.as_ref().map(PointBindingJson::from_binding),
        }
    }
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
enum PointIterationJson {
    Offset {
        #[serde(rename = "seedIndex")]
        seed_index: usize,
        dx: f64,
        dy: f64,
        depth: usize,
        #[serde(rename = "parameterName")]
        parameter_name: Option<String>,
    },
    RotateChain {
        #[serde(rename = "seedIndex")]
        seed_index: usize,
        #[serde(rename = "centerIndex")]
        center_index: usize,
        #[serde(rename = "angleDegrees")]
        angle_degrees: f64,
        depth: usize,
    },
    Rotate {
        #[serde(rename = "sourceIndex")]
        source_index: usize,
        #[serde(rename = "centerIndex")]
        center_index: usize,
        #[serde(rename = "angleExpr")]
        angle_expr: FunctionExprJson,
        depth: usize,
        #[serde(rename = "parameterName")]
        parameter_name: Option<String>,
    },
}

impl PointIterationJson {
    fn from_family(family: &PointIterationFamily) -> Self {
        match family {
            PointIterationFamily::Offset {
                seed_index,
                dx,
                dy,
                depth,
                parameter_name,
            } => Self::Offset {
                seed_index: *seed_index,
                dx: *dx,
                dy: *dy,
                depth: *depth,
                parameter_name: parameter_name.clone(),
            },
            PointIterationFamily::RotateChain {
                seed_index,
                center_index,
                angle_degrees,
                depth,
            } => Self::RotateChain {
                seed_index: *seed_index,
                center_index: *center_index,
                angle_degrees: *angle_degrees,
                depth: *depth,
            },
            PointIterationFamily::Rotate {
                source_index,
                center_index,
                angle_expr,
                depth,
                parameter_name,
            } => Self::Rotate {
                source_index: *source_index,
                center_index: *center_index,
                angle_expr: FunctionExprJson::from_expr(angle_expr),
                depth: *depth,
                parameter_name: parameter_name.clone(),
            },
        }
    }
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
enum LineIterationJson {
    Translate {
        #[serde(rename = "startIndex")]
        start_index: usize,
        #[serde(rename = "endIndex")]
        end_index: usize,
        dx: f64,
        dy: f64,
        #[serde(rename = "secondaryDx")]
        secondary_dx: Option<f64>,
        #[serde(rename = "secondaryDy")]
        secondary_dy: Option<f64>,
        depth: usize,
        #[serde(rename = "parameterName")]
        parameter_name: Option<String>,
        color: [u8; 4],
        dashed: bool,
    },
    Affine {
        #[serde(rename = "startIndex")]
        start_index: usize,
        #[serde(rename = "endIndex")]
        end_index: usize,
        #[serde(rename = "sourceTriangleIndices")]
        source_triangle_indices: [usize; 3],
        #[serde(rename = "targetTriangle")]
        target_triangle: [IterationPointHandleJson; 3],
        depth: usize,
        color: [u8; 4],
        dashed: bool,
    },
}

impl LineIterationJson {
    fn from_family(family: &LineIterationFamily) -> Self {
        if let (Some(source_triangle_indices), Some(target_triangle)) = (
            family.affine_source_indices,
            family.affine_target_handles.as_ref(),
        ) {
            return Self::Affine {
                start_index: family.start_index,
                end_index: family.end_index,
                source_triangle_indices,
                target_triangle: target_triangle
                    .clone()
                    .map(|handle| IterationPointHandleJson::from_handle(&handle)),
                depth: family.depth,
                color: family.color,
                dashed: family.dashed,
            };
        }
        Self::Translate {
            start_index: family.start_index,
            end_index: family.end_index,
            dx: family.dx,
            dy: family.dy,
            secondary_dx: family.secondary_dx,
            secondary_dy: family.secondary_dy,
            depth: family.depth,
            parameter_name: family.parameter_name.clone(),
            color: family.color,
            dashed: family.dashed,
        }
    }
}

#[derive(Serialize, Clone)]
#[serde(untagged)]
enum IterationPointHandleJson {
    Point {
        #[serde(rename = "pointIndex")]
        point_index: usize,
    },
    LinePoint {
        #[serde(rename = "lineIndex")]
        line_index: usize,
        #[serde(rename = "segmentIndex")]
        segment_index: usize,
        t: f64,
    },
    Fixed {
        x: f64,
        y: f64,
    },
}

impl IterationPointHandleJson {
    fn from_handle(handle: &IterationPointHandle) -> Self {
        match handle {
            IterationPointHandle::Point { point_index } => Self::Point {
                point_index: *point_index,
            },
            IterationPointHandle::LinePoint {
                line_index,
                segment_index,
                t,
            } => Self::LinePoint {
                line_index: *line_index,
                segment_index: *segment_index,
                t: *t,
            },
            IterationPointHandle::Fixed(point) => Self::Fixed {
                x: point.x,
                y: point.y,
            },
        }
    }
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
enum PolygonIterationJson {
    Translate {
        #[serde(rename = "vertexIndices")]
        vertex_indices: Vec<usize>,
        dx: f64,
        dy: f64,
        #[serde(rename = "secondaryDx")]
        secondary_dx: Option<f64>,
        #[serde(rename = "secondaryDy")]
        secondary_dy: Option<f64>,
        depth: usize,
        #[serde(rename = "parameterName")]
        parameter_name: Option<String>,
        color: [u8; 4],
    },
}

impl PolygonIterationJson {
    fn from_family(family: &PolygonIterationFamily) -> Self {
        Self::Translate {
            vertex_indices: family.vertex_indices.clone(),
            dx: family.dx,
            dy: family.dy,
            secondary_dx: family.secondary_dx,
            secondary_dy: family.secondary_dy,
            depth: family.depth,
            parameter_name: family.parameter_name.clone(),
            color: family.color,
        }
    }
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
enum LabelIterationJson {
    PointExpression {
        #[serde(rename = "seedLabelIndex")]
        seed_label_index: usize,
        #[serde(rename = "pointSeedIndex")]
        point_seed_index: usize,
        #[serde(rename = "parameterName")]
        parameter_name: String,
        expr: FunctionExprJson,
        depth: usize,
        #[serde(rename = "depthParameterName")]
        depth_parameter_name: Option<String>,
    },
}

impl LabelIterationJson {
    fn from_family(family: &LabelIterationFamily) -> Self {
        match family {
            LabelIterationFamily::PointExpression {
                seed_label_index,
                point_seed_index,
                parameter_name,
                expr,
                depth,
                depth_parameter_name,
            } => Self::PointExpression {
                seed_label_index: *seed_label_index,
                point_seed_index: *point_seed_index,
                parameter_name: parameter_name.clone(),
                expr: FunctionExprJson::from_expr(expr),
                depth: *depth,
                depth_parameter_name: depth_parameter_name.clone(),
            },
        }
    }
}

#[derive(Serialize)]
#[serde(tag = "kind")]
enum PointBindingJson {
    #[serde(rename = "graph-calibration")]
    GraphCalibration,
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
        #[serde(rename = "parameterName")]
        parameter_name: Option<String>,
    },
    #[serde(rename = "scale")]
    Scale {
        #[serde(rename = "sourceIndex")]
        source_index: usize,
        #[serde(rename = "centerIndex")]
        center_index: usize,
        factor: f64,
    },
    #[serde(rename = "midpoint")]
    Midpoint {
        #[serde(rename = "startIndex")]
        start_index: usize,
        #[serde(rename = "endIndex")]
        end_index: usize,
    },
    #[serde(rename = "coordinate")]
    Coordinate {
        name: String,
        expr: FunctionExprJson,
    },
    #[serde(rename = "custom-transform")]
    CustomTransform {
        #[serde(rename = "sourceIndex")]
        source_index: usize,
        #[serde(rename = "originIndex")]
        origin_index: usize,
        #[serde(rename = "axisEndIndex")]
        axis_end_index: usize,
        #[serde(rename = "distanceExpr")]
        distance_expr: FunctionExprJson,
        #[serde(rename = "angleExpr")]
        angle_expr: FunctionExprJson,
        #[serde(rename = "distanceRawScale")]
        distance_raw_scale: f64,
        #[serde(rename = "angleDegreesScale")]
        angle_degrees_scale: f64,
    },
}

impl PointBindingJson {
    fn from_binding(binding: &ScenePointBinding) -> Self {
        match binding {
            ScenePointBinding::GraphCalibration => Self::GraphCalibration,
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
                parameter_name,
            } => Self::Rotate {
                source_index: *source_index,
                center_index: *center_index,
                angle_degrees: *angle_degrees,
                parameter_name: parameter_name.clone(),
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
            ScenePointBinding::Midpoint {
                start_index,
                end_index,
            } => Self::Midpoint {
                start_index: *start_index,
                end_index: *end_index,
            },
            ScenePointBinding::Coordinate { name, expr } => Self::Coordinate {
                name: name.clone(),
                expr: FunctionExprJson::from_expr(expr),
            },
            ScenePointBinding::CustomTransform {
                source_index,
                origin_index,
                axis_end_index,
                distance_expr,
                angle_expr,
                distance_raw_scale,
                angle_degrees_scale,
            } => Self::CustomTransform {
                source_index: *source_index,
                origin_index: *origin_index,
                axis_end_index: *axis_end_index,
                distance_expr: FunctionExprJson::from_expr(distance_expr),
                angle_expr: FunctionExprJson::from_expr(angle_expr),
                distance_raw_scale: *distance_raw_scale,
                angle_degrees_scale: *angle_degrees_scale,
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
    #[serde(rename = "circle-arc")]
    CircleArc {
        #[serde(rename = "centerIndex")]
        center_index: usize,
        #[serde(rename = "startIndex")]
        start_index: usize,
        #[serde(rename = "endIndex")]
        end_index: usize,
        t: f64,
    },
    #[serde(rename = "arc")]
    Arc {
        #[serde(rename = "startIndex")]
        start_index: usize,
        #[serde(rename = "midIndex")]
        mid_index: usize,
        #[serde(rename = "endIndex")]
        end_index: usize,
        t: f64,
    },
    #[serde(rename = "line-intersection")]
    LineIntersection {
        left: LineConstraintJson,
        right: LineConstraintJson,
    },
    #[serde(rename = "line-circle-intersection")]
    LineCircleIntersection {
        line: LineConstraintJson,
        #[serde(rename = "centerIndex")]
        center_index: usize,
        #[serde(rename = "radiusIndex")]
        radius_index: usize,
        variant: usize,
    },
    #[serde(rename = "circle-circle-intersection")]
    CircleCircleIntersection {
        #[serde(rename = "leftCenterIndex")]
        left_center_index: usize,
        #[serde(rename = "leftRadiusIndex")]
        left_radius_index: usize,
        #[serde(rename = "rightCenterIndex")]
        right_center_index: usize,
        #[serde(rename = "rightRadiusIndex")]
        right_radius_index: usize,
        variant: usize,
    },
    #[serde(rename = "circular-intersection")]
    CircularIntersection {
        left: CircularConstraintJson,
        right: CircularConstraintJson,
        variant: usize,
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
            ScenePointConstraint::OnCircleArc {
                center_index,
                start_index,
                end_index,
                t,
            } => Some(Self::CircleArc {
                center_index: *center_index,
                start_index: *start_index,
                end_index: *end_index,
                t: *t,
            }),
            ScenePointConstraint::OnArc {
                start_index,
                mid_index,
                end_index,
                t,
            } => Some(Self::Arc {
                start_index: *start_index,
                mid_index: *mid_index,
                end_index: *end_index,
                t: *t,
            }),
            ScenePointConstraint::LineIntersection { left, right } => {
                Some(Self::LineIntersection {
                    left: LineConstraintJson::from_constraint(left),
                    right: LineConstraintJson::from_constraint(right),
                })
            }
            ScenePointConstraint::LineCircleIntersection {
                line,
                center_index,
                radius_index,
                variant,
            } => Some(Self::LineCircleIntersection {
                line: LineConstraintJson::from_constraint(line),
                center_index: *center_index,
                radius_index: *radius_index,
                variant: *variant,
            }),
            ScenePointConstraint::CircleCircleIntersection {
                left_center_index,
                left_radius_index,
                right_center_index,
                right_radius_index,
                variant,
            } => Some(Self::CircleCircleIntersection {
                left_center_index: *left_center_index,
                left_radius_index: *left_radius_index,
                right_center_index: *right_center_index,
                right_radius_index: *right_radius_index,
                variant: *variant,
            }),
            ScenePointConstraint::CircularIntersection {
                left,
                right,
                variant,
            } => Some(Self::CircularIntersection {
                left: CircularConstraintJson::from_constraint(left),
                right: CircularConstraintJson::from_constraint(right),
                variant: *variant,
            }),
        }
    }
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
enum CircularConstraintJson {
    Circle {
        #[serde(rename = "centerIndex")]
        center_index: usize,
        #[serde(rename = "radiusIndex")]
        radius_index: usize,
    },
    ThreePointArc {
        #[serde(rename = "startIndex")]
        start_index: usize,
        #[serde(rename = "midIndex")]
        mid_index: usize,
        #[serde(rename = "endIndex")]
        end_index: usize,
    },
}

impl CircularConstraintJson {
    fn from_constraint(constraint: &CircularConstraint) -> Self {
        match constraint {
            CircularConstraint::Circle {
                center_index,
                radius_index,
            } => Self::Circle {
                center_index: *center_index,
                radius_index: *radius_index,
            },
            CircularConstraint::ThreePointArc {
                start_index,
                mid_index,
                end_index,
            } => Self::ThreePointArc {
                start_index: *start_index,
                mid_index: *mid_index,
                end_index: *end_index,
            },
        }
    }
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
enum LineConstraintJson {
    Segment {
        #[serde(rename = "startIndex")]
        start_index: usize,
        #[serde(rename = "endIndex")]
        end_index: usize,
    },
    Line {
        #[serde(rename = "startIndex")]
        start_index: usize,
        #[serde(rename = "endIndex")]
        end_index: usize,
    },
    Ray {
        #[serde(rename = "startIndex")]
        start_index: usize,
        #[serde(rename = "endIndex")]
        end_index: usize,
    },
    #[serde(rename = "perpendicular-line")]
    PerpendicularLine {
        #[serde(rename = "throughIndex")]
        through_index: usize,
        #[serde(rename = "lineStartIndex")]
        line_start_index: usize,
        #[serde(rename = "lineEndIndex")]
        line_end_index: usize,
    },
    #[serde(rename = "parallel-line")]
    ParallelLine {
        #[serde(rename = "throughIndex")]
        through_index: usize,
        #[serde(rename = "lineStartIndex")]
        line_start_index: usize,
        #[serde(rename = "lineEndIndex")]
        line_end_index: usize,
    },
    #[serde(rename = "angle-bisector-ray")]
    AngleBisectorRay {
        #[serde(rename = "startIndex")]
        start_index: usize,
        #[serde(rename = "vertexIndex")]
        vertex_index: usize,
        #[serde(rename = "endIndex")]
        end_index: usize,
    },
}

impl LineConstraintJson {
    fn from_constraint(constraint: &LineConstraint) -> Self {
        match constraint {
            LineConstraint::Segment {
                start_index,
                end_index,
            } => Self::Segment {
                start_index: *start_index,
                end_index: *end_index,
            },
            LineConstraint::Line {
                start_index,
                end_index,
            } => Self::Line {
                start_index: *start_index,
                end_index: *end_index,
            },
            LineConstraint::Ray {
                start_index,
                end_index,
            } => Self::Ray {
                start_index: *start_index,
                end_index: *end_index,
            },
            LineConstraint::PerpendicularLine {
                through_index,
                line_start_index,
                line_end_index,
            } => Self::PerpendicularLine {
                through_index: *through_index,
                line_start_index: *line_start_index,
                line_end_index: *line_end_index,
            },
            LineConstraint::ParallelLine {
                through_index,
                line_start_index,
                line_end_index,
            } => Self::ParallelLine {
                through_index: *through_index,
                line_start_index: *line_start_index,
                line_end_index: *line_end_index,
            },
            LineConstraint::AngleBisectorRay {
                start_index,
                vertex_index,
                end_index,
            } => Self::AngleBisectorRay {
                start_index: *start_index,
                vertex_index: *vertex_index,
                end_index: *end_index,
            },
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
                plot_mode: match function_def.domain.mode {
                    FunctionPlotMode::Cartesian => PlotModeJson::Cartesian,
                    FunctionPlotMode::Polar => PlotModeJson::Polar,
                },
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
    plot_mode: PlotModeJson,
}

#[derive(Serialize)]
#[serde(rename_all = "kebab-case")]
enum PlotModeJson {
    Cartesian,
    Polar,
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
    #[serde(rename = "power")]
    Power {
        base: Box<FunctionTermJson>,
        exponent: Box<FunctionTermJson>,
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
            FunctionTerm::Power(base, exponent) => Self::Power {
                base: Box::new(Self::from_term(base)),
                exponent: Box::new(Self::from_term(exponent)),
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
