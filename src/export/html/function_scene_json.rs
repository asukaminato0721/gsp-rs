use super::function_expr_json::FunctionExprJson;
use crate::runtime::functions::FunctionPlotMode;
use serde::Serialize;
use ts_rs::TS;

#[derive(Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub(super) struct ParameterJson {
    name: String,
    value: f64,
    unit: Option<String>,
    label_index: Option<usize>,
}

impl ParameterJson {
    pub(super) fn from_parameter(parameter: &crate::runtime::scene::SceneParameter) -> Self {
        Self {
            name: parameter.name.clone(),
            value: parameter.value,
            unit: parameter.unit.clone(),
            label_index: parameter.label_index,
        }
    }
}

#[derive(Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub(super) struct FunctionJson {
    key: usize,
    name: String,
    derivative: bool,
    domain: DomainJson,
    line_index: Option<usize>,
    label_index: usize,
    constrained_point_indices: Vec<usize>,
    expr: FunctionExprJson,
}

impl FunctionJson {
    pub(super) fn from_function(function_def: &crate::runtime::scene::SceneFunction) -> Self {
        Self {
            key: function_def.key,
            name: function_def.name.clone(),
            derivative: function_def.derivative,
            domain: DomainJson::from_descriptor(&function_def.domain),
            line_index: function_def.line_index,
            label_index: function_def.label_index,
            constrained_point_indices: function_def.constrained_point_indices.clone(),
            expr: FunctionExprJson::from_expr(&function_def.expr),
        }
    }
}

#[derive(Serialize, TS)]
#[serde(rename_all = "camelCase")]
struct DomainJson {
    x_min: f64,
    x_max: f64,
    sample_count: usize,
    plot_mode: PlotModeJson,
}

impl DomainJson {
    fn from_descriptor(descriptor: &crate::runtime::functions::FunctionPlotDescriptor) -> Self {
        Self {
            x_min: descriptor.x_min,
            x_max: descriptor.x_max,
            sample_count: descriptor.sample_count,
            plot_mode: PlotModeJson::from_mode(descriptor.mode),
        }
    }
}

#[derive(Serialize, TS)]
#[serde(rename_all = "kebab-case")]
enum PlotModeJson {
    Cartesian,
    Polar,
}

impl PlotModeJson {
    fn from_mode(mode: FunctionPlotMode) -> Self {
        match mode {
            FunctionPlotMode::Cartesian => Self::Cartesian,
            FunctionPlotMode::Polar => Self::Polar,
        }
    }
}
