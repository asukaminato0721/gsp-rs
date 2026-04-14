use super::scene_json::{DebugSourceJson, PointJson};
use crate::runtime::geometry::darken;
use crate::runtime::scene::{ArcBoundaryKind, ColorBinding, LineBinding, ShapeBinding};
use serde::Serialize;
use ts_rs::TS;

#[derive(Serialize, TS)]
pub(super) struct LineJson {
    points: Vec<PointJson>,
    color: [u8; 4],
    dashed: bool,
    visible: bool,
    binding: Option<LineBindingJson>,
    #[serde(skip_serializing_if = "Option::is_none")]
    debug: Option<DebugSourceJson>,
}

impl LineJson {
    pub(super) fn from_line(line: &crate::runtime::scene::LineShape) -> Self {
        Self {
            points: PointJson::collect(&line.points),
            color: line.color,
            dashed: line.dashed,
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
        #[serde(rename = "lineStartIndex", skip_serializing_if = "Option::is_none")]
        line_start_index: Option<usize>,
        #[serde(rename = "lineEndIndex", skip_serializing_if = "Option::is_none")]
        line_end_index: Option<usize>,
        #[serde(rename = "lineIndex", skip_serializing_if = "Option::is_none")]
        line_index: Option<usize>,
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
    #[serde(rename = "coordinate-trace")]
    CoordinateTrace {
        #[serde(rename = "pointIndex")]
        point_index: usize,
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
                line_index,
            } => Self::ReflectLine {
                source_index: *source_index,
                line_start_index: *line_start_index,
                line_end_index: *line_end_index,
                line_index: *line_index,
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
            LineBinding::CoordinateTrace {
                point_index,
                x_min,
                x_max,
                sample_count,
            } => Self::CoordinateTrace {
                point_index: *point_index,
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
    outline_color: [u8; 4],
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
            outline_color: darken(polygon.color, 80),
            visible: polygon.visible,
            binding: polygon.binding.as_ref().map(ShapeBindingJson::from_binding),
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
            fill_color_binding: circle
                .fill_color_binding
                .as_ref()
                .map(ColorBindingJson::from_binding),
            dashed: circle.dashed,
            visible: circle.visible,
            binding: circle.binding.as_ref().map(ShapeBindingJson::from_binding),
            debug: circle.debug.as_ref().map(DebugSourceJson::from_source),
        }
    }
}

#[derive(Serialize, TS)]
#[serde(tag = "kind")]
enum ColorBindingJson {
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
            debug: arc.debug.as_ref().map(DebugSourceJson::from_source),
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
    #[serde(rename = "translate-circle")]
    TranslateCircle {
        #[serde(rename = "sourceIndex")]
        source_index: usize,
        dx: f64,
        dy: f64,
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
        #[serde(rename = "lineStartIndex", skip_serializing_if = "Option::is_none")]
        line_start_index: Option<usize>,
        #[serde(rename = "lineEndIndex", skip_serializing_if = "Option::is_none")]
        line_end_index: Option<usize>,
        #[serde(rename = "lineIndex", skip_serializing_if = "Option::is_none")]
        line_index: Option<usize>,
    },
    #[serde(rename = "reflect-circle")]
    ReflectCircle {
        #[serde(rename = "sourceIndex")]
        source_index: usize,
        #[serde(rename = "lineStartIndex", skip_serializing_if = "Option::is_none")]
        line_start_index: Option<usize>,
        #[serde(rename = "lineEndIndex", skip_serializing_if = "Option::is_none")]
        line_end_index: Option<usize>,
        #[serde(rename = "lineIndex", skip_serializing_if = "Option::is_none")]
        line_index: Option<usize>,
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
            ShapeBinding::PointPolygon { vertex_indices } => Self::PointPolygon {
                vertex_indices: vertex_indices.clone(),
            },
            ShapeBinding::ArcBoundaryPolygon {
                host_key,
                boundary_kind,
                center_index,
                start_index,
                mid_index,
                end_index,
                reversed,
                complement,
            } => Self::ArcBoundaryPolygon {
                host_key: *host_key,
                boundary_kind: ArcBoundaryKindJson::from_kind(*boundary_kind),
                center_index: *center_index,
                start_index: *start_index,
                mid_index: *mid_index,
                end_index: *end_index,
                reversed: *reversed,
                complement: *complement,
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
            ShapeBinding::TranslateCircle {
                source_index,
                dx,
                dy,
            } => Self::TranslateCircle {
                source_index: *source_index,
                dx: *dx,
                dy: *dy,
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
                line_index,
            } => Self::ReflectPolygon {
                source_index: *source_index,
                line_start_index: *line_start_index,
                line_end_index: *line_end_index,
                line_index: *line_index,
            },
            ShapeBinding::ReflectCircle {
                source_index,
                line_start_index,
                line_end_index,
                line_index,
            } => Self::ReflectCircle {
                source_index: *source_index,
                line_start_index: *line_start_index,
                line_end_index: *line_end_index,
                line_index: *line_index,
            },
        }
    }
}
