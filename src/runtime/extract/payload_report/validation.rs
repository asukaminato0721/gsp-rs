use super::*;
use crate::runtime::extract::points::resolve_line_like_points_raw;
use anyhow::{Context, Result, bail};

#[derive(Debug, Clone)]
pub(super) struct UnsupportedPayloadIssue {
    pub(super) summary: String,
    pub(super) group_ordinals: Vec<usize>,
}

pub(super) fn collect_unsupported_payload_issues(
    file: &GspFile,
    groups: &[ObjectGroup],
) -> Vec<UnsupportedPayloadIssue> {
    let mut issues = Vec::new();
    let point_map = collect_point_objects(file, groups);
    let anchors = collect_raw_object_anchors(file, groups, &point_map, None);
    for group in groups {
        collect_validation_issue(
            &mut issues,
            &[group.ordinal],
            validate_indexed_path_payload(file, groups, group),
        );
        collect_validation_issue(&mut issues, &[group.ordinal], validate_group_kind(group));
        collect_validation_issue(
            &mut issues,
            &[group.ordinal],
            validate_action_button_payload(file, groups, &anchors, group),
        );
        collect_validation_issue(
            &mut issues,
            &[group.ordinal],
            validate_image_payload(file, group),
        );
        collect_validation_issue(
            &mut issues,
            &[group.ordinal],
            validate_constructed_line_payload(file, groups, group),
        );
        collect_validation_issue(
            &mut issues,
            &function_issue_group_ordinals(file, groups, group),
            validate_function_payload(file, groups, group),
        );
    }
    issues
}

fn validate_indexed_path_payload(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
) -> Result<()> {
    let Some(path) = try_find_indexed_path(file, group).map_err(anyhow::Error::msg)? else {
        return Ok(());
    };
    if let Some(reference) = path
        .refs
        .iter()
        .copied()
        .find(|reference| *reference == 0 || *reference > groups.len())
    {
        bail!(
            "malformed indexed path: group #{} references nonexistent object #{reference}",
            group.ordinal
        );
    }
    Ok(())
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

fn validate_constructed_line_payload(
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
) -> Result<()> {
    let kind = group.header.kind();
    if !matches!(
        kind,
        GroupKind::PerpendicularLine | GroupKind::ParallelLine | GroupKind::AngleBisectorRay
    ) {
        return Ok(());
    }

    let expected_refs = if kind == GroupKind::AngleBisectorRay {
        3
    } else {
        2
    };
    let path = try_find_indexed_path(file, group)
        .map_err(anyhow::Error::msg)?
        .with_context(|| {
            format!(
                "unsupported payload: constructed line is missing indexed path in {}",
                describe_group(group)
            )
        })?;
    if path.refs.len() != expected_refs {
        bail!(
            "unsupported payload: constructed line has {} refs, expected {expected_refs} in {}",
            path.refs.len(),
            describe_group(group)
        );
    }

    let point_map = collect_point_objects(file, groups);
    let anchors = collect_raw_object_anchors(file, groups, &point_map, None);
    match kind {
        GroupKind::PerpendicularLine | GroupKind::ParallelLine => {
            let through_index = path.refs[0].checked_sub(1).with_context(|| {
                format!(
                    "unsupported payload: constructed line through-point ordinal underflow in {}",
                    describe_group(group)
                )
            })?;
            let host_index = path.refs[1].checked_sub(1).with_context(|| {
                format!(
                    "unsupported payload: constructed line host ordinal underflow in {}",
                    describe_group(group)
                )
            })?;
            anchor_at(&anchors, through_index).with_context(|| {
                format!(
                    "unsupported payload: constructed line references missing through point #{} in {}",
                    path.refs[0],
                    describe_group(group)
                )
            })?;
            let host_group = groups.get(host_index).with_context(|| {
                format!(
                    "unsupported payload: constructed line references missing host #{} in {}",
                    path.refs[1],
                    describe_group(group)
                )
            })?;
            resolve_line_like_points_raw(file, groups, &anchors, host_group).with_context(|| {
                format!(
                    "unsupported payload: constructed line host #{} is not a decodable line in {}",
                    path.refs[1],
                    describe_group(group)
                )
            })?;
        }
        GroupKind::AngleBisectorRay => {
            let start = anchor_at(&anchors, path.refs[0].saturating_sub(1)).with_context(|| {
                format!(
                    "unsupported payload: angle bisector references missing start point #{} in {}",
                    path.refs[0],
                    describe_group(group)
                )
            })?;
            let vertex = anchor_at(&anchors, path.refs[1].saturating_sub(1)).with_context(|| {
                format!(
                    "unsupported payload: angle bisector references missing vertex point #{} in {}",
                    path.refs[1],
                    describe_group(group)
                )
            })?;
            let end = anchor_at(&anchors, path.refs[2].saturating_sub(1)).with_context(|| {
                format!(
                    "unsupported payload: angle bisector references missing end point #{} in {}",
                    path.refs[2],
                    describe_group(group)
                )
            })?;
            if !line_points_are_distinct(&start, &vertex)
                || !line_points_are_distinct(&end, &vertex)
            {
                bail!(
                    "unsupported payload: angle bisector has degenerate input points in {}",
                    describe_group(group)
                );
            }
        }
        _ => {}
    }

    Ok(())
}

fn anchor_at(anchors: &[Option<PointRecord>], index: usize) -> Option<PointRecord> {
    anchors.get(index).cloned().flatten()
}

fn line_points_are_distinct(left: &PointRecord, right: &PointRecord) -> bool {
    (left.x - right.x).hypot(left.y - right.y) > 1e-9
}

fn validate_action_button_payload(
    file: &GspFile,
    groups: &[ObjectGroup],
    anchors: &[Option<PointRecord>],
    group: &ObjectGroup,
) -> Result<()> {
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
    if !matches!(
        action_kind,
        (2 | 8, 0) | (4, 0 | 1) | (7, _) | (3, 0..=3) | (0 | 1, 0..=7)
    ) {
        bail!(
            "unsupported payload: action button uses unsupported action kind ({}, {}) in {}",
            action_kind.0,
            action_kind.1,
            describe_group(group)
        );
    }

    if decode::decode_action_button_anchor(file, groups, group, anchors).is_none() {
        bail!(
            "unsupported payload: action button is missing screen anchor in {}",
            describe_group(group)
        );
    }
    if decode::decode_action_button_text(file, group).is_none() {
        bail!(
            "unsupported payload: action button is missing label text in {}",
            describe_group(group)
        );
    }

    Ok(())
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

fn is_supported_group_kind(kind: GroupKind) -> bool {
    matches!(
        kind,
        GroupKind::Point
            | GroupKind::Midpoint
            | GroupKind::Segment
            | GroupKind::Circle
            | GroupKind::CircleCenterRadius
            | GroupKind::PerpendicularLine
            | GroupKind::ParallelLine
            | GroupKind::AngleBisectorRay
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
