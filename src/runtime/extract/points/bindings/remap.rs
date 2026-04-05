use crate::runtime::extract::CircleShape;
use crate::runtime::scene::{
    LineBinding, LineShape, PolygonShape, ShapeBinding, TextLabel, TextLabelBinding,
};

pub(crate) fn remap_label_bindings(
    labels: &mut [TextLabel],
    group_to_point_index: &[Option<usize>],
) {
    for label in labels {
        let Some(binding) = label.binding.as_mut() else {
            continue;
        };
        if let TextLabelBinding::PointExpressionValue { point_index, .. } = binding {
            let Some(mapped_index) = group_to_point_index
                .get(*point_index)
                .and_then(|mapped_index| *mapped_index)
            else {
                label.binding = None;
                continue;
            };
            *point_index = mapped_index;
            continue;
        }
        let point_index = match binding {
            TextLabelBinding::ParameterValue { .. } | TextLabelBinding::ExpressionValue { .. } => {
                continue;
            }
            TextLabelBinding::PolygonBoundaryParameter { point_index, .. } => point_index,
            TextLabelBinding::SegmentParameter { point_index, .. } => point_index,
            TextLabelBinding::CircleParameter { point_index, .. } => point_index,
            TextLabelBinding::PointExpressionValue { .. } => unreachable!(),
        };
        let Some(mapped_index) = group_to_point_index
            .get(*point_index)
            .and_then(|mapped_index| *mapped_index)
        else {
            label.binding = None;
            continue;
        };
        *point_index = mapped_index;
    }
}

pub(crate) fn remap_circle_bindings(
    circles: &mut [CircleShape],
    group_to_point_index: &[Option<usize>],
    group_to_circle_index: &[Option<usize>],
) {
    for circle in circles {
        let Some(binding) = circle.binding.as_mut() else {
            continue;
        };
        let (source_index, center_index) = match binding {
            ShapeBinding::PointRadiusCircle {
                center_index,
                radius_index,
            } => {
                let Some(mapped_center_index) = group_to_point_index
                    .get(*center_index)
                    .and_then(|mapped_index| *mapped_index)
                else {
                    circle.binding = None;
                    continue;
                };
                let Some(mapped_radius_index) = group_to_point_index
                    .get(*radius_index)
                    .and_then(|mapped_index| *mapped_index)
                else {
                    circle.binding = None;
                    continue;
                };
                *center_index = mapped_center_index;
                *radius_index = mapped_radius_index;
                continue;
            }
            ShapeBinding::SegmentRadiusCircle {
                center_index,
                line_start_index,
                line_end_index,
            } => {
                let Some(mapped_center_index) = group_to_point_index
                    .get(*center_index)
                    .and_then(|mapped_index| *mapped_index)
                else {
                    circle.binding = None;
                    continue;
                };
                let Some(mapped_line_start_index) = group_to_point_index
                    .get(*line_start_index)
                    .and_then(|mapped_index| *mapped_index)
                else {
                    circle.binding = None;
                    continue;
                };
                let Some(mapped_line_end_index) = group_to_point_index
                    .get(*line_end_index)
                    .and_then(|mapped_index| *mapped_index)
                else {
                    circle.binding = None;
                    continue;
                };
                *center_index = mapped_center_index;
                *line_start_index = mapped_line_start_index;
                *line_end_index = mapped_line_end_index;
                continue;
            }
            ShapeBinding::TranslateCircle { source_index, .. } => {
                let Some(mapped_source_index) = group_to_circle_index
                    .get(*source_index)
                    .and_then(|mapped_index| *mapped_index)
                else {
                    circle.binding = None;
                    continue;
                };
                *source_index = mapped_source_index;
                continue;
            }
            ShapeBinding::RotateCircle {
                source_index,
                center_index,
                ..
            } => (source_index, center_index),
            ShapeBinding::ScaleCircle {
                source_index,
                center_index,
                ..
            } => (source_index, center_index),
            ShapeBinding::ReflectCircle {
                source_index,
                line_start_index,
                line_end_index,
            } => {
                let Some(mapped_source_index) = group_to_circle_index
                    .get(*source_index)
                    .and_then(|mapped_index| *mapped_index)
                else {
                    circle.binding = None;
                    continue;
                };
                let Some(mapped_line_start_index) = group_to_point_index
                    .get(*line_start_index)
                    .and_then(|mapped_index| *mapped_index)
                else {
                    circle.binding = None;
                    continue;
                };
                let Some(mapped_line_end_index) = group_to_point_index
                    .get(*line_end_index)
                    .and_then(|mapped_index| *mapped_index)
                else {
                    circle.binding = None;
                    continue;
                };
                *source_index = mapped_source_index;
                *line_start_index = mapped_line_start_index;
                *line_end_index = mapped_line_end_index;
                continue;
            }
            _ => continue,
        };
        let Some(mapped_source_index) = group_to_circle_index
            .get(*source_index)
            .and_then(|mapped_index| *mapped_index)
        else {
            circle.binding = None;
            continue;
        };
        let Some(mapped_center_index) = group_to_point_index
            .get(*center_index)
            .and_then(|mapped_index| *mapped_index)
        else {
            circle.binding = None;
            continue;
        };
        *source_index = mapped_source_index;
        *center_index = mapped_center_index;
    }
}

pub(crate) fn remap_polygon_bindings(
    polygons: &mut [PolygonShape],
    group_to_point_index: &[Option<usize>],
    group_to_polygon_index: &[Option<usize>],
) {
    for polygon in polygons {
        let Some(binding) = polygon.binding.as_mut() else {
            continue;
        };
        let (source_index, center_index) = match binding {
            ShapeBinding::TranslatePolygon {
                source_index,
                vector_start_index,
                vector_end_index,
            } => {
                let Some(mapped_source_index) = group_to_polygon_index
                    .get(*source_index)
                    .and_then(|mapped_index| *mapped_index)
                else {
                    polygon.binding = None;
                    continue;
                };
                let Some(mapped_vector_start_index) = group_to_point_index
                    .get(*vector_start_index)
                    .and_then(|mapped_index| *mapped_index)
                else {
                    polygon.binding = None;
                    continue;
                };
                let Some(mapped_vector_end_index) = group_to_point_index
                    .get(*vector_end_index)
                    .and_then(|mapped_index| *mapped_index)
                else {
                    polygon.binding = None;
                    continue;
                };
                *source_index = mapped_source_index;
                *vector_start_index = mapped_vector_start_index;
                *vector_end_index = mapped_vector_end_index;
                continue;
            }
            ShapeBinding::RotatePolygon {
                source_index,
                center_index,
                ..
            } => (source_index, center_index),
            ShapeBinding::ScalePolygon {
                source_index,
                center_index,
                ..
            } => (source_index, center_index),
            ShapeBinding::ReflectPolygon {
                source_index,
                line_start_index,
                line_end_index,
            } => {
                let Some(mapped_source_index) = group_to_polygon_index
                    .get(*source_index)
                    .and_then(|mapped_index| *mapped_index)
                else {
                    polygon.binding = None;
                    continue;
                };
                let Some(mapped_line_start_index) = group_to_point_index
                    .get(*line_start_index)
                    .and_then(|mapped_index| *mapped_index)
                else {
                    polygon.binding = None;
                    continue;
                };
                let Some(mapped_line_end_index) = group_to_point_index
                    .get(*line_end_index)
                    .and_then(|mapped_index| *mapped_index)
                else {
                    polygon.binding = None;
                    continue;
                };
                *source_index = mapped_source_index;
                *line_start_index = mapped_line_start_index;
                *line_end_index = mapped_line_end_index;
                continue;
            }
            _ => continue,
        };
        let Some(mapped_source_index) = group_to_polygon_index
            .get(*source_index)
            .and_then(|mapped_index| *mapped_index)
        else {
            polygon.binding = None;
            continue;
        };
        let Some(mapped_center_index) = group_to_point_index
            .get(*center_index)
            .and_then(|mapped_index| *mapped_index)
        else {
            polygon.binding = None;
            continue;
        };
        *source_index = mapped_source_index;
        *center_index = mapped_center_index;
    }
}

pub(crate) fn remap_line_bindings(
    lines: &mut [LineShape],
    group_to_point_index: &[Option<usize>],
    group_to_line_index: &[Option<usize>],
) {
    for line in lines {
        let Some(binding) = line.binding.as_mut() else {
            continue;
        };
        match binding {
            LineBinding::AngleBisectorRay {
                start_index,
                vertex_index,
                end_index,
            } => {
                let Some(mapped_start_index) = group_to_point_index
                    .get(*start_index)
                    .and_then(|mapped_index| *mapped_index)
                else {
                    line.binding = None;
                    continue;
                };
                let Some(mapped_vertex_index) = group_to_point_index
                    .get(*vertex_index)
                    .and_then(|mapped_index| *mapped_index)
                else {
                    line.binding = None;
                    continue;
                };
                let Some(mapped_end_index) = group_to_point_index
                    .get(*end_index)
                    .and_then(|mapped_index| *mapped_index)
                else {
                    line.binding = None;
                    continue;
                };
                *start_index = mapped_start_index;
                *vertex_index = mapped_vertex_index;
                *end_index = mapped_end_index;
            }
            LineBinding::PerpendicularLine {
                through_index,
                line_start_index,
                line_end_index,
                line_index,
            } => {
                let Some(mapped_through_index) = group_to_point_index
                    .get(*through_index)
                    .and_then(|mapped_index| *mapped_index)
                else {
                    line.binding = None;
                    continue;
                };

                let mapped_line_start_index = line_start_index.and_then(|index| {
                    group_to_point_index
                        .get(index)
                        .and_then(|mapped_index| *mapped_index)
                });
                let mapped_line_end_index = line_end_index.and_then(|index| {
                    group_to_point_index
                        .get(index)
                        .and_then(|mapped_index| *mapped_index)
                });
                let mapped_line_index = line_index.and_then(|index| {
                    group_to_line_index
                        .get(index)
                        .and_then(|mapped_index| *mapped_index)
                });

                if mapped_line_index.is_none()
                    && (mapped_line_start_index.is_none() || mapped_line_end_index.is_none())
                {
                    line.binding = None;
                    continue;
                }

                *through_index = mapped_through_index;
                *line_start_index = mapped_line_start_index;
                *line_end_index = mapped_line_end_index;
                *line_index = mapped_line_index;
            }
            LineBinding::ParallelLine {
                through_index,
                line_start_index,
                line_end_index,
                line_index,
            } => {
                let Some(mapped_through_index) = group_to_point_index
                    .get(*through_index)
                    .and_then(|mapped_index| *mapped_index)
                else {
                    line.binding = None;
                    continue;
                };

                let mapped_line_start_index = line_start_index.and_then(|index| {
                    group_to_point_index
                        .get(index)
                        .and_then(|mapped_index| *mapped_index)
                });
                let mapped_line_end_index = line_end_index.and_then(|index| {
                    group_to_point_index
                        .get(index)
                        .and_then(|mapped_index| *mapped_index)
                });
                let mapped_line_index = line_index.and_then(|index| {
                    group_to_line_index
                        .get(index)
                        .and_then(|mapped_index| *mapped_index)
                });

                if mapped_line_index.is_none()
                    && (mapped_line_start_index.is_none() || mapped_line_end_index.is_none())
                {
                    line.binding = None;
                    continue;
                }

                *through_index = mapped_through_index;
                *line_start_index = mapped_line_start_index;
                *line_end_index = mapped_line_end_index;
                *line_index = mapped_line_index;
            }
            LineBinding::TranslateLine {
                source_index,
                vector_start_index,
                vector_end_index,
            } => {
                let Some(mapped_source_index) = group_to_line_index
                    .get(*source_index)
                    .and_then(|mapped_index| *mapped_index)
                else {
                    line.binding = None;
                    continue;
                };
                let Some(mapped_vector_start_index) = group_to_point_index
                    .get(*vector_start_index)
                    .and_then(|mapped_index| *mapped_index)
                else {
                    line.binding = None;
                    continue;
                };
                let Some(mapped_vector_end_index) = group_to_point_index
                    .get(*vector_end_index)
                    .and_then(|mapped_index| *mapped_index)
                else {
                    line.binding = None;
                    continue;
                };
                *source_index = mapped_source_index;
                *vector_start_index = mapped_vector_start_index;
                *vector_end_index = mapped_vector_end_index;
            }
            LineBinding::Line {
                start_index,
                end_index,
            }
            | LineBinding::Segment {
                start_index,
                end_index,
            }
            | LineBinding::Ray {
                start_index,
                end_index,
            } => {
                let Some(mapped_start_index) = group_to_point_index
                    .get(*start_index)
                    .and_then(|mapped_index| *mapped_index)
                else {
                    line.binding = None;
                    continue;
                };
                let Some(mapped_end_index) = group_to_point_index
                    .get(*end_index)
                    .and_then(|mapped_index| *mapped_index)
                else {
                    line.binding = None;
                    continue;
                };
                *start_index = mapped_start_index;
                *end_index = mapped_end_index;
            }
            LineBinding::AngleMarker {
                start_index,
                vertex_index,
                end_index,
                ..
            } => {
                let Some(mapped_start_index) = group_to_point_index
                    .get(*start_index)
                    .and_then(|mapped_index| *mapped_index)
                else {
                    line.binding = None;
                    continue;
                };
                let Some(mapped_vertex_index) = group_to_point_index
                    .get(*vertex_index)
                    .and_then(|mapped_index| *mapped_index)
                else {
                    line.binding = None;
                    continue;
                };
                let Some(mapped_end_index) = group_to_point_index
                    .get(*end_index)
                    .and_then(|mapped_index| *mapped_index)
                else {
                    line.binding = None;
                    continue;
                };
                *start_index = mapped_start_index;
                *vertex_index = mapped_vertex_index;
                *end_index = mapped_end_index;
            }
            LineBinding::SegmentMarker {
                start_index,
                end_index,
                ..
            } => {
                let Some(mapped_start_index) = group_to_point_index
                    .get(*start_index)
                    .and_then(|mapped_index| *mapped_index)
                else {
                    line.binding = None;
                    continue;
                };
                let Some(mapped_end_index) = group_to_point_index
                    .get(*end_index)
                    .and_then(|mapped_index| *mapped_index)
                else {
                    line.binding = None;
                    continue;
                };
                *start_index = mapped_start_index;
                *end_index = mapped_end_index;
            }
            LineBinding::RotateLine {
                source_index,
                center_index,
                ..
            }
            | LineBinding::ScaleLine {
                source_index,
                center_index,
                ..
            } => {
                let Some(mapped_source_index) = group_to_line_index
                    .get(*source_index)
                    .and_then(|mapped_index| *mapped_index)
                else {
                    line.binding = None;
                    continue;
                };
                let Some(mapped_center_index) = group_to_point_index
                    .get(*center_index)
                    .and_then(|mapped_index| *mapped_index)
                else {
                    line.binding = None;
                    continue;
                };
                *source_index = mapped_source_index;
                *center_index = mapped_center_index;
            }
            LineBinding::ReflectLine {
                source_index,
                line_start_index,
                line_end_index,
            } => {
                let Some(mapped_source_index) = group_to_line_index
                    .get(*source_index)
                    .and_then(|mapped_index| *mapped_index)
                else {
                    line.binding = None;
                    continue;
                };
                let Some(mapped_line_start_index) = group_to_point_index
                    .get(*line_start_index)
                    .and_then(|mapped_index| *mapped_index)
                else {
                    line.binding = None;
                    continue;
                };
                let Some(mapped_line_end_index) = group_to_point_index
                    .get(*line_end_index)
                    .and_then(|mapped_index| *mapped_index)
                else {
                    line.binding = None;
                    continue;
                };
                *source_index = mapped_source_index;
                *line_start_index = mapped_line_start_index;
                *line_end_index = mapped_line_end_index;
            }
            LineBinding::RotateEdge {
                center_index,
                vertex_index,
                ..
            } => {
                let Some(mapped_center_index) = group_to_point_index
                    .get(*center_index)
                    .and_then(|mapped_index| *mapped_index)
                else {
                    line.binding = None;
                    continue;
                };
                let Some(mapped_vertex_index) = group_to_point_index
                    .get(*vertex_index)
                    .and_then(|mapped_index| *mapped_index)
                else {
                    line.binding = None;
                    continue;
                };
                *center_index = mapped_center_index;
                *vertex_index = mapped_vertex_index;
            }
        }
    }
}
