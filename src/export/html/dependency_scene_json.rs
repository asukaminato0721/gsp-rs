use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;
use ts_rs::TS;

use crate::runtime::{
    functions::{FunctionExpr, function_expr_label},
    scene::{
        ArcConstraint, AxisBinding, CircleIterationFamily, CircularConstraint, ColorBinding,
        IterationPointHandle, IterationTable, LabelIterationFamily, LineBinding, LineConstraint,
        LineIterationFamily, LineTransformBinding, PointIterationFamily, PolygonIterationFamily,
        RichTextExpressionRef, RichTextExpressionValue, Scene, ScenePointBinding,
        ScenePointConstraint, ShapeBinding, ShapeTransformBinding, TextLabelBinding,
    },
};

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub(super) struct DependencyGraphJson {
    nodes: Vec<DependencyNodeJson>,
    derived_label_order: Vec<usize>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub(super) struct DependencyNodeJson {
    id: String,
    kind: String,
    depends_on: Vec<String>,
    recipe: Option<DependencyRecipeJson>,
}

#[derive(Debug, Clone, Copy, Serialize, TS)]
#[serde(rename_all = "kebab-case")]
enum DependencyRecipeJson {
    SyncBaseDynamics,
    RefreshDerivedPoints,
    RebuildIterationGeometry,
    RefreshDynamicLabels,
}

type Dependencies = BTreeSet<String>;

struct Collector<'a> {
    scene: &'a Scene,
    known_parameters: &'a BTreeSet<String>,
    derived_parameter_deps: &'a BTreeMap<String, Dependencies>,
}

impl DependencyGraphJson {
    pub(super) fn from_scene(scene: &Scene) -> Self {
        let known_parameters = scene
            .parameters
            .iter()
            .map(|parameter| parameter.name.clone())
            .collect::<BTreeSet<_>>();
        let derived_parameter_deps = collect_derived_parameter_deps(scene, &known_parameters);
        let collect = Collector {
            scene,
            known_parameters: &known_parameters,
            derived_parameter_deps: &derived_parameter_deps,
        };
        let mut nodes = Vec::new();

        for parameter in &scene.parameters {
            nodes.push(node(
                parameter_root_id(&parameter.name),
                "parameter-root",
                [],
                None,
            ));
            nodes.push(node(
                format!("parameter-sync:{}", parameter.name),
                "parameter-sync",
                [parameter_root_id(&parameter.name)],
                Some(DependencyRecipeJson::SyncBaseDynamics),
            ));
        }
        for index in 0..scene.points.len() {
            nodes.push(node(source_point_root_id(index), "source-point", [], None));
        }
        for index in 0..scene.lines.len() {
            nodes.push(node(source_line_root_id(index), "source-line", [], None));
        }
        for index in 0..scene.circles.len() {
            nodes.push(node(
                source_circle_root_id(index),
                "source-circle",
                [],
                None,
            ));
        }
        for index in 0..scene.polygons.len() {
            nodes.push(node(
                source_polygon_root_id(index),
                "source-polygon",
                [],
                None,
            ));
        }

        for (index, point) in scene.points.iter().enumerate() {
            if point.binding.is_none() && matches!(point.constraint, ScenePointConstraint::Free) {
                continue;
            }
            let mut deps = Dependencies::new();
            if let Some(binding) = &point.binding {
                collect.point_binding(&mut deps, binding);
            }
            collect.point_constraint(&mut deps, &point.constraint);
            nodes.push(node(
                format!("point:{index}"),
                "point",
                deps,
                Some(DependencyRecipeJson::RefreshDerivedPoints),
            ));
        }
        for (index, line) in scene.lines.iter().enumerate() {
            let Some(binding) = &line.binding else {
                continue;
            };
            let mut deps = Dependencies::new();
            collect.line_binding(&mut deps, binding);
            if let LineBinding::PointTrace {
                point_index,
                driver_index,
                ..
            } = binding
            {
                for point_index in [*point_index, *driver_index] {
                    if let Some(point) = scene.points.get(point_index) {
                        if let Some(binding) = &point.binding {
                            collect.point_binding(&mut deps, binding);
                        }
                        collect.point_constraint(&mut deps, &point.constraint);
                    }
                }
            }
            nodes.push(node(
                format!("line:{index}"),
                "line",
                deps,
                Some(DependencyRecipeJson::RefreshDerivedPoints),
            ));
        }
        for (index, circle) in scene.circles.iter().enumerate() {
            if circle.binding.is_none() && circle.fill_color_binding.is_none() {
                continue;
            }
            let mut deps = Dependencies::new();
            if let Some(binding) = &circle.binding {
                collect.shape_binding(&mut deps, binding, ShapeSource::Circle);
            }
            if let Some(binding) = &circle.fill_color_binding {
                collect.color_binding(&mut deps, binding);
            }
            nodes.push(node(
                format!("circle:{index}"),
                "circle",
                deps,
                Some(DependencyRecipeJson::RefreshDerivedPoints),
            ));
        }
        for (index, polygon) in scene.polygons.iter().enumerate() {
            if polygon.binding.is_none() && polygon.color_binding.is_none() {
                continue;
            }
            let mut deps = Dependencies::new();
            if let Some(binding) = &polygon.binding {
                collect.shape_binding(&mut deps, binding, ShapeSource::Polygon);
            }
            if let Some(binding) = &polygon.color_binding {
                collect.color_binding(&mut deps, binding);
            }
            nodes.push(node(
                format!("polygon:{index}"),
                "polygon",
                deps,
                Some(DependencyRecipeJson::RefreshDerivedPoints),
            ));
        }
        for (index, function) in scene.functions.iter().enumerate() {
            let mut deps = Dependencies::new();
            collect.expr(&mut deps, &function.expr);
            collect.points(
                &mut deps,
                function.constrained_point_indices.iter().copied(),
            );
            nodes.push(node(
                format!("function:{index}"),
                "function",
                deps,
                Some(DependencyRecipeJson::SyncBaseDynamics),
            ));
        }
        for (index, label) in scene.labels.iter().enumerate() {
            let Some(binding) = &label.binding else {
                continue;
            };
            let mut deps = Dependencies::new();
            collect.label_binding(&mut deps, binding);
            nodes.push(node(
                format!("label:{index}"),
                "label",
                deps,
                Some(DependencyRecipeJson::RefreshDynamicLabels),
            ));
        }
        append_iteration_nodes(&mut nodes, &collect, scene);
        Self {
            nodes,
            derived_label_order: derived_label_order(scene),
        }
    }
}

fn node(
    id: String,
    kind: &str,
    depends_on: impl IntoIterator<Item = String>,
    recipe: Option<DependencyRecipeJson>,
) -> DependencyNodeJson {
    let mut depends_on = depends_on
        .into_iter()
        .filter(|dep| dep != &id)
        .collect::<Vec<_>>();
    depends_on.sort();
    depends_on.dedup();
    DependencyNodeJson {
        id,
        kind: kind.into(),
        depends_on,
        recipe,
    }
}

#[derive(Clone, Copy)]
enum ShapeSource {
    Circle,
    Polygon,
}

fn parameter_root_id(name: &str) -> String {
    format!("param:{name}")
}
fn source_point_root_id(index: usize) -> String {
    format!("source-point:{index}")
}
fn source_line_root_id(index: usize) -> String {
    format!("source-line:{index}")
}
fn source_circle_root_id(index: usize) -> String {
    format!("source-circle:{index}")
}
fn source_polygon_root_id(index: usize) -> String {
    format!("source-polygon:{index}")
}

impl Collector<'_> {
    fn parameter(&self, deps: &mut Dependencies, name: Option<&str>) {
        let Some(name) = name else { return };
        if self.known_parameters.contains(name) {
            deps.insert(parameter_root_id(name));
        }
        if let Some(derived) = self.derived_parameter_deps.get(name) {
            deps.extend(derived.iter().cloned());
        }
    }

    fn expr(&self, deps: &mut Dependencies, expr: &FunctionExpr) {
        for name in gsp_runtime_core::expression_parameter_names(expr) {
            self.parameter(deps, Some(&name));
        }
    }

    fn point(&self, deps: &mut Dependencies, index: Option<usize>) {
        let Some(index) = index else { return };
        deps.insert(source_point_root_id(index));
        if let Some(point) = self.scene.points.get(index)
            && (point.binding.is_some() || !matches!(point.constraint, ScenePointConstraint::Free))
        {
            deps.insert(format!("point:{index}"));
        }
    }

    fn points(&self, deps: &mut Dependencies, indices: impl IntoIterator<Item = usize>) {
        for index in indices {
            self.point(deps, Some(index));
        }
    }

    fn optional_points(
        &self,
        deps: &mut Dependencies,
        indices: impl IntoIterator<Item = Option<usize>>,
    ) {
        for index in indices {
            self.point(deps, index);
        }
    }

    fn line(&self, deps: &mut Dependencies, index: Option<usize>) {
        let Some(index) = index else { return };
        deps.insert(source_line_root_id(index));
        if self
            .scene
            .lines
            .get(index)
            .is_some_and(|line| line.binding.is_some())
        {
            deps.insert(format!("line:{index}"));
        }
    }

    fn circle(&self, deps: &mut Dependencies, index: usize) {
        deps.insert(source_circle_root_id(index));
        if self
            .scene
            .circles
            .get(index)
            .is_some_and(|circle| circle.binding.is_some() || circle.fill_color_binding.is_some())
        {
            deps.insert(format!("circle:{index}"));
        }
    }

    fn polygon(&self, deps: &mut Dependencies, index: usize) {
        deps.insert(source_polygon_root_id(index));
        if self
            .scene
            .polygons
            .get(index)
            .is_some_and(|polygon| polygon.binding.is_some() || polygon.color_binding.is_some())
        {
            deps.insert(format!("polygon:{index}"));
        }
    }

    fn line_constraint(&self, deps: &mut Dependencies, line: &LineConstraint) {
        match line {
            LineConstraint::Segment {
                start_index,
                end_index,
            }
            | LineConstraint::Line {
                start_index,
                end_index,
            }
            | LineConstraint::Ray {
                start_index,
                end_index,
            } => self.points(deps, [*start_index, *end_index]),
            LineConstraint::PerpendicularLine {
                through_index,
                line_start_index,
                line_end_index,
            }
            | LineConstraint::ParallelLine {
                through_index,
                line_start_index,
                line_end_index,
            } => {
                self.points(deps, [*through_index, *line_start_index, *line_end_index]);
            }
            LineConstraint::AngleBisectorRay {
                start_index,
                vertex_index,
                end_index,
            } => {
                self.points(deps, [*start_index, *vertex_index, *end_index]);
            }
            LineConstraint::PerpendicularTo {
                through_index,
                line,
            }
            | LineConstraint::ParallelTo {
                through_index,
                line,
            } => {
                self.point(deps, Some(*through_index));
                self.line_constraint(deps, line);
            }
            LineConstraint::Translated {
                line,
                vector_start_index,
                vector_end_index,
            } => {
                self.line_constraint(deps, line);
                self.points(deps, [*vector_start_index, *vector_end_index]);
            }
            LineConstraint::TranslatedDelta { line, .. } => self.line_constraint(deps, line),
            LineConstraint::Reflected { line, axis } => {
                self.line_constraint(deps, line);
                self.line_constraint(deps, axis);
            }
            LineConstraint::Rotated { line, rotation } => {
                self.line_constraint(deps, line);
                self.point(deps, Some(rotation.center_index));
                self.points(
                    deps,
                    [
                        rotation.angle_start_index,
                        rotation.angle_vertex_index,
                        rotation.angle_end_index,
                    ]
                    .into_iter()
                    .flatten(),
                );
                if let Some(expr) = &rotation.angle_expr {
                    self.expr(deps, expr);
                }
            }
        }
    }

    fn circular_constraint(&self, deps: &mut Dependencies, circle: &CircularConstraint) {
        match circle {
            CircularConstraint::Circle {
                center_index,
                radius_index,
            } => self.points(deps, [*center_index, *radius_index]),
            CircularConstraint::SegmentRadiusCircle {
                center_index,
                line_start_index,
                line_end_index,
            } => {
                self.points(deps, [*center_index, *line_start_index, *line_end_index]);
            }
            CircularConstraint::ParameterRadiusCircle {
                center_index,
                parameter_name,
                ..
            } => {
                self.point(deps, Some(*center_index));
                self.parameter(deps, Some(parameter_name));
            }
            CircularConstraint::ExpressionRadiusCircle {
                center_index, expr, ..
            } => {
                self.point(deps, Some(*center_index));
                self.expr(deps, expr);
            }
            CircularConstraint::TranslateCircle { source, .. } => {
                self.circular_constraint(deps, source)
            }
            CircularConstraint::VectorTranslateCircle {
                source,
                vector_start_index,
                vector_end_index,
            } => {
                self.circular_constraint(deps, source);
                self.points(deps, [*vector_start_index, *vector_end_index]);
            }
            CircularConstraint::ReflectCircle {
                source,
                line_start_index,
                line_end_index,
                line_index,
            } => {
                self.circular_constraint(deps, source);
                self.optional_points(deps, [*line_start_index, *line_end_index]);
                self.line(deps, *line_index);
            }
            CircularConstraint::ScaleCircle {
                source,
                center_index,
                ..
            } => {
                self.circular_constraint(deps, source);
                self.point(deps, Some(*center_index));
            }
            CircularConstraint::RotateCircle {
                source,
                center_index,
                ..
            } => {
                self.circular_constraint(deps, source);
                self.point(deps, Some(*center_index));
            }
            CircularConstraint::CircleArc {
                center_index,
                start_index,
                end_index,
            } => {
                self.points(deps, [*center_index, *start_index, *end_index]);
            }
            CircularConstraint::ThreePointArc {
                start_index,
                mid_index,
                end_index,
            } => {
                self.points(deps, [*start_index, *mid_index, *end_index]);
            }
        }
    }

    fn arc_constraint(&self, deps: &mut Dependencies, arc: &ArcConstraint) {
        match arc {
            ArcConstraint::CenterArc {
                center_index,
                start_index,
                end_index,
            } => self.points(deps, [*center_index, *start_index, *end_index]),
            ArcConstraint::CircleArc {
                circle,
                start_index,
                end_index,
            } => {
                self.circular_constraint(deps, circle);
                self.points(deps, [*start_index, *end_index]);
            }
            ArcConstraint::ThreePointArc {
                start_index,
                mid_index,
                end_index,
            } => self.points(deps, [*start_index, *mid_index, *end_index]),
            ArcConstraint::Reflected { arc, axis } => {
                self.arc_constraint(deps, arc);
                self.line_constraint(deps, axis);
            }
        }
    }

    fn axis(&self, deps: &mut Dependencies, axis: &AxisBinding) {
        self.optional_points(deps, [axis.line_start_index, axis.line_end_index]);
        self.line(deps, axis.line_index);
    }

    fn line_transform(&self, deps: &mut Dependencies, transform: &LineTransformBinding) {
        match transform {
            LineTransformBinding::Translate {
                vector_start_index,
                vector_end_index,
            } => self.points(deps, [*vector_start_index, *vector_end_index]),
            LineTransformBinding::Rotate(rotation) => {
                self.point(deps, Some(rotation.center_index));
                self.optional_points(
                    deps,
                    [
                        rotation.angle_start_index,
                        rotation.angle_vertex_index,
                        rotation.angle_end_index,
                    ],
                );
                self.parameter(deps, rotation.parameter_name.as_deref());
            }
            LineTransformBinding::Scale(scale) => self.point(deps, Some(scale.center_index)),
            LineTransformBinding::Reflect(axis) => self.axis(deps, axis),
        }
    }

    fn shape_transform(&self, deps: &mut Dependencies, transform: &ShapeTransformBinding) {
        match transform {
            ShapeTransformBinding::TranslateDelta { .. } => {}
            ShapeTransformBinding::TranslateVector {
                vector_start_index,
                vector_end_index,
            } => self.points(deps, [*vector_start_index, *vector_end_index]),
            ShapeTransformBinding::Rotate(rotation) => {
                self.point(deps, Some(rotation.center_index));
                self.optional_points(
                    deps,
                    [
                        rotation.angle_start_index,
                        rotation.angle_vertex_index,
                        rotation.angle_end_index,
                    ],
                );
                self.parameter(deps, rotation.parameter_name.as_deref());
            }
            ShapeTransformBinding::Scale(scale) => self.point(deps, Some(scale.center_index)),
            ShapeTransformBinding::Reflect(axis) => self.axis(deps, axis),
        }
    }

    fn point_binding(&self, deps: &mut Dependencies, binding: &ScenePointBinding) {
        match binding {
            ScenePointBinding::GraphCalibration => {}
            ScenePointBinding::ProjectedCoordinate { source_index, .. } => {
                self.point(deps, Some(*source_index))
            }
            ScenePointBinding::Parameter { name } => self.parameter(deps, Some(name)),
            ScenePointBinding::DerivedParameter {
                source_index,
                parameter_start_index,
                parameter_end_index,
            } => {
                self.point(deps, Some(*source_index));
                self.optional_points(deps, [*parameter_start_index, *parameter_end_index]);
            }
            ScenePointBinding::ConstraintParameterExpr { expr } => self.expr(deps, expr),
            ScenePointBinding::ConstraintParameterPointDistanceRatio {
                origin_index,
                denominator_index,
                numerator_index,
                ..
            } => self.points(deps, [*origin_index, *denominator_index, *numerator_index]),
            ScenePointBinding::ConstraintParameterFromPointExpr {
                source_index,
                parameter_name,
                expr,
                expression_sources,
                ..
            } => {
                self.point(deps, Some(*source_index));
                for source in expression_sources {
                    self.point(deps, Some(source.point_index));
                    match &source.domain {
                        Some(crate::runtime::scene::ScenePointParameterDomain::Circular(
                            circle,
                        )) => self.circular_constraint(deps, circle),
                        Some(
                            crate::runtime::scene::ScenePointParameterDomain::PolygonBoundary {
                                vertex_indices,
                            },
                        ) => self.points(deps, vertex_indices.iter().copied()),
                        None => {}
                    }
                }
                self.parameter(deps, Some(parameter_name));
                self.expr(deps, expr);
            }
            ScenePointBinding::Translate {
                source_index,
                vector_start_index,
                vector_end_index,
            } => self.points(
                deps,
                [*source_index, *vector_start_index, *vector_end_index],
            ),
            ScenePointBinding::DirectedAngleAnchor {
                first_start_index,
                first_end_index,
                second_start_index,
                second_end_index,
                ..
            } => self.points(
                deps,
                [
                    *first_start_index,
                    *first_end_index,
                    *second_start_index,
                    *second_end_index,
                ],
            ),
            ScenePointBinding::Reflect {
                source_index,
                line_start_index,
                line_end_index,
            } => self.points(deps, [*source_index, *line_start_index, *line_end_index]),
            ScenePointBinding::ReflectLineConstraint { source_index, line } => {
                self.point(deps, Some(*source_index));
                self.line_constraint(deps, line);
            }
            ScenePointBinding::Rotate {
                source_index,
                center_index,
                parameter_name,
                angle_expr,
                angle_start_index,
                angle_vertex_index,
                angle_end_index,
                angle_parameter_point_index,
                angle_parameter_start_index,
                angle_parameter_end_index,
                ..
            } => {
                self.points(deps, [*source_index, *center_index]);
                self.optional_points(
                    deps,
                    [
                        *angle_start_index,
                        *angle_vertex_index,
                        *angle_end_index,
                        *angle_parameter_point_index,
                        *angle_parameter_start_index,
                        *angle_parameter_end_index,
                    ],
                );
                self.parameter(deps, parameter_name.as_deref());
                if let Some(expr) = angle_expr {
                    self.expr(deps, expr);
                }
            }
            ScenePointBinding::ScaleByRatio {
                source_index,
                center_index,
                ratio_origin_index,
                ratio_denominator_index,
                ratio_numerator_index,
                ..
            } => {
                self.points(
                    deps,
                    [
                        *source_index,
                        *center_index,
                        *ratio_origin_index,
                        *ratio_denominator_index,
                        *ratio_numerator_index,
                    ],
                );
            }
            ScenePointBinding::Scale {
                source_index,
                center_index,
                parameter_name,
                factor_expr,
                factor_parameter_point_index,
                factor_parameter_start_index,
                factor_parameter_end_index,
                ..
            } => {
                self.points(deps, [*source_index, *center_index]);
                self.optional_points(
                    deps,
                    [
                        *factor_parameter_point_index,
                        *factor_parameter_start_index,
                        *factor_parameter_end_index,
                    ],
                );
                self.parameter(deps, parameter_name.as_deref());
                if let Some(expr) = factor_expr {
                    self.expr(deps, expr);
                }
            }
            ScenePointBinding::MarkedAngleTranslation {
                target_index,
                angle_start_index,
                angle_vertex_index,
                angle_end_index,
                distance_expr,
                ..
            } => {
                self.points(
                    deps,
                    [
                        *target_index,
                        *angle_start_index,
                        *angle_vertex_index,
                        *angle_end_index,
                    ],
                );
                self.expr(deps, distance_expr);
            }
            ScenePointBinding::Midpoint {
                start_index,
                end_index,
            } => self.points(deps, [*start_index, *end_index]),
            ScenePointBinding::Circumcenter {
                start_index,
                mid_index,
                end_index,
            } => self.points(deps, [*start_index, *mid_index, *end_index]),
            ScenePointBinding::Coordinate { expr, .. } => self.expr(deps, expr),
            ScenePointBinding::CoordinateSource {
                source_index, expr, ..
            } => {
                self.point(deps, Some(*source_index));
                self.expr(deps, expr);
            }
            ScenePointBinding::CoordinateSource2d {
                source_index,
                x_expr,
                y_expr,
                ..
            } => {
                self.point(deps, Some(*source_index));
                self.expr(deps, x_expr);
                self.expr(deps, y_expr);
            }
            ScenePointBinding::PolarOffset {
                source_index,
                distance_expr,
                ..
            } => {
                self.point(deps, Some(*source_index));
                self.expr(deps, distance_expr);
            }
            ScenePointBinding::PolarTransform {
                source_index,
                distance_expr,
                angle_expr,
                ..
            } => {
                self.point(deps, Some(*source_index));
                self.expr(deps, distance_expr);
                self.expr(deps, angle_expr);
            }
            ScenePointBinding::RadiusOffset {
                source_index,
                circle,
                ..
            } => {
                self.point(deps, Some(*source_index));
                self.circular_constraint(deps, circle);
            }
            ScenePointBinding::BoundaryLengthOffset {
                source_index,
                boundary,
                ..
            } => {
                self.point(deps, Some(*source_index));
                self.circular_constraint(deps, boundary);
            }
            ScenePointBinding::CustomTransform {
                source_index,
                origin_index,
                axis_end_index,
                distance_expr,
                angle_expr,
                ..
            } => {
                self.points(deps, [*source_index, *origin_index, *axis_end_index]);
                self.expr(deps, distance_expr);
                self.expr(deps, angle_expr);
            }
        }
    }

    fn point_constraint(&self, deps: &mut Dependencies, constraint: &ScenePointConstraint) {
        match constraint {
            ScenePointConstraint::Free | ScenePointConstraint::OnPolyline { .. } => {}
            ScenePointConstraint::Offset { origin_index, .. } => {
                self.point(deps, Some(*origin_index))
            }
            ScenePointConstraint::OnSegment {
                start_index,
                end_index,
                ..
            }
            | ScenePointConstraint::OnLine {
                start_index,
                end_index,
                ..
            }
            | ScenePointConstraint::OnRay {
                start_index,
                end_index,
                ..
            } => self.points(deps, [*start_index, *end_index]),
            ScenePointConstraint::OnLineConstraint { line, .. }
            | ScenePointConstraint::OnRayConstraint { line, .. } => {
                self.line_constraint(deps, line)
            }
            ScenePointConstraint::OnPolygonBoundary { vertex_indices, .. }
            | ScenePointConstraint::OnPolygonBoundaryParameter { vertex_indices, .. } => {
                self.points(deps, vertex_indices.iter().copied())
            }
            ScenePointConstraint::OnTranslatedPolygonBoundary {
                vertex_indices,
                vector_start_index,
                vector_end_index,
                ..
            } => {
                self.points(deps, vertex_indices.iter().copied());
                self.points(deps, [*vector_start_index, *vector_end_index]);
            }
            ScenePointConstraint::OnCircle {
                center_index,
                radius_index,
                ..
            } => self.points(deps, [*center_index, *radius_index]),
            ScenePointConstraint::OnCircularConstraint { circle, .. } => {
                self.circular_constraint(deps, circle)
            }
            ScenePointConstraint::OnCircleArc {
                center_index,
                start_index,
                end_index,
                ..
            } => self.points(deps, [*center_index, *start_index, *end_index]),
            ScenePointConstraint::OnArc {
                start_index,
                mid_index,
                end_index,
                ..
            } => self.points(deps, [*start_index, *mid_index, *end_index]),
            ScenePointConstraint::OnArcConstraint { arc, .. } => self.arc_constraint(deps, arc),
            ScenePointConstraint::LineIntersection { left, right } => {
                self.line_constraint(deps, left);
                self.line_constraint(deps, right);
            }
            ScenePointConstraint::LinePolygonIntersection {
                line,
                vertex_indices,
                ..
            } => {
                self.line_constraint(deps, line);
                self.points(deps, vertex_indices.iter().copied());
            }
            ScenePointConstraint::LineTraceIntersection {
                line, point_index, ..
            } => {
                self.line_constraint(deps, line);
                self.point(deps, Some(*point_index));
            }
            ScenePointConstraint::CircularTraceIntersection {
                circle,
                point_index,
                ..
            } => {
                self.circular_constraint(deps, circle);
                self.point(deps, Some(*point_index));
            }
            ScenePointConstraint::LineFunctionIntersection { line, expr, .. } => {
                self.line_constraint(deps, line);
                self.expr(deps, expr);
            }
            ScenePointConstraint::PointCircularTangent {
                point_index,
                circle,
                ..
            } => {
                self.point(deps, Some(*point_index));
                self.circular_constraint(deps, circle);
            }
            ScenePointConstraint::LineCircularIntersection { line, circle, .. } => {
                self.line_constraint(deps, line);
                self.circular_constraint(deps, circle);
            }
            ScenePointConstraint::LineCircleIntersection {
                line,
                center_index,
                radius_index,
                ..
            } => {
                self.line_constraint(deps, line);
                self.points(deps, [*center_index, *radius_index]);
            }
            ScenePointConstraint::CircleCircleIntersection {
                left_center_index,
                left_radius_index,
                right_center_index,
                right_radius_index,
                ..
            } => self.points(
                deps,
                [
                    *left_center_index,
                    *left_radius_index,
                    *right_center_index,
                    *right_radius_index,
                ],
            ),
            ScenePointConstraint::CircularIntersection { left, right, .. } => {
                self.circular_constraint(deps, left);
                self.circular_constraint(deps, right);
            }
        }
    }

    fn line_binding(&self, deps: &mut Dependencies, binding: &LineBinding) {
        match binding {
            LineBinding::GraphHelperLine {
                start_index,
                end_index,
            }
            | LineBinding::Segment {
                start_index,
                end_index,
            }
            | LineBinding::SegmentMarker {
                start_index,
                end_index,
                ..
            }
            | LineBinding::Line {
                start_index,
                end_index,
            }
            | LineBinding::Ray {
                start_index,
                end_index,
            } => self.points(deps, [*start_index, *end_index]),
            LineBinding::AngleMarker {
                start_index,
                vertex_index,
                end_index,
                ..
            }
            | LineBinding::AngleBisectorRay {
                start_index,
                vertex_index,
                end_index,
            } => self.points(deps, [*start_index, *vertex_index, *end_index]),
            LineBinding::PerpendicularLine {
                through_index,
                line_start_index,
                line_end_index,
                line_index,
            }
            | LineBinding::ParallelLine {
                through_index,
                line_start_index,
                line_end_index,
                line_index,
            } => {
                self.point(deps, Some(*through_index));
                self.optional_points(deps, [*line_start_index, *line_end_index]);
                self.line(deps, *line_index);
            }
            LineBinding::DerivedTransform {
                source_index,
                transform,
            } => {
                self.line(deps, Some(*source_index));
                self.line_transform(deps, transform);
            }
            LineBinding::CustomTransformTrace {
                point_index,
                driver_index,
                ..
            }
            | LineBinding::PointTrace {
                point_index,
                driver_index,
                ..
            } => self.points(deps, [*point_index, *driver_index]),
            LineBinding::CoordinateTrace { point_index, .. } => {
                self.point(deps, Some(*point_index))
            }
            LineBinding::SegmentTrace {
                start_index,
                end_index,
                driver_index,
                ..
            } => self.points(deps, [*start_index, *end_index, *driver_index]),
            LineBinding::ColorizedSpectrum {
                line_index,
                trace_line_index,
                point_index,
                trace_endpoint_index: _,
                reflection_source_index,
                reflection_axis_line_index,
                reflection_focus_index,
                reflection_directrix_line_index,
                depth_parameter_name,
                ..
            } => {
                self.line(deps, Some(*line_index));
                self.line(deps, Some(*trace_line_index));
                self.line(deps, *reflection_axis_line_index);
                self.line(deps, *reflection_directrix_line_index);
                self.point(deps, Some(*point_index));
                self.point(deps, *reflection_source_index);
                self.point(deps, *reflection_focus_index);
                self.parameter(deps, depth_parameter_name.as_deref());
            }
            LineBinding::ParametricCurve { x_expr, y_expr, .. } => {
                self.expr(deps, x_expr);
                self.expr(deps, y_expr);
            }
            LineBinding::ArcBoundary {
                center_index,
                start_index,
                mid_index,
                end_index,
                ..
            } => {
                self.optional_points(
                    deps,
                    [
                        *center_index,
                        Some(*start_index),
                        *mid_index,
                        Some(*end_index),
                    ],
                );
            }
        }
    }

    fn shape_binding(&self, deps: &mut Dependencies, binding: &ShapeBinding, source: ShapeSource) {
        match binding {
            ShapeBinding::PointRadiusCircle {
                center_index,
                radius_index,
            } => self.points(deps, [*center_index, *radius_index]),
            ShapeBinding::PointPolygon { vertex_indices } => {
                self.points(deps, vertex_indices.iter().copied())
            }
            ShapeBinding::ArcBoundaryPolygon {
                center_index,
                start_index,
                mid_index,
                end_index,
                ..
            } => self.optional_points(
                deps,
                [
                    *center_index,
                    Some(*start_index),
                    *mid_index,
                    Some(*end_index),
                ],
            ),
            ShapeBinding::SegmentRadiusCircle {
                center_index,
                line_start_index,
                line_end_index,
            } => self.points(deps, [*center_index, *line_start_index, *line_end_index]),
            ShapeBinding::ParameterRadiusCircle {
                center_index,
                parameter_name,
                ..
            } => {
                self.point(deps, Some(*center_index));
                self.parameter(deps, Some(parameter_name));
            }
            ShapeBinding::ExpressionRadiusCircle {
                center_index, expr, ..
            } => {
                self.point(deps, Some(*center_index));
                self.expr(deps, expr);
            }
            ShapeBinding::DerivedTransform {
                source_index,
                transform,
            } => {
                match source {
                    ShapeSource::Circle => self.circle(deps, *source_index),
                    ShapeSource::Polygon => self.polygon(deps, *source_index),
                }
                self.shape_transform(deps, transform);
            }
        }
    }

    fn color_binding(&self, deps: &mut Dependencies, binding: &ColorBinding) {
        match binding {
            ColorBinding::Spectrum { point_index, .. } => self.point(deps, Some(*point_index)),
            ColorBinding::Rgb {
                red_point_index,
                green_point_index,
                blue_point_index,
                ..
            } => self.points(
                deps,
                [*red_point_index, *green_point_index, *blue_point_index],
            ),
            ColorBinding::Hsb {
                hue_point_index,
                saturation_point_index,
                brightness_point_index,
                ..
            } => self.points(
                deps,
                [
                    *hue_point_index,
                    *saturation_point_index,
                    *brightness_point_index,
                ],
            ),
        }
    }

    fn rich_text_ref(&self, deps: &mut Dependencies, reference: &RichTextExpressionRef) {
        match &reference.value {
            RichTextExpressionValue::Expr { expr } => self.expr(deps, expr),
            RichTextExpressionValue::Parameter { name } => self.parameter(deps, Some(name)),
            RichTextExpressionValue::IterationState {
                state_parameter_names,
                state_exprs,
                depth_expr,
                ..
            } => {
                for name in state_parameter_names {
                    self.parameter(deps, Some(name));
                }
                for expr in state_exprs {
                    self.expr(deps, expr);
                }
                if let Some(expr) = depth_expr {
                    self.expr(deps, expr);
                }
            }
        }
    }

    fn label_binding(&self, deps: &mut Dependencies, binding: &TextLabelBinding) {
        match binding {
            TextLabelBinding::ParameterValue { name } => self.parameter(deps, Some(name)),
            TextLabelBinding::ScalarAlias { .. } => {}
            TextLabelBinding::ExpressionValue {
                parameter_name,
                expr,
                ..
            } => {
                self.parameter(deps, Some(parameter_name));
                self.expr(deps, expr);
            }
            TextLabelBinding::PointBoundExpressionValue {
                point_index,
                parameter_name,
                expr,
                ..
            } => {
                self.point(deps, Some(*point_index));
                self.parameter(deps, Some(parameter_name));
                self.expr(deps, expr);
            }
            TextLabelBinding::PointAnchor {
                point_index,
                anchor_y_point_index,
                ..
            } => self.optional_points(deps, [Some(*point_index), *anchor_y_point_index]),
            TextLabelBinding::PointExpressionValue {
                point_index,
                anchor_y_point_index,
                parameter_name,
                expr,
                ..
            } => {
                self.optional_points(deps, [Some(*point_index), *anchor_y_point_index]);
                self.parameter(deps, Some(parameter_name));
                self.expr(deps, expr);
            }
            TextLabelBinding::SequenceExpressionValue {
                parameter_name,
                depth_parameter_name,
                expr,
                ..
            } => {
                self.parameter(deps, Some(parameter_name));
                self.parameter(deps, depth_parameter_name.as_deref());
                self.expr(deps, expr);
            }
            TextLabelBinding::RichTextExpressionValues { refs, .. } => {
                for reference in refs {
                    self.rich_text_ref(deps, reference);
                }
            }
            TextLabelBinding::PointCoordinateValue {
                point_index,
                origin_index,
                x_unit_index,
                y_unit_index,
                ..
            }
            | TextLabelBinding::PointAxisValue {
                point_index,
                origin_index,
                x_unit_index,
                y_unit_index,
                ..
            } => self.optional_points(
                deps,
                [
                    Some(*point_index),
                    *origin_index,
                    *x_unit_index,
                    *y_unit_index,
                ],
            ),
            TextLabelBinding::PointDistanceValue {
                left_index,
                right_index,
                ..
            } => self.points(deps, [*left_index, *right_index]),
            TextLabelBinding::PointAngleValue {
                start_index,
                vertex_index,
                end_index,
                ..
            }
            | TextLabelBinding::AngleMarkerValue {
                start_index,
                vertex_index,
                end_index,
                ..
            } => self.points(deps, [*start_index, *vertex_index, *end_index]),
            TextLabelBinding::PolygonAreaValue { point_indices, .. } => {
                self.points(deps, point_indices.iter().copied())
            }
            TextLabelBinding::PointDistanceRatioValue {
                origin_index,
                denominator_index,
                numerator_index,
                ..
            } => self.points(deps, [*origin_index, *denominator_index, *numerator_index]),
            TextLabelBinding::PolygonBoundaryParameter { point_index, .. }
            | TextLabelBinding::PolylineParameter { point_index, .. }
            | TextLabelBinding::CircleParameter { point_index, .. }
            | TextLabelBinding::CustomTransformValue { point_index, .. } => {
                self.point(deps, Some(*point_index));
                if let TextLabelBinding::CustomTransformValue { expr, .. } = binding {
                    self.expr(deps, expr);
                }
            }
            TextLabelBinding::LineProjectionParameter {
                point_index,
                start_index,
                end_index,
                ..
            } => self.points(deps, [*point_index, *start_index, *end_index]),
        }
    }
}

fn derived_label_order(scene: &Scene) -> Vec<usize> {
    let mut derived = scene
        .labels
        .iter()
        .enumerate()
        .filter_map(|(label_index, label)| {
            let binding = label.binding.as_ref()?;
            let output_names = derived_label_output_names(binding);
            (!output_names.is_empty()).then_some((label_index, binding, output_names))
        })
        .collect::<Vec<_>>();
    derived.sort_by_key(|(label_index, _, _)| {
        scene.labels[*label_index]
            .debug
            .as_ref()
            .map(|debug| debug.group_ordinal)
            .unwrap_or(usize::MAX)
    });
    let mut producers = BTreeMap::<String, String>::new();
    let mut nodes = Vec::with_capacity(derived.len());
    for (index, (_, binding, output_names)) in derived.iter().enumerate() {
        let id = format!("derived-label:{index}");
        let mut referenced = BTreeSet::new();
        if let TextLabelBinding::ExpressionValue { expr, .. }
        | TextLabelBinding::PointBoundExpressionValue { expr, .. } = binding
        {
            referenced.extend(gsp_runtime_core::expression_parameter_names(expr));
        }
        nodes.push(gsp_runtime_core::DependencyNodeInput {
            id: id.clone(),
            depends_on: referenced
                .into_iter()
                .filter_map(|name| producers.get(&name).cloned())
                .collect(),
        });
        for name in output_names {
            producers.insert(name.clone(), id.clone());
        }
    }
    let plan = gsp_runtime_core::DependencyPlan::build(&nodes)
        .expect("derived label dependencies must not contain a cycle");
    plan.topo_order()
        .iter()
        .map(|index| derived[*index].0)
        .collect()
}

fn derived_label_output_names(binding: &TextLabelBinding) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    match binding {
        TextLabelBinding::PointDistanceRatioValue { name, .. }
        | TextLabelBinding::PointDistanceValue { name, .. }
        | TextLabelBinding::PointAngleValue { name, .. }
        | TextLabelBinding::PolygonAreaValue { name, .. }
        | TextLabelBinding::PointAxisValue { name, .. } => {
            names.insert(name.clone());
        }
        TextLabelBinding::LineProjectionParameter { point_name, .. }
        | TextLabelBinding::PolylineParameter { point_name, .. }
        | TextLabelBinding::PolygonBoundaryParameter { point_name, .. }
        | TextLabelBinding::CircleParameter { point_name, .. } => {
            names.insert(point_name.clone());
        }
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
            names.extend(result_name.iter().cloned());
            names.insert(expr_label.clone());
            names.insert(function_expr_label(expr.clone()));
        }
        _ => {}
    }
    names.retain(|name| !name.is_empty());
    names
}

fn collect_derived_parameter_deps(
    scene: &Scene,
    known_parameters: &BTreeSet<String>,
) -> BTreeMap<String, Dependencies> {
    let empty = BTreeMap::new();
    let collector = Collector {
        scene,
        known_parameters,
        derived_parameter_deps: &empty,
    };
    let mut definitions = BTreeMap::<String, (Dependencies, BTreeSet<String>)>::new();
    for label in &scene.labels {
        let Some(binding) = &label.binding else {
            continue;
        };
        let Some(name) = derived_parameter_name(binding) else {
            continue;
        };
        let entry = definitions.entry(name).or_default();
        collector.label_binding(&mut entry.0, binding);
        label_referenced_parameter_names(binding, &mut entry.1);
    }

    resolve_name_indexed_dependencies(&definitions, known_parameters)
}

fn resolve_name_indexed_dependencies(
    definitions: &BTreeMap<String, (Dependencies, BTreeSet<String>)>,
    known_parameters: &BTreeSet<String>,
) -> BTreeMap<String, Dependencies> {
    let mut resolved = definitions
        .iter()
        .map(|(name, (direct, referenced))| {
            let mut deps = direct.clone();
            deps.extend(
                referenced
                    .iter()
                    .filter(|name| known_parameters.contains(*name))
                    .map(|name| parameter_root_id(name)),
            );
            (name.clone(), deps)
        })
        .collect::<BTreeMap<_, _>>();

    // This compatibility graph is indexed by displayed names. GSP permits a
    // function application to shadow its argument (q(A) -> A), so distinct
    // ordinal values can collapse into cycles here. Propagate the finite set
    // of root dependencies to a least fixed point; the ordinal ObjectGraph
    // remains responsible for the actual evaluation order.
    loop {
        let previous = resolved.clone();
        let mut changed = false;
        for (name, (_, referenced)) in definitions {
            let deps = resolved.entry(name.clone()).or_default();
            for referenced_name in referenced {
                if let Some(referenced_deps) = previous.get(referenced_name) {
                    let old_len = deps.len();
                    deps.extend(referenced_deps.iter().cloned());
                    changed |= deps.len() != old_len;
                }
            }
        }
        if !changed {
            return resolved;
        }
    }
}

fn derived_parameter_name(binding: &TextLabelBinding) -> Option<String> {
    match binding {
        TextLabelBinding::PointDistanceRatioValue { name, .. }
        | TextLabelBinding::PointDistanceValue { name, .. }
        | TextLabelBinding::PointAngleValue { name, .. }
        | TextLabelBinding::PolygonAreaValue { name, .. }
        | TextLabelBinding::PointAxisValue { name, .. } => Some(name.clone()),
        TextLabelBinding::LineProjectionParameter { point_name, .. }
        | TextLabelBinding::PolylineParameter { point_name, .. }
        | TextLabelBinding::PolygonBoundaryParameter { point_name, .. }
        | TextLabelBinding::CircleParameter { point_name, .. } => Some(point_name.clone()),
        TextLabelBinding::ExpressionValue { result_name, .. }
        | TextLabelBinding::PointBoundExpressionValue { result_name, .. } => result_name.clone(),
        _ => None,
    }
}

fn label_referenced_parameter_names(binding: &TextLabelBinding, names: &mut BTreeSet<String>) {
    fn add_expr(names: &mut BTreeSet<String>, expr: &FunctionExpr) {
        names.extend(gsp_runtime_core::expression_parameter_names(expr));
    }
    match binding {
        TextLabelBinding::ParameterValue { name } => {
            names.insert(name.clone());
        }
        TextLabelBinding::ExpressionValue {
            parameter_name,
            expr,
            ..
        }
        | TextLabelBinding::PointBoundExpressionValue {
            parameter_name,
            expr,
            ..
        }
        | TextLabelBinding::PointExpressionValue {
            parameter_name,
            expr,
            ..
        } => {
            names.insert(parameter_name.clone());
            add_expr(names, expr);
        }
        TextLabelBinding::SequenceExpressionValue {
            parameter_name,
            depth_parameter_name,
            expr,
            ..
        } => {
            names.insert(parameter_name.clone());
            if let Some(name) = depth_parameter_name {
                names.insert(name.clone());
            }
            add_expr(names, expr);
        }
        TextLabelBinding::RichTextExpressionValues { refs, .. } => {
            for reference in refs {
                match &reference.value {
                    RichTextExpressionValue::Expr { expr } => add_expr(names, expr),
                    RichTextExpressionValue::Parameter { name } => {
                        names.insert(name.clone());
                    }
                    RichTextExpressionValue::IterationState {
                        state_parameter_names,
                        state_exprs,
                        depth_expr,
                        ..
                    } => {
                        names.extend(state_parameter_names.iter().cloned());
                        for expr in state_exprs {
                            add_expr(names, expr);
                        }
                        if let Some(expr) = depth_expr {
                            add_expr(names, expr);
                        }
                    }
                }
            }
        }
        TextLabelBinding::CustomTransformValue { expr, .. } => add_expr(names, expr),
        _ => {}
    }
}

fn append_iteration_nodes(
    nodes: &mut Vec<DependencyNodeJson>,
    collect: &Collector<'_>,
    scene: &Scene,
) {
    for (index, family) in scene.point_iterations.iter().enumerate() {
        let mut deps = Dependencies::new();
        collect.point_iteration(&mut deps, family);
        nodes.push(node(
            format!("point-iteration:{index}"),
            "point-iteration",
            deps,
            Some(DependencyRecipeJson::RebuildIterationGeometry),
        ));
    }
    for (index, family) in scene.circle_iterations.iter().enumerate() {
        let mut deps = Dependencies::new();
        collect.circle_iteration(&mut deps, family);
        nodes.push(node(
            format!("circle-iteration:{index}"),
            "circle-iteration",
            deps,
            Some(DependencyRecipeJson::RebuildIterationGeometry),
        ));
    }
    for (index, family) in scene.line_iterations.iter().enumerate() {
        let mut deps = Dependencies::new();
        collect.line_iteration(&mut deps, family);
        nodes.push(node(
            format!("line-iteration:{index}"),
            "line-iteration",
            deps,
            Some(DependencyRecipeJson::RebuildIterationGeometry),
        ));
    }
    for (index, family) in scene.polygon_iterations.iter().enumerate() {
        let mut deps = Dependencies::new();
        collect.polygon_iteration(&mut deps, family);
        nodes.push(node(
            format!("polygon-iteration:{index}"),
            "polygon-iteration",
            deps,
            Some(DependencyRecipeJson::RebuildIterationGeometry),
        ));
    }
    for (index, family) in scene.label_iterations.iter().enumerate() {
        let mut deps = Dependencies::new();
        collect.label_iteration(&mut deps, family);
        nodes.push(node(
            format!("label-iteration:{index}"),
            "label-iteration",
            deps,
            Some(DependencyRecipeJson::RebuildIterationGeometry),
        ));
    }
    for (index, table) in scene.iteration_tables.iter().enumerate() {
        let mut deps = Dependencies::new();
        collect.iteration_table(&mut deps, table);
        nodes.push(node(
            format!("iteration-table:{index}"),
            "iteration-table",
            deps,
            Some(DependencyRecipeJson::RebuildIterationGeometry),
        ));
    }
}

impl Collector<'_> {
    fn point_handle(&self, deps: &mut Dependencies, handle: &IterationPointHandle) {
        match handle {
            IterationPointHandle::Point { point_index } => self.point(deps, Some(*point_index)),
            IterationPointHandle::LinePoint { line_index, .. } => {
                self.line(deps, Some(*line_index))
            }
            IterationPointHandle::Fixed(_) => {}
        }
    }
    fn point_iteration(&self, deps: &mut Dependencies, family: &PointIterationFamily) {
        match family {
            PointIterationFamily::Interpreted {
                point_index,
                depth_parameter_name,
                ..
            } => {
                self.point(deps, Some(*point_index));
                self.parameter(deps, depth_parameter_name.as_deref());
            }
        }
    }
    fn line_iteration(&self, deps: &mut Dependencies, family: &LineIterationFamily) {
        match family {
            LineIterationFamily::Rotate {
                source_index,
                center_index,
                angle_expr,
                parameter_name,
                depth_parameter_name,
                ..
            } => {
                self.line(deps, Some(*source_index));
                self.point(deps, Some(*center_index));
                self.parameter(deps, parameter_name.as_deref());
                self.parameter(deps, depth_parameter_name.as_deref());
                self.expr(deps, angle_expr);
            }
            LineIterationFamily::Translate {
                start_index,
                end_index,
                start_control_index,
                end_control_index,
                vector_start_index,
                vector_end_index,
                parameter_name,
                depth_expr,
                ..
            } => {
                self.points(deps, [*start_index, *end_index]);
                self.optional_points(
                    deps,
                    [
                        *start_control_index,
                        *end_control_index,
                        *vector_start_index,
                        *vector_end_index,
                    ],
                );
                self.parameter(deps, parameter_name.as_deref());
                if let Some(expr) = depth_expr {
                    self.expr(deps, expr);
                }
            }
            LineIterationFamily::Affine {
                start_index,
                end_index,
                source_triangle_indices,
                target_triangle,
                ..
            } => {
                self.points(deps, [*start_index, *end_index]);
                self.points(deps, source_triangle_indices.iter().copied());
                for handle in target_triangle {
                    self.point_handle(deps, handle);
                }
            }
            LineIterationFamily::Branching {
                start_index,
                end_index,
                target_segments,
                parameter_name,
                ..
            } => {
                self.points(deps, [*start_index, *end_index]);
                for segment in target_segments {
                    self.point_handle(deps, &segment[0]);
                    self.point_handle(deps, &segment[1]);
                }
                self.parameter(deps, parameter_name.as_deref());
            }
            LineIterationFamily::ParameterizedPointTrace {
                point_index,
                driver_index,
                depth_parameter_name,
                trace_parameter_name,
                step_expr,
                ..
            } => {
                self.points(deps, [*point_index, *driver_index]);
                self.parameter(deps, depth_parameter_name.as_deref());
                self.parameter(deps, Some(trace_parameter_name));
                self.expr(deps, step_expr);
            }
        }
    }
    fn circle_iteration(&self, deps: &mut Dependencies, family: &CircleIterationFamily) {
        self.circle(deps, family.source_circle_index);
        self.points(
            deps,
            [family.source_center_index, family.source_next_center_index],
        );
        self.points(deps, family.vertex_indices.iter().copied());
        self.parameter(deps, family.depth_parameter_name.as_deref());
    }
    fn polygon_iteration(&self, deps: &mut Dependencies, family: &PolygonIterationFamily) {
        match family {
            PolygonIterationFamily::Similarity {
                source_index,
                source_start_index,
                source_end_index,
                target_start_index,
                target_end_index,
                depth_expr,
                ..
            } => {
                self.polygon(deps, *source_index);
                self.points(
                    deps,
                    [
                        *source_start_index,
                        *source_end_index,
                        *target_start_index,
                        *target_end_index,
                    ],
                );
                if let Some(expr) = depth_expr {
                    self.expr(deps, expr);
                }
            }
            PolygonIterationFamily::Translate {
                vertex_indices,
                vector_start_index,
                vector_end_index,
                parameter_name,
                depth_expr,
                ..
            } => {
                self.points(deps, vertex_indices.iter().copied());
                self.optional_points(deps, [*vector_start_index, *vector_end_index]);
                self.parameter(deps, parameter_name.as_deref());
                if let Some(expr) = depth_expr {
                    self.expr(deps, expr);
                }
            }
            PolygonIterationFamily::CoordinateGrid {
                vertex_indices,
                parameter_name,
                step_expr,
                x_expr,
                y_expr,
                depth_expr,
                ..
            } => {
                self.points(deps, vertex_indices.iter().copied());
                self.parameter(deps, Some(parameter_name));
                self.expr(deps, step_expr);
                self.expr(deps, x_expr);
                self.expr(deps, y_expr);
                if let Some(expr) = depth_expr {
                    self.expr(deps, expr);
                }
            }
        }
    }
    fn label_iteration(&self, deps: &mut Dependencies, family: &LabelIterationFamily) {
        match family {
            LabelIterationFamily::PointExpression {
                seed_label_index,
                point_seed_index,
                parameter_name,
                expr,
                depth_parameter_name,
                ..
            } => {
                deps.insert(format!("label:{seed_label_index}"));
                self.point(deps, Some(*point_seed_index));
                self.parameter(deps, Some(parameter_name));
                self.parameter(deps, depth_parameter_name.as_deref());
                self.expr(deps, expr);
            }
            LabelIterationFamily::TranslateExpression {
                seed_label_index,
                vector_start_index,
                vector_end_index,
                parameter_name,
                expr,
                depth_expr,
                depth_parameter_name,
                ..
            } => {
                deps.insert(format!("label:{seed_label_index}"));
                self.points(deps, [*vector_start_index, *vector_end_index]);
                self.parameter(deps, Some(parameter_name));
                self.parameter(deps, depth_parameter_name.as_deref());
                self.expr(deps, expr);
                if let Some(expr) = depth_expr {
                    self.expr(deps, expr);
                }
            }
        }
    }
    fn iteration_table(&self, deps: &mut Dependencies, table: &IterationTable) {
        self.parameter(deps, Some(&table.parameter_name));
        self.parameter(deps, table.depth_parameter_name.as_deref());
        self.expr(deps, &table.expr);
        if let Some(expr) = &table.depth_expr {
            self.expr(deps, expr);
        }
        for column in &table.columns {
            self.parameter(deps, Some(&column.parameter_name));
            self.expr(deps, &column.expr);
            if let Some(crate::runtime::scene::IterationTableValueBinding::AngleMarker {
                start_index,
                vertex_index,
                end_index,
            }) = &column.value_binding
            {
                self.points(deps, [*start_index, *vertex_index, *end_index]);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recipe_names_are_stable() {
        assert_eq!(
            serde_json::to_string(&DependencyRecipeJson::RefreshDerivedPoints).unwrap(),
            "\"refresh-derived-points\""
        );
    }

    #[test]
    fn name_indexed_dependency_cycles_reach_a_root_fixed_point() {
        let definitions = BTreeMap::from([
            (
                "A".into(),
                (
                    BTreeSet::from(["point:7".into()]),
                    BTreeSet::from(["B".into()]),
                ),
            ),
            (
                "B".into(),
                (BTreeSet::new(), BTreeSet::from(["A".into(), "k".into()])),
            ),
        ]);
        let resolved =
            resolve_name_indexed_dependencies(&definitions, &BTreeSet::from(["k".into()]));
        let expected = BTreeSet::from(["point:7".into(), parameter_root_id("k")]);
        assert_eq!(resolved["A"], expected);
        assert_eq!(resolved["B"], expected);
    }
}
