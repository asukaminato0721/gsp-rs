use super::function_expr_json::FunctionExprJson;
use crate::runtime::scene::{AxisBinding, GeometryTransformBinding};
use serde::Serialize;
use ts_rs::TS;

#[derive(Serialize, TS)]
#[serde(tag = "kind")]
pub(super) enum TransformJson {
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
        #[serde(rename = "angleExpr", skip_serializing_if = "Option::is_none")]
        angle_expr: Option<FunctionExprJson>,
        #[serde(rename = "angleStartIndex", skip_serializing_if = "Option::is_none")]
        angle_start_index: Option<usize>,
        #[serde(rename = "angleVertexIndex", skip_serializing_if = "Option::is_none")]
        angle_vertex_index: Option<usize>,
        #[serde(rename = "angleEndIndex", skip_serializing_if = "Option::is_none")]
        angle_end_index: Option<usize>,
    },
    #[serde(rename = "scale")]
    Scale {
        #[serde(rename = "centerIndex")]
        center_index: usize,
        factor: f64,
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
    #[serde(rename = "reflect")]
    Reflect {
        #[serde(rename = "lineStartIndex", skip_serializing_if = "Option::is_none")]
        line_start_index: Option<usize>,
        #[serde(rename = "lineEndIndex", skip_serializing_if = "Option::is_none")]
        line_end_index: Option<usize>,
        #[serde(rename = "lineIndex", skip_serializing_if = "Option::is_none")]
        line_index: Option<usize>,
    },
    #[serde(rename = "rotate-source-point")]
    RotateSourcePoint {
        #[serde(rename = "sourcePointIndex")]
        source_point_index: usize,
        #[serde(rename = "angleDegrees")]
        angle_degrees: f64,
    },
    #[serde(rename = "translate-source-point")]
    TranslateSourcePoint {
        #[serde(rename = "sourcePointIndex")]
        source_point_index: usize,
        #[serde(rename = "targetIndex")]
        target_index: usize,
    },
}

impl TransformJson {
    pub(super) fn from_transform(transform: &GeometryTransformBinding) -> Self {
        match transform {
            GeometryTransformBinding::TranslateVector {
                vector_start_index,
                vector_end_index,
            } => Self::Translate {
                vector_start_index: *vector_start_index,
                vector_end_index: *vector_end_index,
            },
            GeometryTransformBinding::TranslateDelta { dx, dy } => {
                Self::TranslateDelta { dx: *dx, dy: *dy }
            }
            GeometryTransformBinding::Rotate(binding) => Self::Rotate {
                center_index: binding.center_index,
                angle_degrees: binding.angle_degrees,
                parameter_name: binding.parameter_name.clone(),
                angle_expr: binding.angle_expr.as_ref().map(FunctionExprJson::from_expr),
                angle_start_index: binding.angle_start_index,
                angle_vertex_index: binding.angle_vertex_index,
                angle_end_index: binding.angle_end_index,
            },
            GeometryTransformBinding::Scale(binding) => Self::Scale {
                center_index: binding.center_index,
                factor: binding.factor,
            },
            GeometryTransformBinding::ScaleByRatio(binding) => Self::ScaleByRatio {
                center_index: binding.center_index,
                ratio_origin_index: binding.ratio_origin_index,
                ratio_denominator_index: binding.ratio_denominator_index,
                ratio_numerator_index: binding.ratio_numerator_index,
                signed: binding.signed,
                clamp_to_unit: binding.clamp_to_unit,
            },
            GeometryTransformBinding::Reflect(axis) => Self::from_axis(axis),
            GeometryTransformBinding::RotateAroundSourcePoint {
                source_point_index,
                angle_degrees,
            } => Self::RotateSourcePoint {
                source_point_index: *source_point_index,
                angle_degrees: *angle_degrees,
            },
            GeometryTransformBinding::TranslateSourcePointToPoint {
                source_point_index,
                target_index,
            } => Self::TranslateSourcePoint {
                source_point_index: *source_point_index,
                target_index: *target_index,
            },
        }
    }

    fn from_axis(axis: &AxisBinding) -> Self {
        Self::Reflect {
            line_start_index: axis.line_start_index,
            line_end_index: axis.line_end_index,
            line_index: axis.line_index,
        }
    }
}
