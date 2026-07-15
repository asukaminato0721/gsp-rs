use gsp_runtime_core::LineKind;
use serde::Serialize;
use ts_rs::TS;

use crate::runtime::scene::{Scene, SceneObjectSourceBinding, ScenePointControl};

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub(super) struct ObjectGraphJson {
    geometry_complete: bool,
    nodes: Vec<ObjectGraphNodeJson>,
    sources: Vec<ObjectGraphSourceJson>,
    pending_operations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, TS)]
struct ObjectGraphNodeJson {
    id: String,
    #[ts(type = "unknown")]
    definition: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, TS)]
struct ObjectGraphSourceJson {
    id: String,
    #[ts(type = "unknown")]
    value: serde_json::Value,
    binding: ObjectGraphSourceBindingJson,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(tag = "kind")]
enum ObjectGraphSourceBindingJson {
    #[serde(rename = "initial")]
    Initial,
    #[serde(rename = "parameter")]
    Parameter { name: String },
    #[serde(rename = "point")]
    Point {
        #[serde(rename = "pointIndex")]
        point_index: usize,
    },
    #[serde(rename = "line")]
    Line {
        #[serde(rename = "lineIndex")]
        line_index: usize,
        #[serde(rename = "lineKind")]
        line_kind: Option<SourceLineKindJson>,
    },
    #[serde(rename = "circle")]
    Circle {
        #[serde(rename = "circleIndex")]
        circle_index: usize,
    },
    #[serde(rename = "polygon")]
    Polygon {
        #[serde(rename = "polygonIndex")]
        polygon_index: usize,
    },
    #[serde(rename = "point-control")]
    PointControl {
        #[serde(rename = "pointIndex")]
        point_index: usize,
        control: PointControlJson,
    },
}

#[derive(Debug, Clone, Copy, Serialize, TS)]
#[serde(rename_all = "kebab-case")]
enum PointControlJson {
    Parameter,
    UnitX,
    UnitY,
    Boundary,
}

#[derive(Debug, Clone, Copy, Serialize, TS)]
#[serde(rename_all = "kebab-case")]
enum SourceLineKindJson {
    Segment,
    Line,
    Ray,
}

impl ObjectGraphSourceBindingJson {
    fn from_binding(binding: &SceneObjectSourceBinding) -> Self {
        match binding {
            SceneObjectSourceBinding::Initial => Self::Initial,
            SceneObjectSourceBinding::Parameter { name } => Self::Parameter { name: name.clone() },
            SceneObjectSourceBinding::Point { point_index } => Self::Point {
                point_index: *point_index,
            },
            SceneObjectSourceBinding::Line {
                line_index,
                line_kind,
            } => Self::Line {
                line_index: *line_index,
                line_kind: line_kind.map(|kind| match kind {
                    LineKind::Segment => SourceLineKindJson::Segment,
                    LineKind::Line => SourceLineKindJson::Line,
                    LineKind::Ray => SourceLineKindJson::Ray,
                }),
            },
            SceneObjectSourceBinding::Circle { circle_index } => Self::Circle {
                circle_index: *circle_index,
            },
            SceneObjectSourceBinding::Polygon { polygon_index } => Self::Polygon {
                polygon_index: *polygon_index,
            },
            SceneObjectSourceBinding::PointControl {
                point_index,
                control,
            } => Self::PointControl {
                point_index: *point_index,
                control: match control {
                    ScenePointControl::Parameter => PointControlJson::Parameter,
                    ScenePointControl::UnitX => PointControlJson::UnitX,
                    ScenePointControl::UnitY => PointControlJson::UnitY,
                    ScenePointControl::Boundary => PointControlJson::Boundary,
                },
            },
        }
    }
}

impl ObjectGraphJson {
    pub(super) fn from_scene(scene: &Scene) -> Self {
        let graph = &scene.object_graph;
        Self {
            geometry_complete: graph.geometry_complete,
            nodes: graph
                .nodes
                .iter()
                .map(|node| ObjectGraphNodeJson {
                    id: node.id.clone(),
                    definition: serde_json::to_value(&node.definition)
                        .expect("object graph definition should serialize"),
                })
                .collect(),
            sources: graph
                .sources
                .iter()
                .map(|source| ObjectGraphSourceJson {
                    id: source.id.clone(),
                    value: serde_json::to_value(&source.value)
                        .expect("object graph source should serialize"),
                    binding: ObjectGraphSourceBindingJson::from_binding(&source.binding),
                })
                .collect(),
            pending_operations: graph.pending_operations.clone(),
        }
    }
}
