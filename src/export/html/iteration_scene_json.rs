use super::function_expr_json::FunctionExprJson;
use crate::runtime::scene::{
    CircleIterationFamily, IterationPointHandle, IterationTable, LabelIterationFamily,
    LineIterationFamily, PointIterationFamily, PolygonIterationFamily,
};
use serde::Serialize;
use ts_rs::TS;

#[derive(Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub(super) struct CircleIterationJson {
    source_circle_index: usize,
    source_center_index: usize,
    source_next_center_index: usize,
    vertex_indices: Vec<usize>,
    seed_parameter: f64,
    step_parameter: f64,
    depth: usize,
    depth_parameter_name: Option<String>,
    visible: bool,
}

impl CircleIterationJson {
    pub(super) fn from_family(family: &CircleIterationFamily) -> Self {
        Self {
            source_circle_index: family.source_circle_index,
            source_center_index: family.source_center_index,
            source_next_center_index: family.source_next_center_index,
            vertex_indices: family.vertex_indices.clone(),
            seed_parameter: family.seed_parameter,
            step_parameter: family.step_parameter,
            depth: family.depth,
            depth_parameter_name: family.depth_parameter_name.clone(),
            visible: family.visible,
        }
    }
}

#[derive(Serialize, TS)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub(super) enum PointIterationJson {
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
    pub(super) fn from_family(family: &PointIterationFamily) -> Self {
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

#[derive(Serialize, TS)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub(super) enum LineIterationJson {
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
        bidirectional: bool,
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
    Branching {
        #[serde(rename = "startIndex")]
        start_index: usize,
        #[serde(rename = "endIndex")]
        end_index: usize,
        #[serde(rename = "targetSegments")]
        target_segments: Vec<[IterationPointHandleJson; 2]>,
        depth: usize,
        #[serde(rename = "parameterName")]
        parameter_name: Option<String>,
        color: [u8; 4],
        dashed: bool,
    },
    ParameterizedPointTrace {
        #[serde(rename = "pointIndex")]
        point_index: usize,
        #[serde(rename = "driverIndex")]
        driver_index: usize,
        #[serde(rename = "depthParameterName")]
        depth_parameter_name: Option<String>,
        #[serde(rename = "traceParameterName")]
        trace_parameter_name: String,
        #[serde(rename = "stepExpr")]
        step_expr: FunctionExprJson,
        depth: usize,
        #[serde(rename = "xMin")]
        x_min: f64,
        #[serde(rename = "xMax")]
        x_max: f64,
        #[serde(rename = "sampleCount")]
        sample_count: usize,
        color: [u8; 4],
        dashed: bool,
    },
}

impl LineIterationJson {
    pub(super) fn from_family(family: &LineIterationFamily) -> Self {
        if let (
            Some(point_index),
            Some(driver_index),
            Some(trace_parameter_name),
            Some(step_expr),
            Some(x_min),
            Some(x_max),
            Some(sample_count),
        ) = (
            family.trace_point_index,
            family.trace_driver_index,
            family.trace_parameter_name.as_ref(),
            family.trace_step_expr.as_ref(),
            family.trace_x_min,
            family.trace_x_max,
            family.trace_sample_count,
        ) {
            return Self::ParameterizedPointTrace {
                point_index,
                driver_index,
                depth_parameter_name: family.parameter_name.clone(),
                trace_parameter_name: trace_parameter_name.clone(),
                step_expr: FunctionExprJson::from_expr(step_expr),
                depth: family.depth,
                x_min,
                x_max,
                sample_count,
                color: family.color,
                dashed: family.dashed,
            };
        }
        if let Some(target_segments) = family.branch_target_segments.as_ref() {
            return Self::Branching {
                start_index: family.start_index,
                end_index: family.end_index,
                target_segments: target_segments
                    .iter()
                    .cloned()
                    .map(|segment| {
                        segment.map(|handle| IterationPointHandleJson::from_handle(&handle))
                    })
                    .collect(),
                depth: family.depth,
                parameter_name: family.parameter_name.clone(),
                color: family.color,
                dashed: family.dashed,
            };
        }
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
            bidirectional: family.bidirectional,
            color: family.color,
            dashed: family.dashed,
        }
    }
}

#[derive(Serialize, Clone, TS)]
#[serde(untagged)]
pub(super) enum IterationPointHandleJson {
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

#[derive(Serialize, TS)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub(super) enum PolygonIterationJson {
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
        bidirectional: bool,
        color: [u8; 4],
    },
    CoordinateGrid {
        #[serde(rename = "vertexIndices")]
        vertex_indices: Vec<usize>,
        #[serde(rename = "parameterName")]
        parameter_name: String,
        #[serde(rename = "stepExpr")]
        step_expr: FunctionExprJson,
        #[serde(rename = "xExpr")]
        x_expr: FunctionExprJson,
        #[serde(rename = "yExpr")]
        y_expr: FunctionExprJson,
        #[serde(rename = "xRawScale")]
        x_raw_scale: f64,
        #[serde(rename = "yRawScale")]
        y_raw_scale: f64,
        depth: usize,
        #[serde(rename = "depthExpr")]
        depth_expr: Option<FunctionExprJson>,
        color: [u8; 4],
    },
}

impl PolygonIterationJson {
    pub(super) fn from_family(family: &PolygonIterationFamily) -> Self {
        match family {
            PolygonIterationFamily::Translate {
                vertex_indices,
                dx,
                dy,
                secondary_dx,
                secondary_dy,
                depth,
                parameter_name,
                bidirectional,
                color,
            } => Self::Translate {
                vertex_indices: vertex_indices.clone(),
                dx: *dx,
                dy: *dy,
                secondary_dx: *secondary_dx,
                secondary_dy: *secondary_dy,
                depth: *depth,
                parameter_name: parameter_name.clone(),
                bidirectional: *bidirectional,
                color: *color,
            },
            PolygonIterationFamily::CoordinateGrid {
                vertex_indices,
                parameter_name,
                step_expr,
                x_expr,
                y_expr,
                x_raw_scale,
                y_raw_scale,
                depth,
                depth_expr,
                color,
            } => Self::CoordinateGrid {
                vertex_indices: vertex_indices.clone(),
                parameter_name: parameter_name.clone(),
                step_expr: FunctionExprJson::from_expr(step_expr),
                x_expr: FunctionExprJson::from_expr(x_expr),
                y_expr: FunctionExprJson::from_expr(y_expr),
                x_raw_scale: *x_raw_scale,
                y_raw_scale: *y_raw_scale,
                depth: *depth,
                depth_expr: depth_expr.as_ref().map(FunctionExprJson::from_expr),
                color: *color,
            },
        }
    }
}

#[derive(Serialize, TS)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub(super) enum LabelIterationJson {
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
    pub(super) fn from_family(family: &LabelIterationFamily) -> Self {
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

#[derive(Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub(super) struct IterationTableJson {
    x: f64,
    y: f64,
    expr_label: String,
    parameter_name: String,
    expr: FunctionExprJson,
    depth: usize,
    depth_parameter_name: Option<String>,
    visible: bool,
}

impl IterationTableJson {
    pub(super) fn from_table(table: &IterationTable) -> Self {
        Self {
            x: table.anchor.x,
            y: table.anchor.y,
            expr_label: table.expr_label.clone(),
            parameter_name: table.parameter_name.clone(),
            expr: FunctionExprJson::from_expr(&table.expr),
            depth: table.depth,
            depth_parameter_name: table.depth_parameter_name.clone(),
            visible: table.visible,
        }
    }
}
