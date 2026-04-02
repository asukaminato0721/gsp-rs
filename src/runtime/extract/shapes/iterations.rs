use super::*;

pub(crate) fn collect_rotational_iteration_lines(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
) -> Vec<LineShape> {
    groups
        .iter()
        .filter(|group| (group.header.class_id & 0xffff) == 77)
        .filter_map(|group| {
            let path = find_indexed_path(file, group)?;
            let source_group = groups.get(path.refs.first()?.checked_sub(1)?)?;
            if (source_group.header.class_id & 0xffff) != 2 {
                return None;
            }
            let iter_group = groups.get(path.refs.get(1)?.checked_sub(1)?)?;
            if (iter_group.header.class_id & 0xffff) != 89 {
                return None;
            }
            let (center_group_index, angle_expr, parameter_name, n) =
                regular_polygon_iteration_step(file, groups, iter_group)?;
            let angle_degrees = -360.0 / n;
            let center = anchors.get(center_group_index)?.clone()?;
            let source_path = find_indexed_path(file, source_group)?;
            if source_path.refs.len() != 2 {
                return None;
            }
            let seed_vertex_group = groups.get(path.refs.first()?.checked_sub(1)?)?;
            let seed_vertex_path = find_indexed_path(file, seed_vertex_group)?;
            let vertex_group_index = seed_vertex_path.refs.first()?.checked_sub(1)?;
            let vertex = anchors.get(vertex_group_index)?.clone()?;
            let depth = iter_group
                .records
                .iter()
                .find(|record| record.record_type == 0x090a)
                .map(|record| record.payload(&file.data))
                .filter(|payload| payload.len() >= 20)
                .map(|payload| read_u32(payload, 16) as usize)
                .unwrap_or(0);
            let mut lines = Vec::new();
            let rotate = |point: &PointRecord, step: usize| {
                rotate_around(point, &center, (angle_degrees * step as f64).to_radians())
            };
            for step in 0..=depth {
                lines.push(LineShape {
                    points: vec![
                        rotate(&vertex, step),
                        rotate(&vertex, (step + 1) % (depth + 1)),
                    ],
                    color: color_from_style(source_group.header.style_b),
                    dashed: false,
                    binding: Some(LineBinding::RotateEdge {
                        center_index: center_group_index,
                        vertex_index: vertex_group_index,
                        parameter_name: parameter_name.clone(),
                        angle_expr: angle_expr.clone(),
                        start_step: step,
                        end_step: (step + 1) % (depth + 1),
                    }),
                });
            }
            Some(lines)
        })
        .flatten()
        .collect()
}

pub(crate) fn collect_carried_iteration_lines(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
) -> Vec<LineShape> {
    groups
        .iter()
        .filter(|group| (group.header.class_id & 0xffff) == 77)
        .filter_map(|group| {
            let path = find_indexed_path(file, group)?;
            let source_group = groups.get(path.refs.first()?.checked_sub(1)?)?;
            if (source_group.header.class_id & 0xffff) != 2 {
                return None;
            }
            let iter_group = groups.get(path.refs.get(1)?.checked_sub(1)?)?;
            if !matches!(iter_group.header.class_id & 0xffff, 76 | 89) {
                return None;
            }
            let source_path = find_indexed_path(file, source_group)?;
            if source_path.refs.len() != 2 {
                return None;
            }
            let start = anchors.get(source_path.refs[0].checked_sub(1)?)?.clone()?;
            let end = anchors.get(source_path.refs[1].checked_sub(1)?)?.clone()?;
            let steps = carried_iteration_steps(file, groups, iter_group, anchors);
            let Some(step) = steps.first().cloned() else {
                return None;
            };
            let secondary_step = steps.get(1).cloned();
            let depth = iter_group
                .records
                .iter()
                .find(|record| record.record_type == 0x090a)
                .map(|record| record.payload(&file.data))
                .filter(|payload| payload.len() >= 20)
                .map(|payload| read_u32(payload, 16) as usize)
                .unwrap_or(3);
            let color = color_from_style(source_group.header.style_b);
            Some(
                carried_iteration_line_deltas(&step, secondary_step.as_ref(), depth)
                    .into_iter()
                    .map(|delta| LineShape {
                        points: vec![start.clone() + delta.clone(), end.clone() + delta],
                        color,
                        dashed: false,
                        binding: None,
                    })
                    .collect::<Vec<_>>(),
            )
        })
        .flatten()
        .collect()
}

pub(crate) fn collect_carried_line_iteration_families(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
    group_to_point_index: &[Option<usize>],
) -> Vec<LineIterationFamily> {
    groups
        .iter()
        .filter(|group| (group.header.class_id & 0xffff) == 77)
        .filter_map(|group| {
            let path = find_indexed_path(file, group)?;
            let source_group_index = path.refs.first()?.checked_sub(1)?;
            let source_group = groups.get(source_group_index)?;
            if (source_group.header.class_id & 0xffff) != 2 {
                return None;
            }
            let iter_group = groups.get(path.refs.get(1)?.checked_sub(1)?)?;
            if !matches!(iter_group.header.class_id & 0xffff, 76 | 89) {
                return None;
            }
            let source_path = find_indexed_path(file, source_group)?;
            if source_path.refs.len() != 2 {
                return None;
            }
            let start_index = group_to_point_index
                .get(source_path.refs[0].checked_sub(1)?)
                .copied()
                .flatten()?;
            let end_index = group_to_point_index
                .get(source_path.refs[1].checked_sub(1)?)
                .copied()
                .flatten()?;
            let steps = carried_iteration_steps(file, groups, iter_group, anchors);
            let Some(step) = steps.first().cloned() else {
                return None;
            };
            let secondary_step = steps.get(1).cloned();
            let depth = carried_iteration_depth(file, iter_group, 3);
            if depth == 0 {
                return None;
            }
            Some(LineIterationFamily {
                start_index,
                end_index,
                dx: step.x,
                dy: step.y,
                secondary_dx: secondary_step.as_ref().map(|step| step.x),
                secondary_dy: secondary_step.as_ref().map(|step| step.y),
                depth,
                parameter_name: carried_iteration_parameter_name(file, groups, iter_group),
                color: color_from_style(source_group.header.style_b),
                dashed: false,
            })
        })
        .collect()
}

fn carried_iteration_steps(
    file: &GspFile,
    groups: &[ObjectGroup],
    iter_group: &ObjectGroup,
    anchors: &[Option<PointRecord>],
) -> Vec<PointRecord> {
    let Some(iter_path) = find_indexed_path(file, iter_group) else {
        return Vec::new();
    };
    let translated_steps = iter_path
        .refs
        .iter()
        .filter_map(|ordinal| ordinal.checked_sub(1).and_then(|index| groups.get(index)))
        .filter_map(|group| decode_translated_point_constraint(file, group))
        .map(|constraint| PointRecord {
            x: constraint.dx,
            y: constraint.dy,
        })
        .fold(Vec::<PointRecord>::new(), |mut acc, step| {
            let already_present = acc.iter().any(|existing| {
                (existing.x - step.x).abs() < 1e-6 && (existing.y - step.y).abs() < 1e-6
            });
            if !already_present {
                acc.push(step);
            }
            acc
        });
    if !translated_steps.is_empty() {
        return translated_steps;
    }
    if iter_path.refs.len() < 2 {
        return Vec::new();
    }
    let Some(base_start_index) = iter_path.refs[0].checked_sub(1) else {
        return Vec::new();
    };
    let Some(base_end_index) = iter_path.refs[1].checked_sub(1) else {
        return Vec::new();
    };
    let Some(base_start) = anchors.get(base_start_index).cloned().flatten() else {
        return Vec::new();
    };
    let Some(base_end) = anchors.get(base_end_index).cloned().flatten() else {
        return Vec::new();
    };
    vec![PointRecord {
        x: base_end.x - base_start.x,
        y: base_end.y - base_start.y,
    }]
}

fn carried_iteration_line_deltas(
    step: &PointRecord,
    secondary_step: Option<&PointRecord>,
    depth: usize,
) -> Vec<PointRecord> {
    if let Some(secondary) = secondary_step {
        let mut deltas = Vec::new();
        for primary_index in 0..=depth {
            for secondary_index in 0..=depth - primary_index {
                if primary_index == 0 && secondary_index == 0 {
                    continue;
                }
                deltas.push(PointRecord {
                    x: step.x * primary_index as f64 + secondary.x * secondary_index as f64,
                    y: step.y * primary_index as f64 + secondary.y * secondary_index as f64,
                });
            }
        }
        return deltas;
    }

    (1..=depth)
        .map(|index| PointRecord {
            x: step.x * index as f64,
            y: step.y * index as f64,
        })
        .collect()
}

fn carried_iteration_polygon_deltas(
    step: &PointRecord,
    secondary_step: Option<&PointRecord>,
    depth: usize,
) -> Vec<PointRecord> {
    if let Some(secondary) = secondary_step {
        let mut deltas = Vec::new();
        for primary_index in 0..=depth {
            for secondary_index in 0..=depth - primary_index {
                deltas.push(PointRecord {
                    x: step.x * primary_index as f64 + secondary.x * secondary_index as f64,
                    y: step.y * primary_index as f64 + secondary.y * secondary_index as f64,
                });
            }
        }
        return deltas;
    }

    (0..=depth)
        .map(|index| PointRecord {
            x: step.x * index as f64,
            y: step.y * index as f64,
        })
        .collect()
}

fn carried_iteration_depth(
    file: &GspFile,
    iter_group: &ObjectGroup,
    default_depth: usize,
) -> usize {
    iter_group
        .records
        .iter()
        .find(|record| record.record_type == 0x090a)
        .map(|record| record.payload(&file.data))
        .filter(|payload| payload.len() >= 20)
        .map(|payload| read_u32(payload, 16) as usize)
        .unwrap_or(default_depth)
}

fn carried_iteration_parameter_name(
    file: &GspFile,
    groups: &[ObjectGroup],
    iter_group: &ObjectGroup,
) -> Option<String> {
    let iter_path = find_indexed_path(file, iter_group)?;
    let parameter_group = groups.get(iter_path.refs.first()?.checked_sub(1)?)?;
    ((parameter_group.header.class_id & 0xffff) == 0)
        .then(|| decode_label_name(file, parameter_group))
        .flatten()
        .filter(|name| super::super::points::is_editable_non_graph_parameter_name(name))
}

pub(crate) fn collect_carried_iteration_polygons(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
) -> Vec<PolygonShape> {
    groups
        .iter()
        .filter(|group| (group.header.class_id & 0xffff) == 77)
        .filter_map(|group| {
            let path = find_indexed_path(file, group)?;
            let source_group = groups.get(path.refs.first()?.checked_sub(1)?)?;
            if (source_group.header.class_id & 0xffff) != 8 {
                return None;
            }
            let iter_group = groups.get(path.refs.get(1)?.checked_sub(1)?)?;
            if !matches!(iter_group.header.class_id & 0xffff, 76 | 89) {
                return None;
            }
            let source_path = find_indexed_path(file, source_group)?;
            if source_path.refs.len() < 3 {
                return None;
            }
            let points = source_path
                .refs
                .iter()
                .map(|ordinal| anchors.get(ordinal.checked_sub(1)?).cloned().flatten())
                .collect::<Option<Vec<_>>>()?;
            let steps = carried_iteration_steps(file, groups, iter_group, anchors);
            let Some(step) = steps.first().cloned() else {
                return None;
            };
            let secondary_step = steps.get(1).cloned();
            let depth = iter_group
                .records
                .iter()
                .find(|record| record.record_type == 0x090a)
                .map(|record| record.payload(&file.data))
                .filter(|payload| payload.len() >= 20)
                .map(|payload| read_u32(payload, 16) as usize)
                .unwrap_or(3);
            let color =
                fill_color_from_styles(source_group.header.style_a, source_group.header.style_b);
            Some(
                carried_iteration_polygon_deltas(&step, secondary_step.as_ref(), depth)
                    .into_iter()
                    .map(|delta| PolygonShape {
                        points: points
                            .iter()
                            .cloned()
                            .map(|point| point + delta.clone())
                            .collect(),
                        color,
                        binding: None,
                    })
                    .collect::<Vec<_>>(),
            )
        })
        .flatten()
        .collect()
}

pub(crate) fn collect_carried_polygon_iteration_families(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
    group_to_point_index: &[Option<usize>],
) -> Vec<PolygonIterationFamily> {
    groups
        .iter()
        .filter(|group| (group.header.class_id & 0xffff) == 77)
        .filter_map(|group| {
            let path = find_indexed_path(file, group)?;
            let source_group = groups.get(path.refs.first()?.checked_sub(1)?)?;
            if (source_group.header.class_id & 0xffff) != 8 {
                return None;
            }
            let iter_group = groups.get(path.refs.get(1)?.checked_sub(1)?)?;
            if !matches!(iter_group.header.class_id & 0xffff, 76 | 89) {
                return None;
            }
            let source_path = find_indexed_path(file, source_group)?;
            if source_path.refs.len() < 3 {
                return None;
            }
            let vertex_indices = source_path
                .refs
                .iter()
                .map(|ordinal| {
                    group_to_point_index
                        .get(ordinal.checked_sub(1)?)
                        .copied()
                        .flatten()
                })
                .collect::<Option<Vec<_>>>()?;
            let steps = carried_iteration_steps(file, groups, iter_group, anchors);
            let Some(step) = steps.first().cloned() else {
                return None;
            };
            let secondary_step = steps.get(1).cloned();
            let depth = carried_iteration_depth(file, iter_group, 3);
            if depth == 0 {
                return None;
            }
            Some(PolygonIterationFamily {
                vertex_indices,
                dx: step.x,
                dy: step.y,
                secondary_dx: secondary_step.as_ref().map(|step| step.x),
                secondary_dy: secondary_step.as_ref().map(|step| step.y),
                depth,
                parameter_name: carried_iteration_parameter_name(file, groups, iter_group),
                color: fill_color_from_styles(
                    source_group.header.style_a,
                    source_group.header.style_b,
                ),
            })
        })
        .collect()
}

pub(crate) fn collect_iteration_shapes(
    file: &GspFile,
    groups: &[ObjectGroup],
    circles: &[CircleShape],
) -> (Vec<LineShape>, Vec<PolygonShape>) {
    let mut lines = Vec::new();
    let polygons = Vec::new();

    let has_iteration = groups
        .iter()
        .any(|group| (group.header.class_id & 0xffff) == 89);
    if !has_iteration {
        return (lines, polygons);
    }

    for iter_group in groups
        .iter()
        .filter(|group| (group.header.class_id & 0xffff) == 89)
    {
        let Some(iter_path) = find_indexed_path(file, iter_group) else {
            continue;
        };

        let iter_data = iter_group
            .records
            .iter()
            .find(|record| record.record_type == 0x090a)
            .map(|record| record.payload(&file.data));

        let depth = iter_data
            .filter(|payload| payload.len() >= 20)
            .map(|payload| read_u32(payload, 16) as usize)
            .unwrap_or(0);
        if depth == 0 {
            continue;
        }

        let polygon_group_index = iter_path.refs.iter().find_map(|&obj_ref| {
            let index = obj_ref.checked_sub(1)?;
            let group = groups.get(index)?;
            ((group.header.class_id & 0xffff) == 8).then_some(index)
        });

        let Some(polygon_index) = polygon_group_index else {
            continue;
        };
        let polygon_group = &groups[polygon_index];
        let Some(polygon_path) = find_indexed_path(file, polygon_group) else {
            continue;
        };
        if polygon_path.refs.len() < 3 {
            continue;
        }

        let Some(circle) = circles.first() else {
            continue;
        };
        let cx = circle.center.x;
        let cy = circle.center.y;
        let radius =
            ((circle.radius_point.x - cx).powi(2) + (circle.radius_point.y - cy).powi(2)).sqrt();
        if radius < 1.0 {
            continue;
        }

        let px_per_cm = groups
            .iter()
            .filter(|group| (group.header.class_id & 0xffff) == 21)
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
            .filter(|group| (group.header.class_id & 0xffff) == 21)
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

        let side = param_value * px_per_cm / 2.0;
        if side < 1.0 {
            continue;
        }

        let outline_color = [30, 30, 30, 255];
        let sqrt3 = 3.0_f64.sqrt();
        let col_spacing = sqrt3 * side;
        let row_spacing = 1.5 * side;
        let max_cols = (radius / col_spacing).ceil() as i32 + 2;
        let max_rows = (radius / row_spacing).ceil() as i32 + 2;

        let hex_vertices = |hx: f64, hy: f64| -> Vec<PointRecord> {
            (0..6)
                .map(|i| {
                    let angle =
                        std::f64::consts::FRAC_PI_3 * i as f64 + std::f64::consts::FRAC_PI_6;
                    PointRecord {
                        x: hx + side * angle.cos(),
                        y: hy + side * angle.sin(),
                    }
                })
                .collect()
        };

        for row in -max_rows..=max_rows {
            let y = cy + row as f64 * row_spacing;
            let x_offset = if row.rem_euclid(2) == 1 {
                col_spacing / 2.0
            } else {
                0.0
            };
            for col in -max_cols..=max_cols {
                let x = cx + col as f64 * col_spacing + x_offset;
                let dist = ((x - cx).powi(2) + (y - cy).powi(2)).sqrt();
                if dist > radius + side * 0.5 {
                    continue;
                }
                let verts = hex_vertices(x, y);

                let mut outline = verts.clone();
                outline.push(verts[0].clone());
                lines.push(LineShape {
                    points: outline,
                    color: outline_color,
                    dashed: false,
                    binding: None,
                });
            }
        }
    }

    (lines, polygons)
}
