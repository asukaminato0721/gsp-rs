use super::function_expr_json::FunctionExprJson;
use super::scene_json::{DebugSourceJson, PointJson};
use crate::runtime::scene::{
    CircularConstraint, LineConstraint, ScenePointBinding, ScenePointConstraint,
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
    #[serde(rename = "parameter")]
    Parameter { name: String },
    #[serde(rename = "derived-parameter")]
    DerivedParameter {
        #[serde(rename = "sourceIndex")]
        source_index: usize,
    },
    #[serde(rename = "constraint-parameter-expr")]
    ConstraintParameterExpr { expr: FunctionExprJson },
    #[serde(rename = "constraint-parameter-from-point-expr")]
    ConstraintParameterFromPointExpr {
        #[serde(rename = "sourceIndex")]
        source_index: usize,
        #[serde(rename = "parameterName")]
        parameter_name: String,
        expr: FunctionExprJson,
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
    #[serde(rename = "reflect-line-constraint")]
    ReflectLineConstraint {
        #[serde(rename = "sourceIndex")]
        source_index: usize,
        line: LineConstraintJson,
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
        #[serde(rename = "angleExpr", skip_serializing_if = "Option::is_none")]
        angle_expr: Option<FunctionExprJson>,
    },
    #[serde(rename = "scale-by-ratio")]
    ScaleByRatio {
        #[serde(rename = "sourceIndex")]
        source_index: usize,
        #[serde(rename = "centerIndex")]
        center_index: usize,
        #[serde(rename = "ratioOriginIndex")]
        ratio_origin_index: usize,
        #[serde(rename = "ratioDenominatorIndex")]
        ratio_denominator_index: usize,
        #[serde(rename = "ratioNumeratorIndex")]
        ratio_numerator_index: usize,
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
    #[serde(rename = "coordinate-source")]
    CoordinateSource {
        #[serde(rename = "sourceIndex")]
        source_index: usize,
        name: String,
        expr: FunctionExprJson,
        axis: CoordinateAxisJson,
    },
    #[serde(rename = "coordinate-source-2d")]
    CoordinateSource2d {
        #[serde(rename = "sourceIndex")]
        source_index: usize,
        #[serde(rename = "xName")]
        x_name: String,
        #[serde(rename = "xExpr")]
        x_expr: FunctionExprJson,
        #[serde(rename = "yName")]
        y_name: String,
        #[serde(rename = "yExpr")]
        y_expr: FunctionExprJson,
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
            ScenePointBinding::Parameter { name } => Self::Parameter { name: name.clone() },
            ScenePointBinding::DerivedParameter { source_index } => Self::DerivedParameter {
                source_index: *source_index,
            },
            ScenePointBinding::ConstraintParameterExpr { expr } => Self::ConstraintParameterExpr {
                expr: FunctionExprJson::from_expr(expr),
            },
            ScenePointBinding::ConstraintParameterFromPointExpr {
                source_index,
                parameter_name,
                expr,
            } => Self::ConstraintParameterFromPointExpr {
                source_index: *source_index,
                parameter_name: parameter_name.clone(),
                expr: FunctionExprJson::from_expr(expr),
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
            ScenePointBinding::ReflectLineConstraint { source_index, line } => {
                Self::ReflectLineConstraint {
                    source_index: *source_index,
                    line: LineConstraintJson::from_constraint(line),
                }
            }
            ScenePointBinding::Rotate {
                source_index,
                center_index,
                angle_degrees,
                parameter_name,
                angle_expr,
            } => Self::Rotate {
                source_index: *source_index,
                center_index: *center_index,
                angle_degrees: *angle_degrees,
                parameter_name: parameter_name.clone(),
                angle_expr: angle_expr.as_ref().map(FunctionExprJson::from_expr),
            },
            ScenePointBinding::ScaleByRatio {
                source_index,
                center_index,
                ratio_origin_index,
                ratio_denominator_index,
                ratio_numerator_index,
            } => Self::ScaleByRatio {
                source_index: *source_index,
                center_index: *center_index,
                ratio_origin_index: *ratio_origin_index,
                ratio_denominator_index: *ratio_denominator_index,
                ratio_numerator_index: *ratio_numerator_index,
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
            ScenePointBinding::CoordinateSource {
                source_index,
                name,
                expr,
                axis,
            } => Self::CoordinateSource {
                source_index: *source_index,
                name: name.clone(),
                expr: FunctionExprJson::from_expr(expr),
                axis: CoordinateAxisJson::from_axis(*axis),
            },
            ScenePointBinding::CoordinateSource2d {
                source_index,
                x_name,
                x_expr,
                y_name,
                y_expr,
            } => Self::CoordinateSource2d {
                source_index: *source_index,
                x_name: x_name.clone(),
                x_expr: FunctionExprJson::from_expr(x_expr),
                y_name: y_name.clone(),
                y_expr: FunctionExprJson::from_expr(y_expr),
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
    #[serde(rename = "ray")]
    Ray {
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
    #[serde(rename = "line-trace-intersection")]
    LineTraceIntersection {
        line: LineConstraintJson,
        #[serde(rename = "pointIndex")]
        point_index: usize,
        #[serde(rename = "xMin")]
        x_min: f64,
        #[serde(rename = "xMax")]
        x_max: f64,
        #[serde(rename = "sampleCount")]
        sample_count: usize,
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
            ScenePointConstraint::OnRay {
                start_index,
                end_index,
                t,
            } => Some(Self::Ray {
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
                points: PointJson::collect(points),
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
            ScenePointConstraint::LineTraceIntersection {
                line,
                point_index,
                x_min,
                x_max,
                sample_count,
            } => Some(Self::LineTraceIntersection {
                line: LineConstraintJson::from_constraint(line),
                point_index: *point_index,
                x_min: *x_min,
                x_max: *x_max,
                sample_count: *sample_count,
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
    ScaleCircle {
        source: Box<CircularConstraintJson>,
        #[serde(rename = "centerIndex")]
        center_index: usize,
        factor: f64,
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
            CircularConstraint::ScaleCircle {
                source,
                center_index,
                factor,
            } => Self::ScaleCircle {
                source: Box::new(Self::from_constraint(source)),
                center_index: *center_index,
                factor: *factor,
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
    Translated {
        line: Box<LineConstraintJson>,
        #[serde(rename = "vectorStartIndex")]
        vector_start_index: usize,
        #[serde(rename = "vectorEndIndex")]
        vector_end_index: usize,
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
            LineConstraint::Translated {
                line,
                vector_start_index,
                vector_end_index,
            } => Self::Translated {
                line: Box::new(Self::from_constraint(line)),
                vector_start_index: *vector_start_index,
                vector_end_index: *vector_end_index,
            },
        }
    }
}
