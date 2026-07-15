use gsp_runtime_core::object_graph::{ObjectDefinition, ObjectGraph, ObjectNode};
use gsp_runtime_core::{
    AffineTargetHandle, CoordinateTraceMode, FunctionAst, FunctionExpr, LineKind, ObjectExpression,
    ObjectIterationProgram, ObjectOp, ObjectProgram, ObjectTransform, ObjectValue, PlotMode, Point,
    TraceDriver, expression_parameter_names,
};
use serde::Serialize;
use std::collections::BTreeMap;
use ts_rs::TS;

use crate::format::{GroupKind, PointRecord};
use crate::runtime::functions::function_expr_label;
use crate::runtime::scene::{
    ArcBinding, ArcBoundaryKind, ArcConstraint, CircleIterationFamily, CircularConstraint,
    ColorBinding, CoordinateAxis, GeometryTransformBinding, IterationPointHandle, LineBinding,
    LineConstraint, LineIterationFamily, LineLikeKind, PointIterationFamily,
    PolygonIterationFamily, RotationBinding, Scene, SceneObjectGraph, SceneObjectSource,
    SceneObjectSourceBinding, ScenePointBinding, ScenePointConstraint, ScenePointControl,
    SceneScalar, SceneScalarBinding, ShapeBinding, TextLabelBinding,
};

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

pub(crate) fn build_object_graph(scene: &Scene) -> SceneObjectGraph {
    let mut builder = Builder {
        y_up: scene.y_up,
        payload_dependencies: scene.payload_dependencies.clone(),
        point_bindings: scene
            .points
            .iter()
            .map(|point| point.binding.clone())
            .collect(),
        point_constraints: scene
            .points
            .iter()
            .map(|point| point.constraint.clone())
            .collect(),
        line_group_ordinals: scene
            .lines
            .iter()
            .map(|line| line.debug.as_ref().map(|debug| debug.group_ordinal))
            .collect(),
        function_lines: scene
            .functions
            .iter()
            .filter_map(|function| {
                function
                    .line_index
                    .filter(|line_index| {
                        scene.lines.get(*line_index).is_some_and(|line| {
                            !line.points.is_empty()
                                || line.binding.is_some()
                                || line.debug.is_some()
                        })
                    })
                    .map(|line_index| (line_index, function.clone()))
            })
            .collect(),
        ..Builder::default()
    };
    for (index, point) in scene.points.iter().enumerate() {
        if let Some(debug) = &point.debug {
            builder
                .group_objects
                .entry(debug.group_ordinal)
                .or_insert_with(|| point_id(index));
            builder
                .group_geometry_kinds
                .insert(debug.group_ordinal, GeometryNodeKind::Point);
        }
    }
    for (index, line) in scene.lines.iter().enumerate() {
        if let Some(debug) = &line.debug {
            builder
                .group_objects
                .entry(debug.group_ordinal)
                .or_insert_with(|| line_id(index));
            builder
                .group_geometry_kinds
                .insert(debug.group_ordinal, GeometryNodeKind::Line);
        }
    }
    for (index, circle) in scene.circles.iter().enumerate() {
        if let Some(debug) = &circle.debug {
            builder
                .group_objects
                .entry(debug.group_ordinal)
                .or_insert_with(|| circle_id(index));
            builder
                .group_geometry_kinds
                .insert(debug.group_ordinal, GeometryNodeKind::Circle);
        }
    }
    for (index, arc) in scene.arcs.iter().enumerate() {
        if let Some(debug) = &arc.debug {
            builder
                .group_objects
                .entry(debug.group_ordinal)
                .or_insert_with(|| arc_id(index));
            builder
                .group_geometry_kinds
                .insert(debug.group_ordinal, GeometryNodeKind::Arc);
        }
    }
    for (index, polygon) in scene.polygons.iter().enumerate() {
        if let Some(debug) = &polygon.debug {
            builder
                .group_objects
                .entry(debug.group_ordinal)
                .or_insert_with(|| polygon_id(index));
            builder
                .group_geometry_kinds
                .insert(debug.group_ordinal, GeometryNodeKind::Polygon);
        }
    }
    for parameter in &scene.parameters {
        if builder.named_scalars.contains_key(&parameter.name) {
            continue;
        }
        let id = format!("parameter:{}", parameter.name);
        builder.bound_source(
            id.clone(),
            ObjectValue::Scalar {
                value: parameter.value,
            },
            SceneObjectSourceBinding::Parameter {
                name: parameter.name.clone(),
            },
        );
        builder.named_scalars.insert(parameter.name.clone(), id);
    }
    for scalar in &scene.scalars {
        builder
            .group_scalars
            .insert(scalar.group_ordinal, group_scalar_id(scalar.group_ordinal));
    }
    let mut label_indices = (0..scene.labels.len()).collect::<Vec<_>>();
    label_indices.sort_by_key(|index| {
        scene.labels[*index]
            .debug
            .as_ref()
            .map(|debug| debug.group_ordinal)
            .unwrap_or(usize::MAX)
    });
    for &index in &label_indices {
        let label = &scene.labels[index];
        builder.register_label_scalar(index, label.binding.as_ref());
        if let Some(debug) = &label.debug {
            if Builder::label_binding_is_scalar(label.binding.as_ref()) {
                builder
                    .group_scalars
                    .insert(debug.group_ordinal, label_scalar_id(index));
            } else if let Some(TextLabelBinding::ParameterValue { name }) = &label.binding
                && let Some(parameter_id) = builder.named_scalars.get(name).cloned()
            {
                builder
                    .group_scalars
                    .insert(debug.group_ordinal, parameter_id);
            }
        }
    }
    for index in label_indices {
        let label = &scene.labels[index];
        builder.label_scalar(index, label.binding.as_ref());
    }
    for (group_ordinal, id) in builder.group_scalars.clone() {
        builder.group_objects.entry(group_ordinal).or_insert(id);
    }
    for (index, point) in scene.points.iter().enumerate() {
        builder.point(index, point);
    }
    for (index, family) in scene.point_iterations.iter().enumerate() {
        builder.point_iteration(index, family);
    }
    for scalar in &scene.scalars {
        let id = group_scalar_id(scalar.group_ordinal);
        if builder.group_scalars.get(&scalar.group_ordinal) == Some(&id) {
            builder.scene_scalar(id, scalar);
        }
    }
    let supported_line_iterations = scene.line_iterations.iter().all(|family| match family {
        LineIterationFamily::Translate { .. } | LineIterationFamily::Rotate { .. } => true,
        LineIterationFamily::Affine { .. } => true,
        LineIterationFamily::Branching { .. }
        | LineIterationFamily::ParameterizedPointTrace { .. } => false,
    });
    let generated_line_count = if supported_line_iterations && !scene.line_iterations.is_empty() {
        scene
            .lines
            .iter()
            .rev()
            .take_while(|line| line.binding.is_none() && line.debug.is_none())
            .count()
    } else {
        0
    };
    let base_line_count = scene.lines.len().saturating_sub(generated_line_count);
    let base_lines = scene.lines.iter().take(base_line_count).enumerate();
    for (index, line) in base_lines.clone().filter(|(_, line)| !is_trace_line(line)) {
        builder.line(index, line);
    }
    if supported_line_iterations {
        for (index, family) in scene.line_iterations.iter().enumerate() {
            builder.line_iteration(index, family);
        }
    }
    let generated_circle_count = scene
        .circle_iterations
        .iter()
        .map(|family| family.depth)
        .sum::<usize>();
    let base_circle_count = scene.circles.len().saturating_sub(generated_circle_count);
    for (index, circle) in scene.circles.iter().take(base_circle_count).enumerate() {
        let id = circle_id(index);
        match &circle.binding {
            Some(ShapeBinding::PointRadiusCircle {
                center_index,
                radius_index,
            }) => builder.derived(
                id,
                ObjectOp::CircleByPoints,
                [point_id(*center_index), point_id(*radius_index)],
            ),
            Some(ShapeBinding::SegmentRadiusCircle {
                center_index,
                line_start_index,
                line_end_index,
            }) => builder.derived(
                id,
                ObjectOp::CircleBySegmentRadius,
                [
                    point_id(*center_index),
                    point_id(*line_start_index),
                    point_id(*line_end_index),
                ],
            ),
            Some(ShapeBinding::ParameterRadiusCircle {
                center_index,
                parameter_name,
                raw_per_unit,
            }) => {
                if !builder.parameter_radius_circle(
                    id.clone(),
                    *center_index,
                    parameter_name,
                    *raw_per_unit,
                ) {
                    builder.pending_source(
                        id,
                        "circle-binding",
                        ObjectValue::Circle {
                            center: core_point(&circle.center),
                            radius_point: core_point(&circle.radius_point),
                        },
                    );
                }
            }
            Some(ShapeBinding::ExpressionRadiusCircle {
                center_index,
                expr,
                parameter_group_ordinals,
            }) => {
                builder.expression_radius_circle(id, *center_index, expr, parameter_group_ordinals);
            }
            Some(ShapeBinding::DerivedTransform {
                source_index,
                transform,
            }) => {
                if !builder.geometry_transform(id.clone(), circle_id(*source_index), transform) {
                    builder.pending_source(
                        id,
                        "circle-binding",
                        ObjectValue::Circle {
                            center: core_point(&circle.center),
                            radius_point: core_point(&circle.radius_point),
                        },
                    );
                }
            }
            None => builder.bound_source(
                id,
                ObjectValue::Circle {
                    center: core_point(&circle.center),
                    radius_point: core_point(&circle.radius_point),
                },
                SceneObjectSourceBinding::Circle {
                    circle_index: index,
                },
            ),
            Some(_) => {
                builder.pending_source(
                    id,
                    "circle-binding",
                    ObjectValue::Circle {
                        center: core_point(&circle.center),
                        radius_point: core_point(&circle.radius_point),
                    },
                );
            }
        }
        if let Some(binding) = &circle.fill_color_binding
            && !builder.color_binding(circle_fill_color_id(index), binding)
        {
            builder.pending_source(
                circle_fill_color_id(index),
                "color-operations",
                ObjectValue::Color {
                    color: circle.fill_color.unwrap_or([0, 0, 0, 0]),
                },
            );
        }
    }
    for (index, family) in scene.circle_iterations.iter().enumerate() {
        builder.circle_iteration(index, family);
    }
    let supported_polygon_iterations = scene.polygon_iterations.iter().all(|family| {
        matches!(
            family,
            PolygonIterationFamily::Similarity { .. } | PolygonIterationFamily::Translate { .. }
        )
    });
    let generated_polygon_count =
        if supported_polygon_iterations && !scene.polygon_iterations.is_empty() {
            scene
                .polygons
                .iter()
                .rev()
                .take_while(|polygon| polygon.binding.is_none() && polygon.debug.is_none())
                .count()
        } else {
            0
        };
    let base_polygon_count = scene.polygons.len().saturating_sub(generated_polygon_count);
    for (index, polygon) in scene.polygons.iter().take(base_polygon_count).enumerate() {
        let id = polygon_id(index);
        match &polygon.binding {
            Some(ShapeBinding::PointPolygon { vertex_indices }) => builder.derived(
                id,
                ObjectOp::Polygon,
                vertex_indices.iter().copied().map(point_id),
            ),
            Some(ShapeBinding::ArcBoundaryPolygon {
                boundary_kind,
                center_index,
                start_index,
                mid_index,
                end_index,
                reversed,
                complement,
                ..
            }) => {
                if !builder.arc_boundary_points(
                    id.clone(),
                    *boundary_kind,
                    *center_index,
                    *start_index,
                    *mid_index,
                    *end_index,
                    *reversed,
                    *complement,
                ) {
                    builder.pending_source(
                        id,
                        "polygon-binding",
                        ObjectValue::Points {
                            points: polygon.points.iter().map(core_point).collect(),
                        },
                    );
                }
            }
            Some(ShapeBinding::DerivedTransform {
                source_index,
                transform,
            }) => {
                if !builder.geometry_transform(id.clone(), polygon_id(*source_index), transform) {
                    builder.pending_source(
                        id,
                        "polygon-binding",
                        ObjectValue::Points {
                            points: polygon.points.iter().map(core_point).collect(),
                        },
                    );
                }
            }
            None => builder.bound_source(
                id,
                ObjectValue::Points {
                    points: polygon.points.iter().map(core_point).collect(),
                },
                SceneObjectSourceBinding::Polygon {
                    polygon_index: index,
                },
            ),
            Some(_) => builder.pending_source(
                id,
                "polygon-binding",
                ObjectValue::Points {
                    points: polygon.points.iter().map(core_point).collect(),
                },
            ),
        }
        if let Some(binding) = &polygon.color_binding
            && !builder.color_binding(polygon_color_id(index), binding)
        {
            builder.pending_source(
                polygon_color_id(index),
                "color-operations",
                ObjectValue::Color {
                    color: polygon.color,
                },
            );
        }
    }
    if supported_polygon_iterations {
        for (index, family) in scene.polygon_iterations.iter().enumerate() {
            match family {
                PolygonIterationFamily::Similarity { .. } => {
                    builder.similarity_polygon_iteration(index, family)
                }
                PolygonIterationFamily::Translate { .. } => {
                    builder.translate_polygon_iteration(index, family)
                }
                PolygonIterationFamily::CoordinateGrid { .. } => {}
            }
        }
    }
    for (index, arc) in scene.arcs.iter().enumerate() {
        let id = arc_id(index);
        match &arc.binding {
            Some(ArcBinding::CenterArc {
                center_index,
                start_index,
                end_index,
            }) => builder.derived(
                id,
                ObjectOp::CenterArc { y_up: scene.y_up },
                [
                    point_id(*center_index),
                    point_id(*start_index),
                    point_id(*end_index),
                ],
            ),
            Some(ArcBinding::CircleArc {
                circle_index,
                start_index,
                end_index,
            }) => builder.derived(
                id,
                ObjectOp::CircleArc { y_up: scene.y_up },
                [
                    circle_id(*circle_index),
                    point_id(*start_index),
                    point_id(*end_index),
                ],
            ),
            Some(ArcBinding::ThreePointArc {
                start_index,
                mid_index,
                end_index,
            }) => builder.derived(
                id,
                ObjectOp::ThreePointArc { complement: false },
                [
                    point_id(*start_index),
                    point_id(*mid_index),
                    point_id(*end_index),
                ],
            ),
            Some(ArcBinding::DerivedTransform {
                source_index,
                transform,
            }) => {
                if !builder.geometry_transform(id.clone(), arc_id(*source_index), transform) {
                    builder.pending_source(
                        id,
                        "arc-binding",
                        ObjectValue::Arc {
                            start: core_point(&arc.points[0]),
                            mid: core_point(&arc.points[1]),
                            end: core_point(&arc.points[2]),
                            center: arc.center.as_ref().map(core_point),
                            counterclockwise: arc.counterclockwise,
                            complement: false,
                        },
                    );
                }
            }
            None => builder.pending_source(
                id,
                "arc-binding",
                ObjectValue::Arc {
                    start: core_point(&arc.points[0]),
                    mid: core_point(&arc.points[1]),
                    end: core_point(&arc.points[2]),
                    center: arc.center.as_ref().map(core_point),
                    counterclockwise: arc.counterclockwise,
                    complement: false,
                },
            ),
        }
    }
    // A trace embeds the complete operation program of its target point. Build every
    // non-trace geometry node first so a point constrained to a circle, arc, or polygon
    // can retain that exact parent in the embedded program.
    for (index, line) in base_lines.clone().filter(|(_, line)| {
        is_trace_line(line) && !is_segment_trace_line(line) && !is_custom_transform_trace_line(line)
    }) {
        builder.line(index, line);
    }
    for (index, line) in base_lines
        .clone()
        .filter(|(_, line)| is_custom_transform_trace_line(line))
    {
        builder.line(index, line);
    }
    for (index, line) in base_lines.filter(|(_, line)| is_segment_trace_line(line)) {
        builder.line(index, line);
    }
    if !supported_line_iterations || !supported_polygon_iterations {
        builder.pending("iteration-operations");
    }
    if scene
        .functions
        .iter()
        .any(|function| function.line_index.is_none())
    {
        builder.pending("function-operations");
    }
    if let Err(error) = ObjectGraph::build(builder.nodes.clone()) {
        builder
            .pending_operations
            .push(format!("graph-validation:{error}"));
    }
    SceneObjectGraph {
        geometry_complete: builder.pending_operations.is_empty(),
        nodes: builder.nodes,
        sources: builder.sources,
        pending_operations: builder.pending_operations,
    }
}

#[derive(Default)]
struct Builder {
    nodes: Vec<ObjectNode<ObjectOp>>,
    sources: Vec<SceneObjectSource>,
    pending_operations: Vec<String>,
    named_scalars: BTreeMap<String, String>,
    group_scalars: BTreeMap<usize, String>,
    group_objects: BTreeMap<usize, String>,
    group_geometry_kinds: BTreeMap<usize, GeometryNodeKind>,
    payload_dependencies: BTreeMap<usize, Vec<usize>>,
    point_bindings: Vec<Option<ScenePointBinding>>,
    point_constraints: Vec<ScenePointConstraint>,
    line_group_ordinals: Vec<Option<usize>>,
    function_lines: BTreeMap<usize, crate::runtime::scene::SceneFunction>,
    y_up: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GeometryNodeKind {
    Point,
    Line,
    Circle,
    Arc,
    Polygon,
}

impl Builder {
    fn payload_group_object_id(
        &mut self,
        group_ordinal: usize,
        visiting: &mut std::collections::BTreeSet<usize>,
    ) -> Option<String> {
        if let Some(id) = self.group_objects.get(&group_ordinal) {
            return Some(id.clone());
        }
        if !visiting.insert(group_ordinal) {
            return None;
        }
        let parent_ordinals = self.payload_dependencies.get(&group_ordinal)?.clone();
        if parent_ordinals.is_empty() {
            visiting.remove(&group_ordinal);
            return None;
        }
        let parents = parent_ordinals
            .into_iter()
            .map(|ordinal| self.payload_group_object_id(ordinal, visiting))
            .collect::<Option<Vec<_>>>();
        visiting.remove(&group_ordinal);
        let parents = parents?;
        let id = format!("payload-group:{group_ordinal}");
        self.derived(id.clone(), ObjectOp::SelectParent { index: 0 }, parents);
        self.group_objects.insert(group_ordinal, id.clone());
        Some(id)
    }

    fn scene_scalar(&mut self, id: String, scalar: &SceneScalar) {
        match scalar.binding {
            SceneScalarBinding::CircularMeasure {
                circle_index,
                value_scale,
            } => {
                if value_scale == 1.0 {
                    self.derived(id, ObjectOp::CircularRadius, [circle_id(circle_index)]);
                } else {
                    let radius_id = format!("{id}:radius");
                    self.derived(
                        radius_id.clone(),
                        ObjectOp::CircularRadius,
                        [circle_id(circle_index)],
                    );
                    self.derived(
                        id,
                        ObjectOp::ScaleScalar {
                            factor: value_scale,
                        },
                        [radius_id],
                    );
                }
            }
            SceneScalarBinding::ArcAngle { arc_index } => {
                self.derived(id, ObjectOp::ArcAngleDegrees, [arc_id(arc_index)]);
            }
            SceneScalarBinding::ArcLength { arc_index } => {
                self.derived(id, ObjectOp::ArcLength, [arc_id(arc_index)]);
            }
            SceneScalarBinding::PolarAngle {
                point_index,
                center_index,
                reference_index,
            } => self.derived(
                id,
                ObjectOp::MeasuredRotationDegrees,
                [
                    point_id(point_index),
                    point_id(center_index),
                    point_id(reference_index),
                ],
            ),
            SceneScalarBinding::Alias {
                source_group_ordinal,
            } => {
                if let Some(parent) = self.group_scalars.get(&source_group_ordinal).cloned() {
                    self.derived(id, ObjectOp::Copy, [parent]);
                } else {
                    self.pending(&format!("scalar-alias-group-{source_group_ordinal}"));
                }
            }
            SceneScalarBinding::PointParameter { point_index } => {
                if !self.point_parameter(id.clone(), point_index) {
                    self.pending_source(id, "scalar-binding", ObjectValue::Scalar { value: 0.0 });
                }
            }
            SceneScalarBinding::PointLineParameter {
                point_index,
                start_index,
                end_index,
                line_kind,
            } => self.point_line_parameter(
                id,
                point_index,
                start_index,
                end_index,
                core_line_kind(line_kind),
            ),
            SceneScalarBinding::PointCircleParameter {
                point_index,
                center_index,
                radius_index,
            } => {
                let circle_id = format!("domain:{id}:circle");
                self.derived(
                    circle_id.clone(),
                    ObjectOp::CircleByPoints,
                    [point_id(center_index), point_id(radius_index)],
                );
                self.derived(
                    id,
                    ObjectOp::CircleParameter {
                        invert_y: !self.y_up,
                    },
                    [point_id(point_index), circle_id],
                );
            }
            SceneScalarBinding::PointArcParameter {
                point_index,
                arc_index,
            } => self.derived(
                id,
                ObjectOp::ArcParameterFromPoint,
                [arc_id(arc_index), point_id(point_index)],
            ),
            SceneScalarBinding::PointPolylineParameter {
                point_index,
                line_index,
            } => self.derived(
                id,
                ObjectOp::PolylineParameterFromPoint,
                [line_id(line_index), point_id(point_index)],
            ),
            SceneScalarBinding::PointPolygonParameter {
                point_index,
                ref vertex_indices,
            } => self.derived(
                id,
                ObjectOp::PolygonBoundaryParameterFromPoint,
                vertex_indices
                    .iter()
                    .copied()
                    .map(point_id)
                    .chain(std::iter::once(point_id(point_index))),
            ),
        }
    }

    fn label_binding_is_scalar(binding: Option<&TextLabelBinding>) -> bool {
        matches!(
            binding,
            Some(
                TextLabelBinding::PointDistanceValue { .. }
                    | TextLabelBinding::ScalarAlias { .. }
                    | TextLabelBinding::PointAngleValue { .. }
                    | TextLabelBinding::PolygonAreaValue { .. }
                    | TextLabelBinding::PointDistanceRatioValue { .. }
                    | TextLabelBinding::PointAxisValue { .. }
                    | TextLabelBinding::LineProjectionParameter { .. }
                    | TextLabelBinding::ExpressionValue { .. }
                    | TextLabelBinding::PointBoundExpressionValue { .. }
            )
        )
    }

    fn source(&mut self, id: String, value: ObjectValue) {
        self.bound_source(id, value, SceneObjectSourceBinding::Initial);
    }

    fn bound_source(&mut self, id: String, value: ObjectValue, binding: SceneObjectSourceBinding) {
        self.nodes.push(ObjectNode::source(id.clone()));
        self.sources.push(SceneObjectSource { id, value, binding });
    }

    fn point_control_source(
        &mut self,
        id: String,
        value: f64,
        point_index: usize,
        control: ScenePointControl,
    ) {
        self.bound_source(
            id,
            ObjectValue::Scalar { value },
            SceneObjectSourceBinding::PointControl {
                point_index,
                control,
            },
        );
    }

    fn derived(&mut self, id: String, op: ObjectOp, parents: impl IntoIterator<Item = String>) {
        self.nodes.push(ObjectNode::derived(id, op, parents));
    }

    fn point_line_parameter(
        &mut self,
        id: String,
        point_index: usize,
        start_index: usize,
        end_index: usize,
        line_kind: LineKind,
    ) {
        let line_id = format!("domain:{id}:line");
        self.derived(
            line_id.clone(),
            ObjectOp::Line { line_kind },
            [point_id(start_index), point_id(end_index)],
        );
        self.derived(
            id,
            ObjectOp::PointLineParameter,
            [point_id(point_index), line_id],
        );
    }

    fn pending_source(&mut self, id: String, operation: &str, value: ObjectValue) {
        self.pending_operations.push(format!("{id}:{operation}"));
        self.source(id, value);
    }

    fn pending(&mut self, operation: &str) {
        self.pending_operations.push(operation.into());
    }

    fn register_label_scalar(&mut self, index: usize, binding: Option<&TextLabelBinding>) {
        let Some(binding) = binding else { return };
        let id = label_scalar_id(index);
        let mut register = |name: &str| {
            if !name.is_empty() {
                self.named_scalars.insert(name.to_string(), id.clone());
            }
        };
        match binding {
            TextLabelBinding::ScalarAlias { name, .. } => register(name),
            TextLabelBinding::PointDistanceValue { name, .. }
            | TextLabelBinding::PointAngleValue { name, .. }
            | TextLabelBinding::PolygonAreaValue { name, .. }
            | TextLabelBinding::PointDistanceRatioValue { name, .. }
            | TextLabelBinding::PointAxisValue { name, .. } => register(name),
            TextLabelBinding::LineProjectionParameter { point_name, .. } => register(point_name),
            TextLabelBinding::ExpressionValue {
                result_name,
                expr_label,
                expr,
                ..
            }
            | TextLabelBinding::PointBoundExpressionValue {
                result_name,
                expr_label,
                expr,
                ..
            } => {
                if let Some(result_name) = result_name {
                    register(result_name);
                }
                register(expr_label);
                register(&function_expr_label(expr.clone()));
            }
            _ => {}
        }
    }

    fn label_scalar(&mut self, index: usize, binding: Option<&TextLabelBinding>) {
        let Some(binding) = binding else { return };
        let id = label_scalar_id(index);
        match binding {
            TextLabelBinding::ScalarAlias {
                source_group_ordinal,
                ..
            } => {
                if let Some(parent) = self.group_scalars.get(source_group_ordinal).cloned() {
                    self.derived(id, ObjectOp::Copy, [parent]);
                } else {
                    self.pending(&format!("scalar-alias-group-{source_group_ordinal}"));
                }
            }
            TextLabelBinding::PointDistanceValue {
                left_index,
                right_index,
                value_scale,
                ..
            } => self.derived(
                id,
                ObjectOp::PointDistance {
                    value_scale: *value_scale,
                },
                [point_id(*left_index), point_id(*right_index)],
            ),
            TextLabelBinding::PointAngleValue {
                start_index,
                vertex_index,
                end_index,
                ..
            } => self.derived(
                id,
                ObjectOp::PointAngleDegrees,
                [
                    point_id(*start_index),
                    point_id(*vertex_index),
                    point_id(*end_index),
                ],
            ),
            TextLabelBinding::PolygonAreaValue {
                point_indices,
                value_scale,
                ..
            } => self.derived(
                id,
                ObjectOp::PolygonArea {
                    value_scale: *value_scale,
                },
                point_indices.iter().copied().map(point_id),
            ),
            TextLabelBinding::PointDistanceRatioValue {
                origin_index,
                denominator_index,
                numerator_index,
                clamp_to_unit,
                ..
            } => self.derived(
                id,
                ObjectOp::PointDistanceRatio {
                    clamp_to_unit: *clamp_to_unit,
                },
                [
                    point_id(*origin_index),
                    point_id(*denominator_index),
                    point_id(*numerator_index),
                ],
            ),
            TextLabelBinding::PointAxisValue {
                point_index, axis, ..
            } => self.derived(
                id,
                ObjectOp::PointCoordinate {
                    vertical: matches!(axis, CoordinateAxis::Vertical),
                },
                [point_id(*point_index)],
            ),
            TextLabelBinding::LineProjectionParameter {
                point_index,
                start_index,
                end_index,
                line_kind,
                ..
            } => self.point_line_parameter(
                id,
                *point_index,
                *start_index,
                *end_index,
                core_line_kind(*line_kind),
            ),
            TextLabelBinding::ExpressionValue {
                expr,
                parameter_group_ordinals,
                ..
            }
            | TextLabelBinding::PointBoundExpressionValue {
                expr,
                parameter_group_ordinals,
                ..
            } => {
                if let Some((name, default)) = identity_parameter(expr) {
                    let exact_parent = parameter_group_ordinals
                        .get(name)
                        .and_then(|ordinal| self.group_scalars.get(ordinal));
                    if exact_parent.is_none_or(|parent| parent == &id) {
                        self.source(id, ObjectValue::Scalar { value: default });
                        return;
                    }
                }
                self.expression_with_group_sources(id, expr, parameter_group_ordinals);
            }
            _ => {}
        }
    }

    fn expression(&mut self, id: String, expression: &gsp_runtime_core::FunctionExpr) {
        self.expression_with_group_sources(id, expression, &BTreeMap::new());
    }

    fn expression_with_group_sources(
        &mut self,
        id: String,
        expression: &gsp_runtime_core::FunctionExpr,
        parameter_group_ordinals: &BTreeMap<String, usize>,
    ) {
        let (parameter_names, parents): (Vec<_>, Vec<_>) = expression_parameter_names(expression)
            .into_iter()
            .filter_map(|name| {
                parameter_group_ordinals
                    .get(&name)
                    .and_then(|ordinal| self.group_scalars.get(ordinal))
                    .or_else(|| self.named_scalars.get(&name))
                    .cloned()
                    .map(|parent| (name, parent))
            })
            .unzip();
        self.derived(
            id,
            ObjectOp::EvaluateExpression {
                expression: ObjectExpression::from_function_expr(expression),
                parameter_names,
                x: 0.0,
            },
            parents,
        );
    }

    fn parameter_radius_circle(
        &mut self,
        id: String,
        center_index: usize,
        parameter_name: &str,
        raw_per_unit: f64,
    ) -> bool {
        let Some(parameter_id) = self.named_scalars.get(parameter_name).cloned() else {
            return false;
        };
        let radius_id = format!("scalar:{id}:radius");
        self.derived(
            radius_id.clone(),
            ObjectOp::ScaleScalar {
                factor: raw_per_unit,
            },
            [parameter_id],
        );
        self.derived(
            id,
            ObjectOp::CircleByRadius,
            [point_id(center_index), radius_id],
        );
        true
    }

    fn expression_radius_circle(
        &mut self,
        id: String,
        center_index: usize,
        expression: &gsp_runtime_core::FunctionExpr,
        parameter_group_ordinals: &BTreeMap<String, usize>,
    ) {
        let radius_id = format!("scalar:{id}:radius");
        self.expression_with_group_sources(radius_id.clone(), expression, parameter_group_ordinals);
        self.derived(
            id,
            ObjectOp::CircleByRadius,
            [point_id(center_index), radius_id],
        );
    }

    fn circle_iteration(&mut self, index: usize, family: &CircleIterationFamily) {
        if family.vertex_indices.len() < 2 {
            self.pending("iteration-operations");
            return;
        }
        let id = format!("circle-iteration:{index}");
        let seed_id = format!("scalar:{id}:seed");
        let next_id = format!("scalar:{id}:next");
        if !self.point_parameter(seed_id.clone(), family.source_center_index)
            || !self.point_parameter(next_id.clone(), family.source_next_center_index)
        {
            self.pending("iteration-operations");
            return;
        }
        let Some(depth_id) = self.iteration_depth_scalar(
            &format!("scalar:{id}:depth"),
            family.depth,
            family.depth_parameter_name.as_deref(),
        ) else {
            self.pending("iteration-operations");
            return;
        };
        let mut parents = Vec::with_capacity(family.vertex_indices.len() + 4);
        parents.push(circle_id(family.source_circle_index));
        parents.extend(family.vertex_indices.iter().copied().map(point_id));
        parents.extend([seed_id, next_id, depth_id]);
        self.derived(
            id,
            ObjectOp::CircleIteration {
                vertex_count: family.vertex_indices.len(),
            },
            parents,
        );
    }

    fn point_iteration(&mut self, index: usize, family: &PointIterationFamily) {
        let id = format!("point-iteration:{index}");
        match family {
            PointIterationFamily::Interpreted {
                point_index,
                states,
                depth_parameter_name,
                depth,
            } => {
                let Some(depth_id) = self.iteration_depth_scalar(
                    &format!("scalar:{id}:depth"),
                    *depth,
                    depth_parameter_name.as_deref(),
                ) else {
                    self.pending("iteration-operations");
                    return;
                };
                let resolve_state = |state: &crate::runtime::scene::IterationStatePair,
                                     source: bool|
                 -> Option<String> {
                    let ordinal = if source {
                        state.source_group_ordinal
                    } else {
                        state.image_group_ordinal
                    };
                    match state.kind {
                        crate::runtime::scene::IterationStateKind::Object => {
                            self.group_objects.get(&ordinal).cloned()
                        }
                        crate::runtime::scene::IterationStateKind::Scalar => {
                            self.group_scalars.get(&ordinal).cloned()
                        }
                    }
                };
                let Some(state_source_ids) = states
                    .iter()
                    .map(|state| resolve_state(state, true))
                    .collect::<Option<Vec<_>>>()
                else {
                    self.pending("iteration-state-sources");
                    return;
                };
                let Some(state_target_ids) = states
                    .iter()
                    .map(|state| resolve_state(state, false))
                    .collect::<Option<Vec<_>>>()
                else {
                    self.pending("iteration-state-targets");
                    return;
                };
                let output_id = point_id(*point_index);
                let Some(program) =
                    self.object_iteration_program(&output_id, &state_source_ids, &state_target_ids)
                else {
                    return;
                };
                let mut parents = program.source_ids.clone();
                parents.extend(state_target_ids);
                parents.push(depth_id);
                self.derived(id, ObjectOp::PointIteration { program }, parents);
            }
        }
    }

    fn iteration_depth_scalar(
        &mut self,
        id: &str,
        depth: usize,
        parameter_name: Option<&str>,
    ) -> Option<String> {
        let id = id.to_string();
        if let Some(parameter_name) = parameter_name {
            let parent = self.named_scalars.get(parameter_name)?.clone();
            self.derived(id.clone(), ObjectOp::Copy, [parent]);
        } else {
            self.source(
                id.clone(),
                ObjectValue::Scalar {
                    value: depth as f64,
                },
            );
        }
        Some(id)
    }

    fn line_iteration(&mut self, index: usize, family: &LineIterationFamily) {
        let id = format!("line-iteration:{index}");
        match family {
            LineIterationFamily::Translate {
                start_index,
                end_index,
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
                ..
            } => {
                let Some(depth_id) = self.iteration_depth_expression_scalar(
                    &format!("scalar:{id}:depth"),
                    *depth,
                    depth_expr.as_ref(),
                    parameter_name.as_deref(),
                ) else {
                    self.pending("iteration-operations");
                    return;
                };
                let vector_from_parents =
                    vector_start_index.is_some() && vector_end_index.is_some();
                let mut parents = vec![point_id(*start_index), point_id(*end_index), depth_id];
                if let (Some(start), Some(end)) = (vector_start_index, vector_end_index) {
                    parents.extend([point_id(*start), point_id(*end)]);
                }
                self.derived(
                    id,
                    ObjectOp::LineTranslateIteration {
                        dx: *dx,
                        dy: *dy,
                        secondary_dx: *secondary_dx,
                        secondary_dy: *secondary_dy,
                        bidirectional: *bidirectional,
                        vector_from_parents,
                    },
                    parents,
                );
            }
            LineIterationFamily::Rotate {
                source_index,
                center_index,
                angle_expr,
                depth,
                parameter_name,
                depth_parameter_name,
                ..
            } => {
                let angle_id = format!("scalar:{id}:angle");
                self.expression(angle_id.clone(), angle_expr);
                let Some(depth_id) = self.iteration_depth_expression_scalar(
                    &format!("scalar:{id}:depth"),
                    *depth,
                    None,
                    depth_parameter_name
                        .as_deref()
                        .or(parameter_name.as_deref()),
                ) else {
                    self.pending("iteration-operations");
                    return;
                };
                self.derived(
                    id,
                    ObjectOp::LineRotateIteration,
                    [
                        line_id(*source_index),
                        point_id(*center_index),
                        angle_id,
                        depth_id,
                    ],
                );
            }
            LineIterationFamily::Affine {
                start_index,
                end_index,
                source_triangle_indices,
                target_triangle,
                depth,
                ..
            } => {
                let depth_id = format!("scalar:{id}:depth");
                self.source(
                    depth_id.clone(),
                    ObjectValue::Scalar {
                        value: *depth as f64,
                    },
                );
                let mut target_handles = Vec::with_capacity(3);
                let mut target_parents = Vec::with_capacity(3);
                for handle in target_triangle {
                    match handle {
                        IterationPointHandle::Point { point_index } => {
                            target_handles.push(AffineTargetHandle::ParentPoint);
                            target_parents.push(point_id(*point_index));
                        }
                        IterationPointHandle::LinePoint {
                            line_index,
                            segment_index,
                            t,
                        } => {
                            target_handles.push(AffineTargetHandle::ParentLinePoint {
                                segment_index: *segment_index,
                                t: *t,
                            });
                            target_parents.push(line_id(*line_index));
                        }
                        IterationPointHandle::Fixed(point) => {
                            target_handles.push(AffineTargetHandle::Fixed {
                                point: core_point(point),
                            });
                        }
                    }
                }
                let target_handles: [AffineTargetHandle; 3] = target_handles
                    .try_into()
                    .expect("affine target triangle always has three handles");
                let mut parents = vec![
                    point_id(*start_index),
                    point_id(*end_index),
                    point_id(source_triangle_indices[0]),
                    point_id(source_triangle_indices[1]),
                    point_id(source_triangle_indices[2]),
                ];
                parents.extend(target_parents);
                parents.push(depth_id);
                self.derived(
                    id,
                    ObjectOp::LineAffineIteration { target_handles },
                    parents,
                );
            }
            LineIterationFamily::Branching { .. }
            | LineIterationFamily::ParameterizedPointTrace { .. } => {
                self.pending("iteration-operations");
            }
        }
    }

    fn iteration_depth_expression_scalar(
        &mut self,
        id: &str,
        depth: usize,
        expression: Option<&gsp_runtime_core::FunctionExpr>,
        parameter_name: Option<&str>,
    ) -> Option<String> {
        let id = id.to_string();
        if let Some(expression) = expression {
            self.expression(id.clone(), expression);
        } else if let Some(parameter_name) = parameter_name {
            let parent = self.named_scalars.get(parameter_name)?.clone();
            self.derived(id.clone(), ObjectOp::Copy, [parent]);
        } else {
            self.source(
                id.clone(),
                ObjectValue::Scalar {
                    value: depth as f64,
                },
            );
        }
        Some(id)
    }

    fn similarity_polygon_iteration(&mut self, index: usize, family: &PolygonIterationFamily) {
        let PolygonIterationFamily::Similarity {
            source_index,
            source_start_index,
            source_end_index,
            target_start_index,
            target_end_index,
            depth,
            depth_expr,
            inverse,
            ..
        } = family
        else {
            return;
        };
        let id = format!("polygon-iteration:{index}");
        let depth_id = format!("scalar:{id}:depth");
        if let Some(expression) = depth_expr {
            self.expression(depth_id.clone(), expression);
        } else {
            self.source(
                depth_id.clone(),
                ObjectValue::Scalar {
                    value: *depth as f64,
                },
            );
        }
        self.derived(
            id,
            ObjectOp::SimilarityPolygonIteration { inverse: *inverse },
            [
                polygon_id(*source_index),
                point_id(*source_start_index),
                point_id(*source_end_index),
                point_id(*target_start_index),
                point_id(*target_end_index),
                depth_id,
            ],
        );
    }

    fn translate_polygon_iteration(&mut self, index: usize, family: &PolygonIterationFamily) {
        let PolygonIterationFamily::Translate {
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
            ..
        } = family
        else {
            return;
        };
        if vertex_indices.len() < 2 {
            self.pending("iteration-operations");
            return;
        }
        let id = format!("polygon-iteration:{index}");
        let Some(depth_id) = self.iteration_depth_expression_scalar(
            &format!("scalar:{id}:depth"),
            *depth,
            depth_expr.as_ref(),
            parameter_name.as_deref(),
        ) else {
            self.pending("iteration-operations");
            return;
        };
        let vector_from_parents = vector_start_index.is_some() && vector_end_index.is_some();
        let mut parents = vertex_indices
            .iter()
            .copied()
            .map(point_id)
            .collect::<Vec<_>>();
        parents.push(depth_id);
        if let (Some(start), Some(end)) = (vector_start_index, vector_end_index) {
            parents.extend([point_id(*start), point_id(*end)]);
        }
        self.derived(
            id,
            ObjectOp::TranslatePolygonIteration {
                vertex_count: vertex_indices.len(),
                dx: *dx,
                dy: *dy,
                secondary_dx: *secondary_dx,
                secondary_dy: *secondary_dy,
                bidirectional: *bidirectional,
                vector_from_parents,
            },
            parents,
        );
    }

    fn point(&mut self, index: usize, point: &crate::runtime::scene::ScenePoint) {
        let id = point_id(index);
        let source_value = ObjectValue::point(core_point(&point.position));
        if let Some(binding) = &point.binding {
            match binding {
                ScenePointBinding::GraphCalibration | ScenePointBinding::Parameter { .. } => {
                    self.bound_source(
                        id,
                        source_value,
                        SceneObjectSourceBinding::Point { point_index: index },
                    );
                }
                ScenePointBinding::ProjectedCoordinate {
                    parent_group_ordinals,
                    source_parent,
                    ..
                } => {
                    let parents = parent_group_ordinals
                        .iter()
                        .map(|ordinal| {
                            self.payload_group_object_id(
                                *ordinal,
                                &mut std::collections::BTreeSet::new(),
                            )
                        })
                        .collect::<Option<Vec<_>>>();
                    if let Some(parents) = parents {
                        self.derived(
                            id,
                            ObjectOp::ProjectedCoordinatePoint {
                                source_parent: *source_parent,
                            },
                            parents,
                        );
                    } else {
                        self.pending_source(id, "projected-coordinate-parents", source_value);
                    }
                }
                ScenePointBinding::DerivedParameter {
                    source_index,
                    parameter_start_index,
                    parameter_end_index,
                } => {
                    if !self.derived_parameter_point(
                        id.clone(),
                        index,
                        *source_index,
                        *parameter_start_index,
                        *parameter_end_index,
                    ) {
                        self.pending_source(id, "point-binding", source_value);
                    }
                }
                ScenePointBinding::ConstraintParameterExpr { expr } => {
                    let parameter_id = format!("scalar:{id}:constraint-parameter");
                    self.expression(parameter_id.clone(), expr);
                    if !self.point_at_parameter(id.clone(), index, parameter_id) {
                        self.pending_source(id, "point-binding", source_value);
                    }
                }
                ScenePointBinding::ConstraintParameterPointDistanceRatio {
                    origin_index,
                    denominator_index,
                    numerator_index,
                    clamp_to_unit,
                } => {
                    let parameter_id = format!("scalar:{id}:constraint-parameter");
                    self.derived(
                        parameter_id.clone(),
                        ObjectOp::PointDistanceRatio {
                            clamp_to_unit: *clamp_to_unit,
                        },
                        [
                            point_id(*origin_index),
                            point_id(*denominator_index),
                            point_id(*numerator_index),
                        ],
                    );
                    if !self.point_at_parameter(id.clone(), index, parameter_id) {
                        self.pending_source(id, "point-binding", source_value);
                    }
                }
                ScenePointBinding::ConstraintParameterFromPointExpr {
                    source_index,
                    source_parameter_start_index,
                    source_parameter_end_index,
                    parameter_name,
                    expr,
                    absolute_value,
                    expression_sources,
                    expression_parameter_group_ordinals,
                } => {
                    if !expression_sources.is_empty() {
                        let parameter_id = format!("scalar:{id}:constraint-parameter");
                        let primary_parameter_override =
                            match (source_parameter_start_index, source_parameter_end_index) {
                                (Some(start_index), Some(end_index)) => {
                                    let primary_id = format!("{parameter_id}:primary");
                                    self.point_line_parameter(
                                        primary_id.clone(),
                                        *source_index,
                                        *start_index,
                                        *end_index,
                                        LineKind::Segment,
                                    );
                                    Some(primary_id)
                                }
                                _ => None,
                            };
                        if !self.expression_with_point_parameter_sources(
                            parameter_id.clone(),
                            expr,
                            expression_sources,
                            expression_parameter_group_ordinals,
                            *absolute_value,
                            primary_parameter_override,
                        ) || !self.point_at_parameter(id.clone(), index, parameter_id)
                        {
                            self.pending_source(id, "point-binding", source_value);
                        }
                        return;
                    }
                    let raw_id = format!("scalar:{id}:source-parameter");
                    let bound_id = if let Some(exact_id) = expression_parameter_group_ordinals
                        .get(parameter_name)
                        .and_then(|ordinal| self.group_scalars.get(ordinal))
                        .cloned()
                    {
                        exact_id
                    } else if let (Some(start_index), Some(end_index)) =
                        (source_parameter_start_index, source_parameter_end_index)
                    {
                        self.point_line_parameter(
                            raw_id.clone(),
                            *source_index,
                            *start_index,
                            *end_index,
                            LineKind::Segment,
                        );
                        raw_id
                    } else if self.point_parameter(raw_id.clone(), *source_index) {
                        raw_id
                    } else {
                        self.pending_source(id, "point-binding", source_value);
                        return;
                    };
                    let parameter_id = format!("scalar:{id}:constraint-parameter");
                    let expression_id = if *absolute_value {
                        parameter_id.clone()
                    } else {
                        format!("{parameter_id}:offset")
                    };
                    if !self.expression_with_bound_parameter(
                        expression_id.clone(),
                        expr,
                        parameter_name,
                        bound_id.clone(),
                    ) {
                        self.pending_source(id, "point-binding", source_value);
                        return;
                    }
                    if !absolute_value {
                        let sum_id = format!("{parameter_id}:sum");
                        self.scalar_sum(sum_id.clone(), bound_id, expression_id);
                        self.derived(parameter_id.clone(), ObjectOp::WrapUnitScalar, [sum_id]);
                    }
                    if !self.point_at_parameter(id.clone(), index, parameter_id) {
                        self.pending_source(id, "point-binding", source_value);
                    }
                }
                ScenePointBinding::Translate {
                    source_index,
                    vector_start_index,
                    vector_end_index,
                } => self.derived(
                    id,
                    ObjectOp::TransformObject {
                        transform: ObjectTransform::TranslateByVector,
                    },
                    [
                        point_id(*source_index),
                        point_id(*vector_start_index),
                        point_id(*vector_end_index),
                    ],
                ),
                ScenePointBinding::DirectedAngleAnchor {
                    first_start_index,
                    first_end_index,
                    second_start_index,
                    second_end_index,
                    distance,
                    parameter,
                } => self.derived(
                    id,
                    ObjectOp::DirectedAngleAnchor {
                        distance: *distance,
                        parameter: *parameter,
                    },
                    [
                        point_id(*first_start_index),
                        point_id(*first_end_index),
                        point_id(*second_start_index),
                        point_id(*second_end_index),
                    ],
                ),
                ScenePointBinding::Reflect {
                    source_index,
                    line_start_index,
                    line_end_index,
                } => {
                    let line_id = format!("domain:{id}:reflection-axis");
                    self.derived(
                        line_id.clone(),
                        ObjectOp::Line {
                            line_kind: LineKind::Line,
                        },
                        [point_id(*line_start_index), point_id(*line_end_index)],
                    );
                    self.derived(
                        id,
                        ObjectOp::TransformObject {
                            transform: ObjectTransform::ReflectByLine,
                        },
                        [point_id(*source_index), line_id],
                    );
                }
                ScenePointBinding::ReflectLineConstraint { source_index, line } => {
                    let line_id = format!("domain:{id}:reflection-axis");
                    if self.line_constraint(line_id.clone(), line) {
                        self.derived(
                            id,
                            ObjectOp::TransformObject {
                                transform: ObjectTransform::ReflectByLine,
                            },
                            [point_id(*source_index), line_id],
                        );
                    } else {
                        self.pending_source(id, "point-binding", source_value);
                    }
                }
                ScenePointBinding::Midpoint {
                    start_index,
                    end_index,
                } => self.derived(
                    id,
                    ObjectOp::Midpoint,
                    [point_id(*start_index), point_id(*end_index)],
                ),
                ScenePointBinding::Circumcenter {
                    start_index,
                    mid_index,
                    end_index,
                } => self.derived(
                    id,
                    ObjectOp::Circumcenter,
                    [
                        point_id(*start_index),
                        point_id(*mid_index),
                        point_id(*end_index),
                    ],
                ),
                binding @ ScenePointBinding::Rotate {
                    source_index,
                    center_index,
                    ..
                } => {
                    if let Some(angle_id) = self.rotation_scalar(index, binding) {
                        self.derived(
                            id,
                            ObjectOp::TransformObject {
                                transform: ObjectTransform::RotateDegrees,
                            },
                            [point_id(*source_index), point_id(*center_index), angle_id],
                        );
                    } else {
                        self.pending_source(id, "point-binding", source_value);
                    }
                }
                binding @ ScenePointBinding::Scale {
                    source_index,
                    center_index,
                    ..
                } => {
                    if let Some(factor_id) = self.scale_scalar(index, binding) {
                        self.derived(
                            id,
                            ObjectOp::TransformObject {
                                transform: ObjectTransform::ScaleByScalar,
                            },
                            [point_id(*source_index), point_id(*center_index), factor_id],
                        );
                    } else {
                        self.pending_source(id, "point-binding", source_value);
                    }
                }
                ScenePointBinding::MarkedAngleTranslation {
                    target_index,
                    angle_start_index,
                    angle_vertex_index,
                    angle_end_index,
                    distance_expr,
                    distance_parameter_group_ordinals,
                    ..
                } => {
                    let distance_id =
                        format!("scalar:point:{index}:marked-angle-translation-distance");
                    self.expression_with_group_sources(
                        distance_id.clone(),
                        distance_expr,
                        distance_parameter_group_ordinals,
                    );
                    self.derived(
                        id,
                        ObjectOp::MarkedAngleTranslationPoint,
                        [
                            point_id(*target_index),
                            point_id(*angle_start_index),
                            point_id(*angle_vertex_index),
                            point_id(*angle_end_index),
                            distance_id,
                        ],
                    );
                }
                ScenePointBinding::ScaleByRatio {
                    source_index,
                    center_index,
                    ratio_origin_index,
                    ratio_denominator_index,
                    ratio_numerator_index,
                    signed,
                    clamp_to_unit,
                } => self.derived(
                    id,
                    ObjectOp::TransformObject {
                        transform: ObjectTransform::ScaleByRatio {
                            signed: *signed,
                            clamp_to_unit: *clamp_to_unit,
                        },
                    },
                    [
                        point_id(*source_index),
                        point_id(*center_index),
                        point_id(*ratio_origin_index),
                        point_id(*ratio_denominator_index),
                        point_id(*ratio_numerator_index),
                    ],
                ),
                ScenePointBinding::PolarOffset {
                    source_index,
                    distance_expr,
                    distance_parameter_group_ordinals,
                    x_scale,
                    y_scale,
                } => {
                    let distance_id = format!("scalar:{id}:distance");
                    self.expression_with_group_sources(
                        distance_id.clone(),
                        distance_expr,
                        distance_parameter_group_ordinals,
                    );
                    self.derived(
                        id,
                        ObjectOp::TransformObject {
                            transform: ObjectTransform::TranslateScaledScalar {
                                x_scale: *x_scale,
                                y_scale: *y_scale,
                            },
                        },
                        [point_id(*source_index), distance_id],
                    );
                }
                ScenePointBinding::PolarTransform {
                    source_index,
                    distance_expr,
                    distance_parameter_group_ordinals,
                    distance_scale,
                    angle_expr,
                    angle_parameter_group_ordinals,
                    angle_degrees_scale,
                } => {
                    let distance_id = format!("scalar:{id}:distance");
                    let angle_id = format!("scalar:{id}:angle");
                    self.expression_with_group_sources(
                        distance_id.clone(),
                        distance_expr,
                        distance_parameter_group_ordinals,
                    );
                    self.expression_with_group_sources(
                        angle_id.clone(),
                        angle_expr,
                        angle_parameter_group_ordinals,
                    );
                    self.derived(
                        id,
                        ObjectOp::TransformObject {
                            transform: ObjectTransform::TranslatePolar {
                                invert_y: !self.y_up,
                                distance_scale: *distance_scale,
                                angle_degrees_scale: *angle_degrees_scale,
                            },
                        },
                        [point_id(*source_index), distance_id, angle_id],
                    );
                }
                ScenePointBinding::RadiusOffset {
                    source_index,
                    circle,
                    x_scale,
                    y_scale,
                    ..
                } => {
                    let circle_id = format!("domain:{id}:radius-circle");
                    if !self.circular_constraint(circle_id.clone(), circle) {
                        self.pending_source(id, "point-binding", source_value);
                        return;
                    }
                    let radius_id = format!("scalar:{id}:radius");
                    self.derived(radius_id.clone(), ObjectOp::CircularRadius, [circle_id]);
                    self.derived(
                        id,
                        ObjectOp::TransformObject {
                            transform: ObjectTransform::TranslateScaledScalar {
                                x_scale: *x_scale,
                                y_scale: *y_scale,
                            },
                        },
                        [point_id(*source_index), radius_id],
                    );
                }
                ScenePointBinding::BoundaryLengthOffset {
                    source_index,
                    boundary,
                    x_scale,
                    y_scale,
                } => {
                    let boundary_id = format!("domain:{id}:boundary-length");
                    let distance_id = format!("scalar:{id}:boundary-length");
                    if self.circular_constraint(boundary_id.clone(), boundary) {
                        self.derived(distance_id.clone(), ObjectOp::ArcLength, [boundary_id]);
                        self.derived(
                            id,
                            ObjectOp::TransformObject {
                                transform: ObjectTransform::TranslateScaledScalar {
                                    x_scale: *x_scale,
                                    y_scale: *y_scale,
                                },
                            },
                            [point_id(*source_index), distance_id],
                        );
                    } else {
                        self.pending_source(id, "point-binding", source_value);
                    }
                }
                ScenePointBinding::CustomTransform {
                    source_index,
                    origin_index,
                    axis_end_index,
                    distance_expr,
                    angle_expr,
                    distance_raw_scale,
                    angle_degrees_scale,
                } => {
                    let parameter_id = format!("scalar:{id}:transform-parameter");
                    if !self.point_parameter(parameter_id.clone(), *source_index) {
                        self.pending_source(id, "point-binding", source_value);
                        return;
                    }
                    self.derived(
                        id,
                        ObjectOp::CustomTransformPoint {
                            distance_expression: ObjectExpression::from_function_expr(
                                distance_expr,
                            ),
                            angle_expression: ObjectExpression::from_function_expr(angle_expr),
                            distance_parameter_names: expression_parameter_names(distance_expr),
                            angle_parameter_names: expression_parameter_names(angle_expr),
                            distance_scale: *distance_raw_scale,
                            angle_degrees_scale: *angle_degrees_scale,
                        },
                        [
                            point_id(*origin_index),
                            point_id(*axis_end_index),
                            parameter_id,
                        ],
                    );
                }
                ScenePointBinding::CoordinateSource {
                    source_index,
                    expr,
                    parameter_group_ordinals,
                    axis,
                    ..
                } => {
                    let offset_id = format!("scalar:{id}:coordinate-offset");
                    self.expression_with_group_sources(
                        offset_id.clone(),
                        expr,
                        parameter_group_ordinals,
                    );
                    let (x_scale, y_scale) = match axis {
                        CoordinateAxis::Horizontal => (1.0, 0.0),
                        CoordinateAxis::Vertical => (0.0, 1.0),
                    };
                    self.derived(
                        id,
                        ObjectOp::TransformObject {
                            transform: ObjectTransform::TranslateScaledScalar { x_scale, y_scale },
                        },
                        [point_id(*source_index), offset_id],
                    );
                }
                ScenePointBinding::Coordinate { name, expr } => {
                    let Some(x_id) = self.named_scalars.get(name).cloned() else {
                        self.pending_source(id, "point-binding", source_value);
                        return;
                    };
                    let y_id = format!("scalar:{id}:coordinate-y");
                    self.expression(y_id.clone(), expr);
                    self.derived(id, ObjectOp::PointFromScalars, [x_id, y_id]);
                }
                ScenePointBinding::CoordinateSource2d {
                    source_index,
                    x_scalar_group_ordinal,
                    x_expr,
                    y_scalar_group_ordinal,
                    y_expr,
                    ..
                } => {
                    let x_id = format!("scalar:{id}:coordinate-x");
                    let y_id = format!("scalar:{id}:coordinate-y");
                    if let Some(parent) = x_scalar_group_ordinal
                        .and_then(|ordinal| self.group_scalars.get(&ordinal))
                        .cloned()
                    {
                        self.derived(x_id.clone(), ObjectOp::Copy, [parent]);
                    } else {
                        self.expression(x_id.clone(), x_expr);
                    }
                    if let Some(parent) = y_scalar_group_ordinal
                        .and_then(|ordinal| self.group_scalars.get(&ordinal))
                        .cloned()
                    {
                        self.derived(y_id.clone(), ObjectOp::Copy, [parent]);
                    } else {
                        self.expression(y_id.clone(), y_expr);
                    }
                    self.derived(
                        id,
                        ObjectOp::TransformObject {
                            transform: ObjectTransform::TranslateByScalars,
                        },
                        [point_id(*source_index), x_id, y_id],
                    );
                }
            }
            return;
        }

        match &point.constraint {
            ScenePointConstraint::Free => {
                if let Some(debug) = &point.debug
                    && let variant = match debug.group_kind {
                        GroupKind::LinearIntersectionPoint => Some(0),
                        GroupKind::IntersectionPoint1
                        | GroupKind::CircleCircleIntersectionPoint1 => Some(0),
                        GroupKind::IntersectionPoint2
                        | GroupKind::CircleCircleIntersectionPoint2 => Some(1),
                        _ => None,
                    }
                    && let Some(variant) = variant
                {
                    if self.payload_intersection(id.clone(), debug.group_ordinal, variant) {
                        return;
                    }
                    self.pending_source(id, "point-binding", source_value);
                    return;
                }
                let is_payload_source = point
                    .debug
                    .as_ref()
                    .is_none_or(|debug| debug.group_kind == crate::format::GroupKind::Point);
                if is_payload_source {
                    self.bound_source(
                        id,
                        source_value,
                        SceneObjectSourceBinding::Point { point_index: index },
                    );
                } else {
                    self.pending_source(id, "point-binding", source_value);
                }
            }
            ScenePointConstraint::Offset {
                origin_index,
                dx,
                dy,
            } => self.derived(
                id,
                ObjectOp::TransformObject {
                    transform: ObjectTransform::TranslateDelta { dx: *dx, dy: *dy },
                },
                [point_id(*origin_index)],
            ),
            ScenePointConstraint::OnSegment {
                start_index,
                end_index,
                t,
            } => self.point_on_line(index, id, *start_index, *end_index, *t, LineKind::Segment),
            ScenePointConstraint::OnLine {
                start_index,
                end_index,
                t,
            } => self.point_on_line(index, id, *start_index, *end_index, *t, LineKind::Line),
            ScenePointConstraint::OnRay {
                start_index,
                end_index,
                t,
            } => self.point_on_line(index, id, *start_index, *end_index, *t, LineKind::Ray),
            ScenePointConstraint::OnPolyline {
                function_key,
                parameter,
                ..
            } => {
                let mut matching_lines = self
                    .line_group_ordinals
                    .iter()
                    .enumerate()
                    .filter_map(|(line_index, group_ordinal)| {
                        (*group_ordinal == Some(*function_key)).then_some(line_index)
                    })
                    .collect::<Vec<_>>();
                matching_lines.extend(self.function_lines.iter().filter_map(
                    |(line_index, function)| {
                        (function.plot_key == Some(*function_key)).then_some(*line_index)
                    },
                ));
                matching_lines.sort_unstable();
                matching_lines.dedup();
                if let [line_index] = matching_lines.as_slice() {
                    let parameter_id = format!("control:{id}:t");
                    self.point_control_source(
                        parameter_id.clone(),
                        *parameter,
                        index,
                        ScenePointControl::Parameter,
                    );
                    self.derived(
                        id,
                        ObjectOp::PointOnPolyline,
                        [line_id(*line_index), parameter_id],
                    );
                } else {
                    self.pending_source(id, "point-constraint", source_value);
                }
            }
            ScenePointConstraint::OnLineConstraint { line, t }
            | ScenePointConstraint::OnRayConstraint { line, t } => {
                let domain_id = format!("domain:{id}");
                if self.line_constraint(domain_id.clone(), line) {
                    self.point_on_domain_line(index, id, domain_id, *t);
                } else {
                    self.pending_source(id, "point-constraint", source_value);
                }
            }
            ScenePointConstraint::OnPolygonBoundary {
                vertex_indices,
                edge_index,
                t,
            } => {
                if vertex_indices.len() < 2 {
                    self.pending_source(id, "point-constraint", source_value);
                } else {
                    let local_parameter_id = format!("control:{id}:t");
                    let boundary_parameter_id = format!("control:{id}:boundary");
                    self.point_control_source(
                        local_parameter_id.clone(),
                        *t,
                        index,
                        ScenePointControl::Parameter,
                    );
                    self.derived(
                        boundary_parameter_id.clone(),
                        ObjectOp::PolygonBoundaryParameter {
                            edge_index: *edge_index,
                        },
                        vertex_indices
                            .iter()
                            .copied()
                            .map(point_id)
                            .chain(std::iter::once(local_parameter_id)),
                    );
                    self.derived(
                        id,
                        ObjectOp::PointOnPolygonBoundary,
                        vertex_indices
                            .iter()
                            .copied()
                            .map(point_id)
                            .chain(std::iter::once(boundary_parameter_id)),
                    );
                }
            }
            ScenePointConstraint::OnPolygonBoundaryParameter {
                vertex_indices,
                parameter,
            } => {
                if vertex_indices.len() < 2 {
                    self.pending_source(id, "point-constraint", source_value);
                } else {
                    let parameter_id = format!("control:{id}:boundary");
                    self.point_control_source(
                        parameter_id.clone(),
                        *parameter,
                        index,
                        ScenePointControl::Boundary,
                    );
                    self.derived(
                        id,
                        ObjectOp::PointOnPolygonBoundary,
                        vertex_indices
                            .iter()
                            .copied()
                            .map(point_id)
                            .chain(std::iter::once(parameter_id)),
                    );
                }
            }
            ScenePointConstraint::OnTranslatedPolygonBoundary {
                vertex_indices,
                vector_start_index,
                vector_end_index,
                edge_index,
                t,
            } => {
                if vertex_indices.len() < 2 {
                    self.pending_source(id, "point-constraint", source_value);
                } else {
                    let base_id = format!("domain:{id}:base");
                    let domain_id = format!("domain:{id}");
                    let start_index = vertex_indices[*edge_index % vertex_indices.len()];
                    let end_index = vertex_indices[(*edge_index + 1) % vertex_indices.len()];
                    self.derived(
                        base_id.clone(),
                        ObjectOp::Line {
                            line_kind: LineKind::Segment,
                        },
                        [point_id(start_index), point_id(end_index)],
                    );
                    self.derived(
                        domain_id.clone(),
                        ObjectOp::TransformObject {
                            transform: ObjectTransform::TranslateByVector,
                        },
                        [
                            base_id,
                            point_id(*vector_start_index),
                            point_id(*vector_end_index),
                        ],
                    );
                    self.point_on_domain_line(index, id, domain_id, *t);
                }
            }
            ScenePointConstraint::OnCircle {
                center_index,
                radius_index,
                unit_x,
                unit_y,
            } => {
                let domain_id = format!("domain:{id}");
                let unit_x_id = format!("control:{id}:unit-x");
                let unit_y_id = format!("control:{id}:unit-y");
                self.derived(
                    domain_id.clone(),
                    ObjectOp::CircleByPoints,
                    [point_id(*center_index), point_id(*radius_index)],
                );
                self.point_control_source(
                    unit_x_id.clone(),
                    *unit_x,
                    index,
                    ScenePointControl::UnitX,
                );
                self.point_control_source(
                    unit_y_id.clone(),
                    *unit_y,
                    index,
                    ScenePointControl::UnitY,
                );
                self.derived(
                    id,
                    ObjectOp::PointOnCircle { invert_y: false },
                    [domain_id, unit_x_id, unit_y_id],
                );
            }
            ScenePointConstraint::OnCircularConstraint {
                circle,
                unit_x,
                unit_y,
            } => {
                let domain_id = format!("domain:{id}");
                if self.circular_constraint(domain_id.clone(), circle) {
                    self.point_on_circle(index, id, domain_id, *unit_x, *unit_y, true);
                } else {
                    self.pending_source(id, "point-constraint", source_value);
                }
            }
            ScenePointConstraint::OnArc {
                start_index,
                mid_index,
                end_index,
                t,
            } => {
                let domain_id = format!("domain:{id}");
                let parameter_id = format!("control:{id}:t");
                self.derived(
                    domain_id.clone(),
                    ObjectOp::ThreePointArc { complement: false },
                    [
                        point_id(*start_index),
                        point_id(*mid_index),
                        point_id(*end_index),
                    ],
                );
                self.point_control_source(
                    parameter_id.clone(),
                    *t,
                    index,
                    ScenePointControl::Parameter,
                );
                self.derived(id, ObjectOp::PointOnArc, [domain_id, parameter_id]);
            }
            ScenePointConstraint::OnCircleArc {
                center_index,
                start_index,
                end_index,
                t,
            } => {
                let domain_id = format!("domain:{id}");
                let parameter_id = format!("control:{id}:t");
                self.derived(
                    domain_id.clone(),
                    ObjectOp::CenterArc { y_up: self.y_up },
                    [
                        point_id(*center_index),
                        point_id(*start_index),
                        point_id(*end_index),
                    ],
                );
                self.point_control_source(
                    parameter_id.clone(),
                    *t,
                    index,
                    ScenePointControl::Parameter,
                );
                self.derived(id, ObjectOp::PointOnArc, [domain_id, parameter_id]);
            }
            ScenePointConstraint::OnArcConstraint { arc, t } => {
                let domain_id = format!("domain:{id}");
                let parameter_id = format!("control:{id}:t");
                if self.arc_constraint(domain_id.clone(), arc) {
                    self.point_control_source(
                        parameter_id.clone(),
                        *t,
                        index,
                        ScenePointControl::Parameter,
                    );
                    self.derived(id, ObjectOp::PointOnArc, [domain_id, parameter_id]);
                } else {
                    self.pending_source(id, "point-constraint", source_value);
                }
            }
            ScenePointConstraint::LineIntersection { left, right } => {
                let left_id = format!("domain:{id}:left");
                let right_id = format!("domain:{id}:right");
                if self.line_constraint(left_id.clone(), left)
                    && self.line_constraint(right_id.clone(), right)
                {
                    self.derived(id, ObjectOp::LineIntersection, [left_id, right_id]);
                } else {
                    self.pending_source(id, "point-constraint", source_value);
                }
            }
            ScenePointConstraint::LinePolygonIntersection {
                line,
                vertex_indices,
                variant,
            } => {
                let domain_line_id = format!("domain:{id}:line");
                let domain_polygon_id = format!("domain:{id}:polygon");
                if vertex_indices.len() >= 2 && self.line_constraint(domain_line_id.clone(), line) {
                    self.derived(
                        domain_polygon_id.clone(),
                        ObjectOp::Polygon,
                        vertex_indices
                            .iter()
                            .copied()
                            .chain(vertex_indices.first().copied())
                            .map(point_id),
                    );
                    self.derived(
                        id,
                        ObjectOp::LinePolylineIntersection {
                            variant: *variant,
                            sample_hint: None,
                        },
                        [domain_line_id, domain_polygon_id],
                    );
                } else {
                    self.pending_source(id, "point-constraint", source_value);
                }
            }
            ScenePointConstraint::LineTraceIntersection {
                line,
                trace_key,
                variant,
                ..
            } => {
                let domain_line_id = format!("domain:{id}:line");
                let matching_traces = self
                    .line_group_ordinals
                    .iter()
                    .enumerate()
                    .filter_map(|(line_index, group_ordinal)| {
                        (*group_ordinal == Some(*trace_key)).then_some(line_index)
                    })
                    .collect::<Vec<_>>();
                if self.line_constraint(domain_line_id.clone(), line)
                    && let [trace_index] = matching_traces.as_slice()
                {
                    self.derived(
                        id,
                        ObjectOp::LinePolylineIntersection {
                            variant: *variant,
                            sample_hint: None,
                        },
                        [domain_line_id, line_id(*trace_index)],
                    );
                } else {
                    self.pending_source(id, "point-constraint", source_value);
                }
            }
            ScenePointConstraint::CircularTraceIntersection {
                circle,
                trace_key,
                variant,
                sample_hint,
                ..
            } => {
                let domain_circle_id = format!("domain:{id}:circle");
                let matching_traces = self
                    .line_group_ordinals
                    .iter()
                    .enumerate()
                    .filter_map(|(line_index, group_ordinal)| {
                        (*group_ordinal == Some(*trace_key)).then_some(line_index)
                    })
                    .collect::<Vec<_>>();
                if self.circular_constraint(domain_circle_id.clone(), circle)
                    && let [trace_index] = matching_traces.as_slice()
                {
                    self.derived(
                        id,
                        ObjectOp::CircularPolylineIntersection {
                            variant: *variant,
                            sample_hint: sample_hint.map(|sample| sample as f64),
                        },
                        [domain_circle_id, line_id(*trace_index)],
                    );
                } else {
                    self.pending_source(id, "point-constraint", source_value);
                }
            }
            ScenePointConstraint::LineFunctionIntersection {
                line,
                function_key,
                expr,
                x_min,
                x_max,
                sample_count,
                polar,
                sample_hint,
            } => {
                let domain_line_id = format!("domain:{id}:line");
                let matching_plots = self
                    .function_lines
                    .iter()
                    .filter_map(|(line_index, function)| {
                        (function.plot_key == Some(*function_key)).then_some(*line_index)
                    })
                    .collect::<Vec<_>>();
                if self.line_constraint(domain_line_id.clone(), line) {
                    let function_id = if let [plot_index] = matching_plots.as_slice() {
                        line_id(*plot_index)
                    } else {
                        let function_id = format!("domain:{id}:function");
                        let (parameter_names, parents): (Vec<_>, Vec<_>) =
                            expression_parameter_names(expr)
                                .into_iter()
                                .filter_map(|name| {
                                    self.named_scalars
                                        .get(&name)
                                        .cloned()
                                        .map(|parent| (name, parent))
                                })
                                .unzip();
                        self.derived(
                            function_id.clone(),
                            ObjectOp::FunctionPlot {
                                expression: ObjectExpression::from_function_expr(expr),
                                parameter_names,
                                value_min: *x_min,
                                value_max: *x_max,
                                sample_count: *sample_count,
                                plot_mode: if *polar {
                                    PlotMode::Polar
                                } else {
                                    PlotMode::Cartesian
                                },
                            },
                            parents,
                        );
                        function_id
                    };
                    self.derived(
                        id,
                        ObjectOp::LinePolylineIntersection {
                            variant: 0,
                            sample_hint: sample_hint.map(|sample| {
                                let denominator = sample_count.saturating_sub(1).max(1) as f64;
                                x_min + (sample as f64 / denominator) * (x_max - x_min)
                            }),
                        },
                        [domain_line_id, function_id],
                    );
                } else {
                    self.pending_source(id, "point-constraint", source_value);
                }
            }
            ScenePointConstraint::PointCircularTangent {
                point_index,
                circle,
                variant,
            } => {
                let circle_id = format!("domain:{id}:circle");
                if self.circular_constraint(circle_id.clone(), circle) {
                    self.derived(
                        id,
                        ObjectOp::PointCircleTangent { variant: *variant },
                        [point_id(*point_index), circle_id],
                    );
                } else {
                    self.pending_source(id, "point-constraint", source_value);
                }
            }
            ScenePointConstraint::LineCircularIntersection {
                line,
                circle,
                variant,
            } => {
                let line_id = format!("domain:{id}:line");
                let circle_id = format!("domain:{id}:circle");
                if self.line_constraint(line_id.clone(), line)
                    && self.circular_constraint(circle_id.clone(), circle)
                {
                    self.derived(
                        id,
                        ObjectOp::LineCircleIntersection { variant: *variant },
                        [line_id, circle_id],
                    );
                } else {
                    self.pending_source(id, "point-constraint", source_value);
                }
            }
            ScenePointConstraint::LineCircleIntersection {
                line,
                center_index,
                radius_index,
                variant,
            } => {
                let line_id = format!("domain:{id}:line");
                let circle_id = format!("domain:{id}:circle");
                if self.line_constraint(line_id.clone(), line) {
                    self.derived(
                        circle_id.clone(),
                        ObjectOp::CircleByPoints,
                        [point_id(*center_index), point_id(*radius_index)],
                    );
                    self.derived(
                        id,
                        ObjectOp::LineCircleIntersection { variant: *variant },
                        [line_id, circle_id],
                    );
                } else {
                    self.pending_source(id, "point-constraint", source_value);
                }
            }
            ScenePointConstraint::CircleCircleIntersection {
                left_center_index,
                left_radius_index,
                right_center_index,
                right_radius_index,
                variant,
            } => {
                let left_id = format!("domain:{id}:left-circle");
                let right_id = format!("domain:{id}:right-circle");
                self.derived(
                    left_id.clone(),
                    ObjectOp::CircleByPoints,
                    [point_id(*left_center_index), point_id(*left_radius_index)],
                );
                self.derived(
                    right_id.clone(),
                    ObjectOp::CircleByPoints,
                    [point_id(*right_center_index), point_id(*right_radius_index)],
                );
                self.derived(
                    id,
                    ObjectOp::CircleCircleIntersection { variant: *variant },
                    [left_id, right_id],
                );
            }
            ScenePointConstraint::CircularIntersection {
                left,
                right,
                variant,
            } => {
                let left_id = format!("domain:{id}:left-circle");
                let right_id = format!("domain:{id}:right-circle");
                if self.circular_constraint(left_id.clone(), left)
                    && self.circular_constraint(right_id.clone(), right)
                {
                    self.derived(
                        id,
                        ObjectOp::CircleCircleIntersection { variant: *variant },
                        [left_id, right_id],
                    );
                } else {
                    self.pending_source(id, "point-constraint", source_value);
                }
            }
        }
    }

    fn payload_intersection(&mut self, id: String, group_ordinal: usize, variant: usize) -> bool {
        let Some([left_group, right_group]) = self
            .payload_dependencies
            .get(&group_ordinal)
            .and_then(|groups| <&[usize; 2]>::try_from(groups.as_slice()).ok())
            .copied()
        else {
            return false;
        };
        let Some(left_id) = self.group_objects.get(&left_group).cloned() else {
            return false;
        };
        let Some(right_id) = self.group_objects.get(&right_group).cloned() else {
            return false;
        };
        let Some(left_kind) = self.group_geometry_kinds.get(&left_group).copied() else {
            return false;
        };
        let Some(right_kind) = self.group_geometry_kinds.get(&right_group).copied() else {
            return false;
        };
        match (left_kind, right_kind) {
            (GeometryNodeKind::Line, GeometryNodeKind::Line) => {
                self.derived(id, ObjectOp::LineIntersection, [left_id, right_id]);
            }
            (GeometryNodeKind::Line, GeometryNodeKind::Circle) => self.derived(
                id,
                ObjectOp::LineCircleIntersection { variant },
                [left_id, right_id],
            ),
            (GeometryNodeKind::Circle, GeometryNodeKind::Line) => self.derived(
                id,
                ObjectOp::LineCircleIntersection { variant },
                [right_id, left_id],
            ),
            (GeometryNodeKind::Circle, GeometryNodeKind::Circle) => self.derived(
                id,
                ObjectOp::CircleCircleIntersection { variant },
                [left_id, right_id],
            ),
            _ => return false,
        }
        true
    }

    fn rotation_scalar(
        &mut self,
        point_index: usize,
        binding: &ScenePointBinding,
    ) -> Option<String> {
        let ScenePointBinding::Rotate {
            angle_degrees,
            parameter_name,
            angle_expr,
            angle_parameter_group_ordinals,
            angle_start_index,
            angle_vertex_index,
            angle_end_index,
            angle_parameter_point_index,
            angle_parameter_start_index,
            angle_parameter_end_index,
            angle_parameter_scale,
            ..
        } = binding
        else {
            return None;
        };
        let id = format!("scalar:point:{point_index}:rotation-degrees");
        if let (Some(point_index), Some(start_index), Some(end_index)) = (
            angle_parameter_point_index,
            angle_parameter_start_index,
            angle_parameter_end_index,
        ) {
            let raw_id = format!("{id}:parameter");
            self.point_line_parameter(
                raw_id.clone(),
                *point_index,
                *start_index,
                *end_index,
                LineKind::Segment,
            );
            self.derived(
                id.clone(),
                ObjectOp::ScaleScalar {
                    factor: angle_parameter_scale.unwrap_or(1.0),
                },
                [raw_id],
            );
        } else if let (Some(start_index), Some(vertex_index), Some(end_index)) =
            (angle_start_index, angle_vertex_index, angle_end_index)
        {
            self.derived(
                id.clone(),
                ObjectOp::MeasuredRotationDegrees,
                [
                    point_id(*start_index),
                    point_id(*vertex_index),
                    point_id(*end_index),
                ],
            );
        } else if let Some(expression) = angle_expr {
            self.expression_with_group_sources(
                id.clone(),
                expression,
                angle_parameter_group_ordinals,
            );
        } else if let Some(name) = parameter_name {
            let parent = self.named_scalars.get(name)?.clone();
            self.derived(id.clone(), ObjectOp::Copy, [parent]);
        } else {
            self.source(
                id.clone(),
                ObjectValue::Scalar {
                    value: *angle_degrees,
                },
            );
        }
        Some(id)
    }

    fn derived_parameter_point(
        &mut self,
        id: String,
        target_index: usize,
        source_index: usize,
        parameter_start_index: Option<usize>,
        parameter_end_index: Option<usize>,
    ) -> bool {
        let parameter_id = format!("scalar:{id}:derived-parameter");
        if let (Some(start_index), Some(end_index)) = (parameter_start_index, parameter_end_index) {
            self.point_line_parameter(
                parameter_id.clone(),
                source_index,
                start_index,
                end_index,
                LineKind::Segment,
            );
        } else if !self.point_parameter(parameter_id.clone(), source_index) {
            if !self.point_parameter_on_target_domain(
                parameter_id.clone(),
                source_index,
                target_index,
            ) {
                return false;
            }
        }
        self.point_at_parameter(id, target_index, parameter_id)
    }

    fn point_parameter_on_target_domain(
        &mut self,
        id: String,
        source_index: usize,
        target_index: usize,
    ) -> bool {
        let Some(constraint) = self.point_constraints.get(target_index).cloned() else {
            return false;
        };
        match constraint {
            ScenePointConstraint::OnSegment {
                start_index,
                end_index,
                ..
            } => self.point_line_parameter(
                id,
                source_index,
                start_index,
                end_index,
                LineKind::Segment,
            ),
            ScenePointConstraint::OnLine {
                start_index,
                end_index,
                ..
            } => {
                self.point_line_parameter(id, source_index, start_index, end_index, LineKind::Line)
            }
            ScenePointConstraint::OnRay {
                start_index,
                end_index,
                ..
            } => self.point_line_parameter(id, source_index, start_index, end_index, LineKind::Ray),
            ScenePointConstraint::OnLineConstraint { line, .. }
            | ScenePointConstraint::OnRayConstraint { line, .. } => {
                let domain_id = format!("domain:{id}:line");
                if !self.line_constraint(domain_id.clone(), &line) {
                    return false;
                }
                self.derived(
                    id,
                    ObjectOp::PointLineParameter,
                    [point_id(source_index), domain_id],
                );
            }
            ScenePointConstraint::OnCircle {
                center_index,
                radius_index,
                ..
            } => {
                let domain_id = format!("domain:{id}:circle");
                self.derived(
                    domain_id.clone(),
                    ObjectOp::CircleByPoints,
                    [point_id(center_index), point_id(radius_index)],
                );
                self.derived(
                    id,
                    ObjectOp::CircleParameter {
                        invert_y: !self.y_up,
                    },
                    [point_id(source_index), domain_id],
                );
            }
            ScenePointConstraint::OnCircularConstraint { circle, .. } => {
                let domain_id = format!("domain:{id}:circle");
                if !self.circular_constraint(domain_id.clone(), &circle) {
                    return false;
                }
                self.derived(
                    id,
                    ObjectOp::CircleParameter {
                        invert_y: !self.y_up,
                    },
                    [point_id(source_index), domain_id],
                );
            }
            ScenePointConstraint::OnPolygonBoundary { vertex_indices, .. }
            | ScenePointConstraint::OnPolygonBoundaryParameter { vertex_indices, .. } => {
                if vertex_indices.len() < 2 {
                    return false;
                }
                self.derived(
                    id,
                    ObjectOp::PolygonBoundaryParameterFromPoint,
                    vertex_indices
                        .into_iter()
                        .map(point_id)
                        .chain(std::iter::once(point_id(source_index))),
                );
            }
            ScenePointConstraint::OnTranslatedPolygonBoundary {
                vertex_indices,
                vector_start_index,
                vector_end_index,
                ..
            } => {
                if vertex_indices.len() < 2 {
                    return false;
                }
                let translated_vertices = self.translated_polygon_vertices(
                    &id,
                    &vertex_indices,
                    vector_start_index,
                    vector_end_index,
                );
                self.derived(
                    id,
                    ObjectOp::PolygonBoundaryParameterFromPoint,
                    translated_vertices
                        .into_iter()
                        .chain(std::iter::once(point_id(source_index))),
                );
            }
            ScenePointConstraint::OnCircleArc {
                center_index,
                start_index,
                end_index,
                ..
            } => {
                let domain_id = format!("domain:{id}:arc");
                self.derived(
                    domain_id.clone(),
                    ObjectOp::CenterArc { y_up: self.y_up },
                    [
                        point_id(center_index),
                        point_id(start_index),
                        point_id(end_index),
                    ],
                );
                self.derived(
                    id,
                    ObjectOp::ArcParameterFromPoint,
                    [domain_id, point_id(source_index)],
                );
            }
            ScenePointConstraint::OnArc {
                start_index,
                mid_index,
                end_index,
                ..
            } => {
                let domain_id = format!("domain:{id}:arc");
                self.derived(
                    domain_id.clone(),
                    ObjectOp::ThreePointArc { complement: false },
                    [
                        point_id(start_index),
                        point_id(mid_index),
                        point_id(end_index),
                    ],
                );
                self.derived(
                    id,
                    ObjectOp::ArcParameterFromPoint,
                    [domain_id, point_id(source_index)],
                );
            }
            ScenePointConstraint::OnArcConstraint { arc, .. } => {
                let domain_id = format!("domain:{id}:arc");
                if !self.arc_constraint(domain_id.clone(), &arc) {
                    return false;
                }
                self.derived(
                    id,
                    ObjectOp::ArcParameterFromPoint,
                    [domain_id, point_id(source_index)],
                );
            }
            ScenePointConstraint::OnPolyline { function_key, .. } => {
                let mut matching_lines = self
                    .line_group_ordinals
                    .iter()
                    .enumerate()
                    .filter_map(|(line_index, group_ordinal)| {
                        (*group_ordinal == Some(function_key)).then_some(line_index)
                    })
                    .collect::<Vec<_>>();
                matching_lines.extend(self.function_lines.iter().filter_map(
                    |(line_index, function)| {
                        (function.plot_key == Some(function_key)).then_some(*line_index)
                    },
                ));
                matching_lines.sort_unstable();
                matching_lines.dedup();
                let [line_index] = matching_lines.as_slice() else {
                    return false;
                };
                self.derived(
                    id,
                    ObjectOp::PolylineParameterFromPoint,
                    [line_id(*line_index), point_id(source_index)],
                );
            }
            _ => return false,
        }
        true
    }

    fn expression_with_bound_parameter(
        &mut self,
        id: String,
        expression: &gsp_runtime_core::FunctionExpr,
        bound_name: &str,
        bound_id: String,
    ) -> bool {
        let parameter_names = expression_parameter_names(expression);
        let Some(parents) = parameter_names
            .iter()
            .map(|name| {
                if name == bound_name {
                    Some(bound_id.clone())
                } else {
                    self.named_scalars.get(name).cloned()
                }
            })
            .collect::<Option<Vec<_>>>()
        else {
            return false;
        };
        self.derived(
            id,
            ObjectOp::EvaluateExpression {
                expression: ObjectExpression::from_function_expr(expression),
                parameter_names,
                x: 0.0,
            },
            parents,
        );
        true
    }

    fn scalar_sum(&mut self, id: String, left: String, right: String) {
        self.derived(
            id,
            ObjectOp::EvaluateExpression {
                expression: ObjectExpression::Binary {
                    left: Box::new(ObjectExpression::Parameter {
                        name: "__left".to_string(),
                        default: 0.0,
                    }),
                    op: gsp_runtime_core::BinaryOp::Add,
                    right: Box::new(ObjectExpression::Parameter {
                        name: "__right".to_string(),
                        default: 0.0,
                    }),
                },
                parameter_names: vec!["__left".to_string(), "__right".to_string()],
                x: 0.0,
            },
            [left, right],
        );
    }

    fn expression_with_point_parameter_sources(
        &mut self,
        id: String,
        expression: &gsp_runtime_core::FunctionExpr,
        sources: &[crate::runtime::scene::ScenePointParameterSource],
        parameter_group_ordinals: &BTreeMap<String, usize>,
        absolute_value: bool,
        primary_parameter_override: Option<String>,
    ) -> bool {
        let parameter_names = expression_parameter_names(expression);
        let mut parents = Vec::with_capacity(parameter_names.len());
        for name in &parameter_names {
            if let Some(parent) = parameter_group_ordinals
                .get(name)
                .and_then(|ordinal| self.group_scalars.get(ordinal))
                .cloned()
            {
                parents.push(parent);
            } else if let Some((source_index, source)) = sources
                .iter()
                .enumerate()
                .find(|(_, source)| source.name == *name)
            {
                if source_index == 0
                    && let Some(primary_id) = &primary_parameter_override
                {
                    parents.push(primary_id.clone());
                    continue;
                }
                let scalar_id = format!("{id}:source:{}", parents.len());
                if !self.point_parameter_source(scalar_id.clone(), source) {
                    return false;
                }
                parents.push(scalar_id);
            } else if let Some(parent) = self.named_scalars.get(name).cloned() {
                parents.push(parent);
            } else {
                return false;
            }
        }
        let expression_id = if absolute_value {
            id.clone()
        } else {
            format!("{id}:offset")
        };
        self.derived(
            expression_id.clone(),
            ObjectOp::EvaluateExpression {
                expression: ObjectExpression::from_function_expr(expression),
                parameter_names,
                x: 0.0,
            },
            parents,
        );
        if !absolute_value {
            let Some(primary) = sources.first() else {
                return false;
            };
            let base_id = if let Some(primary_id) = primary_parameter_override {
                primary_id
            } else {
                let base_id = format!("{id}:base");
                if !self.point_parameter_source(base_id.clone(), primary) {
                    return false;
                }
                base_id
            };
            let sum_id = format!("{id}:sum");
            self.scalar_sum(sum_id.clone(), base_id, expression_id);
            self.derived(id, ObjectOp::WrapUnitScalar, [sum_id]);
        }
        true
    }

    fn point_parameter_source(
        &mut self,
        id: String,
        source: &crate::runtime::scene::ScenePointParameterSource,
    ) -> bool {
        match &source.domain {
            Some(crate::runtime::scene::ScenePointParameterDomain::Circular(circle)) => {
                let circle_id = format!("{id}:circle");
                if !self.circular_constraint(circle_id.clone(), circle) {
                    return false;
                }
                self.derived(
                    id,
                    ObjectOp::CircleParameter {
                        invert_y: !self.y_up,
                    },
                    [point_id(source.point_index), circle_id],
                );
            }
            Some(crate::runtime::scene::ScenePointParameterDomain::PolygonBoundary {
                vertex_indices,
            }) => self.derived(
                id,
                ObjectOp::PolygonBoundaryParameterFromPoint,
                vertex_indices
                    .iter()
                    .copied()
                    .map(point_id)
                    .chain(std::iter::once(point_id(source.point_index))),
            ),
            None if !self.point_parameter(id, source.point_index) => return false,
            None => {}
        }
        true
    }

    fn point_parameter(&mut self, id: String, point_index: usize) -> bool {
        let Some(constraint) = self.point_constraints.get(point_index).cloned() else {
            return false;
        };
        match constraint {
            ScenePointConstraint::OnSegment {
                start_index,
                end_index,
                ..
            } => self.point_line_parameter(
                id,
                point_index,
                start_index,
                end_index,
                LineKind::Segment,
            ),
            ScenePointConstraint::OnLine {
                start_index,
                end_index,
                ..
            } => self.point_line_parameter(id, point_index, start_index, end_index, LineKind::Line),
            ScenePointConstraint::OnRay {
                start_index,
                end_index,
                ..
            } => self.point_line_parameter(id, point_index, start_index, end_index, LineKind::Ray),
            ScenePointConstraint::OnLineConstraint { line, .. }
            | ScenePointConstraint::OnRayConstraint { line, .. } => {
                let domain_id = format!("domain:{id}:line");
                if !self.line_constraint(domain_id.clone(), &line) {
                    return false;
                }
                self.derived(
                    id,
                    ObjectOp::PointLineParameter,
                    [point_id(point_index), domain_id],
                );
            }
            ScenePointConstraint::OnCircle {
                center_index,
                radius_index,
                ..
            } => {
                let circle_id = format!("domain:{id}:circle");
                self.derived(
                    circle_id.clone(),
                    ObjectOp::CircleByPoints,
                    [point_id(center_index), point_id(radius_index)],
                );
                self.derived(
                    id,
                    ObjectOp::CircleParameter {
                        invert_y: !self.y_up,
                    },
                    [point_id(point_index), circle_id],
                );
            }
            ScenePointConstraint::OnCircularConstraint { circle, .. } => {
                let circle_id = format!("domain:{id}:circle");
                if !self.circular_constraint(circle_id.clone(), &circle) {
                    return false;
                }
                self.derived(
                    id,
                    ObjectOp::CircleParameter {
                        invert_y: !self.y_up,
                    },
                    [point_id(point_index), circle_id],
                );
            }
            ScenePointConstraint::OnPolyline { .. }
            | ScenePointConstraint::OnArc { .. }
            | ScenePointConstraint::OnCircleArc { .. } => self.derived(
                id,
                ObjectOp::Copy,
                [format!("control:{}:t", point_id(point_index))],
            ),
            ScenePointConstraint::OnPolygonBoundary {
                vertex_indices,
                edge_index,
                ..
            } => {
                if vertex_indices.len() < 2 {
                    return false;
                }
                let point_node_id = point_id(point_index);
                if self.nodes.iter().any(|node| node.id == point_node_id) {
                    self.derived(
                        id,
                        ObjectOp::PolygonBoundaryParameterFromPoint,
                        vertex_indices
                            .into_iter()
                            .map(point_id)
                            .chain(std::iter::once(point_node_id)),
                    );
                } else {
                    self.derived(
                        id,
                        ObjectOp::PolygonBoundaryParameter { edge_index },
                        vertex_indices
                            .into_iter()
                            .map(point_id)
                            .chain(std::iter::once(format!("control:{point_node_id}:t"))),
                    );
                }
            }
            ScenePointConstraint::OnTranslatedPolygonBoundary {
                vertex_indices,
                vector_start_index,
                vector_end_index,
                edge_index,
                ..
            } => {
                if vertex_indices.len() < 2 {
                    return false;
                }
                let point_node_id = point_id(point_index);
                let translated_vertices = self.translated_polygon_vertices(
                    &id,
                    &vertex_indices,
                    vector_start_index,
                    vector_end_index,
                );
                if self.nodes.iter().any(|node| node.id == point_node_id) {
                    self.derived(
                        id,
                        ObjectOp::PolygonBoundaryParameterFromPoint,
                        translated_vertices
                            .into_iter()
                            .chain(std::iter::once(point_node_id)),
                    );
                } else {
                    self.derived(
                        id,
                        ObjectOp::PolygonBoundaryParameter { edge_index },
                        translated_vertices
                            .into_iter()
                            .chain(std::iter::once(format!(
                                "control:{}:t",
                                point_id(point_index)
                            ))),
                    );
                }
            }
            _ => return false,
        }
        true
    }

    fn color_binding(&mut self, id: String, binding: &ColorBinding) -> bool {
        match binding {
            ColorBinding::Spectrum {
                point_index,
                base_value,
                period,
                base_color,
            } => {
                let parameter_id = format!("{id}:value");
                if !self.point_parameter(parameter_id.clone(), *point_index) {
                    return false;
                }
                self.derived(
                    id,
                    ObjectOp::SpectrumColor {
                        base_value: *base_value,
                        period: *period,
                        base_color: *base_color,
                    },
                    [parameter_id],
                );
            }
            ColorBinding::Rgb {
                red_point_index,
                green_point_index,
                blue_point_index,
                alpha,
            } => {
                let red_id = format!("{id}:red");
                let green_id = format!("{id}:green");
                let blue_id = format!("{id}:blue");
                if !self.point_parameter(red_id.clone(), *red_point_index)
                    || !self.point_parameter(green_id.clone(), *green_point_index)
                    || !self.point_parameter(blue_id.clone(), *blue_point_index)
                {
                    return false;
                }
                self.derived(
                    id,
                    ObjectOp::RgbColor { alpha: *alpha },
                    [red_id, green_id, blue_id],
                );
            }
            ColorBinding::Hsb {
                hue_point_index,
                saturation_point_index,
                brightness_point_index,
                alpha,
            } => {
                let hue_id = format!("{id}:hue");
                let saturation_id = format!("{id}:saturation");
                let brightness_id = format!("{id}:brightness");
                if !self.point_parameter(hue_id.clone(), *hue_point_index)
                    || !self.point_parameter(saturation_id.clone(), *saturation_point_index)
                    || !self.point_parameter(brightness_id.clone(), *brightness_point_index)
                {
                    return false;
                }
                self.derived(
                    id,
                    ObjectOp::HsbColor { alpha: *alpha },
                    [hue_id, saturation_id, brightness_id],
                );
            }
        }
        true
    }

    fn point_at_parameter(&mut self, id: String, point_index: usize, parameter_id: String) -> bool {
        let Some(constraint) = self.point_constraints.get(point_index).cloned() else {
            return false;
        };
        match constraint {
            ScenePointConstraint::OnSegment {
                start_index,
                end_index,
                ..
            } => {
                let domain_id = format!("domain:{id}");
                self.derived(
                    domain_id.clone(),
                    ObjectOp::Line {
                        line_kind: LineKind::Segment,
                    },
                    [point_id(start_index), point_id(end_index)],
                );
                self.derived(id, ObjectOp::PointOnLine, [domain_id, parameter_id]);
            }
            ScenePointConstraint::OnLine {
                start_index,
                end_index,
                ..
            } => {
                let domain_id = format!("domain:{id}");
                self.derived(
                    domain_id.clone(),
                    ObjectOp::Line {
                        line_kind: LineKind::Line,
                    },
                    [point_id(start_index), point_id(end_index)],
                );
                self.derived(id, ObjectOp::PointOnLine, [domain_id, parameter_id]);
            }
            ScenePointConstraint::OnRay {
                start_index,
                end_index,
                ..
            } => {
                let domain_id = format!("domain:{id}");
                self.derived(
                    domain_id.clone(),
                    ObjectOp::Line {
                        line_kind: LineKind::Ray,
                    },
                    [point_id(start_index), point_id(end_index)],
                );
                self.derived(id, ObjectOp::PointOnLine, [domain_id, parameter_id]);
            }
            ScenePointConstraint::OnLineConstraint { line, .. }
            | ScenePointConstraint::OnRayConstraint { line, .. } => {
                let domain_id = format!("domain:{id}");
                if !self.line_constraint(domain_id.clone(), &line) {
                    return false;
                }
                self.derived(id, ObjectOp::PointOnLine, [domain_id, parameter_id]);
            }
            ScenePointConstraint::OnCircle {
                center_index,
                radius_index,
                ..
            } => {
                let circle_id = format!("domain:{id}");
                self.derived(
                    circle_id.clone(),
                    ObjectOp::CircleByPoints,
                    [point_id(center_index), point_id(radius_index)],
                );
                self.derived(
                    id,
                    ObjectOp::PointOnCircleParameter {
                        invert_y: !self.y_up,
                    },
                    [circle_id, parameter_id],
                );
            }
            ScenePointConstraint::OnCircularConstraint { circle, .. } => {
                let circle_id = format!("domain:{id}");
                if !self.circular_constraint(circle_id.clone(), &circle) {
                    return false;
                }
                self.derived(
                    id,
                    ObjectOp::PointOnCircleParameter {
                        invert_y: !self.y_up,
                    },
                    [circle_id, parameter_id],
                );
            }
            ScenePointConstraint::OnPolygonBoundary { vertex_indices, .. }
            | ScenePointConstraint::OnPolygonBoundaryParameter { vertex_indices, .. } => {
                if vertex_indices.len() < 2 {
                    return false;
                }
                self.derived(
                    id,
                    ObjectOp::PointOnPolygonBoundary,
                    vertex_indices
                        .into_iter()
                        .map(point_id)
                        .chain(std::iter::once(parameter_id)),
                );
            }
            ScenePointConstraint::OnTranslatedPolygonBoundary {
                vertex_indices,
                vector_start_index,
                vector_end_index,
                ..
            } => {
                if vertex_indices.len() < 2 {
                    return false;
                }
                let translated_vertices = self.translated_polygon_vertices(
                    &id,
                    &vertex_indices,
                    vector_start_index,
                    vector_end_index,
                );
                self.derived(
                    id,
                    ObjectOp::PointOnPolygonBoundary,
                    translated_vertices
                        .into_iter()
                        .chain(std::iter::once(parameter_id)),
                );
            }
            ScenePointConstraint::OnCircleArc {
                center_index,
                start_index,
                end_index,
                ..
            } => {
                let arc_id = format!("domain:{id}");
                self.derived(
                    arc_id.clone(),
                    ObjectOp::CenterArc { y_up: self.y_up },
                    [
                        point_id(center_index),
                        point_id(start_index),
                        point_id(end_index),
                    ],
                );
                self.derived(id, ObjectOp::PointOnArc, [arc_id, parameter_id]);
            }
            ScenePointConstraint::OnArc {
                start_index,
                mid_index,
                end_index,
                ..
            } => {
                let arc_id = format!("domain:{id}");
                self.derived(
                    arc_id.clone(),
                    ObjectOp::ThreePointArc { complement: false },
                    [
                        point_id(start_index),
                        point_id(mid_index),
                        point_id(end_index),
                    ],
                );
                self.derived(id, ObjectOp::PointOnArc, [arc_id, parameter_id]);
            }
            ScenePointConstraint::OnArcConstraint { arc, .. } => {
                let arc_id = format!("domain:{id}");
                if !self.arc_constraint(arc_id.clone(), &arc) {
                    return false;
                }
                self.derived(id, ObjectOp::PointOnArc, [arc_id, parameter_id]);
            }
            ScenePointConstraint::OnPolyline { function_key, .. } => {
                let mut matching_lines = self
                    .line_group_ordinals
                    .iter()
                    .enumerate()
                    .filter_map(|(line_index, group_ordinal)| {
                        (*group_ordinal == Some(function_key)).then_some(line_index)
                    })
                    .collect::<Vec<_>>();
                matching_lines.extend(self.function_lines.iter().filter_map(
                    |(line_index, function)| {
                        (function.plot_key == Some(function_key)).then_some(*line_index)
                    },
                ));
                matching_lines.sort_unstable();
                matching_lines.dedup();
                let [line_index] = matching_lines.as_slice() else {
                    return false;
                };
                self.derived(
                    id,
                    ObjectOp::PointOnPolyline,
                    [line_id(*line_index), parameter_id],
                );
            }
            _ => return false,
        }
        true
    }

    fn translated_polygon_vertices(
        &mut self,
        owner_id: &str,
        vertex_indices: &[usize],
        vector_start_index: usize,
        vector_end_index: usize,
    ) -> Vec<String> {
        vertex_indices
            .iter()
            .enumerate()
            .map(|(ordinal, vertex_index)| {
                let vertex_id = format!("domain:{owner_id}:translated-vertex:{ordinal}");
                self.derived(
                    vertex_id.clone(),
                    ObjectOp::TransformObject {
                        transform: ObjectTransform::TranslateByVector,
                    },
                    [
                        point_id(*vertex_index),
                        point_id(vector_start_index),
                        point_id(vector_end_index),
                    ],
                );
                vertex_id
            })
            .collect()
    }

    fn scale_scalar(&mut self, point_index: usize, binding: &ScenePointBinding) -> Option<String> {
        let ScenePointBinding::Scale {
            factor,
            parameter_name,
            factor_expr,
            factor_parameter_group_ordinals,
            factor_parameter_point_index,
            factor_parameter_start_index,
            factor_parameter_end_index,
            ..
        } = binding
        else {
            return None;
        };
        let id = format!("scalar:point:{point_index}:scale-factor");
        if let (Some(point_index), Some(start_index), Some(end_index)) = (
            factor_parameter_point_index,
            factor_parameter_start_index,
            factor_parameter_end_index,
        ) {
            self.point_line_parameter(
                id.clone(),
                *point_index,
                *start_index,
                *end_index,
                LineKind::Segment,
            );
        } else if let Some(expression) = factor_expr {
            self.expression_with_group_sources(
                id.clone(),
                expression,
                factor_parameter_group_ordinals,
            );
        } else if let Some(name) = parameter_name {
            let parent = self.named_scalars.get(name)?.clone();
            self.derived(id.clone(), ObjectOp::Copy, [parent]);
        } else {
            self.source(id.clone(), ObjectValue::Scalar { value: *factor });
        }
        Some(id)
    }

    fn point_on_line(
        &mut self,
        point_index: usize,
        id: String,
        start_index: usize,
        end_index: usize,
        t: f64,
        line_kind: LineKind,
    ) {
        let domain_id = format!("domain:{id}");
        let parameter_id = format!("control:{id}:t");
        self.derived(
            domain_id.clone(),
            ObjectOp::Line { line_kind },
            [point_id(start_index), point_id(end_index)],
        );
        self.point_control_source(
            parameter_id.clone(),
            t,
            point_index,
            ScenePointControl::Parameter,
        );
        self.derived(id, ObjectOp::PointOnLine, [domain_id, parameter_id]);
    }

    fn point_on_domain_line(&mut self, point_index: usize, id: String, domain_id: String, t: f64) {
        let parameter_id = format!("control:{id}:t");
        self.point_control_source(
            parameter_id.clone(),
            t,
            point_index,
            ScenePointControl::Parameter,
        );
        self.derived(id, ObjectOp::PointOnLine, [domain_id, parameter_id]);
    }

    fn point_on_circle(
        &mut self,
        point_index: usize,
        id: String,
        domain_id: String,
        unit_x: f64,
        unit_y: f64,
        invert_y: bool,
    ) {
        let unit_x_id = format!("control:{id}:unit-x");
        let unit_y_id = format!("control:{id}:unit-y");
        self.point_control_source(
            unit_x_id.clone(),
            unit_x,
            point_index,
            ScenePointControl::UnitX,
        );
        self.point_control_source(
            unit_y_id.clone(),
            unit_y,
            point_index,
            ScenePointControl::UnitY,
        );
        self.derived(
            id,
            ObjectOp::PointOnCircle { invert_y },
            [domain_id, unit_x_id, unit_y_id],
        );
    }

    fn line_constraint(&mut self, id: String, constraint: &LineConstraint) -> bool {
        match constraint {
            LineConstraint::Segment {
                start_index,
                end_index,
            } => self.derived(
                id,
                ObjectOp::Line {
                    line_kind: LineKind::Segment,
                },
                [point_id(*start_index), point_id(*end_index)],
            ),
            LineConstraint::Line {
                start_index,
                end_index,
            } => self.derived(
                id,
                ObjectOp::Line {
                    line_kind: LineKind::Line,
                },
                [point_id(*start_index), point_id(*end_index)],
            ),
            LineConstraint::Ray {
                start_index,
                end_index,
            } => self.derived(
                id,
                ObjectOp::Line {
                    line_kind: LineKind::Ray,
                },
                [point_id(*start_index), point_id(*end_index)],
            ),
            LineConstraint::PerpendicularLine {
                through_index,
                line_start_index,
                line_end_index,
            } => {
                let base_id = format!("{id}:base");
                self.derived(
                    base_id.clone(),
                    ObjectOp::Line {
                        line_kind: LineKind::Line,
                    },
                    [point_id(*line_start_index), point_id(*line_end_index)],
                );
                self.derived(
                    id,
                    ObjectOp::PerpendicularLine,
                    [point_id(*through_index), base_id],
                );
            }
            LineConstraint::ParallelLine {
                through_index,
                line_start_index,
                line_end_index,
            } => {
                let base_id = format!("{id}:base");
                self.derived(
                    base_id.clone(),
                    ObjectOp::Line {
                        line_kind: LineKind::Line,
                    },
                    [point_id(*line_start_index), point_id(*line_end_index)],
                );
                self.derived(
                    id,
                    ObjectOp::ParallelLine,
                    [point_id(*through_index), base_id],
                );
            }
            LineConstraint::PerpendicularTo {
                through_index,
                line,
            }
            | LineConstraint::ParallelTo {
                through_index,
                line,
            } => {
                let base_id = format!("{id}:base");
                if !self.line_constraint(base_id.clone(), line) {
                    return false;
                }
                self.derived(
                    id,
                    if matches!(constraint, LineConstraint::PerpendicularTo { .. }) {
                        ObjectOp::PerpendicularLine
                    } else {
                        ObjectOp::ParallelLine
                    },
                    [point_id(*through_index), base_id],
                );
            }
            LineConstraint::AngleBisectorRay {
                start_index,
                vertex_index,
                end_index,
            } => self.derived(
                id,
                ObjectOp::AngleBisectorRay,
                [
                    point_id(*start_index),
                    point_id(*vertex_index),
                    point_id(*end_index),
                ],
            ),
            LineConstraint::Translated {
                line,
                vector_start_index,
                vector_end_index,
            } => {
                let base_id = format!("{id}:base");
                if !self.line_constraint(base_id.clone(), line) {
                    return false;
                }
                self.derived(
                    id,
                    ObjectOp::TransformObject {
                        transform: ObjectTransform::TranslateByVector,
                    },
                    [
                        base_id,
                        point_id(*vector_start_index),
                        point_id(*vector_end_index),
                    ],
                );
            }
            LineConstraint::TranslatedDelta { line, dx, dy } => {
                let base_id = format!("{id}:base");
                if !self.line_constraint(base_id.clone(), line) {
                    return false;
                }
                self.derived(
                    id,
                    ObjectOp::TransformObject {
                        transform: ObjectTransform::TranslateDelta { dx: *dx, dy: *dy },
                    },
                    [base_id],
                );
            }
            LineConstraint::Reflected { line, axis } => {
                let base_id = format!("{id}:base");
                let axis_id = format!("{id}:axis");
                if !self.line_constraint(base_id.clone(), line)
                    || !self.line_constraint(axis_id.clone(), axis)
                {
                    return false;
                }
                self.derived(
                    id,
                    ObjectOp::TransformObject {
                        transform: ObjectTransform::ReflectByLine,
                    },
                    [base_id, axis_id],
                );
            }
            LineConstraint::Rotated { line, rotation } => {
                let base_id = format!("{id}:base");
                if !self.line_constraint(base_id.clone(), line) {
                    return false;
                }
                let Some(angle_id) = self.shape_rotation_scalar(&id, rotation) else {
                    return false;
                };
                self.derived(
                    id,
                    ObjectOp::TransformObject {
                        transform: ObjectTransform::RotateDegrees,
                    },
                    [base_id, point_id(rotation.center_index), angle_id],
                );
            }
        }
        true
    }

    fn circular_constraint(&mut self, id: String, constraint: &CircularConstraint) -> bool {
        match constraint {
            CircularConstraint::Circle {
                center_index,
                radius_index,
            } => self.derived(
                id,
                ObjectOp::CircleByPoints,
                [point_id(*center_index), point_id(*radius_index)],
            ),
            CircularConstraint::SegmentRadiusCircle {
                center_index,
                line_start_index,
                line_end_index,
            } => self.derived(
                id,
                ObjectOp::CircleBySegmentRadius,
                [
                    point_id(*center_index),
                    point_id(*line_start_index),
                    point_id(*line_end_index),
                ],
            ),
            CircularConstraint::ParameterRadiusCircle {
                center_index,
                parameter_name,
                raw_per_unit,
                ..
            } => {
                if !self.parameter_radius_circle(id, *center_index, parameter_name, *raw_per_unit) {
                    return false;
                }
            }
            CircularConstraint::ExpressionRadiusCircle {
                center_index,
                expr,
                parameter_group_ordinals,
                ..
            } => self.expression_radius_circle(id, *center_index, expr, parameter_group_ordinals),
            CircularConstraint::TranslateCircle { source, dx, dy } => {
                let source_id = format!("{id}:source");
                if !self.circular_constraint(source_id.clone(), source) {
                    return false;
                }
                self.derived(
                    id,
                    ObjectOp::TransformObject {
                        transform: ObjectTransform::TranslateDelta { dx: *dx, dy: *dy },
                    },
                    [source_id],
                );
            }
            CircularConstraint::VectorTranslateCircle {
                source,
                vector_start_index,
                vector_end_index,
            } => {
                let source_id = format!("{id}:source");
                if !self.circular_constraint(source_id.clone(), source) {
                    return false;
                }
                self.derived(
                    id,
                    ObjectOp::TransformObject {
                        transform: ObjectTransform::TranslateByVector,
                    },
                    [
                        source_id,
                        point_id(*vector_start_index),
                        point_id(*vector_end_index),
                    ],
                );
            }
            CircularConstraint::ReflectCircle {
                source,
                line_start_index,
                line_end_index,
                line_index,
            } => {
                let source_id = format!("{id}:source");
                let axis_id = format!("{id}:axis");
                if !self.circular_constraint(source_id.clone(), source) {
                    return false;
                }
                let Some(axis_id) =
                    self.axis_line_parent(axis_id, *line_start_index, *line_end_index, *line_index)
                else {
                    return false;
                };
                self.derived(
                    id,
                    ObjectOp::TransformObject {
                        transform: ObjectTransform::ReflectByLine,
                    },
                    [source_id, axis_id],
                );
            }
            CircularConstraint::ScaleCircle {
                source,
                center_index,
                factor,
            } => {
                let source_id = format!("{id}:source");
                if !self.circular_constraint(source_id.clone(), source) {
                    return false;
                }
                self.derived(
                    id,
                    ObjectOp::TransformObject {
                        transform: ObjectTransform::Scale { factor: *factor },
                    },
                    [source_id, point_id(*center_index)],
                );
            }
            CircularConstraint::RotateCircle {
                source,
                center_index,
                angle_degrees,
            } => {
                let source_id = format!("{id}:source");
                if !self.circular_constraint(source_id.clone(), source) {
                    return false;
                }
                let angle_id = format!("scalar:{id}:rotation-degrees");
                self.source(
                    angle_id.clone(),
                    ObjectValue::Scalar {
                        value: *angle_degrees,
                    },
                );
                self.derived(
                    id,
                    ObjectOp::TransformObject {
                        transform: ObjectTransform::RotateDegrees,
                    },
                    [source_id, point_id(*center_index), angle_id],
                );
            }
            CircularConstraint::CircleArc {
                center_index,
                start_index,
                end_index,
            } => self.derived(
                id,
                ObjectOp::CenterArc { y_up: self.y_up },
                [
                    point_id(*center_index),
                    point_id(*start_index),
                    point_id(*end_index),
                ],
            ),
            CircularConstraint::ThreePointArc {
                start_index,
                mid_index,
                end_index,
            } => self.derived(
                id,
                ObjectOp::ThreePointArc { complement: false },
                [
                    point_id(*start_index),
                    point_id(*mid_index),
                    point_id(*end_index),
                ],
            ),
        }
        true
    }

    fn line(&mut self, index: usize, line: &crate::runtime::scene::LineShape) {
        let id = line_id(index);
        let value = ObjectValue::Points {
            points: line.points.iter().map(core_point).collect(),
        };
        if let Some(function) = self.function_lines.get(&index).cloned() {
            let (parameter_names, parents): (Vec<_>, Vec<_>) =
                expression_parameter_names(&function.expr)
                    .into_iter()
                    .filter_map(|name| {
                        function
                            .parameter_group_ordinals
                            .get(&name)
                            .and_then(|ordinal| self.group_scalars.get(ordinal))
                            .cloned()
                            .or_else(|| self.named_scalars.get(&name).cloned())
                            .map(|parent| (name, parent))
                    })
                    .unzip();
            self.derived(
                id,
                ObjectOp::FunctionPlot {
                    expression: ObjectExpression::from_function_expr(&function.expr),
                    parameter_names,
                    value_min: function.domain.x_min,
                    value_max: function.domain.x_max,
                    sample_count: function.domain.sample_count,
                    plot_mode: match function.domain.mode {
                        crate::runtime::functions::FunctionPlotMode::Cartesian => {
                            PlotMode::Cartesian
                        }
                        crate::runtime::functions::FunctionPlotMode::Polar => PlotMode::Polar,
                    },
                },
                parents,
            );
            return;
        }
        match &line.binding {
            Some(LineBinding::GraphHelperLine {
                start_index,
                end_index,
            })
            | Some(LineBinding::Segment {
                start_index,
                end_index,
            }) => self.derived(
                id,
                ObjectOp::Line {
                    line_kind: LineKind::Segment,
                },
                [point_id(*start_index), point_id(*end_index)],
            ),
            Some(LineBinding::Line {
                start_index,
                end_index,
            }) => self.derived(
                id,
                ObjectOp::Line {
                    line_kind: LineKind::Line,
                },
                [point_id(*start_index), point_id(*end_index)],
            ),
            Some(LineBinding::Ray {
                start_index,
                end_index,
            }) => self.derived(
                id,
                ObjectOp::Line {
                    line_kind: LineKind::Ray,
                },
                [point_id(*start_index), point_id(*end_index)],
            ),
            Some(LineBinding::AngleBisectorRay {
                start_index,
                vertex_index,
                end_index,
            }) => self.derived(
                id,
                ObjectOp::AngleBisectorRay,
                [
                    point_id(*start_index),
                    point_id(*vertex_index),
                    point_id(*end_index),
                ],
            ),
            Some(LineBinding::AngleMarker {
                start_index,
                vertex_index,
                end_index,
                marker_class,
            }) => self.derived(
                id,
                ObjectOp::AngleMarker {
                    marker_class: *marker_class,
                },
                [
                    point_id(*start_index),
                    point_id(*vertex_index),
                    point_id(*end_index),
                ],
            ),
            Some(LineBinding::SegmentMarker {
                start_index,
                end_index,
                t,
                marker_class,
            }) => self.derived(
                id,
                ObjectOp::SegmentMarker {
                    t: *t,
                    marker_class: *marker_class,
                },
                [point_id(*start_index), point_id(*end_index)],
            ),
            Some(LineBinding::PerpendicularLine {
                through_index,
                line_start_index,
                line_end_index,
                line_index,
            }) => {
                if let Some(axis_id) = self.axis_line_parent(
                    format!("domain:{id}:axis"),
                    *line_start_index,
                    *line_end_index,
                    *line_index,
                ) {
                    self.derived(
                        id,
                        ObjectOp::PerpendicularLine,
                        [point_id(*through_index), axis_id],
                    );
                } else {
                    self.pending_source(id, "line-binding", value);
                }
            }
            Some(LineBinding::ParallelLine {
                through_index,
                line_start_index,
                line_end_index,
                line_index,
            }) => {
                if let Some(axis_id) = self.axis_line_parent(
                    format!("domain:{id}:axis"),
                    *line_start_index,
                    *line_end_index,
                    *line_index,
                ) {
                    self.derived(
                        id,
                        ObjectOp::ParallelLine,
                        [point_id(*through_index), axis_id],
                    );
                } else {
                    self.pending_source(id, "line-binding", value);
                }
            }
            Some(LineBinding::DerivedTransform {
                source_index,
                transform,
            }) => {
                if !self.geometry_transform(id.clone(), line_id(*source_index), transform) {
                    self.pending_source(id, "line-binding", value);
                }
            }
            Some(LineBinding::ArcBoundary {
                boundary_kind,
                center_index,
                start_index,
                mid_index,
                end_index,
                reversed,
                complement,
                ..
            }) => {
                if !self.arc_boundary_points(
                    id.clone(),
                    *boundary_kind,
                    *center_index,
                    *start_index,
                    *mid_index,
                    *end_index,
                    *reversed,
                    *complement,
                ) {
                    self.pending_source(id, "line-binding", value);
                }
            }
            Some(LineBinding::CoordinateTrace {
                point_index,
                parameter_group_ordinal,
                x_min,
                x_max,
                sample_count,
            }) => {
                if !self.coordinate_trace(id.clone(), *point_index, *x_min, *x_max, *sample_count)
                    && !self.scalar_point_trace(
                        id.clone(),
                        *point_index,
                        *parameter_group_ordinal,
                        *x_min,
                        *x_max,
                        *sample_count,
                    )
                {
                    self.pending_source(id, "line-binding", value);
                }
            }
            Some(LineBinding::CustomTransformTrace {
                point_index,
                driver_index,
                x_min,
                x_max,
                sample_count,
            }) => {
                if !self.custom_transform_trace(
                    id.clone(),
                    *point_index,
                    *driver_index,
                    *x_min,
                    *x_max,
                    *sample_count,
                ) {
                    self.pending_source(id, "line-binding", value);
                }
            }
            Some(LineBinding::PointTrace {
                point_index,
                driver_index,
                x_min,
                x_max,
                sample_count,
            }) => {
                if !self.point_trace(
                    id.clone(),
                    *point_index,
                    *driver_index,
                    *x_min,
                    *x_max,
                    *sample_count,
                ) {
                    self.pending_source(id, "line-binding", value);
                }
            }
            Some(LineBinding::SegmentTrace {
                start_index,
                end_index,
                driver_index,
                x_min,
                x_max,
                sample_count,
            }) => {
                let start_trace_id = format!("{id}:start-trace");
                let end_trace_id = format!("{id}:end-trace");
                if self.point_trace_or_repeat(
                    start_trace_id.clone(),
                    *start_index,
                    *driver_index,
                    *x_min,
                    *x_max,
                    *sample_count,
                ) && self.point_trace_or_repeat(
                    end_trace_id.clone(),
                    *end_index,
                    *driver_index,
                    *x_min,
                    *x_max,
                    *sample_count,
                ) {
                    self.derived(id, ObjectOp::ZipPointTraces, [start_trace_id, end_trace_id]);
                } else {
                    self.pending_source(id, "line-binding", value);
                }
            }
            Some(LineBinding::ColorizedSpectrum {
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
            }) => {
                let parameter_id = format!("scalar:{id}:trace-parameter");
                if !self.point_parameter(parameter_id.clone(), *point_index) {
                    self.pending_source(id, "line-binding", value);
                    return;
                }
                let depth_id = if let Some(name) = depth_parameter_name {
                    let Some(depth_id) = self.named_scalars.get(name).cloned() else {
                        self.pending_source(id, "line-binding", value);
                        return;
                    };
                    depth_id
                } else {
                    let depth_id = format!("scalar:{id}:depth");
                    self.source(
                        depth_id.clone(),
                        ObjectValue::Scalar {
                            value: *depth as f64,
                        },
                    );
                    depth_id
                };
                let reflected = match (reflection_source_index, reflection_axis_line_index) {
                    (Some(_), Some(_)) => true,
                    (None, None) => false,
                    _ => {
                        self.pending_source(id, "line-binding", value);
                        return;
                    }
                };
                let sampled_reflection_axis =
                    match (reflection_focus_index, reflection_directrix_line_index) {
                        (Some(_), Some(_)) if reflected => true,
                        (None, None) => false,
                        _ => {
                            self.pending_source(id, "line-binding", value);
                            return;
                        }
                    };
                let mut parents = vec![
                    line_id(*line_index),
                    line_id(*trace_line_index),
                    parameter_id,
                    depth_id,
                ];
                if let (Some(source_index), Some(axis_line_index)) =
                    (reflection_source_index, reflection_axis_line_index)
                {
                    parents.push(point_id(*source_index));
                    parents.push(line_id(*axis_line_index));
                }
                if let (Some(focus_index), Some(directrix_line_index)) =
                    (reflection_focus_index, reflection_directrix_line_index)
                {
                    parents.push(point_id(*focus_index));
                    parents.push(line_id(*directrix_line_index));
                }
                self.derived(
                    id,
                    ObjectOp::ColorizedSpectrumLine {
                        trace_endpoint_index: *trace_endpoint_index,
                        step_index: *step_index,
                        ray: *ray,
                        reflected,
                        sampled_reflection_axis,
                    },
                    parents,
                );
            }
            Some(LineBinding::ParametricCurve {
                x_expr,
                y_expr,
                x_min,
                x_max,
                sample_count,
            }) => {
                let mut parameter_names = expression_parameter_names(x_expr);
                parameter_names.extend(expression_parameter_names(y_expr));
                parameter_names.sort();
                parameter_names.dedup();
                let parents = parameter_names
                    .iter()
                    .filter_map(|name| self.named_scalars.get(name).cloned())
                    .collect::<Vec<_>>();
                if parents.len() != parameter_names.len() {
                    self.pending_source(id, "line-binding", value);
                } else {
                    self.derived(
                        id,
                        ObjectOp::ParametricCurve {
                            x_expression: ObjectExpression::from_function_expr(x_expr),
                            y_expression: ObjectExpression::from_function_expr(y_expr),
                            parameter_names,
                            value_min: *x_min,
                            value_max: *x_max,
                            sample_count: *sample_count,
                        },
                        parents,
                    );
                }
            }
            None => {
                let line_kind = line
                    .debug
                    .as_ref()
                    .and_then(|debug| payload_line_kind(debug.group_kind));
                let value = line_kind
                    .zip(line.points.first().zip(line.points.last()))
                    .map_or(value, |(line_kind, (start, end))| ObjectValue::Line {
                        line_kind,
                        start: core_point(start),
                        end: core_point(end),
                    });
                self.bound_source(
                    id,
                    value,
                    SceneObjectSourceBinding::Line {
                        line_index: index,
                        line_kind,
                    },
                );
            }
        }
    }

    fn point_trace(
        &mut self,
        id: String,
        point_index: usize,
        driver_index: usize,
        value_min: f64,
        value_max: f64,
        sample_count: usize,
    ) -> bool {
        let Some(driver) = self.trace_driver(driver_index) else {
            return false;
        };
        let driver_source_ids = trace_driver_source_ids(&driver);
        let target_id = point_id(point_index);
        let Some(program) = self.object_program(&target_id, &driver_source_ids) else {
            return false;
        };
        let parents = program.source_ids.clone();
        self.derived(
            id,
            ObjectOp::PointTrace {
                program,
                driver,
                value_min,
                value_max,
                sample_count,
            },
            parents,
        );
        true
    }

    fn scalar_point_trace(
        &mut self,
        id: String,
        point_index: usize,
        parameter_group_ordinal: usize,
        value_min: f64,
        value_max: f64,
        sample_count: usize,
    ) -> bool {
        let Some(source_id) = self.group_scalars.get(&parameter_group_ordinal).cloned() else {
            return false;
        };
        let driver = TraceDriver::Scalar {
            source_id: source_id.clone(),
            normalized: false,
        };
        let target_id = point_id(point_index);
        let Some(program) = self.object_program(&target_id, std::slice::from_ref(&source_id))
        else {
            return false;
        };
        let parents = program.source_ids.clone();
        self.derived(
            id,
            ObjectOp::PointTrace {
                program,
                driver,
                value_min,
                value_max,
                sample_count,
            },
            parents,
        );
        true
    }

    fn point_trace_or_repeat(
        &mut self,
        id: String,
        point_index: usize,
        driver_index: usize,
        value_min: f64,
        value_max: f64,
        sample_count: usize,
    ) -> bool {
        if self.point_trace(
            id.clone(),
            point_index,
            driver_index,
            value_min,
            value_max,
            sample_count,
        ) {
            return true;
        }
        let Some(driver) = self.trace_driver(driver_index) else {
            return false;
        };
        let driver_source_ids = trace_driver_source_ids(&driver);
        let target_id = point_id(point_index);
        let Some(program) = self.object_program(&target_id, &[]) else {
            return false;
        };
        if program
            .nodes
            .iter()
            .any(|node| driver_source_ids.contains(&node.id))
        {
            return false;
        }
        self.derived(id, ObjectOp::RepeatPoint { sample_count }, [target_id]);
        true
    }

    fn trace_driver(&self, driver_index: usize) -> Option<TraceDriver> {
        let Some(driver_constraint) = self.point_constraints.get(driver_index) else {
            return None;
        };
        let scalar_driver_id = self
            .point_bindings
            .get(driver_index)
            .and_then(Option::as_ref)
            .and_then(|binding| match binding {
                ScenePointBinding::DerivedParameter { .. } => Some(format!(
                    "scalar:{}:derived-parameter",
                    point_id(driver_index)
                )),
                ScenePointBinding::ConstraintParameterExpr { .. }
                | ScenePointBinding::ConstraintParameterFromPointExpr { .. } => Some(format!(
                    "scalar:{}:constraint-parameter",
                    point_id(driver_index)
                )),
                _ => None,
            });
        let scalar_driver = |source_id: String, normalized| TraceDriver::Scalar {
            source_id,
            normalized,
        };
        Some(match driver_constraint {
            ScenePointConstraint::OnSegment { .. } => TraceDriver::Scalar {
                source_id: scalar_driver_id
                    .unwrap_or_else(|| format!("control:{}:t", point_id(driver_index))),
                normalized: true,
            },
            ScenePointConstraint::OnLine { .. }
            | ScenePointConstraint::OnRay { .. }
            | ScenePointConstraint::OnLineConstraint { .. }
            | ScenePointConstraint::OnRayConstraint { .. } => scalar_driver(
                scalar_driver_id.unwrap_or_else(|| format!("control:{}:t", point_id(driver_index))),
                false,
            ),
            ScenePointConstraint::OnArc { .. } | ScenePointConstraint::OnCircleArc { .. } => {
                scalar_driver(
                    scalar_driver_id
                        .unwrap_or_else(|| format!("control:{}:t", point_id(driver_index))),
                    true,
                )
            }
            ScenePointConstraint::OnPolyline { .. } => scalar_driver(
                scalar_driver_id.unwrap_or_else(|| format!("control:{}:t", point_id(driver_index))),
                true,
            ),
            ScenePointConstraint::OnPolygonBoundary { .. } => {
                scalar_driver(format!("control:{}:boundary", point_id(driver_index)), true)
            }
            ScenePointConstraint::OnCircle { .. }
            | ScenePointConstraint::OnCircularConstraint { .. } => {
                if let Some(source_id) = scalar_driver_id {
                    scalar_driver(source_id, true)
                } else {
                    TraceDriver::Circle {
                        unit_x_source_id: format!("control:{}:unit-x", point_id(driver_index)),
                        unit_y_source_id: format!("control:{}:unit-y", point_id(driver_index)),
                    }
                }
            }
            _ => return None,
        })
    }

    fn object_program(
        &self,
        target_id: &str,
        overridden_inputs: &[String],
    ) -> Option<ObjectProgram> {
        let nodes_by_id = self
            .nodes
            .iter()
            .map(|node| (node.id.as_str(), node))
            .collect::<BTreeMap<_, _>>();
        let mut required = std::collections::BTreeSet::new();
        let mut stack = vec![target_id.to_string()];
        while let Some(id) = stack.pop() {
            if !required.insert(id.clone()) {
                continue;
            }
            let node = nodes_by_id.get(id.as_str())?;
            if !overridden_inputs.contains(&id) {
                stack.extend(node.parents().iter().cloned());
            }
        }
        if !overridden_inputs
            .iter()
            .all(|input_id| required.contains(input_id))
        {
            return None;
        }
        let mut nodes = self
            .nodes
            .iter()
            .filter(|node| required.contains(&node.id))
            .cloned()
            .map(|mut node| {
                if overridden_inputs.contains(&node.id) {
                    node.definition = ObjectDefinition::Source;
                }
                node
            })
            .collect::<Vec<_>>();
        fuse_generated_trace_points(&mut nodes);
        retain_program_dependencies(&mut nodes, target_id)?;
        let source_ids = nodes
            .iter()
            .filter(|node| matches!(node.definition, ObjectDefinition::Source))
            .map(|node| node.id.clone())
            .filter(|source_id| !overridden_inputs.contains(source_id))
            .collect();
        Some(ObjectProgram {
            nodes,
            source_ids,
            target_id: target_id.to_string(),
        })
    }

    fn object_iteration_program(
        &self,
        output_id: &str,
        state_source_ids: &[String],
        state_target_ids: &[String],
    ) -> Option<ObjectIterationProgram> {
        if state_source_ids.is_empty() || state_source_ids.len() != state_target_ids.len() {
            return None;
        }
        let nodes_by_id = self
            .nodes
            .iter()
            .map(|node| (node.id.as_str(), node))
            .collect::<BTreeMap<_, _>>();
        let mut required = std::collections::BTreeSet::new();
        let mut stack = state_target_ids.to_vec();
        stack.push(output_id.to_string());
        while let Some(id) = stack.pop() {
            if !required.insert(id.clone()) {
                continue;
            }
            let node = nodes_by_id.get(id.as_str())?;
            if !state_source_ids.contains(&id) {
                stack.extend(node.parents().iter().cloned());
            }
        }
        for source_id in state_source_ids {
            if !nodes_by_id.contains_key(source_id.as_str()) {
                return None;
            }
            required.insert(source_id.clone());
        }
        let nodes = self
            .nodes
            .iter()
            .filter(|node| required.contains(&node.id))
            .cloned()
            .map(|mut node| {
                if state_source_ids.contains(&node.id) {
                    node.definition = ObjectDefinition::Source;
                }
                node
            })
            .collect::<Vec<_>>();
        let source_ids = nodes
            .iter()
            .filter(|node| matches!(node.definition, ObjectDefinition::Source))
            .map(|node| node.id.clone())
            .filter(|source_id| !state_source_ids.contains(source_id))
            .collect();
        Some(ObjectIterationProgram {
            nodes,
            source_ids,
            state_source_ids: state_source_ids.to_vec(),
            state_target_ids: state_target_ids.to_vec(),
            output_id: output_id.to_string(),
        })
    }

    fn coordinate_trace(
        &mut self,
        id: String,
        point_index: usize,
        value_min: f64,
        value_max: f64,
        sample_count: usize,
    ) -> bool {
        let Some(binding) = self
            .point_bindings
            .get(point_index)
            .and_then(Option::as_ref)
            .cloned()
        else {
            return false;
        };
        let (
            source_index,
            x_expression,
            y_expression,
            trace_parameter_name,
            parameter_group_ordinals,
            mode,
        ) = match binding {
            ScenePointBinding::Coordinate { name, expr } => {
                let mut parameter_names = expression_parameter_names(&expr);
                parameter_names.retain(|parameter_name| parameter_name != &name);
                let Some(parents) = parameter_names
                    .iter()
                    .map(|parameter_name| self.named_scalars.get(parameter_name).cloned())
                    .collect::<Option<Vec<_>>>()
                else {
                    return false;
                };
                self.derived(
                    id,
                    ObjectOp::CartesianParameterTrace {
                        expression: ObjectExpression::from_function_expr(&expr),
                        parameter_names,
                        trace_parameter_name: name,
                        value_min,
                        value_max,
                        sample_count,
                    },
                    parents,
                );
                return true;
            }
            ScenePointBinding::CoordinateSource {
                source_index,
                name,
                expr,
                parameter_group_ordinals,
                axis,
            } => (
                source_index,
                expr,
                None,
                name,
                parameter_group_ordinals,
                match axis {
                    CoordinateAxis::Horizontal => CoordinateTraceMode::Horizontal,
                    CoordinateAxis::Vertical => CoordinateTraceMode::Vertical,
                },
            ),
            ScenePointBinding::CoordinateSource2d {
                source_index,
                x_name,
                x_expr,
                y_name,
                y_expr,
                x_scalar_group_ordinal,
                y_scalar_group_ordinal,
            } if x_name == y_name => (
                source_index,
                x_expr,
                Some(y_expr),
                x_name.clone(),
                [x_scalar_group_ordinal, y_scalar_group_ordinal]
                    .into_iter()
                    .flatten()
                    .map(|ordinal| (x_name.clone(), ordinal))
                    .collect(),
                CoordinateTraceMode::TwoDimensional,
            ),
            _ => return false,
        };
        let mut parameter_names = expression_parameter_names(&x_expression);
        if let Some(y_expression) = &y_expression {
            parameter_names.extend(expression_parameter_names(y_expression));
        }
        parameter_names.sort();
        parameter_names.dedup();
        parameter_names.retain(|name| name != &trace_parameter_name);
        let Some(parents) = parameter_names
            .iter()
            .map(|name| {
                parameter_group_ordinals
                    .get(name)
                    .and_then(|ordinal| self.group_scalars.get(ordinal))
                    .or_else(|| self.named_scalars.get(name))
                    .cloned()
            })
            .collect::<Option<Vec<_>>>()
        else {
            return false;
        };
        self.derived(
            id,
            ObjectOp::CoordinateTrace {
                x_expression: ObjectExpression::from_function_expr(&x_expression),
                y_expression: y_expression
                    .as_ref()
                    .map(ObjectExpression::from_function_expr),
                parameter_names,
                trace_parameter_name,
                value_min,
                value_max,
                sample_count,
                mode,
            },
            std::iter::once(point_id(source_index)).chain(parents),
        );
        true
    }

    fn custom_transform_trace(
        &mut self,
        id: String,
        point_index: usize,
        driver_index: usize,
        value_min: f64,
        value_max: f64,
        sample_count: usize,
    ) -> bool {
        let Some(binding) = self
            .point_bindings
            .get(point_index)
            .and_then(Option::as_ref)
            .cloned()
        else {
            return false;
        };
        let ScenePointBinding::CustomTransform {
            source_index,
            origin_index,
            axis_end_index,
            distance_expr,
            angle_expr,
            distance_raw_scale,
            angle_degrees_scale,
        } = binding
        else {
            return self.point_trace(
                id,
                point_index,
                driver_index,
                value_min,
                value_max,
                sample_count,
            );
        };
        if source_index != driver_index {
            return false;
        }
        let parameter_id = format!("scalar:{id}:trace-parameter");
        if !self.point_parameter(parameter_id.clone(), driver_index) {
            return false;
        }
        self.derived(
            id,
            ObjectOp::CustomTransformTrace {
                distance_expression: ObjectExpression::from_function_expr(&distance_expr),
                angle_expression: ObjectExpression::from_function_expr(&angle_expr),
                distance_parameter_names: expression_parameter_names(&distance_expr),
                angle_parameter_names: expression_parameter_names(&angle_expr),
                value_min,
                value_max,
                sample_count,
                distance_scale: distance_raw_scale,
                angle_degrees_scale,
            },
            [
                point_id(origin_index),
                point_id(axis_end_index),
                parameter_id,
            ],
        );
        true
    }

    fn axis_line_parent(
        &mut self,
        id: String,
        start_index: Option<usize>,
        end_index: Option<usize>,
        line_index: Option<usize>,
    ) -> Option<String> {
        if let (Some(start_index), Some(end_index)) = (start_index, end_index) {
            self.derived(
                id.clone(),
                ObjectOp::Line {
                    line_kind: LineKind::Line,
                },
                [point_id(start_index), point_id(end_index)],
            );
            return Some(id);
        }
        line_index.map(line_id)
    }

    #[allow(clippy::too_many_arguments)]
    fn arc_boundary_points(
        &mut self,
        id: String,
        boundary_kind: ArcBoundaryKind,
        center_index: Option<usize>,
        start_index: usize,
        mid_index: Option<usize>,
        end_index: usize,
        reversed: bool,
        complement: bool,
    ) -> bool {
        let (center_arc, parents) = if let Some(center_index) = center_index {
            (
                true,
                vec![
                    point_id(center_index),
                    point_id(start_index),
                    point_id(end_index),
                ],
            )
        } else if let Some(mid_index) = mid_index {
            (
                false,
                vec![
                    point_id(start_index),
                    point_id(mid_index),
                    point_id(end_index),
                ],
            )
        } else {
            return false;
        };
        self.derived(
            id,
            ObjectOp::ArcBoundaryPoints {
                center_arc,
                sector: boundary_kind == ArcBoundaryKind::Sector,
                reversed,
                complement,
                steps: 48,
                y_up: self.y_up,
            },
            parents,
        );
        true
    }

    fn geometry_transform(
        &mut self,
        id: String,
        source_id: String,
        transform: &GeometryTransformBinding,
    ) -> bool {
        match transform {
            GeometryTransformBinding::TranslateDelta { dx, dy } => self.derived(
                id,
                ObjectOp::TransformObject {
                    transform: ObjectTransform::TranslateDelta { dx: *dx, dy: *dy },
                },
                [source_id],
            ),
            GeometryTransformBinding::TranslateVector {
                vector_start_index,
                vector_end_index,
            } => self.derived(
                id,
                ObjectOp::TransformObject {
                    transform: ObjectTransform::TranslateByVector,
                },
                [
                    source_id,
                    point_id(*vector_start_index),
                    point_id(*vector_end_index),
                ],
            ),
            GeometryTransformBinding::Rotate(rotation) => {
                let Some(angle_id) = self.shape_rotation_scalar(&id, rotation) else {
                    return false;
                };
                self.derived(
                    id,
                    ObjectOp::TransformObject {
                        transform: ObjectTransform::RotateDegrees,
                    },
                    [source_id, point_id(rotation.center_index), angle_id],
                );
            }
            GeometryTransformBinding::Scale(scale) => self.derived(
                id,
                ObjectOp::TransformObject {
                    transform: ObjectTransform::Scale {
                        factor: scale.factor,
                    },
                },
                [source_id, point_id(scale.center_index)],
            ),
            GeometryTransformBinding::Reflect(axis) => {
                let Some(axis_id) = self.axis_line_parent(
                    format!("domain:{id}:reflection-axis"),
                    axis.line_start_index,
                    axis.line_end_index,
                    axis.line_index,
                ) else {
                    return false;
                };
                self.derived(
                    id,
                    ObjectOp::TransformObject {
                        transform: ObjectTransform::ReflectByLine,
                    },
                    [source_id, axis_id],
                );
            }
        }
        true
    }

    fn arc_constraint(&mut self, id: String, arc: &ArcConstraint) -> bool {
        match arc {
            ArcConstraint::CenterArc {
                center_index,
                start_index,
                end_index,
            } => self.derived(
                id,
                ObjectOp::CenterArc { y_up: self.y_up },
                [
                    point_id(*center_index),
                    point_id(*start_index),
                    point_id(*end_index),
                ],
            ),
            ArcConstraint::CircleArc {
                circle,
                start_index,
                end_index,
            } => {
                let circle_id = format!("{id}:circle");
                if !self.circular_constraint(circle_id.clone(), circle) {
                    return false;
                }
                self.derived(
                    id,
                    ObjectOp::CircleArc { y_up: self.y_up },
                    [circle_id, point_id(*start_index), point_id(*end_index)],
                );
            }
            ArcConstraint::ThreePointArc {
                start_index,
                mid_index,
                end_index,
            } => self.derived(
                id,
                ObjectOp::ThreePointArc { complement: false },
                [
                    point_id(*start_index),
                    point_id(*mid_index),
                    point_id(*end_index),
                ],
            ),
            ArcConstraint::Reflected { arc, axis } => {
                let source_id = format!("{id}:source");
                let axis_id = format!("{id}:axis");
                if !self.arc_constraint(source_id.clone(), arc)
                    || !self.line_constraint(axis_id.clone(), axis)
                {
                    return false;
                }
                self.derived(
                    id,
                    ObjectOp::TransformObject {
                        transform: ObjectTransform::ReflectByLine,
                    },
                    [source_id, axis_id],
                );
            }
        }
        true
    }

    fn shape_rotation_scalar(
        &mut self,
        shape_id: &str,
        rotation: &RotationBinding,
    ) -> Option<String> {
        let id = format!("scalar:{shape_id}:rotation-degrees");
        if let (Some(start_index), Some(vertex_index), Some(end_index)) = (
            rotation.angle_start_index,
            rotation.angle_vertex_index,
            rotation.angle_end_index,
        ) {
            self.derived(
                id.clone(),
                ObjectOp::MeasuredRotationDegrees,
                [
                    point_id(start_index),
                    point_id(vertex_index),
                    point_id(end_index),
                ],
            );
        } else if let Some(expr) = &rotation.angle_expr {
            self.expression_with_group_sources(
                id.clone(),
                expr,
                &rotation.angle_parameter_group_ordinals,
            );
        } else if let Some(parameter_name) = &rotation.parameter_name {
            let parent = self.named_scalars.get(parameter_name)?.clone();
            self.derived(id.clone(), ObjectOp::Copy, [parent]);
        } else {
            self.source(
                id.clone(),
                ObjectValue::Scalar {
                    value: rotation.angle_degrees,
                },
            );
        }
        Some(id)
    }
}

fn identity_parameter(expression: &FunctionExpr) -> Option<(&str, f64)> {
    match expression {
        FunctionExpr::Parsed(FunctionAst::Parameter(name, default)) => Some((name, *default)),
        _ => None,
    }
}

fn point_id(index: usize) -> String {
    format!("point:{index}")
}

fn core_line_kind(kind: LineLikeKind) -> LineKind {
    match kind {
        LineLikeKind::Segment => LineKind::Segment,
        LineLikeKind::Line => LineKind::Line,
        LineLikeKind::Ray => LineKind::Ray,
    }
}

fn payload_line_kind(kind: GroupKind) -> Option<LineKind> {
    match kind {
        GroupKind::Segment | GroupKind::MeasurementLine | GroupKind::GraphMeasurementSegment => {
            Some(LineKind::Segment)
        }
        GroupKind::Line => Some(LineKind::Line),
        GroupKind::Ray => Some(LineKind::Ray),
        _ => None,
    }
}

fn fuse_generated_trace_points(nodes: &mut [ObjectNode<ObjectOp>]) {
    let traces = nodes
        .iter()
        .filter_map(|node| match &node.definition {
            ObjectDefinition::Derived {
                op:
                    ObjectOp::PointTrace {
                        program,
                        driver,
                        value_min,
                        value_max,
                        ..
                    },
                parents,
            } => Some((
                node.id.clone(),
                (
                    program.clone(),
                    driver.clone(),
                    *value_min,
                    *value_max,
                    parents.clone(),
                ),
            )),
            _ => None,
        })
        .collect::<BTreeMap<_, _>>();

    for node in nodes {
        let ObjectDefinition::Derived { op, parents } = &mut node.definition else {
            continue;
        };
        if !matches!(op, ObjectOp::PointOnPolyline) || parents.len() != 2 {
            continue;
        }
        let Some((program, driver, value_min, value_max, trace_parents)) =
            traces.get(&parents[0]).cloned()
        else {
            continue;
        };
        let parameter_parent = parents[1].clone();
        *op = ObjectOp::PointOnGeneratedTrace {
            program,
            driver,
            value_min,
            value_max,
        };
        *parents = trace_parents
            .into_iter()
            .chain(std::iter::once(parameter_parent))
            .collect();
    }
}

fn retain_program_dependencies(
    nodes: &mut Vec<ObjectNode<ObjectOp>>,
    target_id: &str,
) -> Option<()> {
    let nodes_by_id = nodes
        .iter()
        .map(|node| (node.id.as_str(), node))
        .collect::<BTreeMap<_, _>>();
    let mut required = std::collections::BTreeSet::new();
    let mut stack = vec![target_id.to_string()];
    while let Some(id) = stack.pop() {
        if !required.insert(id.clone()) {
            continue;
        }
        stack.extend(nodes_by_id.get(id.as_str())?.parents().iter().cloned());
    }
    nodes.retain(|node| required.contains(&node.id));
    Some(())
}

fn is_trace_line(line: &crate::runtime::scene::LineShape) -> bool {
    matches!(
        line.binding,
        Some(
            LineBinding::CoordinateTrace { .. }
                | LineBinding::CustomTransformTrace { .. }
                | LineBinding::PointTrace { .. }
                | LineBinding::SegmentTrace { .. }
        )
    )
}

fn is_segment_trace_line(line: &crate::runtime::scene::LineShape) -> bool {
    matches!(line.binding, Some(LineBinding::SegmentTrace { .. }))
}

fn is_custom_transform_trace_line(line: &crate::runtime::scene::LineShape) -> bool {
    matches!(line.binding, Some(LineBinding::CustomTransformTrace { .. }))
}

fn trace_driver_source_ids(driver: &TraceDriver) -> Vec<String> {
    match driver {
        TraceDriver::Scalar { source_id, .. } => vec![source_id.clone()],
        TraceDriver::Circle {
            unit_x_source_id,
            unit_y_source_id,
        } => vec![unit_x_source_id.clone(), unit_y_source_id.clone()],
    }
}

fn line_id(index: usize) -> String {
    format!("line:{index}")
}

fn circle_id(index: usize) -> String {
    format!("circle:{index}")
}

fn circle_fill_color_id(index: usize) -> String {
    format!("circle-fill-color:{index}")
}

fn polygon_id(index: usize) -> String {
    format!("polygon:{index}")
}

fn polygon_color_id(index: usize) -> String {
    format!("polygon-color:{index}")
}

fn arc_id(index: usize) -> String {
    format!("arc:{index}")
}

fn label_scalar_id(index: usize) -> String {
    format!("scalar:label:{index}")
}

fn group_scalar_id(group_ordinal: usize) -> String {
    format!("scalar:group:{group_ordinal}")
}

fn core_point(point: &PointRecord) -> Point {
    Point {
        x: point.x,
        y: point.y,
    }
}
