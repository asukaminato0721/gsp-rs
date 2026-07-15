use super::*;
use crate::runtime::extract::decode::decode_bbox_anchor_raw;
use crate::runtime::geometry;
use anyhow::bail;
use std::fmt::Write as _;
use std::path::Path;

mod htm;
mod validation;

use self::htm::{
    HtmPayloadContext, collect_htm_payload_groups, describe_group_as_htm_payload,
    htm_function_plot_mode, read_reference_htm_construction_lines,
};
use self::validation::collect_unsupported_payload_issues;
use crate::runtime::functions::{with_function_expr_cache, with_numeric_helper_cache};

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
    with_numeric_helper_cache(|| render_payload_log_inner(source_path, file))
}

fn render_payload_log_inner(source_path: &Path, file: &GspFile) -> String {
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
                    write_group_detail(&mut output, file, &groups, group, "   ");
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
        with_function_expr_cache(|| {
            for (index, group) in construction_groups.iter().enumerate() {
                let _ = writeln!(
                    output,
                    "{}",
                    describe_group_as_htm_payload(file, &groups, group, index + 1, &htm_context)
                );
            }
        });
    }
    let _ = writeln!(output);
    let _ = writeln!(output, "Payload Objects");
    with_function_expr_cache(|| {
        for (index, group) in groups.iter().enumerate() {
            let _ = writeln!(
                output,
                "{}. {}",
                index + 1,
                describe_group_in_chinese(file, &groups, group)
            );
        }
    });

    output
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
    if summary.starts_with("unsupported payload: action button is missing screen anchor in ") {
        return format!("{target} 暂时无法导出，因为按钮载荷没有提供明确的屏幕位置。");
    }
    if summary.starts_with("unsupported payload: action button is missing label text in ") {
        return format!("{target} 暂时无法导出，因为按钮载荷没有提供明确的按钮文本。");
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
        GroupKind::PerpendicularLine => {
            if let Some((through_index, host_index)) =
                decode::constructed_line_parent_group_indices(file, groups, group)
            {
                format!(
                    "过 {} 且垂直于 {} 的直线",
                    format_ref(through_index + 1),
                    format_ref_with_kind(groups, host_index + 1)
                )
            } else {
                describe_generic_group(group, &refs)
            }
        }
        GroupKind::ParallelLine => {
            if let Some((through_index, host_index)) =
                decode::constructed_line_parent_group_indices(file, groups, group)
            {
                format!(
                    "过 {} 且平行于 {} 的直线",
                    format_ref(through_index + 1),
                    format_ref_with_kind(groups, host_index + 1)
                )
            } else {
                describe_generic_group(group, &refs)
            }
        }
        GroupKind::AngleBisectorRay => {
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
    let has_image_payload = [
        crate::runtime::payload_consts::RECORD_IMAGE_SIZE,
        crate::runtime::payload_consts::RECORD_IMAGE_TRANSFORM,
        crate::runtime::payload_consts::RECORD_IMAGE_RESOURCE,
    ]
    .into_iter()
    .all(|record_type| {
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
        .find(|record| {
            record.record_type == crate::runtime::payload_consts::RECORD_ACTION_BUTTON_PAYLOAD
        })
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
        GroupKind::PerpendicularLine => "垂线",
        GroupKind::ParallelLine => "平行线",
        GroupKind::AngleBisectorRay => "角平分线",
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
        GroupKind::DirectedAngleAnchor => "有向角锚点",
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
        GroupKind::Line
        | GroupKind::PerpendicularLine
        | GroupKind::ParallelLine
        | GroupKind::AngleBisectorRay => "直线",
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

fn write_group_detail(
    output: &mut String,
    file: &GspFile,
    groups: &[ObjectGroup],
    group: &ObjectGroup,
    indent: &str,
) {
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
                    self::points::RawPointConstraint::PolygonBoundaryParameter {
                        parameter,
                        ..
                    } => format!("polygon boundary parameter={parameter:.6}"),
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
                        parameter,
                        ..
                    } => format!(
                        "polyline function_key={} parameter={:.6} segment={} t={:.6}",
                        function_key, parameter, segment_index, t
                    ),
                    self::points::RawPointConstraint::HostedArc {
                        host_group_index,
                        t,
                    } => format!("arc host=#{} t={:.6}", host_group_index + 1, t),
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
        match try_decode_parameter_control_value_for_group(file, groups, group) {
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
        | crate::runtime::payload_consts::RECORD_INDEXED_PATH_B => decode_indexed_path(payload)
            .map(|path| format!("引用={:?}", path.refs))
            .or_else(|| Some("引用解析失败".to_string())),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::payload_consts::{
        RECORD_ACTION_BUTTON_PAYLOAD, RECORD_INDEXED_PATH_A, RECORD_INDEXED_PATH_B,
        RECORD_LABEL_AUX,
    };

    #[test]
    fn malformed_constructed_line_payload_is_reported() {
        let file = GspFile::parse(include_bytes!(
            "../../../tests/fixtures/gsp/static/perpendicular.gsp"
        ))
        .expect("fixture parses");
        let mut groups = file.object_groups();
        let line_group = groups
            .iter_mut()
            .find(|group| group.header.kind() == GroupKind::PerpendicularLine)
            .expect("fixture should contain a perpendicular line");
        let line_ordinal = line_group.ordinal;
        line_group.records.retain(|record| {
            !matches!(
                record.record_type,
                RECORD_INDEXED_PATH_A | RECORD_INDEXED_PATH_B
            )
        });

        let issues = collect_unsupported_payload_issues(&file, &groups);
        assert!(
            issues.iter().any(|issue| {
                issue
                    .summary
                    .contains("unsupported payload: constructed line is missing indexed path")
                    && issue.group_ordinals == [line_ordinal]
            }),
            "expected malformed constructed line issue, got {issues:?}"
        );
    }

    #[test]
    fn malformed_indexed_path_is_reported_for_every_group_kind() {
        let file = GspFile::parse(include_bytes!(
            "../../../tests/fixtures/gsp/static/perpendicular.gsp"
        ))
        .expect("fixture parses");
        let mut groups = file.object_groups();
        let group = groups
            .iter_mut()
            .find(|group| {
                group.records.iter().any(|record| {
                    matches!(
                        record.record_type,
                        RECORD_INDEXED_PATH_A | RECORD_INDEXED_PATH_B
                    )
                })
            })
            .expect("fixture should contain an indexed path");
        let ordinal = group.ordinal;
        let record = group
            .records
            .iter_mut()
            .find(|record| {
                matches!(
                    record.record_type,
                    RECORD_INDEXED_PATH_A | RECORD_INDEXED_PATH_B
                )
            })
            .expect("indexed-path record");
        record.payload_range.end -= 1;
        record.length -= 1;

        let issues = collect_unsupported_payload_issues(&file, &groups);
        assert!(issues.iter().any(|issue| {
            issue.summary.contains("malformed indexed path") && issue.group_ordinals == [ordinal]
        }));
    }

    #[test]
    fn action_button_without_label_text_is_reported() {
        let file = GspFile::parse(include_bytes!(
            "../../../tests/Samples/个人专栏/向忠作品/正弦波与音乐.gsp"
        ))
        .expect("fixture parses");
        let mut groups = file.object_groups();
        let button_group = groups
            .iter_mut()
            .find(|group| {
                group.header.kind() == GroupKind::ActionButton
                    && decode::decode_action_button_text(&file, group).as_deref() == Some("演奏&M")
            })
            .expect("fixture should contain the play-function action button");
        let button_ordinal = button_group.ordinal;
        button_group
            .records
            .retain(|record| record.record_type != RECORD_LABEL_AUX);

        let issues = collect_unsupported_payload_issues(&file, &groups);
        assert!(
            issues.iter().any(|issue| {
                issue
                    .summary
                    .contains("unsupported payload: action button is missing label text")
                    && issue.group_ordinals == [button_ordinal]
            }),
            "expected missing action button label issue, got {issues:?}"
        );
    }

    #[test]
    fn action_button_without_screen_anchor_is_reported() {
        let file = GspFile::parse(include_bytes!(
            "../../../tests/Samples/个人专栏/向忠作品/正弦波与音乐.gsp"
        ))
        .expect("fixture parses");
        let mut groups = file.object_groups();
        let button_group = groups
            .iter_mut()
            .find(|group| {
                group.header.kind() == GroupKind::ActionButton
                    && decode::decode_action_button_text(&file, group).as_deref() == Some("演奏&M")
            })
            .expect("fixture should contain the play-function action button");
        let button_ordinal = button_group.ordinal;
        let action_record = button_group
            .records
            .iter_mut()
            .find(|record| record.record_type == RECORD_ACTION_BUTTON_PAYLOAD)
            .expect("action button should carry a 0x0906 payload");
        action_record.length = 16;
        action_record.payload_range.end = action_record.payload_range.start + 16;

        let issues = collect_unsupported_payload_issues(&file, &groups);
        assert!(
            issues.iter().any(|issue| {
                issue
                    .summary
                    .contains("unsupported payload: action button is missing screen anchor")
                    && issue.group_ordinals == [button_ordinal]
            }),
            "expected missing action button anchor issue, got {issues:?}"
        );
    }
}
