use std::collections::{BTreeMap, BTreeSet};

use super::super::decode::{
    circle_center_radius_value, decode_label_name, find_indexed_path, is_parameter_control_group,
    try_decode_parameter_control_value_for_group, try_decode_payload_anchor_point,
};
use super::constraints::{
    RawPointConstraint, decode_translated_point_constraint, regular_polygon_iteration_step,
    try_decode_parameter_controlled_point, try_decode_point_constraint,
};
use super::{
    GspFile, ObjectGroup, PointRecord, TransformBindingKind,
    decode_angle_parameter_value_for_group, decode_non_graph_parameter_value_for_group,
    editable_non_graph_parameter_name_for_group, read_f64, try_decode_angle_rotation_binding,
    try_decode_parameter_rotation_binding, try_decode_transform_binding,
};
use crate::format::GroupKind;
use crate::format::read_u32;
use crate::runtime::functions::{
    BinaryOp, FunctionAst, FunctionExpr, evaluate_expr_with_parameters, function_expr_ast,
    try_decode_function_expr, try_decode_function_expr_with_inlined_refs,
    try_decode_function_plot_descriptor,
};
use crate::runtime::geometry::{
    GraphTransform, angle_degrees_from_points, lerp_point, point_on_circle_arc,
    point_on_three_point_arc, reflect_across_line, rotate_around, scale_around,
    three_point_arc_geometry, to_raw_from_world, to_world,
};
use crate::runtime::scene::LineLikeKind;

mod geometry;

use self::geometry::{
    CircularConstraintRaw, distinct_pair, line_line_intersection, line_polyline_intersection,
    normalize_angle_delta_raw, resolve_polyline_point, select_circular_intersection,
    select_line_circle_intersection, select_point_circle_tangent,
};

const PX_PER_CM: f64 = crate::runtime::DEFAULT_GRAPH_RAW_PER_UNIT;

fn first_path_group<'a>(
    file: &GspFile,
    groups: &'a [ObjectGroup],
    group: &ObjectGroup,
) -> Option<&'a ObjectGroup> {
    let path = find_indexed_path(file, group)?;
    let ordinal = path.refs.first().copied()?;
    let index = ordinal.checked_sub(1)?;
    groups.get(index)
}

#[derive(Clone)]
pub(crate) struct CustomTransformBindingDef {
    pub(crate) source_group_index: usize,
    pub(crate) origin_group_index: usize,
    pub(crate) axis_end_group_index: usize,
    pub(crate) distance_expr: crate::runtime::functions::FunctionExpr,
    pub(crate) angle_expr: crate::runtime::functions::FunctionExpr,
    pub(crate) distance_raw_scale: f64,
    pub(crate) angle_degrees_scale: f64,
}

#[derive(Clone)]
pub(crate) struct ExpressionRotationBindingDef {
    pub(crate) source_group_index: usize,
    pub(crate) center_group_index: usize,
    pub(crate) angle_expr: FunctionExpr,
    pub(crate) angle_degrees: f64,
    pub(crate) parameter_name: Option<String>,
}

#[derive(Clone)]
pub(crate) struct ExpressionScaleBindingDef {
    pub(crate) source_group_index: usize,
    pub(crate) center_group_index: usize,
    pub(crate) factor_expr: FunctionExpr,
    pub(crate) factor: f64,
    pub(crate) parameter_name: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct ExpressionRatioScaleBindingDef {
    pub(crate) source_group_index: usize,
    pub(crate) center_group_index: usize,
    pub(crate) ratio_origin_group_index: usize,
    pub(crate) ratio_denominator_group_index: usize,
    pub(crate) ratio_numerator_group_index: usize,
    pub(crate) factor: f64,
}

#[derive(Clone)]
pub(crate) struct ExpressionOffsetBindingDef {
    pub(crate) source_group_index: usize,
    pub(crate) scaled_expr: FunctionExpr,
    pub(crate) parameter_name: Option<String>,
}

#[derive(Clone)]
pub(crate) enum IterationBindingPointAliasKind {
    Offset {
        dx: f64,
        dy: f64,
    },
    Rotate {
        center_group_index: usize,
        angle_degrees: f64,
    },
}

#[derive(Clone)]
pub(crate) struct IterationBindingPointAliasRaw {
    pub(crate) position: PointRecord,
    pub(crate) source_group_index: usize,
    pub(crate) kind: IterationBindingPointAliasKind,
}

fn parameter_anchor_runtime_value(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
    anchors: &[Option<PointRecord>],
) -> Option<(String, f64)> {
    let path = find_indexed_path(file, group)?;
    let point_group_index = path.refs.first()?.checked_sub(1)?;
    let point_group = groups.get(point_group_index)?;
    let value = if let Ok(constraint) =
        try_decode_point_constraint(file, groups, point_group, Some(anchors), &None)
    {
        match constraint {
            RawPointConstraint::Segment(constraint) => constraint.t,
            RawPointConstraint::ConstructedLine { t, .. } => t,
            RawPointConstraint::PolygonBoundary {
                edge_index,
                t,
                vertex_group_indices,
            } => super::super::labels::polygon_boundary_parameter(
                anchors,
                &vertex_group_indices,
                edge_index,
                t,
            )?,
            RawPointConstraint::TranslatedPolygonBoundary {
                edge_index,
                t,
                vertex_group_indices,
                ..
            } => super::super::labels::polygon_boundary_parameter(
                anchors,
                &vertex_group_indices,
                edge_index,
                t,
            )?,
            RawPointConstraint::Circle(constraint) => super::super::labels::circle_parameter(
                anchors,
                constraint.center_group_index,
                constraint.radius_group_index,
                constraint.unit_x,
                constraint.unit_y,
            )?,
            RawPointConstraint::Circular(_)
            | RawPointConstraint::CircleArc(_)
            | RawPointConstraint::Arc(_)
            | RawPointConstraint::Polyline { .. } => return None,
        }
    } else {
        let host_group_index = path.refs.get(1)?.checked_sub(1)?;
        let host_group = groups.get(host_group_index)?;
        if !host_group.header.kind().is_line_like() {
            return None;
        }
        let host_path = find_indexed_path(file, host_group)?;
        let start = anchors
            .get(host_path.refs.first()?.checked_sub(1)?)?
            .as_ref()?;
        let end = anchors
            .get(host_path.refs.get(1)?.checked_sub(1)?)?
            .as_ref()?;
        let point = anchors.get(point_group_index)?.as_ref()?;
        let dx = end.x - start.x;
        let dy = end.y - start.y;
        let len_sq = dx * dx + dy * dy;
        if len_sq <= 1e-9 {
            return None;
        }
        (((point.x - start.x) * dx + (point.y - start.y) * dy) / len_sq).clamp(0.0, 1.0)
    };
    let name = decode_label_name(file, group)
        .or_else(|| decode_label_name(file, point_group))
        .or_else(|| editable_non_graph_parameter_name_for_group(file, groups, point_group))?;
    Some((name, value))
}

fn ratio_value_runtime_value(
    file: &GspFile,
    _groups: &[ObjectGroup],
    group: &ObjectGroup,
    anchors: &[Option<PointRecord>],
) -> Option<(String, f64)> {
    if group.header.kind() != GroupKind::RatioValue {
        return None;
    }
    let path = find_indexed_path(file, group)?;
    let origin_index = path.refs.first()?.checked_sub(1)?;
    let denominator_index = path.refs.get(1)?.checked_sub(1)?;
    let numerator_index = path.refs.get(2)?.checked_sub(1)?;
    let origin = anchors.get(origin_index)?.as_ref()?;
    let denominator = anchors.get(denominator_index)?.as_ref()?;
    let numerator = anchors.get(numerator_index)?.as_ref()?;
    let denominator_length = (denominator.x - origin.x).hypot(denominator.y - origin.y);
    if denominator_length <= 1e-9 {
        return None;
    }
    let numerator_length = (numerator.x - origin.x).hypot(numerator.y - origin.y);
    let name = decode_label_name(file, group)?;
    Some((name, numerator_length / denominator_length))
}

fn ratio_scale_binding_from_value_group(
    file: &GspFile,
    source_group_index: usize,
    center_group_index: usize,
    group: &ObjectGroup,
    anchors: &[Option<PointRecord>],
) -> Option<ExpressionRatioScaleBindingDef> {
    if group.header.kind() != GroupKind::RatioValue {
        return None;
    }
    let path = find_indexed_path(file, group)?;
    let ratio_origin_group_index = path.refs.first()?.checked_sub(1)?;
    let ratio_denominator_group_index = path.refs.get(1)?.checked_sub(1)?;
    let ratio_numerator_group_index = path.refs.get(2)?.checked_sub(1)?;
    let ratio_origin = anchors.get(ratio_origin_group_index)?.as_ref()?;
    let ratio_denominator = anchors.get(ratio_denominator_group_index)?.as_ref()?;
    let ratio_numerator = anchors.get(ratio_numerator_group_index)?.as_ref()?;
    let denominator_dx = ratio_denominator.x - ratio_origin.x;
    let denominator_dy = ratio_denominator.y - ratio_origin.y;
    let numerator_dx = ratio_numerator.x - ratio_origin.x;
    let numerator_dy = ratio_numerator.y - ratio_origin.y;
    let denominator = denominator_dx.hypot(denominator_dy);
    if denominator <= 1e-9 {
        return None;
    }
    let numerator = numerator_dx.hypot(numerator_dy).min(denominator);
    Some(ExpressionRatioScaleBindingDef {
        source_group_index,
        center_group_index,
        ratio_origin_group_index,
        ratio_denominator_group_index,
        ratio_numerator_group_index,
        factor: numerator / denominator,
    })
}

fn collect_expr_runtime_parameters(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
    anchors: &[Option<PointRecord>],
    parameters: &mut BTreeMap<String, f64>,
    visiting: &mut BTreeSet<usize>,
) {
    if !visiting.insert(group.ordinal) {
        return;
    }
    if let Some(path) = find_indexed_path(file, group) {
        for ordinal in path.refs {
            let Some(candidate) = groups.get(ordinal.saturating_sub(1)) else {
                continue;
            };
            match candidate.header.kind() {
                GroupKind::FunctionExpr => {
                    collect_expr_runtime_parameters(
                        file, groups, candidate, anchors, parameters, visiting,
                    );
                }
                GroupKind::ParameterAnchor => {
                    if let Some((name, value)) =
                        parameter_anchor_runtime_value(file, groups, candidate, anchors)
                    {
                        parameters.insert(name, value);
                    }
                }
                GroupKind::RatioValue => {
                    if let Some((name, value)) =
                        ratio_value_runtime_value(file, groups, candidate, anchors)
                    {
                        parameters.insert(name, value);
                    }
                }
                GroupKind::DistanceValue => {
                    if let Some((name, value)) =
                        distance_value_runtime_value(file, groups, candidate, anchors)
                    {
                        parameters.insert(name, value);
                    }
                }
                GroupKind::Point if is_parameter_control_group(candidate) => {
                    let Some(name) =
                        editable_non_graph_parameter_name_for_group(file, groups, candidate)
                            .or_else(|| decode_label_name(file, candidate))
                    else {
                        continue;
                    };
                    let Some(value) =
                        try_decode_parameter_control_value_for_group(file, groups, candidate).ok()
                    else {
                        continue;
                    };
                    parameters.insert(name, value);
                }
                _ => {}
            }
        }
    }
    visiting.remove(&group.ordinal);
}

fn distance_value_runtime_value(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
    anchors: &[Option<PointRecord>],
) -> Option<(String, f64)> {
    if group.header.kind() != GroupKind::DistanceValue {
        return None;
    }
    let path = find_indexed_path(file, group)?;
    if path.refs.len() < 2 {
        return None;
    }
    let left_group_index = path.refs[0].checked_sub(1)?;
    let right_group_index = path.refs[1].checked_sub(1)?;
    let left = anchors.get(left_group_index)?.as_ref()?;
    let right = anchors.get(right_group_index)?.as_ref()?;
    let name = decode_label_name(file, group).unwrap_or_else(|| {
        let left_name = groups
            .get(left_group_index)
            .and_then(|group| decode_label_name(file, group))
            .unwrap_or_else(|| "P".to_string());
        let right_name = groups
            .get(right_group_index)
            .and_then(|group| decode_label_name(file, group))
            .unwrap_or_else(|| "Q".to_string());
        format!("{left_name}{right_name}")
    });
    Some((name, (right.x - left.x).hypot(right.y - left.y)))
}

pub(crate) fn expression_runtime_context(
    file: &GspFile,
    groups: &[ObjectGroup],
    expr_group: &ObjectGroup,
    anchors: &[Option<PointRecord>],
) -> Option<(FunctionExpr, BTreeMap<String, f64>, Option<String>)> {
    if expr_group.header.kind() != GroupKind::FunctionExpr {
        return None;
    }
    let expr = try_decode_function_expr(file, groups, expr_group).ok()?;
    let mut parameters = BTreeMap::new();
    collect_expr_runtime_parameters(
        file,
        groups,
        expr_group,
        anchors,
        &mut parameters,
        &mut BTreeSet::new(),
    );
    let parameter_name = parameters.keys().next().cloned();
    Some((expr, parameters, parameter_name))
}

fn scale_function_expr(expr: FunctionExpr, factor: f64) -> FunctionExpr {
    if (factor - 1.0).abs() <= 1e-9 {
        return expr;
    }
    match expr {
        FunctionExpr::Constant(value) => FunctionExpr::Constant(value * factor),
        other => FunctionExpr::Parsed(FunctionAst::Binary {
            lhs: Box::new(FunctionAst::Constant(factor)),
            op: BinaryOp::Mul,
            rhs: Box::new(function_expr_ast(other)),
        }),
    }
}

pub(crate) fn decode_expression_rotation_binding(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
    anchors: &[Option<PointRecord>],
) -> Option<ExpressionRotationBindingDef> {
    if group.header.kind() != GroupKind::ExpressionRotation {
        return None;
    }
    let path = find_indexed_path(file, group)?;
    if path.refs.len() < 3 {
        return None;
    }
    let source_group_index = path.refs[0].checked_sub(1)?;
    let center_group_index = path.refs[1].checked_sub(1)?;
    let expr_group = groups.get(path.refs[2].checked_sub(1)?)?;
    let (angle_expr, angle_degrees, parameter_name) = if expr_group.header.kind()
        == GroupKind::FunctionExpr
    {
        let (angle_expr, parameters, parameter_name) =
            expression_runtime_context(file, groups, expr_group, anchors)?;
        let angle_is_degrees =
            decode_label_name(file, expr_group).is_some_and(|label| label.contains('°'));
        let angle_expr = if angle_is_degrees {
            angle_expr
        } else {
            scale_function_expr(angle_expr, 180.0 / std::f64::consts::PI)
        };
        let angle_degrees = evaluate_expr_with_parameters(&angle_expr, 0.0, &parameters)?;
        (angle_expr, angle_degrees, parameter_name)
    } else if expr_group.header.kind() == GroupKind::Point {
        let parameter_name = editable_non_graph_parameter_name_for_group(file, groups, expr_group)
            .or_else(|| decode_label_name(file, expr_group));
        let angle_expr = try_decode_function_expr(file, groups, expr_group)
            .ok()
            .or_else(|| {
                let angle_value = decode_angle_parameter_value_for_group(file, expr_group)
                    .or_else(|| {
                        let control = try_decode_payload_anchor_point(file, expr_group)
                            .ok()
                            .flatten()?;
                        Some((-control.y).atan2(control.x).to_degrees())
                    })
                    .or_else(|| {
                        let value =
                            try_decode_parameter_control_value_for_group(file, groups, expr_group)
                                .ok()?;
                        value.is_finite().then_some(value)
                    })?;
                Some(FunctionExpr::Constant(angle_value))
            })?;
        let angle_degrees = evaluate_expr_with_parameters(&angle_expr, 0.0, &BTreeMap::new())?;
        let angle_expr =
            if let (Some(name), FunctionExpr::Constant(value)) = (&parameter_name, &angle_expr) {
                FunctionExpr::Parsed(FunctionAst::Parameter(name.clone(), *value))
            } else {
                angle_expr
            };
        (angle_expr, angle_degrees, parameter_name)
    } else if expr_group.header.kind() == GroupKind::ParameterAnchor {
        let (_name, angle_radians) =
            parameter_anchor_runtime_value(file, groups, expr_group, anchors)?;
        (
            FunctionExpr::Constant(angle_radians.to_degrees()),
            angle_radians.to_degrees(),
            None,
        )
    } else {
        return None;
    };
    Some(ExpressionRotationBindingDef {
        source_group_index,
        center_group_index,
        angle_expr,
        angle_degrees,
        parameter_name,
    })
}

pub(crate) fn decode_expression_scale_binding(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
    anchors: &[Option<PointRecord>],
) -> Option<ExpressionScaleBindingDef> {
    if group.header.kind() != GroupKind::ExpressionRotation {
        return None;
    }
    let path = find_indexed_path(file, group)?;
    if path.refs.len() < 3 {
        return None;
    }
    let source_group_index = path.refs[0].checked_sub(1)?;
    let center_group_index = path.refs[1].checked_sub(1)?;
    let expr_group = groups.get(path.refs[2].checked_sub(1)?)?;
    let (mut factor_expr, mut factor, parameter_name) = if expr_group.header.kind()
        == GroupKind::FunctionExpr
    {
        if decode_label_name(file, expr_group).is_some_and(|label| label.contains('°')) {
            return None;
        }
        let (factor_expr, parameters, parameter_name) =
            expression_runtime_context(file, groups, expr_group, anchors)?;
        let factor = evaluate_expr_with_parameters(&factor_expr, 0.0, &parameters)?;
        (factor_expr, factor, parameter_name)
    } else if expr_group.header.kind() == GroupKind::Point
        && decode_angle_parameter_value_for_group(file, expr_group).is_none()
    {
        let parameter_name = editable_non_graph_parameter_name_for_group(file, groups, expr_group)
            .or_else(|| decode_label_name(file, expr_group));
        let decoded_expr = try_decode_function_expr(file, groups, expr_group).ok();
        let factor = decoded_expr
            .as_ref()
            .and_then(|expr| evaluate_expr_with_parameters(expr, 0.0, &BTreeMap::new()))
            .filter(|value| value.is_finite())
            .or_else(|| {
                try_decode_parameter_control_value_for_group(file, groups, expr_group).ok()
            })?;
        let factor_expr = decoded_expr.unwrap_or_else(|| {
            parameter_name
                .as_ref()
                .map(|name| FunctionExpr::Parsed(FunctionAst::Parameter(name.clone(), factor)))
                .unwrap_or(FunctionExpr::Constant(factor))
        });
        (factor_expr, factor, parameter_name)
    } else {
        return None;
    };
    if groups
        .get(source_group_index)
        .and_then(|source_group| decode_label_name(file, source_group))
        .is_some_and(|label| matches!(label.as_str(), "x" | "y" | "z"))
    {
        factor = -factor;
        factor_expr = scale_function_expr(factor_expr, -1.0);
    }
    Some(ExpressionScaleBindingDef {
        source_group_index,
        center_group_index,
        factor_expr,
        factor,
        parameter_name,
    })
}

pub(crate) fn decode_expression_ratio_scale_binding(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
    anchors: &[Option<PointRecord>],
) -> Option<ExpressionRatioScaleBindingDef> {
    if group.header.kind() != GroupKind::ExpressionRotation {
        return None;
    }
    let path = find_indexed_path(file, group)?;
    if path.refs.len() < 3 {
        return None;
    }
    let source_group_index = path.refs[0].checked_sub(1)?;
    let center_group_index = path.refs[1].checked_sub(1)?;
    let ratio_group = groups.get(path.refs[2].checked_sub(1)?)?;
    ratio_scale_binding_from_value_group(
        file,
        source_group_index,
        center_group_index,
        ratio_group,
        anchors,
    )
}

pub(crate) fn decode_expression_rotation_anchor_raw(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
    anchors: &[Option<PointRecord>],
) -> Option<PointRecord> {
    if let Some(binding) = decode_expression_ratio_scale_binding(file, groups, group, anchors) {
        let source = anchors.get(binding.source_group_index)?.clone()?;
        let center = anchors.get(binding.center_group_index)?.clone()?;
        return Some(scale_around(&source, &center, binding.factor));
    }
    if let Some(binding) = decode_expression_scale_binding(file, groups, group, anchors) {
        let source = anchors.get(binding.source_group_index)?.clone()?;
        let center = anchors.get(binding.center_group_index)?.clone()?;
        return Some(scale_around(&source, &center, binding.factor));
    }
    let binding = decode_expression_rotation_binding(file, groups, group, anchors)?;
    let source = anchors.get(binding.source_group_index)?.clone()?;
    let center = anchors.get(binding.center_group_index)?.clone()?;
    Some(rotate_around(
        &source,
        &center,
        binding.angle_degrees.to_radians(),
    ))
}

pub(crate) fn decode_iteration_binding_point_alias_raw(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
    anchors: &[Option<PointRecord>],
) -> Option<IterationBindingPointAliasRaw> {
    if group.header.kind() != GroupKind::IterationPointAlias {
        return None;
    }
    let path = find_indexed_path(file, group)?;
    let binding_group = groups.get(path.refs.first()?.checked_sub(1)?)?;
    if binding_group.header.kind() != GroupKind::IterationBinding {
        return None;
    }
    let binding_path = find_indexed_path(file, binding_group)?;
    if binding_path.refs.len() < 2 {
        return None;
    }
    let source_group_index = binding_path.refs[0].checked_sub(1)?;
    let iter_group = groups.get(binding_path.refs[1].checked_sub(1)?)?;
    let source_position = anchors.get(source_group_index)?.clone()?;
    let depth = iteration_depth_raw(file, iter_group, 3);
    if depth == 0 {
        return None;
    }

    match iter_group.header.kind() {
        GroupKind::AffineIteration => {
            let seed_group = groups.get(source_group_index)?;
            if matches!(
                seed_group.header.kind(),
                GroupKind::ParameterRotation | GroupKind::Rotation
            ) {
                let binding = if seed_group.header.kind() == GroupKind::ParameterRotation {
                    try_decode_parameter_rotation_binding(file, groups, seed_group).ok()
                } else {
                    try_decode_transform_binding(file, seed_group).ok()
                }?;
                let TransformBindingKind::Rotate { angle_degrees, .. } = binding.kind else {
                    return None;
                };
                let center_position = anchors.get(binding.center_group_index)?.clone()?;
                let total_angle = angle_degrees * depth as f64;
                return Some(IterationBindingPointAliasRaw {
                    position: rotate_around(
                        &source_position,
                        &center_position,
                        total_angle.to_radians(),
                    ),
                    source_group_index,
                    kind: IterationBindingPointAliasKind::Rotate {
                        center_group_index: binding.center_group_index,
                        angle_degrees: total_angle,
                    },
                });
            }

            let iter_path = find_indexed_path(file, iter_group)?;
            if iter_path.refs.len() < 2 {
                return None;
            }
            let base_start = anchors.get(iter_path.refs[0].checked_sub(1)?)?.clone()?;
            let base_end = anchors.get(iter_path.refs[1].checked_sub(1)?)?.clone()?;
            let dx = (base_end.x - base_start.x) * depth as f64;
            let dy = (base_end.y - base_start.y) * depth as f64;
            Some(IterationBindingPointAliasRaw {
                position: PointRecord {
                    x: source_position.x + dx,
                    y: source_position.y + dy,
                },
                source_group_index,
                kind: IterationBindingPointAliasKind::Offset { dx, dy },
            })
        }
        GroupKind::RegularPolygonIteration => {
            if let Some((step_dx, step_dy)) =
                point_iteration_offset_step_raw(file, groups, iter_group, anchors)
            {
                let dx = step_dx * depth as f64;
                let dy = step_dy * depth as f64;
                return Some(IterationBindingPointAliasRaw {
                    position: PointRecord {
                        x: source_position.x + dx,
                        y: source_position.y + dy,
                    },
                    source_group_index,
                    kind: IterationBindingPointAliasKind::Offset { dx, dy },
                });
            }

            let (center_group_index, _angle_expr, _parameter_name, n) =
                regular_polygon_iteration_step(file, groups, iter_group)?;
            let center_position = anchors.get(center_group_index)?.clone()?;
            let total_angle = (-360.0 / n) * depth as f64;
            Some(IterationBindingPointAliasRaw {
                position: rotate_around(
                    &source_position,
                    &center_position,
                    total_angle.to_radians(),
                ),
                source_group_index,
                kind: IterationBindingPointAliasKind::Rotate {
                    center_group_index,
                    angle_degrees: total_angle,
                },
            })
        }
        _ => None,
    }
}

fn iteration_depth_raw(file: &GspFile, group: &ObjectGroup, default_depth: usize) -> usize {
    group
        .records
        .iter()
        .find(|record| record.record_type == 0x090a)
        .map(|record| record.payload(&file.data))
        .filter(|payload| payload.len() >= 20)
        .map(|payload| read_u32(payload, 16) as usize)
        .unwrap_or(default_depth)
}

fn point_iteration_offset_step_raw(
    file: &GspFile,
    groups: &[ObjectGroup],
    iter_group: &ObjectGroup,
    anchors: &[Option<PointRecord>],
) -> Option<(f64, f64)> {
    let path = find_indexed_path(file, iter_group)?;
    if let Some((dx, dy)) = path
        .refs
        .iter()
        .skip(1)
        .filter_map(|ordinal| groups.get(ordinal.checked_sub(1)?))
        .find_map(|group| decode_translated_point_constraint(file, group).map(|c| (c.dx, c.dy)))
    {
        return Some((dx, dy));
    }
    let base_start = anchors.get(path.refs.get(1)?.checked_sub(1)?)?.clone()?;
    let base_end = anchors.get(path.refs.get(2)?.checked_sub(1)?)?.clone()?;
    Some((base_end.x - base_start.x, base_end.y - base_start.y))
}

pub(crate) fn decode_expression_offset_binding(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
    anchors: &[Option<PointRecord>],
) -> Option<ExpressionOffsetBindingDef> {
    if group.header.kind() != GroupKind::ExpressionOffsetPoint {
        return None;
    }
    let path = find_indexed_path(file, group)?;
    if path.refs.len() < 2 {
        return None;
    }
    let expr_group = groups.get(path.refs[1].checked_sub(1)?)?;
    let (expr, _parameters, parameter_name) =
        expression_runtime_context(file, groups, expr_group, anchors)?;
    let payload = group
        .records
        .iter()
        .find(|record| record.record_type == 0x07d3)
        .map(|record| record.payload(&file.data))?;
    if payload.len() < 20 {
        return None;
    }
    let raw_distance = read_f64(payload, 4);
    let world_distance = read_f64(payload, 12);
    if !raw_distance.is_finite() || !world_distance.is_finite() {
        return None;
    }
    let raw_scale = if world_distance.abs() > 1e-9 {
        raw_distance / world_distance
    } else {
        PX_PER_CM
    };
    Some(ExpressionOffsetBindingDef {
        source_group_index: path.refs[0].checked_sub(1)?,
        scaled_expr: scale_function_expr(expr, raw_scale),
        parameter_name,
    })
}

pub(crate) fn decode_expression_offset_anchor_raw(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
    anchors: &[Option<PointRecord>],
) -> Option<PointRecord> {
    let binding = decode_expression_offset_binding(file, groups, group, anchors)?;
    let source = anchors.get(binding.source_group_index)?.clone()?;
    let offset = evaluate_expr_with_parameters(&binding.scaled_expr, 0.0, &BTreeMap::new())
        .or_else(|| {
            let expr_group = find_indexed_path(file, group)
                .and_then(|path| groups.get(path.refs.get(1)?.checked_sub(1)?))?;
            let (_, parameters, _) = expression_runtime_context(file, groups, expr_group, anchors)?;
            evaluate_expr_with_parameters(&binding.scaled_expr, 0.0, &parameters)
        })?;
    Some(PointRecord {
        x: source.x + offset,
        y: source.y,
    })
}

pub(crate) fn decode_graph_calibration_anchor_raw(
    file: &GspFile,
    _groups: &[ObjectGroup],
    group: &ObjectGroup,
    anchors: &[Option<PointRecord>],
    graph: Option<&GraphTransform>,
) -> Option<PointRecord> {
    let unit_length = group
        .records
        .iter()
        .find(|record| record.record_type == 0x07d3 && record.length == 12)
        .and_then(|record| {
            crate::runtime::extract::decode::decode_measurement_value(record.payload(&file.data))
        })
        .or_else(|| graph.map(|graph| graph.raw_per_unit))?;
    let source = find_indexed_path(file, group)
        .and_then(|path| path.refs.first().copied())
        .and_then(|ordinal| anchors.get(ordinal.saturating_sub(1)).cloned().flatten())
        .or_else(|| graph.map(|graph| graph.origin_raw.clone()))?;
    match group.header.kind() {
        crate::format::GroupKind::GraphCalibrationX => Some(PointRecord {
            x: source.x + unit_length,
            y: source.y,
        }),
        crate::format::GroupKind::GraphCalibrationY
        | crate::format::GroupKind::GraphCalibrationYAlt => Some(PointRecord {
            x: graph.map(|graph| graph.origin_raw.x).unwrap_or(source.x),
            y: graph.map(|graph| graph.origin_raw.y).unwrap_or(source.y) - unit_length,
        }),
        _ => None,
    }
}

pub(crate) fn decode_coordinate_expression_anchor_raw(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
    anchors: &[Option<PointRecord>],
    graph: Option<&GraphTransform>,
) -> Option<PointRecord> {
    if !matches!(
        group.header.kind(),
        crate::format::GroupKind::CoordinateExpressionPoint
            | crate::format::GroupKind::CoordinateExpressionPointAlt
            | crate::format::GroupKind::CoordinateExpressionPointPair
    ) {
        return None;
    }

    let path = find_indexed_path(file, group)?;
    if path.refs.len() < 2 {
        return None;
    }
    let source_group_index = path.refs[0].checked_sub(1)?;
    let source_position = anchors.get(source_group_index)?.clone()?;
    let source_world = to_world(&source_position, &graph.cloned());
    let world = match group.header.kind() {
        crate::format::GroupKind::CoordinateExpressionPointPair => {
            let x_calc_group = groups.get(path.refs[1].checked_sub(1)?)?;
            let y_calc_group = groups.get(path.refs[2].checked_sub(1)?)?;
            let x_expr =
                try_decode_function_expr_with_inlined_refs(file, groups, x_calc_group).ok()?;
            let y_expr =
                try_decode_function_expr_with_inlined_refs(file, groups, y_calc_group).ok()?;
            let x_parameter_group = first_path_group(file, groups, x_calc_group)?;
            let y_parameter_group = first_path_group(file, groups, y_calc_group)?;
            let x_parameter_name = decode_label_name(file, x_calc_group)
                .or_else(|| decode_label_name(file, x_parameter_group))?;
            let y_parameter_name = decode_label_name(file, y_calc_group)
                .or_else(|| decode_label_name(file, y_parameter_group))?;
            let x_parameter_value =
                decode_non_graph_parameter_value_for_group(file, x_parameter_group)?;
            let y_parameter_value =
                decode_non_graph_parameter_value_for_group(file, y_parameter_group)?;
            let dx = evaluate_expr_with_parameters(
                &x_expr,
                0.0,
                &BTreeMap::from([(x_parameter_name, x_parameter_value)]),
            )?;
            let dy = evaluate_expr_with_parameters(
                &y_expr,
                0.0,
                &BTreeMap::from([(y_parameter_name, y_parameter_value)]),
            )?;
            PointRecord {
                x: source_world.x + dx,
                y: source_world.y + dy,
            }
        }
        _ => {
            let calc_group = groups.get(path.refs[1].checked_sub(1)?)?;
            let expr = try_decode_function_expr(file, groups, calc_group).ok()?;
            let payload = group
                .records
                .iter()
                .find(|record| record.record_type == 0x07d3)
                .map(|record| record.payload(&file.data))?;
            let axis = match group.header.kind() {
                crate::format::GroupKind::CoordinateExpressionPointAlt => {
                    crate::runtime::scene::CoordinateAxis::Horizontal
                }
                _ => match (payload.len() >= 24).then(|| crate::format::read_u32(payload, 20)) {
                    Some(1) => crate::runtime::scene::CoordinateAxis::Vertical,
                    _ => crate::runtime::scene::CoordinateAxis::Horizontal,
                },
            };
            let parameter_group = first_path_group(file, groups, calc_group)?;
            let parameter_name = decode_label_name(file, parameter_group)?;
            match axis {
                crate::runtime::scene::CoordinateAxis::Horizontal => PointRecord {
                    x: source_world.x
                        + evaluate_expr_with_parameters(
                            &expr,
                            0.0,
                            &BTreeMap::from([(
                                parameter_name,
                                decode_non_graph_parameter_value_for_group(file, parameter_group)?,
                            )]),
                        )?,
                    y: source_world.y,
                },
                crate::runtime::scene::CoordinateAxis::Vertical => PointRecord {
                    x: source_world.x,
                    y: evaluate_expr_with_parameters(
                        &expr,
                        0.0,
                        &BTreeMap::from([(parameter_name, source_world.x)]),
                    )?,
                },
            }
        }
    };
    Some(if let Some(transform) = graph {
        to_raw_from_world(&world, transform)
    } else {
        world
    })
}

pub(crate) fn decode_intersection_anchor_raw(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
    anchors: &[Option<PointRecord>],
    graph: Option<&GraphTransform>,
) -> Option<PointRecord> {
    let kind = group.header.kind();
    let variant = match kind {
        crate::format::GroupKind::LinearIntersectionPoint => None,
        crate::format::GroupKind::IntersectionPoint1 => Some(0),
        crate::format::GroupKind::IntersectionPoint2 => Some(1),
        crate::format::GroupKind::CircleCircleIntersectionPoint1 => Some(0),
        crate::format::GroupKind::CircleCircleIntersectionPoint2 => Some(1),
        crate::format::GroupKind::CoordinateTraceIntersectionPoint => Some(0),
        _ => return None,
    };

    let path = find_indexed_path(file, group)?;
    if path.refs.len() != 2 {
        return None;
    }

    let left_group = groups.get(path.refs[0].checked_sub(1)?)?;
    let right_group = groups.get(path.refs[1].checked_sub(1)?)?;

    if kind == crate::format::GroupKind::CoordinateTraceIntersectionPoint {
        if let (Some((line_start, line_end, line_kind)), Some(trace_points)) = (
            resolve_line_like_constraint_raw(file, groups, anchors, left_group),
            sample_coordinate_trace_points_raw(file, groups, right_group, anchors, graph),
        ) {
            return line_polyline_intersection(line_start, line_end, line_kind, &trace_points);
        }

        if let (Some(trace_points), Some((line_start, line_end, line_kind))) = (
            sample_coordinate_trace_points_raw(file, groups, left_group, anchors, graph),
            resolve_line_like_constraint_raw(file, groups, anchors, right_group),
        ) {
            return line_polyline_intersection(line_start, line_end, line_kind, &trace_points);
        }
    }

    if let (Some((line_start, line_end, line_kind)), Some(circle)) = (
        resolve_line_like_constraint_raw(file, groups, anchors, left_group),
        resolve_circle_like_raw(file, groups, anchors, right_group),
    ) {
        return select_line_circle_intersection(
            line_start,
            line_end,
            line_kind,
            circle.center(),
            circle.radius(),
            variant.unwrap_or(0),
        );
    }

    if let (Some(circle), Some((line_start, line_end, line_kind))) = (
        resolve_circle_like_raw(file, groups, anchors, left_group),
        resolve_line_like_constraint_raw(file, groups, anchors, right_group),
    ) {
        return select_line_circle_intersection(
            line_start,
            line_end,
            line_kind,
            circle.center(),
            circle.radius(),
            variant.unwrap_or(0),
        );
    }

    if let (Some(left_circle), Some(right_circle)) = (
        resolve_circle_like_raw(file, groups, anchors, left_group),
        resolve_circle_like_raw(file, groups, anchors, right_group),
    ) {
        return select_circular_intersection(&left_circle, &right_circle, variant.unwrap_or(0));
    }

    if let (Some(point), Some(circle)) = (
        anchors.get(path.refs[0].checked_sub(1)?)?.clone(),
        resolve_circle_like_raw(file, groups, anchors, right_group),
    ) {
        return select_point_circle_tangent(&point, &circle, variant.unwrap_or(0));
    }

    if let (Some(circle), Some(point)) = (
        resolve_circle_like_raw(file, groups, anchors, left_group),
        anchors.get(path.refs[1].checked_sub(1)?)?.clone(),
    ) {
        return select_point_circle_tangent(&point, &circle, variant.unwrap_or(0));
    }

    if variant.is_none() {
        let (left_start, left_end, left_kind) =
            resolve_line_like_constraint_raw(file, groups, anchors, left_group)?;
        let (right_start, right_end, right_kind) =
            resolve_line_like_constraint_raw(file, groups, anchors, right_group)?;
        return line_line_intersection(
            &left_start,
            &left_end,
            left_kind,
            &right_start,
            &right_end,
            right_kind,
        );
    }

    let (left_start, left_end, left_kind) =
        resolve_line_like_constraint_raw(file, groups, anchors, left_group)?;
    let (right_start, right_end, right_kind) =
        resolve_line_like_constraint_raw(file, groups, anchors, right_group)?;
    line_line_intersection(
        &left_start,
        &left_end,
        left_kind,
        &right_start,
        &right_end,
        right_kind,
    )
}

fn sample_coordinate_trace_points_raw(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
    anchors: &[Option<PointRecord>],
    graph: Option<&GraphTransform>,
) -> Option<Vec<PointRecord>> {
    if (group.header.kind()) != crate::format::GroupKind::CoordinateTrace {
        return None;
    }

    let path = find_indexed_path(file, group)?;
    if path.refs.len() < 3 {
        return None;
    }
    let driver_group = groups.get(path.refs[0].checked_sub(1)?)?;
    let parameter_group = groups.get(path.refs[1].checked_sub(1)?)?;
    let calc_group = groups.get(path.refs[2].checked_sub(1)?)?;
    let payload = group
        .records
        .iter()
        .find(|record| record.record_type == 0x0902)
        .map(|record| record.payload(&file.data))?;
    let descriptor = try_decode_function_plot_descriptor(payload).ok()?;
    let expr = try_decode_function_expr(file, groups, calc_group).ok()?;
    let parameter_name =
        super::editable_non_graph_parameter_name_for_group(file, groups, parameter_group)
            .or_else(|| decode_label_name(file, parameter_group))?;

    let mut points = Vec::with_capacity(descriptor.sample_count);
    let last = descriptor.sample_count.saturating_sub(1).max(1) as f64;
    let driver = match driver_group.header.kind() {
        GroupKind::CoordinateExpressionPointPair
        | GroupKind::CoordinateExpressionPoint
        | GroupKind::CoordinateExpressionPointAlt => {
            let driver_path = find_indexed_path(file, driver_group)?;
            let source_group_index = driver_path.refs[0].checked_sub(1)?;
            let source_position = anchors.get(source_group_index)?.clone()?;
            let source_world = to_world(&source_position, &graph.cloned());
            match driver_group.header.kind() {
                GroupKind::CoordinateExpressionPointPair => {
                    let x_calc_group = groups.get(driver_path.refs[1].checked_sub(1)?)?;
                    let y_calc_group = groups.get(driver_path.refs[2].checked_sub(1)?)?;
                    let x_expr =
                        try_decode_function_expr_with_inlined_refs(file, groups, x_calc_group)
                            .ok()?;
                    let y_expr =
                        try_decode_function_expr_with_inlined_refs(file, groups, y_calc_group)
                            .ok()?;
                    Some((source_world, None, Some((x_expr, y_expr))))
                }
                GroupKind::CoordinateExpressionPointAlt => Some((
                    source_world,
                    Some(crate::runtime::scene::CoordinateAxis::Horizontal),
                    None,
                )),
                GroupKind::CoordinateExpressionPoint => {
                    let payload = driver_group
                        .records
                        .iter()
                        .find(|record| record.record_type == 0x07d3)
                        .map(|record| record.payload(&file.data))?;
                    let axis =
                        match (payload.len() >= 24).then(|| crate::format::read_u32(payload, 20)) {
                            Some(1) => crate::runtime::scene::CoordinateAxis::Vertical,
                            _ => crate::runtime::scene::CoordinateAxis::Horizontal,
                        };
                    Some((source_world, Some(axis), None))
                }
                _ => None,
            }
        }
        _ => None,
    };
    for index in 0..descriptor.sample_count {
        let t = index as f64 / last;
        let x = descriptor.x_min + (descriptor.x_max - descriptor.x_min) * t;
        let offset = evaluate_expr_with_parameters(
            &expr,
            0.0,
            &BTreeMap::from([(parameter_name.clone(), x)]),
        )?;
        let world = match &driver {
            Some((_source_world, Some(crate::runtime::scene::CoordinateAxis::Horizontal), _)) => {
                PointRecord { x: offset, y: x }
            }
            Some((_source_world, Some(crate::runtime::scene::CoordinateAxis::Vertical), _)) => {
                PointRecord { x, y: offset }
            }
            Some((source_world, None, Some((x_expr, y_expr)))) => {
                let dx = evaluate_expr_with_parameters(
                    x_expr,
                    0.0,
                    &BTreeMap::from([(parameter_name.clone(), x)]),
                )?;
                let dy = evaluate_expr_with_parameters(
                    y_expr,
                    0.0,
                    &BTreeMap::from([(parameter_name.clone(), x)]),
                )?;
                PointRecord {
                    x: source_world.x + dx,
                    y: source_world.y + dy,
                }
            }
            Some((_, None, None)) => return None,
            None => match descriptor.mode {
                crate::runtime::functions::FunctionPlotMode::Cartesian => {
                    PointRecord { x, y: offset }
                }
                crate::runtime::functions::FunctionPlotMode::Polar => PointRecord {
                    x: offset * x.cos(),
                    y: offset * x.sin(),
                },
            },
        };
        points.push(if let Some(transform) = graph {
            to_raw_from_world(&world, transform)
        } else {
            world
        });
    }
    (points.len() >= 2).then_some(points)
}

pub(crate) fn resolve_circle_like_raw(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
    group: &ObjectGroup,
) -> Option<CircularConstraintRaw> {
    let path = find_indexed_path(file, group)?;
    match group.header.kind() {
        crate::format::GroupKind::Circle => {
            if path.refs.len() != 2 {
                return None;
            }
            let center = anchors.get(path.refs[0].checked_sub(1)?)?.clone()?;
            let radius_point = anchors.get(path.refs[1].checked_sub(1)?)?.clone()?;
            let radius =
                ((radius_point.x - center.x).powi(2) + (radius_point.y - center.y).powi(2)).sqrt();
            (radius > 1e-9).then_some(CircularConstraintRaw::Circle { center, radius })
        }
        crate::format::GroupKind::CircleCenterRadius => {
            if path.refs.len() != 2 {
                return None;
            }
            let center = anchors.get(path.refs[0].checked_sub(1)?)?.clone()?;
            let radius_group = groups.get(path.refs[1].checked_sub(1)?)?;
            let radius = circle_center_radius_value(file, groups, anchors, radius_group)?;
            (radius > 1e-9).then_some(CircularConstraintRaw::Circle { center, radius })
        }
        crate::format::GroupKind::CartesianOffsetPoint
        | crate::format::GroupKind::PolarOffsetPoint => {
            let constraint = decode_translated_point_constraint(file, group)?;
            let source_group = groups.get(constraint.origin_group_index)?;
            let source = resolve_circle_like_raw(file, groups, anchors, source_group)?;
            match source {
                CircularConstraintRaw::Circle { center, radius } => {
                    (radius > 1e-9).then_some(CircularConstraintRaw::Circle {
                        center: PointRecord {
                            x: center.x + constraint.dx,
                            y: center.y + constraint.dy,
                        },
                        radius,
                    })
                }
                CircularConstraintRaw::ThreePointArc {
                    start,
                    mid,
                    end,
                    center,
                    radius,
                    ccw_span,
                    ccw_mid,
                } => Some(CircularConstraintRaw::ThreePointArc {
                    start: PointRecord {
                        x: start.x + constraint.dx,
                        y: start.y + constraint.dy,
                    },
                    mid: PointRecord {
                        x: mid.x + constraint.dx,
                        y: mid.y + constraint.dy,
                    },
                    end: PointRecord {
                        x: end.x + constraint.dx,
                        y: end.y + constraint.dy,
                    },
                    center: PointRecord {
                        x: center.x + constraint.dx,
                        y: center.y + constraint.dy,
                    },
                    radius,
                    ccw_span,
                    ccw_mid,
                }),
            }
        }
        crate::format::GroupKind::Scale => {
            let binding = try_decode_transform_binding(file, group).ok()?;
            let center = anchors.get(binding.center_group_index)?.clone()?;
            let factor = match binding.kind {
                TransformBindingKind::Scale { factor } => factor,
                _ => return None,
            };
            let source_group = groups.get(binding.source_group_index)?;
            let source = resolve_circle_like_raw(file, groups, anchors, source_group)?;
            match source {
                CircularConstraintRaw::Circle {
                    center: source_center,
                    radius,
                } => (radius > 1e-9).then_some(CircularConstraintRaw::Circle {
                    center: PointRecord {
                        x: center.x + (source_center.x - center.x) * factor,
                        y: center.y + (source_center.y - center.y) * factor,
                    },
                    radius: radius * factor.abs(),
                }),
                CircularConstraintRaw::ThreePointArc { .. } => None,
            }
        }
        crate::format::GroupKind::Reflection => {
            let source_group = groups.get(path.refs.first()?.checked_sub(1)?)?;
            let line_group = groups.get(path.refs.get(1)?.checked_sub(1)?)?;
            let source = resolve_circle_like_raw(file, groups, anchors, source_group)?;
            let (line_start, line_end) =
                super::resolve_line_like_points_raw(file, groups, anchors, line_group)?;
            match source {
                CircularConstraintRaw::Circle { center, radius } => {
                    let reflected_center = reflect_across_line(&center, &line_start, &line_end)?;
                    (radius > 1e-9).then_some(CircularConstraintRaw::Circle {
                        center: reflected_center,
                        radius,
                    })
                }
                CircularConstraintRaw::ThreePointArc { .. } => None,
            }
        }
        crate::format::GroupKind::CenterArc => {
            if path.refs.len() != 3 {
                return None;
            }
            let center = anchors.get(path.refs[0].checked_sub(1)?)?.clone()?;
            let start = anchors.get(path.refs[1].checked_sub(1)?)?.clone()?;
            let end = anchors.get(path.refs[2].checked_sub(1)?)?.clone()?;
            let [start, mid, end] =
                crate::runtime::geometry::arc_on_circle_control_points(&center, &start, &end)?;
            let radius = ((start.x - center.x).powi(2) + (start.y - center.y).powi(2)).sqrt();
            let start_angle = (start.y - center.y).atan2(start.x - center.x);
            let end_angle = (end.y - center.y).atan2(end.x - center.x);
            let ccw_span = normalize_angle_delta_raw(start_angle, end_angle);
            let ccw_mid =
                normalize_angle_delta_raw(start_angle, (mid.y - center.y).atan2(mid.x - center.x));
            Some(CircularConstraintRaw::ThreePointArc {
                start,
                mid,
                end,
                center,
                radius,
                ccw_span,
                ccw_mid,
            })
        }
        crate::format::GroupKind::ArcOnCircle => {
            if path.refs.len() != 3 {
                return None;
            }
            let circle_group = groups.get(path.refs[0].checked_sub(1)?)?;
            let circle_path = find_indexed_path(file, circle_group)?;
            if circle_path.refs.len() != 2 {
                return None;
            }
            let center = anchors.get(circle_path.refs[0].checked_sub(1)?)?.clone()?;
            let start = anchors.get(path.refs[1].checked_sub(1)?)?.clone()?;
            let end = anchors.get(path.refs[2].checked_sub(1)?)?.clone()?;
            let [start, mid, end] =
                crate::runtime::geometry::arc_on_circle_control_points(&center, &start, &end)?;
            let radius = ((start.x - center.x).powi(2) + (start.y - center.y).powi(2)).sqrt();
            let start_angle = (start.y - center.y).atan2(start.x - center.x);
            let end_angle = (end.y - center.y).atan2(end.x - center.x);
            let ccw_span = normalize_angle_delta_raw(start_angle, end_angle);
            let ccw_mid =
                normalize_angle_delta_raw(start_angle, (mid.y - center.y).atan2(mid.x - center.x));
            Some(CircularConstraintRaw::ThreePointArc {
                start,
                mid,
                end,
                center,
                radius,
                ccw_span,
                ccw_mid,
            })
        }
        crate::format::GroupKind::ThreePointArc => {
            if path.refs.len() != 3 {
                return None;
            }
            let start = anchors.get(path.refs[0].checked_sub(1)?)?.clone()?;
            let mid = anchors.get(path.refs[1].checked_sub(1)?)?.clone()?;
            let end = anchors.get(path.refs[2].checked_sub(1)?)?.clone()?;
            let geometry = three_point_arc_geometry(&start, &mid, &end)?;
            let ccw_span = normalize_angle_delta_raw(geometry.start_angle, geometry.end_angle);
            let ccw_mid = normalize_angle_delta_raw(
                geometry.start_angle,
                (mid.y - geometry.center.y).atan2(mid.x - geometry.center.x),
            );
            Some(CircularConstraintRaw::ThreePointArc {
                start,
                mid,
                end,
                center: geometry.center,
                radius: geometry.radius,
                ccw_span,
                ccw_mid,
            })
        }
        _ => None,
    }
}

pub(crate) fn resolve_line_like_points_raw(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
    group: &ObjectGroup,
) -> Option<(PointRecord, PointRecord)> {
    let (start, end, _) = resolve_line_like_constraint_raw(file, groups, anchors, group)?;
    Some((start, end))
}

pub(crate) fn resolve_line_like_constraint_raw(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
    group: &ObjectGroup,
) -> Option<(PointRecord, PointRecord, LineLikeKind)> {
    let path = find_indexed_path(file, group)?;
    match group.header.kind() {
        crate::format::GroupKind::Segment
        | crate::format::GroupKind::Line
        | crate::format::GroupKind::Ray
        | crate::format::GroupKind::MeasurementLine
        | crate::format::GroupKind::AxisLine => {
            if path.refs.len() != 2 {
                return None;
            }
            let start = anchors.get(path.refs[0].checked_sub(1)?)?.clone()?;
            let end = anchors.get(path.refs[1].checked_sub(1)?)?.clone()?;
            let kind = match group.header.kind() {
                crate::format::GroupKind::Segment => LineLikeKind::Segment,
                crate::format::GroupKind::Ray => LineLikeKind::Ray,
                _ => LineLikeKind::Line,
            };
            distinct_pair(start, end).map(|(start, end)| (start, end, kind))
        }
        crate::format::GroupKind::GraphMeasurementSegment => {
            if path.refs.len() != 2 {
                return None;
            }
            let origin = anchors.get(path.refs[0].checked_sub(1)?)?.clone()?;
            let host_group = groups.get(path.refs[1].checked_sub(1)?)?;
            let (host_start, host_end) =
                resolve_line_like_points_raw(file, groups, anchors, host_group)?;
            let start_distance =
                (host_start.x - origin.x).powi(2) + (host_start.y - origin.y).powi(2);
            let end_distance = (host_end.x - origin.x).powi(2) + (host_end.y - origin.y).powi(2);
            let end = if end_distance >= start_distance {
                host_end
            } else {
                host_start
            };
            distinct_pair(origin.clone(), end)
                .map(|(start, end)| (start, end, LineLikeKind::Segment))
        }
        crate::format::GroupKind::PerpendicularLine => {
            if path.refs.len() != 2 {
                return None;
            }
            let through = anchors.get(path.refs[0].checked_sub(1)?)?.clone()?;
            let host_group = groups.get(path.refs[1].checked_sub(1)?)?;
            let (host_start, host_end) =
                resolve_line_like_points_raw(file, groups, anchors, host_group)?;
            let dx = host_end.x - host_start.x;
            let dy = host_end.y - host_start.y;
            let len = (dx * dx + dy * dy).sqrt();
            if len <= 1e-9 {
                return None;
            }
            distinct_pair(
                through.clone(),
                PointRecord {
                    x: through.x - dy,
                    y: through.y + dx,
                },
            )
            .map(|(start, end)| (start, end, LineLikeKind::Line))
        }
        crate::format::GroupKind::ParallelLine => {
            if path.refs.len() != 2 {
                return None;
            }
            let through = anchors.get(path.refs[0].checked_sub(1)?)?.clone()?;
            let host_group = groups.get(path.refs[1].checked_sub(1)?)?;
            let (host_start, host_end) =
                resolve_line_like_points_raw(file, groups, anchors, host_group)?;
            let dx = host_end.x - host_start.x;
            let dy = host_end.y - host_start.y;
            let len = (dx * dx + dy * dy).sqrt();
            if len <= 1e-9 {
                return None;
            }
            distinct_pair(
                through.clone(),
                PointRecord {
                    x: through.x + dx,
                    y: through.y + dy,
                },
            )
            .map(|(start, end)| (start, end, LineLikeKind::Line))
        }
        crate::format::GroupKind::AngleBisectorRay => {
            if path.refs.len() != 3 {
                return None;
            }
            let start = anchors.get(path.refs[0].checked_sub(1)?)?.clone()?;
            let vertex = anchors.get(path.refs[1].checked_sub(1)?)?.clone()?;
            let end = anchors.get(path.refs[2].checked_sub(1)?)?.clone()?;
            let first_dx = start.x - vertex.x;
            let first_dy = start.y - vertex.y;
            let first_len = (first_dx * first_dx + first_dy * first_dy).sqrt();
            let second_dx = end.x - vertex.x;
            let second_dy = end.y - vertex.y;
            let second_len = (second_dx * second_dx + second_dy * second_dy).sqrt();
            if first_len <= 1e-9 || second_len <= 1e-9 {
                return None;
            }
            let sum_x = first_dx / first_len + second_dx / second_len;
            let sum_y = first_dy / first_len + second_dy / second_len;
            let sum_len = (sum_x * sum_x + sum_y * sum_y).sqrt();
            let (dir_x, dir_y) = if sum_len > 1e-9 {
                (sum_x / sum_len, sum_y / sum_len)
            } else {
                (-first_dy / first_len, first_dx / first_len)
            };
            distinct_pair(
                vertex.clone(),
                PointRecord {
                    x: vertex.x + dir_x,
                    y: vertex.y + dir_y,
                },
            )
            .map(|(start, end)| (start, end, LineLikeKind::Ray))
        }
        crate::format::GroupKind::Translation => {
            if path.refs.len() < 3 {
                return None;
            }
            let source_group = groups.get(path.refs[0].checked_sub(1)?)?;
            let (start, end, kind) =
                resolve_line_like_constraint_raw(file, groups, anchors, source_group)?;
            let vector_start = anchors.get(path.refs[1].checked_sub(1)?)?.clone()?;
            let vector_end = anchors.get(path.refs[2].checked_sub(1)?)?.clone()?;
            let dx = vector_end.x - vector_start.x;
            let dy = vector_end.y - vector_start.y;
            distinct_pair(
                PointRecord {
                    x: start.x + dx,
                    y: start.y + dy,
                },
                PointRecord {
                    x: end.x + dx,
                    y: end.y + dy,
                },
            )
            .map(|(start, end)| (start, end, kind))
        }
        crate::format::GroupKind::Rotation
        | crate::format::GroupKind::ParameterRotation
        | crate::format::GroupKind::Scale => {
            let binding = if group.header.kind() == crate::format::GroupKind::ParameterRotation {
                try_decode_parameter_rotation_binding(file, groups, group)
                    .ok()
                    .or_else(|| {
                        decode_measured_angle_parameter_rotation_binding_raw(
                            file, groups, group, anchors,
                        )
                    })?
            } else {
                try_decode_transform_binding(file, group).ok()?
            };
            let source_group = groups.get(binding.source_group_index)?;
            let (start, end, kind) =
                resolve_line_like_constraint_raw(file, groups, anchors, source_group)?;
            let center = anchors.get(binding.center_group_index)?.clone()?;
            let (start, end) = match binding.kind {
                TransformBindingKind::Rotate { angle_degrees, .. } => (
                    rotate_around(&start, &center, angle_degrees.to_radians()),
                    rotate_around(&end, &center, angle_degrees.to_radians()),
                ),
                TransformBindingKind::Scale { factor } => (
                    scale_around(&start, &center, factor),
                    scale_around(&end, &center, factor),
                ),
            };
            distinct_pair(start, end).map(|(start, end)| (start, end, kind))
        }
        _ => None,
    }
}

pub(crate) fn decode_regular_polygon_vertex_anchor_raw(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
    anchors: &[Option<PointRecord>],
) -> Option<PointRecord> {
    if (group.header.kind()) != crate::format::GroupKind::ParameterRotation {
        return None;
    }
    let path = find_indexed_path(file, group)?;
    if path.refs.len() < 3 {
        return None;
    }
    let source = anchors.get(path.refs[0].checked_sub(1)?)?.clone()?;
    let center = anchors.get(path.refs[1].checked_sub(1)?)?.clone()?;
    let calc_group = groups.get(path.refs[2].checked_sub(1)?)?;
    let calc_path = find_indexed_path(file, calc_group)?;
    let parameter_group = groups.get(calc_path.refs.first()?.checked_sub(1)?)?;
    let n = decode_non_graph_parameter_value_for_group(file, parameter_group)?;
    if n.abs() < 3.0 {
        return None;
    }
    Some(rotate_around(&source, &center, (-360.0 / n).to_radians()))
}

pub(crate) fn decode_custom_transform_anchor_raw(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
    anchors: &[Option<PointRecord>],
) -> Option<PointRecord> {
    if (group.header.kind()) != crate::format::GroupKind::CustomTransformPoint {
        return None;
    }
    let binding = decode_custom_transform_binding(file, groups, group.ordinal)?;
    let t = decode_custom_transform_parameter(file, groups, binding.source_group_index, anchors)?;
    resolve_custom_transform_point(anchors, &binding, t)
}

pub(crate) fn decode_custom_transform_binding(
    file: &GspFile,
    groups: &[ObjectGroup],
    target_ordinal: usize,
) -> Option<CustomTransformBindingDef> {
    let transform_group = groups.iter().find(|candidate| {
        (candidate.header.kind()) == crate::format::GroupKind::CustomTransformTrace
            && find_indexed_path(file, candidate).is_some_and(|path| {
                path.refs.first().copied() == Some(target_ordinal)
                    || path.refs.last().copied() == Some(target_ordinal)
            })
    })?;
    let path = find_indexed_path(file, transform_group)?;
    if path.refs.len() < 6 {
        return None;
    }
    let source_group_index = path.refs.get(2)?.checked_sub(1)?;
    let (origin_group_index, axis_end_group_index) =
        custom_transform_basis_indices(file, groups, source_group_index).or_else(|| {
            let axis_group = groups.get(path.refs.get(1)?.checked_sub(1)?)?;
            let axis_path = find_indexed_path(file, axis_group)?;
            Some((
                axis_path.refs.first()?.checked_sub(1)?,
                axis_path.refs.get(1)?.checked_sub(1)?,
            ))
        })?;
    let distance_expr_group = groups.get(path.refs.get(4)?.checked_sub(1)?)?;
    let angle_expr_group = groups.get(path.refs.get(5)?.checked_sub(1)?)?;
    let distance_expr = try_decode_function_expr(file, groups, distance_expr_group).ok()?;
    let angle_expr = try_decode_function_expr(file, groups, angle_expr_group).ok()?;
    Some(CustomTransformBindingDef {
        source_group_index,
        origin_group_index,
        axis_end_group_index,
        distance_expr,
        angle_expr,
        distance_raw_scale: decode_custom_transform_distance_scale(file, distance_expr_group)?,
        angle_degrees_scale: decode_custom_transform_angle_scale(file, angle_expr_group)?,
    })
}

fn custom_transform_basis_indices(
    file: &GspFile,
    groups: &[ObjectGroup],
    source_group_index: usize,
) -> Option<(usize, usize)> {
    let source_group = groups.get(source_group_index)?;
    match source_group.header.kind() {
        kind if kind.is_point_constraint() => {
            let host_group = groups.get(
                find_indexed_path(file, source_group)?
                    .refs
                    .first()?
                    .checked_sub(1)?,
            )?;
            if (host_group.header.kind()) != crate::format::GroupKind::Segment {
                return None;
            }
            let host_path = find_indexed_path(file, host_group)?;
            Some((
                host_path.refs.first()?.checked_sub(1)?,
                host_path.refs.get(1)?.checked_sub(1)?,
            ))
        }
        crate::format::GroupKind::ParameterControlledPoint => {
            let host_group = groups.get(
                find_indexed_path(file, source_group)?
                    .refs
                    .get(1)?
                    .checked_sub(1)?,
            )?;
            if (host_group.header.kind()) != crate::format::GroupKind::Segment {
                return None;
            }
            let host_path = find_indexed_path(file, host_group)?;
            Some((
                host_path.refs.first()?.checked_sub(1)?,
                host_path.refs.get(1)?.checked_sub(1)?,
            ))
        }
        _ => None,
    }
}

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
