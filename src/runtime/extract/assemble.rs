use crate::format::PointRecord;
use crate::runtime::functions::{BinaryOp, FunctionAst, FunctionExpr, function_expr_ast};
use crate::runtime::geometry::{include_line_bounds, rotate_around, to_world};
use crate::runtime::scene::{
    CircleIterationFamily, CircularConstraint, LabelIterationFamily, LineConstraint,
    LineIterationFamily, LineShape, PointIterationFamily, PolygonIterationFamily, PolygonShape,
    Scene, SceneArc, SceneCircle, SceneImage, ScenePoint, ScenePointBinding, ScenePointConstraint,
    SceneScalar, ShapeBinding, TextLabel, TextLabelBinding,
};

use super::analysis::{BoundsData, CollectedShapes, SceneAnalysis, WorldData};
use super::graph::{BoundsInputs, collect_bounds, expand_bounds};
use super::world::{world_line_iteration_family, world_line_shape, world_polygon_iteration_family};

pub(super) struct SceneAssemblyArtifacts {
    pub(super) payload_dependencies: std::collections::BTreeMap<usize, Vec<usize>>,
    pub(super) circle_iterations: Vec<CircleIterationFamily>,
    pub(super) line_iterations: Vec<LineIterationFamily>,
    pub(super) polygon_iterations: Vec<PolygonIterationFamily>,
    pub(super) label_iterations: Vec<LabelIterationFamily>,
    pub(super) iteration_tables: Vec<crate::runtime::scene::IterationTable>,
    pub(super) buttons: Vec<crate::runtime::scene::SceneButton>,
    pub(super) images: Vec<SceneImage>,
    pub(super) parameters: Vec<crate::runtime::scene::SceneParameter>,
    pub(super) scalars: Vec<SceneScalar>,
    pub(super) functions: Vec<crate::runtime::scene::SceneFunction>,
    pub(super) function_definitions: Vec<crate::runtime::scene::SceneFunctionDefinition>,
}

pub(super) fn build_world_data(
    analysis: &SceneAnalysis,
    visible_points: &[ScenePoint],
    standalone_parameter_points: &[ScenePoint],
    raw_point_iterations: Vec<super::points::RawPointIterationFamily>,
) -> WorldData {
    let mut world_points = visible_points
        .iter()
        .chain(standalone_parameter_points.iter())
        .map(|point| ScenePoint {
            position: to_world(&point.position, &analysis.graph_ref),
            color: point.color,
            visible: point.visible,
            draggable: point.draggable,
            constraint: match &point.constraint {
                ScenePointConstraint::Free => ScenePointConstraint::Free,
                ScenePointConstraint::Offset {
                    origin_index,
                    dx,
                    dy,
                } => {
                    let (dx, dy) = if let Some(transform) = &analysis.graph_ref {
                        (dx / transform.raw_per_unit, -dy / transform.raw_per_unit)
                    } else {
                        (*dx, *dy)
                    };
                    ScenePointConstraint::Offset {
                        origin_index: *origin_index,
                        dx,
                        dy,
                    }
                }
                ScenePointConstraint::OnSegment {
                    start_index,
                    end_index,
                    t,
                } => ScenePointConstraint::OnSegment {
                    start_index: *start_index,
                    end_index: *end_index,
                    t: *t,
                },
                ScenePointConstraint::OnLine {
                    start_index,
                    end_index,
                    t,
                } => ScenePointConstraint::OnLine {
                    start_index: *start_index,
                    end_index: *end_index,
                    t: *t,
                },
                ScenePointConstraint::OnLineConstraint { line, t } => {
                    ScenePointConstraint::OnLineConstraint {
                        line: clone_line_constraint(line),
                        t: *t,
                    }
                }
                ScenePointConstraint::OnRay {
                    start_index,
                    end_index,
                    t,
                } => ScenePointConstraint::OnRay {
                    start_index: *start_index,
                    end_index: *end_index,
                    t: *t,
                },
                ScenePointConstraint::OnRayConstraint { line, t } => {
                    ScenePointConstraint::OnRayConstraint {
                        line: clone_line_constraint(line),
                        t: *t,
                    }
                }
                ScenePointConstraint::OnPolyline {
                    function_key,
                    points,
                    segment_index,
                    t,
                    parameter,
                } => ScenePointConstraint::OnPolyline {
                    function_key: *function_key,
                    points: points
                        .iter()
                        .map(|point| to_world(point, &analysis.graph_ref))
                        .collect(),
                    segment_index: *segment_index,
                    t: *t,
                    parameter: *parameter,
                },
                ScenePointConstraint::OnPolygonBoundary {
                    vertex_indices,
                    edge_index,
                    t,
                } => ScenePointConstraint::OnPolygonBoundary {
                    vertex_indices: vertex_indices.clone(),
                    edge_index: *edge_index,
                    t: *t,
                },
                ScenePointConstraint::OnPolygonBoundaryParameter {
                    vertex_indices,
                    parameter,
                } => ScenePointConstraint::OnPolygonBoundaryParameter {
                    vertex_indices: vertex_indices.clone(),
                    parameter: *parameter,
                },
                ScenePointConstraint::OnPolygonShapeBoundary {
                    polygon_index,
                    edge_index,
                    t,
                } => ScenePointConstraint::OnPolygonShapeBoundary {
                    polygon_index: *polygon_index,
                    edge_index: *edge_index,
                    t: *t,
                },
                ScenePointConstraint::OnCircle {
                    center_index,
                    radius_index,
                    unit_x,
                    unit_y,
                } => ScenePointConstraint::OnCircle {
                    center_index: *center_index,
                    radius_index: *radius_index,
                    unit_x: *unit_x,
                    unit_y: if analysis.graph_ref.is_some() {
                        *unit_y
                    } else {
                        -*unit_y
                    },
                },
                ScenePointConstraint::OnCircularConstraint {
                    circle,
                    unit_x,
                    unit_y,
                } => ScenePointConstraint::OnCircularConstraint {
                    circle: clone_circular_constraint(circle),
                    unit_x: *unit_x,
                    unit_y: if analysis.graph_ref.is_some() {
                        *unit_y
                    } else {
                        -*unit_y
                    },
                },
                ScenePointConstraint::OnCircleArc {
                    center_index,
                    start_index,
                    end_index,
                    t,
                } => ScenePointConstraint::OnCircleArc {
                    center_index: *center_index,
                    start_index: *start_index,
                    end_index: *end_index,
                    t: *t,
                },
                ScenePointConstraint::OnArc {
                    start_index,
                    mid_index,
                    end_index,
                    t,
                } => ScenePointConstraint::OnArc {
                    start_index: *start_index,
                    mid_index: *mid_index,
                    end_index: *end_index,
                    t: *t,
                },
                ScenePointConstraint::OnArcConstraint { arc, t } => {
                    ScenePointConstraint::OnArcConstraint {
                        arc: clone_arc_constraint(arc),
                        t: *t,
                    }
                }
                ScenePointConstraint::LineIntersection { left, right } => {
                    ScenePointConstraint::LineIntersection {
                        left: clone_line_constraint(left),
                        right: clone_line_constraint(right),
                    }
                }
                ScenePointConstraint::LinePolygonIntersection {
                    line,
                    vertex_indices,
                    variant,
                } => ScenePointConstraint::LinePolygonIntersection {
                    line: clone_line_constraint(line),
                    vertex_indices: vertex_indices.clone(),
                    variant: *variant,
                },
                ScenePointConstraint::LineTraceIntersection {
                    line,
                    trace_key,
                    point_index,
                    x_min,
                    x_max,
                    sample_count,
                    variant,
                } => ScenePointConstraint::LineTraceIntersection {
                    line: clone_line_constraint(line),
                    trace_key: *trace_key,
                    point_index: *point_index,
                    x_min: *x_min,
                    x_max: *x_max,
                    sample_count: *sample_count,
                    variant: *variant,
                },
                ScenePointConstraint::CircularTraceIntersection {
                    circle,
                    trace_key,
                    point_index,
                    x_min,
                    x_max,
                    sample_count,
                    variant,
                    sample_hint,
                } => ScenePointConstraint::CircularTraceIntersection {
                    circle: clone_circular_constraint(circle),
                    trace_key: *trace_key,
                    point_index: *point_index,
                    x_min: *x_min,
                    x_max: *x_max,
                    sample_count: *sample_count,
                    variant: *variant,
                    sample_hint: *sample_hint,
                },
                ScenePointConstraint::LineFunctionIntersection {
                    line,
                    function_key,
                    expr,
                    x_min,
                    x_max,
                    sample_count,
                    polar,
                    sample_hint,
                } => ScenePointConstraint::LineFunctionIntersection {
                    line: clone_line_constraint(line),
                    function_key: *function_key,
                    expr: expr.clone(),
                    x_min: *x_min,
                    x_max: *x_max,
                    sample_count: *sample_count,
                    polar: *polar,
                    sample_hint: *sample_hint,
                },
                ScenePointConstraint::PointCircularTangent {
                    point_index,
                    circle,
                    variant,
                } => ScenePointConstraint::PointCircularTangent {
                    point_index: *point_index,
                    circle: clone_circular_constraint(circle),
                    variant: *variant,
                },
                ScenePointConstraint::LineCircularIntersection {
                    line,
                    circle,
                    variant,
                } => ScenePointConstraint::LineCircularIntersection {
                    line: clone_line_constraint(line),
                    circle: clone_circular_constraint(circle),
                    variant: *variant,
                },
                ScenePointConstraint::LineCircleIntersection {
                    line,
                    center_index,
                    radius_index,
                    variant,
                } => ScenePointConstraint::LineCircleIntersection {
                    line: clone_line_constraint(line),
                    center_index: *center_index,
                    radius_index: *radius_index,
                    variant: *variant,
                },
                ScenePointConstraint::CircleCircleIntersection {
                    left_center_index,
                    left_radius_index,
                    right_center_index,
                    right_radius_index,
                    variant,
                } => ScenePointConstraint::CircleCircleIntersection {
                    left_center_index: *left_center_index,
                    left_radius_index: *left_radius_index,
                    right_center_index: *right_center_index,
                    right_radius_index: *right_radius_index,
                    variant: *variant,
                },
                ScenePointConstraint::CircularIntersection {
                    left,
                    right,
                    variant,
                } => ScenePointConstraint::CircularIntersection {
                    left: clone_circular_constraint(left),
                    right: clone_circular_constraint(right),
                    variant: *variant,
                },
            },
            binding: match &point.binding {
                Some(ScenePointBinding::DirectedAngleAnchor {
                    first_start_index,
                    first_end_index,
                    second_start_index,
                    second_end_index,
                    distance,
                    parameter,
                }) => Some(ScenePointBinding::DirectedAngleAnchor {
                    first_start_index: *first_start_index,
                    first_end_index: *first_end_index,
                    second_start_index: *second_start_index,
                    second_end_index: *second_end_index,
                    distance: analysis
                        .graph_ref
                        .as_ref()
                        .map_or(*distance, |transform| distance / transform.raw_per_unit),
                    parameter: *parameter,
                }),
                Some(ScenePointBinding::PolarTransform {
                    source_index,
                    distance_expr,
                    distance_parameter_group_ordinals,
                    distance_scale,
                    angle_expr,
                    angle_parameter_group_ordinals,
                    angle_degrees_scale,
                }) => Some(ScenePointBinding::PolarTransform {
                    source_index: *source_index,
                    distance_expr: distance_expr.clone(),
                    distance_parameter_group_ordinals: distance_parameter_group_ordinals.clone(),
                    distance_scale: analysis
                        .graph_ref
                        .as_ref()
                        .map_or(*distance_scale, |transform| {
                            distance_scale / transform.raw_per_unit
                        }),
                    angle_expr: angle_expr.clone(),
                    angle_parameter_group_ordinals: angle_parameter_group_ordinals.clone(),
                    angle_degrees_scale: *angle_degrees_scale,
                }),
                binding => binding.clone(),
            },
            debug: point.debug.clone(),
        })
        .collect::<Vec<_>>();
    orient_rotation_bindings_to_world_positions(&mut world_points);

    let world_point_positions = visible_points
        .iter()
        .filter(|point| point.visible)
        .map(|point| point.position.clone())
        .collect::<Vec<_>>();

    let point_iterations = raw_point_iterations
        .into_iter()
        .map(|family| match family {
            super::points::RawPointIterationFamily::Interpreted {
                point_index,
                states,
                depth_parameter_name,
                depth,
            } => PointIterationFamily::Interpreted {
                point_index,
                states,
                depth_parameter_name,
                depth,
            },
        })
        .collect::<Vec<_>>();

    WorldData {
        world_points,
        world_point_positions,
        point_iterations,
    }
}

fn clone_arc_constraint(
    arc: &crate::runtime::scene::ArcConstraint,
) -> crate::runtime::scene::ArcConstraint {
    use crate::runtime::scene::ArcConstraint;
    match arc {
        ArcConstraint::CenterArc {
            center_index,
            start_index,
            end_index,
        } => ArcConstraint::CenterArc {
            center_index: *center_index,
            start_index: *start_index,
            end_index: *end_index,
        },
        ArcConstraint::CircleArc {
            circle,
            start_index,
            end_index,
        } => ArcConstraint::CircleArc {
            circle: clone_circular_constraint(circle),
            start_index: *start_index,
            end_index: *end_index,
        },
        ArcConstraint::ThreePointArc {
            start_index,
            mid_index,
            end_index,
        } => ArcConstraint::ThreePointArc {
            start_index: *start_index,
            mid_index: *mid_index,
            end_index: *end_index,
        },
        ArcConstraint::Reflected { arc, axis } => ArcConstraint::Reflected {
            arc: Box::new(clone_arc_constraint(arc)),
            axis: clone_line_constraint(axis),
        },
    }
}

fn scale_function_expr(expr: FunctionExpr, factor: f64) -> FunctionExpr {
    if (factor - 1.0).abs() <= 1e-9 {
        return expr;
    }
    match expr {
        FunctionExpr::Constant(value) => FunctionExpr::Constant(value * factor),
        other => FunctionExpr::Parsed(FunctionAst::Binary {
            lhs: Box::new(FunctionAst::Constant(factor)),
            op: BinaryOp::Mul,
            rhs: Box::new(function_expr_ast(other)),
        }),
    }
}

fn squared_distance(left: &PointRecord, right: &PointRecord) -> f64 {
    (left.x - right.x).powi(2) + (left.y - right.y).powi(2)
}

fn orient_rotation_bindings_to_world_positions(points: &mut [ScenePoint]) {
    for index in 0..points.len() {
        let Some((source_index, center_index, angle_degrees)) = points[index]
            .binding
            .as_ref()
            .and_then(|binding| match binding {
                ScenePointBinding::Rotate {
                    source_index,
                    center_index,
                    angle_degrees,
                    angle_expr: Some(_),
                    ..
                } => Some((*source_index, *center_index, *angle_degrees)),
                _ => None,
            })
        else {
            continue;
        };
        let Some(source) = points.get(source_index).map(|point| &point.position) else {
            continue;
        };
        let Some(center) = points.get(center_index).map(|point| &point.position) else {
            continue;
        };
        let forward = rotate_around(source, center, angle_degrees.to_radians());
        let reverse = rotate_around(source, center, (-angle_degrees).to_radians());
        if squared_distance(&reverse, &points[index].position) + 1e-6
            >= squared_distance(&forward, &points[index].position)
        {
            continue;
        }
        if let Some(ScenePointBinding::Rotate {
            angle_degrees,
            angle_expr: Some(angle_expr),
            ..
        }) = points[index].binding.as_mut()
        {
            *angle_degrees = -*angle_degrees;
            *angle_expr = scale_function_expr(angle_expr.clone(), -1.0);
        }
    }
}

fn clone_circular_constraint(constraint: &CircularConstraint) -> CircularConstraint {
    match constraint {
        CircularConstraint::Circle {
            center_index,
            radius_index,
        } => CircularConstraint::Circle {
            center_index: *center_index,
            radius_index: *radius_index,
        },
        CircularConstraint::SegmentRadiusCircle {
            center_index,
            line_start_index,
            line_end_index,
        } => CircularConstraint::SegmentRadiusCircle {
            center_index: *center_index,
            line_start_index: *line_start_index,
            line_end_index: *line_end_index,
        },
        CircularConstraint::ParameterRadiusCircle {
            center_index,
            parameter_name,
            parameter_value,
            raw_per_unit,
        } => CircularConstraint::ParameterRadiusCircle {
            center_index: *center_index,
            parameter_name: parameter_name.clone(),
            parameter_value: *parameter_value,
            raw_per_unit: *raw_per_unit,
        },
        CircularConstraint::ExpressionRadiusCircle {
            center_index,
            expr,
            initial_value,
            parameter_group_ordinals,
        } => CircularConstraint::ExpressionRadiusCircle {
            center_index: *center_index,
            expr: expr.clone(),
            initial_value: *initial_value,
            parameter_group_ordinals: parameter_group_ordinals.clone(),
        },
        CircularConstraint::TranslateCircle { source, dx, dy } => {
            CircularConstraint::TranslateCircle {
                source: Box::new(clone_circular_constraint(source)),
                dx: *dx,
                dy: *dy,
            }
        }
        CircularConstraint::VectorTranslateCircle {
            source,
            vector_start_index,
            vector_end_index,
        } => CircularConstraint::VectorTranslateCircle {
            source: Box::new(clone_circular_constraint(source)),
            vector_start_index: *vector_start_index,
            vector_end_index: *vector_end_index,
        },
        CircularConstraint::ReflectCircle {
            source,
            line_start_index,
            line_end_index,
            line_index,
        } => CircularConstraint::ReflectCircle {
            source: Box::new(clone_circular_constraint(source)),
            line_start_index: *line_start_index,
            line_end_index: *line_end_index,
            line_index: *line_index,
        },
        CircularConstraint::ScaleCircle {
            source,
            center_index,
            factor,
        } => CircularConstraint::ScaleCircle {
            source: Box::new(clone_circular_constraint(source)),
            center_index: *center_index,
            factor: *factor,
        },
        CircularConstraint::RotateCircle {
            source,
            center_index,
            angle_degrees,
        } => CircularConstraint::RotateCircle {
            source: Box::new(clone_circular_constraint(source)),
            center_index: *center_index,
            angle_degrees: *angle_degrees,
        },
        CircularConstraint::CircleArc {
            center_index,
            start_index,
            end_index,
        } => CircularConstraint::CircleArc {
            center_index: *center_index,
            start_index: *start_index,
            end_index: *end_index,
        },
        CircularConstraint::ThreePointArc {
            start_index,
            mid_index,
            end_index,
        } => CircularConstraint::ThreePointArc {
            start_index: *start_index,
            mid_index: *mid_index,
            end_index: *end_index,
        },
    }
}

fn clone_line_constraint(constraint: &LineConstraint) -> LineConstraint {
    constraint.clone()
}

pub(super) fn compute_scene_bounds(
    analysis: &SceneAnalysis,
    shapes: &CollectedShapes,
    labels: &[TextLabel],
    world_point_positions: &[PointRecord],
) -> BoundsData {
    let bounds_lines = shapes
        .lines
        .iter()
        .chain(shapes.trace_lines.iter())
        .chain(shapes.axes.iter())
        .chain(shapes.post_function_lines.iter())
        .cloned()
        .collect::<Vec<_>>();
    let bounds_polygons = shapes.polygons.clone();
    let bounds_circles = shapes
        .circles
        .iter()
        .filter(|circle| {
            !matches!(
                &circle.binding,
                Some(ShapeBinding::MatrixApply { matrices, .. })
                    if matches!(
                        matrices.as_slice(),
                        [crate::runtime::scene::GeometryTransformBinding::TranslateDelta { .. }]
                            | [crate::runtime::scene::GeometryTransformBinding::TranslateVector { .. }]
                    )
            ) && circle.debug.is_some()
        })
        .cloned()
        .collect::<Vec<_>>();
    let bounds_arcs = shapes.arcs.clone();

    let mut bounds = collect_bounds(
        &analysis.graph_ref,
        BoundsInputs {
            segments: &bounds_lines,
            measurements: &[],
            axes: &[],
            polygons: &bounds_polygons,
            circles: &bounds_circles,
            arcs: &bounds_arcs,
            labels,
            points_only: world_point_positions,
        },
    );
    include_line_bounds(&mut bounds, &analysis.function_plots, &analysis.graph_ref);
    let use_saved_viewport = analysis.saved_viewport.is_some();
    if let Some(viewport) = analysis.saved_viewport.filter(|_| use_saved_viewport) {
        bounds = viewport;
    } else if let Some(viewport) = analysis.document_viewport {
        bounds = viewport;
    } else {
        if let Some((domain_min_x, domain_max_x)) = analysis.function_plot_domain {
            bounds.min_x = bounds.min_x.min(domain_min_x);
            bounds.max_x = bounds.max_x.max(domain_max_x);
            bounds.min_y = bounds.min_y.min(0.0);
            bounds.max_y = bounds.max_y.max(0.0);
        }
        expand_bounds(&mut bounds);
    }

    BoundsData {
        bounds,
        use_saved_viewport,
    }
}

pub(super) fn assemble_scene(
    analysis: SceneAnalysis,
    shapes: CollectedShapes,
    labels: Vec<TextLabel>,
    world_data: WorldData,
    bounds_data: BoundsData,
    artifacts: SceneAssemblyArtifacts,
) -> Scene {
    let CollectedShapes {
        lines,
        trace_lines,
        axes,
        post_function_lines,
        polygons,
        circles,
        arcs,
    } = shapes;

    let raw_lines = lines
        .into_iter()
        .chain(trace_lines)
        .chain(axes)
        .chain(analysis.function_plots.iter().cloned())
        .chain(post_function_lines)
        .collect::<Vec<_>>();
    let functions =
        remap_function_line_indices(artifacts.functions, &analysis.function_plots, &raw_lines);

    Scene {
        object_graph: Default::default(),
        payload_dependencies: artifacts.payload_dependencies,
        background_color: analysis.background_color,
        graph_mode: analysis.graph_mode,
        pi_mode: analysis.pi_mode,
        saved_viewport: bounds_data.use_saved_viewport,
        y_up: analysis.graph_ref.is_some(),
        origin: analysis
            .graph_ref
            .as_ref()
            .map(|transform| to_world(&transform.origin_raw, &analysis.graph_ref)),
        graph_transform: analysis.graph_ref.clone(),
        bounds: bounds_data.bounds,
        images: artifacts
            .images
            .into_iter()
            .map(|image| SceneImage {
                top_left: if image.screen_space {
                    image.top_left
                } else {
                    to_world(&image.top_left, &analysis.graph_ref)
                },
                bottom_right: if image.screen_space {
                    image.bottom_right
                } else {
                    to_world(&image.bottom_right, &analysis.graph_ref)
                },
                src: image.src,
                visible: image.visible,
                screen_space: image.screen_space,
                debug: image.debug,
            })
            .collect(),
        lines: raw_lines
            .into_iter()
            .map(|line| world_line_shape(line, &analysis.graph_ref, &bounds_data.bounds))
            .collect(),
        polygons: polygons
            .into_iter()
            .map(|polygon| PolygonShape {
                points: polygon
                    .points
                    .into_iter()
                    .map(|point| to_world(&point, &analysis.graph_ref))
                    .collect(),
                color: polygon.color,
                color_binding: polygon.color_binding,
                visible: polygon.visible,
                binding: polygon.binding,
                debug: polygon.debug,
            })
            .collect(),
        circles: circles
            .into_iter()
            .map(|circle| SceneCircle {
                center: to_world(&circle.center, &analysis.graph_ref),
                radius_point: to_world(&circle.radius_point, &analysis.graph_ref),
                color: circle.color,
                fill_color: circle.fill_color,
                fill_visible: circle.fill_visible,
                fill_color_binding: circle.fill_color_binding,
                dashed: circle.dashed,
                visible: circle.visible,
                binding: world_shape_binding(circle.binding, &analysis.graph_ref),
                debug: circle.debug,
            })
            .collect(),
        arcs: arcs
            .into_iter()
            .map(|arc| SceneArc {
                points: arc
                    .points
                    .map(|point| to_world(&point, &analysis.graph_ref)),
                color: arc.color,
                center: arc
                    .center
                    .map(|center| to_world(&center, &analysis.graph_ref)),
                counterclockwise: arc.counterclockwise,
                visible: arc.visible,
                binding: arc.binding,
                debug: arc.debug,
            })
            .collect(),
        labels: labels
            .into_iter()
            .map(|label| TextLabel {
                anchor: if label.screen_space {
                    label.anchor
                } else {
                    to_world(&label.anchor, &analysis.graph_ref)
                },
                text: label.text,
                rich_markup: label.rich_markup,
                color: label.color,
                font_size: label.font_size,
                font_family: label.font_family,
                visible: label.visible,
                binding: world_label_binding(label.binding, &analysis.graph_ref),
                screen_space: label.screen_space,
                hotspots: label.hotspots,
                debug: label.debug,
            })
            .collect(),
        points: world_data.world_points,
        point_iterations: world_data.point_iterations,
        circle_iterations: artifacts.circle_iterations,
        line_iterations: artifacts
            .line_iterations
            .into_iter()
            .map(|family| world_line_iteration_family(family, &analysis.graph_ref))
            .collect(),
        polygon_iterations: artifacts
            .polygon_iterations
            .into_iter()
            .map(|family| world_polygon_iteration_family(family, &analysis.graph_ref))
            .collect(),
        label_iterations: artifacts.label_iterations,
        iteration_tables: artifacts.iteration_tables,
        buttons: artifacts.buttons,
        parameters: artifacts.parameters,
        scalars: artifacts.scalars,
        functions,
        function_definitions: artifacts.function_definitions,
    }
}

fn world_label_delta(
    dx: f64,
    dy: f64,
    graph_ref: &Option<crate::runtime::geometry::GraphTransform>,
) -> (f64, f64) {
    if let Some(transform) = graph_ref {
        (dx / transform.raw_per_unit, -dy / transform.raw_per_unit)
    } else {
        (dx, dy)
    }
}

fn world_shape_binding(
    binding: Option<ShapeBinding>,
    graph_ref: &Option<crate::runtime::geometry::GraphTransform>,
) -> Option<ShapeBinding> {
    match binding {
        Some(ShapeBinding::ParameterRadiusCircle {
            center_index,
            parameter_name,
            raw_per_unit,
        }) => Some(ShapeBinding::ParameterRadiusCircle {
            center_index,
            parameter_name,
            raw_per_unit: graph_ref.as_ref().map_or(raw_per_unit, |transform| {
                raw_per_unit / transform.raw_per_unit
            }),
        }),
        other => other,
    }
}

fn world_label_binding(
    binding: Option<TextLabelBinding>,
    graph_ref: &Option<crate::runtime::geometry::GraphTransform>,
) -> Option<TextLabelBinding> {
    match binding {
        Some(TextLabelBinding::PointBoundExpressionValue {
            point_index,
            anchor_dx,
            anchor_dy,
            parameter_name,
            result_name,
            expr_label,
            expr,
            parameter_group_ordinals,
        }) => {
            let (anchor_dx, anchor_dy) = world_label_delta(anchor_dx, anchor_dy, graph_ref);
            Some(TextLabelBinding::PointBoundExpressionValue {
                point_index,
                anchor_dx,
                anchor_dy,
                parameter_name,
                result_name,
                expr_label,
                expr,
                parameter_group_ordinals,
            })
        }
        Some(TextLabelBinding::PointAnchor {
            point_index,
            anchor_dx,
            anchor_dy,
            anchor_y_point_index,
            anchor_y_dy,
        }) => {
            let (anchor_dx, anchor_dy) = world_label_delta(anchor_dx, anchor_dy, graph_ref);
            let anchor_y_dy = anchor_y_dy.map(|dy| world_label_delta(0.0, dy, graph_ref).1);
            Some(TextLabelBinding::PointAnchor {
                point_index,
                anchor_dx,
                anchor_dy,
                anchor_y_point_index,
                anchor_y_dy,
            })
        }
        Some(TextLabelBinding::PointExpressionValue {
            point_index,
            anchor_dx,
            anchor_dy,
            anchor_y_point_index,
            anchor_y_dy,
            parameter_name,
            expr,
        }) => {
            let (anchor_dx, anchor_dy) = world_label_delta(anchor_dx, anchor_dy, graph_ref);
            let anchor_y_dy = anchor_y_dy.map(|dy| world_label_delta(0.0, dy, graph_ref).1);
            Some(TextLabelBinding::PointExpressionValue {
                point_index,
                anchor_dx,
                anchor_dy,
                anchor_y_point_index,
                anchor_y_dy,
                parameter_name,
                expr,
            })
        }
        other => other,
    }
}

fn remap_function_line_indices(
    functions: Vec<crate::runtime::scene::SceneFunction>,
    function_plots: &[LineShape],
    raw_lines: &[LineShape],
) -> Vec<crate::runtime::scene::SceneFunction> {
    functions
        .into_iter()
        .enumerate()
        .map(|(index, mut function)| {
            if let Some(plot) = function_plots.get(index)
                && let Some(line_index) = raw_lines
                    .iter()
                    .position(|line| line_shape_matches(line, plot))
            {
                function.line_index = Some(line_index);
            }
            function
        })
        .collect()
}

fn line_shape_matches(left: &LineShape, right: &LineShape) -> bool {
    left.points.len() == right.points.len()
        && left.color == right.color
        && left.dashed == right.dashed
        && left.visible == right.visible
        && left
            .points
            .iter()
            .zip(&right.points)
            .all(|(left, right)| (left.x - right.x).abs() < 1e-9 && (left.y - right.y).abs() < 1e-9)
}
