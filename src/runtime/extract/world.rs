use crate::format::PointRecord;
use crate::runtime::geometry::{
    Bounds, GraphTransform, clip_line_to_bounds, clip_ray_to_bounds, to_world,
};
use crate::runtime::scene::{LineBinding, LineIterationFamily, LineShape, PolygonIterationFamily};

pub(super) fn world_line_shape(
    line: LineShape,
    graph_ref: &Option<GraphTransform>,
    bounds: &Bounds,
) -> LineShape {
    let mut world_points = line
        .points
        .into_iter()
        .map(|point| to_world(&point, graph_ref))
        .collect::<Vec<_>>();

    if let Some(binding) = &line.binding {
        let clipped = match binding {
            LineBinding::Segment { .. } => None,
            LineBinding::AngleBisectorRay { .. } if world_points.len() >= 2 => {
                clip_ray_to_bounds(&world_points[0], &world_points[1], bounds)
            }
            LineBinding::PerpendicularLine { .. } if world_points.len() >= 2 => {
                clip_line_to_bounds(&world_points[0], &world_points[1], bounds)
            }
            LineBinding::ParallelLine { .. } if world_points.len() >= 2 => {
                clip_line_to_bounds(&world_points[0], &world_points[1], bounds)
            }
            LineBinding::Line { .. } if world_points.len() >= 2 => {
                clip_line_to_bounds(&world_points[0], &world_points[1], bounds)
            }
            LineBinding::Ray { .. } if world_points.len() >= 2 => {
                clip_ray_to_bounds(&world_points[0], &world_points[1], bounds)
            }
            _ => None,
        };
        if let Some([start, end]) = clipped {
            world_points = vec![start, end];
        }
    }

    LineShape {
        points: world_points,
        color: line.color,
        dashed: line.dashed,
        visible: line.visible,
        binding: line.binding,
    }
}

fn world_delta(delta: &PointRecord, graph_ref: &Option<GraphTransform>) -> PointRecord {
    let zero = PointRecord { x: 0.0, y: 0.0 };
    let world_zero = to_world(&zero, graph_ref);
    let world_delta = to_world(delta, graph_ref);
    PointRecord {
        x: world_delta.x - world_zero.x,
        y: world_delta.y - world_zero.y,
    }
}

pub(super) fn world_line_iteration_family(
    family: LineIterationFamily,
    graph_ref: &Option<GraphTransform>,
) -> LineIterationFamily {
    let delta = world_delta(
        &PointRecord {
            x: family.dx,
            y: family.dy,
        },
        graph_ref,
    );
    LineIterationFamily {
        dx: delta.x,
        dy: delta.y,
        secondary_dx: match (family.secondary_dx, family.secondary_dy) {
            (Some(dx), Some(dy)) => Some(world_delta(&PointRecord { x: dx, y: dy }, graph_ref).x),
            _ => None,
        },
        secondary_dy: match (family.secondary_dx, family.secondary_dy) {
            (Some(dx), Some(dy)) => Some(world_delta(&PointRecord { x: dx, y: dy }, graph_ref).y),
            _ => None,
        },
        ..family
    }
}

pub(super) fn world_polygon_iteration_family(
    family: PolygonIterationFamily,
    graph_ref: &Option<GraphTransform>,
) -> PolygonIterationFamily {
    let delta = world_delta(
        &PointRecord {
            x: family.dx,
            y: family.dy,
        },
        graph_ref,
    );
    PolygonIterationFamily {
        dx: delta.x,
        dy: delta.y,
        secondary_dx: match (family.secondary_dx, family.secondary_dy) {
            (Some(dx), Some(dy)) => Some(world_delta(&PointRecord { x: dx, y: dy }, graph_ref).x),
            _ => None,
        },
        secondary_dy: match (family.secondary_dx, family.secondary_dy) {
            (Some(dx), Some(dy)) => Some(world_delta(&PointRecord { x: dx, y: dy }, graph_ref).y),
            _ => None,
        },
        ..family
    }
}
