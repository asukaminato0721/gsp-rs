use super::*;
use crate::runtime::payload_consts::RECORD_RICH_TEXT;
use std::fs;
use std::path::Path;

pub(super) fn read_reference_htm_construction_lines(source_path: &Path) -> Option<Vec<String>> {
    let htm = fs::read_to_string(source_path.with_extension("htm")).ok()?;
    let marker = "<PARAM NAME=Construction VALUE=\"";
    let start = htm.find(marker)? + marker.len();
    let end = htm[start..].find("\">")?;
    Some(
        htm[start..start + end]
            .replace("&#xD;", "\n")
            .replace("&quot;", "\"")
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(str::to_string)
            .collect(),
    )
}

pub(super) fn collect_htm_payload_groups<'a>(
    file: &GspFile,
    groups: &'a [ObjectGroup],
) -> Vec<&'a ObjectGroup> {
    let blocked_ordinals = htm_payload_blocked_ordinals(file, groups);
    groups
        .iter()
        .filter(|group| {
            !blocked_ordinals.contains(&group.ordinal)
                || htm_blocked_group_still_renders(file, group, &blocked_ordinals)
        })
        .filter(|group| match group.header.kind() {
            GroupKind::Point => {
                group
                    .records
                    .iter()
                    .any(|record| record.record_type == RECORD_POINT_F64_PAIR)
                    || decode::is_parameter_control_group(group)
                    || (!group.header.is_hidden() && try_decode_group_label_text_placeholder(group))
            }
            GroupKind::PointConstraint | GroupKind::PathPoint => find_indexed_path(file, group)
                .and_then(|path| path.refs.first().copied())
                .is_some_and(|host| {
                    host != group.ordinal
                        && groups.get(host.saturating_sub(1)).is_none_or(|host_group| {
                            !matches!(
                                host_group.header.kind(),
                                GroupKind::ArcOnCircle
                                    | GroupKind::CenterArc
                                    | GroupKind::ThreePointArc
                            )
                        })
                }),
            GroupKind::Midpoint
            | GroupKind::LinearIntersectionPoint
            | GroupKind::IntersectionPoint1
            | GroupKind::IntersectionPoint2
            | GroupKind::CircleCircleIntersectionPoint1
            | GroupKind::CircleCircleIntersectionPoint2
            | GroupKind::GraphCalibrationX
            | GroupKind::GraphCalibrationY
            | GroupKind::GraphCalibrationYAlt
            | GroupKind::Segment
            | GroupKind::Circle
            | GroupKind::CircleCenterRadius
            | GroupKind::CircleInterior
            | GroupKind::Line
            | GroupKind::PerpendicularLine
            | GroupKind::ParallelLine
            | GroupKind::AngleBisectorRay
            | GroupKind::MeasurementLine
            | GroupKind::AxisLine
            | GroupKind::GraphMeasurementSegment
            | GroupKind::Ray
            | GroupKind::Polygon
            | GroupKind::CartesianOffsetPoint
            | GroupKind::PolarOffsetPoint
            | GroupKind::Translation
            | GroupKind::Rotation
            | GroupKind::ParameterRotation
            | GroupKind::Scale
            | GroupKind::RatioScale
            | GroupKind::PointTrace
            | GroupKind::CoordinatePoint
            | GroupKind::CoordinateReadoutLabel
            | GroupKind::CoordinateXValue
            | GroupKind::CoordinateYValue
            | GroupKind::DistanceValue
            | GroupKind::GraphDistanceValue
            | GroupKind::FunctionPlot
            | GroupKind::LegacyFunctionPlot
            | GroupKind::FunctionDefinition
            | GroupKind::FunctionExpr
            | GroupKind::RichTextLabel => true,
            GroupKind::ActionButton => {
                matches!(htm_action_button_kind(file, group), Some((0, 7) | (2, 0)))
            }
            _ => false,
        })
        .collect()
}

fn htm_blocked_group_still_renders(
    file: &GspFile,
    group: &ObjectGroup,
    blocked_ordinals: &BTreeSet<usize>,
) -> bool {
    group.header.kind() == GroupKind::ActionButton
        && htm_action_button_kind(file, group) == Some((2, 0))
        && find_indexed_path(file, group).is_some_and(|path| {
            !path
                .refs
                .iter()
                .any(|reference| blocked_ordinals.contains(reference))
        })
}

fn htm_payload_blocked_ordinals(file: &GspFile, groups: &[ObjectGroup]) -> BTreeSet<usize> {
    let mut blocked = BTreeSet::new();
    if let Some(iteration_ordinal) = groups
        .iter()
        .find(|group| group.header.kind() == GroupKind::RegularPolygonIteration)
        .map(|group| group.ordinal)
    {
        blocked.extend(
            groups
                .iter()
                .filter(|group| group.ordinal >= iteration_ordinal)
                .map(|group| group.ordinal),
        );
    }
    for group in groups {
        if htm_group_is_payload_blocking_seed(file, groups, group) {
            blocked.insert(group.ordinal);
        }
    }
    let mut changed = true;
    while changed {
        changed = false;
        for group in groups {
            if blocked.contains(&group.ordinal) {
                continue;
            }
            if find_indexed_path(file, group).is_some_and(|path| {
                path.refs
                    .iter()
                    .any(|reference| blocked.contains(reference))
            }) {
                changed |= blocked.insert(group.ordinal);
            }
        }
    }
    blocked
}

fn htm_group_is_payload_blocking_seed(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
) -> bool {
    if htm_point_is_image_anchor(file, group) {
        return true;
    }
    if group.header.kind() == GroupKind::PathPoint
        && group.records.iter().any(|record| {
            matches!(
                record.record_type,
                crate::runtime::payload_consts::RECORD_LABEL_AUX
                    | crate::runtime::payload_consts::RECORD_PATH_POINT_AUX
            )
        })
    {
        return true;
    }
    if matches!(
        group.header.kind(),
        GroupKind::PointConstraint | GroupKind::PathPoint | GroupKind::ParameterControlledPoint
    ) && find_indexed_path(file, group)
        .and_then(|path| path.refs.first().copied())
        .and_then(|host| groups.get(host.saturating_sub(1)))
        .is_some_and(|host_group| {
            matches!(
                host_group.header.kind(),
                GroupKind::ArcOnCircle
                    | GroupKind::CenterArc
                    | GroupKind::ThreePointArc
                    | GroupKind::SectorBoundary
                    | GroupKind::CircularSegmentBoundary
            )
        })
    {
        return true;
    }
    group.header.kind() == GroupKind::FunctionExpr
        && find_indexed_path(file, group)
            .and_then(|path| path.refs.first().copied())
            .and_then(|reference| groups.get(reference.saturating_sub(1)))
            .is_some_and(|source| source.header.kind() == GroupKind::ParameterAnchor)
}

fn try_decode_group_label_text_placeholder(group: &ObjectGroup) -> bool {
    group.records.iter().any(|record| {
        matches!(
            record.record_type,
            RECORD_LABEL_AUX | 0x08fc | 0x08fd | 0x08fe | 0x08ff
        )
    })
}

pub(super) struct HtmPayloadContext<'a> {
    pub(super) ordinal_map: &'a BTreeMap<usize, usize>,
    pub(super) graph: Option<&'a GraphTransform>,
    pub(super) has_point_function_plot: bool,
}

pub(super) fn describe_group_as_htm_payload(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
    ordinal: usize,
    context: &HtmPayloadContext<'_>,
) -> String {
    let refs = find_indexed_path(file, group)
        .map(|path| path.refs)
        .unwrap_or_default();
    let (object_type, args) = htm_payload_signature(file, groups, group, &refs, context);
    let attrs = htm_payload_attributes(file, groups, group, context);
    if object_type == "Function" {
        if let Some((function_args, ref_args)) = args.split_once('\u{1f}') {
            if attrs.is_empty() {
                return format!("{{{ordinal}}} {object_type}({function_args})({ref_args});");
            }
            return format!("{{{ordinal}}} {object_type}({function_args})({ref_args})[{attrs}];");
        }
        if attrs.is_empty() {
            return format!("{{{ordinal}}} {object_type}({args})();");
        }
        return format!("{{{ordinal}}} {object_type}({args})()[{attrs}];");
    }
    if object_type == "ToggleVisibilityButton"
        && let Some((button_args, target_args)) = args.split_once('\u{1f}')
    {
        if attrs.is_empty() {
            return format!("{{{ordinal}}} {object_type}({button_args})({target_args});");
        }
        return format!("{{{ordinal}}} {object_type}({button_args})({target_args})[{attrs}];");
    }
    if attrs.is_empty() {
        format!("{{{ordinal}}} {object_type}({args});")
    } else {
        format!("{{{ordinal}}} {object_type}({args})[{attrs}];")
    }
}

fn htm_payload_signature(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
    refs: &[usize],
    context: &HtmPayloadContext<'_>,
) -> (&'static str, String) {
    let ordinal_map = context.ordinal_map;
    let graph = context.graph;
    match group.header.kind() {
        GroupKind::Point if decode::is_parameter_control_group(group) => {
            if function_definition_refs_this_group(file, groups, group) {
                ("Function", htm_function_args(file, groups, group))
            } else {
                ("Parameter", htm_parameter_args(file, groups, group))
            }
        }
        GroupKind::Point if htm_point_is_image_anchor(file, group) => {
            ("ImageAnchor", String::new())
        }
        GroupKind::Point if decode_group_point(file, group).is_none() => {
            ("FixedText", htm_fixed_text_args(file, group))
        }
        GroupKind::Point => (
            "Point",
            decode_group_point(file, group)
                .map(|point| format!("{},{}", format_number(point.x), format_number(point.y)))
                .unwrap_or_default(),
        ),
        GroupKind::PointConstraint | GroupKind::PathPoint | GroupKind::ParameterControlledPoint => {
            let t = decode_htm_object_parameter(file, groups, group).unwrap_or(0.0);
            (
                "Point on object",
                refs.first()
                    .map(|host| {
                        format!(
                            "{},{}",
                            map_htm_ordinal(*host, ordinal_map),
                            format_htm_parameter(t)
                        )
                    })
                    .unwrap_or_else(|| format!("0,{}", format_htm_parameter(t))),
            )
        }
        GroupKind::Midpoint => ("Midpoint", format_ref_args(refs, ordinal_map)),
        GroupKind::LinearIntersectionPoint => ("Intersect", format_ref_args(refs, ordinal_map)),
        GroupKind::IntersectionPoint1 | GroupKind::CircleCircleIntersectionPoint1 => {
            ("Intersect1", format_ref_args(refs, ordinal_map))
        }
        GroupKind::IntersectionPoint2 | GroupKind::CircleCircleIntersectionPoint2 => {
            ("Intersect2", format_ref_args(refs, ordinal_map))
        }
        GroupKind::GraphCalibrationX => (
            "UnitPoint",
            format!(
                "{},{}",
                refs.first()
                    .map(|reference| map_htm_ordinal(*reference, ordinal_map))
                    .unwrap_or(0),
                decode_graph_calibration_unit_length(file, group)
                    .or_else(|| graph.map(|graph| graph.raw_per_unit))
                    .map(format_htm_unit_length)
                    .unwrap_or_default()
            ),
        ),
        GroupKind::GraphCalibrationY | GroupKind::GraphCalibrationYAlt => {
            if context.has_point_function_plot {
                (
                    "RectangularUnitPoint",
                    format!(
                        "{},{}",
                        refs.first()
                            .map(|reference| map_htm_ordinal(*reference, ordinal_map))
                            .unwrap_or(0),
                        graph
                            .map(|graph| format_htm_significant(graph.raw_per_unit, 6))
                            .unwrap_or_default()
                    ),
                )
            } else {
                (
                    "SquareUnitPoint",
                    refs.first()
                        .map(|reference| map_htm_ordinal(*reference, ordinal_map).to_string())
                        .unwrap_or_default(),
                )
            }
        }
        GroupKind::Segment => ("Segment", format_reversed_ref_args(refs, ordinal_map)),
        GroupKind::Circle => ("Circle", format_ref_args(refs, ordinal_map)),
        GroupKind::CircleCenterRadius => ("Circle by radius", format_ref_args(refs, ordinal_map)),
        GroupKind::CircleInterior => ("Circle interior", format_ref_args(refs, ordinal_map)),
        GroupKind::Line | GroupKind::GraphMeasurementSegment => {
            ("Line", format_reversed_ref_args(refs, ordinal_map))
        }
        GroupKind::MeasurementLine => match decode::decode_label_name(file, group).as_deref() {
            Some("x") => ("HorizontalAxis", format_ref_args(refs, ordinal_map)),
            Some("y") => ("VerticalAxis", format_ref_args(refs, ordinal_map)),
            _ => ("Line", format_reversed_ref_args(refs, ordinal_map)),
        },
        GroupKind::AxisLine => ("CoordSysByAxes", format_ref_args(refs, ordinal_map)),
        GroupKind::PerpendicularLine => {
            ("Perpendicular", format_reversed_ref_args(refs, ordinal_map))
        }
        GroupKind::ParallelLine => ("Parallel", format_reversed_ref_args(refs, ordinal_map)),
        GroupKind::AngleBisectorRay => ("Bisector", format_ref_args(refs, ordinal_map)),
        GroupKind::Ray => ("Ray", format_reversed_ref_args(refs, ordinal_map)),
        GroupKind::Polygon => ("Polygon", format_ref_args(refs, ordinal_map)),
        GroupKind::CartesianOffsetPoint | GroupKind::PolarOffsetPoint => (
            "Translation",
            htm_offset_point_args(file, group, ordinal_map),
        ),
        GroupKind::Translation => ("VectorTranslation", format_ref_args(refs, ordinal_map)),
        GroupKind::Rotation => ("Rotation", htm_rotation_args(file, group, ordinal_map)),
        GroupKind::ParameterRotation => {
            ("Rotation/MeasuredAngle", format_ref_args(refs, ordinal_map))
        }
        GroupKind::Scale => ("Dilation", htm_scale_args(file, group, ordinal_map)),
        GroupKind::RatioScale => ("Dilation/3PtRatio", format_ref_args(refs, ordinal_map)),
        GroupKind::PointTrace => (
            "Locus",
            htm_locus_args(file, groups, group, refs, ordinal_map),
        ),
        GroupKind::CoordinatePoint => ("PlotXY", htm_plot_xy_args(refs, ordinal_map)),
        GroupKind::CoordinateXValue => (
            "Abscissa",
            htm_measurement_args(file, group, refs, ordinal_map, "x"),
        ),
        GroupKind::CoordinateYValue => (
            "Ordinate",
            htm_measurement_args(file, group, refs, ordinal_map, "y"),
        ),
        GroupKind::CoordinateReadoutLabel => (
            "Coordinates",
            htm_coordinate_readout_args(file, group, refs, ordinal_map),
        ),
        GroupKind::DistanceValue | GroupKind::GraphDistanceValue => (
            "Distance",
            htm_graph_distance_args(file, group, refs, ordinal_map),
        ),
        GroupKind::FunctionPlot | GroupKind::LegacyFunctionPlot => (
            "FunctionPlot",
            htm_function_plot_args(file, group, refs, ordinal_map),
        ),
        GroupKind::FunctionDefinition => (
            "Function",
            htm_function_definition_args(file, groups, group, refs, ordinal_map),
        ),
        GroupKind::FunctionExpr => (
            "Calculate",
            htm_calculate_args(file, groups, group, refs, ordinal_map, graph),
        ),
        GroupKind::ArcOnCircle | GroupKind::CenterArc | GroupKind::ThreePointArc => {
            ("Arc", format_ref_args(refs, ordinal_map))
        }
        GroupKind::RichTextLabel => ("FixedText", htm_fixed_text_args(file, group)),
        GroupKind::ActionButton => match htm_action_button_kind(file, group) {
            Some((2, 0)) => (
                "AnimateButton",
                htm_animate_button_args(file, groups, group, refs, ordinal_map),
            ),
            _ => (
                "ToggleVisibilityButton",
                htm_action_button_args(file, group, refs, ordinal_map),
            ),
        },
        _ => (
            htm_payload_type_name(group.header.kind()),
            format_ref_args(refs, ordinal_map),
        ),
    }
}

fn htm_payload_type_name(kind: GroupKind) -> &'static str {
    match kind {
        GroupKind::Point => "Point",
        GroupKind::Midpoint => "Midpoint",
        GroupKind::Segment => "Segment",
        GroupKind::Circle => "Circle",
        GroupKind::CircleCenterRadius => "Circle by radius",
        GroupKind::CircleInterior => "Circle interior",
        GroupKind::Line | GroupKind::GraphMeasurementSegment => "Line",
        GroupKind::MeasurementLine => "Axis",
        GroupKind::AxisLine => "CoordSysByAxes",
        GroupKind::PerpendicularLine => "Perpendicular",
        GroupKind::ParallelLine => "Parallel",
        GroupKind::AngleBisectorRay => "Bisector",
        GroupKind::Ray => "Ray",
        GroupKind::Polygon => "Polygon",
        GroupKind::PointConstraint | GroupKind::PathPoint | GroupKind::ParameterControlledPoint => {
            "Point on object"
        }
        GroupKind::LinearIntersectionPoint => "Intersect",
        GroupKind::IntersectionPoint1 | GroupKind::CircleCircleIntersectionPoint1 => "Intersect1",
        GroupKind::IntersectionPoint2 | GroupKind::CircleCircleIntersectionPoint2 => "Intersect2",
        GroupKind::GraphCalibrationX => "UnitPoint",
        GroupKind::GraphCalibrationY | GroupKind::GraphCalibrationYAlt => "SquareUnitPoint",
        GroupKind::ArcOnCircle | GroupKind::CenterArc | GroupKind::ThreePointArc => "Arc",
        GroupKind::RichTextLabel => "FixedText",
        GroupKind::FunctionPlot | GroupKind::LegacyFunctionPlot => "FunctionPlot",
        GroupKind::FunctionExpr => "Calculate",
        GroupKind::ParametricFunctionPlot => "ParametricFunction",
        GroupKind::ActionButton => "ToggleVisibilityButton",
        GroupKind::SectorBoundary => "SectorBoundary",
        GroupKind::CircularSegmentBoundary => "CircularSegmentBoundary",
        GroupKind::AngleMarker => "AngleMarker",
        GroupKind::SegmentMarker => "SegmentMarker",
        GroupKind::Translation => "Translation",
        GroupKind::Rotation | GroupKind::AngleRotation | GroupKind::ParameterRotation => "Rotation",
        GroupKind::Scale | GroupKind::RatioScale => "Scale",
        GroupKind::Reflection => "Reflection",
        GroupKind::RegularPolygonIteration => "RegularPolygonIteration",
        GroupKind::IterationBinding => "IterationBinding",
        GroupKind::ParameterAnchor => "ParameterAnchor",
        GroupKind::FunctionDefinition => "FunctionDefinition",
        GroupKind::Unknown(_) => "Unknown",
        _ => "PayloadObject",
    }
}

fn htm_payload_attributes(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
    context: &HtmPayloadContext<'_>,
) -> String {
    let mut attrs = Vec::new();
    if group.header.is_hidden() {
        attrs.push("hidden".to_string());
        return attrs.join(",");
    }
    if matches!(
        group.header.kind(),
        GroupKind::GraphCalibrationY | GroupKind::GraphCalibrationYAlt
    ) && !context.has_point_function_plot
    {
        attrs.push("hidden".to_string());
        return attrs.join(",");
    }
    if group.header.kind() == GroupKind::Point && decode::is_parameter_control_group(group) {
        attrs.push(htm_color_attr(color_from_style(group.header.style_b)));
        return attrs.join(",");
    }
    let htm_label_lives_on_object = matches!(
        group.header.kind(),
        GroupKind::Point
            | GroupKind::Midpoint
            | GroupKind::PointConstraint
            | GroupKind::PathPoint
            | GroupKind::LinearIntersectionPoint
            | GroupKind::IntersectionPoint1
            | GroupKind::IntersectionPoint2
            | GroupKind::CircleCircleIntersectionPoint1
            | GroupKind::CircleCircleIntersectionPoint2
            | GroupKind::GraphCalibrationX
            | GroupKind::GraphCalibrationY
            | GroupKind::GraphCalibrationYAlt
            | GroupKind::ParameterControlledPoint
            | GroupKind::CartesianOffsetPoint
            | GroupKind::PolarOffsetPoint
            | GroupKind::Translation
            | GroupKind::Rotation
            | GroupKind::ParameterRotation
            | GroupKind::Scale
            | GroupKind::RatioScale
    );
    if htm_label_lives_on_object
        && decode::decode_label_visible(file, group).unwrap_or(!group.header.is_hidden())
        && let Some(name) = decode::decode_label_name_raw(file, group)
    {
        attrs.push(format!("label('{}')", htm_quote_text(&name)));
    }
    match group.header.kind() {
        GroupKind::Point => {
            if decode_group_point(file, group).is_none() {
                attrs.push("black".to_string());
                return attrs.join(",");
            }
            let color = color_from_style(group.header.style_b);
            if !matches!(color, [255, 0, 0, 255] | [0, 0, 0, 255]) {
                attrs.push(htm_color_attr(color));
            }
            if htm_free_point_has_medium_size(group.header.style_a) {
                attrs.push("mediumPoint".to_string());
            }
        }
        GroupKind::Midpoint
        | GroupKind::PointConstraint
        | GroupKind::PathPoint
        | GroupKind::LinearIntersectionPoint
        | GroupKind::IntersectionPoint1
        | GroupKind::IntersectionPoint2
        | GroupKind::CircleCircleIntersectionPoint1
        | GroupKind::CircleCircleIntersectionPoint2
        | GroupKind::GraphCalibrationX
        | GroupKind::GraphCalibrationY
        | GroupKind::GraphCalibrationYAlt
        | GroupKind::ParameterControlledPoint => {
            let color = color_from_style(group.header.style_b);
            if color == [0, 0, 0, 255] {
                attrs.push("black".to_string());
            } else {
                attrs.push(htm_color_attr(color));
            }
            if htm_point_has_medium_size(group.header.style_a)
                || (!matches!(color, [255, 0, 0, 255] | [0, 0, 0, 255])
                    && htm_free_point_has_medium_size(group.header.style_a))
            {
                attrs.push("mediumPoint".to_string());
            }
        }
        GroupKind::Translation
        | GroupKind::CartesianOffsetPoint
        | GroupKind::PolarOffsetPoint
        | GroupKind::Rotation
        | GroupKind::ParameterRotation
        | GroupKind::Scale
        | GroupKind::RatioScale => {
            if htm_transform_result_is_line(file, groups, group) {
                attrs.push(htm_stroke_color_attr(color_from_style(
                    group.header.style_b,
                )));
                if line_is_dashed(group.header.style_a) {
                    if htm_line_has_medium_width(group.header.style_a) {
                        attrs.push("mediumLine".to_string());
                    }
                    attrs.push("dashed".to_string());
                } else {
                    attrs.push("mediumLine".to_string());
                }
            } else {
                let color = color_from_style(group.header.style_b);
                if color != [0, 0, 0, 255] {
                    attrs.push(htm_color_attr(color));
                }
                if htm_point_has_medium_size(group.header.style_a) {
                    attrs.push("mediumPoint".to_string());
                }
            }
        }
        GroupKind::Segment
        | GroupKind::Circle
        | GroupKind::CircleCenterRadius
        | GroupKind::Line
        | GroupKind::PerpendicularLine
        | GroupKind::ParallelLine
        | GroupKind::AngleBisectorRay
        | GroupKind::Ray
        | GroupKind::GraphMeasurementSegment => {
            attrs.push(htm_stroke_color_attr(color_from_style(
                group.header.style_b,
            )));
            if line_is_dashed(group.header.style_a) && group.header.kind() == GroupKind::Segment {
                if htm_line_has_medium_width(group.header.style_a) {
                    attrs.push("mediumLine".to_string());
                }
                attrs.push("dashed".to_string());
            } else if line_is_dashed(group.header.style_a)
                && matches!(
                    group.header.kind(),
                    GroupKind::Circle | GroupKind::CircleCenterRadius
                )
            {
                attrs.push("mediumLine".to_string());
                attrs.push("dashed".to_string());
            } else if line_is_dashed(group.header.style_a) {
                attrs.push("dashed".to_string());
            } else if htm_line_has_medium_width(group.header.style_a) {
                attrs.push("mediumLine".to_string());
            }
        }
        GroupKind::MeasurementLine | GroupKind::AxisLine => {
            attrs.push(htm_stroke_color_attr(color_from_style(
                group.header.style_b,
            )));
        }
        GroupKind::FunctionExpr
        | GroupKind::FunctionDefinition
        | GroupKind::CoordinateXValue
        | GroupKind::CoordinateYValue
        | GroupKind::CoordinateReadoutLabel
        | GroupKind::DistanceValue
        | GroupKind::GraphDistanceValue => {
            attrs.push("black".to_string());
        }
        GroupKind::FunctionPlot | GroupKind::LegacyFunctionPlot => {
            attrs.push(htm_stroke_color_attr(color_from_style(
                group.header.style_b,
            )));
            if htm_line_has_medium_width(group.header.style_a) {
                attrs.push("mediumLine".to_string());
            }
        }
        GroupKind::Polygon => {
            attrs.push(htm_color_attr(geometry::fill_color_from_styles(
                group.header.style_b,
                group.header.style_c,
            )));
        }
        GroupKind::CircleInterior => {
            attrs.push(htm_color_attr(geometry::fill_color_from_styles(
                group.header.style_b,
                group.header.style_c,
            )));
        }
        GroupKind::PointTrace => {
            attrs.push(htm_rgb_color_attr(color_from_style(group.header.style_b)));
            attrs.push("mediumLine".to_string());
        }
        GroupKind::CoordinatePoint => {
            let color = color_from_style(group.header.style_b);
            if color != [0, 0, 0, 255] {
                attrs.push(htm_color_attr(color));
            }
            attrs.push("mediumPoint".to_string());
        }
        GroupKind::RichTextLabel | GroupKind::ActionButton => {
            attrs.push(htm_color_attr(color_from_style(group.header.style_b)));
        }
        _ => {}
    }
    attrs.join(",")
}

fn htm_color_attr(color: [u8; 4]) -> String {
    match color {
        [0, 0, 0, _] => "black".to_string(),
        [255, 0, 0, _] => "red".to_string(),
        [255, 0, 255, _] => "magenta".to_string(),
        [255, 255, 0, _] => "yellow".to_string(),
        [0, 128, 0, _] | [0, 255, 0, _] => "green".to_string(),
        [0, 0, 255, _] => "blue".to_string(),
        [0, 255, 255, _] => "cyan".to_string(),
        [r, g, b, _] => format!("color({r},{g},{b})"),
    }
}

fn htm_rgb_color_attr(color: [u8; 4]) -> String {
    let [r, g, b, _] = color;
    format!("color({r},{g},{b})")
}

fn htm_stroke_color_attr(color: [u8; 4]) -> String {
    match color {
        [255, 0, 0, _] => "red".to_string(),
        [0, 0, 255, _] => "blue".to_string(),
        [r, g, b, _] => format!("color({r},{g},{b})"),
    }
}

fn htm_line_has_medium_width(style_a: u32) -> bool {
    geometry::line_stroke_width_from_style(style_a) > 1.0
}

fn htm_point_has_medium_size(style_a: u32) -> bool {
    ((style_a >> 24) & 0xff) == 0x02
}

fn htm_free_point_has_medium_size(style_a: u32) -> bool {
    htm_point_has_medium_size(style_a)
}

fn format_ref_args(refs: &[usize], ordinal_map: &BTreeMap<usize, usize>) -> String {
    refs.iter()
        .map(|reference| map_htm_ordinal(*reference, ordinal_map).to_string())
        .collect::<Vec<_>>()
        .join(",")
}

fn format_reversed_ref_args(refs: &[usize], ordinal_map: &BTreeMap<usize, usize>) -> String {
    if refs.len() == 2 {
        format!(
            "{},{}",
            map_htm_ordinal(refs[1], ordinal_map),
            map_htm_ordinal(refs[0], ordinal_map)
        )
    } else {
        format_ref_args(refs, ordinal_map)
    }
}

fn htm_offset_point_args(
    file: &GspFile,
    group: &ObjectGroup,
    ordinal_map: &BTreeMap<usize, usize>,
) -> String {
    let Some(constraint) = decode_translated_point_constraint(file, group) else {
        return format_ref_args(
            &find_indexed_path(file, group)
                .map(|path| path.refs)
                .unwrap_or_default(),
            ordinal_map,
        );
    };
    format!(
        "{},{},{}",
        map_htm_ordinal(constraint.origin_group_index + 1, ordinal_map),
        format_htm_unit_length(constraint.dx),
        format_htm_unit_length(-constraint.dy)
    )
}

fn map_htm_ordinal(ordinal: usize, ordinal_map: &BTreeMap<usize, usize>) -> usize {
    ordinal_map.get(&ordinal).copied().unwrap_or(ordinal)
}

fn htm_quote_text(text: &str) -> String {
    text.replace('\'', "''").replace('\n', " ")
}

fn htm_point_is_image_anchor(file: &GspFile, group: &ObjectGroup) -> bool {
    group.header.kind() == GroupKind::Point
        && !decode::is_parameter_control_group(group)
        && decode_group_point(file, group).is_none()
        && group.records.iter().any(|record| {
            matches!(
                record.record_type,
                crate::runtime::payload_consts::RECORD_BBOX_C
                    | crate::runtime::payload_consts::RECORD_IMAGE_TRANSFORM
                    | RECORD_RICH_TEXT
            )
        })
        && try_decode_group_label_text(file, group)
            .unwrap_or_default()
            .trim()
            .is_empty()
}

fn function_definition_refs_this_group(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
) -> bool {
    groups.iter().any(|candidate| {
        matches!(
            candidate.header.kind(),
            GroupKind::FunctionPlot | GroupKind::LegacyFunctionPlot
        ) && find_indexed_path(file, candidate).and_then(|path| path.refs.first().copied())
            == Some(group.ordinal)
    })
}

fn htm_fixed_text_args(file: &GspFile, group: &ObjectGroup) -> String {
    let rich_text = try_decode_group_rich_text(file, group);
    let text = rich_text
        .as_ref()
        .map(|content| content.text.clone())
        .or_else(|| try_decode_group_label_text(file, group))
        .unwrap_or_default();
    try_decode_payload_anchor_point(file, group)
        .ok()
        .flatten()
        .or_else(|| decode_bbox_anchor_raw(file, group))
        .map(|anchor| {
            format!(
                "{},{},'{}'",
                format_number(anchor.x),
                format_number(anchor.y),
                htm_quote_text(&text)
            )
        })
        .unwrap_or_default()
}

fn htm_payload_position(file: &GspFile, group: &ObjectGroup, dx: f64, dy: f64) -> (f64, f64) {
    try_decode_payload_anchor_point(file, group)
        .ok()
        .flatten()
        .map(|anchor| {
            let point = file.document_display_point(PointRecord {
                x: anchor.x + dx,
                y: anchor.y + dy,
            });
            (point.x, point.y)
        })
        .unwrap_or((0.0, 0.0))
}

fn htm_parameter_args(file: &GspFile, groups: &[ObjectGroup], group: &ObjectGroup) -> String {
    let mut value = try_decode_parameter_control_value_for_group(file, groups, group)
        .ok()
        .unwrap_or(0.0);
    let is_iteration_depth_parameter = groups.iter().any(|candidate| {
        candidate.header.kind() == GroupKind::RegularPolygonIteration
            && find_indexed_path(file, candidate).and_then(|path| path.refs.first().copied())
                == Some(group.ordinal)
    });
    if is_iteration_depth_parameter {
        value *= 10.0;
    }
    if groups.iter().any(|candidate| {
        candidate.header.kind() == GroupKind::CoordinatePoint
            && find_indexed_path(file, candidate).and_then(|path| path.refs.first().copied())
                == Some(group.ordinal)
    }) {
        value = 0.0;
    }
    let (x, y) = try_decode_bbox_rect_raw(file, group)
        .ok()
        .flatten()
        .map(|(left, top, _width, height)| (left + 4.0, top + height - 9.0))
        .or_else(|| {
            try_decode_payload_anchor_point(file, group)
                .ok()
                .flatten()
                .map(|anchor| (anchor.x + 4.0, anchor.y + 23.0))
        })
        .map(|(x, y)| {
            let mut point = file.document_display_point(PointRecord { x, y });
            if is_iteration_depth_parameter {
                point.y += 2.0;
            } else if !has_graph_classes(groups)
                && groups.iter().any(|candidate| {
                    candidate.header.kind() == GroupKind::FunctionExpr
                        && find_indexed_path(file, candidate)
                            .is_some_and(|path| path.refs.contains(&group.ordinal))
                })
                || groups.iter().any(|candidate| {
                    candidate.header.kind() == GroupKind::FunctionDefinition
                        && find_indexed_path(file, candidate)
                            .is_some_and(|path| path.refs.contains(&group.ordinal))
                })
            {
                point.y += 1.0;
            }
            (point.x, point.y)
        })
        .unwrap_or((0.0, 0.0));
    let name = decode_htm_label_name(file, group).unwrap_or_default();
    format!(
        "{},{},{},'{} = '",
        format_htm_parameter(value),
        format_number(x),
        format_number(y),
        htm_quote_text(&name)
    )
}

fn htm_function_args(file: &GspFile, groups: &[ObjectGroup], group: &ObjectGroup) -> String {
    let (x, y) = htm_payload_position(file, group, 4.0, 20.0);
    let name = decode_htm_label_name(file, group).unwrap_or_else(|| "f".to_string());
    let plot_mode = htm_referencing_function_plot_mode(file, groups, group.ordinal)
        .unwrap_or(FunctionPlotMode::Cartesian);
    let variable = match plot_mode {
        FunctionPlotMode::Cartesian => "x",
        FunctionPlotMode::Polar => "θ",
    };
    let expr = try_decode_function_expr(file, groups, group).ok();
    let label = expr
        .clone()
        .map(|expr| htm_expr_label(&expr, variable, &BTreeMap::new()))
        .unwrap_or_default();
    let rpn = expr
        .as_ref()
        .map(|expr| htm_expr_rpn(expr, &BTreeMap::new()))
        .unwrap_or_default();
    match plot_mode {
        FunctionPlotMode::Cartesian => format!(
            "{},{},'{}(x) = {}','{} '",
            format_number(x),
            format_number(y),
            htm_quote_text(&name),
            htm_quote_text(&label),
            rpn
        ),
        FunctionPlotMode::Polar => format!(
            "{},{},'r = {}','{} '",
            format_number(x),
            format_number(y),
            htm_quote_text(&label),
            rpn
        ),
    }
}

fn htm_referencing_function_plot_mode(
    file: &GspFile,
    groups: &[ObjectGroup],
    function_ordinal: usize,
) -> Option<FunctionPlotMode> {
    groups.iter().find_map(|group| {
        if !matches!(
            group.header.kind(),
            GroupKind::FunctionPlot | GroupKind::LegacyFunctionPlot
        ) {
            return None;
        }
        let refs_this_function = find_indexed_path(file, group)
            .and_then(|path| path.refs.first().copied())
            == Some(function_ordinal);
        if !refs_this_function {
            return None;
        }
        htm_function_plot_mode(file, group)
    })
}

pub(super) fn htm_function_plot_mode(
    file: &GspFile,
    group: &ObjectGroup,
) -> Option<FunctionPlotMode> {
    group
        .records
        .iter()
        .find(|record| record.record_type == RECORD_FUNCTION_PLOT_DESCRIPTOR)
        .and_then(|record| {
            crate::runtime::functions::try_decode_function_plot_descriptor(
                record.payload(&file.data),
            )
            .ok()
        })
        .map(|descriptor| descriptor.mode)
}

fn htm_function_definition_args(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
    refs: &[usize],
    ordinal_map: &BTreeMap<usize, usize>,
) -> String {
    let (x, mut y) = try_decode_bbox_rect_raw(file, group)
        .ok()
        .flatten()
        .map(|(left, top, _width, height)| (left + 4.0, top + height - 9.0))
        .map(|(x, y)| {
            let point = file.document_display_point(PointRecord { x, y });
            (point.x, point.y)
        })
        .unwrap_or_else(|| htm_payload_position(file, group, 4.0, 20.0));
    y += 1.0;
    let parameter_letters = htm_parameter_letters(file, groups, refs);
    let expr = try_decode_function_expr(file, groups, group).ok();
    let label = expr
        .as_ref()
        .map(htm_function_definition_label)
        .unwrap_or_default();
    let rpn = expr
        .as_ref()
        .map(|expr| htm_expr_rpn(expr, &parameter_letters))
        .unwrap_or_default();
    format!(
        "{},{},'y = {}','{} '\u{1f}{}",
        format_number(x),
        format_number(y),
        htm_quote_text(&label),
        rpn,
        format_ref_args(refs, ordinal_map)
    )
}

fn htm_calculate_args(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
    refs: &[usize],
    ordinal_map: &BTreeMap<usize, usize>,
    graph: Option<&GraphTransform>,
) -> String {
    let parameter_letters = htm_parameter_letters(file, groups, refs);
    let expr = try_decode_function_expr(file, groups, group).ok();
    let (x, mut y) = htm_payload_position(
        file,
        group,
        4.0,
        if graph.is_some() || group.header.is_hidden() {
            20.0
        } else {
            14.0
        },
    );
    if graph.is_none()
        && !group.header.is_hidden()
        && expr
            .as_ref()
            .is_some_and(|expr| !matches!(expr, FunctionExpr::Parsed(FunctionAst::Parameter(_, _))))
    {
        y += 3.0;
        if expr.as_ref().is_some_and(|expr| {
            matches!(
                expr,
                FunctionExpr::Parsed(FunctionAst::Binary {
                    op: BinaryOp::Div,
                    ..
                })
            )
        }) {
            y += 9.0;
        }
    }
    let mut expr_label = if group.header.is_hidden() {
        String::new()
    } else {
        expr.as_ref().map(htm_calculate_label).unwrap_or_default()
    };
    if y >= 120.0
        && expr.as_ref().is_some_and(|expr| {
            matches!(
                expr,
                FunctionExpr::Parsed(FunctionAst::Binary {
                    op: BinaryOp::Add,
                    ..
                })
            )
        })
        && !expr_label.starts_with("((")
    {
        expr_label = format!("({expr_label})");
    }
    if expr.as_ref().is_some_and(|expr| {
        matches!(
            expr,
            FunctionExpr::Parsed(FunctionAst::Binary {
                op: BinaryOp::Div,
                ..
            })
        )
    }) {
        expr_label = expr_label.replace(") / 2", "/2");
    }
    let mut rpn = expr
        .as_ref()
        .map(|expr| htm_expr_rpn(expr, &parameter_letters))
        .unwrap_or_default();
    if let Some(graph) = graph {
        rpn = format!("A {} *", format_htm_significant(graph.raw_per_unit, 6));
        expr_label.push_str(" 厘米");
    }
    let refs = format_ref_args(refs, ordinal_map);
    if group.header.is_hidden() {
        return format!(
            "{},{},'','{} ')({}",
            format_number(x),
            format_number(y),
            rpn,
            refs
        );
    }
    format!(
        "{},{},'{} = ','{} ')({}",
        format_number(x),
        format_number(y),
        htm_quote_text(&expr_label),
        rpn,
        refs
    )
}

fn htm_parameter_letters(
    file: &GspFile,
    groups: &[ObjectGroup],
    refs: &[usize],
) -> BTreeMap<String, char> {
    refs.iter()
        .enumerate()
        .filter_map(|(index, ordinal)| {
            let letter = (b'A' + u8::try_from(index).ok()?) as char;
            let group = groups.get(ordinal.saturating_sub(1))?;
            let name = decode_htm_label_name(file, group)
                .or_else(|| decode::decode_label_name_raw(file, group))?;
            Some((htm_unsubscript_digits(&name), letter))
        })
        .collect()
}

fn htm_calculate_label(expr: &FunctionExpr) -> String {
    match expr {
        FunctionExpr::Parsed(ast) => match ast {
            FunctionAst::Binary {
                op: BinaryOp::Pow, ..
            } => format!("({})", htm_ast_label(ast, "x", false)),
            FunctionAst::Binary {
                lhs,
                op: BinaryOp::Div,
                rhs,
            } => format!(
                "({}/{})",
                htm_ast_label(lhs, "x", false),
                htm_ast_label(rhs, "x", false)
            ),
            _ => htm_ast_label(ast, "x", false),
        },
        _ => htm_unsubscript_digits(&function_expr_label_with_variable(expr.clone(), "x")),
    }
}

fn htm_function_definition_label(expr: &FunctionExpr) -> String {
    match expr {
        FunctionExpr::Parsed(ast) => htm_ast_label_for_function_definition(ast),
        _ => htm_unsubscript_digits(&function_expr_label_with_variable(expr.clone(), "x")),
    }
}

fn htm_ast_label_for_function_definition(ast: &FunctionAst) -> String {
    match ast {
        FunctionAst::Binary {
            lhs,
            op: BinaryOp::Add,
            rhs,
        } => format!(
            "{} + {}",
            htm_ast_label_for_function_definition(lhs),
            htm_ast_label_for_function_definition(rhs)
        ),
        FunctionAst::Binary {
            lhs,
            op: BinaryOp::Sub,
            rhs,
        } => format!(
            "{} - {}",
            htm_ast_label_for_function_definition(lhs),
            htm_ast_label_for_function_definition(rhs)
        ),
        FunctionAst::Binary {
            lhs,
            op: BinaryOp::Mul,
            rhs,
        } => {
            let left = htm_ast_label_for_function_definition(lhs);
            let right = htm_ast_label_for_function_definition(rhs);
            if matches!(
                **rhs,
                FunctionAst::Binary {
                    op: BinaryOp::Pow,
                    ..
                }
            ) {
                format!("{left}*({right})")
            } else {
                format!("{left}*{right}")
            }
        }
        _ => htm_ast_label(ast, "x", false),
    }
}

fn htm_expr_label(expr: &FunctionExpr, variable: &str, letters: &BTreeMap<String, char>) -> String {
    match expr {
        FunctionExpr::Parsed(ast) => htm_ast_label_with_letters(ast, variable, letters, false),
        _ => htm_unsubscript_digits(&function_expr_label_with_variable(expr.clone(), variable)),
    }
}

fn htm_ast_label(ast: &FunctionAst, variable: &str, wrap_binary: bool) -> String {
    htm_ast_label_with_letters(ast, variable, &BTreeMap::new(), wrap_binary)
}

fn htm_ast_label_with_letters(
    ast: &FunctionAst,
    variable: &str,
    letters: &BTreeMap<String, char>,
    wrap_binary: bool,
) -> String {
    let text = match ast {
        FunctionAst::Variable => variable.to_string(),
        FunctionAst::Constant(value) => format_htm_parameter(*value),
        FunctionAst::PiAngle => "π".to_string(),
        FunctionAst::Parameter(name, _) => htm_unsubscript_digits(name),
        FunctionAst::Unary { op, expr } => {
            let inner = htm_ast_label_with_letters(expr, variable, letters, false);
            match op {
                UnaryFunction::Sin => format!("sin({inner})"),
                UnaryFunction::Cos => format!("cos({inner})"),
                UnaryFunction::Tan => format!("tan({inner})"),
                UnaryFunction::Abs => format!("|{inner}|"),
                UnaryFunction::Sqrt => format!("√({inner})"),
                UnaryFunction::Ln => format!("ln({inner})"),
                UnaryFunction::Log10 => format!("log({inner})"),
                UnaryFunction::Sign => format!("sgn({inner})"),
                UnaryFunction::Round => format!("round({inner})"),
                UnaryFunction::Trunc => format!("trunc({inner})"),
            }
        }
        FunctionAst::Binary { lhs, op, rhs } => {
            let lhs_text = match (&**lhs, op) {
                (
                    FunctionAst::Binary {
                        op: BinaryOp::Add | BinaryOp::Sub,
                        ..
                    },
                    BinaryOp::Mul | BinaryOp::Div | BinaryOp::Pow,
                )
                | (FunctionAst::Binary { .. }, BinaryOp::Pow) => {
                    format!(
                        "({})",
                        htm_ast_label_with_letters(lhs, variable, letters, false)
                    )
                }
                (FunctionAst::Binary { .. }, BinaryOp::Add | BinaryOp::Sub) => {
                    format!(
                        "({})",
                        htm_ast_label_with_letters(lhs, variable, letters, false)
                    )
                }
                _ => htm_ast_label_with_letters(lhs, variable, letters, false),
            };
            let rhs_text = match (&**rhs, op) {
                (FunctionAst::Binary { .. }, _) => {
                    format!(
                        "({})",
                        htm_ast_label_with_letters(rhs, variable, letters, false)
                    )
                }
                _ => htm_ast_label_with_letters(rhs, variable, letters, false),
            };
            match op {
                BinaryOp::Add => format!("{lhs_text} + {rhs_text}"),
                BinaryOp::Sub => format!("{lhs_text} - {rhs_text}"),
                BinaryOp::Mul => format!("{lhs_text}*{rhs_text}"),
                BinaryOp::Div => format!("{lhs_text} / {rhs_text}"),
                BinaryOp::Pow => format!("{lhs_text}^{rhs_text}"),
            }
        }
    };
    let _ = letters;
    if wrap_binary && matches!(ast, FunctionAst::Binary { .. }) {
        format!("({text})")
    } else {
        text
    }
}

fn htm_expr_rpn(expr: &FunctionExpr, letters: &BTreeMap<String, char>) -> String {
    match expr {
        FunctionExpr::Parsed(ast) => htm_ast_rpn(ast, letters),
        FunctionExpr::Constant(value) => format_htm_parameter(*value),
        FunctionExpr::Identity => "x".to_string(),
        FunctionExpr::SinIdentity => "x @sin_".to_string(),
        FunctionExpr::CosIdentityPlus(value) => {
            format!("x @cos_ {} +", format_htm_parameter(*value))
        }
        FunctionExpr::TanIdentityMinus(value) => {
            format!("x @tan_ {} -", format_htm_parameter(*value))
        }
    }
}

fn htm_ast_rpn(ast: &FunctionAst, letters: &BTreeMap<String, char>) -> String {
    match ast {
        FunctionAst::Variable => "x".to_string(),
        FunctionAst::Constant(value) => format_htm_parameter(*value),
        FunctionAst::PiAngle => "3.14159".to_string(),
        FunctionAst::Parameter(name, _) => letters
            .get(&htm_unsubscript_digits(name))
            .copied()
            .unwrap_or('A')
            .to_string(),
        FunctionAst::Unary { op, expr } => {
            let inner = htm_ast_rpn(expr, letters);
            let op = match op {
                UnaryFunction::Sin => "@sin_",
                UnaryFunction::Cos => "@cos_",
                UnaryFunction::Tan => "@tan_",
                UnaryFunction::Abs => "@abs_",
                UnaryFunction::Sqrt => "@sqrt_",
                UnaryFunction::Ln => "@ln_",
                UnaryFunction::Log10 => "@log_",
                UnaryFunction::Sign => "@sgn_",
                UnaryFunction::Round => "@round_",
                UnaryFunction::Trunc => "@trnc",
            };
            format!("{inner} {op}")
        }
        FunctionAst::Binary { lhs, op, rhs } => {
            let op = match op {
                BinaryOp::Add => "+",
                BinaryOp::Sub => "-",
                BinaryOp::Mul => "*",
                BinaryOp::Div => "/",
                BinaryOp::Pow => "^",
            };
            format!(
                "{} {} {op}",
                htm_ast_rpn(lhs, letters),
                htm_ast_rpn(rhs, letters)
            )
        }
    }
}

fn htm_transform_result_is_line(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
) -> bool {
    let Some(source_ordinal) =
        find_indexed_path(file, group).and_then(|path| path.refs.first().copied())
    else {
        return false;
    };
    let Some(source) = groups.get(source_ordinal.saturating_sub(1)) else {
        return false;
    };
    match source.header.kind() {
        GroupKind::Segment
        | GroupKind::Line
        | GroupKind::PerpendicularLine
        | GroupKind::ParallelLine
        | GroupKind::AngleBisectorRay
        | GroupKind::Ray
        | GroupKind::Circle
        | GroupKind::CircleCenterRadius => true,
        GroupKind::Translation | GroupKind::Rotation | GroupKind::Scale | GroupKind::RatioScale => {
            htm_transform_result_is_line(file, groups, source)
        }
        _ => false,
    }
}

fn htm_rotation_args(
    file: &GspFile,
    group: &ObjectGroup,
    ordinal_map: &BTreeMap<usize, usize>,
) -> String {
    let refs = find_indexed_path(file, group)
        .map(|path| path.refs)
        .unwrap_or_default();
    let angle_radians = try_decode_transform_binding(file, group)
        .ok()
        .and_then(|binding| match binding.kind {
            TransformBindingKind::Rotate { angle_degrees, .. } => Some(angle_degrees.to_radians()),
            _ => None,
        })
        .unwrap_or(0.0);
    format!(
        "{},{},{}",
        refs.first()
            .map(|reference| map_htm_ordinal(*reference, ordinal_map))
            .unwrap_or(0),
        refs.get(1)
            .map(|reference| map_htm_ordinal(*reference, ordinal_map))
            .unwrap_or(0),
        format_htm_parameter(angle_radians)
    )
}

fn htm_scale_args(
    file: &GspFile,
    group: &ObjectGroup,
    ordinal_map: &BTreeMap<usize, usize>,
) -> String {
    let refs = find_indexed_path(file, group)
        .map(|path| path.refs)
        .unwrap_or_default();
    let factor = try_decode_transform_binding(file, group)
        .ok()
        .and_then(|binding| match binding.kind {
            TransformBindingKind::Scale { factor } => Some(factor),
            _ => None,
        })
        .unwrap_or(1.0);
    format!(
        "{},{},{}",
        refs.first()
            .map(|reference| map_htm_ordinal(*reference, ordinal_map))
            .unwrap_or(0),
        refs.get(1)
            .map(|reference| map_htm_ordinal(*reference, ordinal_map))
            .unwrap_or(0),
        format_htm_parameter(factor)
    )
}

fn htm_locus_args(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
    refs: &[usize],
    ordinal_map: &BTreeMap<usize, usize>,
) -> String {
    let sample_count = group
        .records
        .iter()
        .find(|record| record.record_type == RECORD_FUNCTION_PLOT_DESCRIPTOR)
        .and_then(|record| {
            crate::runtime::functions::try_decode_function_plot_descriptor(
                record.payload(&file.data),
            )
            .ok()
        })
        .map(|descriptor| descriptor.sample_count)
        .unwrap_or(500);
    let target = refs.first().copied().unwrap_or(0);
    let driver = refs.iter().copied().find(|ordinal| {
        groups
            .get(ordinal.saturating_sub(1))
            .is_some_and(|candidate| {
                matches!(
                    candidate.header.kind(),
                    GroupKind::PointConstraint
                        | GroupKind::PathPoint
                        | GroupKind::ParameterControlledPoint
                )
            })
    });
    let host = driver.and_then(|driver_ordinal| {
        groups
            .get(driver_ordinal.saturating_sub(1))
            .and_then(|driver_group| find_indexed_path(file, driver_group))
            .and_then(|path| path.refs.first().copied())
    });
    match (driver, host) {
        (Some(driver), Some(host)) => format!(
            "{},{},{},{}",
            map_htm_ordinal(target, ordinal_map),
            map_htm_ordinal(driver, ordinal_map),
            map_htm_ordinal(host, ordinal_map),
            sample_count
        ),
        _ => format!("{},{}", format_ref_args(refs, ordinal_map), sample_count),
    }
}

fn htm_plot_xy_args(refs: &[usize], ordinal_map: &BTreeMap<usize, usize>) -> String {
    if refs.len() >= 3 {
        format!(
            "{},{},{}",
            map_htm_ordinal(refs[1], ordinal_map),
            map_htm_ordinal(refs[2], ordinal_map),
            map_htm_ordinal(refs[0], ordinal_map)
        )
    } else {
        format_ref_args(refs, ordinal_map)
    }
}

fn htm_measurement_args(
    file: &GspFile,
    group: &ObjectGroup,
    refs: &[usize],
    ordinal_map: &BTreeMap<usize, usize>,
    axis_name: &str,
) -> String {
    let mut position = htm_payload_position(file, group, 4.0, 14.0);
    if position == (0.0, 0.0) {
        position = match group.header.kind() {
            GroupKind::CoordinateXValue => (14.0, 25.0),
            GroupKind::CoordinateYValue => (14.0, 53.0),
            _ => position,
        };
    }
    let (x, y) = position;
    let point_ref = refs.first().copied().unwrap_or(0);
    let axis_ref = refs.get(1).copied().unwrap_or(0);
    let point_name = groups_label_name(file, point_ref).unwrap_or_default();
    format!(
        "{},{},{},{},'{}[{}] = '",
        map_htm_ordinal(point_ref, ordinal_map),
        map_htm_ordinal(axis_ref, ordinal_map),
        format_number(x),
        format_number(y),
        axis_name,
        htm_quote_text(&point_name)
    )
}

fn htm_coordinate_readout_args(
    file: &GspFile,
    group: &ObjectGroup,
    refs: &[usize],
    ordinal_map: &BTreeMap<usize, usize>,
) -> String {
    let mut position = htm_payload_position(file, group, 0.0, 0.0);
    if position == (0.0, 0.0) {
        position = (10.0, 77.0);
    }
    let (x, y) = position;
    let point_ref = refs.first().copied().unwrap_or(0);
    let axis_ref = refs.get(1).copied().unwrap_or(0);
    let point_name = groups_label_name(file, point_ref).unwrap_or_default();
    format!(
        "{},{},{},{},'{}: '",
        map_htm_ordinal(point_ref, ordinal_map),
        map_htm_ordinal(axis_ref, ordinal_map),
        format_number(x),
        format_number(y),
        htm_quote_text(&point_name)
    )
}

fn htm_graph_distance_args(
    file: &GspFile,
    group: &ObjectGroup,
    refs: &[usize],
    ordinal_map: &BTreeMap<usize, usize>,
) -> String {
    let mut position = htm_payload_position(file, group, 4.0, 14.0);
    if position == (0.0, 0.0) {
        position = (14.0, 105.0);
    }
    let (x, y) = position;
    let left = refs.first().copied().unwrap_or(0);
    let right = refs.get(1).copied().unwrap_or(0);
    let left_name = groups_label_name(file, left).unwrap_or_default();
    let right_name = groups_label_name(file, right).unwrap_or_default();
    format!(
        "{},{},{},{},'{}{} = '",
        map_htm_ordinal(right, ordinal_map),
        map_htm_ordinal(left, ordinal_map),
        format_number(x),
        format_number(y),
        htm_quote_text(&left_name),
        htm_quote_text(&right_name)
    )
}

fn groups_label_name(file: &GspFile, ordinal: usize) -> Option<String> {
    let groups = file.object_groups();
    let group = groups.get(ordinal.checked_sub(1)?)?;
    decode::decode_label_name_raw(file, group)
}

fn htm_function_plot_args(
    file: &GspFile,
    group: &ObjectGroup,
    refs: &[usize],
    ordinal_map: &BTreeMap<usize, usize>,
) -> String {
    let descriptor = group
        .records
        .iter()
        .find(|record| record.record_type == RECORD_FUNCTION_PLOT_DESCRIPTOR)
        .and_then(|record| {
            crate::runtime::functions::try_decode_function_plot_descriptor(
                record.payload(&file.data),
            )
            .ok()
        });
    let (samples, x_min, x_max, mode) = descriptor
        .map(|descriptor| {
            (
                descriptor.sample_count,
                format_htm_significant(descriptor.x_min, 6),
                format_htm_significant(descriptor.x_max, 6),
                match descriptor.mode {
                    FunctionPlotMode::Cartesian => 0,
                    FunctionPlotMode::Polar => 2,
                },
            )
        })
        .unwrap_or((0, String::new(), String::new(), 0));
    format!(
        "{},{},{},{},{},{}",
        refs.first()
            .map(|reference| map_htm_ordinal(*reference, ordinal_map))
            .unwrap_or(0),
        refs.get(1)
            .map(|reference| map_htm_ordinal(*reference, ordinal_map))
            .unwrap_or(0),
        samples,
        x_min,
        x_max,
        mode
    )
}

fn htm_action_button_args(
    file: &GspFile,
    group: &ObjectGroup,
    refs: &[usize],
    ordinal_map: &BTreeMap<usize, usize>,
) -> String {
    let anchor = decode::decode_button_screen_anchor(file, group)
        .unwrap_or(PointRecord { x: 24.0, y: 24.0 });
    let text = decode::decode_label_name_raw(file, group).unwrap_or_else(|| "按钮".to_string());
    let alternate = text
        .strip_prefix("隐藏")
        .map(|rest| {
            let concise = rest.split_once(' ').map(|(head, _)| head).unwrap_or(rest);
            format!("显示{concise}")
        })
        .unwrap_or_else(|| text.clone());
    format!(
        "{},{},'{}|{}'\u{1f}{}",
        format_number(anchor.x),
        format_number(anchor.y),
        htm_quote_text(&text),
        htm_quote_text(&alternate),
        format_ref_args(refs, ordinal_map)
    )
}

fn htm_animate_button_args(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
    refs: &[usize],
    ordinal_map: &BTreeMap<usize, usize>,
) -> String {
    let anchor = decode::decode_button_screen_anchor(file, group)
        .unwrap_or(PointRecord { x: 24.0, y: 24.0 });
    let text = decode::decode_label_name_raw(file, group).unwrap_or_else(|| "动画点".to_string());
    let target_refs = htm_animate_button_target_refs(file, groups, refs);
    format!(
        "{},{},'{}')({})(1)(0)(1",
        format_number(anchor.x),
        format_number(anchor.y),
        htm_quote_text(&text),
        format_ref_args(&target_refs, ordinal_map)
    )
}

fn htm_animate_button_target_refs(
    file: &GspFile,
    groups: &[ObjectGroup],
    refs: &[usize],
) -> Vec<usize> {
    let mut target_refs = refs.to_vec();
    if refs.len() == 1
        && let Some(target_group) = refs
            .first()
            .and_then(|reference| groups.get(reference.saturating_sub(1)))
        && matches!(
            target_group.header.kind(),
            GroupKind::PointConstraint | GroupKind::PathPoint | GroupKind::ParameterControlledPoint
        )
        && let Some(host) =
            find_indexed_path(file, target_group).and_then(|path| path.refs.first().copied())
        && host != refs[0]
    {
        target_refs.push(host);
    }
    target_refs
}

fn htm_action_button_kind(file: &GspFile, group: &ObjectGroup) -> Option<(u16, u16)> {
    group
        .records
        .iter()
        .find(|record| {
            record.record_type == crate::runtime::payload_consts::RECORD_ACTION_BUTTON_PAYLOAD
        })
        .map(|record| record.payload(&file.data))
        .filter(|payload| payload.len() >= 16)
        .map(|payload| (read_u16(payload, 12), read_u16(payload, 14)))
}

fn decode_htm_label_name(file: &GspFile, group: &ObjectGroup) -> Option<String> {
    let payload = group
        .records
        .iter()
        .find(|record| record.record_type == RECORD_LABEL_AUX)
        .map(|record| record.payload(&file.data))?;
    if payload.len() < 24 {
        return None;
    }
    let name_len = read_u16(payload, 22) as usize;
    if name_len == 0 || 24 + name_len > payload.len() {
        return None;
    }
    Some(String::from_utf8_lossy(&payload[24..24 + name_len]).to_string())
}

fn htm_unsubscript_digits(text: &str) -> String {
    text.replace('₁', "[1]")
        .replace('₂', "[2]")
        .replace('₃', "[3]")
        .replace('₄', "[4]")
}

fn decode_group_point(file: &GspFile, group: &ObjectGroup) -> Option<PointRecord> {
    group
        .records
        .iter()
        .find(|record| record.record_type == RECORD_POINT_F64_PAIR)
        .and_then(|record| decode_point_record(record.payload(&file.data)))
        .map(|point| file.document_display_point(point))
}

fn decode_object_parameter(file: &GspFile, group: &ObjectGroup) -> Option<f64> {
    group
        .records
        .iter()
        .find(|record| record.record_type == RECORD_BINDING_PAYLOAD)
        .map(|record| record.payload(&file.data))
        .filter(|payload| payload.len() >= 12)
        .map(|payload| read_f64(payload, 4))
        .filter(|value| value.is_finite())
}

fn decode_graph_calibration_unit_length(file: &GspFile, group: &ObjectGroup) -> Option<f64> {
    group
        .records
        .iter()
        .find(|record| {
            record.record_type == crate::runtime::payload_consts::RECORD_BINDING_PAYLOAD
                && record.length == 12
        })
        .and_then(|record| decode::decode_measurement_value(record.payload(&file.data)))
}

fn decode_htm_object_parameter(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
) -> Option<f64> {
    let graph = None;
    match try_decode_point_constraint(file, groups, group, None, &graph).ok()? {
        RawPointConstraint::Segment(constraint) => Some(constraint.t),
        RawPointConstraint::ConstructedLine { t, .. } => Some(t),
        RawPointConstraint::Polyline { t, .. } => Some(t),
        RawPointConstraint::PolygonBoundary { t, .. } => Some(t),
        RawPointConstraint::TranslatedPolygonBoundary { t, .. } => Some(t),
        RawPointConstraint::Circle(constraint) => {
            Some((-constraint.unit_y).atan2(constraint.unit_x))
        }
        RawPointConstraint::Circular(constraint) => {
            Some((-constraint.unit_y).atan2(constraint.unit_x))
        }
        RawPointConstraint::CircleArc(constraint) => Some(constraint.t),
        RawPointConstraint::Arc(constraint) => Some(constraint.t),
    }
    .or_else(|| decode_object_parameter(file, group))
}
