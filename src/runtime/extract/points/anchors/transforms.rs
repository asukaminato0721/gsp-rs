pub(crate) fn resolve_custom_transform_point(
    anchors: &[Option<PointRecord>],
    binding: &CustomTransformBindingDef,
    t: f64,
) -> Option<PointRecord> {
    let origin = anchors.get(binding.origin_group_index)?.clone()?;
    let axis_end = anchors.get(binding.axis_end_group_index)?.clone()?;
    let parameters = expression_parameter_map(&binding.distance_expr, &binding.angle_expr, t);
    let distance = evaluate_expr_with_parameters(&binding.distance_expr, t, &parameters)?
        * binding.distance_raw_scale;
    let angle_degrees = evaluate_expr_with_parameters(&binding.angle_expr, t, &parameters)?
        * binding.angle_degrees_scale;
    let base_angle = (-(axis_end.y - origin.y))
        .atan2(axis_end.x - origin.x)
        .to_degrees();
    let total_radians = (base_angle + angle_degrees).to_radians();
    Some(PointRecord {
        x: origin.x + distance * total_radians.cos(),
        y: origin.y - distance * total_radians.sin(),
    })
}

pub(crate) fn decode_custom_transform_parameter(
    file: &GspFile,
    groups: &[ObjectGroup],
    source_group_index: usize,
    anchors: &[Option<PointRecord>],
) -> Option<f64> {
    let source_group = groups.get(source_group_index)?;
    match source_group.header.kind() {
        kind if kind.is_point_constraint() => {
            match try_decode_point_constraint(file, groups, source_group, Some(anchors), &None)
                .ok()?
            {
                RawPointConstraint::Segment(constraint) => Some(constraint.t),
                RawPointConstraint::ConstructedLine { t, .. } => Some(t),
                RawPointConstraint::Polyline { t, .. } => Some(t),
                RawPointConstraint::PolygonBoundary { t, .. } => Some(t),
                RawPointConstraint::TranslatedPolygonBoundary { t, .. } => Some(t),
                RawPointConstraint::Circle(constraint) => {
                    let angle = (-constraint.unit_y).atan2(constraint.unit_x);
                    let tau = std::f64::consts::TAU;
                    Some(((angle % tau) + tau) % tau / tau)
                }
                RawPointConstraint::Circular(constraint) => {
                    let angle = (-constraint.unit_y).atan2(constraint.unit_x);
                    let tau = std::f64::consts::TAU;
                    let _ = constraint.circle_group_index;
                    Some(((angle % tau) + tau) % tau / tau)
                }
                RawPointConstraint::CircleArc(constraint) => Some(constraint.t),
                RawPointConstraint::Arc(constraint) => Some(constraint.t),
            }
        }
        crate::format::GroupKind::ParameterControlledPoint => {
            let parameter_point =
                try_decode_parameter_controlled_point(file, groups, source_group, anchors).ok()?;
            match parameter_point.constraint {
                RawPointConstraint::Segment(constraint) => Some(constraint.t),
                RawPointConstraint::ConstructedLine { t, .. } => Some(t),
                RawPointConstraint::Polyline { t, .. } => Some(t),
                RawPointConstraint::PolygonBoundary { t, .. } => Some(t),
                RawPointConstraint::TranslatedPolygonBoundary { t, .. } => Some(t),
                RawPointConstraint::Circle(constraint) => {
                    let angle = (-constraint.unit_y).atan2(constraint.unit_x);
                    let tau = std::f64::consts::TAU;
                    Some(((angle % tau) + tau) % tau / tau)
                }
                RawPointConstraint::Circular(constraint) => {
                    let angle = (-constraint.unit_y).atan2(constraint.unit_x);
                    let tau = std::f64::consts::TAU;
                    let _ = constraint.circle_group_index;
                    Some(((angle % tau) + tau) % tau / tau)
                }
                RawPointConstraint::CircleArc(constraint) => Some(constraint.t),
                RawPointConstraint::Arc(constraint) => Some(constraint.t),
            }
        }
        _ => None,
    }
}

fn decode_custom_transform_distance_scale(file: &GspFile, expr_group: &ObjectGroup) -> Option<f64> {
    Some(match custom_transform_suffix(file, expr_group)? {
        0x0201 => PX_PER_CM,
        _ => 1.0,
    })
}

fn decode_custom_transform_angle_scale(file: &GspFile, expr_group: &ObjectGroup) -> Option<f64> {
    Some(match custom_transform_suffix(file, expr_group)? {
        0x0101 => 100.0,
        _ => 1.0,
    })
}

fn custom_transform_suffix(file: &GspFile, expr_group: &ObjectGroup) -> Option<u16> {
    let payload = expr_group
        .records
        .iter()
        .find(|record| record.record_type == 0x0907)?
        .payload(&file.data);
    let words = payload
        .chunks_exact(2)
        .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
        .collect::<Vec<_>>();
    words.last().copied().or_else(|| {
        (words.len() >= 3 && words[words.len() - 3..] == [0x0000, 0x0000, 0x0101]).then_some(0x0101)
    })
}

fn expression_parameter_map(
    left: &crate::runtime::functions::FunctionExpr,
    right: &crate::runtime::functions::FunctionExpr,
    t: f64,
) -> BTreeMap<String, f64> {
    let mut parameters = BTreeMap::new();
    collect_expr_parameter_names(left, &mut parameters, t);
    collect_expr_parameter_names(right, &mut parameters, t);
    parameters
}

pub(crate) fn custom_transform_expression_parameter_map(
    left: &crate::runtime::functions::FunctionExpr,
    right: &crate::runtime::functions::FunctionExpr,
    t: f64,
) -> BTreeMap<String, f64> {
    expression_parameter_map(left, right, t)
}

pub(crate) fn custom_transform_trace_parameter(
    point: &crate::runtime::scene::ScenePoint,
) -> Option<f64> {
    match &point.constraint {
        crate::runtime::scene::ScenePointConstraint::OnSegment { t, .. }
        | crate::runtime::scene::ScenePointConstraint::OnLine { t, .. }
        | crate::runtime::scene::ScenePointConstraint::OnRay { t, .. }
        | crate::runtime::scene::ScenePointConstraint::OnCircleArc { t, .. }
        | crate::runtime::scene::ScenePointConstraint::OnArc { t, .. } => Some(*t),
        crate::runtime::scene::ScenePointConstraint::OnPolygonBoundary { t, .. }
        | crate::runtime::scene::ScenePointConstraint::OnTranslatedPolygonBoundary { t, .. } => {
            Some(*t)
        }
        crate::runtime::scene::ScenePointConstraint::OnCircle { unit_x, unit_y, .. } => {
            let angle = (-*unit_y).atan2(*unit_x);
            let tau = std::f64::consts::TAU;
            Some(((angle % tau) + tau) % tau / tau)
        }
        _ => None,
    }
}

fn collect_expr_parameter_names(
    expr: &crate::runtime::functions::FunctionExpr,
    parameters: &mut BTreeMap<String, f64>,
    value: f64,
) {
    if let crate::runtime::functions::FunctionExpr::Parsed(ast) = expr {
        collect_term_parameter_names(ast, parameters, value);
    }
}

fn collect_term_parameter_names(
    term: &crate::runtime::functions::FunctionAst,
    parameters: &mut BTreeMap<String, f64>,
    value: f64,
) {
    match term {
        crate::runtime::functions::FunctionAst::Parameter(name, _) => {
            parameters.insert(name.clone(), value);
        }
        crate::runtime::functions::FunctionAst::Unary { expr, .. } => {
            collect_term_parameter_names(expr, parameters, value);
        }
        crate::runtime::functions::FunctionAst::Binary {
            lhs: left,
            rhs: right,
            ..
        } => {
            collect_term_parameter_names(left, parameters, value);
            collect_term_parameter_names(right, parameters, value);
        }
        _ => {}
    }
}

pub(crate) fn decode_parameter_rotation_anchor_raw(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
    anchors: &[Option<PointRecord>],
) -> Option<PointRecord> {
    let binding = if let Ok(binding) = try_decode_parameter_rotation_binding(file, groups, group) {
        binding
    } else if let Some(binding) =
        decode_measured_angle_parameter_rotation_binding_raw(file, groups, group, anchors)
    {
        binding
    } else {
        let path = find_indexed_path(file, group)?;
        let source_group_index = path.refs.first()?.checked_sub(1)?;
        let center_group_index = path.refs.get(1)?.checked_sub(1)?;
        let angle_group = groups.get(path.refs.get(2)?.checked_sub(1)?)?;
        let (angle_degrees, parameter_name) = match angle_group.header.kind() {
            GroupKind::FunctionExpr => {
                let (angle_expr, parameters, parameter_name) =
                    expression_runtime_context(file, groups, angle_group, anchors)?;
                let angle_expr = scale_angle_expr_to_degrees(file, angle_group, angle_expr);
                (
                    evaluate_expr_with_parameters(&angle_expr, 0.0, &parameters)?,
                    parameter_name,
                )
            }
            GroupKind::ParameterAnchor => {
                let (_, angle_radians) =
                    parameter_anchor_runtime_value(file, groups, angle_group, anchors)?;
                (angle_radians.to_degrees(), None)
            }
            _ => return None,
        };
        super::bindings::TransformBinding {
            source_group_index,
            center_group_index,
            kind: TransformBindingKind::Rotate {
                angle_degrees,
                parameter_name,
            },
        }
    };
    let source = anchors.get(binding.source_group_index)?.clone()?;
    let center = anchors.get(binding.center_group_index)?.clone()?;
    match binding.kind {
        TransformBindingKind::Rotate { angle_degrees, .. } => {
            Some(rotate_around(&source, &center, angle_degrees.to_radians()))
        }
        TransformBindingKind::Scale { factor } => Some(scale_around(&source, &center, factor)),
    }
}

pub(crate) fn decode_derived_polar_endpoint_binding(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
) -> Option<DerivedPolarEndpointBindingDef> {
    if !matches!(
        group.header.kind(),
        GroupKind::DerivedSegment24 | GroupKind::DerivedSegment75
    ) {
        return None;
    }
    let path = find_indexed_path(file, group)?;
    if path.refs.len() != 2 {
        return None;
    }
    let center_group_index = path.refs[0].checked_sub(1)?;
    let radius_group_index = path.refs[1].checked_sub(1)?;
    let radius_group = groups.get(radius_group_index)?;
    let parameter_name = decode_label_name(file, radius_group)?;
    let parameter_value = try_decode_parameter_control_value_for_group(file, groups, radius_group)
        .ok()?
        .abs();
    let payload = group
        .records
        .iter()
        .find(|record| record.record_type == 0x07d3)
        .map(|record| record.payload(&file.data))?;
    if payload.len() < 28 || read_u32(payload, 0) != u32::from(group.header.kind().raw()) {
        return None;
    }
    let radius_scale = read_f64(payload, 4);
    let angle_radians = read_f64(payload, 20);
    if !radius_scale.is_finite() || !angle_radians.is_finite() {
        return None;
    }
    Some(DerivedPolarEndpointBindingDef {
        center_group_index,
        parameter_name,
        parameter_value: parameter_value * radius_scale.abs(),
        radius_scale: radius_scale.abs(),
        angle_radians,
    })
}

pub(crate) fn decode_derived_polar_endpoint_anchor_raw(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
    anchors: &[Option<PointRecord>],
    graph: Option<&GraphTransform>,
) -> Option<PointRecord> {
    let binding = decode_derived_polar_endpoint_binding(file, groups, group)?;
    let center = anchors.get(binding.center_group_index)?.clone()?;
    let center_world = to_world(&center, &graph.cloned());
    let world = PointRecord {
        x: center_world.x + binding.parameter_value * binding.angle_radians.cos(),
        y: center_world.y + binding.parameter_value * binding.angle_radians.sin(),
    };
    Some(if let Some(transform) = graph {
        to_raw_from_world(&world, transform)
    } else {
        world
    })
}

fn decode_measured_angle_parameter_rotation_binding_raw(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
    anchors: &[Option<PointRecord>],
) -> Option<super::bindings::TransformBinding> {
    if group.header.kind() != GroupKind::ParameterRotation {
        return None;
    }
    let path = find_indexed_path(file, group)?;
    let source_group_index = path.refs.first()?.checked_sub(1)?;
    let center_group_index = path.refs.get(1)?.checked_sub(1)?;
    let angle_group = groups.get(path.refs.get(2)?.checked_sub(1)?)?;
    if angle_group.header.kind() != GroupKind::AngleValue {
        return None;
    }
    let angle_path = find_indexed_path(file, angle_group)?;
    let angle_start = anchors
        .get(angle_path.refs.first()?.checked_sub(1)?)?
        .clone()?;
    let angle_vertex = anchors
        .get(angle_path.refs.get(1)?.checked_sub(1)?)?
        .clone()?;
    let angle_end = anchors
        .get(angle_path.refs.get(2)?.checked_sub(1)?)?
        .clone()?;
    let angle_degrees = angle_degrees_from_points(&angle_start, &angle_vertex, &angle_end)?;
    Some(super::bindings::TransformBinding {
        source_group_index,
        center_group_index,
        kind: TransformBindingKind::Rotate {
            angle_degrees,
            parameter_name: None,
        },
    })
}

pub(crate) fn decode_angle_rotation_anchor_raw(
    file: &GspFile,
    group: &ObjectGroup,
    anchors: &[Option<PointRecord>],
) -> Option<PointRecord> {
    let binding = try_decode_angle_rotation_binding(file, group).ok()?;
    let source = anchors.get(binding.source_group_index)?.clone()?;
    let center = anchors.get(binding.center_group_index)?.clone()?;
    let angle_start = anchors.get(binding.angle_start_group_index)?.clone()?;
    let angle_vertex = anchors.get(binding.angle_vertex_group_index)?.clone()?;
    let angle_end = anchors.get(binding.angle_end_group_index)?.clone()?;
    let angle_degrees = angle_degrees_from_points(&angle_start, &angle_vertex, &angle_end)?;
    Some(rotate_around(&source, &center, angle_degrees.to_radians()))
}

pub(crate) fn decode_legacy_angle_rotation_anchor_raw(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
    anchors: &[Option<PointRecord>],
) -> Option<PointRecord> {
    if group.header.kind() != GroupKind::LegacyAngleRotation {
        return None;
    }
    let path = find_indexed_path(file, group)?;
    if path.refs.len() < 3 {
        return None;
    }
    let source = anchors.get(path.refs[0].checked_sub(1)?)?.clone()?;
    let center = anchors.get(path.refs[1].checked_sub(1)?)?.clone()?;
    let angle_group = groups.get(path.refs[2].checked_sub(1)?)?;
    let angle_path = find_indexed_path(file, angle_group)?;
    if angle_path.refs.len() < 3 {
        return None;
    }
    let angle_start = anchors.get(angle_path.refs[0].checked_sub(1)?)?.clone()?;
    let angle_vertex = anchors.get(angle_path.refs[1].checked_sub(1)?)?.clone()?;
    let angle_end = anchors.get(angle_path.refs[2].checked_sub(1)?)?.clone()?;
    let angle_degrees = angle_degrees_from_points(&angle_start, &angle_vertex, &angle_end)?;
    Some(rotate_around(&source, &center, angle_degrees.to_radians()))
}

pub(crate) fn decode_ratio_scale_anchor_raw(
    file: &GspFile,
    _groups: &[ObjectGroup],
    group: &ObjectGroup,
    anchors: &[Option<PointRecord>],
) -> Option<PointRecord> {
    if (group.header.kind()) != crate::format::GroupKind::RatioScale {
        return None;
    }
    let path = find_indexed_path(file, group)?;
    if path.refs.len() < 5 {
        return None;
    }
    let source = anchors.get(path.refs[0].checked_sub(1)?)?.clone()?;
    let center = anchors.get(path.refs[1].checked_sub(1)?)?.clone()?;
    let ratio_origin = anchors.get(path.refs[2].checked_sub(1)?)?.clone()?;
    let ratio_denominator = anchors.get(path.refs[3].checked_sub(1)?)?.clone()?;
    let ratio_numerator = anchors.get(path.refs[4].checked_sub(1)?)?.clone()?;
    let denominator_dx = ratio_denominator.x - ratio_origin.x;
    let denominator_dy = ratio_denominator.y - ratio_origin.y;
    let numerator_dx = ratio_numerator.x - ratio_origin.x;
    let numerator_dy = ratio_numerator.y - ratio_origin.y;
    let denominator = denominator_dx.hypot(denominator_dy);
    if denominator <= 1e-9 {
        return None;
    }
    let numerator = numerator_dx.hypot(numerator_dy);
    let direction = if denominator_dx * numerator_dx + denominator_dy * numerator_dy < 0.0 {
        -1.0
    } else {
        1.0
    };
    let factor = direction * numerator / denominator;
    Some(crate::runtime::geometry::scale_around(
        &source, &center, factor,
    ))
}

pub(crate) fn decode_reflection_anchor_raw(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
    anchors: &[Option<PointRecord>],
) -> Option<PointRecord> {
    if (group.header.kind()) != crate::format::GroupKind::Reflection {
        return None;
    }
    let path = find_indexed_path(file, group)?;
    let source_group_index = path.refs.first()?.checked_sub(1)?;
    let source = anchors.get(source_group_index)?.clone()?;
    let line_group = groups.get(path.refs.get(1)?.checked_sub(1)?)?;
    let (line_start, line_end) = resolve_line_like_points_raw(file, groups, anchors, line_group)?;
    reflect_point_across_line(&source, &line_start, &line_end)
}

pub(crate) fn decode_point_pair_translation_anchor_raw(
    file: &GspFile,
    _groups: &[ObjectGroup],
    group: &ObjectGroup,
    anchors: &[Option<PointRecord>],
) -> Option<PointRecord> {
    if (group.header.kind()) != crate::format::GroupKind::Translation {
        return None;
    }
    let path = find_indexed_path(file, group)?;
    let source_group_index = path.refs.first()?.checked_sub(1)?;
    let (vector_start_group_index, vector_end_group_index) =
        translation_point_pair_group_indices(file, group)?;
    let source = anchors.get(source_group_index)?.clone()?;
    let vector_start = anchors.get(vector_start_group_index)?.clone()?;
    let vector_end = anchors.get(vector_end_group_index)?.clone()?;
    Some(PointRecord {
        x: source.x + (vector_end.x - vector_start.x),
        y: source.y + (vector_end.y - vector_start.y),
    })
}

pub(crate) fn decode_parameter_controlled_anchor_raw(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
    anchors: &[Option<PointRecord>],
) -> Option<PointRecord> {
    try_decode_parameter_controlled_point(file, groups, group, anchors)
        .ok()
        .map(|point| point.position)
}

pub(crate) fn reflection_line_group_indices(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
) -> Option<(usize, usize)> {
    let path = find_indexed_path(file, group)?;
    let line_group = groups.get(path.refs.get(1)?.checked_sub(1)?)?;
    if !line_group.header.kind().is_line_like() {
        return None;
    }
    let line_path = find_indexed_path(file, line_group)?;
    Some((
        line_path.refs.first()?.checked_sub(1)?,
        line_path.refs.get(1)?.checked_sub(1)?,
    ))
}

pub(crate) fn translation_point_pair_group_indices(
    file: &GspFile,
    group: &ObjectGroup,
) -> Option<(usize, usize)> {
    if (group.header.kind()) != crate::format::GroupKind::Translation {
        return None;
    }
    let path = find_indexed_path(file, group)?;
    Some((
        path.refs.get(1)?.checked_sub(1)?,
        path.refs.get(2)?.checked_sub(1)?,
    ))
}

pub(crate) fn reflect_point_across_line(
    point: &PointRecord,
    line_start: &PointRecord,
    line_end: &PointRecord,
) -> Option<PointRecord> {
    reflect_across_line(point, line_start, line_end)
}

pub(crate) fn decode_point_on_ray_anchor_raw(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
    anchors: &[Option<PointRecord>],
) -> Option<PointRecord> {
    if !group.header.kind().is_point_constraint() {
        return None;
    }

    let host_ref = find_indexed_path(file, group)?
        .refs
        .first()
        .copied()
        .filter(|ordinal| *ordinal > 0)?;
    let host_group = groups.get(host_ref - 1)?;
    if (host_group.header.kind()) != crate::format::GroupKind::Ray {
        return None;
    }

    let host_path = find_indexed_path(file, host_group)?;
    let origin = anchors
        .get(host_path.refs.first()?.checked_sub(1)?)?
        .clone()?;
    let direction_group = groups.get(host_path.refs.get(1)?.checked_sub(1)?)?;
    let direction_payload = direction_group
        .records
        .iter()
        .find(|record| record.record_type == 0x07d3)
        .map(|record| record.payload(&file.data))?;
    if direction_payload.len() < 20 {
        return None;
    }

    let unit_x = read_f64(direction_payload, 4);
    let unit_y = read_f64(direction_payload, 12);
    if !unit_x.is_finite() || !unit_y.is_finite() {
        return None;
    }

    let payload = group
        .records
        .iter()
        .find(|record| record.record_type == 0x07d3)
        .map(|record| record.payload(&file.data))?;
    if payload.len() < 12 {
        return None;
    }

    let distance = read_f64(payload, 4);
    if !distance.is_finite() {
        return None;
    }

    Some(PointRecord {
        x: origin.x + distance * unit_x,
        y: origin.y - distance * unit_y,
    })
}

pub(crate) fn decode_translated_point_anchor_raw(
    file: &GspFile,
    group: &ObjectGroup,
    anchors: &[Option<PointRecord>],
) -> Option<PointRecord> {
    let constraint = decode_translated_point_constraint(file, group)?;
    let path = find_indexed_path(file, group)?;
    let origin = anchors.get(path.refs.first()?.checked_sub(1)?)?.clone()?;
    Some(PointRecord {
        x: origin.x + constraint.dx,
        y: origin.y + constraint.dy,
    })
}

pub(crate) fn decode_line_midpoint_anchor_raw(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
    anchors: &[Option<PointRecord>],
) -> Option<PointRecord> {
    if (group.header.kind()) != crate::format::GroupKind::Midpoint {
        return None;
    }

    let path = find_indexed_path(file, group)?;
    let host_group = groups.get(path.refs.first()?.checked_sub(1)?)?;
    if !host_group.header.kind().is_line_like() {
        return None;
    }

    let host_path = find_indexed_path(file, host_group)?;
    let start = anchors
        .get(host_path.refs.first()?.checked_sub(1)?)?
        .clone()?;
    let end = anchors
        .get(host_path.refs.get(1)?.checked_sub(1)?)?
        .clone()?;
    Some(PointRecord {
        x: (start.x + end.x) * 0.5,
        y: (start.y + end.y) * 0.5,
    })
}

pub(crate) fn decode_offset_anchor_raw(
    file: &GspFile,
    group: &ObjectGroup,
    anchors: &[Option<PointRecord>],
) -> Option<PointRecord> {
    if (group.header.kind()) != crate::format::GroupKind::OffsetAnchor {
        return None;
    }

    let path = find_indexed_path(file, group)?;
    let origin = anchors.get(path.refs.first()?.checked_sub(1)?)?.clone()?;
    let payload = group
        .records
        .iter()
        .find(|record| record.record_type == 0x07d3)
        .map(|record| record.payload(&file.data))?;
    if payload.len() < 20 {
        return None;
    }

    let dx = read_f64(payload, 4);
    let dy = read_f64(payload, 12);
    if !dx.is_finite() || !dy.is_finite() {
        return None;
    }

    Some(PointRecord {
        x: origin.x + dx,
        y: origin.y + dy,
    })
}

pub(crate) fn decode_point_constraint_anchor(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
    anchors: &[Option<PointRecord>],
    graph: Option<&GraphTransform>,
) -> Option<PointRecord> {
    let graph = graph.cloned();
    match try_decode_point_constraint(file, groups, group, Some(anchors), &graph).ok()? {
        RawPointConstraint::Segment(constraint) => {
            let start = anchors.get(constraint.start_group_index)?.clone()?;
            let end = anchors.get(constraint.end_group_index)?.clone()?;

            Some(lerp_point(&start, &end, constraint.t))
        }
        RawPointConstraint::ConstructedLine {
            host_group_index,
            t,
            line_like_kind: _,
        } => {
            let host_group = groups.get(host_group_index)?;
            let (start, end) = resolve_line_like_points_raw(file, groups, anchors, host_group)?;
            Some(lerp_point(&start, &end, t))
        }
        RawPointConstraint::Polyline {
            points,
            segment_index,
            t,
            ..
        } => resolve_polyline_point(&points, segment_index, t),
        RawPointConstraint::PolygonBoundary {
            vertex_group_indices,
            edge_index,
            t,
        } => {
            let vertices = vertex_group_indices
                .iter()
                .map(|group_index| anchors.get(*group_index)?.clone())
                .collect::<Option<Vec<_>>>()?;
            resolve_polygon_boundary_point_raw(&vertices, edge_index, t)
        }
        RawPointConstraint::TranslatedPolygonBoundary {
            vertex_group_indices,
            vector_start_group_index,
            vector_end_group_index,
            edge_index,
            t,
        } => {
            let vertices = vertex_group_indices
                .iter()
                .map(|group_index| anchors.get(*group_index)?.clone())
                .collect::<Option<Vec<_>>>()?;
            let base = resolve_polygon_boundary_point_raw(&vertices, edge_index, t)?;
            let vector_start = anchors.get(vector_start_group_index)?.clone()?;
            let vector_end = anchors.get(vector_end_group_index)?.clone()?;
            Some(PointRecord {
                x: base.x + vector_end.x - vector_start.x,
                y: base.y + vector_end.y - vector_start.y,
            })
        }
        RawPointConstraint::Circle(constraint) => {
            let center = anchors.get(constraint.center_group_index)?.clone()?;
            let radius_point = anchors.get(constraint.radius_group_index)?.clone()?;

            Some(resolve_circle_point_raw(
                &center,
                &radius_point,
                constraint.unit_x,
                constraint.unit_y,
            ))
        }
        RawPointConstraint::Circular(constraint) => {
            let circle_group = groups.get(constraint.circle_group_index)?;
            let circle = resolve_circle_like_raw(file, groups, anchors, circle_group)?;
            match circle {
                CircularConstraintRaw::Circle { center, radius } => Some(PointRecord {
                    x: center.x + radius * constraint.unit_x,
                    y: center.y - radius * constraint.unit_y,
                }),
                CircularConstraintRaw::ThreePointArc { .. } => None,
            }
        }
        RawPointConstraint::CircleArc(constraint) => {
            let center = anchors.get(constraint.center_group_index)?.clone()?;
            let start = anchors.get(constraint.start_group_index)?.clone()?;
            let end = anchors.get(constraint.end_group_index)?.clone()?;
            point_on_circle_arc(&center, &start, &end, constraint.t)
        }
        RawPointConstraint::Arc(constraint) => {
            let start = anchors.get(constraint.start_group_index)?.clone()?;
            let mid = anchors.get(constraint.mid_group_index)?.clone()?;
            let end = anchors.get(constraint.end_group_index)?.clone()?;
            point_on_three_point_arc(&start, &mid, &end, constraint.t)
        }
    }
}

pub(crate) fn resolve_circle_point_raw(
    center: &PointRecord,
    radius_point: &PointRecord,
    unit_x: f64,
    unit_y: f64,
) -> PointRecord {
    let radius = ((radius_point.x - center.x).powi(2) + (radius_point.y - center.y).powi(2)).sqrt();
    PointRecord {
        x: center.x + radius * unit_x,
        y: center.y - radius * unit_y,
    }
}

pub(crate) fn resolve_polygon_boundary_point_raw(
    vertices: &[PointRecord],
    edge_index: usize,
    t: f64,
) -> Option<PointRecord> {
    if vertices.len() < 2 {
        return None;
    }

    let start = &vertices[edge_index % vertices.len()];
    let end = &vertices[(edge_index + 1) % vertices.len()];
    Some(lerp_point(start, end, t))
}
