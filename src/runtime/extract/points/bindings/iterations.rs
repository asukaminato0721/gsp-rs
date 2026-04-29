use std::collections::BTreeSet;

use super::{
    GspFile, ObjectGroup, PointRecord, RawPointIterationFamily, TransformBindingKind,
    decode_translated_point_constraint, iteration_depth, regular_polygon_iteration_step,
    rotate_around, try_decode_parameter_rotation_binding, try_decode_transform_binding,
};
use crate::runtime::extract::decode::decode_label_name;
use crate::runtime::extract::find_indexed_path;
use crate::runtime::extract::points::{
    editable_non_graph_parameter_name_for_group, is_editable_non_graph_parameter_name,
    regular_polygon_angle_expr,
};
use crate::runtime::functions::try_decode_function_expr;
use crate::runtime::geometry::color_from_style;
use crate::runtime::scene::{ScenePoint, ScenePointBinding, ScenePointConstraint};

fn mapped_point_index(group_to_point_index: &[Option<usize>], group_index: usize) -> Option<usize> {
    group_to_point_index.get(group_index).copied().flatten()
}

pub(crate) fn collect_point_iteration_points(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
    group_to_point_index: &[Option<usize>],
) -> (Vec<ScenePoint>, Vec<RawPointIterationFamily>) {
    let mut derived_points = Vec::new();
    let mut families = Vec::new();

    for group in groups
        .iter()
        .filter(|group| (group.header.kind()) == crate::format::GroupKind::IterationBinding)
    {
        let Some(path) = find_indexed_path(file, group) else {
            continue;
        };
        if path.refs.len() < 2 {
            continue;
        }
        let Some(seed_group_index) = path.refs[0].checked_sub(1) else {
            continue;
        };
        let Some(iter_group_index) = path.refs[1].checked_sub(1) else {
            continue;
        };
        let Some(seed_index) = mapped_point_index(group_to_point_index, seed_group_index) else {
            continue;
        };
        let Some(iter_group) = groups.get(iter_group_index) else {
            continue;
        };
        let seed_color = color_from_style(groups[seed_group_index].header.style_b);
        match iter_group.header.kind() {
            crate::format::GroupKind::AffineIteration => {
                let depth = iteration_depth(file, iter_group, 3);
                if depth == 0 {
                    continue;
                }
                let seed_group = &groups[seed_group_index];
                let rotation = match seed_group.header.kind() {
                    crate::format::GroupKind::ParameterRotation => {
                        try_decode_parameter_rotation_binding(file, groups, seed_group).ok()
                    }
                    crate::format::GroupKind::Rotation => {
                        try_decode_transform_binding(file, seed_group).ok()
                    }
                    _ => None,
                };
                if let Some(binding) = rotation {
                    let Some(center_index) =
                        mapped_point_index(group_to_point_index, binding.center_group_index)
                    else {
                        continue;
                    };
                    let TransformBindingKind::Rotate { angle_degrees, .. } = binding.kind else {
                        continue;
                    };
                    let Some(center_position) =
                        anchors.get(binding.center_group_index).cloned().flatten()
                    else {
                        continue;
                    };
                    let Some(seed_position) = anchors.get(seed_group_index).cloned().flatten()
                    else {
                        continue;
                    };

                    let mut previous_index = seed_index;
                    let mut current_position: PointRecord = seed_position;
                    for _ in 0..depth {
                        current_position = rotate_around(
                            &current_position,
                            &center_position,
                            angle_degrees.to_radians(),
                        );
                        derived_points.push(ScenePoint {
                            position: current_position.clone(),
                            color: seed_color,
                            visible: true,
                            draggable: false,
                            constraint: ScenePointConstraint::Free,
                            binding: Some(ScenePointBinding::Rotate {
                                source_index: previous_index,
                                center_index,
                                angle_degrees,
                                parameter_name: None,
                                angle_expr: None,
                                angle_start_index: None,
                                angle_vertex_index: None,
                                angle_end_index: None,
                                angle_parameter_point_index: None,
                                angle_parameter_start_index: None,
                                angle_parameter_end_index: None,
                                angle_parameter_scale: None,
                            }),
                            debug: None,
                        });
                        previous_index = seed_index + derived_points.len();
                    }
                    families.push(RawPointIterationFamily::RotateChain {
                        seed_index,
                        center_index,
                        angle_degrees,
                        depth,
                    });
                    continue;
                }
                let Some(iter_path) = find_indexed_path(file, iter_group) else {
                    continue;
                };
                if iter_path.refs.len() < 2 {
                    continue;
                }
                let Some(base_start) = anchors
                    .get(iter_path.refs[0].saturating_sub(1))
                    .cloned()
                    .flatten()
                else {
                    continue;
                };
                let Some(base_end) = anchors
                    .get(iter_path.refs[1].saturating_sub(1))
                    .cloned()
                    .flatten()
                else {
                    continue;
                };
                let dx = base_end.x - base_start.x;
                let dy = base_end.y - base_start.y;
                let Some(seed_position) = anchors.get(seed_group_index).cloned().flatten() else {
                    continue;
                };

                let mut previous_index = seed_index + derived_points.len();
                let mut current_position = seed_position;
                for _ in 0..depth {
                    current_position += PointRecord { x: dx, y: dy };
                    derived_points.push(ScenePoint {
                        position: current_position.clone(),
                        color: seed_color,
                        visible: true,
                        draggable: false,
                        constraint: ScenePointConstraint::Offset {
                            origin_index: previous_index,
                            dx,
                            dy,
                        },
                        binding: None,
                        debug: None,
                    });
                    previous_index = seed_index + derived_points.len();
                }
                families.push(RawPointIterationFamily::Offset {
                    seed_index,
                    dx,
                    dy,
                    depth,
                    parameter_name: None,
                });
            }
            crate::format::GroupKind::RegularPolygonIteration => {
                let Some(iter_path) = find_indexed_path(file, iter_group) else {
                    continue;
                };
                let depth = iteration_depth(file, iter_group, 3);
                if depth == 0 {
                    continue;
                }
                if let Some((depth_parameter_name, trace_parameter_name, step_expr)) =
                    parameterized_point_iteration(groups, iter_group, file)
                {
                    for point_index in parameterized_point_trace_indices(
                        file,
                        groups,
                        seed_group_index,
                        iter_group_index,
                        seed_index,
                        group_to_point_index,
                    ) {
                        families.push(RawPointIterationFamily::Parameterized {
                            point_index,
                            depth_parameter_name: depth_parameter_name.clone(),
                            trace_parameter_name: trace_parameter_name.clone(),
                            step_expr: step_expr.clone(),
                            depth,
                        });
                    }
                    continue;
                }
                if let Some((parameter_name, dx, dy)) =
                    parameter_iteration_step(groups, iter_group, anchors, file)
                {
                    let Some(seed_position) = anchors.get(seed_group_index).cloned().flatten()
                    else {
                        continue;
                    };
                    let mut previous_index = seed_index + derived_points.len();
                    let mut current_position: PointRecord = seed_position;
                    for _ in 0..depth {
                        current_position += PointRecord { x: dx, y: dy };
                        derived_points.push(ScenePoint {
                            position: current_position.clone(),
                            color: seed_color,
                            visible: true,
                            draggable: false,
                            constraint: ScenePointConstraint::Offset {
                                origin_index: previous_index,
                                dx,
                                dy,
                            },
                            binding: None,
                            debug: None,
                        });
                        previous_index = seed_index + derived_points.len();
                    }
                    families.push(RawPointIterationFamily::Offset {
                        seed_index,
                        dx,
                        dy,
                        depth,
                        parameter_name: is_editable_non_graph_parameter_name(&parameter_name)
                            .then_some(parameter_name),
                    });
                } else if let Some((center_group_index, _angle_expr, parameter_name, n)) =
                    regular_polygon_iteration_step(file, groups, iter_group)
                {
                    let Some(center_index) =
                        mapped_point_index(group_to_point_index, center_group_index)
                    else {
                        continue;
                    };
                    let Some(seed_position) = anchors.get(seed_group_index).cloned().flatten()
                    else {
                        continue;
                    };
                    let Some(center_position) = anchors.get(center_group_index).cloned().flatten()
                    else {
                        continue;
                    };
                    let angle_degrees = -360.0 / n;
                    for step in 1..=depth {
                        let radians = (angle_degrees * step as f64).to_radians();
                        let cos = radians.cos();
                        let sin = radians.sin();
                        let dx = seed_position.x - center_position.x;
                        let dy = seed_position.y - center_position.y;
                        let position = PointRecord {
                            x: center_position.x + dx * cos + dy * sin,
                            y: center_position.y - dx * sin + dy * cos,
                        };
                        derived_points.push(ScenePoint {
                            position,
                            color: seed_color,
                            visible: true,
                            draggable: false,
                            constraint: ScenePointConstraint::Free,
                            binding: Some(ScenePointBinding::Rotate {
                                source_index: seed_index,
                                center_index,
                                angle_degrees: angle_degrees * step as f64,
                                parameter_name: None,
                                angle_expr: None,
                                angle_start_index: None,
                                angle_vertex_index: None,
                                angle_end_index: None,
                                angle_parameter_point_index: None,
                                angle_parameter_start_index: None,
                                angle_parameter_end_index: None,
                                angle_parameter_scale: None,
                            }),
                            debug: None,
                        });
                    }
                    let angle_expr = regular_polygon_angle_expr(&parameter_name, n);
                    families.push(RawPointIterationFamily::Rotate {
                        source_index: seed_index,
                        center_index,
                        angle_expr,
                        depth,
                        parameter_name: is_editable_non_graph_parameter_name(&parameter_name)
                            .then_some(parameter_name),
                    });
                } else if iter_path.refs.len() >= 2 {
                    let _ = iter_path;
                }
            }
            _ => {}
        }
    }

    (derived_points, families)
}

fn parameterized_point_iteration(
    groups: &[ObjectGroup],
    iter_group: &ObjectGroup,
    file: &GspFile,
) -> Option<(
    Option<String>,
    String,
    crate::runtime::functions::FunctionExpr,
)> {
    let path = find_indexed_path(file, iter_group)?;
    if path.refs.len() < 3 {
        return None;
    }
    let depth_parameter_group = groups.get(path.refs[0].checked_sub(1)?)?;
    let trace_parameter_group = groups.get(path.refs[1].checked_sub(1)?)?;
    let step_group = groups.get(path.refs[2].checked_sub(1)?)?;
    let trace_parameter_name =
        editable_non_graph_parameter_name_for_group(file, groups, trace_parameter_group)?;
    let step_expr = try_decode_function_expr(file, groups, step_group).ok()?;
    let depth_parameter_name =
        editable_non_graph_parameter_name_for_group(file, groups, depth_parameter_group);
    Some((depth_parameter_name, trace_parameter_name, step_expr))
}

fn parameterized_point_trace_indices(
    file: &GspFile,
    groups: &[ObjectGroup],
    seed_group_index: usize,
    iter_group_index: usize,
    seed_index: usize,
    group_to_point_index: &[Option<usize>],
) -> Vec<usize> {
    let mut point_indices = BTreeSet::new();
    let has_seed_p_alias = groups.iter().any(|group| {
        group.header.kind() == crate::format::GroupKind::IterationPointAlias
            && decode_label_name(file, group).as_deref() == Some("P")
            && iteration_alias_matches(file, groups, group, seed_group_index, iter_group_index)
    });
    if !has_seed_p_alias {
        return Vec::new();
    }

    {
        point_indices.insert(seed_index);
    }

    for (group_index, group) in groups.iter().enumerate() {
        if group.header.kind() != crate::format::GroupKind::Translation {
            continue;
        }
        let Some(point_index) = mapped_point_index(group_to_point_index, group_index) else {
            continue;
        };
        let Some(path) = find_indexed_path(file, group) else {
            continue;
        };
        let references_target_alias = path.refs.iter().any(|ordinal| {
            groups
                .get(ordinal.saturating_sub(1))
                .is_some_and(|alias_group| {
                    alias_group.header.kind() == crate::format::GroupKind::IterationPointAlias
                        && iteration_alias_matches(
                            file,
                            groups,
                            alias_group,
                            seed_group_index,
                            iter_group_index,
                        )
                })
        });
        if references_target_alias {
            point_indices.insert(point_index);
        }
    }

    point_indices.into_iter().collect()
}

fn iteration_alias_matches(
    file: &GspFile,
    groups: &[ObjectGroup],
    alias_group: &ObjectGroup,
    seed_group_index: usize,
    iter_group_index: usize,
) -> bool {
    let Some(alias_path) = find_indexed_path(file, alias_group) else {
        return false;
    };
    let Some(binding_ordinal) = alias_path.refs.first().copied() else {
        return false;
    };
    let Some(binding_group) = groups.get(binding_ordinal.saturating_sub(1)) else {
        return false;
    };
    if binding_group.header.kind() != crate::format::GroupKind::IterationBinding {
        return false;
    }
    find_indexed_path(file, binding_group).is_some_and(|binding_path| {
        binding_path.refs.first().copied() == Some(seed_group_index + 1)
            && binding_path.refs.get(1).copied() == Some(iter_group_index + 1)
    })
}

fn parameter_iteration_step(
    groups: &[ObjectGroup],
    iter_group: &ObjectGroup,
    anchors: &[Option<PointRecord>],
    file: &GspFile,
) -> Option<(String, f64, f64)> {
    let path = find_indexed_path(file, iter_group)?;
    if path.refs.len() < 3 {
        return None;
    }
    let parameter_group = groups.get(path.refs[0].checked_sub(1)?)?;
    if (parameter_group.header.kind()) != crate::format::GroupKind::Point {
        return None;
    }
    let parameter_name =
        editable_non_graph_parameter_name_for_group(file, groups, parameter_group)?;
    if let Some((dx, dy)) = path
        .refs
        .iter()
        .skip(1)
        .filter_map(|ordinal: &usize| {
            let index = ordinal.checked_sub(1)?;
            groups.get(index)
        })
        .find_map(|group| {
            decode_translated_point_constraint(file, group)
                .map(|constraint| (constraint.dx, constraint.dy))
        })
    {
        return Some((parameter_name, dx, dy));
    }
    let base_start = anchors.get(path.refs[1].checked_sub(1)?)?.clone()?;
    let base_end = anchors.get(path.refs[2].checked_sub(1)?)?.clone()?;
    Some((
        parameter_name,
        base_end.x - base_start.x,
        base_end.y - base_start.y,
    ))
}
