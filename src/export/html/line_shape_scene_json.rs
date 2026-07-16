use super::function_expr_json::FunctionExprJson;
use super::scene_json::{DebugSourceJson, PointJson};
use super::transform_json::TransformJson;
use crate::runtime::scene::{ArcBoundaryKind, ColorBinding, LineBinding, ShapeBinding};
use serde::Serialize;
use ts_rs::TS;

#[derive(Serialize, TS)]
pub(super) struct LineJson {
    points: Vec<PointJson>,
    #[serde(skip_serializing_if = "Option::is_none")]
    segments: Option<Vec<Vec<PointJson>>>,
    color: [u8; 4],
    dashed: bool,
    #[serde(rename = "strokeWidth")]
    stroke_width: f64,
    visible: bool,
    binding: Option<LineBindingJson>,
    #[serde(skip_serializing_if = "Option::is_none")]
    debug: Option<DebugSourceJson>,
}

impl LineJson {
    pub(super) fn from_line(line: &crate::runtime::scene::LineShape) -> Self {
        Self {
            points: PointJson::collect(&line.points),
            segments: matches!(line.binding, Some(LineBinding::SegmentTrace { .. })).then(|| {
                line.points
                    .chunks_exact(2)
                    .map(PointJson::collect)
                    .collect()
            }),
            color: line.color,
            dashed: line.dashed,
            stroke_width: line.stroke_width.unwrap_or(1.0),
            visible: line.visible,
            binding: line.binding.as_ref().map(LineBindingJson::from_binding),
            debug: line.debug.as_ref().map(DebugSourceJson::from_source),
        }
    }
}

#[derive(Serialize, TS)]
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
    #[serde(rename = "matrix-apply")]
    MatrixApply {
        #[serde(rename = "sourceIndex")]
        #[serde(skip_serializing_if = "Option::is_none")]
        source_index: Option<usize>,
        #[serde(rename = "sourceStartIndex")]
        #[serde(skip_serializing_if = "Option::is_none")]
        source_start_index: Option<usize>,
        #[serde(rename = "sourceEndIndex")]
        #[serde(skip_serializing_if = "Option::is_none")]
        source_end_index: Option<usize>,
        #[serde(rename = "matrixApply")]
        matrix_apply: Vec<TransformJson>,
    },
    #[serde(rename = "custom-transform-trace")]
    CustomTransformTrace {
        #[serde(rename = "pointIndex")]
        point_index: usize,
        #[serde(rename = "driverIndex")]
        driver_index: usize,
        #[serde(rename = "xMin")]
        x_min: f64,
        #[serde(rename = "xMax")]
        x_max: f64,
        #[serde(rename = "sampleCount")]
        sample_count: usize,
    },
    #[serde(rename = "coordinate-trace")]
    CoordinateTrace {
        #[serde(rename = "pointIndex")]
        point_index: usize,
        #[serde(rename = "parameterGroupOrdinal")]
        parameter_group_ordinal: usize,
        #[serde(rename = "xMin")]
        x_min: f64,
        #[serde(rename = "xMax")]
        x_max: f64,
        #[serde(rename = "sampleCount")]
        sample_count: usize,
    },
    #[serde(rename = "point-trace")]
    PointTrace {
        #[serde(rename = "pointIndex")]
        point_index: usize,
        #[serde(rename = "driverIndex")]
        driver_index: usize,
        #[serde(rename = "xMin")]
        x_min: f64,
        #[serde(rename = "xMax")]
        x_max: f64,
        #[serde(rename = "sampleCount")]
        sample_count: usize,
    },
    #[serde(rename = "segment-trace")]
    SegmentTrace {
        #[serde(rename = "startIndex")]
        start_index: usize,
        #[serde(rename = "endIndex")]
        end_index: usize,
        #[serde(rename = "driverIndex")]
        driver_index: usize,
        #[serde(rename = "xMin")]
        x_min: f64,
        #[serde(rename = "xMax")]
        x_max: f64,
        #[serde(rename = "sampleCount")]
        sample_count: usize,
    },
    #[serde(rename = "colorized-spectrum")]
    ColorizedSpectrum {
        #[serde(rename = "lineIndex")]
        line_index: usize,
        #[serde(rename = "traceLineIndex")]
        trace_line_index: usize,
        #[serde(rename = "pointIndex")]
        point_index: usize,
        #[serde(rename = "traceEndpointIndex")]
        trace_endpoint_index: usize,
        #[serde(
            rename = "reflectionSourceIndex",
            skip_serializing_if = "Option::is_none"
        )]
        reflection_source_index: Option<usize>,
        #[serde(
            rename = "reflectionAxisLineIndex",
            skip_serializing_if = "Option::is_none"
        )]
        reflection_axis_line_index: Option<usize>,
        #[serde(
            rename = "reflectionFocusIndex",
            skip_serializing_if = "Option::is_none"
        )]
        reflection_focus_index: Option<usize>,
        #[serde(
            rename = "reflectionDirectrixLineIndex",
            skip_serializing_if = "Option::is_none"
        )]
        reflection_directrix_line_index: Option<usize>,
        #[serde(rename = "stepIndex")]
        step_index: usize,
        depth: usize,
        #[serde(rename = "depthParameterName", skip_serializing_if = "Option::is_none")]
        depth_parameter_name: Option<String>,
        ray: bool,
    },
    #[serde(rename = "parametric-curve")]
    ParametricCurve {
        #[serde(rename = "xExpr")]
        x_expr: FunctionExprJson,
        #[serde(rename = "yExpr")]
        y_expr: FunctionExprJson,
        #[serde(rename = "xMin")]
        x_min: f64,
        #[serde(rename = "xMax")]
        x_max: f64,
        #[serde(rename = "sampleCount")]
        sample_count: usize,
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
        complement: bool,
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
            LineBinding::MatrixApply {
                source_index,
                source_start_index,
                source_end_index,
                matrices,
            } => Self::MatrixApply {
                source_index: *source_index,
                source_start_index: *source_start_index,
                source_end_index: *source_end_index,
                matrix_apply: matrices.iter().map(TransformJson::from_transform).collect(),
            },
            LineBinding::CustomTransformTrace {
                point_index,
                driver_index,
                x_min,
                x_max,
                sample_count,
            } => Self::CustomTransformTrace {
                point_index: *point_index,
                driver_index: *driver_index,
                x_min: *x_min,
                x_max: *x_max,
                sample_count: *sample_count,
            },
            LineBinding::CoordinateTrace {
                point_index,
                parameter_group_ordinal,
                x_min,
                x_max,
                sample_count,
            } => Self::CoordinateTrace {
                point_index: *point_index,
                parameter_group_ordinal: *parameter_group_ordinal,
                x_min: *x_min,
                x_max: *x_max,
                sample_count: *sample_count,
            },
            LineBinding::PointTrace {
                point_index,
                driver_index,
                x_min,
                x_max,
                sample_count,
            } => Self::PointTrace {
                point_index: *point_index,
                driver_index: *driver_index,
                x_min: *x_min,
                x_max: *x_max,
                sample_count: *sample_count,
            },
            LineBinding::SegmentTrace {
                start_index,
                end_index,
                driver_index,
                x_min,
                x_max,
                sample_count,
            } => Self::SegmentTrace {
                start_index: *start_index,
                end_index: *end_index,
                driver_index: *driver_index,
                x_min: *x_min,
                x_max: *x_max,
                sample_count: *sample_count,
            },
            LineBinding::ColorizedSpectrum {
                line_index,
                trace_line_index,
                point_index,
                trace_endpoint_index,
                reflection_source_index,
                reflection_axis_line_index,
                reflection_focus_index,
                reflection_directrix_line_index,
                step_index,
                depth,
                depth_parameter_name,
                ray,
            } => Self::ColorizedSpectrum {
                line_index: *line_index,
                trace_line_index: *trace_line_index,
                point_index: *point_index,
                trace_endpoint_index: *trace_endpoint_index,
                reflection_source_index: *reflection_source_index,
                reflection_axis_line_index: *reflection_axis_line_index,
                reflection_focus_index: *reflection_focus_index,
                reflection_directrix_line_index: *reflection_directrix_line_index,
                step_index: *step_index,
                depth: *depth,
                depth_parameter_name: depth_parameter_name.clone(),
                ray: *ray,
            },
            LineBinding::ParametricCurve {
                x_expr,
                y_expr,
                x_min,
                x_max,
                sample_count,
            } => Self::ParametricCurve {
                x_expr: FunctionExprJson::from_expr(x_expr),
                y_expr: FunctionExprJson::from_expr(y_expr),
                x_min: *x_min,
                x_max: *x_max,
                sample_count: *sample_count,
            },
            LineBinding::ArcBoundary {
                host_key,
                boundary_kind,
                center_index,
                start_index,
                mid_index,
                end_index,
                reversed,
                complement,
            } => Self::ArcBoundary {
                host_key: *host_key,
                boundary_kind: ArcBoundaryKindJson::from_kind(*boundary_kind),
                center_index: *center_index,
                start_index: *start_index,
                mid_index: *mid_index,
                end_index: *end_index,
                reversed: *reversed,
                complement: *complement,
            },
        }
    }
}

#[derive(Serialize, TS)]
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

#[derive(Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub(super) struct PolygonJson {
    points: Vec<PointJson>,
    color: [u8; 4],
    color_binding: Option<ColorBindingJson>,
    visible: bool,
    binding: Option<ShapeBindingJson>,
    #[serde(skip_serializing_if = "Option::is_none")]
    debug: Option<DebugSourceJson>,
}

impl PolygonJson {
    pub(super) fn from_polygon(polygon: &crate::runtime::scene::PolygonShape) -> Self {
        Self {
            points: PointJson::collect(&polygon.points),
            color: polygon.color,
            color_binding: polygon
                .color_binding
                .as_ref()
                .map(ColorBindingJson::from_binding),
            visible: polygon.visible,
            binding: polygon
                .binding
                .as_ref()
                .and_then(ShapeBindingJson::try_from_polygon_binding),
            debug: polygon.debug.as_ref().map(DebugSourceJson::from_source),
        }
    }
}

#[derive(Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub(super) struct CircleJson {
    center: PointJson,
    radius_point: PointJson,
    color: [u8; 4],
    fill_color: Option<[u8; 4]>,
    fill_visible: bool,
    fill_color_binding: Option<ColorBindingJson>,
    dashed: bool,
    visible: bool,
    binding: Option<ShapeBindingJson>,
    #[serde(skip_serializing_if = "Option::is_none")]
    debug: Option<DebugSourceJson>,
}

impl CircleJson {
    pub(super) fn from_circle(circle: &crate::runtime::scene::SceneCircle) -> Self {
        Self {
            center: PointJson::from_point(&circle.center),
            radius_point: PointJson::from_point(&circle.radius_point),
            color: circle.color,
            fill_color: circle.fill_color,
            fill_visible: circle.fill_visible,
            fill_color_binding: circle
                .fill_color_binding
                .as_ref()
                .map(ColorBindingJson::from_binding),
            dashed: circle.dashed,
            visible: circle.visible,
            binding: circle
                .binding
                .as_ref()
                .and_then(ShapeBindingJson::try_from_circle_binding),
            debug: circle.debug.as_ref().map(DebugSourceJson::from_source),
        }
    }
}

#[derive(Serialize, TS)]
#[serde(tag = "kind")]
enum ColorBindingJson {
    #[serde(rename = "spectrum")]
    Spectrum {
        #[serde(rename = "pointIndex")]
        point_index: usize,
        #[serde(rename = "baseValue")]
        base_value: f64,
        period: f64,
        #[serde(rename = "baseColor")]
        base_color: [u8; 4],
    },
    #[serde(rename = "rgb")]
    Rgb {
        #[serde(rename = "redPointIndex")]
        red_point_index: usize,
        #[serde(rename = "greenPointIndex")]
        green_point_index: usize,
        #[serde(rename = "bluePointIndex")]
        blue_point_index: usize,
        alpha: u8,
    },
    #[serde(rename = "hsb")]
    Hsb {
        #[serde(rename = "huePointIndex")]
        hue_point_index: usize,
        #[serde(rename = "saturationPointIndex")]
        saturation_point_index: usize,
        #[serde(rename = "brightnessPointIndex")]
        brightness_point_index: usize,
        alpha: u8,
    },
}

impl ColorBindingJson {
    fn from_binding(binding: &ColorBinding) -> Self {
        match binding {
            ColorBinding::Spectrum {
                point_index,
                base_value,
                period,
                base_color,
            } => Self::Spectrum {
                point_index: *point_index,
                base_value: *base_value,
                period: *period,
                base_color: *base_color,
            },
            ColorBinding::Rgb {
                red_point_index,
                green_point_index,
                blue_point_index,
                alpha,
            } => Self::Rgb {
                red_point_index: *red_point_index,
                green_point_index: *green_point_index,
                blue_point_index: *blue_point_index,
                alpha: *alpha,
            },
            ColorBinding::Hsb {
                hue_point_index,
                saturation_point_index,
                brightness_point_index,
                alpha,
            } => Self::Hsb {
                hue_point_index: *hue_point_index,
                saturation_point_index: *saturation_point_index,
                brightness_point_index: *brightness_point_index,
                alpha: *alpha,
            },
        }
    }
}

#[derive(Serialize, TS)]
pub(super) struct ArcJson {
    points: Vec<PointJson>,
    color: [u8; 4],
    center: Option<PointJson>,
    counterclockwise: bool,
    visible: bool,
    binding: Option<ArcBindingJson>,
    #[serde(skip_serializing_if = "Option::is_none")]
    debug: Option<DebugSourceJson>,
}

impl ArcJson {
    pub(super) fn from_arc(arc: &crate::runtime::scene::SceneArc) -> Self {
        Self {
            points: PointJson::collect(&arc.points),
            color: arc.color,
            center: arc.center.as_ref().map(PointJson::from_point),
            counterclockwise: arc.counterclockwise,
            visible: arc.visible,
            binding: arc.binding.as_ref().map(ArcBindingJson::from_binding),
            debug: arc.debug.as_ref().map(DebugSourceJson::from_source),
        }
    }
}

#[derive(Serialize, TS)]
#[serde(tag = "kind")]
enum ArcBindingJson {
    #[serde(rename = "center-arc")]
    CenterArc {
        #[serde(rename = "centerIndex")]
        center_index: usize,
        #[serde(rename = "startIndex")]
        start_index: usize,
        #[serde(rename = "endIndex")]
        end_index: usize,
    },
    #[serde(rename = "circle-arc")]
    CircleArc {
        #[serde(rename = "circleIndex")]
        circle_index: usize,
        #[serde(rename = "startIndex")]
        start_index: usize,
        #[serde(rename = "endIndex")]
        end_index: usize,
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
    #[serde(rename = "matrix-apply")]
    MatrixApply {
        #[serde(rename = "sourceIndex")]
        source_index: usize,
        #[serde(rename = "matrixApply")]
        matrix_apply: Vec<TransformJson>,
    },
}

impl ArcBindingJson {
    fn from_binding(binding: &crate::runtime::scene::ArcBinding) -> Self {
        use crate::runtime::scene::ArcBinding;
        match binding {
            ArcBinding::CenterArc {
                center_index,
                start_index,
                end_index,
            } => Self::CenterArc {
                center_index: *center_index,
                start_index: *start_index,
                end_index: *end_index,
            },
            ArcBinding::CircleArc {
                circle_index,
                start_index,
                end_index,
            } => Self::CircleArc {
                circle_index: *circle_index,
                start_index: *start_index,
                end_index: *end_index,
            },
            ArcBinding::ThreePointArc {
                start_index,
                mid_index,
                end_index,
            } => Self::ThreePointArc {
                start_index: *start_index,
                mid_index: *mid_index,
                end_index: *end_index,
            },
            ArcBinding::MatrixApply {
                source_index,
                matrices,
            } => Self::MatrixApply {
                source_index: *source_index,
                matrix_apply: matrices.iter().map(TransformJson::from_transform).collect(),
            },
        }
    }
}

#[derive(Serialize, TS)]
#[serde(tag = "kind")]
enum ShapeBindingJson {
    #[serde(rename = "point-radius-circle")]
    PointRadiusCircle {
        #[serde(rename = "centerIndex")]
        center_index: usize,
        #[serde(rename = "radiusIndex")]
        radius_index: usize,
    },
    #[serde(rename = "point-polygon")]
    PointPolygon {
        #[serde(rename = "vertexIndices")]
        vertex_indices: Vec<usize>,
    },
    #[serde(rename = "arc-boundary-polygon")]
    ArcBoundaryPolygon {
        #[serde(rename = "hostKey")]
        host_key: usize,
        #[serde(rename = "boundaryKind")]
        boundary_kind: ArcBoundaryKindJson,
        #[serde(rename = "centerIndex")]
        center_index: Option<usize>,
        #[serde(rename = "startIndex")]
        start_index: usize,
        #[serde(rename = "midIndex")]
        mid_index: Option<usize>,
        #[serde(rename = "endIndex")]
        end_index: usize,
        reversed: bool,
        complement: bool,
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
    #[serde(rename = "parameter-radius-circle")]
    ParameterRadiusCircle {
        #[serde(rename = "centerIndex")]
        center_index: usize,
        #[serde(rename = "parameterName")]
        parameter_name: String,
        #[serde(rename = "rawPerUnit")]
        raw_per_unit: f64,
    },
    #[serde(rename = "expression-radius-circle")]
    ExpressionRadiusCircle {
        #[serde(rename = "centerIndex")]
        center_index: usize,
        expr: FunctionExprJson,
    },
    #[serde(rename = "matrix-apply")]
    MatrixApply {
        #[serde(rename = "sourceIndex")]
        source_index: usize,
        #[serde(rename = "matrixApply")]
        matrix_apply: Vec<TransformJson>,
    },
}

impl ShapeBindingJson {
    fn try_from_polygon_binding(binding: &ShapeBinding) -> Option<Self> {
        match binding {
            ShapeBinding::PointPolygon { vertex_indices } => Some(Self::PointPolygon {
                vertex_indices: vertex_indices.clone(),
            }),
            ShapeBinding::ArcBoundaryPolygon {
                host_key,
                boundary_kind,
                center_index,
                start_index,
                mid_index,
                end_index,
                reversed,
                complement,
            } => Some(Self::ArcBoundaryPolygon {
                host_key: *host_key,
                boundary_kind: ArcBoundaryKindJson::from_kind(*boundary_kind),
                center_index: *center_index,
                start_index: *start_index,
                mid_index: *mid_index,
                end_index: *end_index,
                reversed: *reversed,
                complement: *complement,
            }),
            ShapeBinding::MatrixApply {
                source_index,
                matrices,
            } => Some(Self::MatrixApply {
                source_index: *source_index,
                matrix_apply: matrices.iter().map(TransformJson::from_transform).collect(),
            }),
            ShapeBinding::PointRadiusCircle { .. }
            | ShapeBinding::SegmentRadiusCircle { .. }
            | ShapeBinding::ParameterRadiusCircle { .. }
            | ShapeBinding::ExpressionRadiusCircle { .. } => None,
        }
    }

    fn try_from_circle_binding(binding: &ShapeBinding) -> Option<Self> {
        match binding {
            ShapeBinding::PointRadiusCircle {
                center_index,
                radius_index,
            } => Some(Self::PointRadiusCircle {
                center_index: *center_index,
                radius_index: *radius_index,
            }),
            ShapeBinding::SegmentRadiusCircle {
                center_index,
                line_start_index,
                line_end_index,
            } => Some(Self::SegmentRadiusCircle {
                center_index: *center_index,
                line_start_index: *line_start_index,
                line_end_index: *line_end_index,
            }),
            ShapeBinding::ParameterRadiusCircle {
                center_index,
                parameter_name,
                raw_per_unit,
            } => Some(Self::ParameterRadiusCircle {
                center_index: *center_index,
                parameter_name: parameter_name.clone(),
                raw_per_unit: *raw_per_unit,
            }),
            ShapeBinding::ExpressionRadiusCircle {
                center_index, expr, ..
            } => Some(Self::ExpressionRadiusCircle {
                center_index: *center_index,
                expr: FunctionExprJson::from_expr(expr),
            }),
            ShapeBinding::MatrixApply {
                source_index,
                matrices,
            } => Some(Self::MatrixApply {
                source_index: *source_index,
                matrix_apply: matrices.iter().map(TransformJson::from_transform).collect(),
            }),
            ShapeBinding::PointPolygon { .. } | ShapeBinding::ArcBoundaryPolygon { .. } => None,
        }
    }
}
