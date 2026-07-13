use super::analysis::CollectedShapes;
use super::shapes::collect_carried_polygon_edge_segment_groups;
use super::*;

pub(super) struct BindingMaps {
    pub(super) circle_group_to_index: Vec<Option<usize>>,
    pub(super) polygon_group_to_index: Vec<Option<usize>>,
    pub(super) line_group_to_index: Vec<Option<usize>>,
}

fn circle_group_to_index_map(
    groups: &[ObjectGroup],
    shapes: &CollectedShapes,
) -> Vec<Option<usize>> {
    let mut mapping = vec![None; groups.len()];
    for (shape_index, circle) in shapes.circles.iter().enumerate() {
        let Some(group_ordinal) = circle.debug.as_ref().map(|debug| debug.group_ordinal) else {
            continue;
        };
        if let Some(group_index) = group_ordinal.checked_sub(1)
            && group_index < mapping.len()
        {
            mapping[group_index] = Some(shape_index);
        }
    }
    mapping
}

fn polygon_group_to_index_map(
    groups: &[ObjectGroup],
    shapes: &CollectedShapes,
) -> Vec<Option<usize>> {
    let mut mapping = vec![None; groups.len()];
    for (shape_index, polygon) in shapes.polygons.iter().enumerate() {
        let Some(group_ordinal) = polygon.debug.as_ref().map(|debug| debug.group_ordinal) else {
            continue;
        };
        if let Some(group_index) = group_ordinal.checked_sub(1)
            && group_index < mapping.len()
        {
            mapping[group_index] = Some(shape_index);
        }
    }
    mapping
}

fn map_line_shape(mapping: &mut [Option<usize>], line: &LineShape, shape_index: usize) {
    let Some(group_ordinal) = line.debug.as_ref().map(|debug| debug.group_ordinal) else {
        return;
    };
    if let Some(group_index) = group_ordinal.checked_sub(1)
        && group_index < mapping.len()
    {
        mapping[group_index] = Some(shape_index);
    }
}

fn line_group_to_index_map(
    groups: &[ObjectGroup],
    shapes: &CollectedShapes,
    function_plot_count: usize,
) -> Vec<Option<usize>> {
    let mut mapping = vec![None; groups.len()];
    let mut next_index = 0usize;
    for line in shapes
        .lines
        .iter()
        .chain(shapes.trace_lines.iter())
        .chain(shapes.axes.iter())
    {
        map_line_shape(&mut mapping, line, next_index);
        next_index += 1;
    }
    next_index += function_plot_count;
    for line in &shapes.post_function_lines {
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
    function_plot_count: usize,
    shapes: &mut CollectedShapes,
) -> (
    BindingMaps,
    Vec<LineIterationFamily>,
    Vec<PolygonIterationFamily>,
) {
    let suppressed_carried_polygon_segments =
        collect_carried_polygon_edge_segment_groups(file, groups);
    let line_group_to_index = line_group_to_index_map(groups, shapes, function_plot_count);
    let circle_group_to_index = circle_group_to_index_map(groups, shapes);
    remap_circle_bindings(
        &mut shapes.circles,
        group_to_point_index,
        &circle_group_to_index,
        &line_group_to_index,
    );
    let polygon_group_to_index = polygon_group_to_index_map(groups, shapes);
    remap_polygon_bindings(
        &mut shapes.polygons,
        group_to_point_index,
        &polygon_group_to_index,
        &line_group_to_index,
    );
    remap_line_bindings(
        &mut shapes.lines,
        group_to_point_index,
        &line_group_to_index,
    );
    remap_line_bindings(
        &mut shapes.trace_lines,
        group_to_point_index,
        &line_group_to_index,
    );
    remap_line_bindings(&mut shapes.axes, group_to_point_index, &line_group_to_index);
    remap_line_bindings(
        &mut shapes.post_function_lines,
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
            let Some(parameter_ordinal) = path.refs.get(1).copied() else {
                continue;
            };
            let Some(point_index) = resolve_color_parameter_point_index(
                file,
                groups,
                group_to_point_index,
                parameter_ordinal,
            ) else {
                continue;
            };
            let Some(base_value) =
                resolve_payload_color_parameter_value(file, groups, raw_anchors, parameter_ordinal)
            else {
                continue;
            };
            let Some((range_start, range_end)) = decode_spectrum_range(file, group) else {
                continue;
            };
            let period = (range_end - range_start).abs();
            if period <= 1e-9 {
                continue;
            }
            polygon.color_binding = Some(ColorBinding::Spectrum {
                point_index,
                base_value,
                period,
                base_color: polygon.color,
            });
            polygon.visible = !group.header.is_hidden();
            continue;
        }
        if path.refs.len() < 4 {
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
        let Some(color_model) = decode_color_model(file, group) else {
            continue;
        };
        let (binding, resolved_fill) = match color_model {
            PayloadColorModel::Rgb => {
                let color = normalized_rgb(first_value, second_value, third_value);
                (
                    ColorBinding::Rgb {
                        red_point_index: first_point_index,
                        green_point_index: second_point_index,
                        blue_point_index: third_point_index,
                        alpha,
                    },
                    [color[0], color[1], color[2], alpha],
                )
            }
            PayloadColorModel::Hsv => {
                let color = normalized_hsb(first_value, second_value, third_value);
                (
                    ColorBinding::Hsb {
                        hue_point_index: first_point_index,
                        saturation_point_index: second_point_index,
                        brightness_point_index: third_point_index,
                        alpha,
                    },
                    [color[0], color[1], color[2], alpha],
                )
            }
        };

        if let Some(circle) = shapes.circles.get_mut(circle_index) {
            circle.fill_color = Some(resolved_fill);
            circle.fill_visible = true;
            circle.fill_color_binding = Some(binding);
        }
    }
}

fn resolve_color_parameter_point_index(
    file: &GspFile,
    groups: &[ObjectGroup],
    group_to_point_index: &[Option<usize>],
    anchor_ordinal: usize,
) -> Option<usize> {
    let anchor_group = groups.get(anchor_ordinal.checked_sub(1)?)?;
    let anchor_path = find_indexed_path(file, anchor_group)?;
    let point_group_index = anchor_path.refs.first()?.checked_sub(1)?;
    group_to_point_index
        .get(point_group_index)
        .copied()
        .flatten()
}

fn decode_spectrum_range(file: &GspFile, group: &ObjectGroup) -> Option<(f64, f64)> {
    let payload = group
        .records
        .iter()
        .find(|record| {
            record.record_type == crate::runtime::payload_consts::RECORD_BINDING_PAYLOAD
        })?
        .payload(&file.data);
    if payload.len() < 24 {
        return None;
    }
    let start = crate::runtime::extract::read_f64(payload, 8);
    let end = crate::runtime::extract::read_f64(payload, 16);
    (start.is_finite() && end.is_finite()).then_some((start, end))
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PayloadColorModel {
    Rgb,
    Hsv,
}

const COLORIZED_RGB_OPCODE: u32 = 0x32;
const COLORIZED_HSV_OPCODE: u32 = 0x42;

fn decode_color_model(file: &GspFile, group: &ObjectGroup) -> Option<PayloadColorModel> {
    let payload = group
        .records
        .iter()
        .find(|record| record.record_type == RECORD_BINDING_PAYLOAD)?
        .payload(&file.data);
    if payload.len() < 8 || read_u32(payload, 0) != u32::from(GroupKind::DerivedSegment75.raw()) {
        return None;
    }
    match read_u32(payload, 4) {
        COLORIZED_RGB_OPCODE => Some(PayloadColorModel::Rgb),
        COLORIZED_HSV_OPCODE => Some(PayloadColorModel::Hsv),
        _ => None,
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn color_model(path: &str, ordinal: usize) -> PayloadColorModel {
        let data = std::fs::read(path).expect("color fixture");
        let file = GspFile::parse(&data).expect("valid gsp fixture");
        let group = file
            .object_groups()
            .into_iter()
            .find(|group| group.ordinal == ordinal)
            .expect("color binding group");
        decode_color_model(&file, &group).expect("explicit color model opcode")
    }

    #[test]
    fn decodes_rgb_and_hsv_from_payload_opcodes() {
        assert_eq!(
            color_model(
                "tests/Samples/热研系列/迭代系列/m×n网络交错填充砌墙.gsp",
                41,
            ),
            PayloadColorModel::Rgb,
        );
        assert_eq!(
            color_model("tests/Samples/个人专栏/向忠作品/正二十面体.gsp", 333),
            PayloadColorModel::Hsv,
        );
    }
}
