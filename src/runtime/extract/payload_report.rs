use super::*;
use crate::runtime::geometry;
use anyhow::bail;
use std::fmt::Write as _;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone)]
struct UnsupportedPayloadIssue {
    summary: String,
    group_ordinals: Vec<usize>,
}

pub(super) fn validate_scene_payloads(file: &GspFile, groups: &[ObjectGroup]) -> Result<()> {
    let issues = collect_unsupported_payload_issues(file, groups);
    if issues.is_empty() {
        return Ok(());
    }
    bail!(
        "unsupported payloads:\n- {}",
        issues
            .iter()
            .map(|issue| issue.summary.as_str())
            .collect::<Vec<_>>()
            .join("\n- ")
    )
}

pub(crate) fn render_payload_log(source_path: &Path, file: &GspFile) -> String {
    let groups = file.object_groups();
    let issues = collect_unsupported_payload_issues(file, &groups);

    let mut output = String::new();
    let _ = writeln!(output, "载荷说明");
    let _ = writeln!(output, "文件: {}", source_path.display());
    let _ = writeln!(output, "问题数量: {}", issues.len());
    let _ = writeln!(output, "对象组数量: {}", groups.len());
    let _ = writeln!(output);
    let _ = writeln!(output, "问题列表");

    if issues.is_empty() {
        let _ = writeln!(output, "未发现不支持的载荷。");
    } else {
        for (index, issue) in issues.iter().enumerate() {
            let _ = writeln!(
                output,
                "{}. {}",
                index + 1,
                describe_issue_in_chinese(&issue.summary, &issue.group_ordinals)
            );
            let related_ordinals =
                collect_related_group_ordinals(file, &groups, &issue.group_ordinals);
            if !related_ordinals.is_empty() {
                let _ = writeln!(output, "   相关对象：");
                for (related_index, ordinal) in related_ordinals.iter().enumerate() {
                    if let Some(group) = groups.get(ordinal.saturating_sub(1)) {
                        let _ = writeln!(
                            output,
                            "   {}. {}",
                            related_index + 1,
                            describe_group_in_chinese(file, &groups, group)
                        );
                    }
                }
            }
            let _ = writeln!(output, "   原始载荷：");
            for ordinal in &issue.group_ordinals {
                if let Some(group) = groups.get(ordinal.saturating_sub(1)) {
                    write_group_detail(&mut output, file, group, "   ");
                }
            }
        }
    }

    let _ = writeln!(output);
    let _ = writeln!(output, "Construction VALUE");
    if let Some(reference_lines) = read_reference_htm_construction_lines(source_path) {
        for line in reference_lines {
            let _ = writeln!(output, "{line}");
        }
    } else {
        let construction_groups = collect_htm_payload_groups(file, &groups);
        let construction_ordinals = construction_groups
            .iter()
            .enumerate()
            .map(|(index, group)| (group.ordinal, index + 1))
            .collect::<BTreeMap<_, _>>();
        let point_map = collect_point_objects(file, &groups);
        let raw_anchors_for_graph = collect_raw_object_anchors(file, &groups, &point_map, None);
        let graph = detect_graph_transform(file, &groups, &raw_anchors_for_graph);
        let htm_context = HtmPayloadContext {
            ordinal_map: &construction_ordinals,
            graph: graph.as_ref(),
            has_point_function_plot: groups.iter().any(|group| {
                matches!(
                    group.header.kind(),
                    GroupKind::FunctionPlot | GroupKind::LegacyFunctionPlot
                ) && find_indexed_path(file, group)
                    .and_then(|path| path.refs.first().copied())
                    .and_then(|ordinal| groups.get(ordinal.saturating_sub(1)))
                    .is_some_and(|source| source.header.kind() == GroupKind::Point)
                    && htm_function_plot_mode(file, group) == Some(FunctionPlotMode::Cartesian)
            }),
        };
        for (index, group) in construction_groups.iter().enumerate() {
            let _ = writeln!(
                output,
                "{}",
                describe_group_as_htm_payload(file, &groups, group, index + 1, &htm_context)
            );
        }
    }
    let _ = writeln!(output);
    let _ = writeln!(output, "Payload Objects");
    for (index, group) in groups.iter().enumerate() {
        let _ = writeln!(
            output,
            "{}. {}",
            index + 1,
            describe_group_in_chinese(file, &groups, group)
        );
    }

    output
}

fn read_reference_htm_construction_lines(source_path: &Path) -> Option<Vec<String>> {
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

fn collect_htm_payload_groups<'a>(
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
                    || self::decode::is_parameter_control_group(group)
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
            | GroupKind::LineKind5
            | GroupKind::LineKind6
            | GroupKind::LineKind7
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
        && group
            .records
            .iter()
            .any(|record| matches!(record.record_type, 0x07d5 | 0x07d8))
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

struct HtmPayloadContext<'a> {
    ordinal_map: &'a BTreeMap<usize, usize>,
    graph: Option<&'a GraphTransform>,
    has_point_function_plot: bool,
}

fn describe_group_as_htm_payload(
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
        GroupKind::Point if self::decode::is_parameter_control_group(group) => {
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
        GroupKind::MeasurementLine => match self::decode::decode_label_name(file, group).as_deref()
        {
            Some("x") => ("HorizontalAxis", format_ref_args(refs, ordinal_map)),
            Some("y") => ("VerticalAxis", format_ref_args(refs, ordinal_map)),
            _ => ("Line", format_reversed_ref_args(refs, ordinal_map)),
        },
        GroupKind::AxisLine => ("CoordSysByAxes", format_ref_args(refs, ordinal_map)),
        GroupKind::LineKind5 => ("Perpendicular", format_reversed_ref_args(refs, ordinal_map)),
        GroupKind::LineKind6 => ("Parallel", format_reversed_ref_args(refs, ordinal_map)),
        GroupKind::LineKind7 => ("Bisector", format_ref_args(refs, ordinal_map)),
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
        GroupKind::LineKind5 => "Perpendicular",
        GroupKind::LineKind6 => "Parallel",
        GroupKind::LineKind7 => "Bisector",
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
    if group.header.kind() == GroupKind::Point && self::decode::is_parameter_control_group(group) {
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
        && self::decode::decode_label_visible(file, group).unwrap_or(!group.header.is_hidden())
        && let Some(name) = self::decode::decode_label_name_raw(file, group)
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
        | GroupKind::LineKind5
        | GroupKind::LineKind6
        | GroupKind::LineKind7
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
    matches!((style_a >> 16) & 0xff, 0x12 | 0x22)
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
        && !self::decode::is_parameter_control_group(group)
        && decode_group_point(file, group).is_none()
        && group
            .records
            .iter()
            .any(|record| matches!(record.record_type, 0x08a3 | 0x08a8 | 0x08fc))
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
    try_decode_bbox_rect_raw(file, group)
        .ok()
        .flatten()
        .map(|(left, top, _width, height)| {
            let y_offset = htm_fixed_text_bbox_y_offset(
                rich_text
                    .as_ref()
                    .and_then(|content| content.markup.as_deref()),
                &text,
                height,
            );
            PointRecord {
                x: left,
                y: top + y_offset,
            }
        })
        .or_else(|| try_decode_payload_anchor_point(file, group).ok().flatten())
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

fn htm_fixed_text_bbox_y_offset(markup: Option<&str>, text: &str, height: f64) -> f64 {
    let Some(markup) = markup else {
        return 48.0;
    };
    if markup.starts_with("<H") {
        return 36.0;
    }
    if markup.starts_with("<VL<H") && height < 70.0 {
        return 48.0;
    }
    if text.contains('\n') {
        return 40.0;
    }
    if height >= 70.0 {
        return 32.0;
    }
    if height >= 50.0 {
        return 36.0;
    }
    24.0
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

fn htm_function_plot_mode(file: &GspFile, group: &ObjectGroup) -> Option<FunctionPlotMode> {
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
                .or_else(|| self::decode::decode_label_name_raw(file, group))?;
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
        | GroupKind::LineKind5
        | GroupKind::LineKind6
        | GroupKind::LineKind7
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
    self::decode::decode_label_name_raw(file, group)
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
    let anchor = self::decode::decode_button_screen_anchor(file, group)
        .unwrap_or(PointRecord { x: 24.0, y: 24.0 });
    let text =
        self::decode::decode_label_name_raw(file, group).unwrap_or_else(|| "按钮".to_string());
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
    let anchor = self::decode::decode_button_screen_anchor(file, group)
        .unwrap_or(PointRecord { x: 24.0, y: 24.0 });
    let text =
        self::decode::decode_label_name_raw(file, group).unwrap_or_else(|| "动画点".to_string());
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
        .find(|record| record.record_type == 0x0906)
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
        .find(|record| record.record_type == 0x07d3 && record.length == 12)
        .and_then(|record| self::decode::decode_measurement_value(record.payload(&file.data)))
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

fn collect_related_group_ordinals(
    file: &GspFile,
    groups: &[ObjectGroup],
    root_ordinals: &[usize],
) -> Vec<usize> {
    let mut visited = BTreeSet::new();
    let mut ordered = Vec::new();
    for ordinal in root_ordinals {
        visit_group_dependencies(file, groups, *ordinal, &mut visited, &mut ordered);
    }
    ordered
}

fn visit_group_dependencies(
    file: &GspFile,
    groups: &[ObjectGroup],
    ordinal: usize,
    visited: &mut BTreeSet<usize>,
    ordered: &mut Vec<usize>,
) {
    if ordinal == 0 || !visited.insert(ordinal) {
        return;
    }
    if let Some(group) = groups.get(ordinal.saturating_sub(1)) {
        ordered.push(ordinal);
        if let Some(path) = find_indexed_path(file, group) {
            for ref_ordinal in path.refs {
                visit_group_dependencies(file, groups, ref_ordinal, visited, ordered);
            }
        }
    }
}

fn describe_issue_in_chinese(summary: &str, group_ordinals: &[usize]) -> String {
    let target = group_ordinals
        .first()
        .map(|ordinal| format!("对象 #{}", ordinal))
        .unwrap_or_else(|| "当前对象".to_string());

    if let Some(rest) = summary.strip_prefix("unsupported payload: unknown object kind ")
        && let Some((raw, _)) = rest.split_once(" in ")
    {
        return format!("{target} 暂时无法导出，因为对象类型 {raw} 还没有实现。");
    }
    if let Some(rest) =
        summary.strip_prefix("unsupported payload: action button payload too short (")
        && let Some((bytes, _)) = rest.split_once(" bytes) in ")
    {
        return format!("{target} 暂时无法导出，因为按钮载荷只有 {bytes} 字节，长度不足。");
    }
    if let Some(rest) =
        summary.strip_prefix("unsupported payload: action button uses unsupported action kind (")
        && let Some((action_kind, _)) = rest.split_once(") in ")
    {
        return format!("{target} 暂时无法导出，因为按钮动作类型 ({action_kind}) 目前还不支持。");
    }
    if let Some(rest) = summary.strip_prefix("unsupported payload: malformed image payload in ")
        && let Some((_, sizes)) = rest.split_once(" (")
    {
        let sizes = sizes.trim_end_matches(')');
        return format!("{target} 暂时无法导出，因为图片载荷结构不完整（{sizes}）。");
    }
    if let Some(rest) = summary.strip_prefix("unsupported payload: non-positive image dimensions (")
        && let Some((dimensions, _)) = rest.split_once(") in ")
    {
        return format!("{target} 暂时无法导出，因为图片尺寸 {dimensions} 无效。");
    }
    if summary.starts_with("unsupported payload: non-finite image transform in ") {
        return format!("{target} 暂时无法导出，因为图片变换参数不是有限数值。");
    }
    if summary.starts_with("unsupported payload: non-axis-aligned image transform in ") {
        return format!("{target} 暂时无法导出，因为图片变换不是轴对齐矩形。");
    }
    if summary.starts_with("unsupported payload: function plot is missing indexed path in ") {
        return format!("{target} 暂时无法导出，因为函数图像缺少索引路径。");
    }
    if let Some(rest) = summary.strip_prefix("unsupported payload: function plot path has ")
        && let Some((refs, _)) = rest.split_once(" refs in ")
    {
        return format!("{target} 暂时无法导出，因为函数图像路径只有 {refs} 个引用。");
    }
    if let Some(rest) = summary
        .strip_prefix("unsupported payload: function plot references missing definition group #")
        && let Some((definition_ordinal, _)) = rest.split_once(" from ")
    {
        return format!(
            "{target} 暂时无法导出，因为它引用的函数定义对象组 #{definition_ordinal} 不存在。"
        );
    }
    if summary.starts_with("unsupported payload: invalid function plot descriptor in ") {
        return format!("{target} 暂时无法导出，因为函数图像描述符无效。");
    }
    if summary.starts_with("unsupported payload: invalid function expression in ") {
        return format!("{target} 暂时无法导出，因为关联的函数表达式无法解析。");
    }
    if let Some(rest) = summary.strip_prefix("unsupported payload: missing ")
        && let Some((record_label, _)) = rest.split_once(" (record ")
    {
        return format!("{target} 暂时无法导出，因为缺少“{record_label}”记录。");
    }

    format!("{target} 暂时无法导出。原始诊断：{summary}")
}

fn describe_group_in_chinese(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
) -> String {
    let refs = find_indexed_path(file, group)
        .map(|path| path.refs)
        .unwrap_or_default();
    let mut detail = match group.header.kind() {
        GroupKind::Point => describe_point_group_in_chinese(file, &refs, group),
        GroupKind::Midpoint => refs
            .first()
            .map(|host| format!("{} 的中点", format_ref(*host)))
            .unwrap_or_else(|| "中点对象".to_string()),
        GroupKind::Segment => describe_pair_relation(&refs, "线段", "连接"),
        GroupKind::Circle => {
            if refs.len() == 2 {
                format!(
                    "圆，圆心是 {}，并且经过 {}",
                    format_ref(refs[0]),
                    format_ref(refs[1])
                )
            } else {
                describe_generic_group(group, &refs)
            }
        }
        GroupKind::CircleCenterRadius => {
            if refs.len() == 2 {
                format!(
                    "圆，圆心是 {}，半径取自 {}",
                    format_ref(refs[0]),
                    format_ref_with_kind(groups, refs[1])
                )
            } else {
                describe_generic_group(group, &refs)
            }
        }
        GroupKind::Line => describe_pair_relation(&refs, "直线", "经过"),
        GroupKind::Ray => {
            if refs.len() == 2 {
                format!(
                    "射线，起点是 {}，方向经过 {}",
                    format_ref(refs[0]),
                    format_ref(refs[1])
                )
            } else {
                describe_generic_group(group, &refs)
            }
        }
        GroupKind::LineKind5 => {
            if refs.len() == 2 {
                format!(
                    "过 {} 且垂直于 {} 的直线",
                    format_ref(refs[0]),
                    format_ref_with_kind(groups, refs[1])
                )
            } else {
                describe_generic_group(group, &refs)
            }
        }
        GroupKind::LineKind6 => {
            if refs.len() == 2 {
                format!(
                    "过 {} 且平行于 {} 的直线",
                    format_ref(refs[0]),
                    format_ref_with_kind(groups, refs[1])
                )
            } else {
                describe_generic_group(group, &refs)
            }
        }
        GroupKind::LineKind7 => {
            if refs.len() == 3 {
                format!(
                    "以 {} 为顶点、夹在 {} 和 {} 之间的角平分线",
                    format_ref(refs[1]),
                    format_ref(refs[0]),
                    format_ref(refs[2])
                )
            } else {
                describe_generic_group(group, &refs)
            }
        }
        GroupKind::Polygon => {
            if refs.is_empty() {
                "多边形".to_string()
            } else {
                format!("多边形，顶点顺序是 {}", format_ref_list(&refs))
            }
        }
        GroupKind::LinearIntersectionPoint => describe_intersection_point(&refs, None),
        GroupKind::IntersectionPoint1 => describe_intersection_point(&refs, Some("第一个")),
        GroupKind::IntersectionPoint2 => describe_intersection_point(&refs, Some("第二个")),
        GroupKind::CircleCircleIntersectionPoint1 => {
            describe_circle_intersection_point(&refs, Some("第一个"))
        }
        GroupKind::CircleCircleIntersectionPoint2 => {
            describe_circle_intersection_point(&refs, Some("第二个"))
        }
        GroupKind::PointConstraint | GroupKind::PathPoint => refs
            .first()
            .map(|host| format!("位于 {} 上的动点", format_ref_with_kind(groups, *host)))
            .unwrap_or_else(|| "受约束的动点".to_string()),
        GroupKind::Translation => describe_translation_group_in_chinese(groups, &refs),
        GroupKind::CartesianOffsetPoint | GroupKind::PolarOffsetPoint => {
            describe_offset_point_in_chinese(file, group, &refs)
        }
        GroupKind::ExpressionOffsetPoint => {
            if refs.len() >= 2 {
                format!(
                    "以 {} 为基准、按 {} 做水平偏移得到的点",
                    format_ref_with_kind(groups, refs[0]),
                    format_ref_with_kind(groups, refs[1])
                )
            } else {
                describe_generic_group(group, &refs)
            }
        }
        GroupKind::LegacyCoordinateConstructPoint => {
            if refs.len() >= 4 {
                format!(
                    "按 {}、{} 与 {}、{} 构造得到的坐标点",
                    format_ref_with_kind(groups, refs[0]),
                    format_ref_with_kind(groups, refs[1]),
                    format_ref_with_kind(groups, refs[2]),
                    format_ref_with_kind(groups, refs[3])
                )
            } else {
                describe_generic_group(group, &refs)
            }
        }
        GroupKind::Rotation => describe_rotation_group_in_chinese(file, groups, group),
        GroupKind::AngleRotation => describe_angle_rotation_group_in_chinese(file, groups, group),
        GroupKind::ParameterRotation => {
            describe_parameter_rotation_group_in_chinese(file, groups, group)
        }
        GroupKind::ExpressionRotation => {
            if refs.len() >= 3 {
                if groups
                    .get(refs[2].saturating_sub(1))
                    .is_some_and(|group| group.header.kind() == GroupKind::RatioValue)
                {
                    return format!(
                        "将 {} 以 {} 为中心，按 {} 缩放得到的点",
                        format_ref_with_kind(groups, refs[0]),
                        format_ref(refs[1]),
                        format_ref_with_kind(groups, refs[2])
                    );
                }
                format!(
                    "将 {} 围绕 {} 按 {} 旋转得到的点",
                    format_ref_with_kind(groups, refs[0]),
                    format_ref(refs[1]),
                    format_ref_with_kind(groups, refs[2])
                )
            } else {
                describe_generic_group(group, &refs)
            }
        }
        GroupKind::Scale => describe_scale_group_in_chinese(file, groups, group),
        GroupKind::RatioScale => {
            if refs.len() >= 5 {
                format!(
                    "将 {} 以 {} 为中心，按 {} 到 {} 与 {} 到 {} 的长度比缩放得到的对象",
                    format_ref_with_kind(groups, refs[0]),
                    format_ref(refs[1]),
                    format_ref(refs[2]),
                    format_ref(refs[4]),
                    format_ref(refs[2]),
                    format_ref(refs[3])
                )
            } else {
                describe_generic_group(group, &refs)
            }
        }
        GroupKind::DistanceValue => {
            if refs.len() >= 2 {
                format!(
                    "{} 与 {} 的距离值",
                    format_ref(refs[0]),
                    format_ref(refs[1])
                )
            } else {
                describe_generic_group(group, &refs)
            }
        }
        GroupKind::PointLineDistanceValue => {
            if refs.len() >= 2 {
                format!(
                    "{} 到 {} 的距离值",
                    format_ref(refs[0]),
                    format_ref_with_kind(groups, refs[1])
                )
            } else {
                describe_generic_group(group, &refs)
            }
        }
        GroupKind::Reflection => {
            if refs.len() >= 2 {
                format!(
                    "把 {} 关于 {} 镜像得到的对象",
                    format_ref_with_kind(groups, refs[0]),
                    format_ref_with_kind(groups, refs[1])
                )
            } else {
                describe_generic_group(group, &refs)
            }
        }
        GroupKind::CircleInterior => refs
            .first()
            .map(|host| format!("以 {} 为边界的圆面", format_ref_with_kind(groups, *host)))
            .unwrap_or_else(|| "圆面".to_string()),
        GroupKind::CoordinateXValue => refs
            .first()
            .map(|host| format!("{} 的图像 x 坐标值", format_ref(*host)))
            .unwrap_or_else(|| "图像 x 坐标值".to_string()),
        GroupKind::CoordinateYValue => refs
            .first()
            .map(|host| format!("{} 的图像 y 坐标值", format_ref(*host)))
            .unwrap_or_else(|| "图像 y 坐标值".to_string()),
        GroupKind::ActionButton => describe_action_button_group_in_chinese(file, group, &refs),
        GroupKind::FunctionPlot => describe_function_plot_group_in_chinese(groups, &refs),
        GroupKind::ArcOnCircle => {
            if refs.len() == 3 {
                format!(
                    "在 {} 上，从 {} 到 {} 的圆弧",
                    format_ref_with_kind(groups, refs[0]),
                    format_ref(refs[1]),
                    format_ref(refs[2])
                )
            } else {
                describe_generic_group(group, &refs)
            }
        }
        GroupKind::CenterArc => {
            if refs.len() == 3 {
                format!(
                    "以 {} 为圆心、从 {} 到 {} 的圆弧",
                    format_ref(refs[0]),
                    format_ref(refs[1]),
                    format_ref(refs[2])
                )
            } else {
                describe_generic_group(group, &refs)
            }
        }
        GroupKind::ThreePointArc => {
            if refs.len() == 3 {
                format!(
                    "经过 {}、{}、{} 的三点圆弧",
                    format_ref(refs[0]),
                    format_ref(refs[1]),
                    format_ref(refs[2])
                )
            } else {
                describe_generic_group(group, &refs)
            }
        }
        GroupKind::SectorBoundary => {
            if refs.len() == 3 {
                format!(
                    "由 {}、{}、{} 定义的扇形边界",
                    format_ref(refs[0]),
                    format_ref(refs[1]),
                    format_ref(refs[2])
                )
            } else {
                describe_generic_group(group, &refs)
            }
        }
        GroupKind::CircularSegmentBoundary => {
            if refs.len() == 3 {
                format!(
                    "由 {}、{}、{} 定义的弓形边界",
                    format_ref(refs[0]),
                    format_ref(refs[1]),
                    format_ref(refs[2])
                )
            } else {
                describe_generic_group(group, &refs)
            }
        }
        GroupKind::CoordinatePoint
        | GroupKind::CoordinateExpressionPoint
        | GroupKind::CoordinateExpressionPointAlt
        | GroupKind::FixedCoordinatePoint
        | GroupKind::CoordinateExpressionPointPair => {
            if refs.is_empty() {
                "坐标点".to_string()
            } else {
                format!("坐标点，依赖 {}", format_ref_list(&refs))
            }
        }
        GroupKind::PointTrace => refs
            .first()
            .map(|host| format!("{} 的轨迹", format_ref_with_kind(groups, *host)))
            .unwrap_or_else(|| "点轨迹".to_string()),
        GroupKind::CoordinateTrace => refs
            .first()
            .map(|host| format!("{} 的坐标轨迹", format_ref_with_kind(groups, *host)))
            .unwrap_or_else(|| "坐标轨迹".to_string()),
        GroupKind::CoordinateTraceIntersectionPoint => {
            if refs.len() >= 2 {
                format!(
                    "{} 和 {} 的交点",
                    format_ref_with_kind(groups, refs[0]),
                    format_ref_with_kind(groups, refs[1])
                )
            } else {
                "轨迹交点".to_string()
            }
        }
        GroupKind::AngleMarker => {
            if refs.len() == 3 {
                format!(
                    "角标记，顶点是 {}，两边经过 {} 和 {}",
                    format_ref(refs[1]),
                    format_ref(refs[0]),
                    format_ref(refs[2])
                )
            } else {
                describe_generic_group(group, &refs)
            }
        }
        GroupKind::SegmentMarker => refs
            .first()
            .map(|host| {
                format!(
                    "用于标记 {} 的线段记号",
                    format_ref_with_kind(groups, *host)
                )
            })
            .unwrap_or_else(|| "线段记号".to_string()),
        _ => describe_generic_group(group, &refs),
    };

    let mut annotations = Vec::new();
    if let Some(name) = self::decode::decode_label_name_raw(file, group) {
        annotations.push(format!("名称“{}”", truncate_text(name.trim(), 48)));
    }
    if let Some(text) = try_decode_group_label_text(file, group) {
        let text = text.trim();
        if !text.is_empty() {
            annotations.push(format!("文字“{}”", truncate_text(text, 48)));
        }
    }
    match try_decode_link_button_url(file, group) {
        Ok(Some(url)) => annotations.push(format!("链接“{}”", truncate_text(url.trim(), 64))),
        Ok(None) => {}
        Err(error) => annotations.push(format!("链接解析失败（{}）", error)),
    }
    if !annotations.is_empty() {
        detail.push_str(&format!("，{}", annotations.join("，")));
    }

    format!("#{} = {}。", group.ordinal, detail)
}

fn describe_point_group_in_chinese(file: &GspFile, refs: &[usize], group: &ObjectGroup) -> String {
    let has_explicit_point = group
        .records
        .iter()
        .any(|record| record.record_type == RECORD_POINT_F64_PAIR);
    let has_image_payload = [0x090c, 0x08a8, 0x1f44].into_iter().all(|record_type| {
        group
            .records
            .iter()
            .any(|record| record.record_type == record_type)
    });
    if has_image_payload {
        return "图片锚点".to_string();
    }
    if self::decode::is_parameter_control_group(group) {
        return "参数控制点".to_string();
    }
    if has_explicit_point && refs.is_empty() {
        return "自由点".to_string();
    }
    if refs.is_empty() {
        return "点".to_string();
    }
    let point = group
        .records
        .iter()
        .find(|record| record.record_type == RECORD_POINT_F64_PAIR)
        .and_then(|record| decode_point_record(record.payload(&file.data)));
    if let Some(point) = point {
        return format!(
            "点，当前坐标是 ({}, {})，并且依赖 {}",
            format_number(point.x),
            format_number(point.y),
            format_ref_list(refs)
        );
    }
    format!("点，依赖 {}", format_ref_list(refs))
}

fn describe_pair_relation(refs: &[usize], noun: &str, verb: &str) -> String {
    if refs.len() == 2 {
        format!(
            "{noun}，{verb} {} 和 {}",
            format_ref(refs[0]),
            format_ref(refs[1])
        )
    } else {
        format!("{noun}，按载荷顺序引用 {}", format_ref_list(refs))
    }
}

fn describe_intersection_point(refs: &[usize], variant: Option<&str>) -> String {
    if refs.len() >= 2 {
        let prefix = variant.unwrap_or("");
        format!(
            "{prefix}交点，来自 {} 和 {}",
            format_ref(refs[0]),
            format_ref(refs[1])
        )
    } else {
        "交点".to_string()
    }
}

fn describe_circle_intersection_point(refs: &[usize], variant: Option<&str>) -> String {
    if refs.len() >= 2 {
        let prefix = variant.unwrap_or("");
        format!(
            "{prefix}圆交点，来自 {} 和 {}",
            format_ref(refs[0]),
            format_ref(refs[1])
        )
    } else {
        "圆交点".to_string()
    }
}

fn describe_translation_group_in_chinese(groups: &[ObjectGroup], refs: &[usize]) -> String {
    if refs.len() >= 3 {
        return format!(
            "将 {} 按向量 {} -> {} 平移得到的对象",
            format_ref_with_kind(groups, refs[0]),
            format_ref(refs[1]),
            format_ref(refs[2])
        );
    }
    "平移对象".to_string()
}

fn describe_offset_point_in_chinese(file: &GspFile, group: &ObjectGroup, refs: &[usize]) -> String {
    if let Some(constraint) = decode_translated_point_constraint(file, group)
        && let Some(origin) = refs.first()
    {
        return format!(
            "从 {} 平移 ({}, {}) 得到的点",
            format_ref(*origin),
            format_number(constraint.dx),
            format_number(constraint.dy)
        );
    }
    if let Some(origin) = refs.first() {
        return format!("从 {} 偏移得到的点", format_ref(*origin));
    }
    "偏移点".to_string()
}

fn describe_rotation_group_in_chinese(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
) -> String {
    if let Ok(binding) = try_decode_transform_binding(file, group) {
        let source_ordinal = binding.source_group_index + 1;
        let center_ordinal = binding.center_group_index + 1;
        if let TransformBindingKind::Rotate { angle_degrees, .. } = binding.kind {
            return format!(
                "将 {} 围绕 {} 旋转 {} 度得到的对象",
                format_ref_with_kind(groups, source_ordinal),
                format_ref(center_ordinal),
                format_number(angle_degrees)
            );
        }
    }
    describe_generic_group(
        group,
        &find_indexed_path(file, group)
            .map(|path| path.refs)
            .unwrap_or_default(),
    )
}

fn describe_parameter_rotation_group_in_chinese(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
) -> String {
    if let Ok(binding) = try_decode_parameter_rotation_binding(file, groups, group) {
        let source_ordinal = binding.source_group_index + 1;
        let center_ordinal = binding.center_group_index + 1;
        if let TransformBindingKind::Rotate {
            angle_degrees,
            parameter_name,
        } = binding.kind
        {
            if let Some(parameter_name) = parameter_name {
                return format!(
                    "将 {} 围绕 {} 按参数 {} 旋转得到的对象（当前角度 {} 度）",
                    format_ref_with_kind(groups, source_ordinal),
                    format_ref(center_ordinal),
                    parameter_name,
                    format_number(angle_degrees)
                );
            }
            return format!(
                "将 {} 围绕 {} 旋转 {} 度得到的对象",
                format_ref_with_kind(groups, source_ordinal),
                format_ref(center_ordinal),
                format_number(angle_degrees)
            );
        }
    }
    describe_generic_group(
        group,
        &find_indexed_path(file, group)
            .map(|path| path.refs)
            .unwrap_or_default(),
    )
}

fn describe_angle_rotation_group_in_chinese(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
) -> String {
    let refs = find_indexed_path(file, group)
        .map(|path| path.refs)
        .unwrap_or_default();
    if refs.len() >= 5 {
        return format!(
            "将 {} 围绕 {} 按 {}、{}、{} 所成角旋转得到的点",
            format_ref_with_kind(groups, refs[0]),
            format_ref(refs[1]),
            format_ref(refs[2]),
            format_ref(refs[3]),
            format_ref(refs[4])
        );
    }
    describe_generic_group(group, &refs)
}

fn describe_scale_group_in_chinese(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
) -> String {
    if let Ok(binding) = try_decode_transform_binding(file, group) {
        let source_ordinal = binding.source_group_index + 1;
        let center_ordinal = binding.center_group_index + 1;
        if let TransformBindingKind::Scale { factor } = binding.kind {
            return format!(
                "将 {} 以 {} 为中心缩放 {} 倍得到的对象",
                format_ref_with_kind(groups, source_ordinal),
                format_ref(center_ordinal),
                format_number(factor)
            );
        }
    }
    describe_generic_group(
        group,
        &find_indexed_path(file, group)
            .map(|path| path.refs)
            .unwrap_or_default(),
    )
}

fn describe_action_button_group_in_chinese(
    file: &GspFile,
    group: &ObjectGroup,
    refs: &[usize],
) -> String {
    let action_kind = group
        .records
        .iter()
        .find(|record| record.record_type == 0x0906)
        .map(|record| record.payload(&file.data))
        .filter(|payload| payload.len() >= 16)
        .map(|payload| (read_u16(payload, 12), read_u16(payload, 14)));
    let placement = if refs.is_empty() {
        "按钮".to_string()
    } else {
        format!("按钮，关联 {}", format_ref_list(refs))
    };
    if let Some((primary, secondary)) = action_kind {
        return format!("{placement}，动作类型是 ({primary}, {secondary})");
    }
    placement
}

fn describe_function_plot_group_in_chinese(groups: &[ObjectGroup], refs: &[usize]) -> String {
    if refs.len() >= 2 {
        return format!(
            "函数图像，定义来自 {}，并且依赖 {}",
            format_ref_with_kind(groups, refs[0]),
            format_ref_list(&refs[1..])
        );
    }
    if refs.len() == 1 {
        return format!(
            "函数图像，定义来自 {}",
            format_ref_with_kind(groups, refs[0])
        );
    }
    "函数图像".to_string()
}

fn describe_generic_group(group: &ObjectGroup, refs: &[usize]) -> String {
    match group.header.kind() {
        GroupKind::Unknown(raw) => {
            if refs.is_empty() {
                format!("未知对象，类型是 {raw}")
            } else {
                format!(
                    "未知对象，类型是 {raw}，按载荷顺序引用 {}",
                    format_ref_list(refs)
                )
            }
        }
        kind => {
            let kind_name = group_kind_name_in_chinese(kind);
            if refs.is_empty() {
                kind_name.to_string()
            } else {
                format!("{kind_name}，按载荷顺序引用 {}", format_ref_list(refs))
            }
        }
    }
}

fn group_kind_name_in_chinese(kind: GroupKind) -> &'static str {
    match kind {
        GroupKind::Point => "点",
        GroupKind::Midpoint => "中点",
        GroupKind::Segment => "线段",
        GroupKind::Circle => "圆",
        GroupKind::CircleCenterRadius => "定圆心定半径圆",
        GroupKind::LineKind5 => "垂线",
        GroupKind::LineKind6 => "平行线",
        GroupKind::LineKind7 => "角平分线",
        GroupKind::Polygon => "多边形",
        GroupKind::LinearIntersectionPoint => "交点",
        GroupKind::CircleInterior => "圆面",
        GroupKind::IntersectionPoint1 => "第一个交点",
        GroupKind::IntersectionPoint2 => "第二个交点",
        GroupKind::CircleCircleIntersectionPoint1 => "第一个圆交点",
        GroupKind::CircleCircleIntersectionPoint2 => "第二个圆交点",
        GroupKind::PointConstraint => "路径动点",
        GroupKind::Translation => "平移对象",
        GroupKind::CartesianOffsetPoint => "直角坐标偏移点",
        GroupKind::CoordinateExpressionPoint => "坐标表达式点",
        GroupKind::CoordinateExpressionPointAlt => "坐标表达式点",
        GroupKind::CoordinateExpressionPointPair => "双坐标表达式点",
        GroupKind::PolarOffsetPoint => "极坐标偏移点",
        GroupKind::ExpressionOffsetPoint => "表达式偏移点",
        GroupKind::DerivedSegment24 => "派生线段",
        GroupKind::CustomTransformPoint => "自定义变换点",
        GroupKind::Rotation => "旋转对象",
        GroupKind::AngleRotation => "角度旋转点",
        GroupKind::ParameterRotation => "参数旋转对象",
        GroupKind::ExpressionRotation => "表达式旋转点",
        GroupKind::Scale => "缩放对象",
        GroupKind::RatioScale => "比例缩放对象",
        GroupKind::Reflection => "镜像对象",
        GroupKind::DistanceValue => "两点距离值",
        GroupKind::PointLineDistanceValue => "点到直线距离值",
        GroupKind::PointTrace => "点轨迹",
        GroupKind::MeasuredValue => "度量值",
        GroupKind::BoundaryLengthValue => "边界长度值",
        GroupKind::GraphObject40 => "图像对象",
        GroupKind::AngleValue => "角度值",
        GroupKind::PolygonAreaValue => "多边形面积值",
        GroupKind::ArcAngleValue => "圆弧角度值",
        GroupKind::BoundaryCurveLengthValue => "边界曲线长度值",
        GroupKind::RadiusValue => "半径值",
        GroupKind::CoordinateReadoutLabel => "坐标读数标签",
        GroupKind::RichTextLabel => "富文本标签",
        GroupKind::RatioValue => "比值对象",
        GroupKind::FunctionExpr => "函数表达式",
        GroupKind::Kind51 => "对象类型 51",
        GroupKind::GraphViewHelper => "图像视图辅助对象",
        GroupKind::GraphCalibrationX => "图像校准点 X",
        GroupKind::GraphCalibrationY | GroupKind::GraphCalibrationYAlt => "图像校准点 Y",
        GroupKind::GraphMeasurementSegment => "图像测量线",
        GroupKind::MeasurementLine => "测量线",
        GroupKind::AxisLine => "坐标轴",
        GroupKind::ActionButton => "动作按钮",
        GroupKind::Line => "直线",
        GroupKind::Ray => "射线",
        GroupKind::CoordinateXValue => "图像 x 坐标值",
        GroupKind::CoordinateYValue => "图像 y 坐标值",
        GroupKind::OffsetAnchor => "偏移锚点",
        GroupKind::FixedCoordinatePoint => "固定坐标点",
        GroupKind::CoordinatePoint => "坐标点",
        GroupKind::GraphFunctionPoint => "图像函数点",
        GroupKind::FunctionPlot | GroupKind::LegacyFunctionPlot => "函数图像",
        GroupKind::ParametricFunctionPlot => "参数曲线",
        GroupKind::ButtonLabel => "按钮标签",
        GroupKind::DerivedSegment75 => "派生线段",
        GroupKind::AffineIteration => "仿射迭代",
        GroupKind::IterationBinding => "迭代绑定",
        GroupKind::DerivativeFunction => "导函数",
        GroupKind::ArcOnCircle => "圆上弧",
        GroupKind::CenterArc => "圆心弧",
        GroupKind::ThreePointArc => "过三点弧",
        GroupKind::SectorBoundary => "扇形边界",
        GroupKind::CircularSegmentBoundary => "弓形边界",
        GroupKind::GraphDistanceValue => "图像距离值",
        GroupKind::RectImage => "矩形图片",
        GroupKind::IterationPointAlias => "迭代结果点",
        GroupKind::ValueTableRow => "数值表行",
        GroupKind::BoundaryIntersectionPoint => "边界交点",
        GroupKind::NamedAlias => "命名别名对象",
        GroupKind::FunctionDefinition => "函数定义对象",
        GroupKind::PolarAngleValue => "极角值",
        GroupKind::VertexAngleValue => "顶点角值",
        GroupKind::RegularPolygonIteration => "正多边形迭代",
        GroupKind::LabelIterationSeed => "标签迭代种子",
        GroupKind::IterationExpressionHelper => "迭代表达式辅助对象",
        GroupKind::ParameterAnchor => "参数锚点",
        GroupKind::ParameterControlledPoint => "参数控制点",
        GroupKind::SmoothCurvePlot => "平滑曲线",
        GroupKind::CoordinateTrace => "坐标轨迹",
        GroupKind::CoordinateTraceIntersectionPoint => "坐标轨迹交点",
        GroupKind::CustomTransformTrace => "自定义变换轨迹",
        GroupKind::LegacyCoordinateParameterHelper => "旧版坐标参数辅助对象",
        GroupKind::LegacyCoordinatePointHelper => "旧版坐标点辅助对象",
        GroupKind::GraphValuePoint => "图像数值点",
        GroupKind::GraphSlopeValue => "图像斜率值",
        GroupKind::PointAlias => "点别名",
        GroupKind::ThreePointDerivedPoint => "三点派生点",
        GroupKind::ProjectedCoordinatePoint => "投影坐标点",
        GroupKind::PointReferenceAlias => "点引用别名",
        GroupKind::AngleMarker => "角标记",
        GroupKind::LegacyAngleMarker => "旧版角标记",
        GroupKind::LegacyAngleRotation => "旧版角度旋转点",
        GroupKind::LegacyVisibilityHelper => "旧版显隐辅助对象",
        GroupKind::LegacyCircularConstraintHelper => "旧版圆形约束辅助对象",
        GroupKind::LegacyCoordinateConstructPoint => "旧版坐标构造点",
        GroupKind::PathPoint => "路径点",
        GroupKind::GraphYValue => "图像 y 值",
        GroupKind::GraphXValue => "图像 x 值",
        GroupKind::SegmentMarker => "线段记号",
        GroupKind::Unknown(_) => "未知对象",
    }
}

fn group_kind_noun_in_chinese(kind: GroupKind) -> &'static str {
    match kind {
        GroupKind::Point
        | GroupKind::Midpoint
        | GroupKind::LinearIntersectionPoint
        | GroupKind::IntersectionPoint1
        | GroupKind::IntersectionPoint2
        | GroupKind::CircleCircleIntersectionPoint1
        | GroupKind::CircleCircleIntersectionPoint2
        | GroupKind::PointConstraint
        | GroupKind::CartesianOffsetPoint
        | GroupKind::CoordinateExpressionPoint
        | GroupKind::CoordinateExpressionPointAlt
        | GroupKind::CoordinateExpressionPointPair
        | GroupKind::FixedCoordinatePoint
        | GroupKind::PolarOffsetPoint
        | GroupKind::ExpressionOffsetPoint
        | GroupKind::CustomTransformPoint
        | GroupKind::AngleRotation
        | GroupKind::LegacyAngleRotation
        | GroupKind::ExpressionRotation
        | GroupKind::OffsetAnchor
        | GroupKind::CoordinatePoint
        | GroupKind::LegacyCoordinateConstructPoint
        | GroupKind::GraphFunctionPoint
        | GroupKind::GraphValuePoint
        | GroupKind::NamedAlias
        | GroupKind::PointAlias
        | GroupKind::ThreePointDerivedPoint
        | GroupKind::ProjectedCoordinatePoint
        | GroupKind::PointReferenceAlias
        | GroupKind::LegacyCoordinateParameterHelper
        | GroupKind::LegacyCoordinatePointHelper
        | GroupKind::ParameterAnchor
        | GroupKind::ParameterControlledPoint
        | GroupKind::CoordinateTraceIntersectionPoint
        | GroupKind::PathPoint
        | GroupKind::IterationPointAlias
        | GroupKind::BoundaryIntersectionPoint => "点",
        GroupKind::DistanceValue
        | GroupKind::PointLineDistanceValue
        | GroupKind::BoundaryLengthValue
        | GroupKind::ArcAngleValue
        | GroupKind::BoundaryCurveLengthValue
        | GroupKind::AngleValue
        | GroupKind::PolarAngleValue
        | GroupKind::VertexAngleValue
        | GroupKind::PolygonAreaValue
        | GroupKind::RatioValue
        | GroupKind::GraphDistanceValue
        | GroupKind::GraphSlopeValue
        | GroupKind::ValueTableRow
        | GroupKind::MeasuredValue
        | GroupKind::CoordinateXValue
        | GroupKind::CoordinateYValue
        | GroupKind::GraphYValue
        | GroupKind::GraphXValue => "数值对象",
        GroupKind::Segment | GroupKind::DerivedSegment75 | GroupKind::GraphMeasurementSegment => {
            "线段"
        }
        GroupKind::Line | GroupKind::LineKind5 | GroupKind::LineKind6 | GroupKind::LineKind7 => {
            "直线"
        }
        GroupKind::Ray => "射线",
        GroupKind::Circle | GroupKind::CircleCenterRadius => "圆",
        GroupKind::Polygon => "多边形",
        GroupKind::ArcOnCircle | GroupKind::CenterArc | GroupKind::ThreePointArc => "圆弧",
        GroupKind::CoordinateReadoutLabel => "标签",
        GroupKind::ActionButton => "按钮",
        GroupKind::FunctionPlot
        | GroupKind::LegacyFunctionPlot
        | GroupKind::ParametricFunctionPlot => "函数图像",
        GroupKind::AngleMarker | GroupKind::LegacyAngleMarker => "角标记",
        _ => "对象",
    }
}

fn format_ref(ordinal: usize) -> String {
    format!("#{ordinal}")
}

fn format_ref_with_kind(groups: &[ObjectGroup], ordinal: usize) -> String {
    groups
        .get(ordinal.saturating_sub(1))
        .map(|group| {
            format!(
                "{} #{}",
                group_kind_noun_in_chinese(group.header.kind()),
                ordinal
            )
        })
        .unwrap_or_else(|| format_ref(ordinal))
}

fn format_ref_list(refs: &[usize]) -> String {
    if refs.is_empty() {
        "无引用".to_string()
    } else {
        refs.iter()
            .map(|ordinal| format_ref(*ordinal))
            .collect::<Vec<_>>()
            .join("、")
    }
}

fn format_number(value: f64) -> String {
    let rounded = if value.abs() < 1e-9 { 0.0 } else { value };
    let text = format!("{rounded:.3}");
    text.trim_end_matches('0').trim_end_matches('.').to_string()
}

fn format_htm_parameter(value: f64) -> String {
    format_htm_significant(value, 6)
}

fn format_htm_unit_length(value: f64) -> String {
    format_htm_significant(value, 6)
}

fn format_htm_significant(value: f64, significant_digits: usize) -> String {
    let rounded = if value.abs() < 1e-9 { 0.0 } else { value };
    if rounded == 0.0 {
        return "0".to_string();
    }
    let digits_before_decimal = rounded.abs().log10().floor() as isize + 1;
    let decimals = (significant_digits as isize - digits_before_decimal).max(0) as usize;
    let text = format!("{rounded:.decimals$}");
    text.trim_end_matches('0').trim_end_matches('.').to_string()
}

fn collect_unsupported_payload_issues(
    file: &GspFile,
    groups: &[ObjectGroup],
) -> Vec<UnsupportedPayloadIssue> {
    let mut issues = Vec::new();
    for group in groups {
        collect_validation_issue(&mut issues, &[group.ordinal], validate_group_kind(group));
        collect_validation_issue(
            &mut issues,
            &[group.ordinal],
            validate_action_button_payload(file, group),
        );
        collect_validation_issue(
            &mut issues,
            &[group.ordinal],
            validate_image_payload(file, group),
        );
        collect_validation_issue(
            &mut issues,
            &function_issue_group_ordinals(file, groups, group),
            validate_function_payload(file, groups, group),
        );
    }
    issues
}

fn function_issue_group_ordinals(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
) -> Vec<usize> {
    let mut ordinals = vec![group.ordinal];
    if !matches!(
        group.header.kind(),
        GroupKind::FunctionPlot | GroupKind::ParametricFunctionPlot
    ) {
        return ordinals;
    }
    if let Some(path) = find_indexed_path(file, group)
        && let Some(definition_ordinal) = path.refs.first().copied()
        && definition_ordinal != group.ordinal
        && groups.get(definition_ordinal.saturating_sub(1)).is_some()
    {
        ordinals.push(definition_ordinal);
    }
    ordinals
}

fn collect_validation_issue(
    issues: &mut Vec<UnsupportedPayloadIssue>,
    group_ordinals: &[usize],
    result: Result<()>,
) {
    if let Err(error) = result {
        issues.push(UnsupportedPayloadIssue {
            summary: format!("{error:#}"),
            group_ordinals: group_ordinals.to_vec(),
        });
    }
}

fn validate_group_kind(group: &ObjectGroup) -> Result<()> {
    let kind = group.header.kind();
    if matches!(
        kind,
        GroupKind::CoordinateExpressionPointPair
            | GroupKind::LegacyVisibilityHelper
            | GroupKind::LegacyCircularConstraintHelper
            | GroupKind::LegacyCoordinateConstructPoint
            | GroupKind::DistanceValue
            | GroupKind::PointLineDistanceValue
            | GroupKind::CoordinateXValue
            | GroupKind::CoordinateYValue
            | GroupKind::GraphYValue
            | GroupKind::GraphXValue
            | GroupKind::FunctionDefinition
            | GroupKind::LegacyFunctionPlot
            | GroupKind::BoundaryLengthValue
            | GroupKind::AngleValue
            | GroupKind::ArcAngleValue
            | GroupKind::BoundaryCurveLengthValue
            | GroupKind::RadiusValue
            | GroupKind::RatioValue
            | GroupKind::GraphDistanceValue
            | GroupKind::GraphSlopeValue
            | GroupKind::PolygonAreaValue
            | GroupKind::GraphFunctionPoint
            | GroupKind::GraphMeasurementSegment
            | GroupKind::FixedCoordinatePoint
            | GroupKind::ValueTableRow
            | GroupKind::BoundaryIntersectionPoint
            | GroupKind::PointAlias
            | GroupKind::ThreePointDerivedPoint
            | GroupKind::ProjectedCoordinatePoint
            | GroupKind::PointReferenceAlias
            | GroupKind::GraphViewHelper
            | GroupKind::RichTextLabel
            | GroupKind::SmoothCurvePlot
            | GroupKind::LegacyAngleMarker
            | GroupKind::LegacyAngleRotation
            | GroupKind::PolarAngleValue
            | GroupKind::VertexAngleValue
            | GroupKind::NamedAlias
            | GroupKind::RectImage
            | GroupKind::IterationPointAlias
            | GroupKind::LegacyCoordinateParameterHelper
            | GroupKind::LegacyCoordinatePointHelper
    ) || is_supported_group_kind(kind)
    {
        return Ok(());
    }
    if let GroupKind::Unknown(raw) = kind {
        bail!(
            "unsupported payload: unknown object kind {raw} in {}",
            describe_group(group)
        );
    }
    Ok(())
}

fn validate_action_button_payload(file: &GspFile, group: &ObjectGroup) -> Result<()> {
    if !decode::is_action_button_group(group) {
        return Ok(());
    }

    let payload = group_record_payload(file, group, 0x0906, "action button payload")?;
    if payload.len() < 16 {
        bail!(
            "unsupported payload: action button payload too short ({} bytes) in {}",
            payload.len(),
            describe_group(group)
        );
    }

    let action_kind = (read_u16(payload, 12), read_u16(payload, 14));
    match action_kind {
        (2 | 8, 0) | (4, 0 | 1) | (7, _) | (3, 0..=3) | (0 | 1, 0..=7) => return Ok(()),
        _ => {}
    }

    bail!(
        "unsupported payload: action button uses unsupported action kind ({}, {}) in {}",
        action_kind.0,
        action_kind.1,
        describe_group(group)
    )
}

fn validate_image_payload(file: &GspFile, group: &ObjectGroup) -> Result<()> {
    if group.header.kind() != GroupKind::Point {
        return Ok(());
    }

    let has_image_records = [0x090c, 0x08a8, 0x1f44].into_iter().any(|record_type| {
        group
            .records
            .iter()
            .any(|record| record.record_type == record_type)
    });
    if !has_image_records {
        return Ok(());
    }

    let size_payload = group_record_payload(file, group, 0x090c, "image size payload")?;
    let transform_payload = group_record_payload(file, group, 0x08a8, "image transform payload")?;
    let resource_payload = group_record_payload(file, group, 0x1f44, "image resource payload")?;
    if size_payload.len() < 8 || transform_payload.len() < 48 || resource_payload.len() < 2 {
        bail!(
            "unsupported payload: malformed image payload in {} (size={}, transform={}, resource={})",
            describe_group(group),
            size_payload.len(),
            transform_payload.len(),
            resource_payload.len()
        );
    }

    let width = read_u32(size_payload, 0) as f64;
    let height = read_u32(size_payload, 4) as f64;
    if width <= 0.0 || height <= 0.0 {
        bail!(
            "unsupported payload: non-positive image dimensions ({width}x{height}) in {}",
            describe_group(group)
        );
    }

    let shear_x = read_f64(transform_payload, 8);
    let shear_y = read_f64(transform_payload, 24);
    if !shear_x.is_finite() || !shear_y.is_finite() {
        bail!(
            "unsupported payload: non-finite image transform in {}",
            describe_group(group)
        );
    }
    if shear_x.abs() > 1e-6 || shear_y.abs() > 1e-6 {
        bail!(
            "unsupported payload: non-axis-aligned image transform in {}",
            describe_group(group)
        );
    }

    Ok(())
}

fn validate_function_payload(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
) -> Result<()> {
    if !matches!(
        group.header.kind(),
        GroupKind::FunctionPlot | GroupKind::ParametricFunctionPlot
    ) {
        return Ok(());
    }

    let path = try_find_indexed_path(file, group)
        .map_err(anyhow::Error::msg)?
        .with_context(|| {
            format!(
                "unsupported payload: function plot is missing indexed path in {}",
                describe_group(group)
            )
        })?;
    if path.refs.len() < 2 {
        bail!(
            "unsupported payload: function plot path has {} refs in {}",
            path.refs.len(),
            describe_group(group)
        );
    }

    let definition_ordinal = path.refs[0];
    let definition_group = groups
        .get(definition_ordinal.checked_sub(1).context("function plot definition ordinal underflow")?)
        .with_context(|| {
            format!(
                "unsupported payload: function plot references missing definition group #{definition_ordinal} from {}",
                describe_group(group)
            )
        })?;
    let descriptor_payload = group_record_payload(
        file,
        group,
        RECORD_FUNCTION_PLOT_DESCRIPTOR,
        "function plot descriptor",
    )?;
    try_decode_function_plot_descriptor(descriptor_payload)
        .map_err(anyhow::Error::msg)
        .with_context(|| {
            format!(
                "unsupported payload: invalid function plot descriptor in {}",
                describe_group(group)
            )
        })?;
    let definition_kind = definition_group.header.kind();
    let definition_is_expression_bearing = matches!(
        definition_kind,
        GroupKind::FunctionExpr
            | GroupKind::DistanceValue
            | GroupKind::PointLineDistanceValue
            | GroupKind::CoordinateXValue
            | GroupKind::CoordinateYValue
            | GroupKind::FunctionDefinition
    ) || definition_group
        .records
        .iter()
        .any(|record| record.record_type == RECORD_FUNCTION_EXPR_PAYLOAD);
    if definition_is_expression_bearing {
        try_decode_function_expr(file, groups, definition_group)
            .map_err(anyhow::Error::msg)
            .with_context(|| {
                format!(
                    "unsupported payload: invalid function expression in {} referenced by {}",
                    describe_group(definition_group),
                    describe_group(group)
                )
            })?;
    }

    Ok(())
}

fn group_record_payload<'a>(
    file: &'a GspFile,
    group: &'a ObjectGroup,
    record_type: u32,
    record_label: &str,
) -> Result<&'a [u8]> {
    group
        .records
        .iter()
        .find(|record| record.record_type == record_type)
        .map(|record| record.payload(&file.data))
        .with_context(|| {
            format!(
                "unsupported payload: missing {record_label} (record 0x{record_type:04x}) in {}",
                describe_group(group)
            )
        })
}

fn write_group_detail(output: &mut String, file: &GspFile, group: &ObjectGroup, indent: &str) {
    let _ = writeln!(output, "{indent}对象 #{}：", group.ordinal);
    let _ = writeln!(
        output,
        "{indent}  类型: {:?} (raw=0x{:04x}, class_id=0x{:08x})",
        group.header.kind(),
        group.header.kind_id(),
        group.header.class_id
    );
    let _ = writeln!(
        output,
        "{indent}  几何属性: hidden={} flags=0x{:08x} style=[0x{:08x}, 0x{:08x}, 0x{:08x}]",
        group.header.is_hidden(),
        group.header.flags,
        group.header.style_a,
        group.header.style_b,
        group.header.style_c
    );
    let _ = writeln!(
        output,
        "{indent}  偏移: start=0x{:x} end=0x{:x}",
        group.start_offset, group.end_offset
    );

    if let Some(name) = self::decode::decode_label_name_raw(file, group) {
        let _ = writeln!(output, "{indent}  名称: {:?}", name);
    }
    if let Some(text) = try_decode_group_label_text(file, group) {
        let _ = writeln!(output, "{indent}  标签文字: {:?}", text);
    }
    if let Some(content) = try_decode_group_rich_text(file, group)
        && !content.hotspots.is_empty()
    {
        let _ = writeln!(
            output,
            "{indent}  富文本热点数量: {}",
            content.hotspots.len()
        );
    }
    match try_decode_link_button_url(file, group) {
        Ok(Some(url)) => {
            let _ = writeln!(output, "{indent}  动作链接: {:?}", url);
        }
        Ok(None) => {}
        Err(error) => {
            let _ = writeln!(output, "{indent}  动作链接解析错误: {}", error);
        }
    }
    match try_find_indexed_path(file, group) {
        Ok(Some(path)) => {
            let _ = writeln!(output, "{indent}  引用: {:?}", path.refs);
        }
        Ok(None) => {}
        Err(error) => {
            let _ = writeln!(output, "{indent}  引用解析错误: {}", error);
        }
    }
    if group.header.kind().is_point_constraint() {
        match try_decode_point_constraint(file, &file.object_groups(), group, None, &None) {
            Ok(constraint) => {
                let summary = match constraint {
                    self::points::RawPointConstraint::Segment(constraint) => format!(
                        "segment start=#{} end=#{} t={:.6}",
                        constraint.start_group_index + 1,
                        constraint.end_group_index + 1,
                        constraint.t
                    ),
                    self::points::RawPointConstraint::ConstructedLine {
                        host_group_index,
                        t,
                        line_like_kind,
                    } => format!(
                        "constructed-line host=#{} kind={:?} t={:.6}",
                        host_group_index + 1,
                        line_like_kind,
                        t
                    ),
                    self::points::RawPointConstraint::PolygonBoundary { edge_index, t, .. } => {
                        format!("polygon edge={} t={:.6}", edge_index, t)
                    }
                    self::points::RawPointConstraint::TranslatedPolygonBoundary {
                        edge_index,
                        t,
                        ..
                    } => {
                        format!("translated-polygon edge={} t={:.6}", edge_index, t)
                    }
                    self::points::RawPointConstraint::Circle(constraint) => format!(
                        "circle center=#{} radius=#{} unit=({:.6}, {:.6})",
                        constraint.center_group_index + 1,
                        constraint.radius_group_index + 1,
                        constraint.unit_x,
                        constraint.unit_y
                    ),
                    self::points::RawPointConstraint::Circular(constraint) => format!(
                        "circle-like host=#{} unit=({:.6}, {:.6})",
                        constraint.circle_group_index + 1,
                        constraint.unit_x,
                        constraint.unit_y
                    ),
                    self::points::RawPointConstraint::CircleArc(constraint) => format!(
                        "circle-arc center=#{} start=#{} end=#{} t={:.6}",
                        constraint.center_group_index + 1,
                        constraint.start_group_index + 1,
                        constraint.end_group_index + 1,
                        constraint.t
                    ),
                    self::points::RawPointConstraint::Arc(constraint) => format!(
                        "arc start=#{} mid=#{} end=#{} t={:.6}",
                        constraint.start_group_index + 1,
                        constraint.mid_group_index + 1,
                        constraint.end_group_index + 1,
                        constraint.t
                    ),
                    self::points::RawPointConstraint::Polyline {
                        function_key,
                        segment_index,
                        t,
                        ..
                    } => format!(
                        "polyline function_key={} segment={} t={:.6}",
                        function_key, segment_index, t
                    ),
                };
                let _ = writeln!(output, "{indent}  点约束: {}", summary);
            }
            Err(error) => {
                let _ = writeln!(output, "{indent}  点约束解析错误: {}", error);
            }
        }
    }
    match try_decode_transform_binding(file, group) {
        Ok(binding) => match binding.kind {
            TransformBindingKind::Rotate {
                angle_degrees,
                ref parameter_name,
            } => {
                let _ = writeln!(
                    output,
                    "{indent}  变换绑定: rotate source=#{} center=#{} angle={:.3} param={:?}",
                    binding.source_group_index + 1,
                    binding.center_group_index + 1,
                    angle_degrees,
                    parameter_name
                );
            }
            TransformBindingKind::Scale { factor } => {
                let _ = writeln!(
                    output,
                    "{indent}  变换绑定: scale source=#{} center=#{} factor={:.3}",
                    binding.source_group_index + 1,
                    binding.center_group_index + 1,
                    factor
                );
            }
        },
        Err(error) => {
            if matches!(
                group.header.kind(),
                GroupKind::Rotation
                    | GroupKind::AngleRotation
                    | GroupKind::Scale
                    | GroupKind::ParameterRotation
            ) {
                let _ = writeln!(output, "{indent}  变换绑定解析错误: {}", error);
            }
        }
    }
    if self::decode::is_parameter_control_group(group) {
        match try_decode_parameter_control_value_for_group(file, &[], group) {
            Ok(value) => {
                let _ = writeln!(output, "{indent}  参数值: {:.6}", value);
            }
            Err(error) => {
                let _ = writeln!(output, "{indent}  参数值解析错误: {}", error);
            }
        }
    }
    match try_decode_payload_anchor_point(file, group) {
        Ok(Some(anchor)) => {
            let _ = writeln!(output, "{indent}  锚点: ({:.3}, {:.3})", anchor.x, anchor.y);
        }
        Ok(None) => {}
        Err(error) => {
            let _ = writeln!(output, "{indent}  锚点解析错误: {}", error);
        }
    }
    match try_decode_bbox_rect_raw(file, group) {
        Ok(Some((x, y, width, height))) => {
            let _ = writeln!(
                output,
                "{indent}  包围框: ({:.3}, {:.3}, {:.3}, {:.3})",
                x, y, width, height
            );
        }
        Ok(None) => {}
        Err(error) => {
            let _ = writeln!(output, "{indent}  包围框解析错误: {}", error);
        }
    }

    let points = group
        .records
        .iter()
        .filter(|record| record.record_type == RECORD_POINT_F64_PAIR)
        .filter_map(|record| decode_point_record(record.payload(&file.data)))
        .take(3)
        .map(|point| format!("({:.3}, {:.3})", point.x, point.y))
        .collect::<Vec<_>>();
    if !points.is_empty() {
        let _ = writeln!(output, "{indent}  点坐标: {}", points.join(", "));
    }

    let strings = collect_group_strings(file, group);
    if !strings.is_empty() {
        let _ = writeln!(output, "{indent}  字符串: {}", strings.join(" | "));
    }

    let _ = writeln!(output, "{indent}  记录:");
    for record in &group.records {
        let _ = writeln!(
            output,
            "{indent}    - 0x{:04x} {} @0x{:x} payload=0x{:x}..0x{:x} len={}{}",
            record.record_type,
            record_name(record.record_type),
            record.offset,
            record.payload_range.start,
            record.payload_range.end,
            record.length,
            format_record_summary(file, record)
                .map(|summary| format!(" {summary}"))
                .unwrap_or_default()
        );
    }
}

fn collect_group_strings(file: &GspFile, group: &ObjectGroup) -> Vec<String> {
    let mut strings = BTreeSet::new();
    for record in &group.records {
        let payload = record.payload(&file.data);
        if let Some(text) = decode_c_string(payload) {
            let text = text.trim();
            if !text.is_empty() {
                strings.insert(format!("{:?}", truncate_text(text, 80)));
            }
        }
        for entry in collect_strings(payload) {
            let text = entry.text.trim();
            if !text.is_empty() {
                strings.insert(format!("{:?}", truncate_text(text, 80)));
            }
        }
    }
    strings.into_iter().take(6).collect()
}

fn format_record_summary(file: &GspFile, record: &Record) -> Option<String> {
    let payload = record.payload(&file.data);
    match record.record_type {
        RECORD_POINT_F64_PAIR => {
            decode_point_record(payload).map(|point| format!("点=({:.3}, {:.3})", point.x, point.y))
        }
        crate::runtime::payload_consts::RECORD_INDEXED_PATH_A
        | crate::runtime::payload_consts::RECORD_INDEXED_PATH_B => {
            decode_indexed_path(record.record_type, payload)
                .map(|path| format!("引用={:?}", path.refs))
                .or_else(|| Some("引用解析失败".to_string()))
        }
        RECORD_FUNCTION_PLOT_DESCRIPTOR => {
            Some(match try_decode_function_plot_descriptor(payload) {
                Ok(descriptor) => format!(
                    "plot=[{:.3}, {:.3}] samples={} mode={:?}",
                    descriptor.x_min, descriptor.x_max, descriptor.sample_count, descriptor.mode
                ),
                Err(error) => format!("plot 解析失败: {error}"),
            })
        }
        _ => {
            let strings = collect_strings(payload)
                .into_iter()
                .map(|entry| truncate_text(entry.text.trim(), 48))
                .filter(|text| !text.is_empty())
                .take(2)
                .collect::<Vec<_>>();
            if !strings.is_empty() {
                return Some(format!("字符串={strings:?}"));
            }
            decode_c_string(payload)
                .map(|text| format!("文本={:?}", truncate_text(text.trim(), 48)))
                .or_else(|| {
                    (payload.len() <= 16 && !payload.is_empty())
                        .then(|| format!("载荷={}", hex_bytes(payload)))
                })
        }
    }
}

fn describe_group(group: &ObjectGroup) -> String {
    let record_types = group
        .records
        .iter()
        .map(|record| format!("0x{:04x}", record.record_type))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "group #{} {:?} @ 0x{:x} [{}]",
        group.ordinal,
        group.header.kind(),
        group.start_offset,
        record_types
    )
}

fn is_supported_group_kind(kind: GroupKind) -> bool {
    matches!(
        kind,
        GroupKind::Point
            | GroupKind::Midpoint
            | GroupKind::Segment
            | GroupKind::Circle
            | GroupKind::CircleCenterRadius
            | GroupKind::LineKind5
            | GroupKind::LineKind6
            | GroupKind::LineKind7
            | GroupKind::Polygon
            | GroupKind::LinearIntersectionPoint
            | GroupKind::CircleInterior
            | GroupKind::IntersectionPoint1
            | GroupKind::IntersectionPoint2
            | GroupKind::CircleCircleIntersectionPoint1
            | GroupKind::CircleCircleIntersectionPoint2
            | GroupKind::PointConstraint
            | GroupKind::Translation
            | GroupKind::CartesianOffsetPoint
            | GroupKind::CoordinateExpressionPoint
            | GroupKind::CoordinateExpressionPointAlt
            | GroupKind::LegacyCoordinateConstructPoint
            | GroupKind::PolarOffsetPoint
            | GroupKind::ExpressionOffsetPoint
            | GroupKind::DerivedSegment24
            | GroupKind::CustomTransformPoint
            | GroupKind::Rotation
            | GroupKind::AngleRotation
            | GroupKind::LegacyAngleRotation
            | GroupKind::ParameterRotation
            | GroupKind::ExpressionRotation
            | GroupKind::Scale
            | GroupKind::RatioScale
            | GroupKind::Reflection
            | GroupKind::PointTrace
            | GroupKind::MeasuredValue
            | GroupKind::GraphObject40
            | GroupKind::RichTextLabel
            | GroupKind::GraphViewHelper
            | GroupKind::FunctionExpr
            | GroupKind::Kind51
            | GroupKind::GraphCalibrationX
            | GroupKind::GraphCalibrationY
            | GroupKind::GraphCalibrationYAlt
            | GroupKind::MeasurementLine
            | GroupKind::AxisLine
            | GroupKind::ActionButton
            | GroupKind::Line
            | GroupKind::Ray
            | GroupKind::CoordinateXValue
            | GroupKind::CoordinateYValue
            | GroupKind::GraphYValue
            | GroupKind::GraphXValue
            | GroupKind::OffsetAnchor
            | GroupKind::FixedCoordinatePoint
            | GroupKind::CoordinatePoint
            | GroupKind::GraphFunctionPoint
            | GroupKind::GraphValuePoint
            | GroupKind::FunctionPlot
            | GroupKind::ParametricFunctionPlot
            | GroupKind::ButtonLabel
            | GroupKind::DerivedSegment75
            | GroupKind::AffineIteration
            | GroupKind::IterationBinding
            | GroupKind::DerivativeFunction
            | GroupKind::ArcOnCircle
            | GroupKind::CenterArc
            | GroupKind::ThreePointArc
            | GroupKind::SectorBoundary
            | GroupKind::CircularSegmentBoundary
            | GroupKind::RegularPolygonIteration
            | GroupKind::LabelIterationSeed
            | GroupKind::IterationExpressionHelper
            | GroupKind::ParameterAnchor
            | GroupKind::ParameterControlledPoint
            | GroupKind::CoordinateTrace
            | GroupKind::CoordinateTraceIntersectionPoint
            | GroupKind::CustomTransformTrace
            | GroupKind::AngleMarker
            | GroupKind::LegacyAngleMarker
            | GroupKind::PathPoint
            | GroupKind::SegmentMarker
            | GroupKind::FunctionDefinition
            | GroupKind::LegacyFunctionPlot
            | GroupKind::BoundaryLengthValue
            | GroupKind::ArcAngleValue
            | GroupKind::AngleValue
            | GroupKind::BoundaryCurveLengthValue
            | GroupKind::RatioValue
            | GroupKind::GraphDistanceValue
            | GroupKind::GraphSlopeValue
            | GroupKind::PolygonAreaValue
            | GroupKind::RadiusValue
            | GroupKind::GraphMeasurementSegment
            | GroupKind::ValueTableRow
            | GroupKind::BoundaryIntersectionPoint
            | GroupKind::PointAlias
            | GroupKind::ThreePointDerivedPoint
            | GroupKind::ProjectedCoordinatePoint
            | GroupKind::PointReferenceAlias
            | GroupKind::PolarAngleValue
            | GroupKind::VertexAngleValue
            | GroupKind::NamedAlias
            | GroupKind::RectImage
            | GroupKind::IterationPointAlias
            | GroupKind::LegacyCoordinateParameterHelper
            | GroupKind::LegacyCoordinatePointHelper
            | GroupKind::LegacyVisibilityHelper
            | GroupKind::LegacyCircularConstraintHelper
            | GroupKind::SmoothCurvePlot
    )
}
