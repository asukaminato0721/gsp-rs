use std::collections::BTreeMap;

use serde::Deserialize;

use crate::{
    LineKind, Point, angle_bisector_direction, choose_point_candidate, circle_circle_intersections,
    lerp_point, line_circle_intersection_candidate, line_line_intersection, point_circle_tangents,
    point_on_circle_arc, point_on_three_point_arc, project_to_line_like, reflect_across_line,
    rotate_around, scale_around, scale_by_three_point_ratio, three_point_arc_geometry,
};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ResolveInput {
    points: Vec<InputPoint>,
    #[serde(default)]
    y_up: bool,
    #[serde(default)]
    parameters: BTreeMap<String, f64>,
    #[serde(default)]
    point_order: Vec<usize>,
}

#[derive(Clone, Deserialize)]
struct InputPoint {
    x: f64,
    y: f64,
    #[serde(default)]
    constraint: Option<PointConstraint>,
    #[serde(default)]
    binding: Option<PointBinding>,
}

impl InputPoint {
    fn position(&self) -> Point {
        Point {
            x: self.x,
            y: self.y,
        }
    }
}

#[derive(Clone, Deserialize)]
#[serde(tag = "kind")]
enum PointBinding {
    #[serde(rename = "graph-calibration")]
    GraphCalibration,
    #[serde(rename = "parameter")]
    Parameter,
    #[serde(rename = "derived-parameter")]
    DerivedParameter {
        #[serde(rename = "sourceIndex")]
        source_index: usize,
        #[serde(rename = "parameterStartIndex")]
        parameter_start_index: Option<usize>,
        #[serde(rename = "parameterEndIndex")]
        parameter_end_index: Option<usize>,
    },
    #[serde(rename = "constraint-parameter-expr")]
    ConstraintParameterExpr { expr: serde_json::Value },
    #[serde(rename = "constraint-parameter-from-point-expr")]
    ConstraintParameterFromPointExpr {
        #[serde(rename = "sourceIndex")]
        source_index: usize,
        #[serde(rename = "parameterName")]
        parameter_name: String,
        expr: serde_json::Value,
        #[serde(rename = "absoluteValue")]
        absolute_value: bool,
    },
    #[serde(rename = "derived")]
    Derived {
        #[serde(rename = "sourceIndex")]
        source_index: usize,
        transform: PointTransform,
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
        signed: bool,
        #[serde(rename = "clampToUnit")]
        clamp_to_unit: bool,
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
        expr: serde_json::Value,
    },
    #[serde(rename = "coordinate-source")]
    CoordinateSource {
        #[serde(rename = "sourceIndex")]
        source_index: usize,
        name: String,
        expr: serde_json::Value,
        axis: CoordinateAxis,
    },
    #[serde(rename = "coordinate-source-2d")]
    CoordinateSource2d {
        #[serde(rename = "sourceIndex")]
        source_index: usize,
        #[serde(rename = "xName")]
        x_name: String,
        #[serde(rename = "xExpr")]
        x_expr: serde_json::Value,
        #[serde(rename = "yName")]
        y_name: String,
        #[serde(rename = "yExpr")]
        y_expr: serde_json::Value,
    },
    #[serde(rename = "polar-offset")]
    PolarOffset {
        #[serde(rename = "sourceIndex")]
        source_index: usize,
        #[serde(rename = "distanceExpr")]
        distance_expr: serde_json::Value,
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
        distance_expr: serde_json::Value,
        #[serde(rename = "angleExpr")]
        angle_expr: serde_json::Value,
        #[serde(rename = "distanceRawScale")]
        distance_raw_scale: f64,
        #[serde(rename = "angleDegreesScale")]
        angle_degrees_scale: f64,
    },
    #[serde(other)]
    Unsupported,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum CoordinateAxis {
    Horizontal,
    Vertical,
}

#[derive(Clone, Deserialize)]
#[serde(tag = "kind")]
enum PointTransform {
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
    ReflectConstraint { line: LineConstraint },
    #[serde(rename = "rotate")]
    Rotate {
        #[serde(rename = "centerIndex")]
        center_index: usize,
        #[serde(rename = "angleDegrees")]
        angle_degrees: f64,
        #[serde(rename = "parameterName")]
        parameter_name: Option<String>,
        #[serde(rename = "angleExpr")]
        angle_expr: Option<serde_json::Value>,
        #[serde(rename = "angleStartIndex")]
        angle_start_index: Option<usize>,
        #[serde(rename = "angleVertexIndex")]
        angle_vertex_index: Option<usize>,
        #[serde(rename = "angleEndIndex")]
        angle_end_index: Option<usize>,
        #[serde(rename = "angleParameterPointIndex")]
        angle_parameter_point_index: Option<usize>,
        #[serde(rename = "angleParameterStartIndex")]
        angle_parameter_start_index: Option<usize>,
        #[serde(rename = "angleParameterEndIndex")]
        angle_parameter_end_index: Option<usize>,
        #[serde(rename = "angleParameterScale")]
        angle_parameter_scale: Option<f64>,
    },
    #[serde(rename = "scale")]
    Scale {
        #[serde(rename = "centerIndex")]
        center_index: usize,
        factor: f64,
        #[serde(rename = "parameterName")]
        parameter_name: Option<String>,
        #[serde(rename = "factorExpr")]
        factor_expr: Option<serde_json::Value>,
        #[serde(rename = "factorParameterPointIndex")]
        factor_parameter_point_index: Option<usize>,
        #[serde(rename = "factorParameterStartIndex")]
        factor_parameter_start_index: Option<usize>,
        #[serde(rename = "factorParameterEndIndex")]
        factor_parameter_end_index: Option<usize>,
    },
    #[serde(other)]
    Unsupported,
}

#[derive(Clone, Deserialize)]
#[serde(tag = "kind")]
enum PointConstraint {
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
    #[serde(rename = "line-constraint")]
    OnLineConstraint { line: LineConstraint, t: f64 },
    #[serde(rename = "ray-constraint")]
    OnRayConstraint { line: LineConstraint, t: f64 },
    #[serde(rename = "polyline")]
    Polyline {
        points: Vec<Point>,
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
    #[serde(rename = "translated-polygon-boundary")]
    TranslatedPolygonBoundary {
        #[serde(rename = "vertexIndices")]
        vertex_indices: Vec<usize>,
        #[serde(rename = "vectorStartIndex")]
        vector_start_index: usize,
        #[serde(rename = "vectorEndIndex")]
        vector_end_index: usize,
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
        circle: CircularConstraint,
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
        left: LineConstraint,
        right: LineConstraint,
    },
    #[serde(rename = "point-circular-tangent")]
    PointCircularTangent {
        #[serde(rename = "pointIndex")]
        point_index: usize,
        circle: CircularConstraint,
        variant: usize,
    },
    #[serde(rename = "line-circular-intersection")]
    LineCircularIntersection {
        line: LineConstraint,
        circle: CircularConstraint,
        variant: usize,
    },
    #[serde(rename = "line-circle-intersection")]
    LineCircleIntersection {
        line: LineConstraint,
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
        left: CircularConstraint,
        right: CircularConstraint,
        variant: usize,
    },
    #[serde(other)]
    Unsupported,
}

#[derive(Clone, Deserialize)]
#[serde(tag = "kind")]
enum LineConstraint {
    #[serde(rename = "segment")]
    Segment {
        #[serde(rename = "startIndex")]
        start_index: usize,
        #[serde(rename = "endIndex")]
        end_index: usize,
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
    #[serde(rename = "perpendicular-line")]
    Perpendicular {
        #[serde(rename = "throughIndex")]
        through_index: usize,
        #[serde(rename = "lineStartIndex")]
        line_start_index: usize,
        #[serde(rename = "lineEndIndex")]
        line_end_index: usize,
    },
    #[serde(rename = "parallel-line")]
    Parallel {
        #[serde(rename = "throughIndex")]
        through_index: usize,
        #[serde(rename = "lineStartIndex")]
        line_start_index: usize,
        #[serde(rename = "lineEndIndex")]
        line_end_index: usize,
    },
    #[serde(rename = "angle-bisector-ray")]
    AngleBisector {
        #[serde(rename = "startIndex")]
        start_index: usize,
        #[serde(rename = "vertexIndex")]
        vertex_index: usize,
        #[serde(rename = "endIndex")]
        end_index: usize,
    },
    #[serde(rename = "translated")]
    Translated {
        line: Box<LineConstraint>,
        #[serde(rename = "vectorStartIndex")]
        vector_start_index: usize,
        #[serde(rename = "vectorEndIndex")]
        vector_end_index: usize,
    },
}

#[derive(Clone, Deserialize)]
#[serde(tag = "kind")]
enum CircularConstraint {
    #[serde(rename = "circle")]
    Circle {
        #[serde(rename = "centerIndex")]
        center_index: usize,
        #[serde(rename = "radiusIndex")]
        radius_index: usize,
    },
    #[serde(rename = "segment-radius-circle")]
    SegmentRadius {
        #[serde(rename = "centerIndex")]
        center_index: usize,
        #[serde(rename = "lineStartIndex")]
        line_start_index: usize,
        #[serde(rename = "lineEndIndex")]
        line_end_index: usize,
    },
    #[serde(rename = "parameter-radius-circle")]
    ParameterRadius {
        #[serde(rename = "centerIndex")]
        center_index: usize,
        #[serde(rename = "parameterName")]
        parameter_name: String,
        #[serde(rename = "parameterValue")]
        parameter_value: f64,
        #[serde(rename = "rawPerUnit")]
        raw_per_unit: f64,
    },
    #[serde(rename = "expression-radius-circle")]
    ExpressionRadius {
        #[serde(rename = "centerIndex")]
        center_index: usize,
        expr: serde_json::Value,
        #[serde(rename = "initialValue")]
        initial_value: f64,
    },
    #[serde(rename = "derived")]
    Derived {
        source: Box<CircularConstraint>,
        transform: CircleTransform,
    },
    #[serde(rename = "circle-arc")]
    CircleArc {
        #[serde(rename = "centerIndex")]
        center_index: usize,
        #[serde(rename = "startIndex")]
        start_index: usize,
        #[serde(rename = "endIndex")]
        _end_index: usize,
    },
    #[serde(rename = "three-point-arc")]
    ThreePointArc {
        #[serde(rename = "startIndex")]
        start_index: usize,
        #[serde(rename = "midIndex")]
        mid_index: usize,
        #[serde(rename = "endIndex")]
        end_index: usize,
    },
    #[serde(other)]
    Unsupported,
}

#[derive(Clone, Deserialize)]
#[serde(tag = "kind")]
enum CircleTransform {
    #[serde(rename = "translate-delta")]
    TranslateDelta { dx: f64, dy: f64 },
    #[serde(rename = "scale")]
    Scale {
        #[serde(rename = "centerIndex")]
        center_index: usize,
        factor: f64,
    },
    #[serde(rename = "reflect")]
    Reflect {
        #[serde(rename = "lineStartIndex")]
        line_start_index: Option<usize>,
        #[serde(rename = "lineEndIndex")]
        line_end_index: Option<usize>,
    },
    #[serde(other)]
    Unsupported,
}

/// Resolves all point constraints supported by the pure geometry runtime.
/// Unsupported scene-dependent constraints are returned as `None`, allowing the
/// browser to route only those cases through its trace/function resolver.
pub fn resolve_point_constraints_json(
    bytes: &[u8],
) -> Result<Vec<Option<Point>>, serde_json::Error> {
    let input = serde_json::from_slice::<ResolveInput>(bytes)?;
    Ok(Resolver::new(input).resolve_all())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct InverseTransformInput {
    world: Point,
    points: Vec<InputPoint>,
    #[serde(default)]
    parameters: BTreeMap<String, f64>,
    transform: PointTransform,
}

pub fn inverse_point_transform_json(bytes: &[u8]) -> Result<Option<Point>, serde_json::Error> {
    let input = serde_json::from_slice::<InverseTransformInput>(bytes)?;
    let mut resolver = Resolver::new(ResolveInput {
        points: input.points,
        y_up: false,
        parameters: input.parameters,
        point_order: Vec::new(),
    });
    Ok(resolver.inverse_transform(&input.transform, input.world))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TransformPointsInput {
    points: Vec<Point>,
    #[serde(default)]
    scene_points: Vec<InputPoint>,
    #[serde(default)]
    lines: Vec<TransformLine>,
    #[serde(default)]
    parameters: BTreeMap<String, f64>,
    transform: ShapeTransform,
}

#[derive(Clone, Deserialize)]
struct TransformLine {
    #[serde(default)]
    points: Vec<ShapePointHandle>,
}

#[derive(Clone, Deserialize)]
#[serde(untagged)]
enum ShapePointHandle {
    Point(Point),
    PointIndex {
        #[serde(rename = "pointIndex")]
        point_index: usize,
    },
    LineIndex {
        #[serde(rename = "lineIndex")]
        line_index: usize,
        #[serde(rename = "segmentIndex", default)]
        segment_index: usize,
        #[serde(default = "default_half")]
        t: f64,
    },
}

fn default_half() -> f64 {
    0.5
}

#[derive(Clone, Deserialize)]
#[serde(tag = "kind")]
enum ShapeTransform {
    #[serde(rename = "translate")]
    Translate {
        #[serde(rename = "vectorStartIndex")]
        vector_start_index: usize,
        #[serde(rename = "vectorEndIndex")]
        vector_end_index: usize,
    },
    #[serde(rename = "translate-delta")]
    TranslateDelta { dx: f64, dy: f64 },
    #[serde(rename = "rotate")]
    Rotate {
        #[serde(rename = "centerIndex")]
        center_index: usize,
        #[serde(rename = "angleDegrees")]
        angle_degrees: f64,
        #[serde(rename = "parameterName")]
        parameter_name: Option<String>,
        #[serde(rename = "angleStartIndex")]
        angle_start_index: Option<usize>,
        #[serde(rename = "angleVertexIndex")]
        angle_vertex_index: Option<usize>,
        #[serde(rename = "angleEndIndex")]
        angle_end_index: Option<usize>,
    },
    #[serde(rename = "scale")]
    Scale {
        #[serde(rename = "centerIndex")]
        center_index: usize,
        factor: f64,
    },
    #[serde(rename = "reflect")]
    Reflect {
        #[serde(rename = "lineStartIndex")]
        line_start_index: Option<usize>,
        #[serde(rename = "lineEndIndex")]
        line_end_index: Option<usize>,
        #[serde(rename = "lineIndex")]
        line_index: Option<usize>,
    },
    #[serde(other)]
    Unsupported,
}

pub fn transform_points_json(bytes: &[u8]) -> Result<Option<Vec<Point>>, serde_json::Error> {
    let input = serde_json::from_slice::<TransformPointsInput>(bytes)?;
    let mut resolver = Resolver::new(ResolveInput {
        points: input.scene_points,
        y_up: false,
        parameters: input.parameters,
        point_order: Vec::new(),
    });
    let transform = resolve_shape_transform(&mut resolver, &input.lines, &input.transform);
    Ok(transform.map(|transform| {
        input
            .points
            .into_iter()
            .filter_map(|point| apply_shape_transform(point, transform))
            .collect()
    }))
}

#[derive(Clone, Copy)]
enum ResolvedShapeTransform {
    Translate { dx: f64, dy: f64 },
    Rotate { center: Point, radians: f64 },
    Scale { center: Point, factor: f64 },
    Reflect { start: Point, end: Point },
}

fn resolve_shape_transform(
    resolver: &mut Resolver,
    lines: &[TransformLine],
    transform: &ShapeTransform,
) -> Option<ResolvedShapeTransform> {
    match transform {
        ShapeTransform::Translate {
            vector_start_index,
            vector_end_index,
        } => {
            let start = resolver.resolve(*vector_start_index)?;
            let end = resolver.resolve(*vector_end_index)?;
            Some(ResolvedShapeTransform::Translate {
                dx: end.x - start.x,
                dy: end.y - start.y,
            })
        }
        ShapeTransform::TranslateDelta { dx, dy } => {
            Some(ResolvedShapeTransform::Translate { dx: *dx, dy: *dy })
        }
        ShapeTransform::Rotate {
            center_index,
            angle_degrees,
            parameter_name,
            angle_start_index,
            angle_vertex_index,
            angle_end_index,
        } => {
            let center = resolver.resolve(*center_index)?;
            let degrees = if let (Some(start), Some(vertex), Some(end)) =
                (angle_start_index, angle_vertex_index, angle_end_index)
            {
                let start = resolver.resolve(*start)?;
                let vertex = resolver.resolve(*vertex)?;
                let end = resolver.resolve(*end)?;
                crate::measured_rotation_radians(start, vertex, end)?.to_degrees()
            } else if let Some(name) = parameter_name {
                resolver.input.parameters.get(name).copied()?
            } else {
                *angle_degrees
            };
            degrees
                .is_finite()
                .then_some(ResolvedShapeTransform::Rotate {
                    center,
                    radians: degrees.to_radians(),
                })
        }
        ShapeTransform::Scale {
            center_index,
            factor,
        } => factor.is_finite().then_some(ResolvedShapeTransform::Scale {
            center: resolver.resolve(*center_index)?,
            factor: *factor,
        }),
        ShapeTransform::Reflect {
            line_start_index: Some(start_index),
            line_end_index: Some(end_index),
            ..
        } => Some(ResolvedShapeTransform::Reflect {
            start: resolver.resolve(*start_index)?,
            end: resolver.resolve(*end_index)?,
        }),
        ShapeTransform::Reflect {
            line_index: Some(line_index),
            ..
        } => {
            let line = lines.get(*line_index)?;
            Some(ResolvedShapeTransform::Reflect {
                start: resolve_shape_handle(resolver, lines, line.points.first()?)?,
                end: resolve_shape_handle(resolver, lines, line.points.last()?)?,
            })
        }
        ShapeTransform::Reflect { .. } | ShapeTransform::Unsupported => None,
    }
}

fn resolve_shape_handle(
    resolver: &mut Resolver,
    lines: &[TransformLine],
    handle: &ShapePointHandle,
) -> Option<Point> {
    match handle {
        ShapePointHandle::Point(point) => Some(*point),
        ShapePointHandle::PointIndex { point_index } => resolver.resolve(*point_index),
        ShapePointHandle::LineIndex {
            line_index,
            segment_index,
            t,
        } => {
            let line = lines.get(*line_index)?;
            let last = line.points.len().checked_sub(2)?;
            let index = (*segment_index).min(last);
            Some(lerp_point(
                resolve_shape_handle(resolver, lines, &line.points[index])?,
                resolve_shape_handle(resolver, lines, &line.points[index + 1])?,
                *t,
            ))
        }
    }
}

fn apply_shape_transform(point: Point, transform: ResolvedShapeTransform) -> Option<Point> {
    match transform {
        ResolvedShapeTransform::Translate { dx, dy } => Some(Point {
            x: point.x + dx,
            y: point.y + dy,
        }),
        ResolvedShapeTransform::Rotate { center, radians } => {
            Some(rotate_around(point, center, radians))
        }
        ResolvedShapeTransform::Scale { center, factor } => {
            Some(scale_around(point, center, factor))
        }
        ResolvedShapeTransform::Reflect { start, end } => reflect_across_line(point, start, end),
    }
}

struct Resolver {
    input: ResolveInput,
    cache: Vec<Option<Option<Point>>>,
    parameter_values: Vec<Option<f64>>,
    visiting: Vec<bool>,
}

impl Resolver {
    fn new(input: ResolveInput) -> Self {
        let count = input.points.len();
        Self {
            input,
            cache: vec![None; count],
            parameter_values: vec![None; count],
            visiting: vec![false; count],
        }
    }

    fn resolve_all(mut self) -> Vec<Option<Point>> {
        let order = if self.input.point_order.is_empty() {
            (0..self.input.points.len()).collect::<Vec<_>>()
        } else {
            self.input.point_order.clone()
        };
        for index in order {
            self.resolve(index);
        }
        (0..self.input.points.len())
            .map(|index| self.resolve(index))
            .collect()
    }

    fn resolve(&mut self, index: usize) -> Option<Point> {
        if let Some(result) = self.cache.get(index).copied().flatten() {
            return result;
        }
        if self.visiting.get(index).copied().unwrap_or(true) {
            return None;
        }
        let point = self.input.points.get(index)?.clone();
        let reference = point.position();
        self.visiting[index] = true;
        let result = (|| {
            let mut constraint = point.constraint;
            let bound = match point.binding.as_ref() {
                Some(binding) => self.resolve_bound(index, binding, &mut constraint, reference),
                None => Some(reference),
            }?;
            match constraint.as_ref() {
                Some(constraint) => self.resolve_constraint(constraint, bound),
                None => Some(bound),
            }
        })();
        self.visiting[index] = false;
        self.cache[index] = Some(result);
        result
    }

    fn resolve_bound(
        &mut self,
        index: usize,
        binding: &PointBinding,
        constraint: &mut Option<PointConstraint>,
        current: Point,
    ) -> Option<Point> {
        let value = match binding {
            PointBinding::DerivedParameter {
                source_index,
                parameter_start_index: Some(start_index),
                parameter_end_index: Some(end_index),
            } => {
                let source = self.resolve(*source_index)?;
                let start = self.resolve(*start_index)?;
                let end = self.resolve(*end_index)?;
                Some(project_to_line_like(source, start, end, LineKind::Segment)?.t)
            }
            PointBinding::DerivedParameter { source_index, .. } => {
                Some(self.constraint_parameter_value(*source_index)?)
            }
            PointBinding::ConstraintParameterExpr { expr } => {
                Some(self.evaluate(expr, 0.0, &[])?)
            }
            PointBinding::ConstraintParameterFromPointExpr {
                source_index,
                parameter_name,
                expr,
                absolute_value,
            } => {
                let source_value = self.constraint_parameter_value(*source_index)?;
                let expr_value =
                    self.evaluate(expr, 0.0, &[(parameter_name.as_str(), source_value)])?;
                Some(if *absolute_value {
                    expr_value
                } else {
                    source_value + expr_value
                })
            }
            _ => None,
        };
        if let Some(value) = value {
            self.apply_constraint_parameter(constraint.as_mut()?, value)?;
            self.parameter_values[index] = Some(value);
            return Some(current);
        }
        self.resolve_binding(binding, current)
    }

    fn resolve_binding(&mut self, binding: &PointBinding, current: Point) -> Option<Point> {
        match binding {
            PointBinding::GraphCalibration | PointBinding::Parameter => Some(current),
            PointBinding::Derived {
                source_index,
                transform,
            } => {
                let source = self.resolve(*source_index)?;
                match transform {
                    PointTransform::Translate {
                        vector_start_index,
                        vector_end_index,
                    } => {
                        let start = self.resolve(*vector_start_index)?;
                        let end = self.resolve(*vector_end_index)?;
                        Some(Point {
                            x: source.x + end.x - start.x,
                            y: source.y + end.y - start.y,
                        })
                    }
                    PointTransform::Reflect {
                        line_start_index,
                        line_end_index,
                    } => reflect_across_line(
                        source,
                        self.resolve(*line_start_index)?,
                        self.resolve(*line_end_index)?,
                    ),
                    PointTransform::ReflectConstraint { line } => {
                        let [start, end] = self.line_points(line)?;
                        reflect_across_line(source, start, end)
                    }
                    PointTransform::Rotate { center_index, .. } => {
                        let center = self.resolve(*center_index)?;
                        let degrees = self.rotation_degrees(transform)?;
                        Some(rotate_around(source, center, degrees.to_radians()))
                    }
                    PointTransform::Scale { center_index, .. } => {
                        let center = self.resolve(*center_index)?;
                        let factor = self.scale_factor(transform)?;
                        Some(scale_around(source, center, factor))
                    }
                    PointTransform::Unsupported => None,
                }
            }
            PointBinding::ScaleByRatio {
                source_index,
                center_index,
                ratio_origin_index,
                ratio_denominator_index,
                ratio_numerator_index,
                signed,
                clamp_to_unit,
            } => scale_by_three_point_ratio(
                self.resolve(*source_index)?,
                self.resolve(*center_index)?,
                self.resolve(*ratio_origin_index)?,
                self.resolve(*ratio_denominator_index)?,
                self.resolve(*ratio_numerator_index)?,
                *signed,
                *clamp_to_unit,
            ),
            PointBinding::Midpoint {
                start_index,
                end_index,
            } => Some(lerp_point(
                self.resolve(*start_index)?,
                self.resolve(*end_index)?,
                0.5,
            )),
            PointBinding::Circumcenter {
                start_index,
                mid_index,
                end_index,
            } => Some(
                three_point_arc_geometry(
                    self.resolve(*start_index)?,
                    self.resolve(*mid_index)?,
                    self.resolve(*end_index)?,
                )?
                .center,
            ),
            PointBinding::Coordinate { name, expr } => {
                let x = self.input.parameters.get(name).copied()?;
                let y = self.evaluate(expr, 0.0, &[])?;
                Some(Point { x, y })
            }
            PointBinding::CoordinateSource {
                source_index,
                name,
                expr,
                axis,
            } => {
                let source = self.resolve(*source_index)?;
                let value = self.input.parameters.get(name).copied()?;
                let offset = self.evaluate(expr, 0.0, &[(name.as_str(), value)])?;
                Some(match axis {
                    CoordinateAxis::Horizontal => Point {
                        x: source.x + offset,
                        y: source.y,
                    },
                    CoordinateAxis::Vertical => Point {
                        x: source.x,
                        y: source.y + offset,
                    },
                })
            }
            PointBinding::CoordinateSource2d {
                source_index,
                x_name,
                x_expr,
                y_name,
                y_expr,
            } => {
                let source = self.resolve(*source_index)?;
                let x_value = self.input.parameters.get(x_name).copied()?;
                let y_value = self.input.parameters.get(y_name).copied()?;
                let locals = [(x_name.as_str(), x_value), (y_name.as_str(), y_value)];
                let dx = self.evaluate(x_expr, 0.0, &locals)?;
                let dy = self.evaluate(y_expr, 0.0, &locals)?;
                Some(Point {
                    x: source.x + dx,
                    y: source.y + dy,
                })
            }
            PointBinding::PolarOffset {
                source_index,
                distance_expr,
                x_scale,
                y_scale,
            } => {
                let source = self.resolve(*source_index)?;
                let distance = self.evaluate(distance_expr, 0.0, &[])?;
                Some(Point {
                    x: source.x + distance * x_scale,
                    y: source.y + distance * y_scale,
                })
            }
            PointBinding::CustomTransform {
                source_index,
                origin_index,
                axis_end_index,
                distance_expr,
                angle_expr,
                distance_raw_scale,
                angle_degrees_scale,
            } => {
                let value = self.constraint_parameter_value(*source_index)?;
                let origin = self.resolve(*origin_index)?;
                let axis_end = self.resolve(*axis_end_index)?;
                let distance =
                    self.evaluate_with_driver(distance_expr, value)? * distance_raw_scale;
                let angle = (-(axis_end.y - origin.y))
                    .atan2(axis_end.x - origin.x)
                    .to_degrees()
                    + self.evaluate_with_driver(angle_expr, value)? * angle_degrees_scale;
                let radians = angle.to_radians();
                Some(Point {
                    x: origin.x + distance * radians.cos(),
                    y: origin.y - distance * radians.sin(),
                })
            }
            PointBinding::DerivedParameter { .. }
            | PointBinding::ConstraintParameterExpr { .. }
            | PointBinding::ConstraintParameterFromPointExpr { .. }
            | PointBinding::Unsupported => None,
        }
    }

    fn rotation_degrees(&mut self, transform: &PointTransform) -> Option<f64> {
        let PointTransform::Rotate {
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
        } = transform
        else {
            return None;
        };
        if let (Some(point_index), Some(start_index), Some(end_index)) = (
            angle_parameter_point_index,
            angle_parameter_start_index,
            angle_parameter_end_index,
        ) {
            let point = self.resolve(*point_index)?;
            let start = self.resolve(*start_index)?;
            let end = self.resolve(*end_index)?;
            return Some(
                project_to_line_like(point, start, end, LineKind::Segment)?.t
                    * angle_parameter_scale.unwrap_or(1.0),
            );
        }
        if let (Some(start_index), Some(vertex_index), Some(end_index)) =
            (angle_start_index, angle_vertex_index, angle_end_index)
        {
            let start = self.resolve(*start_index)?;
            let vertex = self.resolve(*vertex_index)?;
            let end = self.resolve(*end_index)?;
            return Some(crate::measured_rotation_radians(start, vertex, end)?.to_degrees());
        }
        if let Some(expr) = angle_expr {
            return self.evaluate(expr, 0.0, &[]);
        }
        if let Some(name) = parameter_name {
            return self.input.parameters.get(name).copied();
        }
        angle_degrees.is_finite().then_some(*angle_degrees)
    }

    fn scale_factor(&mut self, transform: &PointTransform) -> Option<f64> {
        let PointTransform::Scale {
            factor,
            parameter_name,
            factor_expr,
            factor_parameter_point_index,
            factor_parameter_start_index,
            factor_parameter_end_index,
            ..
        } = transform
        else {
            return None;
        };
        if let (Some(point_index), Some(start_index), Some(end_index)) = (
            factor_parameter_point_index,
            factor_parameter_start_index,
            factor_parameter_end_index,
        ) {
            let point = self.resolve(*point_index)?;
            let start = self.resolve(*start_index)?;
            let end = self.resolve(*end_index)?;
            if let Some(projection) = project_to_line_like(point, start, end, LineKind::Segment) {
                return Some(projection.t);
            }
        }
        if let Some(expr) = factor_expr {
            return self.evaluate(expr, 0.0, &[]);
        }
        if let Some(name) = parameter_name {
            return self.input.parameters.get(name).copied();
        }
        factor.is_finite().then_some(*factor)
    }

    fn inverse_transform(&mut self, transform: &PointTransform, world: Point) -> Option<Point> {
        match transform {
            PointTransform::Translate {
                vector_start_index,
                vector_end_index,
            } => {
                let start = self.resolve(*vector_start_index)?;
                let end = self.resolve(*vector_end_index)?;
                Some(Point {
                    x: world.x - (end.x - start.x),
                    y: world.y - (end.y - start.y),
                })
            }
            PointTransform::Reflect {
                line_start_index,
                line_end_index,
            } => reflect_across_line(
                world,
                self.resolve(*line_start_index)?,
                self.resolve(*line_end_index)?,
            ),
            PointTransform::ReflectConstraint { line } => {
                let [start, end] = self.line_points(line)?;
                reflect_across_line(world, start, end)
            }
            PointTransform::Rotate { center_index, .. } => {
                let center = self.resolve(*center_index)?;
                let degrees = self.rotation_degrees(transform)?;
                Some(rotate_around(world, center, -degrees.to_radians()))
            }
            PointTransform::Scale { center_index, .. } => {
                let center = self.resolve(*center_index)?;
                let factor = self.scale_factor(transform)?;
                (factor.abs() > 1e-12).then(|| scale_around(world, center, factor.recip()))
            }
            PointTransform::Unsupported => None,
        }
    }

    fn evaluate(
        &self,
        encoded_expr: &serde_json::Value,
        x: f64,
        locals: &[(&str, f64)],
    ) -> Option<f64> {
        let encoded = serde_json::to_vec(encoded_expr).ok()?;
        let expr = crate::parse_expression_json(&encoded).ok()?;
        let mut parameters = self.input.parameters.clone();
        for (name, value) in locals {
            parameters.insert((*name).to_owned(), *value);
        }
        crate::evaluate_expr(&expr, x, &parameters).filter(|value| value.is_finite())
    }

    fn evaluate_with_driver(
        &self,
        encoded_expr: &serde_json::Value,
        driver_value: f64,
    ) -> Option<f64> {
        let encoded = serde_json::to_vec(encoded_expr).ok()?;
        let expr = crate::parse_expression_json(&encoded).ok()?;
        let mut parameters = self.input.parameters.clone();
        for name in crate::expression_parameter_names(&expr) {
            parameters.insert(name, driver_value);
        }
        crate::evaluate_expr(&expr, driver_value, &parameters).filter(|value| value.is_finite())
    }

    fn constraint_parameter_value(&mut self, index: usize) -> Option<f64> {
        self.resolve(index)?;
        if let Some(value) = self.parameter_values.get(index).copied().flatten() {
            return Some(value);
        }
        let constraint = self.input.points.get(index)?.constraint.clone()?;
        match constraint {
            PointConstraint::Segment { t, .. }
            | PointConstraint::Line { t, .. }
            | PointConstraint::Ray { t, .. }
            | PointConstraint::OnLineConstraint { t, .. }
            | PointConstraint::OnRayConstraint { t, .. }
            | PointConstraint::Polyline { t, .. }
            | PointConstraint::CircleArc { t, .. }
            | PointConstraint::Arc { t, .. } => t.is_finite().then_some(t),
            PointConstraint::PolygonBoundary {
                vertex_indices,
                edge_index,
                t,
            }
            | PointConstraint::TranslatedPolygonBoundary {
                vertex_indices,
                edge_index,
                t,
                ..
            } => self.polygon_boundary_parameter(&vertex_indices, edge_index, t),
            PointConstraint::Circle { unit_x, unit_y, .. }
            | PointConstraint::CircularConstraint { unit_x, unit_y, .. } => Some(
                (-unit_y).atan2(unit_x).rem_euclid(std::f64::consts::TAU) / std::f64::consts::TAU,
            ),
            _ => None,
        }
    }

    fn polygon_boundary_parameter(
        &mut self,
        vertex_indices: &[usize],
        edge_index: usize,
        t: f64,
    ) -> Option<f64> {
        if vertex_indices.len() < 2 {
            return None;
        }
        let mut perimeter = 0.0;
        let mut traveled = 0.0;
        for index in 0..vertex_indices.len() {
            let start = self.resolve(vertex_indices[index])?;
            let end = self.resolve(vertex_indices[(index + 1) % vertex_indices.len()])?;
            let length = distance(start, end);
            perimeter += length;
            if index < edge_index {
                traveled += length;
            } else if index == edge_index {
                traveled += length * t.clamp(0.0, 1.0);
            }
        }
        (perimeter > 1e-9).then_some(traveled / perimeter)
    }

    fn apply_constraint_parameter(
        &mut self,
        constraint: &mut PointConstraint,
        value: f64,
    ) -> Option<()> {
        if !value.is_finite() {
            return None;
        }
        match constraint {
            PointConstraint::Segment { t, .. } => *t = value.clamp(0.0, 1.0),
            PointConstraint::Line { t, .. } | PointConstraint::OnLineConstraint { t, .. } => {
                *t = value
            }
            PointConstraint::Ray { t, .. } | PointConstraint::OnRayConstraint { t, .. } => {
                *t = value.max(0.0)
            }
            PointConstraint::Polyline {
                points,
                segment_index,
                t,
            } => {
                let count = points.len();
                if count < 2 {
                    return None;
                }
                let scaled = value.rem_euclid(1.0) * (count - 1) as f64;
                *segment_index = (scaled.floor() as usize).min(count - 2);
                *t = scaled - *segment_index as f64;
            }
            PointConstraint::PolygonBoundary {
                vertex_indices,
                edge_index,
                t,
            }
            | PointConstraint::TranslatedPolygonBoundary {
                vertex_indices,
                edge_index,
                t,
                ..
            } => {
                let vertices = vertex_indices
                    .iter()
                    .map(|index| self.resolve(*index))
                    .collect::<Option<Vec<_>>>()?;
                let lengths = vertices
                    .iter()
                    .zip(vertices.iter().cycle().skip(1))
                    .take(vertices.len())
                    .map(|(start, end)| distance(*start, *end))
                    .collect::<Vec<_>>();
                let perimeter = lengths.iter().sum::<f64>();
                if perimeter <= 1e-9 {
                    return None;
                }
                let target = value.rem_euclid(1.0) * perimeter;
                let mut traveled = 0.0;
                for (index, length) in lengths.into_iter().enumerate() {
                    if traveled + length >= target || index + 1 == vertices.len() {
                        *edge_index = index;
                        *t = if length <= 1e-9 {
                            0.0
                        } else {
                            ((target - traveled) / length).clamp(0.0, 1.0)
                        };
                        break;
                    }
                    traveled += length;
                }
            }
            PointConstraint::Circle { unit_x, unit_y, .. }
            | PointConstraint::CircularConstraint { unit_x, unit_y, .. } => {
                let angle = std::f64::consts::TAU * value.rem_euclid(1.0);
                *unit_x = angle.cos();
                *unit_y = -angle.sin();
            }
            PointConstraint::CircleArc { t, .. } | PointConstraint::Arc { t, .. } => {
                *t = value.clamp(0.0, 1.0)
            }
            _ => return None,
        }
        Some(())
    }

    fn resolve_constraint(
        &mut self,
        constraint: &PointConstraint,
        reference: Point,
    ) -> Option<Point> {
        match constraint {
            PointConstraint::Offset {
                origin_index,
                dx,
                dy,
            } => {
                let origin = self.resolve(*origin_index)?;
                Some(Point {
                    x: origin.x + dx,
                    y: origin.y + dy,
                })
            }
            PointConstraint::Segment {
                start_index,
                end_index,
                t,
            }
            | PointConstraint::Line {
                start_index,
                end_index,
                t,
            }
            | PointConstraint::Ray {
                start_index,
                end_index,
                t,
            } => Some(lerp_point(
                self.resolve(*start_index)?,
                self.resolve(*end_index)?,
                *t,
            )),
            PointConstraint::OnLineConstraint { line, t }
            | PointConstraint::OnRayConstraint { line, t } => {
                let [start, end] = self.line_points(line)?;
                Some(lerp_point(start, end, *t))
            }
            PointConstraint::Polyline {
                points,
                segment_index,
                t,
            } => {
                let last = points.len().checked_sub(2)?;
                let index = (*segment_index).min(last);
                Some(lerp_point(points[index], points[index + 1], *t))
            }
            PointConstraint::PolygonBoundary {
                vertex_indices,
                edge_index,
                t,
            } => self.polygon_edge(vertex_indices, *edge_index, *t),
            PointConstraint::TranslatedPolygonBoundary {
                vertex_indices,
                vector_start_index,
                vector_end_index,
                edge_index,
                t,
            } => {
                let base = self.polygon_edge(vertex_indices, *edge_index, *t)?;
                let vector_start = self.resolve(*vector_start_index)?;
                let vector_end = self.resolve(*vector_end_index)?;
                Some(Point {
                    x: base.x + vector_end.x - vector_start.x,
                    y: base.y + vector_end.y - vector_start.y,
                })
            }
            PointConstraint::Circle {
                center_index,
                radius_index,
                unit_x,
                unit_y,
            } => {
                let center = self.resolve(*center_index)?;
                let radius_point = self.resolve(*radius_index)?;
                let radius = distance(center, radius_point);
                Some(Point {
                    x: center.x + radius * unit_x,
                    y: center.y + radius * unit_y,
                })
            }
            PointConstraint::CircularConstraint {
                circle,
                unit_x,
                unit_y,
            } => {
                let (center, radius) = self.circle(circle)?;
                Some(Point {
                    x: center.x + radius * unit_x,
                    y: center.y - radius * unit_y,
                })
            }
            PointConstraint::CircleArc {
                center_index,
                start_index,
                end_index,
                t,
            } => point_on_circle_arc(
                self.resolve(*center_index)?,
                self.resolve(*start_index)?,
                self.resolve(*end_index)?,
                *t,
                self.input.y_up,
            ),
            PointConstraint::Arc {
                start_index,
                mid_index,
                end_index,
                t,
            } => point_on_three_point_arc(
                self.resolve(*start_index)?,
                self.resolve(*mid_index)?,
                self.resolve(*end_index)?,
                *t,
            ),
            PointConstraint::LineIntersection { left, right } => {
                let (left_start, left_end, left_kind) = self.line_geometry(left)?;
                let (right_start, right_end, right_kind) = self.line_geometry(right)?;
                line_line_intersection(
                    left_start,
                    left_end,
                    left_kind,
                    right_start,
                    right_end,
                    right_kind,
                )
            }
            PointConstraint::PointCircularTangent {
                point_index,
                circle,
                variant,
            } => {
                let point = self.resolve(*point_index)?;
                let (center, radius) = self.full_circle(circle)?;
                choose_point_candidate(
                    &point_circle_tangents(point, center, radius),
                    Some(reference),
                    *variant,
                )
            }
            PointConstraint::LineCircularIntersection {
                line,
                circle,
                variant,
            } => {
                let (start, end, kind) = self.line_geometry(line)?;
                let (center, radius) = self.full_circle(circle)?;
                line_circle_intersection_candidate(start, end, kind, center, radius, *variant)
            }
            PointConstraint::LineCircleIntersection {
                line,
                center_index,
                radius_index,
                variant,
            } => {
                let (start, end, kind) = self.line_geometry(line)?;
                let center = self.resolve(*center_index)?;
                let radius_point = self.resolve(*radius_index)?;
                line_circle_intersection_candidate(
                    start,
                    end,
                    kind,
                    center,
                    distance(center, radius_point),
                    *variant,
                )
            }
            PointConstraint::CircleCircleIntersection {
                left_center_index,
                left_radius_index,
                right_center_index,
                right_radius_index,
                variant,
            } => {
                let left_center = self.resolve(*left_center_index)?;
                let left_radius_point = self.resolve(*left_radius_index)?;
                let right_center = self.resolve(*right_center_index)?;
                let right_radius_point = self.resolve(*right_radius_index)?;
                choose_point_candidate(
                    &circle_circle_intersections(
                        left_center,
                        distance(left_center, left_radius_point),
                        right_center,
                        distance(right_center, right_radius_point),
                    ),
                    Some(reference),
                    *variant,
                )
            }
            PointConstraint::CircularIntersection {
                left,
                right,
                variant,
            } => {
                let (left_center, left_radius) = self.full_circle(left)?;
                let (right_center, right_radius) = self.full_circle(right)?;
                choose_point_candidate(
                    &circle_circle_intersections(
                        left_center,
                        left_radius,
                        right_center,
                        right_radius,
                    ),
                    Some(reference),
                    *variant,
                )
            }
            PointConstraint::Unsupported => None,
        }
    }

    fn polygon_edge(&mut self, indices: &[usize], edge_index: usize, t: f64) -> Option<Point> {
        if indices.len() < 2 {
            return None;
        }
        let start = self.resolve(indices[edge_index % indices.len()])?;
        let end = self.resolve(indices[(edge_index + 1) % indices.len()])?;
        Some(lerp_point(start, end, t))
    }

    fn line_points(&mut self, line: &LineConstraint) -> Option<[Point; 2]> {
        let (start, end, _) = self.line_geometry(line)?;
        Some([start, end])
    }

    fn line_geometry(&mut self, line: &LineConstraint) -> Option<(Point, Point, LineKind)> {
        match line {
            LineConstraint::Segment {
                start_index,
                end_index,
            } => Some((
                self.resolve(*start_index)?,
                self.resolve(*end_index)?,
                LineKind::Segment,
            )),
            LineConstraint::Line {
                start_index,
                end_index,
            } => Some((
                self.resolve(*start_index)?,
                self.resolve(*end_index)?,
                LineKind::Line,
            )),
            LineConstraint::Ray {
                start_index,
                end_index,
            } => Some((
                self.resolve(*start_index)?,
                self.resolve(*end_index)?,
                LineKind::Ray,
            )),
            LineConstraint::Perpendicular {
                through_index,
                line_start_index,
                line_end_index,
            } => {
                let through = self.resolve(*through_index)?;
                let start = self.resolve(*line_start_index)?;
                let end = self.resolve(*line_end_index)?;
                let dx = end.x - start.x;
                let dy = end.y - start.y;
                (dx.hypot(dy) > 1e-9).then_some((
                    through,
                    Point {
                        x: through.x - dy,
                        y: through.y + dx,
                    },
                    LineKind::Line,
                ))
            }
            LineConstraint::Parallel {
                through_index,
                line_start_index,
                line_end_index,
            } => {
                let through = self.resolve(*through_index)?;
                let start = self.resolve(*line_start_index)?;
                let end = self.resolve(*line_end_index)?;
                let dx = end.x - start.x;
                let dy = end.y - start.y;
                (dx.hypot(dy) > 1e-9).then_some((
                    through,
                    Point {
                        x: through.x + dx,
                        y: through.y + dy,
                    },
                    LineKind::Line,
                ))
            }
            LineConstraint::AngleBisector {
                start_index,
                vertex_index,
                end_index,
            } => {
                let start = self.resolve(*start_index)?;
                let vertex = self.resolve(*vertex_index)?;
                let end = self.resolve(*end_index)?;
                let direction = angle_bisector_direction(start, vertex, end)?;
                Some((
                    vertex,
                    Point {
                        x: vertex.x + direction.x,
                        y: vertex.y + direction.y,
                    },
                    LineKind::Ray,
                ))
            }
            LineConstraint::Translated {
                line,
                vector_start_index,
                vector_end_index,
            } => {
                let (start, end, kind) = self.line_geometry(line)?;
                let vector_start = self.resolve(*vector_start_index)?;
                let vector_end = self.resolve(*vector_end_index)?;
                let dx = vector_end.x - vector_start.x;
                let dy = vector_end.y - vector_start.y;
                Some((
                    Point {
                        x: start.x + dx,
                        y: start.y + dy,
                    },
                    Point {
                        x: end.x + dx,
                        y: end.y + dy,
                    },
                    kind,
                ))
            }
        }
    }

    fn full_circle(&mut self, circle: &CircularConstraint) -> Option<(Point, f64)> {
        is_full_circle(circle)
            .then(|| self.circle(circle))
            .flatten()
    }

    fn circle(&mut self, circle: &CircularConstraint) -> Option<(Point, f64)> {
        match circle {
            CircularConstraint::Circle {
                center_index,
                radius_index,
            } => {
                let center = self.resolve(*center_index)?;
                let radius = distance(center, self.resolve(*radius_index)?);
                Some((center, radius))
            }
            CircularConstraint::SegmentRadius {
                center_index,
                line_start_index,
                line_end_index,
            } => {
                let center = self.resolve(*center_index)?;
                let radius = distance(
                    self.resolve(*line_start_index)?,
                    self.resolve(*line_end_index)?,
                );
                Some((center, radius))
            }
            CircularConstraint::ParameterRadius {
                center_index,
                parameter_name,
                parameter_value,
                raw_per_unit,
            } => {
                let center = self.resolve(*center_index)?;
                let value = self
                    .input
                    .parameters
                    .get(parameter_name)
                    .copied()
                    .unwrap_or(*parameter_value);
                value
                    .is_finite()
                    .then_some((center, value.abs() * raw_per_unit))
            }
            CircularConstraint::ExpressionRadius {
                center_index,
                expr,
                initial_value,
            } => {
                let center = self.resolve(*center_index)?;
                let encoded = serde_json::to_vec(expr).ok()?;
                let value = crate::parse_expression_json(&encoded)
                    .ok()
                    .and_then(|expr| crate::evaluate_expr(&expr, 0.0, &self.input.parameters))
                    .unwrap_or(*initial_value);
                value.is_finite().then_some((center, value.abs()))
            }
            CircularConstraint::Derived { source, transform } => {
                let (source_center, source_radius) = self.circle(source)?;
                match transform {
                    CircleTransform::TranslateDelta { dx, dy } => Some((
                        Point {
                            x: source_center.x + dx,
                            y: source_center.y + dy,
                        },
                        source_radius,
                    )),
                    CircleTransform::Scale {
                        center_index,
                        factor,
                    } => Some((
                        crate::scale_around(source_center, self.resolve(*center_index)?, *factor),
                        source_radius * factor.abs(),
                    )),
                    CircleTransform::Reflect {
                        line_start_index: Some(line_start_index),
                        line_end_index: Some(line_end_index),
                    } => Some((
                        crate::reflect_across_line(
                            source_center,
                            self.resolve(*line_start_index)?,
                            self.resolve(*line_end_index)?,
                        )?,
                        source_radius,
                    )),
                    CircleTransform::Reflect { .. } | CircleTransform::Unsupported => None,
                }
            }
            CircularConstraint::CircleArc {
                center_index,
                start_index,
                ..
            } => {
                let center = self.resolve(*center_index)?;
                let radius = distance(center, self.resolve(*start_index)?);
                Some((center, radius))
            }
            CircularConstraint::ThreePointArc {
                start_index,
                mid_index,
                end_index,
            } => {
                let geometry = crate::three_point_arc_geometry(
                    self.resolve(*start_index)?,
                    self.resolve(*mid_index)?,
                    self.resolve(*end_index)?,
                )?;
                Some((geometry.center, geometry.radius))
            }
            CircularConstraint::Unsupported => None,
        }
    }
}

fn is_full_circle(circle: &CircularConstraint) -> bool {
    match circle {
        CircularConstraint::Circle { .. }
        | CircularConstraint::SegmentRadius { .. }
        | CircularConstraint::ParameterRadius { .. }
        | CircularConstraint::ExpressionRadius { .. } => true,
        CircularConstraint::Derived { source, .. } => is_full_circle(source),
        CircularConstraint::CircleArc { .. }
        | CircularConstraint::ThreePointArc { .. }
        | CircularConstraint::Unsupported => false,
    }
}

fn distance(left: Point, right: Point) -> f64 {
    (right.x - left.x).hypot(right.y - left.y)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_linked_constraints_in_one_batch() {
        let input = br#"{"points":[{"x":0,"y":0,"constraint":null},{"x":0,"y":0,"constraint":{"kind":"offset","originIndex":0,"dx":2,"dy":3}},{"x":0,"y":0,"constraint":{"kind":"segment","startIndex":0,"endIndex":1,"t":0.5}}],"pointOrder":[1,2]}"#;
        let points = resolve_point_constraints_json(input).unwrap();
        assert_eq!(points[1], Some(Point { x: 2.0, y: 3.0 }));
        assert_eq!(points[2], Some(Point { x: 1.0, y: 1.5 }));
    }

    #[test]
    fn leaves_scene_dependent_constraint_for_browser_fallback() {
        let input =
            br#"{"points":[{"x":1,"y":2,"constraint":{"kind":"line-function-intersection"}}]}"#;
        assert_eq!(resolve_point_constraints_json(input).unwrap(), vec![None]);
    }

    #[test]
    fn resolves_line_circle_and_tangent_intersections() {
        let input = br#"{
          "points": [
            {"x":-2,"y":0},
            {"x":2,"y":0},
            {"x":0,"y":0},
            {"x":0,"y":1},
            {"x":-1,"y":0,"constraint":{"kind":"line-circle-intersection","line":{"kind":"line","startIndex":0,"endIndex":1},"centerIndex":2,"radiusIndex":3,"variant":0}},
            {"x":1,"y":0},
            {"x":0.5,"y":0.9,"constraint":{"kind":"circle-circle-intersection","leftCenterIndex":2,"leftRadiusIndex":3,"rightCenterIndex":5,"rightRadiusIndex":1,"variant":0}},
            {"x":0,"y":-2},
            {"x":0,"y":2},
            {"x":0,"y":0,"constraint":{"kind":"line-intersection","left":{"kind":"line","startIndex":0,"endIndex":1},"right":{"kind":"line","startIndex":7,"endIndex":8}}},
            {"x":0.5,"y":0.9,"constraint":{"kind":"point-circular-tangent","pointIndex":1,"circle":{"kind":"circle","centerIndex":2,"radiusIndex":3},"variant":0}}
          ]
        }"#;
        let points = resolve_point_constraints_json(input).unwrap();
        assert_eq!(points[4], Some(Point { x: -1.0, y: 0.0 }));
        assert_eq!(points[9], Some(Point { x: 0.0, y: 0.0 }));
        for index in [6, 10] {
            let point = points[index].unwrap();
            assert!((point.x - 0.5).abs() < 1e-9);
            assert!((point.y - 3.0_f64.sqrt() / 2.0).abs() < 1e-9);
        }
    }

    #[test]
    fn leaves_arc_intersections_for_arc_membership_fallback() {
        let input = br#"{
          "points": [
            {"x":-2,"y":0}, {"x":2,"y":0}, {"x":0,"y":0},
            {"x":1,"y":0}, {"x":0,"y":1},
            {"x":0,"y":0,"constraint":{"kind":"line-circular-intersection","line":{"kind":"line","startIndex":0,"endIndex":1},"circle":{"kind":"circle-arc","centerIndex":2,"startIndex":3,"endIndex":4},"variant":0}}
          ]
        }"#;
        let points = resolve_point_constraints_json(input).unwrap();
        assert_eq!(points[5], None);
    }

    #[test]
    fn resolves_derived_point_bindings_in_the_same_batch() {
        let input = br#"{
          "parameters":{"t":5},
          "points":[
            {"x":2,"y":0},
            {"x":0,"y":0},
            {"x":1,"y":3},
            {"x":0,"y":0,"binding":{"kind":"derived","sourceIndex":0,"transform":{"kind":"translate","vectorStartIndex":1,"vectorEndIndex":2}}},
            {"x":0,"y":0,"binding":{"kind":"derived","sourceIndex":0,"transform":{"kind":"rotate","centerIndex":1,"angleDegrees":90,"parameterName":null}}},
            {"x":0,"y":0,"binding":{"kind":"derived","sourceIndex":0,"transform":{"kind":"scale","centerIndex":1,"factor":2,"parameterName":null}}},
            {"x":0,"y":0,"binding":{"kind":"midpoint","startIndex":3,"endIndex":4}},
            {"x":0,"y":0,"binding":{"kind":"coordinate","name":"t","expr":{"kind":"parsed","expr":{"kind":"binary","lhs":{"kind":"parameter","name":"t","value":0},"op":"mul","rhs":{"kind":"constant","value":2}}}}},
            {"x":0,"y":0,"binding":{"kind":"coordinate-source","sourceIndex":1,"name":"t","expr":{"kind":"constant","value":2},"axis":"horizontal"}},
            {"x":0,"y":0,"binding":{"kind":"derived","sourceIndex":10,"transform":{"kind":"translate","vectorStartIndex":1,"vectorEndIndex":1}}},
            {"x":0,"y":0,"binding":{"kind":"derived","sourceIndex":9,"transform":{"kind":"translate","vectorStartIndex":1,"vectorEndIndex":1}}}
          ]
        }"#;
        let points = resolve_point_constraints_json(input).unwrap();
        assert_eq!(points[3], Some(Point { x: 3.0, y: 3.0 }));
        assert!((points[4].unwrap().x).abs() < 1e-9);
        assert!((points[4].unwrap().y + 2.0).abs() < 1e-9);
        assert_eq!(points[5], Some(Point { x: 4.0, y: 0.0 }));
        let midpoint = points[6].unwrap();
        assert!((midpoint.x - 1.5).abs() < 1e-9);
        assert!((midpoint.y - 0.5).abs() < 1e-9);
        assert_eq!(points[7], Some(Point { x: 5.0, y: 10.0 }));
        assert_eq!(points[8], Some(Point { x: 2.0, y: 0.0 }));
        assert_eq!(points[9], None);
        assert_eq!(points[10], None);
    }

    #[test]
    fn inverts_payload_defined_point_transforms() {
        let translated = br#"{
          "world":{"x":5,"y":7},
          "points":[{"x":1,"y":2},{"x":4,"y":6}],
          "transform":{"kind":"translate","vectorStartIndex":0,"vectorEndIndex":1}
        }"#;
        assert_eq!(
            inverse_point_transform_json(translated).unwrap(),
            Some(Point { x: 2.0, y: 3.0 })
        );

        let rotated = br#"{
          "world":{"x":0,"y":-2},
          "points":[{"x":0,"y":0}],
          "transform":{"kind":"rotate","centerIndex":0,"angleDegrees":90,"parameterName":null}
        }"#;
        let source = inverse_point_transform_json(rotated).unwrap().unwrap();
        assert!((source.x - 2.0).abs() < 1e-9);
        assert!(source.y.abs() < 1e-9);

        let scaled = br#"{
          "world":{"x":4,"y":2},
          "points":[{"x":0,"y":0}],
          "transform":{"kind":"scale","centerIndex":0,"factor":2,"parameterName":null}
        }"#;
        assert_eq!(
            inverse_point_transform_json(scaled).unwrap(),
            Some(Point { x: 2.0, y: 1.0 })
        );
    }

    #[test]
    fn transforms_shape_points_from_payload_transform() {
        let input = br#"{
          "points":[{"x":2,"y":0},{"x":0,"y":2}],
          "scenePoints":[{"x":0,"y":0}],
          "parameters":{"angle":90},
          "transform":{"kind":"rotate","centerIndex":0,"angleDegrees":0,"parameterName":"angle","angleStartIndex":null,"angleVertexIndex":null,"angleEndIndex":null}
        }"#;
        let points = transform_points_json(input).unwrap().unwrap();
        assert!(points[0].x.abs() < 1e-9);
        assert!((points[0].y + 2.0).abs() < 1e-9);
        assert!((points[1].x - 2.0).abs() < 1e-9);
        assert!(points[1].y.abs() < 1e-9);
    }

    #[test]
    fn resolves_constraint_parameter_and_custom_transform_bindings() {
        let input = br#"{
          "points":[
            {"x":0,"y":0},
            {"x":10,"y":0},
            {"x":3,"y":0,"constraint":{"kind":"segment","startIndex":0,"endIndex":1,"t":0.3}},
            {"x":0,"y":0,"constraint":{"kind":"segment","startIndex":0,"endIndex":1,"t":0},"binding":{"kind":"derived-parameter","sourceIndex":2,"parameterStartIndex":null,"parameterEndIndex":null}},
            {"x":0,"y":0,"constraint":{"kind":"segment","startIndex":0,"endIndex":1,"t":0},"binding":{"kind":"constraint-parameter-expr","expr":{"kind":"constant","value":0.7}}},
            {"x":0,"y":0,"binding":{"kind":"custom-transform","sourceIndex":2,"originIndex":0,"axisEndIndex":1,"distanceExpr":{"kind":"parsed","expr":{"kind":"parameter","name":"t","value":0}},"angleExpr":{"kind":"constant","value":0},"distanceRawScale":10,"angleDegreesScale":1}}
          ]
        }"#;
        let points = resolve_point_constraints_json(input).unwrap();
        assert_eq!(points[3], Some(Point { x: 3.0, y: 0.0 }));
        assert_eq!(points[4], Some(Point { x: 7.0, y: 0.0 }));
        assert_eq!(points[5], Some(Point { x: 3.0, y: 0.0 }));
    }
}
