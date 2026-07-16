use super::function_expr_json::FunctionExprJson;
use super::scene_json::{DebugSourceJson, PointJson};
use super::transform_json::TransformJson;
use crate::runtime::functions::{FunctionExpr, FunctionPlotMode};
use crate::runtime::scene::{
    ArcConstraint, CircularConstraint, LineConstraint, ScenePointBinding, ScenePointConstraint,
};
use serde::Serialize;
use ts_rs::TS;

#[derive(Serialize, TS)]
pub(super) struct ScenePointJson {
    x: f64,
    y: f64,
    color: [u8; 4],
    visible: bool,
    draggable: bool,
    constraint: Option<PointConstraintJson>,
    binding: Option<PointBindingJson>,
    #[serde(skip_serializing_if = "Option::is_none")]
    debug: Option<DebugSourceJson>,
}

impl ScenePointJson {
    pub(super) fn from_scene_point(point: &crate::runtime::scene::ScenePoint) -> Self {
        Self {
            x: point.position.x,
            y: point.position.y,
            color: point.color,
            visible: point.visible,
            draggable: point.draggable,
            constraint: PointConstraintJson::from_constraint(&point.constraint),
            binding: point.binding.as_ref().map(PointBindingJson::from_binding),
            debug: point.debug.as_ref().map(DebugSourceJson::from_source),
        }
    }
}

#[derive(Serialize, TS)]
#[serde(tag = "kind")]
enum PointBindingJson {
    #[serde(rename = "graph-calibration")]
    GraphCalibration,
    #[serde(rename = "payload-alias")]
    PayloadAlias {
        #[serde(rename = "sourceIndex")]
        source_index: usize,
    },
    #[serde(rename = "parameter")]
    Parameter { name: String },
    #[serde(rename = "derived-parameter")]
    DerivedParameter {
        #[serde(rename = "sourceIndex")]
        source_index: usize,
        #[serde(
            rename = "parameterStartIndex",
            skip_serializing_if = "Option::is_none"
        )]
        parameter_start_index: Option<usize>,
        #[serde(rename = "parameterEndIndex", skip_serializing_if = "Option::is_none")]
        parameter_end_index: Option<usize>,
    },
    #[serde(rename = "constraint-parameter-expr")]
    ConstraintParameterExpr { expr: FunctionExprJson },
    #[serde(rename = "constraint-parameter-point-distance-ratio")]
    ConstraintParameterPointDistanceRatio {
        #[serde(rename = "originIndex")]
        origin_index: usize,
        #[serde(rename = "denominatorIndex")]
        denominator_index: usize,
        #[serde(rename = "numeratorIndex")]
        numerator_index: usize,
        #[serde(rename = "clampToUnit")]
        clamp_to_unit: bool,
    },
    #[serde(rename = "constraint-parameter-from-point-expr")]
    ConstraintParameterFromPointExpr {
        #[serde(rename = "sourceIndex")]
        source_index: usize,
        #[serde(
            rename = "sourceParameterStartIndex",
            skip_serializing_if = "Option::is_none"
        )]
        source_parameter_start_index: Option<usize>,
        #[serde(
            rename = "sourceParameterEndIndex",
            skip_serializing_if = "Option::is_none"
        )]
        source_parameter_end_index: Option<usize>,
        #[serde(rename = "parameterName")]
        parameter_name: String,
        expr: FunctionExprJson,
        #[serde(rename = "absoluteValue")]
        absolute_value: bool,
    },
    #[serde(rename = "matrix-apply")]
    MatrixApply {
        #[serde(rename = "sourceIndex")]
        source_index: usize,
        #[serde(rename = "matrixApply")]
        matrix_apply: Vec<PointTransformJson>,
    },
    #[serde(rename = "directed-angle-anchor")]
    DirectedAngleAnchor {
        #[serde(rename = "firstStartIndex")]
        first_start_index: usize,
        #[serde(rename = "firstEndIndex")]
        first_end_index: usize,
        #[serde(rename = "secondStartIndex")]
        second_start_index: usize,
        #[serde(rename = "secondEndIndex")]
        second_end_index: usize,
        distance: f64,
        parameter: f64,
    },
    #[serde(rename = "marked-angle-translation")]
    MarkedAngleTranslation {
        #[serde(rename = "targetIndex")]
        target_index: usize,
        #[serde(rename = "angleStartIndex")]
        angle_start_index: usize,
        #[serde(rename = "angleVertexIndex")]
        angle_vertex_index: usize,
        #[serde(rename = "angleEndIndex")]
        angle_end_index: usize,
        distance: f64,
        #[serde(rename = "distanceExpr")]
        distance_expr: FunctionExprJson,
    },
    #[serde(rename = "midpoint")]
    Midpoint {
        #[serde(rename = "startIndex")]
        start_index: usize,
        #[serde(rename = "endIndex")]
        end_index: usize,
    },
    #[serde(rename = "circumcenter")]
    Circumcenter {
        #[serde(rename = "startIndex")]
        start_index: usize,
        #[serde(rename = "midIndex")]
        mid_index: usize,
        #[serde(rename = "endIndex")]
        end_index: usize,
    },
    #[serde(rename = "coordinate")]
    Coordinate {
        name: String,
        expr: FunctionExprJson,
    },
    #[serde(rename = "coordinate-source")]
    CoordinateSource {
        #[serde(rename = "sourceIndex")]
        source_index: usize,
        name: String,
        expr: FunctionExprJson,
        #[serde(rename = "parameterGroupOrdinals")]
        parameter_group_ordinals: std::collections::BTreeMap<String, usize>,
        axis: CoordinateAxisJson,
    },
    #[serde(rename = "coordinate-source-2d")]
    CoordinateSource2d {
        #[serde(rename = "sourceIndex")]
        source_index: usize,
        #[serde(rename = "xScalarGroupOrdinal")]
        x_scalar_group_ordinal: Option<usize>,
        #[serde(rename = "xName")]
        x_name: String,
        #[serde(rename = "xExpr")]
        x_expr: FunctionExprJson,
        #[serde(rename = "yScalarGroupOrdinal")]
        y_scalar_group_ordinal: Option<usize>,
        #[serde(rename = "yName")]
        y_name: String,
        #[serde(rename = "yExpr")]
        y_expr: FunctionExprJson,
    },
    #[serde(rename = "polar-offset")]
    PolarOffset {
        #[serde(rename = "sourceIndex")]
        source_index: usize,
        #[serde(rename = "distanceExpr")]
        distance_expr: FunctionExprJson,
        #[serde(rename = "xScale")]
        x_scale: f64,
        #[serde(rename = "yScale")]
        y_scale: f64,
    },
    #[serde(rename = "polar-transform")]
    PolarTransform {
        #[serde(rename = "sourceIndex")]
        source_index: usize,
        #[serde(rename = "distanceExpr")]
        distance_expr: FunctionExprJson,
        #[serde(rename = "distanceScale")]
        distance_scale: f64,
        #[serde(rename = "angleExpr")]
        angle_expr: FunctionExprJson,
        #[serde(rename = "angleDegreesScale")]
        angle_degrees_scale: f64,
    },
    #[serde(rename = "boundary-length-offset")]
    BoundaryLengthOffset {
        #[serde(rename = "sourceIndex")]
        source_index: usize,
        boundary: CircularConstraintJson,
        #[serde(rename = "xScale")]
        x_scale: f64,
        #[serde(rename = "yScale")]
        y_scale: f64,
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

#[derive(Serialize, TS)]
#[serde(tag = "kind")]
enum PointTransformJson {
    #[serde(rename = "translate")]
    Translate {
        #[serde(rename = "vectorStartIndex")]
        vector_start_index: usize,
        #[serde(rename = "vectorEndIndex")]
        vector_end_index: usize,
    },
    #[serde(rename = "reflect")]
    Reflect {
        #[serde(rename = "lineStartIndex")]
        line_start_index: usize,
        #[serde(rename = "lineEndIndex")]
        line_end_index: usize,
    },
    #[serde(rename = "reflect-constraint")]
    ReflectLineConstraint { line: LineConstraintJson },
    #[serde(rename = "rotate")]
    Rotate {
        #[serde(rename = "centerIndex")]
        center_index: usize,
        #[serde(rename = "angleDegrees")]
        angle_degrees: f64,
        #[serde(rename = "parameterName")]
        parameter_name: Option<String>,
        #[serde(rename = "angleExpr", skip_serializing_if = "Option::is_none")]
        angle_expr: Option<FunctionExprJson>,
        #[serde(rename = "angleStartIndex", skip_serializing_if = "Option::is_none")]
        angle_start_index: Option<usize>,
        #[serde(rename = "angleVertexIndex", skip_serializing_if = "Option::is_none")]
        angle_vertex_index: Option<usize>,
        #[serde(rename = "angleEndIndex", skip_serializing_if = "Option::is_none")]
        angle_end_index: Option<usize>,
        #[serde(
            rename = "angleParameterPointIndex",
            skip_serializing_if = "Option::is_none"
        )]
        angle_parameter_point_index: Option<usize>,
        #[serde(
            rename = "angleParameterStartIndex",
            skip_serializing_if = "Option::is_none"
        )]
        angle_parameter_start_index: Option<usize>,
        #[serde(
            rename = "angleParameterEndIndex",
            skip_serializing_if = "Option::is_none"
        )]
        angle_parameter_end_index: Option<usize>,
        #[serde(
            rename = "angleParameterScale",
            skip_serializing_if = "Option::is_none"
        )]
        angle_parameter_scale: Option<f64>,
    },
    #[serde(rename = "scale")]
    Scale {
        #[serde(rename = "centerIndex")]
        center_index: usize,
        factor: f64,
        #[serde(rename = "parameterName")]
        parameter_name: Option<String>,
        #[serde(rename = "factorExpr", skip_serializing_if = "Option::is_none")]
        factor_expr: Option<FunctionExprJson>,
        #[serde(
            rename = "factorParameterPointIndex",
            skip_serializing_if = "Option::is_none"
        )]
        factor_parameter_point_index: Option<usize>,
        #[serde(
            rename = "factorParameterStartIndex",
            skip_serializing_if = "Option::is_none"
        )]
        factor_parameter_start_index: Option<usize>,
        #[serde(
            rename = "factorParameterEndIndex",
            skip_serializing_if = "Option::is_none"
        )]
        factor_parameter_end_index: Option<usize>,
    },
    #[serde(rename = "scale-by-ratio")]
    ScaleByRatio {
        #[serde(rename = "centerIndex")]
        center_index: usize,
        #[serde(rename = "ratioOriginIndex")]
        ratio_origin_index: usize,
        #[serde(rename = "ratioDenominatorIndex")]
        ratio_denominator_index: usize,
        #[serde(rename = "ratioNumeratorIndex")]
        ratio_numerator_index: usize,
        signed: bool,
        #[serde(rename = "clampToUnit")]
        clamp_to_unit: bool,
    },
}

#[derive(Serialize, TS)]
#[serde(rename_all = "kebab-case")]
enum CoordinateAxisJson {
    Horizontal,
    Vertical,
}

impl CoordinateAxisJson {
    fn from_axis(axis: crate::runtime::scene::CoordinateAxis) -> Self {
        match axis {
            crate::runtime::scene::CoordinateAxis::Horizontal => Self::Horizontal,
            crate::runtime::scene::CoordinateAxis::Vertical => Self::Vertical,
        }
    }
}

impl PointBindingJson {
    fn from_binding(binding: &ScenePointBinding) -> Self {
        match binding {
            ScenePointBinding::GraphCalibration => Self::GraphCalibration,
            ScenePointBinding::ProjectedCoordinate { source_index, .. } => Self::PayloadAlias {
                source_index: *source_index,
            },
            ScenePointBinding::Parameter { name } => Self::Parameter { name: name.clone() },
            ScenePointBinding::DerivedParameter {
                source_index,
                parameter_start_index,
                parameter_end_index,
            } => Self::DerivedParameter {
                source_index: *source_index,
                parameter_start_index: *parameter_start_index,
                parameter_end_index: *parameter_end_index,
            },
            ScenePointBinding::ConstraintParameterExpr { expr } => Self::ConstraintParameterExpr {
                expr: FunctionExprJson::from_expr(expr),
            },
            ScenePointBinding::ConstraintParameterPointDistanceRatio {
                origin_index,
                denominator_index,
                numerator_index,
                clamp_to_unit,
            } => Self::ConstraintParameterPointDistanceRatio {
                origin_index: *origin_index,
                denominator_index: *denominator_index,
                numerator_index: *numerator_index,
                clamp_to_unit: *clamp_to_unit,
            },
            ScenePointBinding::ConstraintParameterFromPointExpr {
                source_index,
                source_parameter_start_index,
                source_parameter_end_index,
                parameter_name,
                expr,
                absolute_value,
                ..
            } => Self::ConstraintParameterFromPointExpr {
                source_index: *source_index,
                source_parameter_start_index: *source_parameter_start_index,
                source_parameter_end_index: *source_parameter_end_index,
                parameter_name: parameter_name.clone(),
                expr: FunctionExprJson::from_expr(expr),
                absolute_value: *absolute_value,
            },
            ScenePointBinding::Translate {
                source_index,
                vector_start_index,
                vector_end_index,
            } => Self::MatrixApply {
                source_index: *source_index,
                matrix_apply: vec![PointTransformJson::Translate {
                    vector_start_index: *vector_start_index,
                    vector_end_index: *vector_end_index,
                }],
            },
            ScenePointBinding::DirectedAngleAnchor {
                first_start_index,
                first_end_index,
                second_start_index,
                second_end_index,
                distance,
                parameter,
            } => Self::DirectedAngleAnchor {
                first_start_index: *first_start_index,
                first_end_index: *first_end_index,
                second_start_index: *second_start_index,
                second_end_index: *second_end_index,
                distance: *distance,
                parameter: *parameter,
            },
            ScenePointBinding::Reflect {
                source_index,
                line_start_index,
                line_end_index,
            } => Self::MatrixApply {
                source_index: *source_index,
                matrix_apply: vec![PointTransformJson::Reflect {
                    line_start_index: *line_start_index,
                    line_end_index: *line_end_index,
                }],
            },
            ScenePointBinding::ReflectLineConstraint { source_index, line } => Self::MatrixApply {
                source_index: *source_index,
                matrix_apply: vec![PointTransformJson::ReflectLineConstraint {
                    line: LineConstraintJson::from_constraint(line),
                }],
            },
            ScenePointBinding::Rotate {
                source_index,
                center_index,
                angle_degrees,
                parameter_name,
                angle_expr,
                angle_start_index,
                angle_vertex_index,
                angle_end_index,
                angle_parameter_point_index,
                angle_parameter_start_index,
                angle_parameter_end_index,
                angle_parameter_scale,
                ..
            } => Self::MatrixApply {
                source_index: *source_index,
                matrix_apply: vec![PointTransformJson::Rotate {
                    center_index: *center_index,
                    angle_degrees: *angle_degrees,
                    parameter_name: parameter_name.clone(),
                    angle_expr: angle_expr.as_ref().map(FunctionExprJson::from_expr),
                    angle_start_index: *angle_start_index,
                    angle_vertex_index: *angle_vertex_index,
                    angle_end_index: *angle_end_index,
                    angle_parameter_point_index: *angle_parameter_point_index,
                    angle_parameter_start_index: *angle_parameter_start_index,
                    angle_parameter_end_index: *angle_parameter_end_index,
                    angle_parameter_scale: *angle_parameter_scale,
                }],
            },
            ScenePointBinding::ScaleByRatio {
                source_index,
                center_index,
                ratio_origin_index,
                ratio_denominator_index,
                ratio_numerator_index,
                signed,
                clamp_to_unit,
            } => Self::MatrixApply {
                source_index: *source_index,
                matrix_apply: vec![PointTransformJson::ScaleByRatio {
                    center_index: *center_index,
                    ratio_origin_index: *ratio_origin_index,
                    ratio_denominator_index: *ratio_denominator_index,
                    ratio_numerator_index: *ratio_numerator_index,
                    signed: *signed,
                    clamp_to_unit: *clamp_to_unit,
                }],
            },
            ScenePointBinding::Scale {
                source_index,
                center_index,
                factor,
                parameter_name,
                factor_expr,
                factor_parameter_point_index,
                factor_parameter_start_index,
                factor_parameter_end_index,
                ..
            } => Self::MatrixApply {
                source_index: *source_index,
                matrix_apply: vec![PointTransformJson::Scale {
                    center_index: *center_index,
                    factor: *factor,
                    parameter_name: parameter_name.clone(),
                    factor_expr: factor_expr.as_ref().map(FunctionExprJson::from_expr),
                    factor_parameter_point_index: *factor_parameter_point_index,
                    factor_parameter_start_index: *factor_parameter_start_index,
                    factor_parameter_end_index: *factor_parameter_end_index,
                }],
            },
            ScenePointBinding::MarkedAngleTranslation {
                target_index,
                angle_start_index,
                angle_vertex_index,
                angle_end_index,
                distance,
                distance_expr,
                ..
            } => Self::MarkedAngleTranslation {
                target_index: *target_index,
                angle_start_index: *angle_start_index,
                angle_vertex_index: *angle_vertex_index,
                angle_end_index: *angle_end_index,
                distance: *distance,
                distance_expr: FunctionExprJson::from_expr(distance_expr),
            },
            ScenePointBinding::Midpoint {
                start_index,
                end_index,
            } => Self::Midpoint {
                start_index: *start_index,
                end_index: *end_index,
            },
            ScenePointBinding::Circumcenter {
                start_index,
                mid_index,
                end_index,
            } => Self::Circumcenter {
                start_index: *start_index,
                mid_index: *mid_index,
                end_index: *end_index,
            },
            ScenePointBinding::Coordinate { name, expr } => Self::Coordinate {
                name: name.clone(),
                expr: FunctionExprJson::from_expr(expr),
            },
            ScenePointBinding::CoordinateSource {
                source_index,
                name,
                expr,
                parameter_group_ordinals,
                axis,
            } => Self::CoordinateSource {
                source_index: *source_index,
                name: name.clone(),
                expr: FunctionExprJson::from_expr(expr),
                parameter_group_ordinals: parameter_group_ordinals.clone(),
                axis: CoordinateAxisJson::from_axis(*axis),
            },
            ScenePointBinding::CoordinateSource2d {
                source_index,
                x_scalar_group_ordinal,
                x_name,
                x_expr,
                y_scalar_group_ordinal,
                y_name,
                y_expr,
            } => Self::CoordinateSource2d {
                source_index: *source_index,
                x_scalar_group_ordinal: *x_scalar_group_ordinal,
                x_name: x_name.clone(),
                x_expr: FunctionExprJson::from_expr(x_expr),
                y_scalar_group_ordinal: *y_scalar_group_ordinal,
                y_name: y_name.clone(),
                y_expr: FunctionExprJson::from_expr(y_expr),
            },
            ScenePointBinding::PolarOffset {
                source_index,
                distance_expr,
                x_scale,
                y_scale,
                ..
            } => Self::PolarOffset {
                source_index: *source_index,
                distance_expr: FunctionExprJson::from_expr(distance_expr),
                x_scale: *x_scale,
                y_scale: *y_scale,
            },
            ScenePointBinding::PolarTransform {
                source_index,
                distance_expr,
                distance_scale,
                angle_expr,
                angle_degrees_scale,
                ..
            } => Self::PolarTransform {
                source_index: *source_index,
                distance_expr: FunctionExprJson::from_expr(distance_expr),
                distance_scale: *distance_scale,
                angle_expr: FunctionExprJson::from_expr(angle_expr),
                angle_degrees_scale: *angle_degrees_scale,
            },
            ScenePointBinding::RadiusOffset {
                source_index,
                radius,
                x_scale,
                y_scale,
                ..
            } => Self::PolarOffset {
                source_index: *source_index,
                distance_expr: FunctionExprJson::from_expr(&FunctionExpr::Constant(*radius)),
                x_scale: *x_scale,
                y_scale: *y_scale,
            },
            ScenePointBinding::BoundaryLengthOffset {
                source_index,
                boundary,
                x_scale,
                y_scale,
            } => Self::BoundaryLengthOffset {
                source_index: *source_index,
                boundary: CircularConstraintJson::from_constraint(boundary),
                x_scale: *x_scale,
                y_scale: *y_scale,
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

#[derive(Serialize, TS)]
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
    #[serde(rename = "line")]
    Line {
        #[serde(rename = "startIndex")]
        start_index: usize,
        #[serde(rename = "endIndex")]
        end_index: usize,
        t: f64,
    },
    #[serde(rename = "line-constraint")]
    LineConstraint { line: LineConstraintJson, t: f64 },
    #[serde(rename = "ray")]
    Ray {
        #[serde(rename = "startIndex")]
        start_index: usize,
        #[serde(rename = "endIndex")]
        end_index: usize,
        t: f64,
    },
    #[serde(rename = "ray-constraint")]
    RayConstraint { line: LineConstraintJson, t: f64 },
    #[serde(rename = "polyline")]
    Polyline {
        #[serde(rename = "functionKey")]
        function_key: usize,
        points: Vec<PointJson>,
        #[serde(rename = "segmentIndex")]
        segment_index: usize,
        t: f64,
        parameter: f64,
    },
    #[serde(rename = "polygon-boundary")]
    PolygonBoundary {
        #[serde(rename = "vertexIndices")]
        vertex_indices: Vec<usize>,
        #[serde(rename = "edgeIndex")]
        edge_index: usize,
        t: f64,
    },
    #[serde(rename = "polygon-boundary-parameter")]
    PolygonBoundaryParameter {
        #[serde(rename = "vertexIndices")]
        vertex_indices: Vec<usize>,
        parameter: f64,
    },
    #[serde(rename = "polygon-shape-boundary")]
    PolygonShapeBoundary {
        #[serde(rename = "polygonIndex")]
        polygon_index: usize,
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
    #[serde(rename = "circular-constraint")]
    CircularConstraint {
        circle: CircularConstraintJson,
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
    #[serde(rename = "arc-constraint")]
    ArcConstraint { arc: ArcConstraintJson, t: f64 },
    #[serde(rename = "line-intersection")]
    LineIntersection {
        left: LineConstraintJson,
        right: LineConstraintJson,
    },
    #[serde(rename = "line-polygon-intersection")]
    LinePolygonIntersection {
        line: LineConstraintJson,
        #[serde(rename = "vertexIndices")]
        vertex_indices: Vec<usize>,
        variant: usize,
    },
    #[serde(rename = "line-trace-intersection")]
    LineTraceIntersection {
        line: LineConstraintJson,
        #[serde(rename = "traceKey")]
        trace_key: usize,
        #[serde(rename = "pointIndex")]
        point_index: usize,
        #[serde(rename = "xMin")]
        x_min: f64,
        #[serde(rename = "xMax")]
        x_max: f64,
        #[serde(rename = "sampleCount")]
        sample_count: usize,
        variant: usize,
    },
    #[serde(rename = "circular-trace-intersection")]
    CircularTraceIntersection {
        circle: CircularConstraintJson,
        #[serde(rename = "traceKey")]
        trace_key: usize,
        #[serde(rename = "pointIndex")]
        point_index: usize,
        #[serde(rename = "xMin")]
        x_min: f64,
        #[serde(rename = "xMax")]
        x_max: f64,
        #[serde(rename = "sampleCount")]
        sample_count: usize,
        variant: usize,
        #[serde(rename = "sampleHint", skip_serializing_if = "Option::is_none")]
        sample_hint: Option<usize>,
    },
    #[serde(rename = "line-function-intersection")]
    LineFunctionIntersection {
        line: LineConstraintJson,
        #[serde(rename = "functionKey")]
        function_key: usize,
        expr: FunctionExprJson,
        #[serde(rename = "xMin")]
        x_min: f64,
        #[serde(rename = "xMax")]
        x_max: f64,
        #[serde(rename = "sampleCount")]
        sample_count: usize,
        #[serde(rename = "plotMode")]
        plot_mode: PlotModeConstraintJson,
        #[serde(rename = "sampleHint", skip_serializing_if = "Option::is_none")]
        sample_hint: Option<usize>,
    },
    #[serde(rename = "point-circular-tangent")]
    PointCircularTangent {
        #[serde(rename = "pointIndex")]
        point_index: usize,
        circle: CircularConstraintJson,
        variant: usize,
    },
    #[serde(rename = "line-circular-intersection")]
    LineCircularIntersection {
        line: LineConstraintJson,
        circle: CircularConstraintJson,
        variant: usize,
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
            ScenePointConstraint::OnLine {
                start_index,
                end_index,
                t,
            } => Some(Self::Line {
                start_index: *start_index,
                end_index: *end_index,
                t: *t,
            }),
            ScenePointConstraint::OnLineConstraint { line, t } => Some(Self::LineConstraint {
                line: LineConstraintJson::from_constraint(line),
                t: *t,
            }),
            ScenePointConstraint::OnRay {
                start_index,
                end_index,
                t,
            } => Some(Self::Ray {
                start_index: *start_index,
                end_index: *end_index,
                t: *t,
            }),
            ScenePointConstraint::OnRayConstraint { line, t } => Some(Self::RayConstraint {
                line: LineConstraintJson::from_constraint(line),
                t: *t,
            }),
            ScenePointConstraint::OnPolyline {
                function_key,
                points,
                segment_index,
                t,
                parameter,
            } => Some(Self::Polyline {
                function_key: *function_key,
                points: PointJson::collect(points),
                segment_index: *segment_index,
                t: *t,
                parameter: *parameter,
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
            ScenePointConstraint::OnPolygonBoundaryParameter {
                vertex_indices,
                parameter,
            } => Some(Self::PolygonBoundaryParameter {
                vertex_indices: vertex_indices.clone(),
                parameter: *parameter,
            }),
            ScenePointConstraint::OnPolygonShapeBoundary {
                polygon_index,
                edge_index,
                t,
            } => Some(Self::PolygonShapeBoundary {
                polygon_index: *polygon_index,
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
            ScenePointConstraint::OnCircularConstraint {
                circle,
                unit_x,
                unit_y,
            } => Some(Self::CircularConstraint {
                circle: CircularConstraintJson::from_constraint(circle),
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
            ScenePointConstraint::OnArcConstraint { arc, t } => Some(Self::ArcConstraint {
                arc: ArcConstraintJson::from_constraint(arc),
                t: *t,
            }),
            ScenePointConstraint::LineIntersection { left, right } => {
                Some(Self::LineIntersection {
                    left: LineConstraintJson::from_constraint(left),
                    right: LineConstraintJson::from_constraint(right),
                })
            }
            ScenePointConstraint::LinePolygonIntersection {
                line,
                vertex_indices,
                variant,
            } => Some(Self::LinePolygonIntersection {
                line: LineConstraintJson::from_constraint(line),
                vertex_indices: vertex_indices.clone(),
                variant: *variant,
            }),
            ScenePointConstraint::LineTraceIntersection {
                line,
                trace_key,
                point_index,
                x_min,
                x_max,
                sample_count,
                variant,
            } => Some(Self::LineTraceIntersection {
                line: LineConstraintJson::from_constraint(line),
                trace_key: *trace_key,
                point_index: *point_index,
                x_min: *x_min,
                x_max: *x_max,
                sample_count: *sample_count,
                variant: *variant,
            }),
            ScenePointConstraint::CircularTraceIntersection {
                circle,
                trace_key,
                point_index,
                x_min,
                x_max,
                sample_count,
                variant,
                sample_hint,
            } => Some(Self::CircularTraceIntersection {
                circle: CircularConstraintJson::from_constraint(circle),
                trace_key: *trace_key,
                point_index: *point_index,
                x_min: *x_min,
                x_max: *x_max,
                sample_count: *sample_count,
                variant: *variant,
                sample_hint: *sample_hint,
            }),
            ScenePointConstraint::LineFunctionIntersection {
                line,
                function_key,
                expr,
                x_min,
                x_max,
                sample_count,
                polar,
                sample_hint,
            } => Some(Self::LineFunctionIntersection {
                line: LineConstraintJson::from_constraint(line),
                function_key: *function_key,
                expr: FunctionExprJson::from_expr(expr),
                x_min: *x_min,
                x_max: *x_max,
                sample_count: *sample_count,
                plot_mode: PlotModeConstraintJson::from_polar(*polar),
                sample_hint: *sample_hint,
            }),
            ScenePointConstraint::PointCircularTangent {
                point_index,
                circle,
                variant,
            } => Some(Self::PointCircularTangent {
                point_index: *point_index,
                circle: CircularConstraintJson::from_constraint(circle),
                variant: *variant,
            }),
            ScenePointConstraint::LineCircularIntersection {
                line,
                circle,
                variant,
            } => Some(Self::LineCircularIntersection {
                line: LineConstraintJson::from_constraint(line),
                circle: CircularConstraintJson::from_constraint(circle),
                variant: *variant,
            }),
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

#[derive(Serialize, TS)]
#[serde(tag = "kind", rename_all = "kebab-case")]
enum ArcConstraintJson {
    CenterArc {
        #[serde(rename = "centerIndex")]
        center_index: usize,
        #[serde(rename = "startIndex")]
        start_index: usize,
        #[serde(rename = "endIndex")]
        end_index: usize,
    },
    CircleArc {
        circle: CircularConstraintJson,
        #[serde(rename = "startIndex")]
        start_index: usize,
        #[serde(rename = "endIndex")]
        end_index: usize,
    },
    ThreePointArc {
        #[serde(rename = "startIndex")]
        start_index: usize,
        #[serde(rename = "midIndex")]
        mid_index: usize,
        #[serde(rename = "endIndex")]
        end_index: usize,
    },
    MatrixApply {
        source: Box<ArcConstraintJson>,
        #[serde(rename = "matrixApply")]
        matrix_apply: Vec<TransformJson>,
    },
}

impl ArcConstraintJson {
    fn from_constraint(constraint: &ArcConstraint) -> Self {
        match constraint {
            ArcConstraint::CenterArc {
                center_index,
                start_index,
                end_index,
            } => Self::CenterArc {
                center_index: *center_index,
                start_index: *start_index,
                end_index: *end_index,
            },
            ArcConstraint::CircleArc {
                circle,
                start_index,
                end_index,
            } => Self::CircleArc {
                circle: CircularConstraintJson::from_constraint(circle),
                start_index: *start_index,
                end_index: *end_index,
            },
            ArcConstraint::ThreePointArc {
                start_index,
                mid_index,
                end_index,
            } => Self::ThreePointArc {
                start_index: *start_index,
                mid_index: *mid_index,
                end_index: *end_index,
            },
            ArcConstraint::MatrixApply { source, matrices } => Self::MatrixApply {
                source: Box::new(Self::from_constraint(source)),
                matrix_apply: matrices.iter().map(TransformJson::from_transform).collect(),
            },
        }
    }
}

#[derive(Serialize, TS)]
#[serde(rename_all = "kebab-case")]
enum PlotModeConstraintJson {
    Cartesian,
    Polar,
}

impl PlotModeConstraintJson {
    fn from_polar(polar: bool) -> Self {
        match polar {
            true => Self::from_mode(FunctionPlotMode::Polar),
            false => Self::from_mode(FunctionPlotMode::Cartesian),
        }
    }

    fn from_mode(mode: FunctionPlotMode) -> Self {
        match mode {
            FunctionPlotMode::Cartesian => Self::Cartesian,
            FunctionPlotMode::Polar => Self::Polar,
        }
    }
}

#[derive(Serialize, TS)]
#[serde(tag = "kind", rename_all = "kebab-case")]
enum CircularConstraintJson {
    Circle {
        #[serde(rename = "centerIndex")]
        center_index: usize,
        #[serde(rename = "radiusIndex")]
        radius_index: usize,
    },
    SegmentRadiusCircle {
        #[serde(rename = "centerIndex")]
        center_index: usize,
        #[serde(rename = "lineStartIndex")]
        line_start_index: usize,
        #[serde(rename = "lineEndIndex")]
        line_end_index: usize,
    },
    ParameterRadiusCircle {
        #[serde(rename = "centerIndex")]
        center_index: usize,
        #[serde(rename = "parameterName")]
        parameter_name: String,
        #[serde(rename = "parameterValue")]
        parameter_value: f64,
        #[serde(rename = "rawPerUnit")]
        raw_per_unit: f64,
    },
    ExpressionRadiusCircle {
        #[serde(rename = "centerIndex")]
        center_index: usize,
        expr: FunctionExprJson,
        #[serde(rename = "initialValue")]
        initial_value: f64,
    },
    MatrixApply {
        source: Box<CircularConstraintJson>,
        #[serde(rename = "matrixApply")]
        matrix_apply: Vec<TransformJson>,
    },
    CircleArc {
        #[serde(rename = "centerIndex")]
        center_index: usize,
        #[serde(rename = "startIndex")]
        start_index: usize,
        #[serde(rename = "endIndex")]
        end_index: usize,
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
            CircularConstraint::SegmentRadiusCircle {
                center_index,
                line_start_index,
                line_end_index,
            } => Self::SegmentRadiusCircle {
                center_index: *center_index,
                line_start_index: *line_start_index,
                line_end_index: *line_end_index,
            },
            CircularConstraint::ParameterRadiusCircle {
                center_index,
                parameter_name,
                parameter_value,
                raw_per_unit,
            } => Self::ParameterRadiusCircle {
                center_index: *center_index,
                parameter_name: parameter_name.clone(),
                parameter_value: *parameter_value,
                raw_per_unit: *raw_per_unit,
            },
            CircularConstraint::ExpressionRadiusCircle {
                center_index,
                expr,
                initial_value,
                ..
            } => Self::ExpressionRadiusCircle {
                center_index: *center_index,
                expr: FunctionExprJson::from_expr(expr),
                initial_value: *initial_value,
            },
            CircularConstraint::MatrixApply { source, matrices } => Self::MatrixApply {
                source: Box::new(Self::from_constraint(source)),
                matrix_apply: matrices.iter().map(TransformJson::from_transform).collect(),
            },
            CircularConstraint::CircleArc {
                center_index,
                start_index,
                end_index,
            } => Self::CircleArc {
                center_index: *center_index,
                start_index: *start_index,
                end_index: *end_index,
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

#[derive(Serialize, TS)]
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
    #[serde(rename = "angle-bisector-ray")]
    AngleBisectorRay {
        #[serde(rename = "startIndex")]
        start_index: usize,
        #[serde(rename = "vertexIndex")]
        vertex_index: usize,
        #[serde(rename = "endIndex")]
        end_index: usize,
    },
    MatrixApply {
        source: Box<LineConstraintJson>,
        #[serde(rename = "matrixApply")]
        matrix_apply: Vec<LineConstraintMatrixJson>,
    },
}

#[derive(Serialize, TS)]
#[serde(tag = "kind", rename_all = "kebab-case")]
enum LineConstraintMatrixJson {
    TranslateVector {
        #[serde(rename = "vectorStartIndex")]
        vector_start_index: usize,
        #[serde(rename = "vectorEndIndex")]
        vector_end_index: usize,
    },
    TranslateDelta {
        dx: f64,
        dy: f64,
    },
    Reflect {
        axis: Box<LineConstraintJson>,
    },
    Rotate {
        #[serde(rename = "centerIndex")]
        center_index: usize,
        #[serde(rename = "angleDegrees")]
        angle_degrees: f64,
        #[serde(rename = "parameterName", skip_serializing_if = "Option::is_none")]
        parameter_name: Option<String>,
        #[serde(rename = "angleExpr", skip_serializing_if = "Option::is_none")]
        angle_expr: Option<FunctionExprJson>,
        #[serde(rename = "angleStartIndex", skip_serializing_if = "Option::is_none")]
        angle_start_index: Option<usize>,
        #[serde(rename = "angleVertexIndex", skip_serializing_if = "Option::is_none")]
        angle_vertex_index: Option<usize>,
        #[serde(rename = "angleEndIndex", skip_serializing_if = "Option::is_none")]
        angle_end_index: Option<usize>,
    },
    RotateSourcePoint {
        #[serde(rename = "sourcePointIndex")]
        source_point_index: usize,
        #[serde(rename = "angleDegrees")]
        angle_degrees: f64,
    },
    TranslateSourcePoint {
        #[serde(rename = "sourcePointIndex")]
        source_point_index: usize,
        #[serde(rename = "targetIndex")]
        target_index: usize,
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
            LineConstraint::AngleBisectorRay {
                start_index,
                vertex_index,
                end_index,
            } => Self::AngleBisectorRay {
                start_index: *start_index,
                vertex_index: *vertex_index,
                end_index: *end_index,
            },
            LineConstraint::MatrixApply { source, matrices } => Self::MatrixApply {
                source: Box::new(Self::from_constraint(source)),
                matrix_apply: matrices
                    .iter()
                    .map(LineConstraintMatrixJson::from_matrix)
                    .collect(),
            },
        }
    }
}

impl LineConstraintMatrixJson {
    fn from_matrix(matrix: &crate::runtime::scene::LineConstraintMatrix) -> Self {
        match matrix {
            crate::runtime::scene::LineConstraintMatrix::TranslateVector {
                vector_start_index,
                vector_end_index,
            } => Self::TranslateVector {
                vector_start_index: *vector_start_index,
                vector_end_index: *vector_end_index,
            },
            crate::runtime::scene::LineConstraintMatrix::TranslateDelta { dx, dy } => {
                Self::TranslateDelta { dx: *dx, dy: *dy }
            }
            crate::runtime::scene::LineConstraintMatrix::Reflect { axis } => Self::Reflect {
                axis: Box::new(LineConstraintJson::from_constraint(axis)),
            },
            crate::runtime::scene::LineConstraintMatrix::Rotate { rotation } => Self::Rotate {
                center_index: rotation.center_index,
                angle_degrees: rotation.angle_degrees,
                parameter_name: rotation.parameter_name.clone(),
                angle_expr: rotation
                    .angle_expr
                    .as_ref()
                    .map(FunctionExprJson::from_expr),
                angle_start_index: rotation.angle_start_index,
                angle_vertex_index: rotation.angle_vertex_index,
                angle_end_index: rotation.angle_end_index,
            },
            crate::runtime::scene::LineConstraintMatrix::RotateAroundSourcePoint {
                source_point_index,
                angle_degrees,
            } => Self::RotateSourcePoint {
                source_point_index: *source_point_index,
                angle_degrees: *angle_degrees,
            },
            crate::runtime::scene::LineConstraintMatrix::TranslateSourcePointToPoint {
                source_point_index,
                target_index,
            } => Self::TranslateSourcePoint {
                source_point_index: *source_point_index,
                target_index: *target_index,
            },
        }
    }
}
