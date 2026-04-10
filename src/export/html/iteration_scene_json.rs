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
}

impl LineIterationJson {
    pub(super) fn from_family(family: &LineIterationFamily) -> Self {
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
}

impl PolygonIterationJson {
    pub(super) fn from_family(family: &PolygonIterationFamily) -> Self {
        Self::Translate {
            vertex_indices: family.vertex_indices.clone(),
            dx: family.dx,
            dy: family.dy,
            secondary_dx: family.secondary_dx,
            secondary_dy: family.secondary_dy,
            depth: family.depth,
            parameter_name: family.parameter_name.clone(),
            bidirectional: family.bidirectional,
            color: family.color,
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
