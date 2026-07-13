pub(super) fn resolve_label_hotspots(
    file: &GspFile,
    groups: &[ObjectGroup],
    labels: &mut [TextLabel],
    pending_hotspots: &[PendingLabelHotspot],
    lookups: HotspotIndexLookups<'_>,
) {
    for pending in pending_hotspots {
        let Some(label) = labels.get_mut(pending.label_index) else {
            continue;
        };

        let Some(group) = groups.get(pending.group_ordinal.saturating_sub(1)) else {
            continue;
        };
        let action = match group.header.kind() {
            crate::format::GroupKind::ActionButton => lookups
                .button_group_to_index
                .get(&pending.group_ordinal)
                .copied()
                .map(|button_index| TextLabelHotspotAction::Button { button_index }),
            crate::format::GroupKind::ButtonLabel => (|| {
                let path = find_indexed_path(file, group)?;
                let ordinal = path.refs.first().copied()?;
                lookups
                    .button_group_to_index
                    .get(&ordinal)
                    .copied()
                    .map(|button_index| TextLabelHotspotAction::Button { button_index })
            })(),
            crate::format::GroupKind::Point => lookups
                .group_to_point_index
                .get(pending.group_ordinal.saturating_sub(1))
                .copied()
                .flatten()
                .map(|point_index| TextLabelHotspotAction::Point { point_index }),
            crate::format::GroupKind::Segment => (|| {
                let path = find_indexed_path(file, group)?;
                let start_point_index = mapped_point_index(
                    lookups.group_to_point_index,
                    path.refs.first()?.saturating_sub(1),
                )?;
                let end_point_index = mapped_point_index(
                    lookups.group_to_point_index,
                    path.refs.get(1)?.saturating_sub(1),
                )?;
                Some(TextLabelHotspotAction::Segment {
                    start_point_index,
                    end_point_index,
                })
            })(),
            crate::format::GroupKind::AngleMarker => (|| {
                let path = find_indexed_path(file, group)?;
                let start_point_index = mapped_point_index(
                    lookups.group_to_point_index,
                    path.refs.first()?.saturating_sub(1),
                )?;
                let vertex_point_index = mapped_point_index(
                    lookups.group_to_point_index,
                    path.refs.get(1)?.saturating_sub(1),
                )?;
                let end_point_index = mapped_point_index(
                    lookups.group_to_point_index,
                    path.refs.get(2)?.saturating_sub(1),
                )?;
                Some(TextLabelHotspotAction::AngleMarker {
                    start_point_index,
                    vertex_point_index,
                    end_point_index,
                })
            })(),
            kind if super::decode::is_circle_group_kind(kind) => lookups
                .circle_group_to_index
                .get(pending.group_ordinal.saturating_sub(1))
                .copied()
                .flatten()
                .map(|circle_index| TextLabelHotspotAction::Circle { circle_index }),
            crate::format::GroupKind::Polygon => lookups
                .polygon_group_to_index
                .get(pending.group_ordinal.saturating_sub(1))
                .copied()
                .flatten()
                .map(|polygon_index| TextLabelHotspotAction::Polygon { polygon_index }),
            _ => None,
        };
        let Some(action) = action else {
            continue;
        };
        label.hotspots.push(TextLabelHotspot {
            line: pending.line,
            start: pending.start,
            end: pending.end,
            text: pending.text.clone(),
            action,
        });
    }
}

fn collect_label_iteration_seed_label(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
    anchors: &[Option<PointRecord>],
) -> Option<TextLabel> {
    let path = find_indexed_path(file, group)?;
    if path.refs.len() < 2 {
        return None;
    }
    let point_group_index = path.refs.first()?.checked_sub(1)?;
    let source_group = groups.get(path.refs[1].checked_sub(1)?)?;
    let anchor = decode_label_anchor(file, group, anchors)
        .or_else(|| label_iteration_seed_anchor(&path, anchors))?;
    if (source_group.header.kind()) == crate::format::GroupKind::FunctionExpr {
        return collect_point_expression_label_from_seed(
            file,
            groups,
            group,
            source_group,
            point_group_index,
            anchor,
            anchors,
        );
    }
    let ResolvedLabelText {
        text, rich_markup, ..
    } = resolve_label_text(
        file,
        source_group,
        try_decode_group_label_text(file, group)
            .or_else(|| decode_label_name(file, source_group))
            .or_else(|| decode_label_name(file, group)),
    )?;
    Some(TextLabel {
        anchor,
        text,
        rich_markup,
        color: label_color_for_group(group),
        visible: label_visible_for_group(file, group),
        screen_space: false,
        debug: Some(payload_debug_source(group)),
        ..Default::default()
    })
}

fn label_iteration_seed_anchor(
    path: &crate::format::IndexedPathRecord,
    anchors: &[Option<PointRecord>],
) -> Option<PointRecord> {
    let point_group_index = path.refs.first()?.checked_sub(1)?;
    anchors.get(point_group_index).cloned().flatten()
}

fn collect_point_expression_label_from_seed(
    file: &GspFile,
    groups: &[ObjectGroup],
    seed_group: &ObjectGroup,
    expr_group: &ObjectGroup,
    point_group_index: usize,
    anchor: PointRecord,
    anchors: &[Option<PointRecord>],
) -> Option<TextLabel> {
    let expr = try_decode_function_expr(file, groups, expr_group).ok()?;
    let (parameter_name, parameter_value) =
        resolve_function_expr_parameter(file, groups, expr_group, anchors, &mut BTreeSet::new())?;
    let value = evaluate_expr_with_parameters(
        &expr,
        0.0,
        &BTreeMap::from([(parameter_name.clone(), parameter_value)]),
    )?;
    let point_anchor = anchors
        .get(point_group_index)
        .cloned()
        .flatten()
        .unwrap_or(anchor.clone());
    Some(TextLabel {
        anchor: anchor.clone(),
        text: format_number(value),
        color: [30, 30, 30, 255],
        visible: label_visible_for_group(file, seed_group),
        binding: Some(TextLabelBinding::PointExpressionValue {
            point_index: point_group_index,
            anchor_dx: anchor.x - point_anchor.x,
            anchor_dy: anchor.y - point_anchor.y,
            anchor_y_point_index: None,
            anchor_y_dy: None,
            parameter_name,
            expr,
        }),
        screen_space: false,
        debug: Some(payload_debug_source(seed_group)),
        ..Default::default()
    })
}

fn collect_label_iteration_output_labels(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
    labels: &mut Vec<TextLabel>,
    label_group_to_index: &mut BTreeMap<usize, usize>,
) {
    for binding_group in groups
        .iter()
        .filter(|group| (group.header.kind()) == crate::format::GroupKind::IterationBinding)
    {
        let Some(path) = find_indexed_path(file, binding_group) else {
            continue;
        };
        let Some(seed_group) = path
            .refs
            .first()
            .and_then(|ordinal| groups.get(ordinal.saturating_sub(1)))
        else {
            continue;
        };
        if (seed_group.header.kind()) != crate::format::GroupKind::LabelIterationSeed {
            continue;
        }
        let Some(iter_group) = path
            .refs
            .get(1)
            .and_then(|ordinal| groups.get(ordinal.saturating_sub(1)))
        else {
            continue;
        };
        let Some(output_labels) = collect_label_iteration_output_labels_for_binding(
            file, groups, seed_group, iter_group, anchors,
        ) else {
            continue;
        };
        label_group_to_index.insert(binding_group.ordinal, labels.len());
        for label in output_labels {
            labels.push(TextLabel {
                visible: !binding_group.header.is_hidden(),
                debug: Some(payload_debug_source(binding_group)),
                ..label
            });
        }
    }
}

fn collect_label_iteration_output_labels_for_binding(
    file: &GspFile,
    groups: &[ObjectGroup],
    seed_group: &ObjectGroup,
    iter_group: &ObjectGroup,
    anchors: &[Option<PointRecord>],
) -> Option<Vec<TextLabel>> {
    let seed_path = find_indexed_path(file, seed_group)?;
    if seed_path.refs.len() < 2 {
        return None;
    }
    let source_group = groups.get(seed_path.refs[1].checked_sub(1)?)?;
    if (source_group.header.kind()) != crate::format::GroupKind::FunctionExpr {
        return None;
    }
    let anchor = decode_label_anchor(file, seed_group, anchors)
        .or_else(|| label_iteration_seed_anchor(&seed_path, anchors))?;
    let expr = try_decode_function_expr(file, groups, source_group).ok()?;
    let (parameter_name, parameter_value) = resolve_sequence_expression_state_parameter(
        file,
        groups,
        source_group,
        anchors,
    )
    .or_else(|| {
        resolve_function_expr_parameter(file, groups, source_group, anchors, &mut BTreeSet::new())
    })?;
    let depth = iter_group
        .records
        .iter()
        .find(|record| record.record_type == RECORD_ITERATION_DEFINITION)
        .map(|record| record.payload(&file.data))
        .filter(|payload| payload.len() >= 20)
        .map(|payload| read_u32(payload, 16) as usize)
        .unwrap_or(3);
    let depth_parameter_name = iteration_depth_driver_name(file, groups, iter_group);
    let Some(anchor_step) = label_iteration_anchor_step(file, groups, &seed_path, anchors) else {
        let text =
            evaluate_sequence_expression_value(&expr, &parameter_name, parameter_value, depth)
                .map(format_number)
                .or_else(|| try_decode_group_label_text(file, seed_group))
                .or_else(|| decode_label_name(file, seed_group))
                .unwrap_or_else(|| "未定义".to_string());
        return Some(vec![TextLabel {
            anchor,
            text,
            color: [30, 30, 30, 255],
            binding: Some(TextLabelBinding::SequenceExpressionValue {
                parameter_name,
                expr,
                depth,
                depth_parameter_name,
            }),
            screen_space: false,
            ..Default::default()
        }]);
    };

    let output_labels = (1..=depth)
        .filter_map(|step_index| {
            let text = evaluate_sequence_expression_value(
                &expr,
                &parameter_name,
                parameter_value,
                step_index,
            )
            .map(format_number)?;
            let delta = anchor_step.clone() * step_index as f64;
            Some(TextLabel {
                anchor: anchor.clone() + delta,
                text,
                color: [30, 30, 30, 255],
                binding: None,
                screen_space: false,
                ..Default::default()
            })
        })
        .collect::<Vec<_>>();
    (!output_labels.is_empty()).then_some(output_labels)
}

fn label_iteration_anchor_step(
    file: &GspFile,
    groups: &[ObjectGroup],
    seed_path: &crate::format::IndexedPathRecord,
    anchors: &[Option<PointRecord>],
) -> Option<PointRecord> {
    let point_group = groups.get(seed_path.refs.first()?.checked_sub(1)?)?;
    if point_group.header.kind() != crate::format::GroupKind::Translation {
        return None;
    }
    let path = find_indexed_path(file, point_group)?;
    if path.refs.len() < 3 {
        return None;
    }
    let start = anchors.get(path.refs[1].checked_sub(1)?)?.clone()?;
    let end = anchors.get(path.refs[2].checked_sub(1)?)?.clone()?;
    let step = end - start;
    ((step.x.abs() >= 1e-6) || (step.y.abs() >= 1e-6)).then_some(step)
}

fn resolve_sequence_expression_state_parameter(
    file: &GspFile,
    groups: &[ObjectGroup],
    source_group: &ObjectGroup,
    anchors: &[Option<PointRecord>],
) -> Option<(String, f64)> {
    let source_path = find_indexed_path(file, source_group)?;
    let calc_group = groups.get(source_path.refs.first()?.checked_sub(1)?)?;
    let calc_path = find_indexed_path(file, calc_group)?;
    let first_group = groups.get(calc_path.refs.first()?.checked_sub(1)?)?;
    if first_group.header.kind() != crate::format::GroupKind::FunctionExpr {
        return resolve_function_expr_parameter(
            file,
            groups,
            calc_group,
            anchors,
            &mut BTreeSet::new(),
        );
    }
    calc_path
        .refs
        .iter()
        .skip(1)
        .filter_map(|ordinal| groups.get(ordinal.saturating_sub(1)))
        .find(|group| group.header.kind() == crate::format::GroupKind::FunctionExpr)
        .and_then(|group| {
            resolve_function_expr_parameter(file, groups, group, anchors, &mut BTreeSet::new())
        })
}

fn evaluate_sequence_expression_value(
    expr: &crate::runtime::functions::FunctionExpr,
    parameter_name: &str,
    parameter_value: f64,
    depth: usize,
) -> Option<f64> {
    let mut current_value = parameter_value;
    let mut value = None;
    for _ in 0..=depth {
        let next_value = evaluate_expr_with_parameters(
            expr,
            0.0,
            &BTreeMap::from([(parameter_name.to_string(), current_value)]),
        )?;
        if !next_value.is_finite() {
            return None;
        }
        value = Some(next_value);
        current_value = next_value;
    }
    value
}

pub(super) fn collect_polygon_parameter_labels(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
) -> Vec<TextLabel> {
    let mut labels = groups
        .iter()
        .filter(|group| (group.header.kind()) == crate::format::GroupKind::ParameterAnchor)
        .filter_map(|group| {
            let path = find_indexed_path(file, group)?;
            if path.refs.len() < 2 {
                return None;
            }

            let point_group = groups.get(path.refs[0].checked_sub(1)?)?;
            let polygon_group = groups.get(path.refs[1].checked_sub(1)?)?;
            if !point_group.header.kind().is_point_constraint()
                || (polygon_group.header.kind()) != crate::format::GroupKind::Polygon
            {
                return None;
            }

            let point_name =
                decode_label_name(file, group).or_else(|| decode_label_name(file, point_group))?;
            let polygon_name = polygon_vertex_name(file, groups, polygon_group)?;
            let anchor = decode_label_anchor(file, group, anchors).or_else(|| {
                let anchor_record = group
                    .records
                    .iter()
                    .find(|record| record.record_type == crate::runtime::payload_consts::RECORD_ACTION_AUX)?;
                decode_text_anchor(anchor_record.payload(&file.data))
            })?;
            let RawPointConstraint::PolygonBoundary {
                vertex_group_indices,
                edge_index,
                t,
            } = try_decode_point_constraint(file, groups, point_group, None, &None).ok()?
            else {
                return None;
            };
            let global_t =
                polygon_boundary_parameter(anchors, &vertex_group_indices, edge_index, t)?;

            Some(TextLabel {
                anchor,
                text: if decode_label_name(file, group).is_some() {
                    format!("{point_name} = {:.2}", global_t)
                } else {
                    format!("{point_name}在{polygon_name}上的值 = {:.2}", global_t)
                },
                color: [30, 30, 30, 255],
                visible: !group.header.is_hidden()
                    && (decode_label_name(file, group).is_some()
                        || label_visible_for_group(file, group)),
                binding: Some(TextLabelBinding::PolygonBoundaryParameter {
                    point_index: path.refs[0].checked_sub(1)?,
                    point_name,
                    polygon_name,
                }),
                screen_space: false,
                debug: Some(payload_debug_source(group)),
                ..Default::default()
            })
        })
        .collect::<Vec<_>>();
    labels.iter_mut().for_each(apply_fallback_rich_markup);
    labels
}

pub(super) fn collect_line_projection_parameter_labels(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
) -> Vec<TextLabel> {
    let mut labels = groups
        .iter()
        .filter(|group| (group.header.kind()) == crate::format::GroupKind::ParameterAnchor)
        .filter_map(|group| {
            let path = find_indexed_path(file, group)?;
            if path.refs.len() < 2 {
                return None;
            }

            let point_group = groups.get(path.refs[0].checked_sub(1)?)?;
            let segment_group = groups.get(path.refs[1].checked_sub(1)?)?;
            if !segment_group.header.kind().is_line_like() {
                return None;
            }
            let segment_path = find_indexed_path(file, segment_group)?;
            let start_group_index = segment_path.refs.first()?.checked_sub(1)?;
            let end_group_index = segment_path.refs.get(1)?.checked_sub(1)?;

            let point_name =
                decode_label_name(file, group).or_else(|| decode_label_name(file, point_group))?;
            let object_name = segment_name(file, groups, segment_group)?;
            let line_kind = line_like_kind(segment_group.header.kind())?;
            let anchor_record = group
                .records
                .iter()
                .find(|record| record.record_type == crate::runtime::payload_consts::RECORD_ACTION_AUX)?;
            let anchor = decode_text_anchor(anchor_record.payload(&file.data))?;
            let point = anchors.get(path.refs[0].checked_sub(1)?)?.as_ref()?;
            let start = anchors.get(start_group_index)?.as_ref()?;
            let end = anchors.get(end_group_index)?.as_ref()?;
            let projected_t = line_projection_parameter(point, start, end, line_kind)?;

            Some(TextLabel {
                anchor,
                text: if decode_label_name(file, group).is_some() {
                    format!("{point_name} = {:.2}", projected_t)
                } else {
                    format!("{point_name}在{object_name}上的t值 = {:.2}", projected_t)
                },
                color: [30, 30, 30, 255],
                visible: !group.header.is_hidden()
                    && (decode_label_name(file, group).is_some()
                        || label_visible_for_group(file, group)),
                binding: Some(TextLabelBinding::LineProjectionParameter {
                    point_index: path.refs[0].checked_sub(1)?,
                    start_index: start_group_index,
                    end_index: end_group_index,
                    line_kind,
                    point_name,
                    object_name,
                }),
                screen_space: false,
                debug: Some(payload_debug_source(group)),
                ..Default::default()
            })
        })
        .collect::<Vec<_>>();
    labels.iter_mut().for_each(apply_fallback_rich_markup);
    labels
}

pub(super) fn collect_polyline_parameter_labels(
    file: &GspFile,
    groups: &[ObjectGroup],
    _anchors: &[Option<PointRecord>],
) -> Vec<TextLabel> {
    let mut labels = groups
        .iter()
        .filter(|group| (group.header.kind()) == crate::format::GroupKind::ParameterAnchor)
        .filter_map(|group| {
            let path = find_indexed_path(file, group)?;
            if path.refs.len() < 2 {
                return None;
            }

            let point_group = groups.get(path.refs[0].checked_sub(1)?)?;
            let host_group = groups.get(path.refs[1].checked_sub(1)?)?;
            if !matches!(
                host_group.header.kind(),
                crate::format::GroupKind::PointTrace
                    | crate::format::GroupKind::CoordinateTrace
                    | crate::format::GroupKind::CustomTransformTrace
            ) {
                return None;
            }

            let value = point_on_trace_payload_parameter(file, point_group)?;
            let point_name =
                decode_label_name(file, group).or_else(|| decode_label_name(file, point_group))?;
            let object_name = trace_object_name(file, groups, host_group)?;
            let anchor_record = group
                .records
                .iter()
                .find(|record| record.record_type == crate::runtime::payload_consts::RECORD_ACTION_AUX)?;
            let anchor = decode_text_anchor(anchor_record.payload(&file.data))?;

            Some(TextLabel {
                anchor,
                text: format!(
                    "{point_name}在{object_name}上的值 = {}",
                    format_number(value)
                ),
                color: [30, 30, 30, 255],
                visible: decode_label_name(file, group).is_some()
                    || label_visible_for_group(file, group),
                binding: Some(TextLabelBinding::PolylineParameter {
                    point_index: path.refs[0].checked_sub(1)?,
                    point_name,
                    object_name,
                }),
                screen_space: true,
                debug: Some(payload_debug_source(group)),
                ..Default::default()
            })
        })
        .collect::<Vec<_>>();
    labels.iter_mut().for_each(apply_fallback_rich_markup);
    labels
}

pub(super) fn collect_circle_parameter_labels(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
) -> Vec<TextLabel> {
    let mut labels = groups
        .iter()
        .filter(|group| (group.header.kind()) == crate::format::GroupKind::ParameterAnchor)
        .filter_map(|group| {
            let path = find_indexed_path(file, group)?;
            if path.refs.len() < 2 {
                return None;
            }

            let point_group = groups.get(path.refs[0].checked_sub(1)?)?;
            let circle_group = groups.get(path.refs[1].checked_sub(1)?)?;
            if !point_group.header.kind().is_point_constraint()
                || (circle_group.header.kind()) != crate::format::GroupKind::Circle
            {
                return None;
            }

            let point_name =
                decode_label_name(file, group).or_else(|| decode_label_name(file, point_group))?;
            let circle_name = circle_name(file, groups, circle_group)?;
            let anchor_record = group
                .records
                .iter()
                .find(|record| record.record_type == crate::runtime::payload_consts::RECORD_ACTION_AUX)?;
            let anchor = decode_text_anchor(anchor_record.payload(&file.data))?;
            let RawPointConstraint::Circle(constraint) =
                try_decode_point_constraint(file, groups, point_group, None, &None).ok()?
            else {
                return None;
            };
            let value = circle_parameter(
                anchors,
                constraint.center_group_index,
                constraint.radius_group_index,
                constraint.unit_x,
                constraint.unit_y,
            )?;

            Some(TextLabel {
                anchor,
                text: if decode_label_name(file, group).is_some() {
                    format!("{point_name} = {:.2}", value)
                } else {
                    format!("{point_name}在⊙{circle_name}上的值 = {:.2}", value)
                },
                color: [30, 30, 30, 255],
                visible: decode_label_name(file, group).is_some()
                    || label_visible_for_group(file, group),
                binding: Some(TextLabelBinding::CircleParameter {
                    point_index: path.refs[0].checked_sub(1)?,
                    point_name,
                    circle_name,
                }),
                screen_space: false,
                debug: Some(payload_debug_source(group)),
                ..Default::default()
            })
        })
        .collect::<Vec<_>>();
    labels.iter_mut().for_each(apply_fallback_rich_markup);
    labels
}

pub(super) fn collect_custom_transform_expression_labels(
    file: &GspFile,
    groups: &[ObjectGroup],
    context: &SceneContext<'_>,
    anchors: &[Option<PointRecord>],
) -> Vec<TextLabel> {
    groups
        .iter()
        .filter(|group| (group.header.kind()) == crate::format::GroupKind::CustomTransformTrace)
        .filter_map(|group| {
            let path = context.indexed_path(group)?;
            if path.refs.len() < 6 {
                return None;
            }
            let source_group = context.group_by_ordinal(*path.refs.get(2)?)?;
            let parameter_anchor_group = context.group_by_ordinal(*path.refs.get(3)?)?;
            let distance_expr_group = context.group_by_ordinal(*path.refs.get(4)?)?;
            let angle_expr_group = context.group_by_ordinal(*path.refs.get(5)?)?;
            let base_label =
                custom_transform_parameter_anchor_label(file, groups, parameter_anchor_group)?;
            let t =
                match try_decode_point_constraint(file, groups, source_group, Some(anchors), &None)
                    .ok()?
                {
                    RawPointConstraint::Segment(constraint) => constraint.t,
                    _ => return None,
                };

            let mut labels = Vec::new();
            for (expr_group, suffix, multiplier_text, display_scale, value_suffix, decimals) in [
                (
                    distance_expr_group,
                    custom_transform_expr_suffix(file, distance_expr_group),
                    "1厘米",
                    1.0,
                    " 厘米",
                    4usize,
                ),
                (
                    angle_expr_group,
                    custom_transform_expr_suffix(file, angle_expr_group),
                    "100°",
                    100.0,
                    "°",
                    5usize,
                ),
            ] {
                let suffix_code = suffix?;
                if !matches!(suffix_code, 0x0201 | 0x0101) {
                    continue;
                }
                let anchor = try_decode_payload_anchor_point(file, expr_group)
                    .ok()
                    .flatten()?;
                let expr = context.function_expr(expr_group).ok()?;
                let value = evaluate_expr_with_parameters(
                    &expr,
                    t,
                    &BTreeMap::from([(
                        format!("__param_anchor_{}", parameter_anchor_group.ordinal),
                        t,
                    )]),
                )? * display_scale;
                labels.push(TextLabel {
                    anchor,
                    text: format!(
                        "{base_label}·{multiplier_text} = {value:.decimals$}{value_suffix}"
                    ),
                    color: [30, 30, 30, 255],
                    visible: label_visible_for_group(file, group),
                    binding: Some(TextLabelBinding::CustomTransformValue {
                        point_index: path.refs.get(2)?.checked_sub(1)?,
                        expr_label: format!("{base_label}·{multiplier_text}"),
                        expr,
                        value_scale: display_scale,
                        value_suffix: value_suffix.to_string(),
                    }),
                    screen_space: false,
                    debug: Some(payload_debug_source(group)),
                    ..Default::default()
                });
            }
            Some(labels)
        })
        .flatten()
        .collect()
}

fn custom_transform_parameter_anchor_label(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
) -> Option<String> {
    let path = find_indexed_path(file, group)?;
    if path.refs.len() < 2 {
        return None;
    }
    let point_group = groups.get(path.refs[0].checked_sub(1)?)?;
    let segment_group = groups.get(path.refs[1].checked_sub(1)?)?;
    if !point_group.header.kind().is_point_constraint()
        || !segment_group.header.kind().is_line_like()
    {
        return None;
    }
    Some(format!(
        "{}在{}上的t值",
        decode_label_name(file, point_group)?,
        segment_name(file, groups, segment_group)?
    ))
}

fn custom_transform_expr_suffix(file: &GspFile, expr_group: &ObjectGroup) -> Option<u16> {
    let payload = expr_group
        .records
        .iter()
        .find(|record| record.record_type == crate::runtime::payload_consts::RECORD_FUNCTION_EXPR_PAYLOAD)
        .map(|record| record.payload(&file.data))?;
    payload
        .chunks_exact(2)
        .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
        .next_back()
}
