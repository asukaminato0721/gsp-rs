use crate::runtime::scene::{AxisBinding, LineTransformBinding, ShapeTransformBinding};
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
    },
    #[serde(rename = "scale")]
    Scale {
        #[serde(rename = "centerIndex")]
        center_index: usize,
        factor: f64,
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
}

impl TransformJson {
    pub(super) fn from_line_transform(transform: &LineTransformBinding) -> Self {
        match transform {
            LineTransformBinding::Translate {
                vector_start_index,
                vector_end_index,
            } => Self::Translate {
                vector_start_index: *vector_start_index,
                vector_end_index: *vector_end_index,
            },
            LineTransformBinding::Rotate(binding) => Self::Rotate {
                center_index: binding.center_index,
                angle_degrees: binding.angle_degrees,
                parameter_name: binding.parameter_name.clone(),
            },
            LineTransformBinding::Scale(binding) => Self::Scale {
                center_index: binding.center_index,
                factor: binding.factor,
            },
            LineTransformBinding::Reflect(axis) => Self::from_axis(axis),
        }
    }

    pub(super) fn from_shape_transform(transform: &ShapeTransformBinding) -> Self {
        match transform {
            ShapeTransformBinding::TranslateVector {
                vector_start_index,
                vector_end_index,
            } => Self::Translate {
                vector_start_index: *vector_start_index,
                vector_end_index: *vector_end_index,
            },
            ShapeTransformBinding::TranslateDelta { dx, dy } => Self::TranslateDelta {
                dx: *dx,
                dy: *dy,
            },
            ShapeTransformBinding::Rotate(binding) => Self::Rotate {
                center_index: binding.center_index,
                angle_degrees: binding.angle_degrees,
                parameter_name: binding.parameter_name.clone(),
            },
            ShapeTransformBinding::Scale(binding) => Self::Scale {
                center_index: binding.center_index,
                factor: binding.factor,
            },
            ShapeTransformBinding::Reflect(axis) => Self::from_axis(axis),
        }
    }

    pub(super) fn translate_delta(dx: f64, dy: f64) -> Self {
        Self::TranslateDelta { dx, dy }
    }

    pub(super) fn scale(center_index: usize, factor: f64) -> Self {
        Self::Scale {
            center_index,
            factor,
        }
    }

    pub(super) fn reflect(axis: &AxisBinding) -> Self {
        Self::from_axis(axis)
    }

    fn from_axis(axis: &AxisBinding) -> Self {
        Self::Reflect {
            line_start_index: axis.line_start_index,
            line_end_index: axis.line_end_index,
            line_index: axis.line_index,
        }
    }
}
