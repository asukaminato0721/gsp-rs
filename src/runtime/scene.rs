use crate::format::PointRecord;

use super::functions::{FunctionExpr, FunctionPlotDescriptor};
use super::geometry::Bounds;

#[derive(Debug, Clone)]
pub(crate) struct Scene {
    pub(crate) graph_mode: bool,
    pub(crate) pi_mode: bool,
    pub(crate) saved_viewport: bool,
    pub(crate) y_up: bool,
    pub(crate) origin: Option<PointRecord>,
    pub(crate) bounds: Bounds,
    pub(crate) images: Vec<SceneImage>,
    pub(crate) lines: Vec<LineShape>,
    pub(crate) polygons: Vec<PolygonShape>,
    pub(crate) circles: Vec<SceneCircle>,
    pub(crate) arcs: Vec<SceneArc>,
    pub(crate) labels: Vec<TextLabel>,
    pub(crate) points: Vec<ScenePoint>,
    pub(crate) point_iterations: Vec<PointIterationFamily>,
    pub(crate) line_iterations: Vec<LineIterationFamily>,
    pub(crate) polygon_iterations: Vec<PolygonIterationFamily>,
    pub(crate) label_iterations: Vec<LabelIterationFamily>,
    pub(crate) buttons: Vec<SceneButton>,
    pub(crate) parameters: Vec<SceneParameter>,
    pub(crate) functions: Vec<SceneFunction>,
}

#[derive(Debug, Clone)]
pub(crate) struct SceneImage {
    pub(crate) top_left: PointRecord,
    pub(crate) bottom_right: PointRecord,
    pub(crate) src: String,
    pub(crate) screen_space: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct SceneButton {
    pub(crate) text: String,
    pub(crate) anchor: ScreenPoint,
    pub(crate) rect: Option<ScreenRect>,
    pub(crate) action: ButtonAction,
}

#[derive(Debug, Clone)]
pub(crate) enum ButtonAction {
    Link {
        href: String,
    },
    ToggleVisibility {
        point_indices: Vec<usize>,
        line_indices: Vec<usize>,
        circle_indices: Vec<usize>,
        polygon_indices: Vec<usize>,
    },
    SetVisibility {
        visible: bool,
        point_indices: Vec<usize>,
        line_indices: Vec<usize>,
        circle_indices: Vec<usize>,
        polygon_indices: Vec<usize>,
    },
    ShowHideVisibility {
        point_indices: Vec<usize>,
        line_indices: Vec<usize>,
        circle_indices: Vec<usize>,
        polygon_indices: Vec<usize>,
    },
    MovePoint {
        point_index: usize,
        target_point_index: Option<usize>,
    },
    AnimatePoint {
        point_index: usize,
    },
    ScrollPoint {
        point_index: usize,
    },
    Sequence {
        button_indices: Vec<usize>,
        interval_ms: u32,
    },
}

#[derive(Debug, Clone)]
pub(crate) struct ScreenPoint {
    pub(crate) x: f64,
    pub(crate) y: f64,
}

#[derive(Debug, Clone)]
pub(crate) struct ScreenRect {
    pub(crate) width: f64,
    pub(crate) height: f64,
}

#[derive(Debug, Clone)]
pub(crate) struct ScenePoint {
    pub(crate) position: PointRecord,
    pub(crate) color: [u8; 4],
    pub(crate) visible: bool,
    pub(crate) draggable: bool,
    pub(crate) constraint: ScenePointConstraint,
    pub(crate) binding: Option<ScenePointBinding>,
}

#[derive(Debug, Clone)]
pub(crate) enum PointIterationFamily {
    Offset {
        seed_index: usize,
        dx: f64,
        dy: f64,
        depth: usize,
        parameter_name: Option<String>,
    },
    RotateChain {
        seed_index: usize,
        center_index: usize,
        angle_degrees: f64,
        depth: usize,
    },
    Rotate {
        source_index: usize,
        center_index: usize,
        angle_expr: FunctionExpr,
        depth: usize,
        parameter_name: Option<String>,
    },
}

#[derive(Debug, Clone)]
pub(crate) struct LineIterationFamily {
    pub(crate) start_index: usize,
    pub(crate) end_index: usize,
    pub(crate) dx: f64,
    pub(crate) dy: f64,
    pub(crate) secondary_dx: Option<f64>,
    pub(crate) secondary_dy: Option<f64>,
    pub(crate) depth: usize,
    pub(crate) parameter_name: Option<String>,
    pub(crate) color: [u8; 4],
    pub(crate) dashed: bool,
    pub(crate) affine_source_indices: Option<[usize; 3]>,
    pub(crate) affine_target_handles: Option<[IterationPointHandle; 3]>,
}

#[derive(Debug, Clone)]
pub(crate) enum IterationPointHandle {
    Point {
        point_index: usize,
    },
    LinePoint {
        line_index: usize,
        segment_index: usize,
        t: f64,
    },
    Fixed(PointRecord),
}

#[derive(Debug, Clone)]
pub(crate) struct PolygonIterationFamily {
    pub(crate) vertex_indices: Vec<usize>,
    pub(crate) dx: f64,
    pub(crate) dy: f64,
    pub(crate) secondary_dx: Option<f64>,
    pub(crate) secondary_dy: Option<f64>,
    pub(crate) depth: usize,
    pub(crate) parameter_name: Option<String>,
    pub(crate) color: [u8; 4],
}

#[derive(Debug, Clone)]
pub(crate) enum LabelIterationFamily {
    PointExpression {
        seed_label_index: usize,
        point_seed_index: usize,
        parameter_name: String,
        expr: FunctionExpr,
        depth: usize,
        depth_parameter_name: Option<String>,
    },
}

#[derive(Debug, Clone)]
pub(crate) enum ScenePointConstraint {
    Free,
    Offset {
        origin_index: usize,
        dx: f64,
        dy: f64,
    },
    OnSegment {
        start_index: usize,
        end_index: usize,
        t: f64,
    },
    OnPolyline {
        function_key: usize,
        points: Vec<PointRecord>,
        segment_index: usize,
        t: f64,
    },
    OnPolygonBoundary {
        vertex_indices: Vec<usize>,
        edge_index: usize,
        t: f64,
    },
    OnCircle {
        center_index: usize,
        radius_index: usize,
        unit_x: f64,
        unit_y: f64,
    },
    OnCircleArc {
        center_index: usize,
        start_index: usize,
        end_index: usize,
        t: f64,
    },
    OnArc {
        start_index: usize,
        mid_index: usize,
        end_index: usize,
        t: f64,
    },
    LineIntersection {
        left: LineConstraint,
        right: LineConstraint,
    },
    LineTraceIntersection {
        line: LineConstraint,
        point_index: usize,
        x_min: f64,
        x_max: f64,
        sample_count: usize,
    },
    LineCircleIntersection {
        line: LineConstraint,
        center_index: usize,
        radius_index: usize,
        variant: usize,
    },
    CircleCircleIntersection {
        left_center_index: usize,
        left_radius_index: usize,
        right_center_index: usize,
        right_radius_index: usize,
        variant: usize,
    },
    CircularIntersection {
        left: CircularConstraint,
        right: CircularConstraint,
        variant: usize,
    },
}

#[derive(Debug, Clone)]
pub(crate) enum CircularConstraint {
    Circle {
        center_index: usize,
        radius_index: usize,
    },
    ThreePointArc {
        start_index: usize,
        mid_index: usize,
        end_index: usize,
    },
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum LineLikeKind {
    Segment,
    Line,
    Ray,
}

#[derive(Debug, Clone)]
pub(crate) enum LineConstraint {
    Segment {
        start_index: usize,
        end_index: usize,
    },
    Line {
        start_index: usize,
        end_index: usize,
    },
    Ray {
        start_index: usize,
        end_index: usize,
    },
    PerpendicularLine {
        through_index: usize,
        line_start_index: usize,
        line_end_index: usize,
    },
    ParallelLine {
        through_index: usize,
        line_start_index: usize,
        line_end_index: usize,
    },
    AngleBisectorRay {
        start_index: usize,
        vertex_index: usize,
        end_index: usize,
    },
}

#[derive(Debug, Clone)]
pub(crate) struct LineShape {
    pub(crate) points: Vec<PointRecord>,
    pub(crate) color: [u8; 4],
    pub(crate) dashed: bool,
    pub(crate) visible: bool,
    pub(crate) binding: Option<LineBinding>,
}

#[derive(Debug, Clone)]
pub(crate) enum LineBinding {
    GraphHelperLine {
        start_index: usize,
        end_index: usize,
    },
    Segment {
        start_index: usize,
        end_index: usize,
    },
    AngleMarker {
        start_index: usize,
        vertex_index: usize,
        end_index: usize,
        marker_class: u32,
    },
    SegmentMarker {
        start_index: usize,
        end_index: usize,
        t: f64,
        marker_class: u32,
    },
    AngleBisectorRay {
        start_index: usize,
        vertex_index: usize,
        end_index: usize,
    },
    PerpendicularLine {
        through_index: usize,
        line_start_index: Option<usize>,
        line_end_index: Option<usize>,
        line_index: Option<usize>,
    },
    ParallelLine {
        through_index: usize,
        line_start_index: Option<usize>,
        line_end_index: Option<usize>,
        line_index: Option<usize>,
    },
    Line {
        start_index: usize,
        end_index: usize,
    },
    Ray {
        start_index: usize,
        end_index: usize,
    },
    TranslateLine {
        source_index: usize,
        vector_start_index: usize,
        vector_end_index: usize,
    },
    RotateLine {
        source_index: usize,
        center_index: usize,
        angle_degrees: f64,
        parameter_name: Option<String>,
    },
    ScaleLine {
        source_index: usize,
        center_index: usize,
        factor: f64,
    },
    ReflectLine {
        source_index: usize,
        line_start_index: usize,
        line_end_index: usize,
    },
    CustomTransformTrace {
        point_index: usize,
        x_min: f64,
        x_max: f64,
        sample_count: usize,
    },
    CoordinateTrace {
        point_index: usize,
        x_min: f64,
        x_max: f64,
        sample_count: usize,
    },
    RotateEdge {
        center_index: usize,
        vertex_index: usize,
        parameter_name: String,
        angle_expr: FunctionExpr,
        start_step: usize,
        end_step: usize,
    },
    ArcBoundary {
        host_key: usize,
        boundary_kind: ArcBoundaryKind,
        center_index: Option<usize>,
        start_index: usize,
        mid_index: Option<usize>,
        end_index: usize,
        reversed: bool,
    },
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum ArcBoundaryKind {
    Sector,
    CircularSegment,
}

#[derive(Debug, Clone)]
pub(crate) struct PolygonShape {
    pub(crate) points: Vec<PointRecord>,
    pub(crate) color: [u8; 4],
    pub(crate) visible: bool,
    pub(crate) binding: Option<ShapeBinding>,
}

#[derive(Debug, Clone)]
pub(crate) struct SceneParameter {
    pub(crate) name: String,
    pub(crate) value: f64,
    pub(crate) unit: Option<String>,
    pub(crate) label_index: Option<usize>,
}

#[derive(Debug, Clone)]
pub(crate) enum ScenePointBinding {
    GraphCalibration,
    Parameter {
        name: String,
    },
    DerivedParameter {
        source_index: usize,
    },
    Translate {
        source_index: usize,
        vector_start_index: usize,
        vector_end_index: usize,
    },
    Reflect {
        source_index: usize,
        line_start_index: usize,
        line_end_index: usize,
    },
    Rotate {
        source_index: usize,
        center_index: usize,
        angle_degrees: f64,
        parameter_name: Option<String>,
    },
    Scale {
        source_index: usize,
        center_index: usize,
        factor: f64,
    },
    Midpoint {
        start_index: usize,
        end_index: usize,
    },
    Coordinate {
        name: String,
        expr: FunctionExpr,
    },
    CoordinateSource {
        source_index: usize,
        name: String,
        expr: FunctionExpr,
        axis: CoordinateAxis,
    },
    CoordinateSource2d {
        source_index: usize,
        x_name: String,
        x_expr: FunctionExpr,
        y_name: String,
        y_expr: FunctionExpr,
    },
    CustomTransform {
        source_index: usize,
        origin_index: usize,
        axis_end_index: usize,
        distance_expr: FunctionExpr,
        angle_expr: FunctionExpr,
        distance_raw_scale: f64,
        angle_degrees_scale: f64,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CoordinateAxis {
    Horizontal,
    Vertical,
}

#[derive(Debug, Clone)]
pub(crate) struct SceneFunction {
    pub(crate) key: usize,
    pub(crate) name: String,
    pub(crate) derivative: bool,
    pub(crate) expr: FunctionExpr,
    pub(crate) domain: FunctionPlotDescriptor,
    pub(crate) line_index: Option<usize>,
    pub(crate) label_index: usize,
    pub(crate) constrained_point_indices: Vec<usize>,
}

#[derive(Debug, Clone)]
pub(crate) struct SceneCircle {
    pub(crate) center: PointRecord,
    pub(crate) radius_point: PointRecord,
    pub(crate) color: [u8; 4],
    pub(crate) dashed: bool,
    pub(crate) visible: bool,
    pub(crate) binding: Option<ShapeBinding>,
}

#[derive(Debug, Clone)]
pub(crate) struct SceneArc {
    pub(crate) points: [PointRecord; 3],
    pub(crate) color: [u8; 4],
    pub(crate) center: Option<PointRecord>,
    pub(crate) counterclockwise: bool,
    pub(crate) visible: bool,
}

#[derive(Debug, Clone)]
pub(crate) enum ShapeBinding {
    PointRadiusCircle {
        center_index: usize,
        radius_index: usize,
    },
    SegmentRadiusCircle {
        center_index: usize,
        line_start_index: usize,
        line_end_index: usize,
    },
    TranslatePolygon {
        source_index: usize,
        vector_start_index: usize,
        vector_end_index: usize,
    },
    TranslateCircle {
        source_index: usize,
        vector_start_index: usize,
        vector_end_index: usize,
    },
    RotatePolygon {
        source_index: usize,
        center_index: usize,
        angle_degrees: f64,
        parameter_name: Option<String>,
    },
    RotateCircle {
        source_index: usize,
        center_index: usize,
        angle_degrees: f64,
        parameter_name: Option<String>,
    },
    ScalePolygon {
        source_index: usize,
        center_index: usize,
        factor: f64,
    },
    ScaleCircle {
        source_index: usize,
        center_index: usize,
        factor: f64,
    },
    ReflectPolygon {
        source_index: usize,
        line_start_index: usize,
        line_end_index: usize,
    },
    ReflectCircle {
        source_index: usize,
        line_start_index: usize,
        line_end_index: usize,
    },
}

#[derive(Debug, Clone)]
pub(crate) struct TextLabel {
    pub(crate) anchor: PointRecord,
    pub(crate) text: String,
    pub(crate) color: [u8; 4],
    pub(crate) visible: bool,
    pub(crate) binding: Option<TextLabelBinding>,
    pub(crate) screen_space: bool,
    pub(crate) hotspots: Vec<TextLabelHotspot>,
}

#[derive(Debug, Clone)]
pub(crate) struct TextLabelHotspot {
    pub(crate) line: usize,
    pub(crate) start: usize,
    pub(crate) end: usize,
    pub(crate) text: String,
    pub(crate) action: TextLabelHotspotAction,
}

#[derive(Debug, Clone)]
pub(crate) enum TextLabelHotspotAction {
    Button {
        button_index: usize,
    },
    Point {
        point_index: usize,
    },
    Segment {
        start_point_index: usize,
        end_point_index: usize,
    },
    AngleMarker {
        start_point_index: usize,
        vertex_point_index: usize,
        end_point_index: usize,
    },
    Circle {
        circle_index: usize,
    },
    Polygon {
        polygon_index: usize,
    },
}

#[derive(Debug, Clone)]
pub(crate) enum TextLabelBinding {
    ParameterValue {
        name: String,
    },
    FunctionLabel {
        function_key: usize,
        derivative: bool,
    },
    ExpressionValue {
        parameter_name: String,
        expr_label: String,
        expr: FunctionExpr,
    },
    PointExpressionValue {
        point_index: usize,
        parameter_name: String,
        expr: FunctionExpr,
    },
    PolygonBoundaryParameter {
        point_index: usize,
        point_name: String,
        polygon_name: String,
    },
    SegmentParameter {
        point_index: usize,
        point_name: String,
        segment_name: String,
    },
    CircleParameter {
        point_index: usize,
        point_name: String,
        circle_name: String,
    },
    AngleMarkerValue {
        start_index: usize,
        vertex_index: usize,
        end_index: usize,
        decimals: usize,
    },
    CustomTransformValue {
        point_index: usize,
        expr_label: String,
        expr: FunctionExpr,
        value_scale: f64,
        value_suffix: String,
    },
}
