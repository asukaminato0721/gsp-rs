pub(super) fn collect_label_iterations(
    file: &GspFile,
    groups: &[ObjectGroup],
    label_group_to_index: &BTreeMap<usize, usize>,
    group_to_point_index: &[Option<usize>],
    anchors: &[Option<PointRecord>],
) -> Vec<LabelIterationFamily> {
    groups
        .iter()
        .filter(|group| (group.header.kind()) == crate::format::GroupKind::IterationBinding)
        .filter_map(|group| {
            let path = find_indexed_path(file, group)?;
            if path.refs.len() < 2 {
                return None;
            }
            let seed_group = groups.get(path.refs[0].checked_sub(1)?)?;
            if (seed_group.header.kind()) != crate::format::GroupKind::LabelIterationSeed {
                return None;
            }
            let seed_path = find_indexed_path(file, seed_group)?;
            let point_group_index = seed_path.refs.first()?.checked_sub(1)?;
            let expr_group = groups.get(seed_path.refs.get(1)?.checked_sub(1)?)?;
            if (expr_group.header.kind()) != crate::format::GroupKind::FunctionExpr {
                return None;
            }
            let expr = try_decode_function_expr(file, groups, expr_group).ok()?;
            let expr_path = find_indexed_path(file, expr_group)?;
            let parameter_group = groups.get(expr_path.refs.first()?.checked_sub(1)?)?;
            let parameter_name = decode_label_name(file, parameter_group)?;
            let seed_label_index = *label_group_to_index.get(&seed_group.ordinal)?;

            let iter_group = groups.get(path.refs[1].checked_sub(1)?)?;
            let depth = iter_group
                .records
                .iter()
                .find(|record| record.record_type == 0x090a)
                .map(|record| record.payload(&file.data))
                .filter(|payload| payload.len() >= 20)
                .map(|payload| read_u32(payload, 16) as usize)
                .unwrap_or(3);
            let depth_parameter_name = iteration_depth_driver_name(file, groups, iter_group);
            if let Some((vector_start_index, vector_end_index)) = label_iteration_vector_indices(
                file,
                groups,
                &seed_path,
                group_to_point_index,
                anchors,
            ) {
                return Some(LabelIterationFamily::TranslateExpression {
                    seed_label_index,
                    first_output_label_index: label_group_to_index.get(&group.ordinal).copied(),
                    output_label_count: depth,
                    vector_start_index,
                    vector_end_index,
                    parameter_name,
                    expr,
                    depth,
                    depth_expr: label_iteration_depth_expr(file, groups, iter_group),
                    depth_parameter_name,
                });
            }

            let point_seed_index = mapped_point_index(group_to_point_index, point_group_index)?;

            Some(LabelIterationFamily::PointExpression {
                seed_label_index,
                point_seed_index,
                parameter_name,
                expr,
                depth,
                depth_parameter_name,
            })
        })
        .collect()
}

fn label_iteration_vector_indices(
    file: &GspFile,
    groups: &[ObjectGroup],
    seed_path: &crate::format::IndexedPathRecord,
    group_to_point_index: &[Option<usize>],
    anchors: &[Option<PointRecord>],
) -> Option<(usize, usize)> {
    let point_group_index = seed_path.refs.first()?.checked_sub(1)?;
    let point_group = groups.get(point_group_index)?;
    if point_group.header.kind() != crate::format::GroupKind::Translation {
        return None;
    }
    let path = find_indexed_path(file, point_group)?;
    if path.refs.len() < 3 {
        return None;
    }
    Some((
        mapped_point_index_exact(group_to_point_index, anchors, path.refs[1].checked_sub(1)?)?,
        mapped_point_index_exact(group_to_point_index, anchors, path.refs[2].checked_sub(1)?)?,
    ))
}

fn mapped_point_index_exact(
    group_to_point_index: &[Option<usize>],
    _anchors: &[Option<PointRecord>],
    group_index: usize,
) -> Option<usize> {
    mapped_point_index(group_to_point_index, group_index)
}

fn label_iteration_depth_expr(
    file: &GspFile,
    groups: &[ObjectGroup],
    iter_group: &ObjectGroup,
) -> Option<FunctionExpr> {
    let path = find_indexed_path(file, iter_group)?;
    let depth_group = groups.get(path.refs.first()?.checked_sub(1)?)?;
    if depth_group.header.kind() != crate::format::GroupKind::FunctionExpr {
        return None;
    }
    decode_iteration_depth_expr(file, groups, depth_group)
}

pub(super) fn bind_button_seed_expression_labels(
    file: &GspFile,
    groups: &[ObjectGroup],
    context: &SceneContext<'_>,
    anchors: &[Option<PointRecord>],
    labels: &mut [TextLabel],
    label_group_to_index: &BTreeMap<usize, usize>,
    group_to_point_index: &[Option<usize>],
) {
    for seed_group in groups
        .iter()
        .filter(|group| (group.header.kind()) == crate::format::GroupKind::LabelIterationSeed)
    {
        let Some(seed_path) = context.indexed_path(seed_group) else {
            continue;
        };
        if seed_path.refs.len() < 2 {
            continue;
        }
        let Some(point_group_index) = seed_path
            .refs
            .first()
            .and_then(|ordinal| ordinal.checked_sub(1))
        else {
            continue;
        };
        let Some(point_index) = mapped_point_index(group_to_point_index, point_group_index) else {
            continue;
        };
        let Some(point_anchor) = anchors.get(point_group_index).cloned().flatten() else {
            continue;
        };
        let Some(button_group) = seed_path
            .refs
            .get(1)
            .and_then(|ordinal| context.group_by_ordinal(*ordinal))
        else {
            continue;
        };
        if (button_group.header.kind()) != crate::format::GroupKind::ButtonLabel {
            continue;
        }
        let Some(label_index) = label_group_to_index.get(&button_group.ordinal).copied() else {
            continue;
        };
        let Some(expr_group) = context
            .indexed_path(button_group)
            .and_then(|path| path.refs.first().copied())
            .and_then(|ordinal| context.group_by_ordinal(ordinal))
        else {
            continue;
        };
        if (expr_group.header.kind()) != crate::format::GroupKind::FunctionExpr {
            continue;
        }
        let Some(expr) = context.function_expr(expr_group).ok() else {
            continue;
        };
        let Some((parameter_name, parameter_value)) = resolve_function_expr_parameter(
            file,
            groups,
            expr_group,
            anchors,
            &mut BTreeSet::new(),
        ) else {
            continue;
        };
        let value = evaluate_expr_with_parameters(
            &expr,
            0.0,
            &BTreeMap::from([(parameter_name.clone(), parameter_value)]),
        );
        let expr_label = payload_function_expr_label(
            file,
            groups,
            anchors,
            expr_group,
            &function_expr_label(expr.clone()),
            &mut BTreeSet::new(),
        );
        let Some(label) = labels.get_mut(label_index) else {
            continue;
        };
        let anchor_dx = 0.0;
        let anchor_dy = 0.0;
        let value_text = value
            .map(format_number)
            .unwrap_or_else(|| "未定义".to_string());
        label.anchor = point_anchor;
        label.text = format!("{expr_label} = {value_text}");
        label.rich_markup = build_expression_rich_markup(&expr_label, &value_text);
        label.binding = Some(TextLabelBinding::PointBoundExpressionValue {
            point_index,
            anchor_dx,
            anchor_dy,
            parameter_name,
            result_name: decode_label_name(file, expr_group),
            expr_label,
            expr,
        });
    }
}

pub(super) fn bind_point_label_anchors(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
    group_to_point_index: &[Option<usize>],
    labels: &mut [TextLabel],
    label_group_to_index: &BTreeMap<usize, usize>,
) {
    for group in groups {
        let Some(label_index) = label_group_to_index.get(&group.ordinal).copied() else {
            continue;
        };
        let Some(point_group_index) = point_label_anchor_group_index(file, group) else {
            continue;
        };
        let Some(point_index) =
            mapped_point_index_exact(group_to_point_index, anchors, point_group_index)
        else {
            continue;
        };
        let Some(point_anchor) = anchors.get(point_group_index).cloned().flatten() else {
            continue;
        };
        let Some(label) = labels.get_mut(label_index) else {
            continue;
        };
        if label.binding.is_some() {
            continue;
        }
        label.binding = Some(TextLabelBinding::PointAnchor {
            point_index,
            anchor_dx: label.anchor.x - point_anchor.x,
            anchor_dy: label.anchor.y - point_anchor.y,
            anchor_y_point_index: None,
            anchor_y_dy: None,
        });
    }
}

pub(super) fn bind_label_iteration_seed_anchors(
    file: &GspFile,
    groups: &[ObjectGroup],
    labels: &mut [TextLabel],
    label_group_to_index: &BTreeMap<usize, usize>,
    label_iterations: &[LabelIterationFamily],
    points: &[ScenePoint],
    group_to_point_index: &[Option<usize>],
) {
    let mut origin_by_parameter = BTreeMap::new();
    let label_y_control =
        label_iteration_vertical_control(file, groups, group_to_point_index, points);
    for family in label_iterations {
        let LabelIterationFamily::TranslateExpression {
            seed_label_index,
            vector_start_index,
            vector_end_index,
            parameter_name,
            expr,
            ..
        } = family
        else {
            continue;
        };
        origin_by_parameter.insert(parameter_name.clone(), *vector_start_index);
        let Some(label) = labels.get_mut(*seed_label_index) else {
            continue;
        };
        let Some(point) = points.get(*vector_end_index) else {
            continue;
        };
        label.binding = Some(TextLabelBinding::PointExpressionValue {
            point_index: *vector_end_index,
            anchor_dx: label.anchor.x - point.position.x,
            anchor_dy: label.anchor.y - point.position.y,
            anchor_y_point_index: label_y_control.as_ref().map(|control| control.point_index),
            anchor_y_dy: label_y_control
                .as_ref()
                .map(|control| label.anchor.y - control.base_y),
            parameter_name: parameter_name.clone(),
            expr: expr.clone(),
        });
    }
    for group in groups {
        if group.header.kind() != crate::format::GroupKind::LabelIterationSeed {
            continue;
        }
        let Some(label_index) = label_group_to_index.get(&group.ordinal).copied() else {
            continue;
        };
        let Some(label) = labels.get_mut(label_index) else {
            continue;
        };
        if label.binding.is_some() {
            continue;
        }
        let Some(path) = find_indexed_path(file, group) else {
            continue;
        };
        let Some(expr_group) = path
            .refs
            .get(1)
            .and_then(|ordinal| groups.get(ordinal.saturating_sub(1)))
        else {
            continue;
        };
        if expr_group.header.kind() != crate::format::GroupKind::FunctionExpr {
            if label.text == "0"
                && let Some(point_index) = origin_by_parameter.values().next().copied()
                && let Some(point) = points.get(point_index)
            {
                label.binding = Some(TextLabelBinding::PointAnchor {
                    point_index,
                    anchor_dx: label.anchor.x - point.position.x,
                    anchor_dy: label.anchor.y - point.position.y,
                    anchor_y_point_index: label_y_control
                        .as_ref()
                        .map(|control| control.point_index),
                    anchor_y_dy: label_y_control
                        .as_ref()
                        .map(|control| label.anchor.y - control.base_y),
                });
            }
            continue;
        }
        let Some(expr_path) = find_indexed_path(file, expr_group) else {
            continue;
        };
        let Some(parameter_name) = expr_path
            .refs
            .first()
            .and_then(|ordinal| groups.get(ordinal.saturating_sub(1)))
            .and_then(|parameter_group| decode_label_name(file, parameter_group))
        else {
            if label.text == "0"
                && let Some(point_index) = origin_by_parameter.values().next().copied()
                && let Some(point) = points.get(point_index)
            {
                label.binding = Some(TextLabelBinding::PointAnchor {
                    point_index,
                    anchor_dx: label.anchor.x - point.position.x,
                    anchor_dy: label.anchor.y - point.position.y,
                    anchor_y_point_index: label_y_control
                        .as_ref()
                        .map(|control| control.point_index),
                    anchor_y_dy: label_y_control
                        .as_ref()
                        .map(|control| label.anchor.y - control.base_y),
                });
            }
            continue;
        };
        let Some(point_index) = origin_by_parameter.get(&parameter_name).copied() else {
            if label.text != "0" {
                continue;
            }
            let Some(point_index) = origin_by_parameter.values().next().copied() else {
                continue;
            };
            let Some(point) = points.get(point_index) else {
                continue;
            };
            label.binding = Some(TextLabelBinding::PointAnchor {
                point_index,
                anchor_dx: label.anchor.x - point.position.x,
                anchor_dy: label.anchor.y - point.position.y,
                anchor_y_point_index: label_y_control.as_ref().map(|control| control.point_index),
                anchor_y_dy: label_y_control
                    .as_ref()
                    .map(|control| label.anchor.y - control.base_y),
            });
            continue;
        };
        let Some(point) = points.get(point_index) else {
            continue;
        };
        label.binding = Some(TextLabelBinding::PointAnchor {
            point_index,
            anchor_dx: label.anchor.x - point.position.x,
            anchor_dy: label.anchor.y - point.position.y,
            anchor_y_point_index: label_y_control.as_ref().map(|control| control.point_index),
            anchor_y_dy: label_y_control
                .as_ref()
                .map(|control| label.anchor.y - control.base_y),
        });
    }
}

struct LabelVerticalControl {
    point_index: usize,
    base_y: f64,
}

fn label_iteration_vertical_control(
    file: &GspFile,
    groups: &[ObjectGroup],
    group_to_point_index: &[Option<usize>],
    points: &[ScenePoint],
) -> Option<LabelVerticalControl> {
    let zero_label_point_group_index = groups.iter().find_map(|group| {
        if group.header.kind() != crate::format::GroupKind::LabelIterationSeed {
            return None;
        }
        let path = find_indexed_path(file, group)?;
        let expr_group = path
            .refs
            .get(1)
            .and_then(|ordinal| groups.get(ordinal.checked_sub(1)?))?;
        (expr_group.header.kind() != crate::format::GroupKind::FunctionExpr)
            .then(|| path.refs.first()?.checked_sub(1))
            .flatten()
    })?;
    groups.iter().enumerate().find_map(|(group_index, group)| {
        if group.header.kind() != crate::format::GroupKind::Translation || group.header.is_hidden()
        {
            return None;
        }
        let path = find_indexed_path(file, group)?;
        if path.refs.first()?.checked_sub(1)? != zero_label_point_group_index {
            return None;
        }
        let point_index = group_to_point_index.get(group_index).copied().flatten()?;
        let vector_start_index = path
            .refs
            .get(1)
            .and_then(|ordinal| group_to_point_index.get(ordinal.checked_sub(1)?))
            .copied()
            .flatten()?;
        let vector_end_index = path
            .refs
            .get(2)
            .and_then(|ordinal| group_to_point_index.get(ordinal.checked_sub(1)?))
            .copied()
            .flatten()?;
        let control = points.get(point_index)?;
        let vector_start = points.get(vector_start_index)?;
        let vector_end = points.get(vector_end_index)?;
        Some(LabelVerticalControl {
            point_index,
            base_y: control.position.y - (vector_end.position.y - vector_start.position.y),
        })
    })
}

fn point_label_anchor_group_index(file: &GspFile, group: &ObjectGroup) -> Option<usize> {
    if group.header.kind() == crate::format::GroupKind::LabelIterationSeed {
        let path = find_indexed_path(file, group)?;
        return path.refs.first()?.checked_sub(1);
    }
    if matches!(
        group.header.kind(),
        crate::format::GroupKind::Point
            | crate::format::GroupKind::LegacyCoordinateConstructPoint
            | crate::format::GroupKind::FixedCoordinatePoint
            | crate::format::GroupKind::CustomTransformPoint
            | crate::format::GroupKind::Translation
            | crate::format::GroupKind::Reflection
            | crate::format::GroupKind::Rotation
            | crate::format::GroupKind::ExpressionRotation
            | crate::format::GroupKind::ParameterRotation
            | crate::format::GroupKind::ParameterControlledPoint
            | crate::format::GroupKind::Scale
            | crate::format::GroupKind::PointConstraint
            | crate::format::GroupKind::PathPoint
            | crate::format::GroupKind::LinearIntersectionPoint
            | crate::format::GroupKind::IntersectionPoint1
            | crate::format::GroupKind::IntersectionPoint2
            | crate::format::GroupKind::CircleCircleIntersectionPoint1
            | crate::format::GroupKind::CircleCircleIntersectionPoint2
            | crate::format::GroupKind::Midpoint
            | crate::format::GroupKind::GraphCalibrationX
            | crate::format::GroupKind::GraphCalibrationY
            | crate::format::GroupKind::GraphCalibrationYAlt
    ) {
        return group.ordinal.checked_sub(1);
    }
    None
}

pub(super) fn collect_iteration_tables(
    file: &GspFile,
    groups: &[ObjectGroup],
    context: &SceneContext<'_>,
    anchors: &[Option<PointRecord>],
) -> Vec<IterationTable> {
    groups
        .iter()
        .filter(|group| {
            (group.header.kind()) == crate::format::GroupKind::IterationExpressionHelper
        })
        .filter_map(|group| {
            let path = context.indexed_path(group)?;
            if path.refs.len() < 2 {
                return None;
            }
            let iter_group = context.group_by_ordinal(path.refs[0])?;
            let columns = path
                .refs
                .iter()
                .skip(1)
                .filter_map(|ordinal| {
                    let expr_group = context.group_by_ordinal(*ordinal)?;
                    if expr_group.header.kind() != crate::format::GroupKind::FunctionExpr {
                        return None;
                    }
                    let expr = context.function_expr(expr_group).ok()?;
                    let parameter_name = direct_function_expr_parameter_name(
                        file, groups, expr_group,
                    )
                    .or_else(|| {
                        resolve_function_expr_parameter(
                            file,
                            groups,
                            expr_group,
                            anchors,
                            &mut BTreeSet::new(),
                        )
                        .map(|(name, _)| name)
                    })?;
                    let expr_label = payload_function_expr_label(
                        file,
                        groups,
                        anchors,
                        expr_group,
                        &function_expr_label(expr.clone()),
                        &mut BTreeSet::new(),
                    );
                    Some(IterationTableColumn {
                        expr_label,
                        parameter_name,
                        expr,
                    })
                })
                .collect::<Vec<_>>();
            let first_column = columns.first()?;
            let depth = iter_group
                .records
                .iter()
                .find(|record| record.record_type == 0x090a)
                .map(|record| record.payload(&file.data))
                .filter(|payload| payload.len() >= 20)
                .map(|payload| read_u32(payload, 16) as usize)
                .unwrap_or(3);
            let depth_parameter_name = iteration_depth_driver_name(file, groups, iter_group);
            let depth_expr = context
                .indexed_path(iter_group)
                .and_then(|iter_path| iter_path.refs.first())
                .and_then(|ordinal| context.group_by_ordinal(*ordinal))
                .filter(|group| group.header.kind() == crate::format::GroupKind::FunctionExpr)
                .and_then(|group| context.function_expr(group).ok());
            Some(IterationTable {
                anchor: decode_iteration_table_anchor(file, group)?,
                expr_label: first_column.expr_label.clone(),
                parameter_name: first_column.parameter_name.clone(),
                expr: first_column.expr.clone(),
                columns,
                depth,
                depth_expr,
                depth_parameter_name,
                visible: true,
                debug: Some(payload_debug_source(group)),
            })
        })
        .collect()
}

fn segment_name(
    file: &GspFile,
    groups: &[ObjectGroup],
    segment_group: &ObjectGroup,
) -> Option<String> {
    let path = find_indexed_path(file, segment_group)?;
    let names = path
        .refs
        .iter()
        .map(|&object_ref| {
            let group = groups.get(object_ref.checked_sub(1)?)?;
            decode_label_name(file, group)
        })
        .collect::<Option<Vec<_>>>()?;
    (names.len() >= 2).then(|| names.join(""))
}

fn circle_name(
    file: &GspFile,
    groups: &[ObjectGroup],
    circle_group: &ObjectGroup,
) -> Option<String> {
    let path = find_indexed_path(file, circle_group)?;
    let names = path
        .refs
        .iter()
        .map(|&object_ref| {
            let group = groups.get(object_ref.checked_sub(1)?)?;
            decode_label_name(file, group)
        })
        .collect::<Option<Vec<_>>>()?;
    (names.len() >= 2).then(|| names.join(""))
}

fn trace_object_name(
    file: &GspFile,
    groups: &[ObjectGroup],
    trace_group: &ObjectGroup,
) -> Option<String> {
    decode_label_name(file, trace_group).or_else(|| {
        let kind = trace_group.header.kind();
        let index = groups
            .iter()
            .filter(|group| group.header.kind() == kind)
            .take_while(|group| group.ordinal <= trace_group.ordinal)
            .count()
            .max(1);
        Some(format!("L{}", subscript_number(index)))
    })
}

fn subscript_number(value: usize) -> String {
    value
        .to_string()
        .chars()
        .map(|digit| match digit {
            '0' => '₀',
            '1' => '₁',
            '2' => '₂',
            '3' => '₃',
            '4' => '₄',
            '5' => '₅',
            '6' => '₆',
            '7' => '₇',
            '8' => '₈',
            '9' => '₉',
            other => other,
        })
        .collect()
}

fn polyline_parameter(point_count: usize, segment_index: usize, t: f64) -> Option<f64> {
    if point_count < 2 {
        return None;
    }
    let denominator = point_count.checked_sub(1)? as f64;
    Some((segment_index as f64 + t.clamp(0.0, 1.0)) / denominator)
}

fn point_on_trace_payload_parameter(file: &GspFile, point_group: &ObjectGroup) -> Option<f64> {
    let payload = point_group
        .records
        .iter()
        .find(|record| record.record_type == RECORD_BINDING_PAYLOAD)
        .map(|record| record.payload(&file.data))?;
    let value = read_f64(payload, 4);
    value.is_finite().then_some(value)
}

pub(super) fn circle_parameter(
    anchors: &[Option<PointRecord>],
    center_group_index: usize,
    _radius_group_index: usize,
    unit_x: f64,
    unit_y: f64,
) -> Option<f64> {
    let _center = anchors.get(center_group_index)?.clone()?;
    let point_angle = unit_y.atan2(unit_x);
    let tau = std::f64::consts::TAU;
    Some(point_angle.rem_euclid(tau) / tau)
}

fn polygon_vertex_name(
    file: &GspFile,
    groups: &[ObjectGroup],
    polygon_group: &ObjectGroup,
) -> Option<String> {
    let path = find_indexed_path(file, polygon_group)?;
    let names = path
        .refs
        .iter()
        .map(|&object_ref| {
            let group = groups.get(object_ref.checked_sub(1)?)?;
            decode_label_name(file, group)
        })
        .collect::<Option<Vec<_>>>()?;
    (!names.is_empty()).then(|| names.join(""))
}

pub(super) fn polygon_boundary_parameter(
    anchors: &[Option<PointRecord>],
    vertex_group_indices: &[usize],
    edge_index: usize,
    t: f64,
) -> Option<f64> {
    if vertex_group_indices.len() < 2 {
        return None;
    }

    let vertices = vertex_group_indices
        .iter()
        .map(|group_index| anchors.get(*group_index)?.clone())
        .collect::<Option<Vec<_>>>()?;

    let mut perimeter = 0.0;
    let mut traveled = 0.0;
    for index in 0..vertices.len() {
        let start = &vertices[index];
        let end = &vertices[(index + 1) % vertices.len()];
        let length = ((end.x - start.x).powi(2) + (end.y - start.y).powi(2)).sqrt();
        perimeter += length;
        if index < edge_index % vertices.len() {
            traveled += length;
        } else if index == edge_index % vertices.len() {
            traveled += length * t.clamp(0.0, 1.0);
        }
    }

    (perimeter > 1e-9).then_some(traveled / perimeter)
}
