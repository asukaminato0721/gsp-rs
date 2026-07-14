use crate::object_graph::{ObjectGraph, ObjectNode, ObjectValues};
use crate::{
    BinaryOp, CoordinateTraceMode, FunctionAst, FunctionExpr, LineKind, PlotMode, Point,
    UnaryFunction, affine_iteration_segment, angle_bisector_direction, angle_marker_points,
    choose_point_candidate, circle_arc_control_points, circle_circle_intersections,
    directed_angle_anchor, evaluate_expr, lerp_point, line_circle_intersection_candidate,
    line_circle_intersections, line_line_intersection, line_polyline_intersection,
    measured_rotation_radians, object_graph::OperationTable, point_angle_degrees,
    point_circle_tangents, point_distance, point_distance_ratio, point_on_three_point_arc,
    point_on_three_point_arc_complement, polygon_area, project_to_line_like, reflect_across_line,
    rotate_around, sample_circle_arc, sample_coordinate_trace, sample_custom_transform_trace,
    sample_parametric_curve, sample_three_point_arc, scale_around, scale_by_three_point_ratio,
    segment_marker_points, three_point_arc_geometry, translation_iteration_deltas,
};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ObjectGraphEvaluationInput {
    pub nodes: Vec<ObjectNode<ObjectOp>>,
    pub sources: Vec<ObjectSourceValue>,
}

#[derive(Debug, Clone, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct ObjectSourceValue {
    pub id: String,
    pub value: ObjectValue,
}

#[derive(Debug, Clone, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct ObjectNodeValue {
    pub id: String,
    pub value: ObjectValue,
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ObjectCircle {
    pub center: Point,
    pub radius_point: Point,
}

#[derive(Debug, Clone, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ObjectProgram {
    pub nodes: Vec<ObjectNode<ObjectOp>>,
    pub source_ids: Vec<String>,
    pub target_id: String,
}

#[derive(Debug, Clone, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum TraceDriver {
    Scalar {
        source_id: String,
        normalized: bool,
    },
    Circle {
        unit_x_source_id: String,
        unit_y_source_id: String,
    },
}

pub fn evaluate_object_graph_json(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let input = serde_json::from_slice::<ObjectGraphEvaluationInput>(bytes)
        .map_err(|error| format!("invalid object graph input: {error}"))?;
    let graph = ObjectGraph::build(input.nodes)
        .map_err(|error| format!("invalid object graph: {error}"))?;
    let mut values = ObjectValues::new(&graph);
    for source in input.sources {
        values
            .set_source::<_, ObjectOpError>(&graph, &source.id, source.value)
            .map_err(|error| error.to_string())?;
    }
    values
        .evaluate_all(&graph, &mut BuiltinOperationTable)
        .map_err(|error| error.to_string())?;
    let output = graph
        .nodes()
        .iter()
        .map(|node| {
            values
                .get(&graph, &node.id)
                .cloned()
                .map(|value| ObjectNodeValue {
                    id: node.id.clone(),
                    value,
                })
                .ok_or_else(|| format!("object graph node {} has no value", node.id))
        })
        .collect::<Result<Vec<_>, _>>()?;
    serde_json::to_vec(&output).map_err(|error| format!("failed to encode object graph: {error}"))
}

#[derive(Debug, Clone, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum ObjectValue {
    Scalar {
        value: f64,
    },
    Point {
        x: f64,
        y: f64,
    },
    Line {
        line_kind: LineKind,
        start: Point,
        end: Point,
    },
    Circle {
        center: Point,
        radius_point: Point,
    },
    Arc {
        start: Point,
        mid: Point,
        end: Point,
        center: Option<Point>,
        counterclockwise: bool,
        complement: bool,
    },
    Points {
        points: Vec<Point>,
    },
    Polygons {
        polygons: Vec<Vec<Point>>,
    },
    Circles {
        circles: Vec<ObjectCircle>,
    },
    Color {
        color: [u8; 4],
    },
    Text {
        value: String,
    },
}

impl ObjectValue {
    pub fn point(point: Point) -> Self {
        Self::Point {
            x: point.x,
            y: point.y,
        }
    }

    pub fn as_point(&self) -> Option<Point> {
        match self {
            Self::Point { x, y } => Some(Point { x: *x, y: *y }),
            _ => None,
        }
    }

    pub fn as_scalar(&self) -> Option<f64> {
        match self {
            Self::Scalar { value } => Some(*value),
            _ => None,
        }
    }

    pub fn as_points(&self) -> Option<Vec<Point>> {
        match self {
            Self::Point { .. } => self.as_point().map(|point| vec![point]),
            Self::Line { start, end, .. } => Some(vec![*start, *end]),
            Self::Circle {
                center,
                radius_point,
            } => Some(vec![*center, *radius_point]),
            Self::Arc {
                start, mid, end, ..
            } => Some(vec![*start, *mid, *end]),
            Self::Points { points } => Some(points.clone()),
            Self::Scalar { .. }
            | Self::Polygons { .. }
            | Self::Circles { .. }
            | Self::Color { .. }
            | Self::Text { .. } => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum ObjectOp {
    Copy,
    PointOffset {
        dx: f64,
        dy: f64,
    },
    PointScaledOffset {
        x_scale: f64,
        y_scale: f64,
    },
    PointOffsetByScalars,
    PointFromScalars,
    DirectedAngleAnchor {
        distance: f64,
        parameter: f64,
    },
    PointOnLine,
    PointOnCircle {
        invert_y: bool,
    },
    PointOnCircleParameter {
        invert_y: bool,
    },
    PointOnArc,
    PointOnPolyline,
    PointOnGeneratedTrace {
        program: ObjectProgram,
        driver: TraceDriver,
        value_min: f64,
        value_max: f64,
    },
    PointOnPolylineSegment {
        segment_index: usize,
    },
    PointOnPolygonBoundary,
    Midpoint,
    Circumcenter,
    TranslatePoint,
    ReflectPoint,
    ReflectPointAcrossLine,
    RotatePoint {
        radians: f64,
    },
    RotatePointDegrees,
    ScalePoint {
        factor: f64,
    },
    ScalePointByScalar,
    ScalePointByRatio {
        signed: bool,
        clamp_to_unit: bool,
    },
    Line {
        line_kind: LineKind,
    },
    PerpendicularLine,
    ParallelLine,
    AngleBisectorRay,
    AngleMarker {
        marker_class: u32,
    },
    SegmentMarker {
        t: f64,
        marker_class: u32,
    },
    CoordinateTrace {
        x_expression: ObjectExpression,
        y_expression: Option<ObjectExpression>,
        parameter_names: Vec<String>,
        trace_parameter_name: String,
        value_min: f64,
        value_max: f64,
        sample_count: usize,
        mode: CoordinateTraceMode,
    },
    CartesianParameterTrace {
        expression: ObjectExpression,
        parameter_names: Vec<String>,
        trace_parameter_name: String,
        value_min: f64,
        value_max: f64,
        sample_count: usize,
    },
    ParametricCurve {
        x_expression: ObjectExpression,
        y_expression: ObjectExpression,
        parameter_names: Vec<String>,
        value_min: f64,
        value_max: f64,
        sample_count: usize,
    },
    FunctionPlot {
        expression: ObjectExpression,
        parameter_names: Vec<String>,
        value_min: f64,
        value_max: f64,
        sample_count: usize,
        plot_mode: PlotMode,
    },
    CustomTransformPoint {
        distance_expression: ObjectExpression,
        angle_expression: ObjectExpression,
        distance_parameter_names: Vec<String>,
        angle_parameter_names: Vec<String>,
        distance_scale: f64,
        angle_degrees_scale: f64,
    },
    CustomTransformTrace {
        distance_expression: ObjectExpression,
        angle_expression: ObjectExpression,
        distance_parameter_names: Vec<String>,
        angle_parameter_names: Vec<String>,
        value_min: f64,
        value_max: f64,
        sample_count: usize,
        distance_scale: f64,
        angle_degrees_scale: f64,
    },
    PointTrace {
        program: ObjectProgram,
        driver: TraceDriver,
        value_min: f64,
        value_max: f64,
        sample_count: usize,
    },
    RepeatPoint {
        sample_count: usize,
    },
    ZipPointTraces,
    LinePolylineIntersection {
        variant: usize,
        sample_hint: Option<f64>,
    },
    CircleByPoints,
    CircleBySegmentRadius,
    CircleByRadius,
    ThreePointArc {
        complement: bool,
    },
    CenterArc {
        y_up: bool,
    },
    CircleArc {
        y_up: bool,
    },
    ArcBoundaryPoints {
        center_arc: bool,
        sector: bool,
        reversed: bool,
        complement: bool,
        steps: usize,
        y_up: bool,
    },
    ArcLength,
    CircularRadius,
    Polygon,
    SimilarityPolygonIteration {
        inverse: bool,
    },
    PointOffsetIteration {
        dx: f64,
        dy: f64,
    },
    PointRotateIteration,
    ParameterizedPointIteration {
        program: ObjectProgram,
        trace_source_id: String,
        trace_parameter_name: String,
        step_expression: ObjectExpression,
        step_parameter_names: Vec<String>,
    },
    LineTranslateIteration {
        dx: f64,
        dy: f64,
        secondary_dx: Option<f64>,
        secondary_dy: Option<f64>,
        bidirectional: bool,
        vector_from_parents: bool,
    },
    LineRotateIteration,
    LineAffineIteration,
    TranslatePolygonIteration {
        vertex_count: usize,
        dx: f64,
        dy: f64,
        secondary_dx: Option<f64>,
        secondary_dy: Option<f64>,
        bidirectional: bool,
        vector_from_parents: bool,
    },
    CircleIteration {
        vertex_count: usize,
    },
    LineIntersection,
    LineCircleIntersection {
        variant: usize,
    },
    CircleCircleIntersection {
        variant: usize,
    },
    PointCircleTangent {
        variant: usize,
    },
    PointDistance {
        value_scale: f64,
    },
    PointDistanceRatio {
        clamp_to_unit: bool,
    },
    PointAngleDegrees,
    PointCoordinate {
        vertical: bool,
    },
    MeasuredRotationDegrees,
    PointLineParameter {
        line_kind: LineKind,
    },
    CircleParameter {
        invert_y: bool,
    },
    PolygonBoundaryParameter {
        edge_index: usize,
    },
    PolygonBoundaryParameterFromPoint,
    PolygonArea {
        value_scale: f64,
    },
    EvaluateExpression {
        expression: ObjectExpression,
        parameter_names: Vec<String>,
        x: f64,
    },
    ScaleScalar {
        factor: f64,
    },
    AbsoluteScalar,
    SpectrumColor {
        base_value: f64,
        period: f64,
        base_color: [u8; 4],
    },
    RgbColor {
        alpha: u8,
    },
    HsbColor {
        alpha: u8,
    },
    TranslateShapeDelta {
        dx: f64,
        dy: f64,
    },
    TranslateShape,
    ReflectShape,
    ReflectShapeAcrossLine,
    RotateShape {
        radians: f64,
    },
    RotateShapeDegrees,
    ScaleShape {
        factor: f64,
    },
}

#[derive(Debug, Clone, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum ObjectExpression {
    Constant {
        value: f64,
    },
    Variable,
    PiConstant,
    EulerConstant,
    PiAngle,
    Parameter {
        name: String,
        default: f64,
    },
    Unary {
        op: UnaryFunction,
        expression: Box<ObjectExpression>,
    },
    Binary {
        left: Box<ObjectExpression>,
        op: BinaryOp,
        right: Box<ObjectExpression>,
    },
}

impl ObjectExpression {
    pub fn from_function_expr(expression: &FunctionExpr) -> Self {
        match expression {
            FunctionExpr::Constant(value) => Self::Constant { value: *value },
            FunctionExpr::Identity => Self::Variable,
            FunctionExpr::SinIdentity => Self::Unary {
                op: UnaryFunction::Sin,
                expression: Box::new(Self::Variable),
            },
            FunctionExpr::CosIdentityPlus(offset) => Self::Binary {
                left: Box::new(Self::Unary {
                    op: UnaryFunction::Cos,
                    expression: Box::new(Self::Variable),
                }),
                op: BinaryOp::Add,
                right: Box::new(Self::Constant { value: *offset }),
            },
            FunctionExpr::TanIdentityMinus(offset) => Self::Binary {
                left: Box::new(Self::Unary {
                    op: UnaryFunction::Tan,
                    expression: Box::new(Self::Variable),
                }),
                op: BinaryOp::Sub,
                right: Box::new(Self::Constant { value: *offset }),
            },
            FunctionExpr::Parsed(expression) => Self::from_function_ast(expression),
        }
    }

    fn from_function_ast(expression: &FunctionAst) -> Self {
        match expression {
            FunctionAst::Variable => Self::Variable,
            FunctionAst::Constant(value) => Self::Constant { value: *value },
            FunctionAst::PiConstant => Self::PiConstant,
            FunctionAst::EulerConstant => Self::EulerConstant,
            FunctionAst::PiAngle => Self::PiAngle,
            FunctionAst::Parameter(name, default) => Self::Parameter {
                name: name.clone(),
                default: *default,
            },
            FunctionAst::Unary { op, expr } => Self::Unary {
                op: *op,
                expression: Box::new(Self::from_function_ast(expr)),
            },
            FunctionAst::Binary { lhs, op, rhs } => Self::Binary {
                left: Box::new(Self::from_function_ast(lhs)),
                op: *op,
                right: Box::new(Self::from_function_ast(rhs)),
            },
        }
    }

    fn to_function_ast(&self) -> FunctionAst {
        match self {
            Self::Constant { value } => FunctionAst::Constant(*value),
            Self::Variable => FunctionAst::Variable,
            Self::PiConstant => FunctionAst::PiConstant,
            Self::EulerConstant => FunctionAst::EulerConstant,
            Self::PiAngle => FunctionAst::PiAngle,
            Self::Parameter { name, default } => FunctionAst::Parameter(name.clone(), *default),
            Self::Unary { op, expression } => FunctionAst::Unary {
                op: *op,
                expr: Box::new(expression.to_function_ast()),
            },
            Self::Binary { left, op, right } => FunctionAst::Binary {
                lhs: Box::new(left.to_function_ast()),
                op: *op,
                rhs: Box::new(right.to_function_ast()),
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObjectOpError {
    WrongArity {
        op: &'static str,
        expected: usize,
        actual: usize,
    },
    ExpectedPoint {
        op: &'static str,
        parent: usize,
    },
    ExpectedScalar {
        op: &'static str,
        parent: usize,
    },
    ExpectedShape {
        op: &'static str,
        parent: usize,
    },
    ExpectedLine {
        op: &'static str,
        parent: usize,
    },
    ExpectedCircle {
        op: &'static str,
        parent: usize,
    },
    ExpectedArc {
        op: &'static str,
        parent: usize,
    },
    Degenerate {
        op: &'static str,
    },
    InvalidProgram {
        op: &'static str,
        message: String,
    },
}

impl std::fmt::Display for ObjectOpError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::WrongArity {
                op,
                expected,
                actual,
            } => write!(
                formatter,
                "operation {op} expected {expected} parents but received {actual}"
            ),
            Self::ExpectedPoint { op, parent } => {
                write!(
                    formatter,
                    "operation {op} expected parent {parent} to be a point"
                )
            }
            Self::ExpectedScalar { op, parent } => write!(
                formatter,
                "operation {op} expected parent {parent} to be a scalar"
            ),
            Self::ExpectedShape { op, parent } => {
                write!(
                    formatter,
                    "operation {op} expected parent {parent} to be a shape"
                )
            }
            Self::ExpectedLine { op, parent } => {
                write!(
                    formatter,
                    "operation {op} expected parent {parent} to be a line"
                )
            }
            Self::ExpectedCircle { op, parent } => write!(
                formatter,
                "operation {op} expected parent {parent} to be a circle"
            ),
            Self::ExpectedArc { op, parent } => {
                write!(
                    formatter,
                    "operation {op} expected parent {parent} to be an arc"
                )
            }
            Self::Degenerate { op } => write!(formatter, "operation {op} is degenerate"),
            Self::InvalidProgram { op, message } => {
                write!(
                    formatter,
                    "operation {op} has an invalid program: {message}"
                )
            }
        }
    }
}

#[derive(Debug, Default)]
pub struct BuiltinOperationTable;

impl OperationTable<ObjectOp, ObjectValue> for BuiltinOperationTable {
    type Error = ObjectOpError;

    fn evaluate(
        &mut self,
        _node_id: &str,
        op: &ObjectOp,
        parents: &[&ObjectValue],
    ) -> Result<ObjectValue, Self::Error> {
        match op {
            ObjectOp::Copy => {
                expect_arity("copy", parents, 1)?;
                Ok(parents[0].clone())
            }
            ObjectOp::PointOffset { dx, dy } => {
                expect_arity("point-offset", parents, 1)?;
                let origin = expect_point("point-offset", parents, 0)?;
                Ok(ObjectValue::point(Point {
                    x: origin.x + dx,
                    y: origin.y + dy,
                }))
            }
            ObjectOp::PointScaledOffset { x_scale, y_scale } => {
                expect_arity("point-scaled-offset", parents, 2)?;
                let source = expect_point("point-scaled-offset", parents, 0)?;
                let distance = expect_scalar("point-scaled-offset", parents, 1)?;
                Ok(ObjectValue::point(Point {
                    x: source.x + distance * x_scale,
                    y: source.y + distance * y_scale,
                }))
            }
            ObjectOp::PointOffsetByScalars => {
                expect_arity("point-offset-by-scalars", parents, 3)?;
                let source = expect_point("point-offset-by-scalars", parents, 0)?;
                Ok(ObjectValue::point(Point {
                    x: source.x + expect_scalar("point-offset-by-scalars", parents, 1)?,
                    y: source.y + expect_scalar("point-offset-by-scalars", parents, 2)?,
                }))
            }
            ObjectOp::PointFromScalars => {
                expect_arity("point-from-scalars", parents, 2)?;
                Ok(ObjectValue::point(Point {
                    x: expect_scalar("point-from-scalars", parents, 0)?,
                    y: expect_scalar("point-from-scalars", parents, 1)?,
                }))
            }
            ObjectOp::DirectedAngleAnchor {
                distance,
                parameter,
            } => {
                expect_arity("directed-angle-anchor", parents, 4)?;
                let point = directed_angle_anchor(
                    expect_point("directed-angle-anchor", parents, 0)?,
                    expect_point("directed-angle-anchor", parents, 1)?,
                    expect_point("directed-angle-anchor", parents, 2)?,
                    expect_point("directed-angle-anchor", parents, 3)?,
                    *distance,
                    *parameter,
                )
                .ok_or(ObjectOpError::Degenerate {
                    op: "directed-angle-anchor",
                })?;
                Ok(ObjectValue::point(point))
            }
            ObjectOp::PointOnLine => {
                expect_arity("point-on-line", parents, 2)?;
                let (line_kind, start, end) = expect_line("point-on-line", parents, 0)?;
                let t = expect_scalar("point-on-line", parents, 1)?;
                let point = match line_kind {
                    LineKind::Segment => lerp_point(start, end, t.clamp(0.0, 1.0)),
                    LineKind::Ray => lerp_point(start, end, t.max(0.0)),
                    LineKind::Line => lerp_point(start, end, t),
                };
                Ok(ObjectValue::point(point))
            }
            ObjectOp::PointOnCircle { invert_y } => {
                expect_arity("point-on-circle", parents, 3)?;
                let (center, radius_point) = expect_circle("point-on-circle", parents, 0)?;
                let unit_x = expect_scalar("point-on-circle", parents, 1)?;
                let unit_y = expect_scalar("point-on-circle", parents, 2)?;
                let radius = (radius_point.x - center.x).hypot(radius_point.y - center.y);
                Ok(ObjectValue::point(Point {
                    x: center.x + unit_x * radius,
                    y: center.y + if *invert_y { -unit_y } else { unit_y } * radius,
                }))
            }
            ObjectOp::PointOnCircleParameter { invert_y } => {
                expect_arity("point-on-circle-parameter", parents, 2)?;
                let (center, radius_point) =
                    expect_circle("point-on-circle-parameter", parents, 0)?;
                let parameter = expect_scalar("point-on-circle-parameter", parents, 1)?;
                let angle = parameter.rem_euclid(1.0) * std::f64::consts::TAU;
                let radius = (radius_point.x - center.x).hypot(radius_point.y - center.y);
                Ok(ObjectValue::point(Point {
                    x: center.x + angle.cos() * radius,
                    y: center.y + if *invert_y { -angle.sin() } else { angle.sin() } * radius,
                }))
            }
            ObjectOp::PointOnArc => {
                expect_arity("point-on-arc", parents, 2)?;
                let (start, mid, end, _, _, complement) = expect_arc("point-on-arc", parents, 0)?;
                let t = expect_scalar("point-on-arc", parents, 1)?;
                let point = if complement {
                    point_on_three_point_arc_complement(start, mid, end, t)
                } else {
                    point_on_three_point_arc(start, mid, end, t)
                }
                .unwrap_or(start);
                Ok(ObjectValue::point(point))
            }
            ObjectOp::PointOnPolyline => {
                expect_arity("point-on-polyline", parents, 2)?;
                let points = expect_points("point-on-polyline", parents, 0)?;
                if points.len() < 2 {
                    return Err(ObjectOpError::Degenerate {
                        op: "point-on-polyline",
                    });
                }
                let parameter = expect_scalar("point-on-polyline", parents, 1)?.clamp(0.0, 1.0);
                let scaled = parameter * (points.len() - 1) as f64;
                let segment_index = (scaled.floor() as usize).min(points.len() - 2);
                let t = scaled - segment_index as f64;
                Ok(ObjectValue::point(lerp_point(
                    points[segment_index],
                    points[segment_index + 1],
                    t,
                )))
            }
            ObjectOp::PointOnGeneratedTrace {
                program,
                driver,
                value_min,
                value_max,
            } => {
                expect_arity(
                    "point-on-generated-trace",
                    parents,
                    program.source_ids.len() + 1,
                )?;
                let graph = build_object_program("point-on-generated-trace", program)?;
                let parameter = expect_scalar(
                    "point-on-generated-trace",
                    parents,
                    program.source_ids.len(),
                )?
                .clamp(0.0, 1.0);
                let value = value_min + (value_max - value_min) * parameter;
                let overrides = trace_driver_overrides(driver, value, *value_min, *value_max);
                evaluate_object_program_point(
                    "point-on-generated-trace",
                    program,
                    &graph,
                    &parents[..program.source_ids.len()],
                    &overrides,
                )
                .map(ObjectValue::point)
            }
            ObjectOp::PointOnPolylineSegment { segment_index } => {
                expect_arity("point-on-polyline-segment", parents, 2)?;
                let points = expect_points("point-on-polyline-segment", parents, 0)?;
                if points.len() < 2 {
                    return Err(ObjectOpError::Degenerate {
                        op: "point-on-polyline-segment",
                    });
                }
                let index = (*segment_index).min(points.len() - 2);
                let t = expect_scalar("point-on-polyline-segment", parents, 1)?;
                Ok(ObjectValue::point(lerp_point(
                    points[index],
                    points[index + 1],
                    t.clamp(0.0, 1.0),
                )))
            }
            ObjectOp::PointOnPolygonBoundary => {
                if parents.len() < 3 {
                    return Err(ObjectOpError::WrongArity {
                        op: "point-on-polygon-boundary",
                        expected: 3,
                        actual: parents.len(),
                    });
                }
                let vertex_count = parents.len() - 1;
                let vertices = (0..vertex_count)
                    .map(|index| expect_point("point-on-polygon-boundary", parents, index))
                    .collect::<Result<Vec<_>, _>>()?;
                let parameter = expect_scalar("point-on-polygon-boundary", parents, vertex_count)?;
                point_on_polygon_boundary(&vertices, parameter)
                    .map(ObjectValue::point)
                    .ok_or(ObjectOpError::Degenerate {
                        op: "point-on-polygon-boundary",
                    })
            }
            ObjectOp::Midpoint => {
                expect_arity("midpoint", parents, 2)?;
                Ok(ObjectValue::point(lerp_point(
                    expect_point("midpoint", parents, 0)?,
                    expect_point("midpoint", parents, 1)?,
                    0.5,
                )))
            }
            ObjectOp::Circumcenter => {
                expect_arity("circumcenter", parents, 3)?;
                let geometry = three_point_arc_geometry(
                    expect_point("circumcenter", parents, 0)?,
                    expect_point("circumcenter", parents, 1)?,
                    expect_point("circumcenter", parents, 2)?,
                )
                .ok_or(ObjectOpError::Degenerate { op: "circumcenter" })?;
                Ok(ObjectValue::point(geometry.center))
            }
            ObjectOp::TranslatePoint => {
                expect_arity("translate-point", parents, 3)?;
                let source = expect_point("translate-point", parents, 0)?;
                let vector_start = expect_point("translate-point", parents, 1)?;
                let vector_end = expect_point("translate-point", parents, 2)?;
                Ok(ObjectValue::point(translate(
                    source,
                    vector_start,
                    vector_end,
                )))
            }
            ObjectOp::ReflectPoint => {
                expect_arity("reflect-point", parents, 3)?;
                let reflected = reflect_across_line(
                    expect_point("reflect-point", parents, 0)?,
                    expect_point("reflect-point", parents, 1)?,
                    expect_point("reflect-point", parents, 2)?,
                )
                .ok_or(ObjectOpError::Degenerate {
                    op: "reflect-point",
                })?;
                Ok(ObjectValue::point(reflected))
            }
            ObjectOp::ReflectPointAcrossLine => {
                expect_arity("reflect-point-across-line", parents, 2)?;
                let source = expect_point("reflect-point-across-line", parents, 0)?;
                let (_, line_start, line_end) =
                    expect_line("reflect-point-across-line", parents, 1)?;
                let reflected = reflect_across_line(source, line_start, line_end).ok_or(
                    ObjectOpError::Degenerate {
                        op: "reflect-point-across-line",
                    },
                )?;
                Ok(ObjectValue::point(reflected))
            }
            ObjectOp::RotatePoint { radians } => {
                expect_arity("rotate-point", parents, 2)?;
                Ok(ObjectValue::point(rotate_around(
                    expect_point("rotate-point", parents, 0)?,
                    expect_point("rotate-point", parents, 1)?,
                    *radians,
                )))
            }
            ObjectOp::RotatePointDegrees => {
                expect_arity("rotate-point-degrees", parents, 3)?;
                Ok(ObjectValue::point(rotate_around(
                    expect_point("rotate-point-degrees", parents, 0)?,
                    expect_point("rotate-point-degrees", parents, 1)?,
                    expect_scalar("rotate-point-degrees", parents, 2)?.to_radians(),
                )))
            }
            ObjectOp::ScalePoint { factor } => {
                expect_arity("scale-point", parents, 2)?;
                Ok(ObjectValue::point(scale_around(
                    expect_point("scale-point", parents, 0)?,
                    expect_point("scale-point", parents, 1)?,
                    *factor,
                )))
            }
            ObjectOp::ScalePointByScalar => {
                expect_arity("scale-point-by-scalar", parents, 3)?;
                Ok(ObjectValue::point(scale_around(
                    expect_point("scale-point-by-scalar", parents, 0)?,
                    expect_point("scale-point-by-scalar", parents, 1)?,
                    expect_scalar("scale-point-by-scalar", parents, 2)?,
                )))
            }
            ObjectOp::ScalePointByRatio {
                signed,
                clamp_to_unit,
            } => {
                expect_arity("scale-point-by-ratio", parents, 5)?;
                let point = scale_by_three_point_ratio(
                    expect_point("scale-point-by-ratio", parents, 0)?,
                    expect_point("scale-point-by-ratio", parents, 1)?,
                    expect_point("scale-point-by-ratio", parents, 2)?,
                    expect_point("scale-point-by-ratio", parents, 3)?,
                    expect_point("scale-point-by-ratio", parents, 4)?,
                    *signed,
                    *clamp_to_unit,
                )
                .ok_or(ObjectOpError::Degenerate {
                    op: "scale-point-by-ratio",
                })?;
                Ok(ObjectValue::point(point))
            }
            ObjectOp::Line { line_kind } => {
                expect_arity("line", parents, 2)?;
                Ok(ObjectValue::Line {
                    line_kind: *line_kind,
                    start: expect_point("line", parents, 0)?,
                    end: expect_point("line", parents, 1)?,
                })
            }
            ObjectOp::PerpendicularLine => {
                expect_arity("perpendicular-line", parents, 2)?;
                let through = expect_point("perpendicular-line", parents, 0)?;
                let (_, start, end) = expect_line("perpendicular-line", parents, 1)?;
                let dx = end.x - start.x;
                let dy = end.y - start.y;
                Ok(ObjectValue::Line {
                    line_kind: LineKind::Line,
                    start: through,
                    end: if dx.hypot(dy) <= 1e-9 {
                        through
                    } else {
                        Point {
                            x: through.x - dy,
                            y: through.y + dx,
                        }
                    },
                })
            }
            ObjectOp::ParallelLine => {
                expect_arity("parallel-line", parents, 2)?;
                let through = expect_point("parallel-line", parents, 0)?;
                let (_, start, end) = expect_line("parallel-line", parents, 1)?;
                let dx = end.x - start.x;
                let dy = end.y - start.y;
                Ok(ObjectValue::Line {
                    line_kind: LineKind::Line,
                    start: through,
                    end: if dx.hypot(dy) <= 1e-9 {
                        through
                    } else {
                        Point {
                            x: through.x + dx,
                            y: through.y + dy,
                        }
                    },
                })
            }
            ObjectOp::AngleBisectorRay => {
                expect_arity("angle-bisector-ray", parents, 3)?;
                let start = expect_point("angle-bisector-ray", parents, 0)?;
                let vertex = expect_point("angle-bisector-ray", parents, 1)?;
                let end = expect_point("angle-bisector-ray", parents, 2)?;
                let direction = angle_bisector_direction(start, vertex, end);
                Ok(ObjectValue::Line {
                    line_kind: LineKind::Ray,
                    start: vertex,
                    end: direction.map_or(vertex, |direction| Point {
                        x: vertex.x + direction.x,
                        y: vertex.y + direction.y,
                    }),
                })
            }
            ObjectOp::AngleMarker { marker_class } => {
                expect_arity("angle-marker", parents, 3)?;
                let points = angle_marker_points(
                    expect_point("angle-marker", parents, 0)?,
                    expect_point("angle-marker", parents, 1)?,
                    expect_point("angle-marker", parents, 2)?,
                    *marker_class,
                )
                .unwrap_or_default();
                Ok(ObjectValue::Points { points })
            }
            ObjectOp::SegmentMarker { t, marker_class } => {
                expect_arity("segment-marker", parents, 2)?;
                let points = segment_marker_points(
                    expect_point("segment-marker", parents, 0)?,
                    expect_point("segment-marker", parents, 1)?,
                    *t,
                    *marker_class,
                )
                .ok_or(ObjectOpError::Degenerate {
                    op: "segment-marker",
                })?;
                Ok(ObjectValue::Points { points })
            }
            ObjectOp::CoordinateTrace {
                x_expression,
                y_expression,
                parameter_names,
                trace_parameter_name,
                value_min,
                value_max,
                sample_count,
                mode,
            } => {
                expect_arity("coordinate-trace", parents, parameter_names.len() + 1)?;
                let source = expect_point("coordinate-trace", parents, 0)?;
                let parameters = parameter_names
                    .iter()
                    .enumerate()
                    .map(|(index, name)| {
                        expect_scalar("coordinate-trace", parents, index + 1)
                            .map(|value| (name.clone(), value))
                    })
                    .collect::<Result<BTreeMap<_, _>, _>>()?;
                let x_expression = FunctionExpr::Parsed(x_expression.to_function_ast());
                let y_expression = y_expression
                    .as_ref()
                    .map(|expression| FunctionExpr::Parsed(expression.to_function_ast()));
                let points = sample_coordinate_trace(
                    &x_expression,
                    y_expression.as_ref(),
                    &parameters,
                    y_expression.as_ref().map(|_| &parameters),
                    Some(trace_parameter_name),
                    y_expression.as_ref().map(|_| trace_parameter_name.as_str()),
                    source,
                    *value_min,
                    *value_max,
                    *sample_count,
                    false,
                    *mode,
                );
                if points.len() < 2 {
                    return Err(ObjectOpError::Degenerate {
                        op: "coordinate-trace",
                    });
                }
                Ok(ObjectValue::Points { points })
            }
            ObjectOp::CartesianParameterTrace {
                expression,
                parameter_names,
                trace_parameter_name,
                value_min,
                value_max,
                sample_count,
            } => {
                expect_arity("cartesian-parameter-trace", parents, parameter_names.len())?;
                let parameters = parameter_names
                    .iter()
                    .enumerate()
                    .map(|(index, name)| {
                        expect_scalar("cartesian-parameter-trace", parents, index)
                            .map(|value| (name.clone(), value))
                    })
                    .collect::<Result<BTreeMap<_, _>, _>>()?;
                let expression = FunctionExpr::Parsed(expression.to_function_ast());
                let last = sample_count.saturating_sub(1).max(1) as f64;
                let points = (0..*sample_count)
                    .filter_map(|index| {
                        let t = index as f64 / last;
                        let value = value_min + (value_max - value_min) * t;
                        let mut parameters = parameters.clone();
                        parameters.insert(trace_parameter_name.clone(), value);
                        Some(Point {
                            x: value,
                            y: evaluate_expr(&expression, 0.0, &parameters)?,
                        })
                    })
                    .collect::<Vec<_>>();
                if points.len() < 2 {
                    return Err(ObjectOpError::Degenerate {
                        op: "cartesian-parameter-trace",
                    });
                }
                Ok(ObjectValue::Points { points })
            }
            ObjectOp::ParametricCurve {
                x_expression,
                y_expression,
                parameter_names,
                value_min,
                value_max,
                sample_count,
            } => {
                expect_arity("parametric-curve", parents, parameter_names.len())?;
                let parameters = parameter_names
                    .iter()
                    .enumerate()
                    .map(|(index, name)| {
                        expect_scalar("parametric-curve", parents, index)
                            .map(|value| (name.clone(), value))
                    })
                    .collect::<Result<BTreeMap<_, _>, _>>()?;
                let points = sample_parametric_curve(
                    &FunctionExpr::Parsed(x_expression.to_function_ast()),
                    &FunctionExpr::Parsed(y_expression.to_function_ast()),
                    &parameters,
                    &parameters,
                    *value_min,
                    *value_max,
                    *sample_count,
                );
                if points.len() < 2 {
                    return Err(ObjectOpError::Degenerate {
                        op: "parametric-curve",
                    });
                }
                Ok(ObjectValue::Points { points })
            }
            ObjectOp::FunctionPlot {
                expression,
                parameter_names,
                value_min,
                value_max,
                sample_count,
                plot_mode,
            } => {
                expect_arity("function-plot", parents, parameter_names.len())?;
                let parameters = parameter_names
                    .iter()
                    .enumerate()
                    .map(|(index, name)| {
                        expect_scalar("function-plot", parents, index)
                            .map(|value| (name.clone(), value))
                    })
                    .collect::<Result<BTreeMap<_, _>, _>>()?;
                let points = crate::sample_expression(
                    &FunctionExpr::Parsed(expression.to_function_ast()),
                    &parameters,
                    *value_min,
                    *value_max,
                    *sample_count,
                    *plot_mode,
                )
                .into_iter()
                .flatten()
                .collect::<Vec<_>>();
                if points.len() < 2 {
                    return Err(ObjectOpError::Degenerate {
                        op: "function-plot",
                    });
                }
                Ok(ObjectValue::Points { points })
            }
            ObjectOp::CustomTransformPoint {
                distance_expression,
                angle_expression,
                distance_parameter_names,
                angle_parameter_names,
                distance_scale,
                angle_degrees_scale,
            } => {
                expect_arity("custom-transform-point", parents, 3)?;
                let origin = expect_point("custom-transform-point", parents, 0)?;
                let axis_end = expect_point("custom-transform-point", parents, 1)?;
                let value = expect_scalar("custom-transform-point", parents, 2)?;
                sample_custom_transform_trace(
                    &FunctionExpr::Parsed(distance_expression.to_function_ast()),
                    &FunctionExpr::Parsed(angle_expression.to_function_ast()),
                    &BTreeMap::new(),
                    &BTreeMap::new(),
                    distance_parameter_names,
                    angle_parameter_names,
                    origin,
                    axis_end,
                    value,
                    value,
                    value,
                    1,
                    *distance_scale,
                    *angle_degrees_scale,
                )
                .first()
                .copied()
                .map(ObjectValue::point)
                .ok_or(ObjectOpError::Degenerate {
                    op: "custom-transform-point",
                })
            }
            ObjectOp::CustomTransformTrace {
                distance_expression,
                angle_expression,
                distance_parameter_names,
                angle_parameter_names,
                value_min,
                value_max,
                sample_count,
                distance_scale,
                angle_degrees_scale,
            } => {
                expect_arity("custom-transform-trace", parents, 3)?;
                let points = sample_custom_transform_trace(
                    &FunctionExpr::Parsed(distance_expression.to_function_ast()),
                    &FunctionExpr::Parsed(angle_expression.to_function_ast()),
                    &BTreeMap::new(),
                    &BTreeMap::new(),
                    distance_parameter_names,
                    angle_parameter_names,
                    expect_point("custom-transform-trace", parents, 0)?,
                    expect_point("custom-transform-trace", parents, 1)?,
                    *value_min,
                    *value_max,
                    expect_scalar("custom-transform-trace", parents, 2)?,
                    *sample_count,
                    *distance_scale,
                    *angle_degrees_scale,
                );
                if points.is_empty() {
                    return Err(ObjectOpError::Degenerate {
                        op: "custom-transform-trace",
                    });
                }
                Ok(ObjectValue::Points { points })
            }
            ObjectOp::PointTrace {
                program,
                driver,
                value_min,
                value_max,
                sample_count,
            } => {
                expect_arity("point-trace", parents, program.source_ids.len())?;
                let graph = build_object_program("point-trace", program)?;
                let last = sample_count.saturating_sub(1).max(1) as f64;
                let mut points = Vec::with_capacity(*sample_count);
                for index in 0..*sample_count {
                    let value = value_min + (value_max - value_min) * index as f64 / last;
                    let overrides = trace_driver_overrides(driver, value, *value_min, *value_max);
                    points.push(evaluate_object_program_point(
                        "point-trace",
                        program,
                        &graph,
                        parents,
                        &overrides,
                    )?);
                }
                if points.len() < 2 {
                    return Err(ObjectOpError::Degenerate { op: "point-trace" });
                }
                Ok(ObjectValue::Points { points })
            }
            ObjectOp::RepeatPoint { sample_count } => {
                expect_arity("repeat-point", parents, 1)?;
                let point = expect_point("repeat-point", parents, 0)?;
                if *sample_count < 2 {
                    return Err(ObjectOpError::Degenerate { op: "repeat-point" });
                }
                Ok(ObjectValue::Points {
                    points: vec![point; *sample_count],
                })
            }
            ObjectOp::ZipPointTraces => {
                expect_arity("zip-point-traces", parents, 2)?;
                let starts = expect_points("zip-point-traces", parents, 0)?;
                let ends = expect_points("zip-point-traces", parents, 1)?;
                if starts.len() != ends.len() || starts.is_empty() {
                    return Err(ObjectOpError::Degenerate {
                        op: "zip-point-traces",
                    });
                }
                Ok(ObjectValue::Points {
                    points: starts
                        .iter()
                        .zip(ends)
                        .flat_map(|(start, end)| [*start, *end])
                        .collect(),
                })
            }
            ObjectOp::LinePolylineIntersection {
                variant,
                sample_hint,
            } => {
                expect_arity("line-polyline-intersection", parents, 2)?;
                let (line_kind, start, end) =
                    expect_line("line-polyline-intersection", parents, 0)?;
                let points = expect_points("line-polyline-intersection", parents, 1)?;
                let point = line_polyline_intersection(
                    start,
                    end,
                    line_kind,
                    points,
                    *sample_hint,
                    *variant,
                )
                .ok_or(ObjectOpError::Degenerate {
                    op: "line-polyline-intersection",
                })?;
                Ok(ObjectValue::point(point))
            }
            ObjectOp::CircleByPoints => {
                expect_arity("circle-by-points", parents, 2)?;
                Ok(ObjectValue::Circle {
                    center: expect_point("circle-by-points", parents, 0)?,
                    radius_point: expect_point("circle-by-points", parents, 1)?,
                })
            }
            ObjectOp::CircleBySegmentRadius => {
                expect_arity("circle-by-segment-radius", parents, 3)?;
                let center = expect_point("circle-by-segment-radius", parents, 0)?;
                let start = expect_point("circle-by-segment-radius", parents, 1)?;
                let end = expect_point("circle-by-segment-radius", parents, 2)?;
                let radius = (end.x - start.x).hypot(end.y - start.y);
                Ok(ObjectValue::Circle {
                    center,
                    radius_point: Point {
                        x: center.x + radius,
                        y: center.y,
                    },
                })
            }
            ObjectOp::CircleByRadius => {
                expect_arity("circle-by-radius", parents, 2)?;
                let center = expect_point("circle-by-radius", parents, 0)?;
                let radius = expect_scalar("circle-by-radius", parents, 1)?.abs();
                Ok(ObjectValue::Circle {
                    center,
                    radius_point: Point {
                        x: center.x + radius,
                        y: center.y,
                    },
                })
            }
            ObjectOp::ThreePointArc { complement } => {
                expect_arity("three-point-arc", parents, 3)?;
                let start = expect_point("three-point-arc", parents, 0)?;
                let mid = expect_point("three-point-arc", parents, 1)?;
                let end = expect_point("three-point-arc", parents, 2)?;
                Ok(ObjectValue::Arc {
                    start,
                    mid,
                    end,
                    center: None,
                    counterclockwise: false,
                    complement: *complement,
                })
            }
            ObjectOp::CenterArc { y_up } => {
                expect_arity("center-arc", parents, 3)?;
                let center = expect_point("center-arc", parents, 0)?;
                let start = expect_point("center-arc", parents, 1)?;
                let end = expect_point("center-arc", parents, 2)?;
                let [start, mid, end] = circle_arc_control_points(center, start, end, *y_up)
                    .unwrap_or([start, start, end]);
                Ok(ObjectValue::Arc {
                    start,
                    mid,
                    end,
                    center: Some(center),
                    counterclockwise: true,
                    complement: false,
                })
            }
            ObjectOp::CircleArc { y_up } => {
                expect_arity("circle-arc", parents, 3)?;
                let (center, _) = expect_circle("circle-arc", parents, 0)?;
                let start = expect_point("circle-arc", parents, 1)?;
                let end = expect_point("circle-arc", parents, 2)?;
                let [start, mid, end] = circle_arc_control_points(center, start, end, *y_up)
                    .unwrap_or([start, start, end]);
                Ok(ObjectValue::Arc {
                    start,
                    mid,
                    end,
                    center: Some(center),
                    counterclockwise: true,
                    complement: false,
                })
            }
            ObjectOp::ArcBoundaryPoints {
                center_arc,
                sector,
                reversed,
                complement,
                steps,
                y_up,
            } => {
                expect_arity("arc-boundary-points", parents, 3)?;
                let first = expect_point("arc-boundary-points", parents, 0)?;
                let second = expect_point("arc-boundary-points", parents, 1)?;
                let end = expect_point("arc-boundary-points", parents, 2)?;
                let (center, start, mid, sampled) = if *center_arc {
                    let sampled = sample_circle_arc(first, second, end, *steps, *y_up).ok_or(
                        ObjectOpError::Degenerate {
                            op: "arc-boundary-points",
                        },
                    )?;
                    (Some(first), second, None, sampled)
                } else {
                    let sampled = sample_three_point_arc(first, second, end, *steps, *complement)
                        .ok_or(ObjectOpError::Degenerate {
                        op: "arc-boundary-points",
                    })?;
                    (None, first, Some(second), sampled)
                };
                let mut points = if let Some(center) = center {
                    if *sector {
                        if *reversed {
                            vec![end, center, start]
                        } else {
                            vec![center, start]
                        }
                    } else if *reversed {
                        vec![end, start]
                    } else {
                        vec![start]
                    }
                } else if *sector && *reversed {
                    vec![
                        end,
                        mid.expect("three-point boundary has a midpoint"),
                        start,
                    ]
                } else if *reversed {
                    vec![end, start]
                } else {
                    vec![start]
                };
                points.extend_from_slice(&sampled[1..]);
                if !*reversed && center.is_some() && *sector {
                    points.push(center.expect("center boundary has a center"));
                } else if !*reversed && !*sector {
                    points.push(start);
                }
                Ok(ObjectValue::Points { points })
            }
            ObjectOp::ArcLength => {
                expect_arity("arc-length", parents, 1)?;
                let (start, mid, end, _, _, complement) = expect_arc("arc-length", parents, 0)?;
                let geometry = three_point_arc_geometry(start, mid, end)
                    .ok_or(ObjectOpError::Degenerate { op: "arc-length" })?;
                let contains_mid = geometry.ccw_mid <= geometry.ccw_span + 1e-9;
                let span = if contains_mid {
                    geometry.ccw_span
                } else {
                    std::f64::consts::TAU - geometry.ccw_span
                };
                let span = if complement {
                    std::f64::consts::TAU - span
                } else {
                    span
                };
                Ok(ObjectValue::Scalar {
                    value: geometry.radius * span,
                })
            }
            ObjectOp::CircularRadius => {
                expect_arity("circular-radius", parents, 1)?;
                let value = match parents[0] {
                    ObjectValue::Circle {
                        center,
                        radius_point,
                    } => (radius_point.x - center.x).hypot(radius_point.y - center.y),
                    ObjectValue::Arc { start, center, .. } => {
                        let center = center.ok_or(ObjectOpError::Degenerate {
                            op: "circular-radius",
                        })?;
                        (start.x - center.x).hypot(start.y - center.y)
                    }
                    _ => {
                        return Err(ObjectOpError::ExpectedShape {
                            op: "circular-radius",
                            parent: 0,
                        });
                    }
                };
                Ok(ObjectValue::Scalar { value })
            }
            ObjectOp::Polygon => Ok(ObjectValue::Points {
                points: parents
                    .iter()
                    .enumerate()
                    .map(|(index, _)| expect_point("polygon", parents, index))
                    .collect::<Result<_, _>>()?,
            }),
            ObjectOp::SimilarityPolygonIteration { inverse } => {
                expect_arity("similarity-polygon-iteration", parents, 6)?;
                let source = expect_points("similarity-polygon-iteration", parents, 0)?;
                let source_start = expect_point("similarity-polygon-iteration", parents, 1)?;
                let source_end = expect_point("similarity-polygon-iteration", parents, 2)?;
                let target_start = expect_point("similarity-polygon-iteration", parents, 3)?;
                let target_end = expect_point("similarity-polygon-iteration", parents, 4)?;
                let depth = expect_scalar("similarity-polygon-iteration", parents, 5)?;
                let depth = if depth.is_finite() {
                    (depth + 1e-9).floor().max(0.0) as usize
                } else {
                    0
                };
                let (basis_start, basis_end, image_start, image_end) = if *inverse {
                    (target_start, target_end, source_start, source_end)
                } else {
                    (source_start, source_end, target_start, target_end)
                };
                let basis_dx = basis_end.x - basis_start.x;
                let basis_dy = basis_end.y - basis_start.y;
                let basis_length_squared = basis_dx * basis_dx + basis_dy * basis_dy;
                if basis_length_squared <= 1e-9 {
                    return Err(ObjectOpError::Degenerate {
                        op: "similarity-polygon-iteration",
                    });
                }
                let image_dx = image_end.x - image_start.x;
                let image_dy = image_end.y - image_start.y;
                let transform = |point: Point| {
                    let relative_x = point.x - basis_start.x;
                    let relative_y = point.y - basis_start.y;
                    let alpha =
                        (relative_x * basis_dx + relative_y * basis_dy) / basis_length_squared;
                    let beta =
                        (relative_x * -basis_dy + relative_y * basis_dx) / basis_length_squared;
                    Point {
                        x: image_start.x + alpha * image_dx - beta * image_dy,
                        y: image_start.y + alpha * image_dy + beta * image_dx,
                    }
                };
                let mut current = source.to_vec();
                let mut polygons = Vec::with_capacity(depth);
                for _ in 0..depth {
                    current = current.into_iter().map(transform).collect();
                    polygons.push(current.clone());
                }
                Ok(ObjectValue::Polygons { polygons })
            }
            ObjectOp::PointOffsetIteration { dx, dy } => {
                expect_arity("point-offset-iteration", parents, 2)?;
                let seed = expect_point("point-offset-iteration", parents, 0)?;
                let depth = discrete_depth(expect_scalar("point-offset-iteration", parents, 1)?);
                Ok(ObjectValue::Points {
                    points: (1..=depth)
                        .map(|step| Point {
                            x: seed.x + dx * step as f64,
                            y: seed.y + dy * step as f64,
                        })
                        .collect(),
                })
            }
            ObjectOp::PointRotateIteration => {
                expect_arity("point-rotate-iteration", parents, 4)?;
                let seed = expect_point("point-rotate-iteration", parents, 0)?;
                let center = expect_point("point-rotate-iteration", parents, 1)?;
                let angle = expect_scalar("point-rotate-iteration", parents, 2)?.to_radians();
                let depth = discrete_depth(expect_scalar("point-rotate-iteration", parents, 3)?);
                Ok(ObjectValue::Points {
                    points: (1..=depth)
                        .map(|step| rotate_around(seed, center, angle * step as f64))
                        .collect(),
                })
            }
            ObjectOp::ParameterizedPointIteration {
                program,
                trace_source_id,
                trace_parameter_name,
                step_expression,
                step_parameter_names,
            } => {
                let program_parent_count = program.source_ids.len();
                let step_parent_count = step_parameter_names.len();
                expect_arity(
                    "parameterized-point-iteration",
                    parents,
                    program_parent_count + step_parent_count + 2,
                )?;
                let initial_index = program_parent_count + step_parent_count;
                let mut current =
                    expect_scalar("parameterized-point-iteration", parents, initial_index)?;
                let depth = discrete_depth(expect_scalar(
                    "parameterized-point-iteration",
                    parents,
                    initial_index + 1,
                )?);
                let mut step_parameters = step_parameter_names
                    .iter()
                    .enumerate()
                    .map(|(index, name)| {
                        expect_scalar(
                            "parameterized-point-iteration",
                            parents,
                            program_parent_count + index,
                        )
                        .map(|value| (name.clone(), value))
                    })
                    .collect::<Result<BTreeMap<_, _>, _>>()?;
                let graph = build_object_program("parameterized-point-iteration", program)?;
                let mut points = Vec::with_capacity(depth);
                for _ in 0..depth {
                    step_parameters.insert(trace_parameter_name.clone(), current);
                    let Some(next) = evaluate_expr(
                        &FunctionExpr::Parsed(step_expression.to_function_ast()),
                        0.0,
                        &step_parameters,
                    ) else {
                        break;
                    };
                    points.push(evaluate_object_program_point(
                        "parameterized-point-iteration",
                        program,
                        &graph,
                        &parents[..program_parent_count],
                        &[(trace_source_id.clone(), ObjectValue::Scalar { value: next })],
                    )?);
                    current = next;
                }
                Ok(ObjectValue::Points { points })
            }
            ObjectOp::LineTranslateIteration {
                dx,
                dy,
                secondary_dx,
                secondary_dy,
                bidirectional,
                vector_from_parents,
            } => {
                expect_arity(
                    "line-translate-iteration",
                    parents,
                    if *vector_from_parents { 5 } else { 3 },
                )?;
                let start = expect_point("line-translate-iteration", parents, 0)?;
                let end = expect_point("line-translate-iteration", parents, 1)?;
                let depth = discrete_depth(expect_scalar("line-translate-iteration", parents, 2)?);
                let primary = if *vector_from_parents {
                    let vector_start = expect_point("line-translate-iteration", parents, 3)?;
                    let vector_end = expect_point("line-translate-iteration", parents, 4)?;
                    Point {
                        x: vector_end.x - vector_start.x,
                        y: vector_end.y - vector_start.y,
                    }
                } else {
                    Point { x: *dx, y: *dy }
                };
                let secondary = secondary_dx.zip(*secondary_dy).map(|(x, y)| Point { x, y });
                Ok(ObjectValue::Points {
                    points: translation_iteration_deltas(
                        depth,
                        primary,
                        secondary,
                        *bidirectional,
                        false,
                    )
                    .into_iter()
                    .flat_map(|delta| {
                        [
                            Point {
                                x: start.x + delta.x,
                                y: start.y + delta.y,
                            },
                            Point {
                                x: end.x + delta.x,
                                y: end.y + delta.y,
                            },
                        ]
                    })
                    .collect(),
                })
            }
            ObjectOp::LineRotateIteration => {
                expect_arity("line-rotate-iteration", parents, 4)?;
                let (_, start, end) = expect_line("line-rotate-iteration", parents, 0)?;
                let center = expect_point("line-rotate-iteration", parents, 1)?;
                let angle = expect_scalar("line-rotate-iteration", parents, 2)?.to_radians();
                let depth = discrete_depth(expect_scalar("line-rotate-iteration", parents, 3)?);
                Ok(ObjectValue::Points {
                    points: (1..=depth)
                        .flat_map(|step| {
                            let radians = angle * step as f64;
                            [
                                rotate_around(start, center, radians),
                                rotate_around(end, center, radians),
                            ]
                        })
                        .collect(),
                })
            }
            ObjectOp::LineAffineIteration => {
                expect_arity("line-affine-iteration", parents, 9)?;
                let depth = discrete_depth(expect_scalar("line-affine-iteration", parents, 8)?);
                let points = affine_iteration_segment(
                    expect_point("line-affine-iteration", parents, 0)?,
                    expect_point("line-affine-iteration", parents, 1)?,
                    [
                        expect_point("line-affine-iteration", parents, 2)?,
                        expect_point("line-affine-iteration", parents, 3)?,
                        expect_point("line-affine-iteration", parents, 4)?,
                    ],
                    [
                        expect_point("line-affine-iteration", parents, 5)?,
                        expect_point("line-affine-iteration", parents, 6)?,
                        expect_point("line-affine-iteration", parents, 7)?,
                    ],
                    depth,
                )
                .ok_or(ObjectOpError::Degenerate {
                    op: "line-affine-iteration",
                })?;
                Ok(ObjectValue::Points { points })
            }
            ObjectOp::TranslatePolygonIteration {
                vertex_count,
                dx,
                dy,
                secondary_dx,
                secondary_dy,
                bidirectional,
                vector_from_parents,
            } => {
                let expected = vertex_count + 1 + if *vector_from_parents { 2 } else { 0 };
                expect_arity("translate-polygon-iteration", parents, expected)?;
                let vertices = (0..*vertex_count)
                    .map(|index| expect_point("translate-polygon-iteration", parents, index))
                    .collect::<Result<Vec<_>, _>>()?;
                let depth = discrete_depth(expect_scalar(
                    "translate-polygon-iteration",
                    parents,
                    *vertex_count,
                )?);
                let primary = if *vector_from_parents {
                    let start =
                        expect_point("translate-polygon-iteration", parents, *vertex_count + 1)?;
                    let end =
                        expect_point("translate-polygon-iteration", parents, *vertex_count + 2)?;
                    Point {
                        x: end.x - start.x,
                        y: end.y - start.y,
                    }
                } else {
                    Point { x: *dx, y: *dy }
                };
                let secondary = secondary_dx.zip(*secondary_dy).map(|(x, y)| Point { x, y });
                let polygons =
                    translation_iteration_deltas(depth, primary, secondary, *bidirectional, true)
                        .into_iter()
                        .map(|delta| {
                            vertices
                                .iter()
                                .map(|point| Point {
                                    x: point.x + delta.x,
                                    y: point.y + delta.y,
                                })
                                .collect()
                        })
                        .collect();
                Ok(ObjectValue::Polygons { polygons })
            }
            ObjectOp::CircleIteration { vertex_count } => {
                expect_arity("circle-iteration", parents, vertex_count + 4)?;
                let (source_center, source_radius_point) =
                    expect_circle("circle-iteration", parents, 0)?;
                let vertices = (0..*vertex_count)
                    .map(|index| expect_point("circle-iteration", parents, index + 1))
                    .collect::<Result<Vec<_>, _>>()?;
                let seed = expect_scalar("circle-iteration", parents, vertex_count + 1)?;
                let next = expect_scalar("circle-iteration", parents, vertex_count + 2)?;
                let depth = discrete_depth(expect_scalar(
                    "circle-iteration",
                    parents,
                    vertex_count + 3,
                )?);
                let step = (next - seed).rem_euclid(1.0);
                let radius_delta = Point {
                    x: source_radius_point.x - source_center.x,
                    y: source_radius_point.y - source_center.y,
                };
                let circles = (1..=depth)
                    .filter_map(|index| {
                        point_on_polygon_boundary(&vertices, seed + step * index as f64).map(
                            |center| ObjectCircle {
                                center,
                                radius_point: Point {
                                    x: center.x + radius_delta.x,
                                    y: center.y + radius_delta.y,
                                },
                            },
                        )
                    })
                    .collect::<Vec<_>>();
                if depth > 0 && circles.is_empty() {
                    return Err(ObjectOpError::Degenerate {
                        op: "circle-iteration",
                    });
                }
                Ok(ObjectValue::Circles { circles })
            }
            ObjectOp::LineIntersection => {
                expect_arity("line-intersection", parents, 2)?;
                let (left_kind, left_start, left_end) =
                    expect_line("line-intersection", parents, 0)?;
                let (right_kind, right_start, right_end) =
                    expect_line("line-intersection", parents, 1)?;
                let point = line_line_intersection(
                    left_start,
                    left_end,
                    left_kind,
                    right_start,
                    right_end,
                    right_kind,
                )
                .ok_or(ObjectOpError::Degenerate {
                    op: "line-intersection",
                })?;
                Ok(ObjectValue::point(point))
            }
            ObjectOp::LineCircleIntersection { variant } => {
                expect_arity("line-circle-intersection", parents, 2)?;
                let (line_kind, start, end) = expect_line("line-circle-intersection", parents, 0)?;
                let (center, radius) =
                    circular_center_radius("line-circle-intersection", parents, 1)?;
                let point = if matches!(parents[1], ObjectValue::Arc { .. }) {
                    line_circle_intersections(start, end, line_kind, center, radius)
                        .into_iter()
                        .filter(|point| point_lies_on_circular_value(*point, parents[1]))
                        .nth(*variant)
                } else {
                    line_circle_intersection_candidate(
                        start, end, line_kind, center, radius, *variant,
                    )
                }
                .ok_or(ObjectOpError::Degenerate {
                    op: "line-circle-intersection",
                })?;
                Ok(ObjectValue::point(point))
            }
            ObjectOp::CircleCircleIntersection { variant } => {
                expect_arity("circle-circle-intersection", parents, 2)?;
                let (left_center, left_radius) =
                    circular_center_radius("circle-circle-intersection", parents, 0)?;
                let (right_center, right_radius) =
                    circular_center_radius("circle-circle-intersection", parents, 1)?;
                let candidates = circle_circle_intersections(
                    left_center,
                    left_radius,
                    right_center,
                    right_radius,
                );
                let point = candidates
                    .into_iter()
                    .filter(|point| point_lies_on_circular_value(*point, parents[0]))
                    .filter(|point| point_lies_on_circular_value(*point, parents[1]))
                    .nth(*variant)
                    .ok_or(ObjectOpError::Degenerate {
                        op: "circle-circle-intersection",
                    })?;
                Ok(ObjectValue::point(point))
            }
            ObjectOp::PointCircleTangent { variant } => {
                expect_arity("point-circle-tangent", parents, 2)?;
                let point = expect_point("point-circle-tangent", parents, 0)?;
                let (center, radius_point) = expect_circle("point-circle-tangent", parents, 1)?;
                let candidates = point_circle_tangents(
                    point,
                    center,
                    (radius_point.x - center.x).hypot(radius_point.y - center.y),
                );
                let tangent = choose_point_candidate(&candidates, None, *variant).ok_or(
                    ObjectOpError::Degenerate {
                        op: "point-circle-tangent",
                    },
                )?;
                Ok(ObjectValue::point(tangent))
            }
            ObjectOp::PointDistance { value_scale } => {
                expect_arity("point-distance", parents, 2)?;
                let value = point_distance(
                    expect_point("point-distance", parents, 0)?,
                    expect_point("point-distance", parents, 1)?,
                    *value_scale,
                )
                .ok_or(ObjectOpError::Degenerate {
                    op: "point-distance",
                })?;
                Ok(ObjectValue::Scalar { value })
            }
            ObjectOp::PointDistanceRatio { clamp_to_unit } => {
                expect_arity("point-distance-ratio", parents, 3)?;
                let value = point_distance_ratio(
                    expect_point("point-distance-ratio", parents, 0)?,
                    expect_point("point-distance-ratio", parents, 1)?,
                    expect_point("point-distance-ratio", parents, 2)?,
                    *clamp_to_unit,
                )
                .ok_or(ObjectOpError::Degenerate {
                    op: "point-distance-ratio",
                })?;
                Ok(ObjectValue::Scalar { value })
            }
            ObjectOp::PointAngleDegrees => {
                expect_arity("point-angle-degrees", parents, 3)?;
                let value = point_angle_degrees(
                    expect_point("point-angle-degrees", parents, 0)?,
                    expect_point("point-angle-degrees", parents, 1)?,
                    expect_point("point-angle-degrees", parents, 2)?,
                )
                .ok_or(ObjectOpError::Degenerate {
                    op: "point-angle-degrees",
                })?;
                Ok(ObjectValue::Scalar { value })
            }
            ObjectOp::PointCoordinate { vertical } => {
                expect_arity("point-coordinate", parents, 1)?;
                let point = expect_point("point-coordinate", parents, 0)?;
                Ok(ObjectValue::Scalar {
                    value: if *vertical { point.y } else { point.x },
                })
            }
            ObjectOp::MeasuredRotationDegrees => {
                expect_arity("measured-rotation-degrees", parents, 3)?;
                let value = measured_rotation_radians(
                    expect_point("measured-rotation-degrees", parents, 0)?,
                    expect_point("measured-rotation-degrees", parents, 1)?,
                    expect_point("measured-rotation-degrees", parents, 2)?,
                )
                .ok_or(ObjectOpError::Degenerate {
                    op: "measured-rotation-degrees",
                })?
                .to_degrees();
                Ok(ObjectValue::Scalar { value })
            }
            ObjectOp::PointLineParameter { line_kind } => {
                expect_arity("point-line-parameter", parents, 3)?;
                let projection = project_to_line_like(
                    expect_point("point-line-parameter", parents, 0)?,
                    expect_point("point-line-parameter", parents, 1)?,
                    expect_point("point-line-parameter", parents, 2)?,
                    *line_kind,
                )
                .ok_or(ObjectOpError::Degenerate {
                    op: "point-line-parameter",
                })?;
                Ok(ObjectValue::Scalar {
                    value: projection.t,
                })
            }
            ObjectOp::CircleParameter { invert_y } => {
                expect_arity("circle-parameter", parents, 2)?;
                let point = expect_point("circle-parameter", parents, 0)?;
                let (center, _) = expect_circle("circle-parameter", parents, 1)?;
                let dx = point.x - center.x;
                let dy = point.y - center.y;
                if dx.hypot(dy) <= 1e-9 {
                    return Err(ObjectOpError::Degenerate {
                        op: "circle-parameter",
                    });
                }
                let angle = (if *invert_y { -dy } else { dy }).atan2(dx);
                Ok(ObjectValue::Scalar {
                    value: angle.rem_euclid(std::f64::consts::TAU) / std::f64::consts::TAU,
                })
            }
            ObjectOp::PolygonBoundaryParameter { edge_index } => {
                if parents.len() < 3 {
                    return Err(ObjectOpError::WrongArity {
                        op: "polygon-boundary-parameter",
                        expected: 3,
                        actual: parents.len(),
                    });
                }
                let vertex_count = parents.len() - 1;
                let vertices = (0..vertex_count)
                    .map(|index| expect_point("polygon-boundary-parameter", parents, index))
                    .collect::<Result<Vec<_>, _>>()?;
                let local_t = expect_scalar("polygon-boundary-parameter", parents, vertex_count)?
                    .clamp(0.0, 1.0);
                let lengths = vertices
                    .iter()
                    .zip(vertices.iter().cycle().skip(1))
                    .take(vertex_count)
                    .map(|(start, end)| (end.x - start.x).hypot(end.y - start.y))
                    .collect::<Vec<_>>();
                let perimeter = lengths.iter().sum::<f64>();
                if perimeter <= 1e-9 {
                    return Err(ObjectOpError::Degenerate {
                        op: "polygon-boundary-parameter",
                    });
                }
                let edge_index = edge_index % vertex_count;
                Ok(ObjectValue::Scalar {
                    value: (lengths[..edge_index].iter().sum::<f64>()
                        + local_t * lengths[edge_index])
                        / perimeter,
                })
            }
            ObjectOp::PolygonBoundaryParameterFromPoint => {
                if parents.len() < 3 {
                    return Err(ObjectOpError::WrongArity {
                        op: "polygon-boundary-parameter-from-point",
                        expected: 3,
                        actual: parents.len(),
                    });
                }
                let point_index = parents.len() - 1;
                let vertices = (0..point_index)
                    .map(|index| {
                        expect_point("polygon-boundary-parameter-from-point", parents, index)
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                let point = expect_point(
                    "polygon-boundary-parameter-from-point",
                    parents,
                    point_index,
                )?;
                polygon_boundary_parameter_from_point(&vertices, point)
                    .map(|value| ObjectValue::Scalar { value })
                    .ok_or(ObjectOpError::Degenerate {
                        op: "polygon-boundary-parameter-from-point",
                    })
            }
            ObjectOp::PolygonArea { value_scale } => {
                let points = parents
                    .iter()
                    .enumerate()
                    .map(|(index, _)| expect_point("polygon-area", parents, index))
                    .collect::<Result<Vec<_>, _>>()?;
                let value = polygon_area(&points, *value_scale)
                    .ok_or(ObjectOpError::Degenerate { op: "polygon-area" })?;
                Ok(ObjectValue::Scalar { value })
            }
            ObjectOp::EvaluateExpression {
                expression,
                parameter_names,
                x,
            } => {
                expect_arity("evaluate-expression", parents, parameter_names.len())?;
                let parameters = parameter_names
                    .iter()
                    .enumerate()
                    .map(|(index, name)| {
                        expect_scalar("evaluate-expression", parents, index)
                            .map(|value| (name.clone(), value))
                    })
                    .collect::<Result<BTreeMap<_, _>, _>>()?;
                let value = evaluate_expr(
                    &FunctionExpr::Parsed(expression.to_function_ast()),
                    *x,
                    &parameters,
                )
                .ok_or(ObjectOpError::Degenerate {
                    op: "evaluate-expression",
                })?;
                Ok(ObjectValue::Scalar { value })
            }
            ObjectOp::ScaleScalar { factor } => {
                expect_arity("scale-scalar", parents, 1)?;
                Ok(ObjectValue::Scalar {
                    value: expect_scalar("scale-scalar", parents, 0)? * factor,
                })
            }
            ObjectOp::AbsoluteScalar => {
                expect_arity("absolute-scalar", parents, 1)?;
                Ok(ObjectValue::Scalar {
                    value: expect_scalar("absolute-scalar", parents, 0)?.abs(),
                })
            }
            ObjectOp::SpectrumColor {
                base_value,
                period,
                base_color,
            } => {
                expect_arity("spectrum-color", parents, 1)?;
                if !period.is_finite() || *period <= 1e-9 {
                    return Err(ObjectOpError::Degenerate {
                        op: "spectrum-color",
                    });
                }
                let value = expect_scalar("spectrum-color", parents, 0)?;
                let (hue, saturation, brightness) = rgba_to_hsb(*base_color);
                Ok(ObjectValue::Color {
                    color: hsb_to_rgba(
                        hue + (value - base_value) / period,
                        saturation,
                        brightness,
                        base_color[3],
                    ),
                })
            }
            ObjectOp::RgbColor { alpha } => {
                expect_arity("rgb-color", parents, 3)?;
                let component = |index| {
                    Ok(
                        (expect_scalar("rgb-color", parents, index)?.clamp(0.0, 1.0) * 255.0)
                            .round() as u8,
                    )
                };
                Ok(ObjectValue::Color {
                    color: [component(0)?, component(1)?, component(2)?, *alpha],
                })
            }
            ObjectOp::HsbColor { alpha } => {
                expect_arity("hsb-color", parents, 3)?;
                Ok(ObjectValue::Color {
                    color: hsb_to_rgba(
                        expect_scalar("hsb-color", parents, 0)?.clamp(0.0, 1.0),
                        expect_scalar("hsb-color", parents, 1)?.clamp(0.0, 1.0),
                        expect_scalar("hsb-color", parents, 2)?.clamp(0.0, 1.0),
                        *alpha,
                    ),
                })
            }
            ObjectOp::TranslateShapeDelta { dx, dy } => {
                expect_arity("translate-shape-delta", parents, 1)?;
                transform_shape("translate-shape-delta", parents[0], |point| {
                    Some(Point {
                        x: point.x + dx,
                        y: point.y + dy,
                    })
                })
            }
            ObjectOp::TranslateShape => {
                expect_arity("translate-shape", parents, 3)?;
                let vector_start = expect_point("translate-shape", parents, 1)?;
                let vector_end = expect_point("translate-shape", parents, 2)?;
                transform_shape("translate-shape", parents[0], |point| {
                    Some(translate(point, vector_start, vector_end))
                })
            }
            ObjectOp::ReflectShape => {
                expect_arity("reflect-shape", parents, 3)?;
                let line_start = expect_point("reflect-shape", parents, 1)?;
                let line_end = expect_point("reflect-shape", parents, 2)?;
                transform_shape("reflect-shape", parents[0], |point| {
                    reflect_across_line(point, line_start, line_end)
                })
            }
            ObjectOp::ReflectShapeAcrossLine => {
                expect_arity("reflect-shape-across-line", parents, 2)?;
                let (_, line_start, line_end) =
                    expect_line("reflect-shape-across-line", parents, 1)?;
                transform_shape("reflect-shape-across-line", parents[0], |point| {
                    reflect_across_line(point, line_start, line_end)
                })
            }
            ObjectOp::RotateShape { radians } => {
                expect_arity("rotate-shape", parents, 2)?;
                let center = expect_point("rotate-shape", parents, 1)?;
                transform_shape("rotate-shape", parents[0], |point| {
                    Some(rotate_around(point, center, *radians))
                })
            }
            ObjectOp::RotateShapeDegrees => {
                expect_arity("rotate-shape-degrees", parents, 3)?;
                let center = expect_point("rotate-shape-degrees", parents, 1)?;
                let radians = expect_scalar("rotate-shape-degrees", parents, 2)?.to_radians();
                transform_shape("rotate-shape-degrees", parents[0], |point| {
                    Some(rotate_around(point, center, radians))
                })
            }
            ObjectOp::ScaleShape { factor } => {
                expect_arity("scale-shape", parents, 2)?;
                let center = expect_point("scale-shape", parents, 1)?;
                transform_shape("scale-shape", parents[0], |point| {
                    Some(scale_around(point, center, *factor))
                })
            }
        }
    }
}

fn rgba_to_hsb(color: [u8; 4]) -> (f64, f64, f64) {
    let red = f64::from(color[0]) / 255.0;
    let green = f64::from(color[1]) / 255.0;
    let blue = f64::from(color[2]) / 255.0;
    let max = red.max(green).max(blue);
    let min = red.min(green).min(blue);
    let delta = max - min;
    let hue = if delta <= 1e-9 {
        0.0
    } else if max == red {
        ((green - blue) / delta / 6.0).rem_euclid(1.0)
    } else if max == green {
        ((2.0 + (blue - red) / delta) / 6.0).rem_euclid(1.0)
    } else {
        ((4.0 + (red - green) / delta) / 6.0).rem_euclid(1.0)
    };
    (hue, if max <= 1e-9 { 0.0 } else { delta / max }, max)
}

fn hsb_to_rgba(hue: f64, saturation: f64, brightness: f64, alpha: u8) -> [u8; 4] {
    let hue = hue.rem_euclid(1.0) * 6.0;
    let saturation = saturation.clamp(0.0, 1.0);
    let brightness = brightness.clamp(0.0, 1.0);
    let sector = hue.floor() as usize;
    let fraction = hue - hue.floor();
    let p = brightness * (1.0 - saturation);
    let q = brightness * (1.0 - saturation * fraction);
    let t = brightness * (1.0 - saturation * (1.0 - fraction));
    let (red, green, blue) = match sector {
        0 => (brightness, t, p),
        1 => (q, brightness, p),
        2 => (p, brightness, t),
        3 => (p, q, brightness),
        4 => (t, p, brightness),
        _ => (brightness, p, q),
    };
    [
        (red * 255.0).round() as u8,
        (green * 255.0).round() as u8,
        (blue * 255.0).round() as u8,
        alpha,
    ]
}

fn build_object_program(
    op: &'static str,
    program: &ObjectProgram,
) -> Result<ObjectGraph<ObjectOp>, ObjectOpError> {
    ObjectGraph::build(program.nodes.clone()).map_err(|error| ObjectOpError::InvalidProgram {
        op,
        message: error.to_string(),
    })
}

fn trace_driver_overrides(
    driver: &TraceDriver,
    value: f64,
    value_min: f64,
    value_max: f64,
) -> Vec<(String, ObjectValue)> {
    match driver {
        TraceDriver::Scalar {
            source_id,
            normalized,
        } => {
            let sample = if *normalized {
                if (value_max - value_min).abs() <= 1e-9 {
                    0.0
                } else {
                    ((value - value_min) / (value_max - value_min)).clamp(0.0, 1.0)
                }
            } else {
                value
            };
            vec![(source_id.clone(), ObjectValue::Scalar { value: sample })]
        }
        TraceDriver::Circle {
            unit_x_source_id,
            unit_y_source_id,
        } => vec![
            (
                unit_x_source_id.clone(),
                ObjectValue::Scalar { value: value.cos() },
            ),
            (
                unit_y_source_id.clone(),
                ObjectValue::Scalar {
                    value: -value.sin(),
                },
            ),
        ],
    }
}

fn evaluate_object_program_point(
    op: &'static str,
    program: &ObjectProgram,
    graph: &ObjectGraph<ObjectOp>,
    source_parents: &[&ObjectValue],
    overrides: &[(String, ObjectValue)],
) -> Result<Point, ObjectOpError> {
    if source_parents.len() != program.source_ids.len() {
        return Err(ObjectOpError::InvalidProgram {
            op,
            message: format!(
                "program expects {} source values, got {}",
                program.source_ids.len(),
                source_parents.len()
            ),
        });
    }
    let mut values = ObjectValues::new(graph);
    for (source_id, parent) in program.source_ids.iter().zip(source_parents) {
        values
            .set_source::<_, ObjectOpError>(graph, source_id, (*parent).clone())
            .map_err(|error| ObjectOpError::InvalidProgram {
                op,
                message: error.to_string(),
            })?;
    }
    for (source_id, value) in overrides {
        values
            .set_source::<_, ObjectOpError>(graph, source_id, value.clone())
            .map_err(|error| ObjectOpError::InvalidProgram {
                op,
                message: error.to_string(),
            })?;
    }
    values
        .evaluate_all(graph, &mut BuiltinOperationTable)
        .map_err(|error| ObjectOpError::InvalidProgram {
            op,
            message: error.to_string(),
        })?;
    values
        .get(graph, &program.target_id)
        .and_then(ObjectValue::as_point)
        .ok_or_else(|| ObjectOpError::InvalidProgram {
            op,
            message: format!("target {} is not a point", program.target_id),
        })
}

fn expect_arity(
    op: &'static str,
    parents: &[&ObjectValue],
    expected: usize,
) -> Result<(), ObjectOpError> {
    (parents.len() == expected)
        .then_some(())
        .ok_or(ObjectOpError::WrongArity {
            op,
            expected,
            actual: parents.len(),
        })
}

fn discrete_depth(value: f64) -> usize {
    if value.is_finite() {
        (value + 1e-9).floor().max(0.0) as usize
    } else {
        0
    }
}

fn point_on_polygon_boundary(vertices: &[Point], parameter: f64) -> Option<Point> {
    if vertices.len() < 2 {
        return None;
    }
    let lengths = vertices
        .iter()
        .zip(vertices.iter().cycle().skip(1))
        .take(vertices.len())
        .map(|(start, end)| (end.x - start.x).hypot(end.y - start.y))
        .collect::<Vec<_>>();
    let perimeter = lengths.iter().sum::<f64>();
    if perimeter <= 1e-9 {
        return None;
    }
    let target = parameter.rem_euclid(1.0) * perimeter;
    let mut traveled = 0.0;
    for (edge_index, length) in lengths.iter().copied().enumerate() {
        if traveled + length >= target || edge_index + 1 == vertices.len() {
            let local = if length <= 1e-9 {
                0.0
            } else {
                ((target - traveled) / length).clamp(0.0, 1.0)
            };
            return Some(lerp_point(
                vertices[edge_index],
                vertices[(edge_index + 1) % vertices.len()],
                local,
            ));
        }
        traveled += length;
    }
    None
}

fn polygon_boundary_parameter_from_point(vertices: &[Point], point: Point) -> Option<f64> {
    if vertices.len() < 2 {
        return None;
    }
    let lengths = vertices
        .iter()
        .zip(vertices.iter().cycle().skip(1))
        .take(vertices.len())
        .map(|(start, end)| (end.x - start.x).hypot(end.y - start.y))
        .collect::<Vec<_>>();
    let perimeter = lengths.iter().sum::<f64>();
    if perimeter <= 1e-9 {
        return None;
    }
    let mut traveled = 0.0;
    let mut best = None;
    for edge_index in 0..vertices.len() {
        let start = vertices[edge_index];
        let end = vertices[(edge_index + 1) % vertices.len()];
        let dx = end.x - start.x;
        let dy = end.y - start.y;
        let length_squared = dx * dx + dy * dy;
        let local = if length_squared <= 1e-18 {
            0.0
        } else {
            ((point.x - start.x) * dx + (point.y - start.y) * dy) / length_squared
        }
        .clamp(0.0, 1.0);
        let projected = lerp_point(start, end, local);
        let distance_squared = (point.x - projected.x).powi(2) + (point.y - projected.y).powi(2);
        let parameter = (traveled + local * lengths[edge_index]) / perimeter;
        if best
            .as_ref()
            .is_none_or(|(best_distance, _)| distance_squared < *best_distance)
        {
            best = Some((distance_squared, parameter));
        }
        traveled += lengths[edge_index];
    }
    best.map(|(_, parameter)| parameter.rem_euclid(1.0))
}

fn expect_point(
    op: &'static str,
    parents: &[&ObjectValue],
    index: usize,
) -> Result<Point, ObjectOpError> {
    parents
        .get(index)
        .and_then(|value| value.as_point())
        .ok_or(ObjectOpError::ExpectedPoint { op, parent: index })
}

fn expect_scalar(
    op: &'static str,
    parents: &[&ObjectValue],
    index: usize,
) -> Result<f64, ObjectOpError> {
    parents
        .get(index)
        .and_then(|value| value.as_scalar())
        .ok_or(ObjectOpError::ExpectedScalar { op, parent: index })
}

fn expect_line(
    op: &'static str,
    parents: &[&ObjectValue],
    index: usize,
) -> Result<(LineKind, Point, Point), ObjectOpError> {
    match parents.get(index).copied() {
        Some(ObjectValue::Line {
            line_kind,
            start,
            end,
        }) => Ok((*line_kind, *start, *end)),
        _ => Err(ObjectOpError::ExpectedLine { op, parent: index }),
    }
}

fn expect_circle(
    op: &'static str,
    parents: &[&ObjectValue],
    index: usize,
) -> Result<(Point, Point), ObjectOpError> {
    match parents.get(index).copied() {
        Some(ObjectValue::Circle {
            center,
            radius_point,
        }) => Ok((*center, *radius_point)),
        _ => Err(ObjectOpError::ExpectedCircle { op, parent: index }),
    }
}

fn circular_center_radius(
    op: &'static str,
    parents: &[&ObjectValue],
    index: usize,
) -> Result<(Point, f64), ObjectOpError> {
    match parents.get(index).copied() {
        Some(ObjectValue::Circle {
            center,
            radius_point,
        }) => Ok((
            *center,
            (radius_point.x - center.x).hypot(radius_point.y - center.y),
        )),
        Some(ObjectValue::Arc {
            start,
            mid,
            end,
            center,
            ..
        }) => {
            let center = center
                .or_else(|| three_point_arc_geometry(*start, *mid, *end).map(|arc| arc.center))
                .ok_or(ObjectOpError::Degenerate { op })?;
            Ok((center, (start.x - center.x).hypot(start.y - center.y)))
        }
        _ => Err(ObjectOpError::ExpectedCircle { op, parent: index }),
    }
}

fn point_lies_on_circular_value(point: Point, value: &ObjectValue) -> bool {
    let ObjectValue::Arc {
        start,
        mid,
        end,
        complement,
        ..
    } = value
    else {
        return matches!(value, ObjectValue::Circle { .. });
    };
    let Some(geometry) = three_point_arc_geometry(*start, *mid, *end) else {
        return (point.x - start.x).hypot(point.y - start.y) <= 1e-6
            || (point.x - end.x).hypot(point.y - end.y) <= 1e-6;
    };
    let radial = (point.x - geometry.center.x).hypot(point.y - geometry.center.y);
    if (radial - geometry.radius).abs() > 1e-6 {
        return false;
    }
    let tau = std::f64::consts::TAU;
    let delta = |from: f64, to: f64| (to - from).rem_euclid(tau);
    let point_angle = (point.y - geometry.center.y).atan2(point.x - geometry.center.x);
    let mid_angle = (mid.y - geometry.center.y).atan2(mid.x - geometry.center.x);
    let ccw_span = delta(geometry.start_angle, geometry.end_angle);
    let ccw_mid = delta(geometry.start_angle, mid_angle);
    let on_arc = if ccw_mid <= ccw_span + 1e-9 {
        delta(geometry.start_angle, point_angle) <= ccw_span + 1e-9
    } else {
        delta(point_angle, geometry.start_angle)
            <= delta(geometry.end_angle, geometry.start_angle) + 1e-9
    };
    if *complement { !on_arc } else { on_arc }
}

fn expect_points<'a>(
    op: &'static str,
    parents: &'a [&ObjectValue],
    index: usize,
) -> Result<&'a [Point], ObjectOpError> {
    match parents.get(index).copied() {
        Some(ObjectValue::Points { points }) => Ok(points),
        _ => Err(ObjectOpError::ExpectedShape { op, parent: index }),
    }
}

fn expect_arc(
    op: &'static str,
    parents: &[&ObjectValue],
    index: usize,
) -> Result<(Point, Point, Point, Option<Point>, bool, bool), ObjectOpError> {
    match parents.get(index).copied() {
        Some(ObjectValue::Arc {
            start,
            mid,
            end,
            center,
            counterclockwise,
            complement,
        }) => Ok((*start, *mid, *end, *center, *counterclockwise, *complement)),
        _ => Err(ObjectOpError::ExpectedArc { op, parent: index }),
    }
}

fn translate(point: Point, vector_start: Point, vector_end: Point) -> Point {
    Point {
        x: point.x + vector_end.x - vector_start.x,
        y: point.y + vector_end.y - vector_start.y,
    }
}

fn transform_shape(
    op: &'static str,
    value: &ObjectValue,
    transform: impl Fn(Point) -> Option<Point>,
) -> Result<ObjectValue, ObjectOpError> {
    let map_point = |point| transform(point).ok_or(ObjectOpError::Degenerate { op });
    match value {
        ObjectValue::Point { .. } => Ok(ObjectValue::point(map_point(
            value
                .as_point()
                .ok_or(ObjectOpError::ExpectedShape { op, parent: 0 })?,
        )?)),
        ObjectValue::Line {
            line_kind,
            start,
            end,
        } => Ok(ObjectValue::Line {
            line_kind: *line_kind,
            start: map_point(*start)?,
            end: map_point(*end)?,
        }),
        ObjectValue::Circle {
            center,
            radius_point,
        } => Ok(ObjectValue::Circle {
            center: map_point(*center)?,
            radius_point: map_point(*radius_point)?,
        }),
        ObjectValue::Arc {
            start,
            mid,
            end,
            center,
            counterclockwise,
            complement,
        } => Ok(ObjectValue::Arc {
            start: map_point(*start)?,
            mid: map_point(*mid)?,
            end: map_point(*end)?,
            center: center.map(map_point).transpose()?,
            counterclockwise: *counterclockwise,
            complement: *complement,
        }),
        ObjectValue::Points { points } => Ok(ObjectValue::Points {
            points: points
                .iter()
                .copied()
                .map(map_point)
                .collect::<Result<_, _>>()?,
        }),
        ObjectValue::Circles { circles } => Ok(ObjectValue::Circles {
            circles: circles
                .iter()
                .map(|circle| {
                    Ok(ObjectCircle {
                        center: map_point(circle.center)?,
                        radius_point: map_point(circle.radius_point)?,
                    })
                })
                .collect::<Result<_, ObjectOpError>>()?,
        }),
        ObjectValue::Scalar { .. }
        | ObjectValue::Polygons { .. }
        | ObjectValue::Color { .. }
        | ObjectValue::Text { .. } => Err(ObjectOpError::ExpectedShape { op, parent: 0 }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn directed_angle_anchor_is_derived_from_all_four_point_parents() {
        let first_start = ObjectValue::point(Point { x: 1.0, y: 2.0 });
        let first_end = ObjectValue::point(Point { x: 5.0, y: 2.0 });
        let second_start = ObjectValue::point(Point { x: -3.0, y: -4.0 });
        let second_end = ObjectValue::point(Point { x: -3.0, y: 1.0 });
        let value = BuiltinOperationTable
            .evaluate(
                "anchor",
                &ObjectOp::DirectedAngleAnchor {
                    distance: 2.0,
                    parameter: 0.5,
                },
                &[&first_start, &first_end, &second_start, &second_end],
            )
            .unwrap();
        let point = value.as_point().unwrap();
        assert!((point.x - (1.0 + 2.0_f64.sqrt())).abs() < 1e-9);
        assert!((point.y - (2.0 + 2.0_f64.sqrt())).abs() < 1e-9);
    }

    #[test]
    fn line_circle_intersection_accepts_an_arc_as_its_circular_parent() {
        let line = ObjectValue::Line {
            line_kind: LineKind::Line,
            start: Point { x: -3.0, y: 0.0 },
            end: Point { x: 3.0, y: 0.0 },
        };
        let arc = ObjectValue::Arc {
            start: Point { x: 2.0, y: 0.0 },
            mid: Point { x: 0.0, y: 2.0 },
            end: Point { x: -2.0, y: 0.0 },
            center: Some(Point { x: 0.0, y: 0.0 }),
            counterclockwise: true,
            complement: false,
        };
        let value = BuiltinOperationTable
            .evaluate(
                "intersection",
                &ObjectOp::LineCircleIntersection { variant: 0 },
                &[&line, &arc],
            )
            .unwrap();
        assert_eq!(value.as_point(), Some(Point { x: -2.0, y: 0.0 }));
    }

    #[test]
    fn line_intersection_with_a_collapsed_arc_keeps_its_shared_endpoint() {
        let line = ObjectValue::Line {
            line_kind: LineKind::Line,
            start: Point { x: -3.0, y: 0.0 },
            end: Point { x: 3.0, y: 0.0 },
        };
        let endpoint = Point { x: 2.0, y: 0.0 };
        let arc = ObjectValue::Arc {
            start: endpoint,
            mid: endpoint,
            end: endpoint,
            center: Some(Point { x: 0.0, y: 0.0 }),
            counterclockwise: true,
            complement: false,
        };
        let value = BuiltinOperationTable
            .evaluate(
                "intersection",
                &ObjectOp::LineCircleIntersection { variant: 0 },
                &[&line, &arc],
            )
            .unwrap();
        assert_eq!(value.as_point(), Some(endpoint));
    }

    #[test]
    fn table_drives_a_geometry_construction_from_source_points() {
        let graph = ObjectGraph::build(vec![
            ObjectNode::source("a"),
            ObjectNode::source("b"),
            ObjectNode::source("axis-start"),
            ObjectNode::source("axis-end"),
            ObjectNode::derived("midpoint", ObjectOp::Midpoint, ["a", "b"]),
            ObjectNode::derived(
                "reflected",
                ObjectOp::ReflectPoint,
                ["midpoint", "axis-start", "axis-end"],
            ),
            ObjectNode::derived(
                "segment",
                ObjectOp::Line {
                    line_kind: LineKind::Segment,
                },
                ["midpoint", "reflected"],
            ),
        ])
        .unwrap();
        let mut values = ObjectValues::new(&graph);
        for (id, point) in [
            ("a", Point { x: 0.0, y: 0.0 }),
            ("b", Point { x: 4.0, y: 0.0 }),
            ("axis-start", Point { x: 0.0, y: -1.0 }),
            ("axis-end", Point { x: 0.0, y: 1.0 }),
        ] {
            values
                .set_source::<_, ObjectOpError>(&graph, id, ObjectValue::point(point))
                .unwrap();
        }
        values
            .evaluate_all(&graph, &mut BuiltinOperationTable)
            .unwrap();
        assert_eq!(
            values.get(&graph, "reflected"),
            Some(&ObjectValue::point(Point { x: -2.0, y: 0.0 }))
        );
        assert_eq!(
            values.get(&graph, "segment"),
            Some(&ObjectValue::Line {
                line_kind: LineKind::Segment,
                start: Point { x: 2.0, y: 0.0 },
                end: Point { x: -2.0, y: 0.0 },
            })
        );
    }

    #[test]
    fn operation_arity_is_explicitly_validated() {
        let error = BuiltinOperationTable
            .evaluate("bad", &ObjectOp::Midpoint, &[])
            .unwrap_err();
        assert_eq!(
            error,
            ObjectOpError::WrongArity {
                op: "midpoint",
                expected: 2,
                actual: 0,
            }
        );
    }

    #[test]
    fn line_projection_op_respects_payload_domain() {
        let segment = ObjectValue::Line {
            line_kind: LineKind::Segment,
            start: Point { x: 0.0, y: 0.0 },
            end: Point { x: 10.0, y: 0.0 },
        };
        let parameter = ObjectValue::Scalar { value: 1.5 };
        let projected = BuiltinOperationTable
            .evaluate(
                "on-segment",
                &ObjectOp::PointOnLine,
                &[&segment, &parameter],
            )
            .unwrap();
        assert_eq!(projected, ObjectValue::point(Point { x: 10.0, y: 0.0 }));
        assert!(
            crate::project_to_line_like(
                Point { x: 15.0, y: 3.0 },
                Point { x: 0.0, y: 0.0 },
                Point { x: 10.0, y: 0.0 },
                LineKind::Segment,
            )
            .is_some()
        );
    }

    #[test]
    fn normalized_polyline_parameter_selects_the_live_segment() {
        let polyline = ObjectValue::Points {
            points: vec![
                Point { x: 0.0, y: 0.0 },
                Point { x: 10.0, y: 0.0 },
                Point { x: 10.0, y: 10.0 },
            ],
        };
        let parameter = ObjectValue::Scalar { value: 0.75 };
        let point = BuiltinOperationTable
            .evaluate(
                "on-polyline",
                &ObjectOp::PointOnPolyline,
                &[&polyline, &parameter],
            )
            .unwrap();
        assert_eq!(point, ObjectValue::point(Point { x: 10.0, y: 5.0 }));
    }

    #[test]
    fn angle_marker_is_derived_from_three_parent_points() {
        let start = ObjectValue::point(Point { x: 20.0, y: 0.0 });
        let vertex = ObjectValue::point(Point { x: 0.0, y: 0.0 });
        let end = ObjectValue::point(Point { x: 0.0, y: 20.0 });
        let marker = BuiltinOperationTable
            .evaluate(
                "marker",
                &ObjectOp::AngleMarker { marker_class: 1 },
                &[&start, &vertex, &end],
            )
            .unwrap();
        assert_eq!(
            marker,
            ObjectValue::Points {
                points: vec![
                    Point { x: 10.0, y: 0.0 },
                    Point { x: 10.0, y: 10.0 },
                    Point { x: 0.0, y: 10.0 },
                ]
            }
        );
    }

    #[test]
    fn angle_marker_stays_typed_when_its_parent_points_are_degenerate() {
        let point = ObjectValue::point(Point { x: 0.0, y: 0.0 });
        let marker = BuiltinOperationTable
            .evaluate(
                "marker",
                &ObjectOp::AngleMarker { marker_class: 1 },
                &[&point, &point, &point],
            )
            .unwrap();
        assert_eq!(marker, ObjectValue::Points { points: Vec::new() });
    }

    #[test]
    fn coordinate_trace_and_intersection_follow_source_updates() {
        let graph = ObjectGraph::build(vec![
            ObjectNode::source("origin"),
            ObjectNode::source("scale"),
            ObjectNode::source("line-start"),
            ObjectNode::source("line-end"),
            ObjectNode::derived(
                "axis",
                ObjectOp::Line {
                    line_kind: LineKind::Line,
                },
                ["line-start", "line-end"],
            ),
            ObjectNode::derived(
                "trace",
                ObjectOp::CoordinateTrace {
                    x_expression: ObjectExpression::Binary {
                        left: Box::new(ObjectExpression::Parameter {
                            name: "t".into(),
                            default: 0.0,
                        }),
                        op: BinaryOp::Mul,
                        right: Box::new(ObjectExpression::Parameter {
                            name: "scale".into(),
                            default: 1.0,
                        }),
                    },
                    y_expression: None,
                    parameter_names: vec!["scale".into()],
                    trace_parameter_name: "t".into(),
                    value_min: 0.0,
                    value_max: 5.0,
                    sample_count: 11,
                    mode: CoordinateTraceMode::Vertical,
                },
                ["origin", "scale"],
            ),
            ObjectNode::derived(
                "hit",
                ObjectOp::LinePolylineIntersection {
                    variant: 0,
                    sample_hint: None,
                },
                ["axis", "trace"],
            ),
        ])
        .unwrap();
        let mut values = ObjectValues::new(&graph);
        for (id, value) in [
            ("origin", ObjectValue::point(Point { x: 4.0, y: -2.0 })),
            ("scale", ObjectValue::Scalar { value: 1.0 }),
            ("line-start", ObjectValue::point(Point { x: 0.0, y: 0.0 })),
            ("line-end", ObjectValue::point(Point { x: 1.0, y: 0.0 })),
        ] {
            values
                .set_source::<_, ObjectOpError>(&graph, id, value)
                .unwrap();
        }
        values
            .evaluate_all(&graph, &mut BuiltinOperationTable)
            .unwrap();
        assert_eq!(
            values.get(&graph, "hit"),
            Some(&ObjectValue::point(Point { x: 4.0, y: 0.0 }))
        );

        values
            .set_source::<_, ObjectOpError>(
                &graph,
                "origin",
                ObjectValue::point(Point { x: 6.0, y: -2.0 }),
            )
            .unwrap();
        values
            .evaluate_affected(&graph, &["origin".into()], &mut BuiltinOperationTable)
            .unwrap();
        assert_eq!(
            values.get(&graph, "hit"),
            Some(&ObjectValue::point(Point { x: 6.0, y: 0.0 }))
        );
    }

    #[test]
    fn segment_radius_circle_is_derived_from_three_parent_points() {
        let center = ObjectValue::point(Point { x: 4.0, y: 5.0 });
        let start = ObjectValue::point(Point { x: 1.0, y: 1.0 });
        let end = ObjectValue::point(Point { x: 4.0, y: 5.0 });
        let circle = BuiltinOperationTable
            .evaluate(
                "circle",
                &ObjectOp::CircleBySegmentRadius,
                &[&center, &start, &end],
            )
            .unwrap();
        assert_eq!(
            circle,
            ObjectValue::Circle {
                center: Point { x: 4.0, y: 5.0 },
                radius_point: Point { x: 9.0, y: 5.0 },
            },
        );
    }

    #[test]
    fn center_arc_is_derived_from_its_three_parent_points() {
        let center = ObjectValue::point(Point { x: 0.0, y: 0.0 });
        let start = ObjectValue::point(Point { x: 2.0, y: 0.0 });
        let end = ObjectValue::point(Point { x: 0.0, y: 2.0 });
        let arc = BuiltinOperationTable
            .evaluate(
                "arc",
                &ObjectOp::CenterArc { y_up: true },
                &[&center, &start, &end],
            )
            .unwrap();
        let ObjectValue::Arc {
            start,
            mid,
            end,
            center,
            counterclockwise,
            complement,
        } = arc
        else {
            panic!("center-arc must produce an arc value");
        };
        assert_eq!(start, Point { x: 2.0, y: 0.0 });
        assert_eq!(end, Point { x: 0.0, y: 2.0 });
        assert_eq!(center, Some(Point { x: 0.0, y: 0.0 }));
        assert!((mid.x - 2.0_f64.sqrt()).abs() < 1e-9);
        assert!((mid.y - 2.0_f64.sqrt()).abs() < 1e-9);
        assert!(counterclockwise);
        assert!(!complement);
    }

    #[test]
    fn repeat_point_builds_the_fixed_side_of_a_segment_trace() {
        let point = ObjectValue::point(Point { x: 3.0, y: 4.0 });
        let repeated = BuiltinOperationTable
            .evaluate(
                "trace",
                &ObjectOp::RepeatPoint { sample_count: 3 },
                &[&point],
            )
            .unwrap();
        assert_eq!(
            repeated,
            ObjectValue::Points {
                points: vec![Point { x: 3.0, y: 4.0 }; 3]
            }
        );
    }

    #[test]
    fn constructed_lines_keep_a_typed_degenerate_initial_value() {
        let through = ObjectValue::point(Point { x: 3.0, y: 4.0 });
        let host = ObjectValue::Line {
            line_kind: LineKind::Line,
            start: Point { x: 1.0, y: 2.0 },
            end: Point { x: 1.0, y: 2.0 },
        };
        for op in [ObjectOp::PerpendicularLine, ObjectOp::ParallelLine] {
            assert_eq!(
                BuiltinOperationTable
                    .evaluate("line", &op, &[&through, &host])
                    .unwrap(),
                ObjectValue::Line {
                    line_kind: LineKind::Line,
                    start: Point { x: 3.0, y: 4.0 },
                    end: Point { x: 3.0, y: 4.0 },
                }
            );
        }
    }

    #[test]
    fn arcs_keep_typed_values_at_degenerate_initial_positions() {
        let center = ObjectValue::point(Point { x: 1.0, y: 2.0 });
        let endpoint = ObjectValue::point(Point { x: 1.0, y: 2.0 });
        let arc = BuiltinOperationTable
            .evaluate(
                "arc",
                &ObjectOp::CenterArc { y_up: true },
                &[&center, &endpoint, &endpoint],
            )
            .unwrap();
        assert!(matches!(arc, ObjectValue::Arc { .. }));
        assert_eq!(
            BuiltinOperationTable
                .evaluate(
                    "point",
                    &ObjectOp::PointOnArc,
                    &[&arc, &ObjectValue::Scalar { value: 0.5 }],
                )
                .unwrap(),
            endpoint
        );
    }

    #[test]
    fn arc_length_follows_the_arc_path_through_its_midpoint() {
        let center = ObjectValue::point(Point { x: 0.0, y: 0.0 });
        let start = ObjectValue::point(Point { x: 2.0, y: 0.0 });
        let end = ObjectValue::point(Point { x: 0.0, y: 2.0 });
        let arc = BuiltinOperationTable
            .evaluate(
                "arc",
                &ObjectOp::CenterArc { y_up: true },
                &[&center, &start, &end],
            )
            .unwrap();
        let length = BuiltinOperationTable
            .evaluate("length", &ObjectOp::ArcLength, &[&arc])
            .unwrap();
        assert_eq!(
            length,
            ObjectValue::Scalar {
                value: std::f64::consts::PI
            }
        );
    }

    #[test]
    fn circular_radius_is_derived_from_circle_and_center_arc_parents() {
        let circle = ObjectValue::Circle {
            center: Point { x: 1.0, y: 2.0 },
            radius_point: Point { x: 4.0, y: 6.0 },
        };
        assert_eq!(
            BuiltinOperationTable
                .evaluate("radius", &ObjectOp::CircularRadius, &[&circle])
                .unwrap(),
            ObjectValue::Scalar { value: 5.0 }
        );

        let arc = ObjectValue::Arc {
            start: Point { x: 4.0, y: 6.0 },
            mid: Point { x: 6.0, y: 2.0 },
            end: Point { x: 4.0, y: -2.0 },
            center: Some(Point { x: 1.0, y: 2.0 }),
            counterclockwise: true,
            complement: false,
        };
        assert_eq!(
            BuiltinOperationTable
                .evaluate("radius", &ObjectOp::CircularRadius, &[&arc])
                .unwrap(),
            ObjectValue::Scalar { value: 5.0 }
        );
    }

    #[test]
    fn circle_arc_reads_the_center_from_the_circle_parent() {
        let circle = ObjectValue::Circle {
            center: Point { x: 3.0, y: 4.0 },
            radius_point: Point { x: 5.0, y: 4.0 },
        };
        let start = ObjectValue::point(Point { x: 5.0, y: 4.0 });
        let end = ObjectValue::point(Point { x: 3.0, y: 6.0 });
        let arc = BuiltinOperationTable
            .evaluate(
                "arc",
                &ObjectOp::CircleArc { y_up: true },
                &[&circle, &start, &end],
            )
            .unwrap();
        assert!(matches!(
            arc,
            ObjectValue::Arc {
                center: Some(Point { x: 3.0, y: 4.0 }),
                counterclockwise: true,
                ..
            }
        ));
    }

    #[test]
    fn similarity_iteration_repeatedly_maps_the_parent_polygon() {
        let polygon = ObjectValue::Points {
            points: vec![Point { x: 0.0, y: 0.0 }, Point { x: 1.0, y: 0.0 }],
        };
        let source_start = ObjectValue::point(Point { x: 0.0, y: 0.0 });
        let source_end = ObjectValue::point(Point { x: 1.0, y: 0.0 });
        let target_start = ObjectValue::point(Point { x: 0.0, y: 0.0 });
        let target_end = ObjectValue::point(Point { x: 2.0, y: 0.0 });
        let depth = ObjectValue::Scalar { value: 2.0 };
        let value = BuiltinOperationTable
            .evaluate(
                "iteration",
                &ObjectOp::SimilarityPolygonIteration { inverse: false },
                &[
                    &polygon,
                    &source_start,
                    &source_end,
                    &target_start,
                    &target_end,
                    &depth,
                ],
            )
            .unwrap();
        assert_eq!(
            value,
            ObjectValue::Polygons {
                polygons: vec![
                    vec![Point { x: 0.0, y: 0.0 }, Point { x: 2.0, y: 0.0 }],
                    vec![Point { x: 0.0, y: 0.0 }, Point { x: 4.0, y: 0.0 }],
                ],
            }
        );
    }

    #[test]
    fn parameterized_point_iteration_evaluates_its_typed_point_program() {
        let program = ObjectProgram {
            nodes: vec![
                ObjectNode::source("trace"),
                ObjectNode::source("y"),
                ObjectNode::derived("target", ObjectOp::PointFromScalars, ["trace", "y"]),
            ],
            source_ids: vec!["y".into()],
            target_id: "target".into(),
        };
        let y = ObjectValue::Scalar { value: 9.0 };
        let initial = ObjectValue::Scalar { value: 0.0 };
        let depth = ObjectValue::Scalar { value: 3.0 };
        let value = BuiltinOperationTable
            .evaluate(
                "iteration",
                &ObjectOp::ParameterizedPointIteration {
                    program,
                    trace_source_id: "trace".into(),
                    trace_parameter_name: "t".into(),
                    step_expression: ObjectExpression::Binary {
                        left: Box::new(ObjectExpression::Parameter {
                            name: "t".into(),
                            default: 0.0,
                        }),
                        op: BinaryOp::Add,
                        right: Box::new(ObjectExpression::Constant { value: 1.0 }),
                    },
                    step_parameter_names: Vec::new(),
                },
                &[&y, &initial, &depth],
            )
            .unwrap();
        assert_eq!(
            value,
            ObjectValue::Points {
                points: vec![
                    Point { x: 1.0, y: 9.0 },
                    Point { x: 2.0, y: 9.0 },
                    Point { x: 3.0, y: 9.0 },
                ],
            }
        );
    }

    #[test]
    fn expression_op_reads_only_its_scalar_parent_table() {
        let expression =
            ObjectExpression::from_function_expr(&FunctionExpr::Parsed(FunctionAst::Binary {
                lhs: Box::new(FunctionAst::Parameter("distance".into(), 1.0)),
                op: BinaryOp::Mul,
                rhs: Box::new(FunctionAst::Constant(2.0)),
            }));
        let distance = ObjectValue::Scalar { value: 7.5 };
        let value = BuiltinOperationTable
            .evaluate(
                "expression",
                &ObjectOp::EvaluateExpression {
                    expression,
                    parameter_names: vec!["distance".into()],
                    x: 0.0,
                },
                &[&distance],
            )
            .unwrap();
        assert_eq!(value, ObjectValue::Scalar { value: 15.0 });
    }

    #[test]
    fn color_ops_derive_rgba_from_scalar_parents() {
        let hue_value = ObjectValue::Scalar { value: 2.0 / 3.0 };
        let spectrum = BuiltinOperationTable
            .evaluate(
                "spectrum",
                &ObjectOp::SpectrumColor {
                    base_value: 0.5,
                    period: 1.0,
                    base_color: [0, 255, 255, 127],
                },
                &[&hue_value],
            )
            .unwrap();
        assert_eq!(
            spectrum,
            ObjectValue::Color {
                color: [0, 0, 255, 127],
            }
        );

        let zero = ObjectValue::Scalar { value: 0.0 };
        let half = ObjectValue::Scalar { value: 0.5 };
        let one = ObjectValue::Scalar { value: 1.0 };
        let rgb = BuiltinOperationTable
            .evaluate(
                "rgb",
                &ObjectOp::RgbColor { alpha: 200 },
                &[&one, &half, &zero],
            )
            .unwrap();
        assert_eq!(
            rgb,
            ObjectValue::Color {
                color: [255, 128, 0, 200],
            }
        );
        let hsb = BuiltinOperationTable
            .evaluate(
                "hsb",
                &ObjectOp::HsbColor { alpha: 255 },
                &[&zero, &one, &one],
            )
            .unwrap();
        assert_eq!(
            hsb,
            ObjectValue::Color {
                color: [255, 0, 0, 255],
            }
        );
    }

    #[test]
    fn json_entrypoint_uses_the_same_operation_table() {
        let input = ObjectGraphEvaluationInput {
            nodes: vec![
                ObjectNode::source("a"),
                ObjectNode::source("b"),
                ObjectNode::derived("midpoint", ObjectOp::Midpoint, ["a", "b"]),
            ],
            sources: vec![
                ObjectSourceValue {
                    id: "a".into(),
                    value: ObjectValue::point(Point { x: 2.0, y: 4.0 }),
                },
                ObjectSourceValue {
                    id: "b".into(),
                    value: ObjectValue::point(Point { x: 6.0, y: 8.0 }),
                },
            ],
        };
        let encoded = serde_json::to_vec(&input).unwrap();
        let output = serde_json::from_slice::<Vec<ObjectNodeValue>>(
            &evaluate_object_graph_json(&encoded).unwrap(),
        )
        .unwrap();
        assert_eq!(
            output.last(),
            Some(&ObjectNodeValue {
                id: "midpoint".into(),
                value: ObjectValue::point(Point { x: 4.0, y: 6.0 }),
            })
        );
    }
}
