use crate::runtime::extract::shapes::{ArcShape, CircleShape};
use crate::runtime::scene::{
    ArcBinding, AxisBinding, GeometryTransformBinding, LineBinding, LineShape, PolygonShape,
    ScenePoint, ScenePointConstraint, ShapeBinding, TextLabel, TextLabelBinding,
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

pub(crate) fn remap_point_polygon_constraints(
    points: &mut [ScenePoint],
    group_to_polygon_index: &[Option<usize>],
) -> anyhow::Result<()> {
    for point in points {
        let ScenePointConstraint::OnPolygonShapeBoundary { polygon_index, .. } =
            &mut point.constraint
        else {
            continue;
        };
        let polygon_group_index = *polygon_index;
        *polygon_index =
            mapped_index(group_to_polygon_index, polygon_group_index).ok_or_else(|| {
                anyhow::anyhow!(
                    "polygon point constraint references unexported group #{}",
                    polygon_group_index + 1
                )
            })?;
    }
    Ok(())
}

fn remap_axis_binding(
    axis: &mut AxisBinding,
    group_to_point_index: &[Option<usize>],
    group_to_line_index: &[Option<usize>],
) -> bool {
    let mapped_line_start_index =
        mapped_optional_index(group_to_point_index, axis.line_start_index).unwrap_or(None);
    let mapped_line_end_index =
        mapped_optional_index(group_to_point_index, axis.line_end_index).unwrap_or(None);
    let mapped_line_index =
        mapped_optional_index(group_to_line_index, axis.line_index).unwrap_or(None);
    if mapped_line_index.is_none()
        && (mapped_line_start_index.is_none() || mapped_line_end_index.is_none())
    {
        return false;
    }
    axis.line_start_index = mapped_line_start_index;
    axis.line_end_index = mapped_line_end_index;
    axis.line_index = mapped_line_index;
    true
}

fn remap_geometry_transform(
    transform: &mut GeometryTransformBinding,
    group_to_point_index: &[Option<usize>],
    group_to_line_index: &[Option<usize>],
) -> bool {
    match transform {
        GeometryTransformBinding::TranslateDelta { .. } => true,
        GeometryTransformBinding::TranslateVector {
            vector_start_index,
            vector_end_index,
        } => {
            let Some(mapped_vector_start_index) =
                mapped_index(group_to_point_index, *vector_start_index)
            else {
                return false;
            };
            let Some(mapped_vector_end_index) =
                mapped_index(group_to_point_index, *vector_end_index)
            else {
                return false;
            };
            *vector_start_index = mapped_vector_start_index;
            *vector_end_index = mapped_vector_end_index;
            true
        }
        GeometryTransformBinding::Rotate(binding) => {
            let Some(mapped_center_index) =
                mapped_index(group_to_point_index, binding.center_index)
            else {
                return false;
            };
            let mapped_angle_start_index =
                mapped_optional_index(group_to_point_index, binding.angle_start_index)
                    .unwrap_or(None);
            let mapped_angle_vertex_index =
                mapped_optional_index(group_to_point_index, binding.angle_vertex_index)
                    .unwrap_or(None);
            let mapped_angle_end_index =
                mapped_optional_index(group_to_point_index, binding.angle_end_index)
                    .unwrap_or(None);
            if binding.angle_start_index.is_some()
                && (mapped_angle_start_index.is_none()
                    || mapped_angle_vertex_index.is_none()
                    || mapped_angle_end_index.is_none())
            {
                return false;
            }
            binding.center_index = mapped_center_index;
            binding.angle_start_index = mapped_angle_start_index;
            binding.angle_vertex_index = mapped_angle_vertex_index;
            binding.angle_end_index = mapped_angle_end_index;
            true
        }
        GeometryTransformBinding::Scale(binding) => {
            let Some(mapped_center_index) =
                mapped_index(group_to_point_index, binding.center_index)
            else {
                return false;
            };
            binding.center_index = mapped_center_index;
            true
        }
        GeometryTransformBinding::ScaleByRatio(binding) => {
            let Some(center_index) = mapped_index(group_to_point_index, binding.center_index)
            else {
                return false;
            };
            let Some(ratio_origin_index) =
                mapped_index(group_to_point_index, binding.ratio_origin_index)
            else {
                return false;
            };
            let Some(ratio_denominator_index) =
                mapped_index(group_to_point_index, binding.ratio_denominator_index)
            else {
                return false;
            };
            let Some(ratio_numerator_index) =
                mapped_index(group_to_point_index, binding.ratio_numerator_index)
            else {
                return false;
            };
            binding.center_index = center_index;
            binding.ratio_origin_index = ratio_origin_index;
            binding.ratio_denominator_index = ratio_denominator_index;
            binding.ratio_numerator_index = ratio_numerator_index;
            true
        }
        GeometryTransformBinding::Reflect(axis) => {
            remap_axis_binding(axis, group_to_point_index, group_to_line_index)
        }
        GeometryTransformBinding::RotateAroundSourcePoint { .. } => true,
        GeometryTransformBinding::TranslateSourcePointToPoint { target_index, .. } => {
            let Some(mapped_target_index) = mapped_index(group_to_point_index, *target_index)
            else {
                return false;
            };
            *target_index = mapped_target_index;
            true
        }
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
        | TextLabelBinding::PointBoundExpressionValue { point_index, .. }
        | TextLabelBinding::PointAnchor { point_index, .. }
        | TextLabelBinding::CustomTransformValue { point_index, .. } = binding
        {
            let Some(mapped_index) = mapped_index(group_to_point_index, *point_index) else {
                label.binding = None;
                continue;
            };
            *point_index = mapped_index;
            continue;
        }
        if let TextLabelBinding::PointAxisValue {
            point_index,
            origin_index,
            x_unit_index,
            y_unit_index,
            ..
        } = binding
        {
            let Some(new_point_index) = mapped_index(group_to_point_index, *point_index) else {
                label.binding = None;
                continue;
            };
            *point_index = new_point_index;
            *origin_index =
                origin_index.and_then(|index| mapped_index(group_to_point_index, index));
            *x_unit_index =
                x_unit_index.and_then(|index| mapped_index(group_to_point_index, index));
            *y_unit_index =
                y_unit_index.and_then(|index| mapped_index(group_to_point_index, index));
            continue;
        }
        if let TextLabelBinding::PointCoordinateValue {
            point_index,
            origin_index,
            x_unit_index,
            y_unit_index,
            ..
        } = binding
        {
            let Some(new_point_index) = mapped_index(group_to_point_index, *point_index) else {
                label.binding = None;
                continue;
            };
            *point_index = new_point_index;
            *origin_index =
                origin_index.and_then(|index| mapped_index(group_to_point_index, index));
            *x_unit_index =
                x_unit_index.and_then(|index| mapped_index(group_to_point_index, index));
            *y_unit_index =
                y_unit_index.and_then(|index| mapped_index(group_to_point_index, index));
            continue;
        }
        let point_index = match binding {
            TextLabelBinding::ParameterValue { .. }
            | TextLabelBinding::ScalarAlias { .. }
            | TextLabelBinding::ExpressionValue { .. }
            | TextLabelBinding::SequenceExpressionValue { .. }
            | TextLabelBinding::RichTextExpressionValues { .. } => continue,
            TextLabelBinding::PointDistanceRatioValue {
                origin_index,
                denominator_index,
                numerator_index,
                ..
            } => {
                let Some(mapped_origin_index) = mapped_index(group_to_point_index, *origin_index)
                else {
                    label.binding = None;
                    continue;
                };
                let Some(mapped_denominator_index) =
                    mapped_index(group_to_point_index, *denominator_index)
                else {
                    label.binding = None;
                    continue;
                };
                let Some(mapped_numerator_index) =
                    mapped_index(group_to_point_index, *numerator_index)
                else {
                    label.binding = None;
                    continue;
                };
                *origin_index = mapped_origin_index;
                *denominator_index = mapped_denominator_index;
                *numerator_index = mapped_numerator_index;
                continue;
            }
            TextLabelBinding::PointDistanceValue {
                left_index,
                right_index,
                ..
            } => {
                let Some(mapped_left_index) = mapped_index(group_to_point_index, *left_index)
                else {
                    label.binding = None;
                    continue;
                };
                let Some(mapped_right_index) = mapped_index(group_to_point_index, *right_index)
                else {
                    label.binding = None;
                    continue;
                };
                *left_index = mapped_left_index;
                *right_index = mapped_right_index;
                continue;
            }
            TextLabelBinding::PointAngleValue {
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
            TextLabelBinding::PolygonAreaValue { point_indices, .. } => {
                let mut mapped_indices = Vec::with_capacity(point_indices.len());
                let mut missing_point = false;
                for point_index in point_indices.iter().copied() {
                    let Some(mapped_index) = mapped_index(group_to_point_index, point_index) else {
                        missing_point = true;
                        continue;
                    };
                    mapped_indices.push(mapped_index);
                }
                if missing_point {
                    label.binding = None;
                    continue;
                }
                *point_indices = mapped_indices;
                continue;
            }
            TextLabelBinding::PolygonBoundaryParameter { point_index, .. } => point_index,
            TextLabelBinding::LineProjectionParameter {
                point_index,
                start_index,
                end_index,
                ..
            } => {
                let Some(mapped_point_index) = mapped_index(group_to_point_index, *point_index)
                else {
                    label.binding = None;
                    continue;
                };
                let Some(mapped_start_index) = mapped_index(group_to_point_index, *start_index)
                else {
                    label.binding = None;
                    continue;
                };
                let Some(mapped_end_index) = mapped_index(group_to_point_index, *end_index) else {
                    label.binding = None;
                    continue;
                };
                *point_index = mapped_point_index;
                *start_index = mapped_start_index;
                *end_index = mapped_end_index;
                continue;
            }
            TextLabelBinding::PolylineParameter { point_index, .. } => point_index,
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
            TextLabelBinding::PointCoordinateValue { .. } => unreachable!(),
            TextLabelBinding::PointAxisValue { .. } => unreachable!(),
            TextLabelBinding::CustomTransformValue { .. } => unreachable!(),
            TextLabelBinding::PointExpressionValue { .. } => unreachable!(),
            TextLabelBinding::PointBoundExpressionValue { .. } => unreachable!(),
            TextLabelBinding::PointAnchor { .. } => unreachable!(),
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
    group_to_line_index: &[Option<usize>],
) {
    for circle in circles {
        let Some(binding) = circle.binding.as_mut() else {
            continue;
        };
        match binding {
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
            ShapeBinding::ParameterRadiusCircle { center_index, .. }
            | ShapeBinding::ExpressionRadiusCircle { center_index, .. } => {
                let Some(mapped_center_index) = mapped_index(group_to_point_index, *center_index)
                else {
                    circle.binding = None;
                    continue;
                };
                *center_index = mapped_center_index;
                continue;
            }
            ShapeBinding::MatrixApply {
                source_index,
                matrices,
            } => {
                let Some(mapped_source_index) = mapped_index(group_to_circle_index, *source_index)
                else {
                    circle.binding = None;
                    continue;
                };
                *source_index = mapped_source_index;
                if !matrices.iter_mut().all(|matrix| {
                    remap_geometry_transform(matrix, group_to_point_index, group_to_line_index)
                }) {
                    circle.binding = None;
                }
                continue;
            }
            _ => continue,
        }
    }
}

pub(crate) fn remap_arc_bindings(
    arcs: &mut [ArcShape],
    group_to_point_index: &[Option<usize>],
    group_to_circle_index: &[Option<usize>],
    group_to_arc_index: &[Option<usize>],
    group_to_line_index: &[Option<usize>],
) {
    for arc in arcs {
        let Some(binding) = arc.binding.as_mut() else {
            continue;
        };
        let mapped = (|| match binding {
            ArcBinding::CenterArc {
                center_index,
                start_index,
                end_index,
            } => Some(ArcBinding::CenterArc {
                center_index: mapped_index(group_to_point_index, *center_index)?,
                start_index: mapped_index(group_to_point_index, *start_index)?,
                end_index: mapped_index(group_to_point_index, *end_index)?,
            }),
            ArcBinding::CircleArc {
                circle_index,
                start_index,
                end_index,
            } => Some(ArcBinding::CircleArc {
                circle_index: mapped_index(group_to_circle_index, *circle_index)?,
                start_index: mapped_index(group_to_point_index, *start_index)?,
                end_index: mapped_index(group_to_point_index, *end_index)?,
            }),
            ArcBinding::ThreePointArc {
                start_index,
                mid_index,
                end_index,
            } => Some(ArcBinding::ThreePointArc {
                start_index: mapped_index(group_to_point_index, *start_index)?,
                mid_index: mapped_index(group_to_point_index, *mid_index)?,
                end_index: mapped_index(group_to_point_index, *end_index)?,
            }),
            ArcBinding::MatrixApply {
                source_index,
                matrices,
            } => {
                let source_index = mapped_index(group_to_arc_index, *source_index)?;
                let mut matrices = matrices.clone();
                if !matrices.iter_mut().all(|matrix| {
                    remap_geometry_transform(matrix, group_to_point_index, group_to_line_index)
                }) {
                    return None;
                }
                Some(ArcBinding::MatrixApply {
                    source_index,
                    matrices,
                })
            }
        })();
        arc.binding = mapped;
    }
}

pub(crate) fn remap_polygon_bindings(
    polygons: &mut [PolygonShape],
    group_to_point_index: &[Option<usize>],
    group_to_polygon_index: &[Option<usize>],
    group_to_line_index: &[Option<usize>],
) {
    for polygon in polygons {
        let Some(binding) = polygon.binding.as_mut() else {
            continue;
        };
        match binding {
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
            ShapeBinding::MatrixApply {
                source_index,
                matrices,
            } => {
                let Some(mapped_source_index) = mapped_index(group_to_polygon_index, *source_index)
                else {
                    polygon.binding = None;
                    continue;
                };
                *source_index = mapped_source_index;
                if !matrices.iter_mut().all(|matrix| {
                    remap_geometry_transform(matrix, group_to_point_index, group_to_line_index)
                }) {
                    polygon.binding = None;
                }
                continue;
            }
            _ => continue,
        }
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
            LineBinding::MatrixApply {
                source_index,
                source_start_index,
                source_end_index,
                matrices,
            } => {
                *source_index =
                    source_index.and_then(|index| mapped_index(group_to_line_index, index));
                *source_start_index =
                    source_start_index.and_then(|index| mapped_index(group_to_point_index, index));
                *source_end_index =
                    source_end_index.and_then(|index| mapped_index(group_to_point_index, index));
                if source_index.is_none()
                    && !(source_start_index.is_some() && source_end_index.is_some())
                {
                    line.binding = None;
                    continue;
                }
                if !matrices.iter_mut().all(|matrix| {
                    remap_geometry_transform(matrix, group_to_point_index, group_to_line_index)
                }) {
                    line.binding = None;
                }
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
            LineBinding::CustomTransformTrace {
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
            LineBinding::CoordinateTrace { point_index, .. } => {
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
            LineBinding::SegmentTrace {
                start_index,
                end_index,
                driver_index,
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
                let Some(mapped_driver_index) = mapped_index(group_to_point_index, *driver_index)
                else {
                    line.binding = None;
                    continue;
                };
                *start_index = mapped_start_index;
                *end_index = mapped_end_index;
                *driver_index = mapped_driver_index;
            }
            LineBinding::ColorizedSpectrum {
                line_index,
                trace_line_index,
                point_index,
                reflection_source_index,
                reflection_axis_line_index,
                reflection_focus_index,
                reflection_directrix_line_index,
                ..
            } => {
                let Some(mapped_line_index) = mapped_index(group_to_line_index, *line_index) else {
                    line.binding = None;
                    continue;
                };
                let Some(mapped_trace_line_index) =
                    mapped_index(group_to_line_index, *trace_line_index)
                else {
                    line.binding = None;
                    continue;
                };
                let Some(mapped_point_index) = mapped_index(group_to_point_index, *point_index)
                else {
                    line.binding = None;
                    continue;
                };
                let mapped_reflection_source_index =
                    mapped_optional_index(group_to_point_index, *reflection_source_index)
                        .unwrap_or(None);
                let mapped_reflection_axis_line_index =
                    mapped_optional_index(group_to_line_index, *reflection_axis_line_index)
                        .unwrap_or(None);
                let mapped_reflection_focus_index =
                    mapped_optional_index(group_to_point_index, *reflection_focus_index)
                        .unwrap_or(None);
                let mapped_reflection_directrix_line_index =
                    mapped_optional_index(group_to_line_index, *reflection_directrix_line_index)
                        .unwrap_or(None);
                if (reflection_source_index.is_some() && mapped_reflection_source_index.is_none())
                    || (reflection_axis_line_index.is_some()
                        && mapped_reflection_axis_line_index.is_none())
                    || (reflection_focus_index.is_some() && mapped_reflection_focus_index.is_none())
                    || (reflection_directrix_line_index.is_some()
                        && mapped_reflection_directrix_line_index.is_none())
                {
                    line.binding = None;
                    continue;
                }
                *line_index = mapped_line_index;
                *trace_line_index = mapped_trace_line_index;
                *point_index = mapped_point_index;
                *reflection_source_index = mapped_reflection_source_index;
                *reflection_axis_line_index = mapped_reflection_axis_line_index;
                *reflection_focus_index = mapped_reflection_focus_index;
                *reflection_directrix_line_index = mapped_reflection_directrix_line_index;
            }
            LineBinding::ParametricCurve { .. } => {}
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
