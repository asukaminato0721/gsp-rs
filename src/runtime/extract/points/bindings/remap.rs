use crate::runtime::extract::CircleShape;
use crate::runtime::scene::{
    LineBinding, LineShape, PolygonShape, ShapeBinding, TextLabel, TextLabelBinding,
};

fn mapped_index(mapping: &[Option<usize>], index: usize) -> Option<usize> {
    mapping.get(index).copied().flatten()
}

fn mapped_optional_index(mapping: &[Option<usize>], index: Option<usize>) -> Option<Option<usize>> {
    match index {
        Some(index) => Some(Some(mapped_index(mapping, index)?)),
        None => Some(None),
    }
}

pub(crate) fn remap_label_bindings(
    labels: &mut [TextLabel],
    group_to_point_index: &[Option<usize>],
) {
    for label in labels {
        let Some(binding) = label.binding.as_mut() else {
            continue;
        };
        if let TextLabelBinding::PointExpressionValue { point_index, .. }
        | TextLabelBinding::CustomTransformValue { point_index, .. } = binding
        {
            let Some(mapped_index) = mapped_index(group_to_point_index, *point_index) else {
                label.binding = None;
                continue;
            };
            *point_index = mapped_index;
            continue;
        }
        let point_index = match binding {
            TextLabelBinding::ParameterValue { .. }
            | TextLabelBinding::FunctionLabel { .. }
            | TextLabelBinding::ExpressionValue { .. } => continue,
            TextLabelBinding::PolygonBoundaryExpression { point_index, .. } => point_index,
            TextLabelBinding::PolygonBoundaryParameter { point_index, .. } => point_index,
            TextLabelBinding::SegmentParameter { point_index, .. } => point_index,
            TextLabelBinding::CircleParameter { point_index, .. } => point_index,
            TextLabelBinding::AngleMarkerValue {
                start_index,
                vertex_index,
                end_index,
                ..
            } => {
                let Some(mapped_start_index) = mapped_index(group_to_point_index, *start_index)
                else {
                    label.binding = None;
                    continue;
                };
                let Some(mapped_vertex_index) = mapped_index(group_to_point_index, *vertex_index)
                else {
                    label.binding = None;
                    continue;
                };
                let Some(mapped_end_index) = mapped_index(group_to_point_index, *end_index) else {
                    label.binding = None;
                    continue;
                };
                *start_index = mapped_start_index;
                *vertex_index = mapped_vertex_index;
                *end_index = mapped_end_index;
                continue;
            }
            TextLabelBinding::CustomTransformValue { .. } => unreachable!(),
            TextLabelBinding::PointExpressionValue { .. } => unreachable!(),
        };
        let Some(mapped_index) = mapped_index(group_to_point_index, *point_index) else {
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
                let Some(mapped_center_index) = mapped_index(group_to_point_index, *center_index)
                else {
                    circle.binding = None;
                    continue;
                };
                let Some(mapped_radius_index) = mapped_index(group_to_point_index, *radius_index)
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
                let Some(mapped_center_index) = mapped_index(group_to_point_index, *center_index)
                else {
                    circle.binding = None;
                    continue;
                };
                let Some(mapped_line_start_index) =
                    mapped_index(group_to_point_index, *line_start_index)
                else {
                    circle.binding = None;
                    continue;
                };
                let Some(mapped_line_end_index) =
                    mapped_index(group_to_point_index, *line_end_index)
                else {
                    circle.binding = None;
                    continue;
                };
                *center_index = mapped_center_index;
                *line_start_index = mapped_line_start_index;
                *line_end_index = mapped_line_end_index;
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
                let Some(mapped_source_index) = mapped_index(group_to_circle_index, *source_index)
                else {
                    circle.binding = None;
                    continue;
                };
                let Some(mapped_line_start_index) =
                    mapped_index(group_to_point_index, *line_start_index)
                else {
                    circle.binding = None;
                    continue;
                };
                let Some(mapped_line_end_index) =
                    mapped_index(group_to_point_index, *line_end_index)
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
        let Some(mapped_source_index) = mapped_index(group_to_circle_index, *source_index) else {
            circle.binding = None;
            continue;
        };
        let Some(mapped_center_index) = mapped_index(group_to_point_index, *center_index) else {
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
            ShapeBinding::PointPolygon { vertex_indices } => {
                let mapped = vertex_indices
                    .iter()
                    .map(|group_index| mapped_index(group_to_point_index, *group_index))
                    .collect::<Option<Vec<_>>>();
                let Some(mapped_vertex_indices) = mapped else {
                    polygon.binding = None;
                    continue;
                };
                *vertex_indices = mapped_vertex_indices;
                continue;
            }
            ShapeBinding::ArcBoundaryPolygon {
                center_index,
                start_index,
                mid_index,
                end_index,
                ..
            } => {
                if let Some(mapped_start_index) = mapped_index(group_to_point_index, *start_index) {
                    *start_index = mapped_start_index;
                } else {
                    polygon.binding = None;
                    continue;
                }
                if let Some(mapped_end_index) = mapped_index(group_to_point_index, *end_index) {
                    *end_index = mapped_end_index;
                } else {
                    polygon.binding = None;
                    continue;
                }
                if let Some(index) = center_index {
                    if let Some(mapped_center_index) = mapped_index(group_to_point_index, *index) {
                        *index = mapped_center_index;
                    } else {
                        polygon.binding = None;
                        continue;
                    }
                }
                if let Some(index) = mid_index {
                    if let Some(mapped_mid_index) = mapped_index(group_to_point_index, *index) {
                        *index = mapped_mid_index;
                    } else {
                        polygon.binding = None;
                        continue;
                    }
                }
                continue;
            }
            ShapeBinding::TranslatePolygon {
                source_index,
                vector_start_index,
                vector_end_index,
            } => {
                let Some(mapped_source_index) = mapped_index(group_to_polygon_index, *source_index)
                else {
                    polygon.binding = None;
                    continue;
                };
                let Some(mapped_vector_start_index) =
                    mapped_index(group_to_point_index, *vector_start_index)
                else {
                    polygon.binding = None;
                    continue;
                };
                let Some(mapped_vector_end_index) =
                    mapped_index(group_to_point_index, *vector_end_index)
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
                let Some(mapped_source_index) = mapped_index(group_to_polygon_index, *source_index)
                else {
                    polygon.binding = None;
                    continue;
                };
                let Some(mapped_line_start_index) =
                    mapped_index(group_to_point_index, *line_start_index)
                else {
                    polygon.binding = None;
                    continue;
                };
                let Some(mapped_line_end_index) =
                    mapped_index(group_to_point_index, *line_end_index)
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
        let Some(mapped_source_index) = mapped_index(group_to_polygon_index, *source_index) else {
            polygon.binding = None;
            continue;
        };
        let Some(mapped_center_index) = mapped_index(group_to_point_index, *center_index) else {
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
            LineBinding::GraphHelperLine {
                start_index,
                end_index,
            } => {
                let Some(mapped_start_index) = mapped_index(group_to_point_index, *start_index)
                else {
                    line.binding = None;
                    continue;
                };
                let Some(mapped_end_index) = mapped_index(group_to_point_index, *end_index) else {
                    line.binding = None;
                    continue;
                };
                *start_index = mapped_start_index;
                *end_index = mapped_end_index;
            }
            LineBinding::AngleBisectorRay {
                start_index,
                vertex_index,
                end_index,
            } => {
                let Some(mapped_start_index) = mapped_index(group_to_point_index, *start_index)
                else {
                    line.binding = None;
                    continue;
                };
                let Some(mapped_vertex_index) = mapped_index(group_to_point_index, *vertex_index)
                else {
                    line.binding = None;
                    continue;
                };
                let Some(mapped_end_index) = mapped_index(group_to_point_index, *end_index) else {
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
                let Some(mapped_through_index) = mapped_index(group_to_point_index, *through_index)
                else {
                    line.binding = None;
                    continue;
                };

                let mapped_line_start_index =
                    mapped_optional_index(group_to_point_index, *line_start_index).unwrap_or(None);
                let mapped_line_end_index =
                    mapped_optional_index(group_to_point_index, *line_end_index).unwrap_or(None);
                let mapped_line_index =
                    mapped_optional_index(group_to_line_index, *line_index).unwrap_or(None);

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
                let Some(mapped_through_index) = mapped_index(group_to_point_index, *through_index)
                else {
                    line.binding = None;
                    continue;
                };

                let mapped_line_start_index =
                    mapped_optional_index(group_to_point_index, *line_start_index).unwrap_or(None);
                let mapped_line_end_index =
                    mapped_optional_index(group_to_point_index, *line_end_index).unwrap_or(None);
                let mapped_line_index =
                    mapped_optional_index(group_to_line_index, *line_index).unwrap_or(None);

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
                let Some(mapped_source_index) = mapped_index(group_to_line_index, *source_index)
                else {
                    line.binding = None;
                    continue;
                };
                let Some(mapped_vector_start_index) =
                    mapped_index(group_to_point_index, *vector_start_index)
                else {
                    line.binding = None;
                    continue;
                };
                let Some(mapped_vector_end_index) =
                    mapped_index(group_to_point_index, *vector_end_index)
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
                let Some(mapped_start_index) = mapped_index(group_to_point_index, *start_index)
                else {
                    line.binding = None;
                    continue;
                };
                let Some(mapped_end_index) = mapped_index(group_to_point_index, *end_index) else {
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
                let Some(mapped_start_index) = mapped_index(group_to_point_index, *start_index)
                else {
                    line.binding = None;
                    continue;
                };
                let Some(mapped_vertex_index) = mapped_index(group_to_point_index, *vertex_index)
                else {
                    line.binding = None;
                    continue;
                };
                let Some(mapped_end_index) = mapped_index(group_to_point_index, *end_index) else {
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
                let Some(mapped_start_index) = mapped_index(group_to_point_index, *start_index)
                else {
                    line.binding = None;
                    continue;
                };
                let Some(mapped_end_index) = mapped_index(group_to_point_index, *end_index) else {
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
                let Some(mapped_source_index) = mapped_index(group_to_line_index, *source_index)
                else {
                    line.binding = None;
                    continue;
                };
                let Some(mapped_center_index) = mapped_index(group_to_point_index, *center_index)
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
                let Some(mapped_source_index) = mapped_index(group_to_line_index, *source_index)
                else {
                    line.binding = None;
                    continue;
                };
                let Some(mapped_line_start_index) =
                    mapped_index(group_to_point_index, *line_start_index)
                else {
                    line.binding = None;
                    continue;
                };
                let Some(mapped_line_end_index) =
                    mapped_index(group_to_point_index, *line_end_index)
                else {
                    line.binding = None;
                    continue;
                };
                *source_index = mapped_source_index;
                *line_start_index = mapped_line_start_index;
                *line_end_index = mapped_line_end_index;
            }
            LineBinding::CustomTransformTrace { point_index, .. }
            | LineBinding::CoordinateTrace { point_index, .. } => {
                let Some(mapped_point_index) = mapped_index(group_to_point_index, *point_index)
                else {
                    line.binding = None;
                    continue;
                };
                *point_index = mapped_point_index;
            }
            LineBinding::PointTrace {
                point_index,
                driver_index,
                ..
            } => {
                let Some(mapped_point_index) = mapped_index(group_to_point_index, *point_index)
                else {
                    line.binding = None;
                    continue;
                };
                let Some(mapped_driver_index) = mapped_index(group_to_point_index, *driver_index)
                else {
                    line.binding = None;
                    continue;
                };
                *point_index = mapped_point_index;
                *driver_index = mapped_driver_index;
            }
            LineBinding::RotateEdge {
                center_index,
                vertex_index,
                ..
            } => {
                let Some(mapped_center_index) = mapped_index(group_to_point_index, *center_index)
                else {
                    line.binding = None;
                    continue;
                };
                let Some(mapped_vertex_index) = mapped_index(group_to_point_index, *vertex_index)
                else {
                    line.binding = None;
                    continue;
                };
                *center_index = mapped_center_index;
                *vertex_index = mapped_vertex_index;
            }
            LineBinding::ArcBoundary {
                center_index,
                start_index,
                mid_index,
                end_index,
                ..
            } => {
                let mapped_center_index =
                    mapped_optional_index(group_to_point_index, *center_index).unwrap_or(None);
                if center_index.is_some() && mapped_center_index.is_none() {
                    line.binding = None;
                    continue;
                }
                let Some(mapped_start_index) = mapped_index(group_to_point_index, *start_index)
                else {
                    line.binding = None;
                    continue;
                };
                let mapped_mid_index =
                    mapped_optional_index(group_to_point_index, *mid_index).unwrap_or(None);
                if mid_index.is_some() && mapped_mid_index.is_none() {
                    line.binding = None;
                    continue;
                }
                let Some(mapped_end_index) = mapped_index(group_to_point_index, *end_index) else {
                    line.binding = None;
                    continue;
                };
                *center_index = mapped_center_index;
                *start_index = mapped_start_index;
                *mid_index = mapped_mid_index;
                *end_index = mapped_end_index;
            }
        }
    }
}
