use std::collections::BTreeMap;

use crate::format::{GspFile, ObjectGroup, PointRecord};
use crate::runtime::extract::find_indexed_path;
use crate::runtime::geometry::{
    Bounds, GraphTransform, has_distinct_points, include_line_bounds, to_raw_from_world,
};
use crate::runtime::payload_consts::RECORD_FUNCTION_PLOT_DESCRIPTOR;
use crate::runtime::scene::{LineShape, TextLabel};

use super::decode::{try_decode_function_expr, try_decode_function_plot_descriptor};
use super::eval::sample_function_points;
use super::expr::{FunctionPlotMode, function_expr_label_with_variable, function_variable_symbol};

pub(crate) fn collect_function_plots(
    file: &GspFile,
    groups: &[ObjectGroup],
    graph: &Option<GraphTransform>,
) -> Vec<LineShape> {
    let Some(transform) = graph.as_ref() else {
        return Vec::new();
    };

    let mut plots = Vec::new();
    for group in groups
        .iter()
        .filter(|group| (group.header.kind()) == crate::format::GroupKind::FunctionPlot)
    {
        let Some(path) = find_indexed_path(file, group) else {
            continue;
        };
        if path.refs.len() < 2 {
            continue;
        }

        let Some(definition_index) = path.refs[0].checked_sub(1) else {
            continue;
        };
        let Some(definition_group) = groups.get(definition_index) else {
            continue;
        };
        let Some(descriptor_record) = group
            .records
            .iter()
            .find(|record| record.record_type == RECORD_FUNCTION_PLOT_DESCRIPTOR)
        else {
            continue;
        };
        let Some(descriptor) =
            try_decode_function_plot_descriptor(descriptor_record.payload(&file.data)).ok()
        else {
            continue;
        };
        let Some(expr) = try_decode_function_expr(file, groups, definition_group).ok() else {
            continue;
        };

        for mut points in sample_function_points(&expr, &descriptor) {
            if !has_distinct_points(&points) {
                continue;
            }

            for point in &mut points {
                *point = to_raw_from_world(point, transform);
            }

            plots.push(LineShape {
                points,
                color: crate::runtime::geometry::color_from_style(group.header.style_b),
                dashed: false,
                visible: !group.header.is_hidden(),
                binding: None,
                debug: None,
            });
        }
    }

    plots
}

pub(crate) fn collect_function_plot_domain(
    file: &GspFile,
    groups: &[ObjectGroup],
) -> Option<(f64, f64)> {
    let mut min_x = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut found = false;
    for group in groups
        .iter()
        .filter(|group| (group.header.kind()) == crate::format::GroupKind::FunctionPlot)
    {
        let Some(descriptor_record) = group
            .records
            .iter()
            .find(|record| record.record_type == RECORD_FUNCTION_PLOT_DESCRIPTOR)
        else {
            continue;
        };
        let Some(descriptor) =
            try_decode_function_plot_descriptor(descriptor_record.payload(&file.data)).ok()
        else {
            continue;
        };
        if descriptor.mode != FunctionPlotMode::Cartesian {
            continue;
        }
        min_x = min_x.min(descriptor.x_min);
        max_x = max_x.max(descriptor.x_max);
        found = true;
    }
    found.then_some((min_x, max_x))
}

pub(crate) fn synthesize_function_axes(
    function_plots: &[LineShape],
    domain: Option<(f64, f64)>,
    viewport: Option<Bounds>,
    graph: &Option<GraphTransform>,
) -> Vec<LineShape> {
    let Some(mut world_bounds) =
        viewport.or_else(|| bounds_from_function_plots(function_plots, domain, graph))
    else {
        return Vec::new();
    };
    if (world_bounds.max_y - world_bounds.min_y).abs() < 1e-6 {
        world_bounds.min_y -= 1.0;
        world_bounds.max_y += 1.0;
    }
    if (world_bounds.max_x - world_bounds.min_x).abs() < 1e-6 {
        world_bounds.min_x -= 1.0;
        world_bounds.max_x += 1.0;
    }

    let mut axes = Vec::new();
    if world_bounds.min_x <= 0.0 && 0.0 <= world_bounds.max_x {
        axes.push(LineShape {
            points: vec![
                PointRecord {
                    x: 0.0,
                    y: world_bounds.min_y,
                },
                PointRecord {
                    x: 0.0,
                    y: world_bounds.max_y,
                },
            ]
            .into_iter()
            .map(|point| {
                to_raw_from_world(
                    &point,
                    graph
                        .as_ref()
                        .expect("graph transform required for synthetic axes"),
                )
            })
            .collect(),
            color: [192, 192, 192, 255],
            dashed: false,
            visible: true,
            binding: None,
            debug: None,
        });
    }
    if world_bounds.min_y <= 0.0 && 0.0 <= world_bounds.max_y {
        axes.push(LineShape {
            points: vec![
                PointRecord {
                    x: world_bounds.min_x,
                    y: 0.0,
                },
                PointRecord {
                    x: world_bounds.max_x,
                    y: 0.0,
                },
            ]
            .into_iter()
            .map(|point| {
                to_raw_from_world(
                    &point,
                    graph
                        .as_ref()
                        .expect("graph transform required for synthetic axes"),
                )
            })
            .collect(),
            color: [192, 192, 192, 255],
            dashed: false,
            visible: true,
            binding: None,
            debug: None,
        });
    }

    axes
}

pub(crate) fn synthesize_function_labels(
    file: &GspFile,
    groups: &[ObjectGroup],
    function_plots: &[LineShape],
    viewport: Option<Bounds>,
    graph: &Option<GraphTransform>,
) -> Vec<TextLabel> {
    let Some(bounds) = viewport.or_else(|| {
        bounds_from_function_plots(
            function_plots,
            collect_function_plot_domain(file, groups),
            graph,
        )
    }) else {
        return Vec::new();
    };
    let Some(transform) = graph.as_ref() else {
        return Vec::new();
    };

    let parameter_entries = groups
        .iter()
        .filter(|group| (group.header.kind()) == crate::format::GroupKind::FunctionPlot)
        .filter_map(|group| {
            let path = find_indexed_path(file, group)?;
            let definition_group = groups.get(path.refs.first()?.checked_sub(1)?)?;
            Some(super::scene::collect_parameter_bindings(
                file,
                groups,
                definition_group,
            ))
        })
        .fold(BTreeMap::<String, f64>::new(), |mut acc, bindings| {
            for binding in bindings.into_values() {
                acc.entry(binding.name).or_insert(binding.value);
            }
            acc
        })
        .into_iter()
        .collect::<Vec<_>>();

    let base_entries = groups
        .iter()
        .filter(|group| (group.header.kind()) == crate::format::GroupKind::FunctionPlot)
        .filter_map(|group| {
            let path = find_indexed_path(file, group)?;
            let definition_ordinal = *path.refs.first()?;
            let definition_group = groups.get(definition_ordinal.checked_sub(1)?)?;
            let expr = try_decode_function_expr(file, groups, definition_group).ok()?;
            let descriptor_record = group
                .records
                .iter()
                .find(|record| record.record_type == RECORD_FUNCTION_PLOT_DESCRIPTOR)?;
            let descriptor =
                try_decode_function_plot_descriptor(descriptor_record.payload(&file.data)).ok()?;
            let name = if let Some(record) = definition_group
                .records
                .iter()
                .find(|record| record.record_type == 0x07d5)
            {
                let name = {
                    let payload = record.payload(&file.data);
                    if payload.len() < 24 {
                        None
                    } else {
                        let len = crate::format::read_u16(payload, 22) as usize;
                        if 24 + len > payload.len() {
                            None
                        } else {
                            Some(String::from_utf8_lossy(&payload[24..24 + len]).to_string())
                        }
                    }
                };
                name.filter(|candidate| candidate.chars().all(|ch| ch.is_ascii_alphabetic()))
                    .unwrap_or_default()
            } else {
                String::new()
            };
            Some((definition_ordinal, name, expr, descriptor))
        })
        .collect::<Vec<_>>();

    let total = base_entries.len();
    let mut labels = parameter_entries
        .iter()
        .enumerate()
        .map(|(index, (name, value))| {
            let span_x = (bounds.max_x - bounds.min_x).max(1.0);
            let span_y = (bounds.max_y - bounds.min_y).max(1.0);
            let world_anchor = PointRecord {
                x: bounds.min_x + span_x * 0.18,
                y: bounds.max_y - span_y * (0.08 + 0.11 * index as f64),
            };
            TextLabel {
                anchor: to_raw_from_world(&world_anchor, transform),
                text: format!("{name} = {:.2}", value),
                rich_markup: None,
                color: [30, 30, 30, 255],
                visible: true,
                binding: Some(crate::runtime::scene::TextLabelBinding::ParameterValue {
                    name: name.clone(),
                }),
                screen_space: false,
                hotspots: Vec::new(),
                debug: None,
            }
        })
        .collect::<Vec<_>>();
    let parameter_count = labels.len();
    labels.extend(base_entries.iter().enumerate().map(
        |(index, (definition_ordinal, source_name, expr, descriptor))| {
            let span_x = (bounds.max_x - bounds.min_x).max(1.0);
            let span_y = (bounds.max_y - bounds.min_y).max(1.0);
            let world_anchor = PointRecord {
                x: bounds.min_x + span_x * 0.18,
                y: bounds.max_y - span_y * (0.16 + 0.11 * (index + parameter_count) as f64),
            };
            let definition_group = groups
                .get(definition_ordinal.saturating_sub(1))
                .expect("function definition ordinal should resolve");
            let name = if source_name.is_empty() {
                super::scene::function_name_for_definition(
                    file,
                    definition_group,
                    index,
                    total,
                    expr,
                )
            } else {
                source_name.clone()
            };
            let variable = function_variable_symbol(descriptor.mode);
            let text = if descriptor.mode == FunctionPlotMode::Polar {
                format!(
                    "r = {}",
                    function_expr_label_with_variable(expr.clone(), variable)
                )
            } else {
                format!(
                    "{}({variable}) = {}",
                    name,
                    function_expr_label_with_variable(expr.clone(), variable)
                )
            };
            TextLabel {
                anchor: to_raw_from_world(&world_anchor, transform),
                text,
                rich_markup: None,
                color: [30, 30, 30, 255],
                visible: true,
                binding: Some(crate::runtime::scene::TextLabelBinding::FunctionLabel {
                    function_key: *definition_ordinal,
                    derivative: false,
                }),
                screen_space: false,
                hotspots: Vec::new(),
                debug: None,
            }
        },
    ));

    let derivative_entries = groups
        .iter()
        .filter(|group| (group.header.kind()) == crate::format::GroupKind::DerivativeFunction)
        .filter_map(|group| {
            let path = find_indexed_path(file, group)?;
            let base_definition_ordinal = *path.refs.first()?;
            let base_index = base_entries
                .iter()
                .position(|(definition_ordinal, _, _, _)| {
                    *definition_ordinal == base_definition_ordinal
                })?;
            let expr = try_decode_function_expr(file, groups, group).ok()?;
            Some((base_index, expr))
        })
        .collect::<Vec<_>>();

    let span_x = (bounds.max_x - bounds.min_x).max(1.0);
    let span_y = (bounds.max_y - bounds.min_y).max(1.0);
    let base_count = labels.len();
    labels.extend(derivative_entries.into_iter().enumerate().map(
        |(offset, (base_index, expr))| {
            let label_index = base_count + offset;
            let world_anchor = PointRecord {
                x: bounds.min_x + span_x * 0.18,
                y: bounds.max_y - span_y * (0.16 + 0.11 * label_index as f64),
            };
            TextLabel {
                anchor: to_raw_from_world(&world_anchor, transform),
                text: if base_entries[base_index].3.mode == FunctionPlotMode::Polar {
                    format!(
                        "r'({}) = {}",
                        function_variable_symbol(base_entries[base_index].3.mode),
                        function_expr_label_with_variable(
                            expr,
                            function_variable_symbol(base_entries[base_index].3.mode),
                        )
                    )
                } else {
                    format!(
                        "{}'({}) = {}",
                        base_entries[base_index].1,
                        function_variable_symbol(base_entries[base_index].3.mode),
                        function_expr_label_with_variable(
                            expr,
                            function_variable_symbol(base_entries[base_index].3.mode),
                        )
                    )
                },
                rich_markup: None,
                color: [30, 30, 30, 255],
                visible: true,
                binding: Some(crate::runtime::scene::TextLabelBinding::FunctionLabel {
                    function_key: base_entries[base_index].0,
                    derivative: true,
                }),
                screen_space: false,
                hotspots: Vec::new(),
                debug: None,
            }
        },
    ));

    labels
}

pub(super) fn bounds_from_function_plots(
    function_plots: &[LineShape],
    domain: Option<(f64, f64)>,
    graph: &Option<GraphTransform>,
) -> Option<Bounds> {
    let first = if let Some(line) = function_plots.first() {
        line.points.first()
    } else {
        None
    };
    let mut bounds = if let Some(first) = first {
        let first = crate::runtime::geometry::to_world(first, graph);
        Bounds {
            min_x: first.x,
            max_x: first.x,
            min_y: first.y,
            max_y: first.y,
        }
    } else if let Some((min_x, max_x)) = domain {
        Bounds {
            min_x,
            max_x,
            min_y: 0.0,
            max_y: 0.0,
        }
    } else {
        return None;
    };
    include_line_bounds(&mut bounds, function_plots, graph);
    if let Some((min_x, max_x)) = domain {
        bounds.min_x = bounds.min_x.min(min_x);
        bounds.max_x = bounds.max_x.max(max_x);
    }
    Some(bounds)
}
