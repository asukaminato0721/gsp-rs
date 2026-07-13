pub(crate) fn try_decode_point_constraint(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
    anchors: Option<&[Option<PointRecord>]>,
    graph: &Option<GraphTransform>,
) -> Result<RawPointConstraint, PointConstraintDecodeError> {
    if !group.header.kind().is_point_constraint() {
        return Err(PointConstraintDecodeError::NotPointConstraintKind(
            group.header.kind(),
        ));
    }

    let path =
        find_indexed_path(file, group).ok_or(PointConstraintDecodeError::MissingIndexedPath)?;
    let host_ref = path
        .refs
        .first()
        .copied()
        .filter(|ordinal| *ordinal > 0)
        .ok_or(PointConstraintDecodeError::MissingHostReference)?;
    let host_group = groups
        .get(host_ref - 1)
        .ok_or(PointConstraintDecodeError::MissingHostReference)?;
    let host_kind = host_group.header.kind();
    let payload = group
        .records
        .iter()
        .find(|record| record.record_type == RECORD_BINDING_PAYLOAD)
        .map(|record| record.payload(&file.data))
        .ok_or(PointConstraintDecodeError::MissingPayloadRecord)?;

    if payload.len() < 12 {
        return Err(PointConstraintDecodeError::PayloadTooShort {
            byte_len: payload.len(),
            expected: 12,
        });
    }
    if !read_f64(payload, 4).is_finite() {
        return Err(PointConstraintDecodeError::NonFiniteParameter);
    }

    let host_path = find_indexed_path(file, host_group)
        .ok_or(PointConstraintDecodeError::MissingIndexedPath)?;
    match host_kind {
        crate::format::GroupKind::Circle if host_path.refs.len() != 2 => {
            return Err(PointConstraintDecodeError::HostPathTooShort {
                host_kind,
                expected: 2,
                actual: host_path.refs.len(),
            });
        }
        crate::format::GroupKind::Polygon if host_path.refs.len() < 2 => {
            return Err(PointConstraintDecodeError::HostPathTooShort {
                host_kind,
                expected: 2,
                actual: host_path.refs.len(),
            });
        }
        crate::format::GroupKind::ThreePointArc
        | crate::format::GroupKind::ArcOnCircle
        | crate::format::GroupKind::CenterArc
            if host_path.refs.len() != 3 =>
        {
            return Err(PointConstraintDecodeError::HostPathTooShort {
                host_kind,
                expected: 3,
                actual: host_path.refs.len(),
            });
        }
        crate::format::GroupKind::FunctionPlot
        | crate::format::GroupKind::ParametricFunctionPlot => {
            return try_decode_point_on_function_constraint(
                file, groups, host_group, payload, graph,
            );
        }
        _ => {}
    }

    match (group.header.kind(), host_kind) {
        (GroupKind::PathPoint, _) => {
            return try_decode_path_point_constraint(
                file, groups, host_group, payload, anchors, graph,
            );
        }
        (_, kind)
            if kind.is_line_like()
                || matches!(
                    kind,
                    GroupKind::MeasurementLine
                        | GroupKind::PerpendicularLine
                        | GroupKind::ParallelLine
                        | GroupKind::AngleBisectorRay
                        | GroupKind::Rotation
                ) =>
        {
            return decode_point_on_line_like_constraint(file, groups, group).ok_or(
                PointConstraintDecodeError::UnsupportedOrMalformed {
                    host_kind,
                    payload_len: payload.len(),
                },
            );
        }
        (_, GroupKind::Circle | GroupKind::CircleCenterRadius) => {
            return try_decode_circle_point_constraint(file, host_group, payload);
        }
        (
            _,
            GroupKind::Reflection
            | GroupKind::Scale
            | GroupKind::CartesianOffsetPoint
            | GroupKind::PolarOffsetPoint
            | GroupKind::ParameterRotation,
        ) if anchors.is_some_and(|anchors| {
            resolve_circle_like_raw(file, groups, anchors, host_group).is_some()
        }) =>
        {
            return try_decode_circle_point_constraint(file, host_group, payload);
        }
        (_, GroupKind::Polygon) => {
            return try_decode_polygon_boundary_constraint(file, host_group, payload);
        }
        (_, GroupKind::Translation) => {
            if host_path.refs.len() >= 3
                && let Some(source_group) = groups.get(host_path.refs[0].saturating_sub(1))
                && source_group.header.kind() == GroupKind::Polygon
            {
                let RawPointConstraint::PolygonBoundary {
                    vertex_group_indices,
                    edge_index,
                    t,
                } = try_decode_polygon_boundary_constraint(file, source_group, payload)?
                else {
                    unreachable!();
                };
                return Ok(RawPointConstraint::TranslatedPolygonBoundary {
                    vertex_group_indices,
                    vector_start_group_index: host_path.refs[1].saturating_sub(1),
                    vector_end_group_index: host_path.refs[2].saturating_sub(1),
                    edge_index,
                    t,
                });
            }
        }
        (_, GroupKind::ThreePointArc | GroupKind::ArcOnCircle | GroupKind::CenterArc) => {
            return try_decode_arc_family_constraint(file, groups, host_group, payload);
        }
        _ => {}
    }

    decode_point_constraint_impl(file, groups, group, anchors, graph).ok_or(
        PointConstraintDecodeError::UnsupportedOrMalformed {
            host_kind,
            payload_len: payload.len(),
        },
    )
}

fn decode_point_constraint_impl(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
    anchors: Option<&[Option<PointRecord>]>,
    graph: &Option<GraphTransform>,
) -> Option<RawPointConstraint> {
    if !group.header.kind().is_point_constraint() {
        return None;
    }

    let host_ref = find_indexed_path(file, group)?
        .refs
        .first()
        .copied()
        .filter(|ordinal| *ordinal > 0)?;
    let host_group = groups.get(host_ref - 1)?;
    let host_kind = host_group.header.kind();
    let payload = group
        .records
        .iter()
        .find(|record| record.record_type == RECORD_BINDING_PAYLOAD)
        .map(|record| record.payload(&file.data))?;

    if (group.header.kind()) == crate::format::GroupKind::PathPoint {
        return decode_path_point_constraint(file, groups, host_group, payload, anchors, graph);
    }

    match (host_kind, payload.len()) {
        (
            crate::format::GroupKind::SectorBoundary
            | crate::format::GroupKind::CircularSegmentBoundary,
            12,
        ) => {
            let normalized_t = read_f64(payload, 4);
            if !normalized_t.is_finite() {
                return None;
            }
            let points = decode_arc_boundary_polyline(file, groups, host_group, anchors?)?;
            let (segment_index, t) = locate_polyline_parameter_by_length(&points, normalized_t)?;
            Some(RawPointConstraint::Polyline {
                function_key: host_group.ordinal,
                points,
                segment_index,
                t,
            })
        }
        (crate::format::GroupKind::Circle, 20) => {
            let host_path = find_indexed_path(file, host_group)?;
            if host_path.refs.len() != 2 {
                return None;
            }

            let unit_x = read_f64(payload, 4);
            let unit_y = read_f64(payload, 12);
            if !unit_x.is_finite() || !unit_y.is_finite() {
                return None;
            }

            Some(RawPointConstraint::Circle(PointOnCircleConstraint {
                center_group_index: host_path.refs[0].checked_sub(1)?,
                radius_group_index: host_path.refs[1].checked_sub(1)?,
                unit_x,
                unit_y,
            }))
        }
        (crate::format::GroupKind::Polygon, 20) => {
            let host_path = find_indexed_path(file, host_group)?;
            if host_path.refs.len() < 2 {
                return None;
            }

            let t = read_f64(payload, 4);
            if !t.is_finite() {
                return None;
            }

            Some(RawPointConstraint::PolygonBoundary {
                vertex_group_indices: host_path
                    .refs
                    .iter()
                    .map(|vertex| vertex.checked_sub(1))
                    .collect::<Option<Vec<_>>>()?,
                edge_index: decode_polygon_edge_index(host_path.refs.len(), payload)?,
                t,
            })
        }
        (
            crate::format::GroupKind::FunctionPlot
            | crate::format::GroupKind::ParametricFunctionPlot,
            12,
        ) => decode_point_on_function_constraint(file, groups, host_group, payload, graph),
        (crate::format::GroupKind::ThreePointArc, 12) => {
            let host_path = find_indexed_path(file, host_group)?;
            if host_path.refs.len() != 3 {
                return None;
            }
            let t = read_f64(payload, 4);
            if !t.is_finite() {
                return None;
            }
            Some(RawPointConstraint::Arc(PointOnArcConstraint {
                start_group_index: host_path.refs[0].checked_sub(1)?,
                mid_group_index: host_path.refs[1].checked_sub(1)?,
                end_group_index: host_path.refs[2].checked_sub(1)?,
                t,
            }))
        }
        (crate::format::GroupKind::ArcOnCircle, 12) => {
            let host_path = find_indexed_path(file, host_group)?;
            if host_path.refs.len() != 3 {
                return None;
            }
            let circle_group = groups.get(host_path.refs[0].checked_sub(1)?)?;
            if !is_circle_group_kind(circle_group.header.kind()) {
                return None;
            }
            let circle_path = find_indexed_path(file, circle_group)?;
            if circle_path.refs.len() != 2 {
                return None;
            }
            let t = read_f64(payload, 4);
            if !t.is_finite() {
                return None;
            }
            Some(RawPointConstraint::CircleArc(PointOnCircleArcConstraint {
                center_group_index: circle_path.refs[0].checked_sub(1)?,
                start_group_index: host_path.refs[1].checked_sub(1)?,
                end_group_index: host_path.refs[2].checked_sub(1)?,
                t,
            }))
        }
        (crate::format::GroupKind::CenterArc, 12) => {
            let host_path = find_indexed_path(file, host_group)?;
            if host_path.refs.len() != 3 {
                return None;
            }
            let t = read_f64(payload, 4);
            if !t.is_finite() {
                return None;
            }
            let reversed_t = 1.0 - t;
            Some(RawPointConstraint::CircleArc(PointOnCircleArcConstraint {
                center_group_index: host_path.refs[0].checked_sub(1)?,
                start_group_index: host_path.refs[1].checked_sub(1)?,
                end_group_index: host_path.refs[2].checked_sub(1)?,
                t: reversed_t,
            }))
        }
        _ => decode_point_on_line_like_constraint(file, groups, group),
    }
}

fn decode_path_point_constraint(
    file: &GspFile,
    groups: &[ObjectGroup],
    host_group: &ObjectGroup,
    payload: &[u8],
    anchors: Option<&[Option<PointRecord>]>,
    graph: &Option<GraphTransform>,
) -> Option<RawPointConstraint> {
    if payload.len() < 12 {
        return None;
    }

    let normalized_t = read_f64(payload, 4);
    if !normalized_t.is_finite() {
        return None;
    }

    match host_group.header.kind() {
        crate::format::GroupKind::Segment
        | crate::format::GroupKind::Line
        | crate::format::GroupKind::Ray
        | crate::format::GroupKind::MeasurementLine
        | crate::format::GroupKind::GraphMeasurementSegment => {
            let host_path = find_indexed_path(file, host_group)?;
            if host_path.refs.len() != 2 {
                return None;
            }
            let line_like_kind = match host_group.header.kind() {
                crate::format::GroupKind::Segment => LineLikeKind::Segment,
                crate::format::GroupKind::Line => LineLikeKind::Line,
                crate::format::GroupKind::Ray => LineLikeKind::Ray,
                crate::format::GroupKind::MeasurementLine => LineLikeKind::Segment,
                crate::format::GroupKind::GraphMeasurementSegment => LineLikeKind::Segment,
                _ => unreachable!(),
            };
            let t = match line_like_kind {
                LineLikeKind::Segment => wrap_unit_interval(normalized_t),
                LineLikeKind::Line => normalized_t,
                LineLikeKind::Ray => normalized_t.max(0.0),
            };
            let (start_group_index, end_group_index) =
                if host_group.header.kind() == crate::format::GroupKind::GraphMeasurementSegment {
                    let line_group = groups.get(host_path.refs[1].checked_sub(1)?)?;
                    let line_path = find_indexed_path(file, line_group)?;
                    if line_path.refs.len() != 2 {
                        return None;
                    }
                    let end_ordinal = if line_path.refs[0] == host_path.refs[0] {
                        line_path.refs[1]
                    } else {
                        line_path.refs[0]
                    };
                    (
                        host_path.refs[0].checked_sub(1)?,
                        end_ordinal.checked_sub(1)?,
                    )
                } else {
                    (
                        host_path.refs[0].checked_sub(1)?,
                        host_path.refs[1].checked_sub(1)?,
                    )
                };
            Some(RawPointConstraint::Segment(PointOnSegmentConstraint {
                start_group_index,
                end_group_index,
                t,
                line_like_kind,
            }))
        }
        crate::format::GroupKind::PerpendicularLine
        | crate::format::GroupKind::ParallelLine
        | crate::format::GroupKind::AngleBisectorRay => {
            let line_like_kind = match host_group.header.kind() {
                crate::format::GroupKind::AngleBisectorRay => LineLikeKind::Ray,
                _ => LineLikeKind::Line,
            };
            let t = match line_like_kind {
                LineLikeKind::Ray => normalized_t.max(0.0),
                _ => normalized_t,
            };
            Some(RawPointConstraint::ConstructedLine {
                host_group_index: host_group.ordinal.checked_sub(1)?,
                t,
                line_like_kind,
            })
        }
        crate::format::GroupKind::Rotation => {
            let line_like_kind = transformed_line_like_kind(file, groups, host_group)?;
            let t = match line_like_kind {
                LineLikeKind::Segment => wrap_unit_interval(normalized_t),
                LineLikeKind::Line => normalized_t,
                LineLikeKind::Ray => normalized_t.max(0.0),
            };
            Some(RawPointConstraint::ConstructedLine {
                host_group_index: host_group.ordinal.checked_sub(1)?,
                t,
                line_like_kind,
            })
        }
        crate::format::GroupKind::Circle | crate::format::GroupKind::CircleCenterRadius => {
            let host_path = find_indexed_path(file, host_group)?;
            if host_path.refs.len() != 2 {
                return None;
            }
            let angle = std::f64::consts::TAU * wrap_unit_interval(normalized_t);
            if host_group.header.kind() == crate::format::GroupKind::Circle {
                Some(RawPointConstraint::Circle(PointOnCircleConstraint {
                    center_group_index: host_path.refs[0].checked_sub(1)?,
                    radius_group_index: host_path.refs[1].checked_sub(1)?,
                    unit_x: angle.cos(),
                    unit_y: angle.sin(),
                }))
            } else {
                Some(RawPointConstraint::Circular(PointOnCircularConstraint {
                    circle_group_index: host_group.ordinal.checked_sub(1)?,
                    unit_x: angle.cos(),
                    unit_y: angle.sin(),
                }))
            }
        }
        crate::format::GroupKind::Reflection
        | crate::format::GroupKind::Scale
        | crate::format::GroupKind::CartesianOffsetPoint
        | crate::format::GroupKind::PolarOffsetPoint
        | crate::format::GroupKind::ParameterRotation => {
            let anchors = anchors?;
            resolve_circle_like_raw(file, groups, anchors, host_group)?;
            let angle = std::f64::consts::TAU * wrap_unit_interval(normalized_t);
            Some(RawPointConstraint::Circular(PointOnCircularConstraint {
                circle_group_index: host_group.ordinal.checked_sub(1)?,
                unit_x: angle.cos(),
                unit_y: angle.sin(),
            }))
        }
        crate::format::GroupKind::Polygon => {
            let host_path = find_indexed_path(file, host_group)?;
            if host_path.refs.len() < 2 {
                return None;
            }
            let anchors = anchors?;
            let vertex_group_indices = host_path
                .refs
                .iter()
                .map(|vertex| vertex.checked_sub(1))
                .collect::<Option<Vec<_>>>()?;
            let vertices = vertex_group_indices
                .iter()
                .map(|group_index| anchors.get(*group_index)?.clone())
                .collect::<Option<Vec<_>>>()?;
            let (edge_index, t) = polygon_parameter_to_edge(&vertices, normalized_t)?;
            Some(RawPointConstraint::PolygonBoundary {
                vertex_group_indices,
                edge_index,
                t,
            })
        }
        crate::format::GroupKind::SectorBoundary
        | crate::format::GroupKind::CircularSegmentBoundary => {
            let points = decode_arc_boundary_polyline(file, groups, host_group, anchors?)?;
            let (segment_index, t) = locate_polyline_parameter_by_length(&points, normalized_t)?;
            Some(RawPointConstraint::Polyline {
                function_key: host_group.ordinal,
                points,
                segment_index,
                t,
            })
        }
        crate::format::GroupKind::FunctionPlot
        | crate::format::GroupKind::ParametricFunctionPlot => {
            decode_point_on_function_constraint(file, groups, host_group, payload, graph)
        }
        crate::format::GroupKind::ThreePointArc => {
            let host_path = find_indexed_path(file, host_group)?;
            if host_path.refs.len() != 3 {
                return None;
            }
            Some(RawPointConstraint::Arc(PointOnArcConstraint {
                start_group_index: host_path.refs[0].checked_sub(1)?,
                mid_group_index: host_path.refs[1].checked_sub(1)?,
                end_group_index: host_path.refs[2].checked_sub(1)?,
                t: wrap_unit_interval(normalized_t),
            }))
        }
        crate::format::GroupKind::ArcOnCircle => {
            let host_path = find_indexed_path(file, host_group)?;
            if host_path.refs.len() != 3 {
                return None;
            }
            let circle_group = groups.get(host_path.refs[0].checked_sub(1)?)?;
            if !is_circle_group_kind(circle_group.header.kind()) {
                return None;
            }
            let circle_path = find_indexed_path(file, circle_group)?;
            if circle_path.refs.len() != 2 {
                return None;
            }
            Some(RawPointConstraint::CircleArc(PointOnCircleArcConstraint {
                center_group_index: circle_path.refs[0].checked_sub(1)?,
                start_group_index: host_path.refs[1].checked_sub(1)?,
                end_group_index: host_path.refs[2].checked_sub(1)?,
                t: wrap_unit_interval(normalized_t),
            }))
        }
        crate::format::GroupKind::CenterArc => {
            let host_path = find_indexed_path(file, host_group)?;
            if host_path.refs.len() != 3 {
                return None;
            }
            Some(RawPointConstraint::CircleArc(PointOnCircleArcConstraint {
                center_group_index: host_path.refs[0].checked_sub(1)?,
                start_group_index: host_path.refs[1].checked_sub(1)?,
                end_group_index: host_path.refs[2].checked_sub(1)?,
                t: 1.0 - wrap_unit_interval(normalized_t),
            }))
        }
        _ => None,
    }
}

fn try_decode_circle_point_constraint(
    file: &GspFile,
    host_group: &ObjectGroup,
    payload: &[u8],
) -> Result<RawPointConstraint, PointConstraintDecodeError> {
    if payload.len() < 20 {
        return Err(PointConstraintDecodeError::PayloadTooShort {
            byte_len: payload.len(),
            expected: 20,
        });
    }
    let unit_x = read_f64(payload, 4);
    let unit_y = read_f64(payload, 12);
    if !unit_x.is_finite() || !unit_y.is_finite() {
        return Err(PointConstraintDecodeError::NonFiniteCircleUnit);
    }
    if host_group.header.kind() == crate::format::GroupKind::Circle {
        let host_path = find_indexed_path(file, host_group)
            .filter(|path| path.refs.len() == 2)
            .ok_or(PointConstraintDecodeError::InvalidCircleHostPath)?;
        return Ok(RawPointConstraint::Circle(PointOnCircleConstraint {
            center_group_index: host_path.refs[0]
                .checked_sub(1)
                .ok_or(PointConstraintDecodeError::InvalidCircleHostPath)?,
            radius_group_index: host_path.refs[1]
                .checked_sub(1)
                .ok_or(PointConstraintDecodeError::InvalidCircleHostPath)?,
            unit_x,
            unit_y,
        }));
    }
    Ok(RawPointConstraint::Circular(PointOnCircularConstraint {
        circle_group_index: host_group
            .ordinal
            .checked_sub(1)
            .ok_or(PointConstraintDecodeError::InvalidCircleHostPath)?,
        unit_x,
        unit_y,
    }))
}

fn try_decode_polygon_boundary_constraint(
    file: &GspFile,
    host_group: &ObjectGroup,
    payload: &[u8],
) -> Result<RawPointConstraint, PointConstraintDecodeError> {
    let host_path = find_indexed_path(file, host_group)
        .filter(|path| path.refs.len() >= 2)
        .ok_or(PointConstraintDecodeError::InvalidPolygonHostPath)?;
    let t = read_f64(payload, 4);
    if !t.is_finite() {
        return Err(PointConstraintDecodeError::NonFiniteParameter);
    }
    let vertex_group_indices = host_path
        .refs
        .iter()
        .map(|vertex| vertex.checked_sub(1))
        .collect::<Option<Vec<_>>>()
        .ok_or(PointConstraintDecodeError::InvalidPolygonHostPath)?;
    let edge_index = decode_polygon_edge_index(host_path.refs.len(), payload)
        .ok_or(PointConstraintDecodeError::InvalidPolygonEdgeIndex)?;
    Ok(RawPointConstraint::PolygonBoundary {
        vertex_group_indices,
        edge_index,
        t,
    })
}

fn try_decode_arc_family_constraint(
    file: &GspFile,
    groups: &[ObjectGroup],
    host_group: &ObjectGroup,
    payload: &[u8],
) -> Result<RawPointConstraint, PointConstraintDecodeError> {
    let host_kind = host_group.header.kind();
    let host_path = find_indexed_path(file, host_group)
        .filter(|path| path.refs.len() == 3)
        .ok_or(PointConstraintDecodeError::InvalidArcHostPath(host_kind))?;
    let t = read_f64(payload, 4);
    if !t.is_finite() {
        return Err(PointConstraintDecodeError::NonFiniteParameter);
    }
    match host_kind {
        crate::format::GroupKind::ThreePointArc => {
            Ok(RawPointConstraint::Arc(PointOnArcConstraint {
                start_group_index: host_path.refs[0]
                    .checked_sub(1)
                    .ok_or(PointConstraintDecodeError::InvalidArcHostPath(host_kind))?,
                mid_group_index: host_path.refs[1]
                    .checked_sub(1)
                    .ok_or(PointConstraintDecodeError::InvalidArcHostPath(host_kind))?,
                end_group_index: host_path.refs[2]
                    .checked_sub(1)
                    .ok_or(PointConstraintDecodeError::InvalidArcHostPath(host_kind))?,
                t,
            }))
        }
        crate::format::GroupKind::ArcOnCircle => {
            let circle_group = groups
                .get(
                    host_path.refs[0]
                        .checked_sub(1)
                        .ok_or(PointConstraintDecodeError::ArcHostMissingCircle)?,
                )
                .ok_or(PointConstraintDecodeError::ArcHostMissingCircle)?;
            if !is_circle_group_kind(circle_group.header.kind()) {
                return Err(PointConstraintDecodeError::ArcHostMissingCircle);
            }
            let circle_path = find_indexed_path(file, circle_group)
                .filter(|path| path.refs.len() == 2)
                .ok_or(PointConstraintDecodeError::InvalidArcCirclePath)?;
            Ok(RawPointConstraint::CircleArc(PointOnCircleArcConstraint {
                center_group_index: circle_path.refs[0]
                    .checked_sub(1)
                    .ok_or(PointConstraintDecodeError::InvalidArcCirclePath)?,
                start_group_index: host_path.refs[1]
                    .checked_sub(1)
                    .ok_or(PointConstraintDecodeError::InvalidArcHostPath(host_kind))?,
                end_group_index: host_path.refs[2]
                    .checked_sub(1)
                    .ok_or(PointConstraintDecodeError::InvalidArcHostPath(host_kind))?,
                t,
            }))
        }
        crate::format::GroupKind::CenterArc => {
            Ok(RawPointConstraint::CircleArc(PointOnCircleArcConstraint {
                center_group_index: host_path.refs[0]
                    .checked_sub(1)
                    .ok_or(PointConstraintDecodeError::InvalidArcHostPath(host_kind))?,
                start_group_index: host_path.refs[1]
                    .checked_sub(1)
                    .ok_or(PointConstraintDecodeError::InvalidArcHostPath(host_kind))?,
                end_group_index: host_path.refs[2]
                    .checked_sub(1)
                    .ok_or(PointConstraintDecodeError::InvalidArcHostPath(host_kind))?,
                t: 1.0 - t,
            }))
        }
        _ => Err(PointConstraintDecodeError::UnsupportedOrMalformed {
            host_kind,
            payload_len: payload.len(),
        }),
    }
}

fn try_decode_path_point_constraint(
    file: &GspFile,
    groups: &[ObjectGroup],
    host_group: &ObjectGroup,
    payload: &[u8],
    anchors: Option<&[Option<PointRecord>]>,
    graph: &Option<GraphTransform>,
) -> Result<RawPointConstraint, PointConstraintDecodeError> {
    if payload.len() < 12 {
        return Err(PointConstraintDecodeError::PayloadTooShort {
            byte_len: payload.len(),
            expected: 12,
        });
    }
    let normalized_t = read_f64(payload, 4);
    if !normalized_t.is_finite() {
        return Err(PointConstraintDecodeError::NonFiniteParameter);
    }
    if matches!(
        host_group.header.kind(),
        crate::format::GroupKind::SectorBoundary
            | crate::format::GroupKind::CircularSegmentBoundary
            | crate::format::GroupKind::Polygon
    ) && anchors.is_none()
    {
        return Err(PointConstraintDecodeError::MissingAnchors);
    }
    decode_path_point_constraint(file, groups, host_group, payload, anchors, graph).ok_or(
        PointConstraintDecodeError::UnsupportedOrMalformed {
            host_kind: host_group.header.kind(),
            payload_len: payload.len(),
        },
    )
}

fn decode_arc_boundary_polyline(
    file: &GspFile,
    groups: &[ObjectGroup],
    host_group: &ObjectGroup,
    anchors: &[Option<PointRecord>],
) -> Option<Vec<PointRecord>> {
    let (center, [start, mid, end], starts_from_end, complement) =
        resolve_boundary_arc_geometry(file, groups, host_group, anchors)?;
    let arc_points = if complement {
        sample_three_point_arc_complement(&start, &mid, &end, ARC_BOUNDARY_SUBDIVISIONS)?
    } else {
        sample_three_point_arc(&start, &mid, &end, ARC_BOUNDARY_SUBDIVISIONS)?
    };
    match host_group.header.kind() {
        crate::format::GroupKind::SectorBoundary => {
            let center = center?;
            let mut points = if starts_from_end {
                vec![end.clone(), center.clone(), start.clone()]
            } else {
                vec![center.clone(), start.clone()]
            };
            points.extend(arc_points.into_iter().skip(1));
            if !starts_from_end {
                points.push(center);
            }
            Some(points)
        }
        crate::format::GroupKind::CircularSegmentBoundary => {
            let mut points = if starts_from_end {
                vec![end.clone(), start.clone()]
            } else {
                vec![start.clone()]
            };
            points.extend(arc_points.into_iter().skip(1));
            if !starts_from_end {
                points.push(start);
            }
            Some(points)
        }
        _ => None,
    }
}

fn resolve_boundary_arc_geometry(
    file: &GspFile,
    groups: &[ObjectGroup],
    host_group: &ObjectGroup,
    anchors: &[Option<PointRecord>],
) -> Option<(Option<PointRecord>, [PointRecord; 3], bool, bool)> {
    let path = find_indexed_path(file, host_group)?;
    let arc_group = groups.get(path.refs.first()?.checked_sub(1)?)?;
    match arc_group.header.kind() {
        crate::format::GroupKind::CenterArc => {
            let arc_path = find_indexed_path(file, arc_group)?;
            if arc_path.refs.len() != 3 {
                return None;
            }
            let center = anchors.get(arc_path.refs[0].checked_sub(1)?)?.clone()?;
            let start = anchors.get(arc_path.refs[1].checked_sub(1)?)?.clone()?;
            let end = anchors.get(arc_path.refs[2].checked_sub(1)?)?.clone()?;
            Some((
                Some(center.clone()),
                arc_on_circle_control_points(&center, &start, &end)?,
                true,
                false,
            ))
        }
        crate::format::GroupKind::ArcOnCircle => {
            let arc_path = find_indexed_path(file, arc_group)?;
            if arc_path.refs.len() != 3 {
                return None;
            }
            let circle_group = groups.get(arc_path.refs[0].checked_sub(1)?)?;
            let circle_path = find_indexed_path(file, circle_group)?;
            if circle_path.refs.len() != 2 {
                return None;
            }
            let center = anchors.get(circle_path.refs[0].checked_sub(1)?)?.clone()?;
            let start = anchors.get(arc_path.refs[1].checked_sub(1)?)?.clone()?;
            let end = anchors.get(arc_path.refs[2].checked_sub(1)?)?.clone()?;
            Some((
                Some(center.clone()),
                arc_on_circle_control_points(&center, &start, &end)?,
                false,
                false,
            ))
        }
        crate::format::GroupKind::ThreePointArc => {
            let arc_path = find_indexed_path(file, arc_group)?;
            if arc_path.refs.len() != 3 {
                return None;
            }
            let start = anchors.get(arc_path.refs[0].checked_sub(1)?)?.clone()?;
            let mid = anchors.get(arc_path.refs[1].checked_sub(1)?)?.clone()?;
            let end = anchors.get(arc_path.refs[2].checked_sub(1)?)?.clone()?;
            let center =
                three_point_arc_geometry(&start, &mid, &end).map(|geometry| geometry.center);
            Some((
                center,
                [start, mid, end],
                false,
                (host_group.header.kind()) == crate::format::GroupKind::CircularSegmentBoundary,
            ))
        }
        _ => None,
    }
}

fn decode_point_on_function_constraint(
    file: &GspFile,
    groups: &[ObjectGroup],
    host_group: &ObjectGroup,
    payload: &[u8],
    graph: &Option<GraphTransform>,
) -> Option<RawPointConstraint> {
    let transform = graph.as_ref()?;
    let normalized_t = read_f64(payload, 4);
    if !normalized_t.is_finite() {
        return None;
    }

    let points = crate::runtime::functions::sample_plot_segments(file, groups, host_group)?
        .into_iter()
        .flatten()
        .map(|point| to_raw_from_world(&point, transform))
        .collect::<Vec<_>>();
    let (segment_index, t) = locate_polyline_parameter(&points, normalized_t)?;
    Some(RawPointConstraint::Polyline {
        function_key: host_group.ordinal,
        points,
        segment_index,
        t,
    })
}

fn try_decode_point_on_function_constraint(
    file: &GspFile,
    groups: &[ObjectGroup],
    host_group: &ObjectGroup,
    payload: &[u8],
    graph: &Option<GraphTransform>,
) -> Result<RawPointConstraint, PointConstraintDecodeError> {
    if payload.len() < 12 {
        return Err(PointConstraintDecodeError::PayloadTooShort {
            byte_len: payload.len(),
            expected: 12,
        });
    }
    let normalized_t = read_f64(payload, 4);
    if !normalized_t.is_finite() {
        return Err(PointConstraintDecodeError::NonFiniteParameter);
    }
    if graph.is_none() {
        return Err(PointConstraintDecodeError::MissingGraphTransform);
    }
    let descriptor_record = host_group
        .records
        .iter()
        .find(|record| record.record_type == RECORD_FUNCTION_PLOT_DESCRIPTOR)
        .ok_or(PointConstraintDecodeError::MissingFunctionPlotDescriptor)?;
    try_decode_function_plot_descriptor(descriptor_record.payload(&file.data)).map_err(
        |error| PointConstraintDecodeError::InvalidFunctionPlotDescriptor(error.to_string()),
    )?;
    crate::runtime::functions::sample_plot_segments(file, groups, host_group).ok_or(
        PointConstraintDecodeError::InvalidFunctionExpr(
            "unsupported parametric/function plot sources".to_string(),
        ),
    )?;
    decode_point_on_function_constraint(file, groups, host_group, payload, graph)
        .ok_or(PointConstraintDecodeError::PolylineParameterUnavailable)
}

fn locate_polyline_parameter(points: &[PointRecord], normalized_t: f64) -> Option<(usize, f64)> {
    if points.len() < 2 {
        return None;
    }

    let wrapped_t = wrap_unit_interval(normalized_t);
    let scaled = wrapped_t * (points.len() - 1) as f64;
    let segment_index = scaled.floor() as usize;
    Some((segment_index.min(points.len() - 2), scaled.fract()))
}

fn decode_polygon_edge_index(vertex_count: usize, payload: &[u8]) -> Option<usize> {
    if vertex_count < 2 || payload.len() < 16 {
        return None;
    }

    let discrete = read_u32(payload, 12) as usize;
    if discrete < vertex_count {
        return Some(discrete);
    }

    let selector = read_f64(payload, 12);
    if !selector.is_finite() {
        return None;
    }
    let end_vertex = ((selector * vertex_count as f64) - 0.25).round() as isize;
    Some(((end_vertex + vertex_count as isize - 1).rem_euclid(vertex_count as isize)) as usize)
}
