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
    pub(crate) lines: Vec<LineShape>,
    pub(crate) polygons: Vec<PolygonShape>,
    pub(crate) circles: Vec<SceneCircle>,
    pub(crate) labels: Vec<TextLabel>,
    pub(crate) points: Vec<ScenePoint>,
    pub(crate) parameters: Vec<SceneParameter>,
    pub(crate) functions: Vec<SceneFunction>,
}

#[derive(Debug, Clone)]
pub(crate) struct ScenePoint {
    pub(crate) position: PointRecord,
    pub(crate) constraint: ScenePointConstraint,
    pub(crate) binding: Option<ScenePointBinding>,
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
}

#[derive(Debug, Clone)]
pub(crate) struct LineShape {
    pub(crate) points: Vec<PointRecord>,
    pub(crate) color: [u8; 4],
    pub(crate) dashed: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct PolygonShape {
    pub(crate) points: Vec<PointRecord>,
    pub(crate) color: [u8; 4],
    pub(crate) binding: Option<ShapeBinding>,
}

#[derive(Debug, Clone)]
pub(crate) struct SceneParameter {
    pub(crate) name: String,
    pub(crate) value: f64,
    pub(crate) label_index: Option<usize>,
}

#[derive(Debug, Clone)]
pub(crate) enum ScenePointBinding {
    Parameter {
        name: String,
    },
    DerivedParameter {
        source_index: usize,
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
    },
    Scale {
        source_index: usize,
        center_index: usize,
        factor: f64,
    },
    Coordinate {
        name: String,
        expr: FunctionExpr,
    },
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
    pub(crate) binding: Option<ShapeBinding>,
}

#[derive(Debug, Clone)]
pub(crate) enum ShapeBinding {
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
    pub(crate) binding: Option<TextLabelBinding>,
    pub(crate) screen_space: bool,
}

#[derive(Debug, Clone)]
pub(crate) enum TextLabelBinding {
    ParameterValue {
        name: String,
    },
    ExpressionValue {
        parameter_name: String,
        expr_label: String,
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
}
