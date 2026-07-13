use super::function_expr_json::FunctionExprJson;
use super::scene_json::DebugSourceJson;
use crate::runtime::scene::{
    CircleIterationFamily, IterationPointHandle, IterationTable, IterationTableColumn,
    IterationTableValueBinding, LabelIterationFamily, LineIterationFamily, PointIterationFamily,
    PolygonIterationFamily,
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
    Parameterized {
        #[serde(rename = "pointIndex")]
        point_index: usize,
        #[serde(rename = "depthParameterName")]
        depth_parameter_name: Option<String>,
        #[serde(rename = "traceParameterName")]
        trace_parameter_name: String,
        #[serde(rename = "stepExpr")]
        step_expr: FunctionExprJson,
        depth: usize,
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
            PointIterationFamily::Parameterized {
                point_index,
                depth_parameter_name,
                trace_parameter_name,
                step_expr,
                depth,
            } => Self::Parameterized {
                point_index: *point_index,
                depth_parameter_name: depth_parameter_name.clone(),
                trace_parameter_name: trace_parameter_name.clone(),
                step_expr: FunctionExprJson::from_expr(step_expr),
                depth: *depth,
            },
        }
    }
}

#[derive(Serialize, TS)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub(super) enum LineIterationJson {
    Rotate {
        visible: bool,
        #[serde(rename = "sourceIndex")]
        source_index: usize,
        #[serde(rename = "centerIndex")]
        center_index: usize,
        #[serde(rename = "angleExpr")]
        angle_expr: FunctionExprJson,
        depth: usize,
        #[serde(rename = "parameterName")]
        parameter_name: Option<String>,
        #[serde(rename = "depthParameterName")]
        depth_parameter_name: Option<String>,
        color: [u8; 4],
        dashed: bool,
    },
    Translate {
        visible: bool,
        #[serde(rename = "startIndex")]
        start_index: usize,
        #[serde(rename = "endIndex")]
        end_index: usize,
        #[serde(rename = "startControlIndex")]
        start_control_index: Option<usize>,
        #[serde(rename = "endControlIndex")]
        end_control_index: Option<usize>,
        dx: f64,
        dy: f64,
        #[serde(rename = "vectorStartIndex")]
        vector_start_index: Option<usize>,
        #[serde(rename = "vectorEndIndex")]
        vector_end_index: Option<usize>,
        #[serde(rename = "secondaryDx")]
        secondary_dx: Option<f64>,
        #[serde(rename = "secondaryDy")]
        secondary_dy: Option<f64>,
        depth: usize,
        #[serde(rename = "depthExpr")]
        depth_expr: Option<FunctionExprJson>,
        #[serde(rename = "parameterName")]
        parameter_name: Option<String>,
        bidirectional: bool,
        color: [u8; 4],
        dashed: bool,
    },
    Affine {
        visible: bool,
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
        visible: bool,
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
        visible: bool,
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
        match family {
            LineIterationFamily::Rotate {
                binding_group_ordinal: _,
                visible,
                source_index,
                center_index,
                angle_expr,
                depth,
                parameter_name,
                depth_parameter_name,
                color,
                dashed,
            } => Self::Rotate {
                visible: *visible,
                source_index: *source_index,
                center_index: *center_index,
                angle_expr: FunctionExprJson::from_expr(angle_expr),
                depth: *depth,
                parameter_name: parameter_name.clone(),
                depth_parameter_name: depth_parameter_name.clone(),
                color: *color,
                dashed: *dashed,
            },
            LineIterationFamily::ParameterizedPointTrace {
                binding_group_ordinal: _,
                visible,
                point_index,
                driver_index,
                depth_parameter_name,
                trace_parameter_name,
                step_expr,
                depth,
                x_min,
                x_max,
                sample_count,
                color,
                dashed,
            } => Self::ParameterizedPointTrace {
                visible: *visible,
                point_index: *point_index,
                driver_index: *driver_index,
                depth_parameter_name: depth_parameter_name.clone(),
                trace_parameter_name: trace_parameter_name.clone(),
                step_expr: FunctionExprJson::from_expr(step_expr),
                depth: *depth,
                x_min: *x_min,
                x_max: *x_max,
                sample_count: *sample_count,
                color: *color,
                dashed: *dashed,
            },
            LineIterationFamily::Branching {
                binding_group_ordinal: _,
                visible,
                start_index,
                end_index,
                target_segments,
                depth,
                parameter_name,
                color,
                dashed,
            } => Self::Branching {
                visible: *visible,
                start_index: *start_index,
                end_index: *end_index,
                target_segments: target_segments
                    .iter()
                    .cloned()
                    .map(|segment| {
                        segment.map(|handle| IterationPointHandleJson::from_handle(&handle))
                    })
                    .collect(),
                depth: *depth,
                parameter_name: parameter_name.clone(),
                color: *color,
                dashed: *dashed,
            },
            LineIterationFamily::Affine {
                binding_group_ordinal: _,
                visible,
                start_index,
                end_index,
                source_triangle_indices,
                target_triangle,
                depth,
                color,
                dashed,
            } => Self::Affine {
                visible: *visible,
                start_index: *start_index,
                end_index: *end_index,
                source_triangle_indices: *source_triangle_indices,
                target_triangle: target_triangle
                    .clone()
                    .map(|handle| IterationPointHandleJson::from_handle(&handle)),
                depth: *depth,
                color: *color,
                dashed: *dashed,
            },
            LineIterationFamily::Translate {
                binding_group_ordinal: _,
                visible,
                start_index,
                end_index,
                start_control_index,
                end_control_index,
                dx,
                dy,
                vector_start_index,
                vector_end_index,
                secondary_dx,
                secondary_dy,
                depth,
                depth_expr,
                parameter_name,
                bidirectional,
                color,
                dashed,
            } => Self::Translate {
                visible: *visible,
                start_index: *start_index,
                end_index: *end_index,
                start_control_index: *start_control_index,
                end_control_index: *end_control_index,
                dx: *dx,
                dy: *dy,
                vector_start_index: *vector_start_index,
                vector_end_index: *vector_end_index,
                secondary_dx: *secondary_dx,
                secondary_dy: *secondary_dy,
                depth: *depth,
                depth_expr: depth_expr.as_ref().map(FunctionExprJson::from_expr),
                parameter_name: parameter_name.clone(),
                bidirectional: *bidirectional,
                color: *color,
                dashed: *dashed,
            },
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
        visible: bool,
        #[serde(rename = "vertexIndices")]
        vertex_indices: Vec<usize>,
        dx: f64,
        dy: f64,
        #[serde(rename = "secondaryDx")]
        secondary_dx: Option<f64>,
        #[serde(rename = "secondaryDy")]
        secondary_dy: Option<f64>,
        #[serde(rename = "vectorStartIndex")]
        vector_start_index: Option<usize>,
        #[serde(rename = "vectorEndIndex")]
        vector_end_index: Option<usize>,
        depth: usize,
        #[serde(rename = "depthExpr")]
        depth_expr: Option<FunctionExprJson>,
        #[serde(rename = "parameterName")]
        parameter_name: Option<String>,
        bidirectional: bool,
        color: [u8; 4],
    },
    CoordinateGrid {
        visible: bool,
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
                binding_group_ordinal: _,
                visible,
                vertex_indices,
                dx,
                dy,
                secondary_dx,
                secondary_dy,
                vector_start_index,
                vector_end_index,
                depth,
                depth_expr,
                parameter_name,
                bidirectional,
                color,
            } => Self::Translate {
                visible: *visible,
                vertex_indices: vertex_indices.clone(),
                dx: *dx,
                dy: *dy,
                secondary_dx: *secondary_dx,
                secondary_dy: *secondary_dy,
                vector_start_index: *vector_start_index,
                vector_end_index: *vector_end_index,
                depth: *depth,
                depth_expr: depth_expr.as_ref().map(FunctionExprJson::from_expr),
                parameter_name: parameter_name.clone(),
                bidirectional: *bidirectional,
                color: *color,
            },
            PolygonIterationFamily::CoordinateGrid {
                binding_group_ordinal: _,
                visible,
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
                visible: *visible,
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
    TranslateExpression {
        #[serde(rename = "seedLabelIndex")]
        seed_label_index: usize,
        #[serde(rename = "firstOutputLabelIndex")]
        first_output_label_index: Option<usize>,
        #[serde(rename = "outputLabelCount")]
        output_label_count: usize,
        #[serde(rename = "vectorStartIndex")]
        vector_start_index: usize,
        #[serde(rename = "vectorEndIndex")]
        vector_end_index: usize,
        #[serde(rename = "parameterName")]
        parameter_name: String,
        expr: FunctionExprJson,
        depth: usize,
        #[serde(rename = "depthExpr")]
        depth_expr: Option<FunctionExprJson>,
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
            LabelIterationFamily::TranslateExpression {
                seed_label_index,
                first_output_label_index,
                output_label_count,
                vector_start_index,
                vector_end_index,
                parameter_name,
                expr,
                depth,
                depth_expr,
                depth_parameter_name,
            } => Self::TranslateExpression {
                seed_label_index: *seed_label_index,
                first_output_label_index: *first_output_label_index,
                output_label_count: *output_label_count,
                vector_start_index: *vector_start_index,
                vector_end_index: *vector_end_index,
                parameter_name: parameter_name.clone(),
                expr: FunctionExprJson::from_expr(expr),
                depth: *depth,
                depth_expr: depth_expr.as_ref().map(FunctionExprJson::from_expr),
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
    columns: Vec<IterationTableColumnJson>,
    show_index: bool,
    anchor_at_top: bool,
    depth: usize,
    depth_expr: Option<FunctionExprJson>,
    depth_parameter_name: Option<String>,
    visible: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    debug: Option<DebugSourceJson>,
}

#[derive(Serialize, TS)]
#[serde(rename_all = "camelCase")]
struct IterationTableColumnJson {
    expr_label: String,
    parameter_name: String,
    expr: FunctionExprJson,
    value_binding: Option<IterationTableValueBindingJson>,
}

#[derive(Serialize, TS)]
#[serde(tag = "kind", rename_all = "kebab-case")]
enum IterationTableValueBindingJson {
    AngleMarker {
        #[serde(rename = "startIndex")]
        start_index: usize,
        #[serde(rename = "vertexIndex")]
        vertex_index: usize,
        #[serde(rename = "endIndex")]
        end_index: usize,
    },
}

impl IterationTableJson {
    pub(super) fn from_table(table: &IterationTable) -> Self {
        Self {
            x: table.anchor.x,
            y: table.anchor.y,
            expr_label: table.expr_label.clone(),
            parameter_name: table.parameter_name.clone(),
            expr: FunctionExprJson::from_expr(&table.expr),
            columns: table
                .columns
                .iter()
                .map(IterationTableColumnJson::from_column)
                .collect(),
            show_index: table.show_index,
            anchor_at_top: table.anchor_at_top,
            depth: table.depth,
            depth_expr: table.depth_expr.as_ref().map(FunctionExprJson::from_expr),
            depth_parameter_name: table.depth_parameter_name.clone(),
            visible: table.visible,
            debug: table.debug.as_ref().map(DebugSourceJson::from_source),
        }
    }
}

impl IterationTableColumnJson {
    fn from_column(column: &IterationTableColumn) -> Self {
        Self {
            expr_label: column.expr_label.clone(),
            parameter_name: column.parameter_name.clone(),
            expr: FunctionExprJson::from_expr(&column.expr),
            value_binding: column
                .value_binding
                .as_ref()
                .map(IterationTableValueBindingJson::from_binding),
        }
    }
}

impl IterationTableValueBindingJson {
    fn from_binding(binding: &IterationTableValueBinding) -> Self {
        match binding {
            IterationTableValueBinding::AngleMarker {
                start_index,
                vertex_index,
                end_index,
            } => Self::AngleMarker {
                start_index: *start_index,
                vertex_index: *vertex_index,
                end_index: *end_index,
            },
        }
    }
}
