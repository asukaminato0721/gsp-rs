use crate::format::PointRecord;
use crate::runtime::geometry::{include_line_bounds, to_world};
use crate::runtime::scene::{
    CircleIterationFamily, CircularConstraint, LabelIterationFamily, LineBinding, LineConstraint,
    LineIterationFamily, LineShape, PointIterationFamily, PolygonIterationFamily, PolygonShape,
    Scene, SceneArc, SceneCircle, SceneImage, ScenePoint, ScenePointConstraint, TextLabel,
};

use super::graph::{collect_bounds, dedupe_line_shapes, expand_bounds};
use super::world::{world_line_iteration_family, world_line_shape, world_polygon_iteration_family};
use super::{BoundsData, CollectedShapes, SceneAnalysis, WorldData};

pub(super) fn build_world_data(
    analysis: &SceneAnalysis,
    visible_points: &[ScenePoint],
    derived_iteration_points: &[ScenePoint],
    standalone_parameter_points: &[ScenePoint],
    raw_point_iterations: Vec<super::points::RawPointIterationFamily>,
) -> WorldData {
    let world_points = visible_points
        .iter()
        .chain(derived_iteration_points.iter())
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
                ScenePointConstraint::OnRay {
                    start_index,
                    end_index,
                    t,
                } => ScenePointConstraint::OnRay {
                    start_index: *start_index,
                    end_index: *end_index,
                    t: *t,
                },
                ScenePointConstraint::OnPolyline {
                    function_key,
                    points,
                    segment_index,
                    t,
                } => ScenePointConstraint::OnPolyline {
                    function_key: *function_key,
                    points: points
                        .iter()
                        .map(|point| to_world(point, &analysis.graph_ref))
                        .collect(),
                    segment_index: *segment_index,
                    t: *t,
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
                ScenePointConstraint::LineIntersection { left, right } => {
                    ScenePointConstraint::LineIntersection {
                        left: clone_line_constraint(left),
                        right: clone_line_constraint(right),
                    }
                }
                ScenePointConstraint::LineTraceIntersection {
                    line,
                    point_index,
                    x_min,
                    x_max,
                    sample_count,
                } => ScenePointConstraint::LineTraceIntersection {
                    line: clone_line_constraint(line),
                    point_index: *point_index,
                    x_min: *x_min,
                    x_max: *x_max,
                    sample_count: *sample_count,
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
            binding: point.binding.clone(),
            debug: point.debug.clone(),
        })
        .collect::<Vec<_>>();

    let world_point_positions = world_points
        .iter()
        .filter(|point| point.visible)
        .map(|point| point.position.clone())
        .collect::<Vec<_>>();

    let point_iterations = raw_point_iterations
        .into_iter()
        .map(|family| match family {
            super::points::RawPointIterationFamily::Offset {
                seed_index,
                dx,
                dy,
                depth,
                parameter_name,
            } => {
                let (dx, dy) = if let Some(transform) = &analysis.graph_ref {
                    (dx / transform.raw_per_unit, -dy / transform.raw_per_unit)
                } else {
                    (dx, dy)
                };
                PointIterationFamily::Offset {
                    seed_index,
                    dx,
                    dy,
                    depth,
                    parameter_name,
                }
            }
            super::points::RawPointIterationFamily::RotateChain {
                seed_index,
                center_index,
                angle_degrees,
                depth,
            } => PointIterationFamily::RotateChain {
                seed_index,
                center_index,
                angle_degrees,
                depth,
            },
            super::points::RawPointIterationFamily::Rotate {
                source_index,
                center_index,
                angle_expr,
                depth,
                parameter_name,
            } => PointIterationFamily::Rotate {
                source_index,
                center_index,
                angle_expr,
                depth,
                parameter_name,
            },
        })
        .collect::<Vec<_>>();

    WorldData {
        world_points,
        world_point_positions,
        point_iterations,
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
        CircularConstraint::TranslateCircle { source, dx, dy } => {
            CircularConstraint::TranslateCircle {
                source: Box::new(clone_circular_constraint(source)),
                dx: *dx,
                dy: *dy,
            }
        }
        CircularConstraint::ScaleCircle {
            source,
            center_index,
            factor,
        } => CircularConstraint::ScaleCircle {
            source: Box::new(clone_circular_constraint(source)),
            center_index: *center_index,
            factor: *factor,
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
    match constraint {
        LineConstraint::Segment {
            start_index,
            end_index,
        } => LineConstraint::Segment {
            start_index: *start_index,
            end_index: *end_index,
        },
        LineConstraint::Line {
            start_index,
            end_index,
        } => LineConstraint::Line {
            start_index: *start_index,
            end_index: *end_index,
        },
        LineConstraint::Ray {
            start_index,
            end_index,
        } => LineConstraint::Ray {
            start_index: *start_index,
            end_index: *end_index,
        },
        LineConstraint::PerpendicularLine {
            through_index,
            line_start_index,
            line_end_index,
        } => LineConstraint::PerpendicularLine {
            through_index: *through_index,
            line_start_index: *line_start_index,
            line_end_index: *line_end_index,
        },
        LineConstraint::ParallelLine {
            through_index,
            line_start_index,
            line_end_index,
        } => LineConstraint::ParallelLine {
            through_index: *through_index,
            line_start_index: *line_start_index,
            line_end_index: *line_end_index,
        },
        LineConstraint::AngleBisectorRay {
            start_index,
            vertex_index,
            end_index,
        } => LineConstraint::AngleBisectorRay {
            start_index: *start_index,
            vertex_index: *vertex_index,
            end_index: *end_index,
        },
        LineConstraint::Translated {
            line,
            vector_start_index,
            vector_end_index,
        } => LineConstraint::Translated {
            line: Box::new(clone_line_constraint(line)),
            vector_start_index: *vector_start_index,
            vector_end_index: *vector_end_index,
        },
    }
}

pub(super) fn compute_scene_bounds(
    analysis: &SceneAnalysis,
    shapes: &CollectedShapes,
    labels: &[TextLabel],
    world_point_positions: &[PointRecord],
) -> BoundsData {
    let bounds_lines = shapes
        .polylines
        .iter()
        .chain(shapes.direct_lines.iter())
        .chain(shapes.rays.iter())
        .chain(shapes.translated_lines.iter())
        .chain(shapes.segment_markers.iter())
        .chain(shapes.rotated_lines.iter())
        .chain(shapes.scaled_lines.iter())
        .chain(shapes.reflected_lines.iter())
        .chain(shapes.derived_segments.iter())
        .chain(shapes.measurements.iter())
        .chain(shapes.coordinate_traces.iter())
        .chain(shapes.axes.iter())
        .chain(shapes.iteration_lines.iter())
        .chain(shapes.carried_iteration_lines.iter())
        .cloned()
        .collect::<Vec<_>>();
    let bounds_polygons = shapes
        .polygons
        .iter()
        .chain(shapes.translated_polygons.iter())
        .chain(shapes.rotated_polygons.iter())
        .chain(shapes.transformed_polygons.iter())
        .chain(shapes.reflected_polygons.iter())
        .chain(shapes.iteration_polygons.iter())
        .chain(shapes.carried_iteration_polygons.iter())
        .cloned()
        .collect::<Vec<_>>();
    let bounds_circles = shapes
        .circles
        .iter()
        .chain(shapes.rotated_circles.iter())
        .chain(shapes.transformed_circles.iter())
        .chain(shapes.reflected_circles.iter())
        .cloned()
        .collect::<Vec<_>>();
    let bounds_arcs = shapes.arcs.clone();

    let mut bounds = collect_bounds(
        &analysis.graph_ref,
        &bounds_lines,
        &[],
        &[],
        &bounds_polygons,
        &bounds_circles,
        &bounds_arcs,
        labels,
        world_point_positions,
    );
    include_line_bounds(&mut bounds, &analysis.function_plots, &analysis.graph_ref);
    include_line_bounds(&mut bounds, &shapes.synthetic_axes, &analysis.graph_ref);
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
    circle_iterations: Vec<CircleIterationFamily>,
    line_iterations: Vec<LineIterationFamily>,
    polygon_iterations: Vec<PolygonIterationFamily>,
    label_iterations: Vec<LabelIterationFamily>,
    iteration_tables: Vec<crate::runtime::scene::IterationTable>,
    buttons: Vec<crate::runtime::scene::SceneButton>,
    images: Vec<SceneImage>,
    parameters: Vec<crate::runtime::scene::SceneParameter>,
    functions: Vec<crate::runtime::scene::SceneFunction>,
) -> Scene {
    let CollectedShapes {
        polylines,
        direct_lines,
        rays,
        translated_lines,
        segment_markers,
        derived_segments,
        rotated_lines,
        scaled_lines,
        reflected_lines,
        carried_iteration_lines,
        carried_iteration_polygons,
        carried_iteration_circles,
        measurements,
        coordinate_traces,
        axes,
        iteration_polygon_indices: _,
        polygons,
        circles,
        arcs,
        translated_circles,
        rotated_circles,
        transformed_circles,
        reflected_circles,
        translated_polygons,
        rotated_polygons,
        transformed_polygons,
        reflected_polygons,
        iteration_lines,
        iteration_polygons,
        synthetic_axes,
    } = shapes;

    let raw_polygons = polygons
        .into_iter()
        .chain(translated_polygons)
        .chain(rotated_polygons)
        .chain(transformed_polygons)
        .chain(reflected_polygons)
        .chain(iteration_polygons)
        .chain(carried_iteration_polygons)
        .collect::<Vec<_>>();

    let raw_lines = suppress_polygon_edge_segments(
        dedupe_line_shapes(
            polylines
                .into_iter()
                .chain(direct_lines)
                .chain(rays)
                .chain(translated_lines)
                .chain(segment_markers)
                .chain(rotated_lines)
                .chain(scaled_lines)
                .chain(reflected_lines)
                .chain(derived_segments)
                .chain(measurements)
                .chain(coordinate_traces)
                .chain(axes)
                .chain(analysis.function_plots)
                .chain(synthetic_axes)
                .chain(iteration_lines)
                .chain(carried_iteration_lines)
                .collect(),
        ),
        &raw_polygons,
    );

    Scene {
        graph_mode: analysis.graph_mode,
        pi_mode: analysis.pi_mode,
        saved_viewport: bounds_data.use_saved_viewport,
        y_up: analysis.graph_mode,
        origin: analysis
            .graph_ref
            .as_ref()
            .map(|transform| to_world(&transform.origin_raw, &analysis.graph_ref)),
        bounds: bounds_data.bounds,
        images: images
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
                screen_space: image.screen_space,
                debug: image.debug,
            })
            .collect(),
        lines: raw_lines
            .into_iter()
            .map(|line| world_line_shape(line, &analysis.graph_ref, &bounds_data.bounds))
            .collect(),
        polygons: raw_polygons
            .into_iter()
            .map(|polygon| PolygonShape {
                points: polygon
                    .points
                    .into_iter()
                    .map(|point| to_world(&point, &analysis.graph_ref))
                    .collect(),
                color: polygon.color,
                visible: polygon.visible,
                binding: polygon.binding,
                debug: polygon.debug,
            })
            .collect(),
        circles: circles
            .into_iter()
            .chain(carried_iteration_circles)
            .chain(translated_circles)
            .chain(rotated_circles)
            .chain(transformed_circles)
            .chain(reflected_circles)
            .map(|circle| SceneCircle {
                center: to_world(&circle.center, &analysis.graph_ref),
                radius_point: to_world(&circle.radius_point, &analysis.graph_ref),
                color: circle.color,
                fill_color: circle.fill_color,
                fill_color_binding: circle.fill_color_binding,
                dashed: circle.dashed,
                visible: circle.visible,
                binding: circle.binding,
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
                visible: label.visible,
                binding: label.binding,
                screen_space: label.screen_space,
                hotspots: label.hotspots,
                debug: label.debug,
            })
            .collect(),
        points: world_data.world_points,
        point_iterations: world_data.point_iterations,
        circle_iterations,
        line_iterations: line_iterations
            .into_iter()
            .map(|family| world_line_iteration_family(family, &analysis.graph_ref))
            .collect(),
        polygon_iterations: polygon_iterations
            .into_iter()
            .map(|family| world_polygon_iteration_family(family, &analysis.graph_ref))
            .collect(),
        label_iterations,
        iteration_tables,
        buttons,
        parameters,
        functions,
    }
}

fn suppress_polygon_edge_segments(
    lines: Vec<LineShape>,
    polygons: &[PolygonShape],
) -> Vec<LineShape> {
    lines
        .into_iter()
        .filter(|line| {
            matches!(line.binding, Some(LineBinding::Segment { .. }))
                .then(|| {
                    !polygons
                        .iter()
                        .any(|polygon| polygon_has_matching_edge(polygon, line))
                })
                .unwrap_or(true)
        })
        .collect()
}

fn polygon_has_matching_edge(polygon: &PolygonShape, line: &LineShape) -> bool {
    if polygon.points.len() < 3 || line.points.len() != 2 {
        return false;
    }
    polygon
        .points
        .iter()
        .zip(polygon.points.iter().cycle().skip(1))
        .take(polygon.points.len())
        .any(|(start, end)| points_match_segment(start, end, &line.points[0], &line.points[1]))
}

fn points_match_segment(
    left_start: &PointRecord,
    left_end: &PointRecord,
    right_start: &PointRecord,
    right_end: &PointRecord,
) -> bool {
    points_equal(left_start, right_start) && points_equal(left_end, right_end)
        || points_equal(left_start, right_end) && points_equal(left_end, right_start)
}

fn points_equal(left: &PointRecord, right: &PointRecord) -> bool {
    (left.x - right.x).abs() < 1e-6 && (left.y - right.y).abs() < 1e-6
}
