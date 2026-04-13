use super::function_scene_json::{FunctionJson, ParameterJson};
use super::iteration_scene_json::{
    CircleIterationJson, IterationTableJson, LabelIterationJson, LineIterationJson,
    PointIterationJson, PolygonIterationJson,
};
use super::label_button_scene_json::{ButtonJson, LabelJson};
use super::line_shape_scene_json::{ArcJson, CircleJson, LineJson, PolygonJson};
use super::point_scene_json::ScenePointJson;
use crate::format::PointRecord;
use crate::runtime::scene::{PayloadDebugSource, Scene};
use serde::Serialize;
use ts_rs::{Config, ExportError, TS};

pub(super) fn scene_to_json(scene: &Scene, width: u32, height: u32, pretty: bool) -> String {
    if pretty {
        serde_json::to_string_pretty(&SceneJson::from_scene(scene, width, height))
    } else {
        serde_json::to_string(&SceneJson::from_scene(scene, width, height))
    }
    .expect("scene JSON serialization should succeed")
}

#[derive(Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "SceneData.ts", rename = "SceneData")]
struct SceneJson {
    width: u32,
    height: u32,
    graph_mode: bool,
    pi_mode: bool,
    saved_viewport: bool,
    y_up: bool,
    bounds: BoundsJson,
    origin: Option<PointJson>,
    images: Vec<ImageJson>,
    lines: Vec<LineJson>,
    polygons: Vec<PolygonJson>,
    circles: Vec<CircleJson>,
    arcs: Vec<ArcJson>,
    labels: Vec<LabelJson>,
    points: Vec<ScenePointJson>,
    point_iterations: Vec<PointIterationJson>,
    circle_iterations: Vec<CircleIterationJson>,
    line_iterations: Vec<LineIterationJson>,
    polygon_iterations: Vec<PolygonIterationJson>,
    label_iterations: Vec<LabelIterationJson>,
    iteration_tables: Vec<IterationTableJson>,
    buttons: Vec<ButtonJson>,
    parameters: Vec<ParameterJson>,
    functions: Vec<FunctionJson>,
}

impl SceneJson {
    fn from_scene(scene: &Scene, width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            graph_mode: scene.graph_mode,
            pi_mode: scene.pi_mode,
            saved_viewport: scene.saved_viewport,
            y_up: scene.y_up,
            bounds: BoundsJson::from_scene(scene),
            origin: scene.origin.as_ref().map(PointJson::from_point),
            images: scene.images.iter().map(ImageJson::from_image).collect(),
            lines: scene.lines.iter().map(LineJson::from_line).collect(),
            polygons: scene
                .polygons
                .iter()
                .map(PolygonJson::from_polygon)
                .collect(),
            circles: scene.circles.iter().map(CircleJson::from_circle).collect(),
            arcs: scene.arcs.iter().map(ArcJson::from_arc).collect(),
            labels: scene.labels.iter().map(LabelJson::from_label).collect(),
            points: scene
                .points
                .iter()
                .map(ScenePointJson::from_scene_point)
                .collect(),
            point_iterations: scene
                .point_iterations
                .iter()
                .map(PointIterationJson::from_family)
                .collect(),
            circle_iterations: scene
                .circle_iterations
                .iter()
                .map(CircleIterationJson::from_family)
                .collect(),
            line_iterations: scene
                .line_iterations
                .iter()
                .map(LineIterationJson::from_family)
                .collect(),
            polygon_iterations: scene
                .polygon_iterations
                .iter()
                .map(PolygonIterationJson::from_family)
                .collect(),
            label_iterations: scene
                .label_iterations
                .iter()
                .map(LabelIterationJson::from_family)
                .collect(),
            iteration_tables: scene
                .iteration_tables
                .iter()
                .map(IterationTableJson::from_table)
                .collect(),
            buttons: scene.buttons.iter().map(ButtonJson::from_button).collect(),
            parameters: scene
                .parameters
                .iter()
                .map(ParameterJson::from_parameter)
                .collect(),
            functions: scene
                .functions
                .iter()
                .map(FunctionJson::from_function)
                .collect(),
        }
    }
}

pub(super) fn export_bindings(cfg: &Config) -> Result<(), ExportError> {
    SceneJson::export_all(cfg)
}

#[derive(Serialize, TS)]
#[serde(rename_all = "camelCase")]
struct ImageJson {
    top_left: PointJson,
    bottom_right: PointJson,
    src: String,
    screen_space: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    debug: Option<DebugSourceJson>,
}

impl ImageJson {
    fn from_image(image: &crate::runtime::scene::SceneImage) -> Self {
        Self {
            top_left: PointJson::from_point(&image.top_left),
            bottom_right: PointJson::from_point(&image.bottom_right),
            src: image.src.clone(),
            screen_space: image.screen_space,
            debug: image.debug.as_ref().map(DebugSourceJson::from_source),
        }
    }
}

#[derive(Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub(super) struct DebugSourceJson {
    group_ordinal: usize,
    group_kind: String,
    record_types: Vec<u32>,
    record_names: Vec<String>,
}

impl DebugSourceJson {
    pub(super) fn from_source(source: &PayloadDebugSource) -> Self {
        Self {
            group_ordinal: source.group_ordinal,
            group_kind: source.group_kind.clone(),
            record_types: source.record_types.clone(),
            record_names: source.record_names.clone(),
        }
    }
}

#[derive(Serialize, TS)]
#[serde(rename_all = "camelCase")]
struct BoundsJson {
    min_x: f64,
    max_x: f64,
    min_y: f64,
    max_y: f64,
}

impl BoundsJson {
    fn from_scene(scene: &Scene) -> Self {
        Self {
            min_x: scene.bounds.min_x,
            max_x: scene.bounds.max_x,
            min_y: scene.bounds.min_y,
            max_y: scene.bounds.max_y,
        }
    }
}

#[derive(Serialize, TS)]
pub(super) struct PointJson {
    x: f64,
    y: f64,
}

impl PointJson {
    pub(super) fn from_point(point: &PointRecord) -> Self {
        Self {
            x: point.x,
            y: point.y,
        }
    }

    pub(super) fn collect(points: &[PointRecord]) -> Vec<Self> {
        points.iter().map(Self::from_point).collect()
    }
}
