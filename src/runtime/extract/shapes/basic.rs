use std::collections::{BTreeMap, BTreeSet};

use super::{
    CircleShape, GraphTransform, GspFile, LineBinding, LineShape, ObjectGroup, PointRecord,
    PolygonShape, color_from_style, decode_function_expr, decode_function_plot_descriptor,
    decode_label_name, evaluate_expr_with_parameters, fill_color_from_styles, find_indexed_path,
    has_distinct_points, to_raw_from_world,
};

pub(crate) fn collect_line_shapes(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
    kinds: &[u32],
    fallback_generic: bool,
) -> Vec<LineShape> {
    groups
        .iter()
        .filter(|group| {
            let kind = group.header.class_id & 0xffff;
            kinds.contains(&kind)
                || (fallback_generic
                    && matches!(kind, 2 | 5 | 6 | 7)
                    && find_indexed_path(file, group)
                        .map(|path| path.refs.len() == 2)
                        .unwrap_or(false))
        })
        .filter_map(|group| {
            let path = find_indexed_path(file, group)?;
            let points = path
                .refs
                .iter()
                .filter_map(|object_ref| {
                    anchors.get(object_ref.saturating_sub(1)).cloned().flatten()
                })
                .collect::<Vec<_>>();
            let start_group_index = path.refs.first().and_then(|ordinal| ordinal.checked_sub(1));
            let end_group_index = path.refs.get(1).and_then(|ordinal| ordinal.checked_sub(1));
            (points.len() >= 2 && has_distinct_points(&points)).then_some(LineShape {
                points,
                color: if fallback_generic && !kinds.contains(&(group.header.class_id & 0xffff)) {
                    [40, 40, 40, 255]
                } else {
                    color_from_style(group.header.style_b)
                },
                dashed: (group.header.class_id & 0xffff) == 58,
                binding: match (
                    group.header.class_id & 0xffff,
                    start_group_index,
                    end_group_index,
                ) {
                    (2, Some(start_index), Some(end_index)) => Some(LineBinding::Segment {
                        start_index,
                        end_index,
                    }),
                    _ => None,
                },
            })
        })
        .collect()
}

pub(crate) fn collect_bound_line_shapes(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
    kind: u32,
) -> Vec<LineShape> {
    groups
        .iter()
        .filter(|group| (group.header.class_id & 0xffff) == kind)
        .filter_map(|group| {
            let path = find_indexed_path(file, group)?;
            if path.refs.len() != 2 {
                return None;
            }
            let start_group_index = path.refs[0].checked_sub(1)?;
            let end_group_index = path.refs[1].checked_sub(1)?;
            let start = anchors.get(start_group_index)?.clone()?;
            let end = anchors.get(end_group_index)?.clone()?;
            has_distinct_points(&[start.clone(), end.clone()]).then_some(LineShape {
                points: vec![start, end],
                color: color_from_style(group.header.style_b),
                dashed: false,
                binding: Some(match kind {
                    63 => LineBinding::Line {
                        start_index: start_group_index,
                        end_index: end_group_index,
                    },
                    64 => LineBinding::Ray {
                        start_index: start_group_index,
                        end_index: end_group_index,
                    },
                    _ => return None,
                }),
            })
        })
        .collect()
}

pub(crate) fn collect_polygon_shapes(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
    kinds: &[u32],
) -> Vec<PolygonShape> {
    groups
        .iter()
        .filter(|group| kinds.contains(&(group.header.class_id & 0xffff)))
        .filter_map(|group| {
            let path = find_indexed_path(file, group)?;
            let points = path
                .refs
                .iter()
                .filter_map(|object_ref| {
                    anchors.get(object_ref.saturating_sub(1)).cloned().flatten()
                })
                .collect::<Vec<_>>();
            (points.len() >= 3).then_some(PolygonShape {
                points,
                color: fill_color_from_styles(group.header.style_b, group.header.style_c),
                binding: None,
            })
        })
        .collect()
}

pub(crate) fn collect_circle_shapes(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
) -> Vec<CircleShape> {
    groups
        .iter()
        .filter(|group| (group.header.class_id & 0xffff) == 3)
        .filter_map(|group| {
            let path = find_indexed_path(file, group)?;
            if path.refs.len() != 2 {
                return None;
            }
            let center = anchors.get(path.refs[0].saturating_sub(1))?.clone()?;
            let radius_point = anchors.get(path.refs[1].saturating_sub(1))?.clone()?;
            Some(CircleShape {
                center,
                radius_point,
                color: color_from_style(group.header.style_b),
                binding: None,
            })
        })
        .collect()
}

pub(crate) fn collect_derived_segments(
    file: &GspFile,
    groups: &[ObjectGroup],
    point_map: &[Option<PointRecord>],
    kinds: &[u32],
) -> Vec<LineShape> {
    let refs = groups
        .iter()
        .map(|group| {
            find_indexed_path(file, group)
                .map(|path| path.refs)
                .unwrap_or_default()
        })
        .collect::<Vec<_>>();
    let class_ids = groups
        .iter()
        .map(|group| group.header.class_id & 0xffff)
        .collect::<Vec<_>>();

    fn descend_points(
        ordinal: usize,
        refs: &[Vec<usize>],
        point_map: &[Option<PointRecord>],
        memo: &mut Vec<Option<Vec<PointRecord>>>,
        visiting: &mut BTreeSet<usize>,
    ) -> Vec<PointRecord> {
        if let Some(cached) = &memo[ordinal - 1] {
            return cached.clone();
        }
        if !visiting.insert(ordinal) {
            return Vec::new();
        }

        let mut points = Vec::new();
        if let Some(point) = point_map.get(ordinal - 1).and_then(|point| point.clone()) {
            points.push(point);
        } else {
            for child in &refs[ordinal - 1] {
                if *child > 0 && *child <= refs.len() {
                    points.extend(descend_points(*child, refs, point_map, memo, visiting));
                }
            }
        }

        visiting.remove(&ordinal);
        points.sort_by(|a, b| {
            a.x.partial_cmp(&b.x)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.y.partial_cmp(&b.y).unwrap_or(std::cmp::Ordering::Equal))
        });
        points.dedup_by(|a, b| (a.x - b.x).abs() < 0.001 && (a.y - b.y).abs() < 0.001);
        memo[ordinal - 1] = Some(points.clone());
        points
    }

    let mut memo = vec![None; groups.len()];
    let mut seen = BTreeSet::<((i32, i32), (i32, i32))>::new();
    let mut segments = Vec::new();

    for (index, class_id) in class_ids.iter().enumerate() {
        if !kinds.contains(class_id) {
            continue;
        }
        let points = descend_points(index + 1, &refs, point_map, &mut memo, &mut BTreeSet::new());
        if points.len() < 2 || points.len() > 12 {
            continue;
        }

        let mut best = None;
        let mut best_dist = -1.0_f64;
        for i in 0..points.len() {
            for j in i + 1..points.len() {
                let dx = points[i].x - points[j].x;
                let dy = points[i].y - points[j].y;
                let dist = dx * dx + dy * dy;
                if dist > best_dist {
                    best_dist = dist;
                    best = Some((points[i].clone(), points[j].clone()));
                }
            }
        }

        let Some((a, b)) = best else { continue };
        let a_key = (a.x.round() as i32, a.y.round() as i32);
        let b_key = (b.x.round() as i32, b.y.round() as i32);
        let key = if a_key <= b_key {
            (a_key, b_key)
        } else {
            (b_key, a_key)
        };
        if !seen.insert(key) {
            continue;
        }

        let color = match *class_id {
            24 => [20, 20, 20, 255],
            48 => [70, 70, 70, 255],
            75 => [120, 120, 120, 255],
            _ => [60, 60, 60, 255],
        };
        segments.push(LineShape {
            points: vec![a, b],
            color,
            dashed: false,
            binding: None,
        });
    }

    segments
}

pub(crate) fn collect_coordinate_traces(
    file: &GspFile,
    groups: &[ObjectGroup],
    graph: &Option<GraphTransform>,
) -> Vec<LineShape> {
    groups
        .iter()
        .filter(|group| (group.header.class_id & 0xffff) == 97)
        .filter_map(|group| {
            let path = find_indexed_path(file, group)?;
            if path.refs.len() < 3 {
                return None;
            }
            let parameter_group = groups.get(path.refs[1].checked_sub(1)?)?;
            let calc_group = groups.get(path.refs[2].checked_sub(1)?)?;
            let parameter_name = decode_label_name(file, parameter_group)?;
            let payload = group
                .records
                .iter()
                .find(|record| record.record_type == 0x0902)
                .map(|record| record.payload(&file.data))?;
            let descriptor = decode_function_plot_descriptor(payload)?;
            let expr = decode_function_expr(file, groups, calc_group)?;

            let mut points = Vec::with_capacity(descriptor.sample_count);
            let last = descriptor.sample_count.saturating_sub(1).max(1) as f64;
            for index in 0..descriptor.sample_count {
                let t = index as f64 / last;
                let x = descriptor.x_min + (descriptor.x_max - descriptor.x_min) * t;
                let parameters = BTreeMap::from([(parameter_name.clone(), x)]);
                let y = evaluate_expr_with_parameters(&expr, 0.0, &parameters)?;
                let world = PointRecord { x, y };
                let point = if let Some(transform) = graph {
                    to_raw_from_world(&world, transform)
                } else {
                    world
                };
                points.push(point);
            }

            (points.len() >= 2).then_some(LineShape {
                points,
                color: color_from_style(group.header.style_b),
                dashed: false,
                binding: None,
            })
        })
        .collect()
}
