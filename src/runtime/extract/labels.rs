use std::collections::{BTreeMap, BTreeSet};

use super::CircleShape;
use super::decode::{
    decode_0907_anchor, decode_group_label_text, decode_label_anchor, decode_label_name,
    decode_label_name_raw, decode_link_button_url, decode_measurement_value, decode_text_anchor,
    find_indexed_path, is_action_button_group,
};
use super::points::{
    RawPointConstraint, decode_non_graph_parameter_value_for_group, decode_point_constraint,
    editable_non_graph_parameter_name_for_group, is_editable_non_graph_parameter_name,
    is_non_graph_parameter_group, regular_polygon_angle_expr,
};
use crate::format::{GspFile, ObjectGroup, PointRecord, read_f64, read_u32};
use crate::runtime::functions::{
    decode_function_expr, evaluate_expr_with_parameters, function_expr_label,
};
use crate::runtime::geometry::format_number;
use crate::runtime::scene::{LabelIterationFamily, TextLabel, TextLabelBinding};

pub(super) fn collect_labels(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
    graph_mode: bool,
    include_measurements: bool,
) -> (Vec<TextLabel>, BTreeMap<usize, usize>) {
    let mut labels = Vec::new();
    let mut label_group_to_index = BTreeMap::new();
    for group in groups {
        let kind = group.header.kind();
        match kind {
            crate::format::GroupKind::Point
            | crate::format::GroupKind::Segment
            | crate::format::GroupKind::PointConstraint
            | crate::format::GroupKind::GraphObject40
            | crate::format::GroupKind::Kind51
            | crate::format::GroupKind::ActionButton
            | crate::format::GroupKind::ButtonLabel
            | crate::format::GroupKind::LabelIterationSeed => {
                if kind == crate::format::GroupKind::Point
                    && decode_link_button_url(file, group).is_some()
                {
                    continue;
                }
                if kind == crate::format::GroupKind::ActionButton && is_action_button_group(group) {
                    continue;
                }
                if kind == crate::format::GroupKind::ButtonLabel
                    && find_indexed_path(file, group)
                        .and_then(|path| path.refs.first().copied())
                        .and_then(|ordinal| groups.get(ordinal.checked_sub(1)?))
                        .is_some_and(is_action_button_group)
                {
                    continue;
                }
                if kind == crate::format::GroupKind::LabelIterationSeed {
                    if let Some(label) =
                        collect_point_expression_label(file, groups, group, anchors)
                    {
                        label_group_to_index.insert(group.ordinal, labels.len());
                        labels.push(label);
                    }
                    continue;
                }
                let text = decode_group_label_text(file, group).or_else(|| {
                    (!graph_mode
                        && matches!(
                            kind,
                            crate::format::GroupKind::Point
                                | crate::format::GroupKind::Segment
                                | crate::format::GroupKind::PointConstraint
                        )
                        && !is_non_graph_parameter_group(group))
                    .then(|| decode_label_name(file, group))
                    .flatten()
                });
                if let Some(text) = text {
                    let anchor = decode_label_anchor(file, group, anchors);
                    if let Some(anchor) = anchor {
                        label_group_to_index.insert(group.ordinal, labels.len());
                        labels.push(TextLabel {
                            anchor,
                            text,
                            color: [30, 30, 30, 255],
                            binding: None,
                            screen_space: false,
                        });
                    }
                }
            }
            crate::format::GroupKind::FunctionExpr => {}
            crate::format::GroupKind::GraphCalibrationX
            | crate::format::GroupKind::GraphCalibrationY => {
                if !include_measurements {
                    continue;
                }
                let anchor = anchors
                    .get(group.ordinal.saturating_sub(1))
                    .cloned()
                    .flatten()
                    .or_else(|| {
                        find_indexed_path(file, group).and_then(|path| {
                            path.refs.iter().find_map(|object_ref| {
                                anchors.get(object_ref.saturating_sub(1)).cloned().flatten()
                            })
                        })
                    });
                if anchor
                    .as_ref()
                    .is_some_and(|anchor| anchor.x.abs() < 1e-6 && anchor.y.abs() < 1e-6)
                {
                    continue;
                }
                if let Some(value) = group
                    .records
                    .iter()
                    .find(|record| record.record_type == 0x07d3 && record.length == 12)
                    .and_then(|record| decode_measurement_value(record.payload(&file.data)))
                {
                    if let Some(anchor) = anchor {
                        labels.push(TextLabel {
                            anchor,
                            text: format_number(value),
                            color: [60, 60, 60, 255],
                            binding: None,
                            screen_space: false,
                        });
                    }
                }
            }
            _ => {}
        }
    }
    (labels, label_group_to_index)
}

pub(super) fn collect_coordinate_labels(file: &GspFile, groups: &[ObjectGroup]) -> Vec<TextLabel> {
    let mut labels = Vec::new();
    for group in groups {
        let kind = group.header.kind();
        if kind == crate::format::GroupKind::Point
            && group
                .records
                .iter()
                .any(|record| record.record_type == 0x0907)
            && !group
                .records
                .iter()
                .any(|record| record.record_type == 0x0899)
            && let Some(name) = decode_label_name(file, group)
            && let Some(value) = decode_non_graph_parameter_value_for_group(file, group)
            && let Some(anchor) = decode_0907_anchor(file, group)
        {
            let binding = is_editable_non_graph_parameter_name(&name)
                .then(|| TextLabelBinding::ParameterValue { name: name.clone() });
            labels.push(TextLabel {
                anchor,
                text: format!("{name} = {:.2}", value),
                color: [30, 30, 30, 255],
                binding,
                screen_space: true,
            });
        } else if kind == crate::format::GroupKind::FunctionExpr
            && let Some(expr) = decode_function_expr(file, groups, group)
            && let Some(path) = find_indexed_path(file, group)
            && let Some(parameter_ref) = path.refs.first().copied()
            && let Some(parameter_group) = parameter_ref
                .checked_sub(1)
                .and_then(|index| groups.get(index))
            && let Some(parameter_name) =
                editable_non_graph_parameter_name_for_group(file, parameter_group)
            && let Some(parameter_value) =
                decode_non_graph_parameter_value_for_group(file, parameter_group)
            && let Some(anchor) = decode_0907_anchor(file, group)
        {
            let Some(value) = evaluate_expr_with_parameters(
                &expr,
                0.0,
                &BTreeMap::from([(parameter_name.clone(), parameter_value)]),
            ) else {
                continue;
            };
            let (_expr_label, binding, text) =
                if parameter_name == "n" && function_expr_label(expr.clone()) == "257 / n" {
                    let angle = 360.0 / parameter_value;
                    let angle_expr = regular_polygon_angle_expr(&parameter_name, parameter_value);
                    (
                        "360° / n".to_string(),
                        Some(TextLabelBinding::ExpressionValue {
                            parameter_name: parameter_name.clone(),
                            expr_label: "360° / n".to_string(),
                            expr: angle_expr,
                        }),
                        format!("360°\n——— = {:.2}°\n  n", angle),
                    )
                } else {
                    let expr_label = function_expr_label(expr.clone());
                    (
                        expr_label.clone(),
                        is_editable_non_graph_parameter_name(&parameter_name).then(|| {
                            TextLabelBinding::ExpressionValue {
                                parameter_name: parameter_name.clone(),
                                expr_label: expr_label.clone(),
                                expr: expr.clone(),
                            }
                        }),
                        format!("{expr_label} = {:.2}", value),
                    )
                };
            labels.push(TextLabel {
                anchor,
                text,
                color: [30, 30, 30, 255],
                binding,
                screen_space: true,
            });
        }
    }
    labels
}

fn collect_point_expression_label(
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
    let expr_group = groups.get(path.refs[1].checked_sub(1)?)?;
    if (expr_group.header.kind()) != crate::format::GroupKind::FunctionExpr {
        return None;
    }
    let expr = decode_function_expr(file, groups, expr_group)?;
    let expr_path = find_indexed_path(file, expr_group)?;
    let parameter_group = groups.get(expr_path.refs.first()?.checked_sub(1)?)?;
    let parameter_name = editable_non_graph_parameter_name_for_group(file, parameter_group)?;
    let parameter_value = decode_non_graph_parameter_value_for_group(file, parameter_group)?;
    let value = evaluate_expr_with_parameters(
        &expr,
        0.0,
        &BTreeMap::from([(parameter_name.clone(), parameter_value)]),
    )?;
    let anchor = decode_label_anchor(file, group, anchors)?;
    Some(TextLabel {
        anchor,
        text: format_number(value),
        color: [30, 30, 30, 255],
        binding: Some(TextLabelBinding::PointExpressionValue {
            point_index: point_group_index,
            parameter_name,
            expr,
        }),
        screen_space: false,
    })
}

pub(super) fn collect_polygon_parameter_labels(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
) -> Vec<TextLabel> {
    groups
        .iter()
        .filter(|group| (group.header.kind()) == crate::format::GroupKind::ParameterAnchor)
        .filter_map(|group| {
            let path = find_indexed_path(file, group)?;
            if path.refs.len() < 2 {
                return None;
            }

            let point_group = groups.get(path.refs[0].checked_sub(1)?)?;
            let polygon_group = groups.get(path.refs[1].checked_sub(1)?)?;
            if (point_group.header.kind()) != crate::format::GroupKind::PointConstraint
                || (polygon_group.header.kind()) != crate::format::GroupKind::Polygon
            {
                return None;
            }

            let point_name = decode_label_name(file, point_group)?;
            let polygon_name = polygon_vertex_name(file, groups, polygon_group)?;
            let anchor = group
                .records
                .iter()
                .find(|record| record.record_type == 0x0903)
                .and_then(|record| decode_text_anchor(record.payload(&file.data)))?;
            let RawPointConstraint::PolygonBoundary {
                vertex_group_indices,
                edge_index,
                t,
            } = decode_point_constraint(file, groups, point_group, &None)?
            else {
                return None;
            };
            let global_t =
                polygon_boundary_parameter(anchors, &vertex_group_indices, edge_index, t)?;

            Some(TextLabel {
                anchor,
                text: format!("{point_name}在{polygon_name}上的t值 = {:.2}", global_t),
                color: [30, 30, 30, 255],
                binding: Some(TextLabelBinding::PolygonBoundaryParameter {
                    point_index: path.refs[0].checked_sub(1)?,
                    point_name,
                    polygon_name,
                }),
                screen_space: false,
            })
        })
        .collect()
}

pub(super) fn collect_segment_parameter_labels(
    file: &GspFile,
    groups: &[ObjectGroup],
) -> Vec<TextLabel> {
    groups
        .iter()
        .filter(|group| (group.header.kind()) == crate::format::GroupKind::ParameterAnchor)
        .filter_map(|group| {
            let path = find_indexed_path(file, group)?;
            if path.refs.len() < 2 {
                return None;
            }

            let point_group = groups.get(path.refs[0].checked_sub(1)?)?;
            let segment_group = groups.get(path.refs[1].checked_sub(1)?)?;
            if (point_group.header.kind()) != crate::format::GroupKind::PointConstraint
                || (segment_group.header.kind()) != crate::format::GroupKind::Segment
            {
                return None;
            }

            let point_name = decode_label_name(file, point_group)?;
            let segment_name = segment_name(file, groups, segment_group)?;
            let anchor = group
                .records
                .iter()
                .find(|record| record.record_type == 0x0903)
                .and_then(|record| decode_text_anchor(record.payload(&file.data)))?;
            let RawPointConstraint::Segment(constraint) =
                decode_point_constraint(file, groups, point_group, &None)?
            else {
                return None;
            };

            Some(TextLabel {
                anchor,
                text: format!("{point_name}在{segment_name}上的t值 = {:.2}", constraint.t),
                color: [30, 30, 30, 255],
                binding: Some(TextLabelBinding::SegmentParameter {
                    point_index: path.refs[0].checked_sub(1)?,
                    point_name,
                    segment_name,
                }),
                screen_space: false,
            })
        })
        .collect()
}

pub(super) fn collect_circle_parameter_labels(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
) -> Vec<TextLabel> {
    groups
        .iter()
        .filter(|group| (group.header.kind()) == crate::format::GroupKind::ParameterAnchor)
        .filter_map(|group| {
            let path = find_indexed_path(file, group)?;
            if path.refs.len() < 2 {
                return None;
            }

            let point_group = groups.get(path.refs[0].checked_sub(1)?)?;
            let circle_group = groups.get(path.refs[1].checked_sub(1)?)?;
            if (point_group.header.kind()) != crate::format::GroupKind::PointConstraint
                || (circle_group.header.kind()) != crate::format::GroupKind::Circle
            {
                return None;
            }

            let point_name = decode_label_name(file, point_group)?;
            let circle_name = circle_name(file, groups, circle_group)?;
            let anchor = group
                .records
                .iter()
                .find(|record| record.record_type == 0x0903)
                .and_then(|record| decode_text_anchor(record.payload(&file.data)))?;
            let RawPointConstraint::Circle(constraint) =
                decode_point_constraint(file, groups, point_group, &None)?
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
                text: format!("{point_name}在⊙{circle_name}上的值 = {:.2}", value),
                color: [30, 30, 30, 255],
                binding: Some(TextLabelBinding::CircleParameter {
                    point_index: path.refs[0].checked_sub(1)?,
                    point_name,
                    circle_name,
                }),
                screen_space: false,
            })
        })
        .collect()
}

pub(super) fn collect_label_iterations(
    file: &GspFile,
    groups: &[ObjectGroup],
    label_group_to_index: &BTreeMap<usize, usize>,
    group_to_point_index: &[Option<usize>],
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
            let point_seed_index = group_to_point_index
                .get(point_group_index)
                .and_then(|mapped_index| *mapped_index)?;
            let expr_group = groups.get(seed_path.refs.get(1)?.checked_sub(1)?)?;
            if (expr_group.header.kind()) != crate::format::GroupKind::FunctionExpr {
                return None;
            }
            let expr = decode_function_expr(file, groups, expr_group)?;
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
            let depth_parameter_name = find_indexed_path(file, iter_group)
                .and_then(|iter_path| iter_path.refs.first().copied())
                .and_then(|ordinal| groups.get(ordinal.checked_sub(1)?))
                .and_then(|parameter_group| {
                    editable_non_graph_parameter_name_for_group(file, parameter_group)
                });

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
            groups
                .get(object_ref.checked_sub(1)?)
                .and_then(|group| decode_label_name(file, group))
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
            groups
                .get(object_ref.checked_sub(1)?)
                .and_then(|group| decode_label_name(file, group))
        })
        .collect::<Option<Vec<_>>>()?;
    (names.len() >= 2).then(|| names.join(""))
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
            groups
                .get(object_ref.checked_sub(1)?)
                .and_then(|group| decode_label_name(file, group))
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

pub(super) fn compute_iteration_labels(
    file: &GspFile,
    groups: &[ObjectGroup],
    circles: &[CircleShape],
) -> Vec<TextLabel> {
    let mut labels = Vec::new();

    let has_iteration = groups
        .iter()
        .any(|group| (group.header.kind()) == crate::format::GroupKind::RegularPolygonIteration);
    if !has_iteration {
        return labels;
    }

    let Some(circle) = circles.first() else {
        return labels;
    };
    let cx = circle.center.x;
    let cy = circle.center.y;
    let radius =
        ((circle.radius_point.x - cx).powi(2) + (circle.radius_point.y - cy).powi(2)).sqrt();

    let px_per_cm = groups
        .iter()
        .filter(|group| (group.header.kind()) == crate::format::GroupKind::PolarOffsetPoint)
        .find_map(|group| {
            let payload = group
                .records
                .iter()
                .find(|record| record.record_type == 0x07d3)
                .map(|record| record.payload(&file.data))?;
            (payload.len() >= 40).then(|| read_f64(payload, 32))
        })
        .filter(|v| v.is_finite() && *v > 1.0)
        .unwrap_or(37.79527559055118);

    let param_value = groups
        .iter()
        .filter(|group| (group.header.kind()) == crate::format::GroupKind::PolarOffsetPoint)
        .find_map(|group| {
            let payload = group
                .records
                .iter()
                .find(|record| record.record_type == 0x07d3)
                .map(|record| record.payload(&file.data))?;
            (payload.len() >= 20).then(|| read_f64(payload, 12))
        })
        .filter(|v| v.is_finite() && *v > 0.0)
        .unwrap_or(1.0);

    let t1 = param_value;
    let side = t1 / 2.0 * px_per_cm;
    let sqrt3 = 3.0_f64.sqrt();
    let diameter = 2.0 * radius;
    let m1 = diameter / (2.0 * side) + 0.5;
    let l_val = m1.floor() + 1.0;
    let m2 = diameter / (sqrt3 * side);
    let h_val = m2.ceil();
    let m3 = m2 - m1;
    let m4 = m3 - m3.floor();

    fn format_sub(raw: &str) -> String {
        raw.replace("[1]", "\u{2081}")
            .replace("[2]", "\u{2082}")
            .replace("[3]", "\u{2083}")
            .replace("[4]", "\u{2084}")
    }

    let mut computed_values = BTreeMap::<String, f64>::new();
    computed_values.insert("m\u{2081}".to_string(), m1);
    computed_values.insert("m\u{2082}".to_string(), m2);
    computed_values.insert("m\u{2083}".to_string(), m3);
    computed_values.insert("m\u{2084}".to_string(), m4);
    computed_values.insert("L".to_string(), l_val);
    computed_values.insert("H".to_string(), h_val);
    computed_values.insert("H\u{00b7}L".to_string(), h_val * l_val);

    for group in groups {
        if let Some(raw_name) = decode_label_name_raw(file, group) {
            let name = format_sub(&raw_name);
            if group
                .records
                .iter()
                .any(|record| record.record_type == 0x0907)
                && (group.header.kind()) == crate::format::GroupKind::Point
                && !computed_values.contains_key(&name)
            {
                computed_values.insert(name, t1);
            }
        }
    }

    for group in groups {
        let kind = group.header.kind();
        let has_0907 = group
            .records
            .iter()
            .any(|record| record.record_type == 0x0907);
        if !has_0907
            || !matches!(
                kind,
                crate::format::GroupKind::Point | crate::format::GroupKind::FunctionExpr
            )
        {
            continue;
        }
        if group
            .records
            .iter()
            .any(|record| record.record_type == 0x08fc)
        {
            continue;
        }

        let Some(anchor) = decode_0907_anchor(file, group) else {
            continue;
        };

        let own_label = decode_label_name_raw(file, group).map(|s| format_sub(&s));
        let ref_labels: Vec<String> = find_indexed_path(file, group)
            .map(|path| {
                path.refs
                    .iter()
                    .filter_map(|&obj_ref| {
                        let ref_group = groups.get(obj_ref.checked_sub(1)?)?;
                        decode_label_name_raw(file, ref_group).map(|s| format_sub(&s))
                    })
                    .collect()
            })
            .unwrap_or_default();

        let mut lines = Vec::new();

        if kind == crate::format::GroupKind::Point {
            if let Some(name) = &own_label
                && let Some(&val) = computed_values.get(name.as_str())
            {
                let unit = "\u{5398}\u{7c73}";
                lines.push(format!("{name} = {val:.0} {unit}"));
                lines.push(format!("{name}/2 = {:.2} {unit}", val / 2.0));
            }
        } else {
            let has_h = ref_labels.iter().any(|n| n == "H");
            let has_l = ref_labels.iter().any(|n| n == "L");
            if own_label.is_none() && has_h && has_l {
                if let Some(val) = computed_values.get("H\u{00b7}L") {
                    lines.push(format!("H\u{00b7}L = {val:.2}"));
                }
            } else {
                let mut seen = BTreeSet::new();
                let mut try_add = |name: &str, lines: &mut Vec<String>| {
                    if seen.contains(name) {
                        return;
                    }
                    seen.insert(name.to_string());
                    if let Some(val) = computed_values.get(name) {
                        lines.push(format!("{name} = {val:.2}"));
                    }
                };

                if let Some(ol) = &own_label {
                    try_add(ol, &mut lines);
                }
                for rl in &ref_labels {
                    try_add(rl, &mut lines);
                }
            }

            if lines.is_empty()
                && let Some(ol) = &own_label
            {
                lines.push(ol.clone());
            }
        }

        if !lines.is_empty() {
            labels.push(TextLabel {
                anchor,
                text: lines.join("\n"),
                color: [30, 30, 30, 255],
                binding: None,
                screen_space: false,
            });
        }
    }

    labels
}
