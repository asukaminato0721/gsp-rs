use super::*;

pub(super) struct BindingMaps {
    pub(super) circle_group_to_index: Vec<Option<usize>>,
    pub(super) polygon_group_to_index: Vec<Option<usize>>,
    pub(super) line_group_to_index: Vec<Option<usize>>,
}

fn group_shape_index_map<F>(groups: &[ObjectGroup], predicate: F) -> Vec<Option<usize>>
where
    F: Fn(usize, &ObjectGroup) -> bool,
{
    groups
        .iter()
        .enumerate()
        .filter(|(index, group)| predicate(*index, group))
        .enumerate()
        .fold(
            vec![None; groups.len()],
            |mut acc, (shape_index, (group_index, _))| {
                acc[group_index] = Some(shape_index);
                acc
            },
        )
}

fn circle_group_to_index_map(
    groups: &[ObjectGroup],
    shapes: &CollectedShapes,
) -> Vec<Option<usize>> {
    let mut mapping = vec![None; groups.len()];
    let mut next_index = 0usize;
    for circle in shapes
        .circles
        .iter()
        .chain(shapes.carried_iteration_circles.iter())
        .chain(shapes.translated_circles.iter())
        .chain(shapes.rotated_circles.iter())
        .chain(shapes.transformed_circles.iter())
        .chain(shapes.reflected_circles.iter())
    {
        let Some(group_ordinal) = circle.debug.as_ref().map(|debug| debug.group_ordinal) else {
            next_index += 1;
            continue;
        };
        if let Some(group_index) = group_ordinal.checked_sub(1)
            && group_index < mapping.len()
        {
            mapping[group_index] = Some(next_index);
        }
        next_index += 1;
    }
    mapping
}

fn line_group_to_index_map(groups: &[ObjectGroup], shapes: &CollectedShapes) -> Vec<Option<usize>> {
    let mut mapping = vec![None; groups.len()];
    let mut next_index = 0usize;
    for line in shapes
        .segments
        .iter()
        .chain(shapes.lines.iter())
        .chain(shapes.rays.iter())
        .chain(shapes.translated_lines.iter())
        .chain(shapes.segment_markers.iter())
        .chain(shapes.rotated_lines.iter())
        .chain(shapes.scaled_lines.iter())
        .chain(shapes.reflected_lines.iter())
        .chain(shapes.derived_segments.iter())
        .chain(shapes.measurements.iter())
        .chain(shapes.coordinate_traces.iter())
        .chain(shapes.axes.iter())
        .chain(shapes.iteration_lines.iter())
        .chain(shapes.carried_iteration_lines.iter())
    {
        let Some(group_ordinal) = line.debug.as_ref().map(|debug| debug.group_ordinal) else {
            next_index += 1;
            continue;
        };
        if let Some(group_index) = group_ordinal.checked_sub(1)
            && group_index < mapping.len()
        {
            mapping[group_index] = Some(next_index);
        }
        next_index += 1;
    }
    mapping
}

pub(super) fn remap_scene_bindings(
    file: &GspFile,
    groups: &[ObjectGroup],
    raw_anchors: &[Option<PointRecord>],
    group_to_point_index: &[Option<usize>],
    shapes: &mut CollectedShapes,
) -> (
    BindingMaps,
    Vec<LineIterationFamily>,
    Vec<PolygonIterationFamily>,
) {
    let suppressed_carried_polygon_segments =
        collect_carried_polygon_edge_segment_groups(file, groups);
    let line_group_to_index = line_group_to_index_map(groups, shapes);
    let circle_group_to_index = circle_group_to_index_map(groups, shapes);
    remap_circle_bindings(
        &mut shapes.circles,
        group_to_point_index,
        &circle_group_to_index,
        &line_group_to_index,
    );
    let polygon_group_to_index = group_shape_index_map(groups, |_, group| {
        (group.header.kind()) == crate::format::GroupKind::Polygon
    });
    remap_polygon_bindings(
        &mut shapes.polygons,
        group_to_point_index,
        &polygon_group_to_index,
        &line_group_to_index,
    );
    remap_circle_bindings(
        &mut shapes.translated_circles,
        group_to_point_index,
        &circle_group_to_index,
        &line_group_to_index,
    );
    remap_circle_bindings(
        &mut shapes.rotated_circles,
        group_to_point_index,
        &circle_group_to_index,
        &line_group_to_index,
    );
    remap_circle_bindings(
        &mut shapes.transformed_circles,
        group_to_point_index,
        &circle_group_to_index,
        &line_group_to_index,
    );
    remap_circle_bindings(
        &mut shapes.reflected_circles,
        group_to_point_index,
        &circle_group_to_index,
        &line_group_to_index,
    );
    remap_polygon_bindings(
        &mut shapes.translated_polygons,
        group_to_point_index,
        &polygon_group_to_index,
        &line_group_to_index,
    );
    remap_polygon_bindings(
        &mut shapes.rotated_polygons,
        group_to_point_index,
        &polygon_group_to_index,
        &line_group_to_index,
    );
    remap_polygon_bindings(
        &mut shapes.transformed_polygons,
        group_to_point_index,
        &polygon_group_to_index,
        &line_group_to_index,
    );
    remap_polygon_bindings(
        &mut shapes.reflected_polygons,
        group_to_point_index,
        &polygon_group_to_index,
        &line_group_to_index,
    );
    remap_line_bindings(
        &mut shapes.segments,
        group_to_point_index,
        &line_group_to_index,
    );
    remap_line_bindings(
        &mut shapes.lines,
        group_to_point_index,
        &line_group_to_index,
    );
    remap_line_bindings(&mut shapes.rays, group_to_point_index, &line_group_to_index);
    remap_line_bindings(
        &mut shapes.translated_lines,
        group_to_point_index,
        &line_group_to_index,
    );
    remap_line_bindings(
        &mut shapes.segment_markers,
        group_to_point_index,
        &line_group_to_index,
    );
    remap_line_bindings(
        &mut shapes.rotated_lines,
        group_to_point_index,
        &line_group_to_index,
    );
    remap_line_bindings(
        &mut shapes.scaled_lines,
        group_to_point_index,
        &line_group_to_index,
    );
    remap_line_bindings(
        &mut shapes.reflected_lines,
        group_to_point_index,
        &line_group_to_index,
    );
    remap_line_bindings(
        &mut shapes.coordinate_traces,
        group_to_point_index,
        &line_group_to_index,
    );
    let carried_line_iterations = collect_carried_line_iteration_families(
        file,
        groups,
        raw_anchors,
        group_to_point_index,
        &line_group_to_index,
        &suppressed_carried_polygon_segments,
    );
    let rotational_line_iterations = collect_rotational_line_iteration_families(
        file,
        groups,
        group_to_point_index,
        &line_group_to_index,
    );
    let polygon_iterations =
        collect_carried_polygon_iteration_families(file, groups, raw_anchors, group_to_point_index);
    let mut line_iterations = rotational_line_iterations;
    line_iterations.extend(carried_line_iterations);

    (
        BindingMaps {
            circle_group_to_index,
            polygon_group_to_index,
            line_group_to_index,
        },
        line_iterations,
        polygon_iterations,
    )
}

pub(super) fn apply_payload_color_bindings(
    file: &GspFile,
    groups: &[ObjectGroup],
    raw_anchors: &[Option<PointRecord>],
    group_to_point_index: &[Option<usize>],
    circle_group_to_index: &[Option<usize>],
    polygon_group_to_index: &[Option<usize>],
    shapes: &mut CollectedShapes,
) {
    for group in groups.iter().filter(|group| {
        matches!(
            group.header.kind(),
            GroupKind::DerivedSegment24 | GroupKind::DerivedSegment75
        )
    }) {
        let Some(path) = find_indexed_path(file, group) else {
            continue;
        };
        if path.refs.len() < 4 {
            continue;
        }

        let Some(host_group_index) = path.refs[0].checked_sub(1) else {
            continue;
        };
        let Some(host_group) = groups.get(host_group_index) else {
            continue;
        };
        if host_group.header.kind() == GroupKind::Polygon {
            let Some(polygon_index) = polygon_group_to_index
                .get(host_group_index)
                .copied()
                .flatten()
            else {
                continue;
            };
            let Some(polygon) = shapes.polygons.get_mut(polygon_index) else {
                continue;
            };
            let color = crate::runtime::geometry::color_from_style(group.header.style_b);
            polygon.color = [color[0], color[1], color[2], polygon.color[3]];
            polygon.visible = !group.header.is_hidden();
            continue;
        }
        if host_group.header.kind() != GroupKind::CircleInterior {
            continue;
        }

        let Some(circle_path) = find_indexed_path(file, host_group) else {
            continue;
        };
        let Some(circle_group_index) = circle_path
            .refs
            .first()
            .and_then(|value| value.checked_sub(1))
        else {
            continue;
        };
        let Some(circle_index) = circle_group_to_index
            .get(circle_group_index)
            .copied()
            .flatten()
        else {
            continue;
        };

        let resolve_parameter_point = |ordinal: usize| -> Option<usize> {
            let anchor_group = groups.get(ordinal.checked_sub(1)?)?;
            let anchor_path = find_indexed_path(file, anchor_group)?;
            let point_group_index = anchor_path
                .refs
                .first()
                .and_then(|value| value.checked_sub(1))?;
            group_to_point_index
                .get(point_group_index)
                .copied()
                .flatten()
        };

        let Some(first_point_index) = resolve_parameter_point(path.refs[3]) else {
            continue;
        };
        let Some(second_point_index) = resolve_parameter_point(path.refs[2]) else {
            continue;
        };
        let Some(third_point_index) = resolve_parameter_point(path.refs[1]) else {
            continue;
        };
        let Some(first_value) =
            resolve_payload_color_parameter_value(file, groups, raw_anchors, path.refs[3])
        else {
            continue;
        };
        let Some(second_value) =
            resolve_payload_color_parameter_value(file, groups, raw_anchors, path.refs[2])
        else {
            continue;
        };
        let Some(third_value) =
            resolve_payload_color_parameter_value(file, groups, raw_anchors, path.refs[1])
        else {
            continue;
        };

        let alpha = shapes
            .circles
            .get(circle_index)
            .and_then(|circle| circle.fill_color.map(|color| color[3]))
            .unwrap_or(255);
        let expected = {
            let color = crate::runtime::geometry::color_from_style(group.header.style_b);
            [color[0], color[1], color[2]]
        };
        let rgb_candidate = normalized_rgb(first_value, second_value, third_value);
        let hsb_candidate = normalized_hsb(first_value, second_value, third_value);
        let (binding, resolved_fill) =
            if color_distance(expected, rgb_candidate) <= color_distance(expected, hsb_candidate) {
                (
                    ColorBinding::Rgb {
                        red_point_index: first_point_index,
                        green_point_index: second_point_index,
                        blue_point_index: third_point_index,
                        alpha,
                    },
                    [rgb_candidate[0], rgb_candidate[1], rgb_candidate[2], alpha],
                )
            } else {
                (
                    ColorBinding::Hsb {
                        hue_point_index: first_point_index,
                        saturation_point_index: second_point_index,
                        brightness_point_index: third_point_index,
                        alpha,
                    },
                    [hsb_candidate[0], hsb_candidate[1], hsb_candidate[2], alpha],
                )
            };

        if let Some(circle) = shapes.circles.get_mut(circle_index) {
            circle.fill_color = Some(resolved_fill);
            circle.fill_visible = true;
            circle.fill_color_binding = Some(binding);
        }
    }
}

fn resolve_payload_color_parameter_value(
    file: &GspFile,
    groups: &[ObjectGroup],
    raw_anchors: &[Option<PointRecord>],
    anchor_ordinal: usize,
) -> Option<f64> {
    let anchor_group = groups.get(anchor_ordinal.checked_sub(1)?)?;
    let anchor_path = find_indexed_path(file, anchor_group)?;
    let point_group_index = anchor_path
        .refs
        .first()
        .and_then(|value| value.checked_sub(1))?;
    let point_group = groups.get(point_group_index)?;
    match try_decode_point_constraint(file, groups, point_group, None, &None).ok()? {
        RawPointConstraint::Segment(constraint) => Some(constraint.t),
        RawPointConstraint::ConstructedLine { t, .. } => Some(t),
        RawPointConstraint::PolygonBoundary {
            edge_index,
            t,
            vertex_group_indices,
        } => polygon_boundary_parameter(raw_anchors, &vertex_group_indices, edge_index, t),
        RawPointConstraint::TranslatedPolygonBoundary {
            edge_index,
            t,
            vertex_group_indices,
            ..
        } => polygon_boundary_parameter(raw_anchors, &vertex_group_indices, edge_index, t),
        RawPointConstraint::Circle(constraint) => circle_parameter(
            raw_anchors,
            constraint.center_group_index,
            constraint.radius_group_index,
            constraint.unit_x,
            constraint.unit_y,
        ),
        RawPointConstraint::Circular(_) => None,
        RawPointConstraint::CircleArc(constraint) => Some(constraint.t),
        RawPointConstraint::Arc(constraint) => Some(constraint.t),
        RawPointConstraint::Polyline { t, .. } => Some(t),
    }
}

fn color_distance(expected: [u8; 3], candidate: [u8; 3]) -> u32 {
    u32::from(expected[0].abs_diff(candidate[0]))
        + u32::from(expected[1].abs_diff(candidate[1]))
        + u32::from(expected[2].abs_diff(candidate[2]))
}

fn normalized_channel(value: f64) -> u8 {
    (value.clamp(0.0, 1.0) * 255.0).floor() as u8
}

fn normalized_rgb(first: f64, second: f64, third: f64) -> [u8; 3] {
    [
        normalized_channel(first),
        normalized_channel(second),
        normalized_channel(third),
    ]
}

pub(super) fn normalized_hsb(hue: f64, saturation: f64, brightness: f64) -> [u8; 3] {
    let hue = hue.rem_euclid(1.0);
    let saturation = saturation.clamp(0.0, 1.0);
    let brightness = brightness.clamp(0.0, 1.0);
    if saturation <= 1e-9 {
        let channel = normalized_channel(brightness);
        return [channel, channel, channel];
    }
    let scaled = hue * 6.0;
    let sector = scaled.floor() as usize % 6;
    let fraction = scaled - scaled.floor();
    let p = brightness * (1.0 - saturation);
    let q = brightness * (1.0 - saturation * fraction);
    let t = brightness * (1.0 - saturation * (1.0 - fraction));
    let (red, green, blue) = match sector {
        0 => (brightness, t, p),
        1 => (q, brightness, p),
        2 => (p, brightness, t),
        3 => (p, q, brightness),
        4 => (t, p, brightness),
        _ => (brightness, p, q),
    };
    [
        normalized_channel(red),
        normalized_channel(green),
        normalized_channel(blue),
    ]
}
