use super::{build_scene_checked, render_payload_log};
use crate::format::GspFile;
use crate::runtime::scene::{
    LabelIterationFamily, LineBinding, LineConstraint, LineIterationFamily, PointIterationFamily,
    PolygonIterationFamily, Scene, ScenePointBinding, ScenePointConstraint, TextLabelBinding,
};
use insta::assert_snapshot;
use std::fs;
use std::path::Path;

fn fixture_scene(data: &[u8]) -> Scene {
    let file = GspFile::parse(data).expect("fixture parses");
    build_scene_checked(&file).expect("scene builds")
}

fn fixture_log(data: &[u8], source_path: &str) -> String {
    let file = GspFile::parse(data).expect("fixture parses");
    render_payload_log(Path::new(source_path), &file)
}

fn fixture_bytes(path: &str) -> Option<Vec<u8>> {
    fs::read(path).ok()
}
#[test]
fn preserves_function_iteration_coordinate_point_in_liyougui_fixture() {
    let Some(data) = fixture_bytes("tests/Samples/个人专栏/李有贵作品/函数图象迭代(liyougui).gsp")
    else {
        return;
    };
    let scene = fixture_scene(&data);

    assert!(
        scene.points.iter().any(|point| {
            matches!(
                point.binding,
                Some(ScenePointBinding::CoordinateSource2d { source_index, .. }) if source_index == 0
            )
        }),
        "expected the payload coordinate point to stay exported as a live 2d graph binding"
    );
    assert!(
        scene
            .lines
            .iter()
            .any(|line| matches!(line.binding, Some(LineBinding::PointTrace { .. }))),
        "expected the payload point trace to stay exported"
    );
    assert!(
        scene
            .parameters
            .iter()
            .any(|parameter| parameter.name == "k" && (parameter.value + 1.5).abs() < 1e-6),
        "expected k to open at -1.5"
    );
    assert!(
        scene
            .parameters
            .iter()
            .any(|parameter| parameter.name == "m" && (parameter.value + 4.0).abs() < 1e-6),
        "expected m to open at -4"
    );
    assert!(
        scene
            .parameters
            .iter()
            .any(|parameter| parameter.name == "n" && (parameter.value - 9.7).abs() < 1e-6),
        "expected n to open at the saved payload value 9.7"
    );
    assert!(
        scene
            .line_iterations
            .iter()
            .any(|family| matches!(family, LineIterationFamily::ParameterizedPointTrace { .. })),
        "expected the payload iter on the trace to stay exported as a live line-iteration family"
    );
    assert!(
        scene
            .labels
            .iter()
            .any(|label| label.text.contains("2*(C + m) =")),
        "expected the x-coordinate expression to stay decoded from payload as 2*(C + m)"
    );
}

#[test]
fn preserves_binary_tree_multimap_iteration() {
    let Some(data) = fixture_bytes("../Samples/个人专栏/方小庆作品/二叉树(inRm).gsp")
    else {
        return;
    };
    let scene = fixture_scene(&data);
    assert_eq!(
        scene.line_iterations.len(),
        1,
        "expected one recursive line family for the binary tree payload"
    );
    let LineIterationFamily::Branching {
        target_segments,
        parameter_name,
        depth,
        ..
    } = &scene.line_iterations[0]
    else {
        panic!("expected binary tree iteration to export branching segment handles");
    };
    assert_eq!(
        target_segments.len(),
        2,
        "expected the payload to produce two child segment maps"
    );
    assert_eq!(parameter_name.as_deref(), Some("n"));
    assert_eq!(*depth, 7, "expected depth to stay driven by payload n");
    assert_eq!(
        scene.lines.len(),
        255,
        "expected one seed segment plus 2^1..2^7 recursive branches"
    );
    assert!(
        scene
            .line_iterations
            .iter()
            .all(|family| !matches!(family, LineIterationFamily::Affine { .. })),
        "expected the binary tree payload to avoid the carried affine fallback"
    );
    assert!(
        scene.points.iter().take(2).all(|point| point.draggable),
        "expected the free endpoints to remain interactive"
    );
    assert!(
        scene.points.iter().any(|point| {
            point.visible
                && matches!(
                    point.binding,
                    Some(ScenePointBinding::Parameter { ref name }) if name == "n"
                )
        }),
        "expected the legacy n parameter control point to stay visible in the exported scene"
    );
}

#[test]
fn renders_unsupported_payload_log_in_natural_chinese() {
    let Some(data) = fixture_bytes("tests/fixtures/14.gsp") else {
        return;
    };
    let log = fixture_log(&data, "tests/fixtures/14.gsp");

    assert!(log.contains("载荷说明"));
    assert!(log.contains("问题列表"));
    assert!(log.contains("对象 #19 暂时无法导出，因为对象类型 86 还没有实现。"));
    assert!(log.contains("相关对象："));
    assert!(log.contains("#19 = 未知对象，类型是 86，按载荷顺序引用 #18、#16、#6。"));
    assert!(log.contains("#18 = 将 点 #10 按向量 #1 -> #10 平移得到的对象"));
    assert!(log.contains("#15 = 过 #10 且垂直于 线段 #14 的直线。"));
    assert!(log.contains("#1 = 自由点，名称“O”。"));
    assert!(log.contains("原始载荷："));
    assert!(log.contains("构造步骤"));
    assert!(log.contains("1. #1 = 自由点，名称“O”。"));
}

#[test]
fn renders_payload_log_for_supported_fixture_too() {
    let log = fixture_log(
        include_bytes!("../../../tests/fixtures/gsp/static/point.gsp"),
        "tests/fixtures/gsp/static/point.gsp",
    );

    assert!(log.contains("问题数量: 0"));
    assert!(log.contains("未发现不支持的载荷。"));
    assert!(log.contains("构造步骤"));
    assert!(log.contains("1. #1 = 自由点。"));
}

#[test]
fn snapshots_payload_log_for_point_fixture() {
    let log = fixture_log(
        include_bytes!("../../../tests/fixtures/gsp/static/point.gsp"),
        "tests/fixtures/gsp/static/point.gsp",
    );

    assert_snapshot!("point_fixture_payload_log", log);
}

#[test]
fn builds_function_plot_for_f_gsp() {
    let scene = fixture_scene(include_bytes!("../../../../f.gsp"));

    assert!(scene.graph_mode);
    assert!(
        scene.lines.iter().any(|line| {
            let min_x = line
                .points
                .iter()
                .map(|point| point.x)
                .fold(f64::INFINITY, f64::min);
            let max_x = line
                .points
                .iter()
                .map(|point| point.x)
                .fold(f64::NEG_INFINITY, f64::max);
            min_x <= 0.1 && max_x > 30.0
        }),
        "expected a non-degenerate function plot spanning the graph domain"
    );
    assert!(scene.bounds.min_x < -9.0);
    assert!(scene.bounds.max_y > 14.0);
    assert_eq!(scene.labels.len(), 1);
    assert_eq!(
        scene.labels[0]
            .text
            .strip_prefix("q(x) = ")
            .or_else(|| scene.labels[0].text.strip_prefix("f(x) = ")),
        Some("|x| + √x + ln(x) + log(x) + sgn(x) + round(x) + trunc(x)")
    );
}

#[test]
fn preserves_draw_function_fixture_interactivity() {
    let scene = fixture_scene(include_bytes!(
        "../../../tests/fixtures/未实现的系统功能/绘图函数.gsp"
    ));

    assert!(scene.graph_mode, "expected graph scene");
    assert!(
        scene.images.len() == 1,
        "expected one embedded graph image, got {}",
        scene.images.len()
    );
    assert!(
        scene.images[0].screen_space,
        "expected payload-positioned screen image"
    );
    assert!(
        scene.images[0].src.starts_with("data:image/png;base64,"),
        "expected embedded png data url"
    );
    assert!(
        scene.images[0].top_left.x < scene.images[0].bottom_right.x
            && scene.images[0].top_left.y < scene.images[0].bottom_right.y,
        "expected visible screen-space image bounds"
    );
    assert!(
        scene.lines.len() >= 3,
        "expected graph helpers to remain visible with the embedded image"
    );
}

#[test]
fn preserves_insert_image_fixture() {
    let scene = fixture_scene(include_bytes!(
        "../../../tests/fixtures/未实现的系统功能/插入图片.gsp"
    ));

    assert!(!scene.graph_mode, "expected non-graph image fixture");
    assert_eq!(scene.images.len(), 1, "expected one embedded image");
    assert!(
        scene.images[0].screen_space,
        "expected screen-space image placement"
    );
    assert!(
        scene.images[0].src.starts_with("data:image/png;base64,"),
        "expected embedded png data url"
    );
    assert_eq!(scene.images[0].top_left.x, 118.0);
    assert_eq!(scene.images[0].top_left.y, 112.0);
    assert_eq!(scene.images[0].bottom_right.x, 373.0);
    assert_eq!(scene.images[0].bottom_right.y, 270.0);
    assert!(
        scene.lines.is_empty(),
        "expected image-only fixture without line artifacts"
    );
}

#[test]
fn preserves_points_defined_by_path_value_fixture() {
    let scene = fixture_scene(include_bytes!(
        "../../../tests/fixtures/未实现的系统功能/给定的数值在路径上绘制点.gsp"
    ));

    assert_eq!(
        scene.points.len(),
        6,
        "expected A/B/D/E plus constrained C/F"
    );
    assert!(
        scene
            .points
            .iter()
            .any(|point| matches!(point.constraint, ScenePointConstraint::OnCircle { .. })),
        "expected one point constrained by the circle path payload"
    );
    assert!(
        scene
            .points
            .iter()
            .any(|point| matches!(point.constraint, ScenePointConstraint::OnSegment { .. })),
        "expected one point constrained by the segment path payload"
    );
    let labels = scene
        .labels
        .iter()
        .map(|label| label.text.as_str())
        .collect::<Vec<_>>();
    assert!(
        labels.contains(&"C"),
        "expected path-defined point label C, got {labels:?}"
    );
    assert!(
        labels.contains(&"F"),
        "expected path-defined point label F, got {labels:?}"
    );
}

#[test]
fn preserves_multiline_text_labels() {
    let scene = fixture_scene(include_bytes!("../../../tests/fixtures/gsp/多行文本.gsp"));

    assert_eq!(scene.labels.len(), 1);
    assert_eq!(
        scene.labels[0].text,
        "线段中垂线\n垂线\n平行线\n直角三角形\n点的轨迹\n圆上的弧\n过三点的弧"
    );
}

#[test]
fn preserves_hot_text_actions_in_rich_text_gsp() {
    let scene = fixture_scene(include_bytes!("../../../tests/fixtures/gsp/热文本.gsp"));

    let rich_label = scene
        .labels
        .iter()
        .find(|label| label.text.contains("BAC"))
        .expect("expected hot text label");
    assert_eq!(rich_label.text, "在ACB中，CA=AB，BAC=CBA");
    assert_eq!(
        rich_label
            .hotspots
            .iter()
            .map(|hotspot| hotspot.text.as_str())
            .collect::<Vec<_>>(),
        vec!["ACB", "CA", "AB", "BAC", "CBA"]
    );
    assert!(matches!(
        rich_label.hotspots[0].action,
        crate::runtime::scene::TextLabelHotspotAction::Polygon { .. }
    ));
    assert!(matches!(
        rich_label.hotspots[1].action,
        crate::runtime::scene::TextLabelHotspotAction::Segment { .. }
    ));
    assert!(matches!(
        rich_label.hotspots[2].action,
        crate::runtime::scene::TextLabelHotspotAction::Segment { .. }
    ));
    assert!(matches!(
        rich_label.hotspots[3].action,
        crate::runtime::scene::TextLabelHotspotAction::AngleMarker { .. }
    ));
    assert!(matches!(
        rich_label.hotspots[4].action,
        crate::runtime::scene::TextLabelHotspotAction::AngleMarker { .. }
    ));
    assert_eq!(scene.buttons.len(), 1, "expected linked action button");
    assert_eq!(scene.buttons[0].text, "隐藏三角形 ACB");
}

#[test]
fn preserves_translated_points_in_point_translation_gsp() {
    let scene = fixture_scene(include_bytes!(
        "../../../tests/fixtures/gsp/static/point_translation.gsp"
    ));

    assert_eq!(
        scene.points.len(),
        2,
        "expected base point and translated point"
    );
    let origin = scene
        .points
        .iter()
        .find(|point| matches!(point.constraint, ScenePointConstraint::Free))
        .expect("expected free origin point");
    let translated = scene
        .points
        .iter()
        .find(|point| matches!(point.constraint, ScenePointConstraint::Offset { .. }))
        .expect("expected translated offset point");

    match translated.constraint {
        ScenePointConstraint::Offset {
            origin_index,
            dx,
            dy,
        } => {
            assert_eq!(origin_index, 0);
            assert!(
                dx.abs() < 0.001,
                "expected 90-degree translation to keep x constant, got dx={dx}"
            );
            assert!(
                dy < 0.0,
                "expected upward translation in raw coordinates, got dy={dy}"
            );
            assert!(
                (translated.position.x - (origin.position.x + dx)).abs() < 0.001
                    && (translated.position.y - (origin.position.y + dy)).abs() < 0.001,
                "expected translated point to preserve offset from origin: origin={:?}, translated={:?}",
                origin.position,
                translated.position
            );
        }
        _ => panic!("expected offset constraint"),
    }
}

#[test]
fn preserves_circular_segment_boundary_point_interactivity() {
    let scene = fixture_scene(include_bytes!(
        "../../../tests/fixtures/未实现的系统功能/弓形周界动点.gsp"
    ));

    assert_eq!(
        scene.polygons.len(),
        1,
        "expected one filled circular segment"
    );
    assert!(matches!(
        scene.polygons[0].binding,
        Some(crate::runtime::scene::ShapeBinding::ArcBoundaryPolygon { .. })
    ));

    let boundary_point = scene
        .points
        .iter()
        .find(|point| matches!(point.constraint, ScenePointConstraint::OnPolyline { .. }))
        .expect("expected boundary point constrained to rendered perimeter");
    match &boundary_point.constraint {
        ScenePointConstraint::OnPolyline {
            points,
            segment_index,
            t,
            ..
        } => {
            assert!(points.len() >= 4, "expected sampled boundary polyline");
            assert!(
                *segment_index < points.len() - 1,
                "segment index should reference a valid boundary segment"
            );
            assert!(
                (0.0..=1.0).contains(t),
                "polyline parameter should stay normalized"
            );
        }
        _ => unreachable!(),
    }
    assert!(
        scene.lines.iter().any(|line| line.points.len() >= 4),
        "expected perimeter shape to be rendered as an interactive polyline"
    );
    assert!(
        scene
            .lines
            .iter()
            .any(|line| matches!(line.binding, Some(LineBinding::ArcBoundary { .. }))),
        "expected boundary line to stay payload-bound for reactive updates"
    );
}

#[test]
fn preserves_custom_transform_point_interactivity() {
    let scene = fixture_scene(include_bytes!(
        "../../../tests/fixtures/未实现的系统功能/自定义变换.gsp"
    ));

    assert_eq!(scene.points.len(), 4, "expected custom transform point Q");
    assert!(
        scene.lines.iter().any(|line| line.points.len() > 100),
        "expected sampled custom transform trace"
    );
    let trace = scene
        .lines
        .iter()
        .find(|line| matches!(line.binding, Some(LineBinding::CustomTransformTrace { .. })))
        .expect("expected payload-bound custom transform trace");
    assert!(
        scene.labels.iter().any(|label| label.text == "Q"),
        "expected custom transform point label"
    );
    assert!(
        scene
            .labels
            .iter()
            .any(|label| label.text.contains("1厘米") || label.text.contains("100°")),
        "expected payload-derived custom transform expression labels"
    );
    assert!(matches!(
        &scene.points[3].binding,
        Some(ScenePointBinding::CustomTransform {
            source_index,
            origin_index,
            axis_end_index,
            ..
        }) if *source_index == 2 && *origin_index == 0 && *axis_end_index == 1
    ));
    let (source_t, origin_index) = match scene.points[2].constraint {
        ScenePointConstraint::OnSegment { t, start_index, .. } => (t, start_index),
        ref constraint => {
            panic!("expected source point to stay constrained on segment, got {constraint:?}")
        }
    };
    let origin = &scene.points[origin_index];
    assert!(
        scene.points[3].position.x > origin.position.x
            && scene.points[3].position.y < origin.position.y
            && source_t > 0.0,
        "expected payload-defined custom transform to place Q above/right of O using P's normalized parameter"
    );
    let trace_end = trace.points.last().expect("trace endpoint");
    assert!(
        (trace_end.x - scene.points[3].position.x).abs() < 1e-6
            && (trace_end.y - scene.points[3].position.y).abs() < 1e-6,
        "expected custom transform trace to stop at Q"
    );
}

#[test]
fn preserves_polygon_in_poly_gsp() {
    let scene = fixture_scene(include_bytes!(
        "../../../tests/fixtures/gsp/static/poly.gsp"
    ));

    assert_eq!(scene.polygons.len(), 1, "expected a single polygon");
    assert_eq!(
        scene.polygons[0].points.len(),
        4,
        "expected polygon to keep its four vertices"
    );
    assert_eq!(
        scene.polygons[0].color,
        [255, 128, 0, 127],
        "expected polygon fill opacity from source style metadata"
    );
    assert_eq!(scene.points.len(), 4, "expected four visible points");
    assert!(
        scene
            .points
            .iter()
            .all(|point| matches!(point.constraint, ScenePointConstraint::Free)),
        "expected polygon vertices to stay free points"
    );
}

#[test]
fn preserves_polygon_boundary_point_in_poly_point_gsp() {
    let scene = fixture_scene(include_bytes!(
        "../../../tests/fixtures/gsp/static/poly_point.gsp"
    ));

    assert_eq!(scene.polygons.len(), 1, "expected a single polygon");
    assert_eq!(
        scene.points.len(),
        5,
        "expected four vertices and one constrained point"
    );
    assert_eq!(
        scene
            .points
            .iter()
            .filter(|point| matches!(point.constraint, ScenePointConstraint::Free))
            .count(),
        4,
        "expected four free polygon vertices"
    );
    assert!(scene.points.iter().any(|point| {
        matches!(
            point.constraint,
            ScenePointConstraint::OnPolygonBoundary {
                ref vertex_indices,
                edge_index: 2,
                t,
            } if vertex_indices == &vec![0, 1, 2, 3] && (t - 0.4450450665338869).abs() < 0.001
        )
    }));
    assert!(scene.points.iter().any(|point| {
        (point.position.x - 487.23).abs() < 0.05 && (point.position.y - 262.28).abs() < 0.05
    }));
}

#[test]
fn preserves_polygon_labels_in_poly_point_with_val_gsp() {
    let scene = fixture_scene(include_bytes!(
        "../../../tests/fixtures/gsp/static/poly_point_with_val.gsp"
    ));

    assert_eq!(scene.polygons.len(), 1, "expected a single polygon");
    assert_eq!(
        scene.points.len(),
        5,
        "expected four vertices and one constrained point"
    );
    let texts = scene
        .labels
        .iter()
        .map(|label| label.text.as_str())
        .collect::<Vec<_>>();
    assert!(
        texts.contains(&"A"),
        "expected point label A, got {texts:?}"
    );
    assert!(
        texts.contains(&"B"),
        "expected point label B, got {texts:?}"
    );
    assert!(
        texts.contains(&"C"),
        "expected point label C, got {texts:?}"
    );
    assert!(
        texts.contains(&"D"),
        "expected point label D, got {texts:?}"
    );
    assert!(
        texts.contains(&"E"),
        "expected constrained point label E, got {texts:?}"
    );
    assert!(
        texts.contains(&"E在ABCD上的t值 = 0.58"),
        "expected polygon parameter label, got {texts:?}"
    );
}

#[test]
fn preserves_segment_parameter_label_in_segment_point_value_gsp() {
    let scene = fixture_scene(include_bytes!(
        "../../../tests/fixtures/gsp/static/segment_point_value.gsp"
    ));

    assert_eq!(scene.lines.len(), 1, "expected one segment");
    assert_eq!(
        scene.points.len(),
        3,
        "expected two endpoints and one constrained point"
    );
    let texts = scene
        .labels
        .iter()
        .map(|label| label.text.as_str())
        .collect::<Vec<_>>();
    assert!(
        texts.contains(&"A"),
        "expected point label A, got {texts:?}"
    );
    assert!(
        texts.contains(&"B"),
        "expected point label B, got {texts:?}"
    );
    assert!(
        texts.contains(&"C"),
        "expected point label C, got {texts:?}"
    );
    assert!(
        texts.contains(&"C在AB上的t值 = 0.51"),
        "expected segment parameter label, got {texts:?}"
    );
}

#[test]
fn preserves_line_gsp() {
    let scene = fixture_scene(include_bytes!(
        "../../../tests/fixtures/gsp/static/line.gsp"
    ));

    assert_eq!(scene.lines.len(), 1, "expected one line");
    assert_eq!(scene.points.len(), 2, "expected two defining points");
    let line = &scene.lines[0];
    assert!(matches!(
        line.binding,
        Some(LineBinding::Line { .. } | LineBinding::Segment { .. })
    ));
    let min_x = line
        .points
        .iter()
        .map(|point| point.x)
        .fold(f64::INFINITY, f64::min);
    let max_x = line
        .points
        .iter()
        .map(|point| point.x)
        .fold(f64::NEG_INFINITY, f64::max);
    assert!((min_x - scene.bounds.min_x).abs() < 1e-3);
    assert!((max_x - scene.bounds.max_x).abs() < 1e-3);
}

#[test]
fn preserves_ray_gsp() {
    let scene = fixture_scene(include_bytes!("../../../tests/fixtures/gsp/static/ray.gsp"));

    assert_eq!(scene.lines.len(), 1, "expected one ray");
    assert_eq!(scene.points.len(), 2, "expected two defining points");
    let line = &scene.lines[0];
    assert!(matches!(line.binding, Some(LineBinding::Ray { .. })));
    let max_x = line
        .points
        .iter()
        .map(|point| point.x)
        .fold(f64::NEG_INFINITY, f64::max);
    assert!((max_x - scene.bounds.max_x).abs() < 1e-3);
    assert!(
        line.points
            .iter()
            .any(|point| (point.x - scene.points[0].position.x).abs() < 1e-3),
        "expected ray to include its start point"
    );
}

#[test]
fn preserves_perpendicular_gsp() {
    let scene = fixture_scene(include_bytes!(
        "../../../tests/fixtures/gsp/static/perpendicular.gsp"
    ));

    assert_eq!(
        scene.lines.len(),
        2,
        "expected base segment and perpendicular line"
    );
    assert_eq!(scene.points.len(), 2, "expected two defining points");

    let base = scene
        .lines
        .iter()
        .find(|line| matches!(line.binding, Some(LineBinding::Segment { .. })))
        .expect("expected source segment");
    let perpendicular = scene
        .lines
        .iter()
        .find(|line| matches!(line.binding, Some(LineBinding::PerpendicularLine { .. })))
        .expect("expected synthesized perpendicular line");

    let base_dx = base.points[1].x - base.points[0].x;
    let base_dy = base.points[1].y - base.points[0].y;
    let perp_dx = perpendicular.points[1].x - perpendicular.points[0].x;
    let perp_dy = perpendicular.points[1].y - perpendicular.points[0].y;
    let base_len = (base_dx * base_dx + base_dy * base_dy).sqrt();
    let perp_len = (perp_dx * perp_dx + perp_dy * perp_dy).sqrt();
    let dot = base_dx * perp_dx + base_dy * perp_dy;

    assert!(
        (dot / (base_len * perp_len)).abs() < 1e-6,
        "expected perpendicular directions, got base=({base_dx},{base_dy}) and line=({perp_dx},{perp_dy})"
    );

    let through = &scene.points[1].position;
    let distance = ((through.x - perpendicular.points[0].x) * perp_dy
        - (through.y - perpendicular.points[0].y) * perp_dx)
        .abs()
        / perp_len;
    assert!(
        distance < 1e-6,
        "expected perpendicular line to pass through point B, distance={distance}"
    );
}

#[test]
fn preserves_parallel_gsp() {
    let scene = fixture_scene(include_bytes!("../../../tests/fixtures/gsp/parallel.gsp"));

    assert_eq!(
        scene.lines.len(),
        2,
        "expected base segment and parallel line"
    );
    assert_eq!(
        scene.points.len(),
        3,
        "expected two base points plus through point"
    );

    let base = scene
        .lines
        .iter()
        .find(|line| matches!(line.binding, Some(LineBinding::Segment { .. })))
        .expect("expected source segment");
    let parallel = scene
        .lines
        .iter()
        .find(|line| matches!(line.binding, Some(LineBinding::ParallelLine { .. })))
        .expect("expected synthesized parallel line");

    let base_dx = base.points[1].x - base.points[0].x;
    let base_dy = base.points[1].y - base.points[0].y;
    let parallel_dx = parallel.points[1].x - parallel.points[0].x;
    let parallel_dy = parallel.points[1].y - parallel.points[0].y;
    let base_len = (base_dx * base_dx + base_dy * base_dy).sqrt();
    let parallel_len = (parallel_dx * parallel_dx + parallel_dy * parallel_dy).sqrt();
    let cross = base_dx * parallel_dy - base_dy * parallel_dx;

    assert!(
        (cross / (base_len * parallel_len)).abs() < 1e-6,
        "expected parallel directions, got base=({base_dx},{base_dy}) and line=({parallel_dx},{parallel_dy})"
    );

    let through = &scene.points[2].position;
    let distance = ((through.x - parallel.points[0].x) * parallel_dy
        - (through.y - parallel.points[0].y) * parallel_dx)
        .abs()
        / parallel_len;
    assert!(
        distance < 1e-6,
        "expected parallel line to pass through point C, distance={distance}"
    );
}

#[test]
fn preserves_nested_perpendicular_parallel_bindings_in_pert_vert_gsp() {
    let scene = fixture_scene(include_bytes!("../../../tests/fixtures/gsp/pert_vert.gsp"));

    assert_eq!(
        scene.lines.len(),
        4,
        "expected base line, bisector, and marker strokes"
    );
    assert_eq!(
        scene.points.len(),
        4,
        "expected free anchor point plus midpoint construction"
    );

    let base_index = scene
        .lines
        .iter()
        .position(|line| matches!(line.binding, Some(LineBinding::Segment { .. })))
        .expect("expected source segment");
    let main_perpendicular_index = scene
        .lines
        .iter()
        .position(|line| {
            matches!(
                line.binding,
                Some(LineBinding::PerpendicularLine {
                    through_index: 3,
                    line_index: Some(0),
                    ..
                })
            )
        })
        .expect("expected midpoint perpendicular line bound to the source segment");
    assert_eq!(main_perpendicular_index, 1);
    assert_eq!(base_index, 0);

    assert!(scene.lines.iter().any(|line| {
        matches!(
            line.binding,
            Some(LineBinding::PerpendicularLine {
                through_index: 1,
                line_index: Some(1),
                ..
            })
        )
    }));
    assert!(scene.lines.iter().any(|line| {
        matches!(
            line.binding,
            Some(LineBinding::ParallelLine {
                through_index: 1,
                line_index: Some(1),
                ..
            })
        )
    }));
}

#[test]
fn preserves_bisector_gsp() {
    let scene = fixture_scene(include_bytes!(
        "../../../tests/fixtures/gsp/static/bisector.gsp"
    ));

    assert_eq!(scene.lines.len(), 1, "expected one angle bisector");
    assert_eq!(scene.points.len(), 3, "expected three defining points");

    let bisector = scene
        .lines
        .iter()
        .find(|line| matches!(line.binding, Some(LineBinding::AngleBisectorRay { .. })))
        .expect("expected synthesized angle bisector ray");

    let start = &scene.points[0].position;
    let vertex = &scene.points[1].position;
    let end = &scene.points[2].position;
    assert!(
        (bisector.points[0].x - vertex.x).abs() < 1e-6
            && (bisector.points[0].y - vertex.y).abs() < 1e-6,
        "expected bisector ray to start at the vertex"
    );
    let bisector_dx = bisector.points[1].x - bisector.points[0].x;
    let bisector_dy = bisector.points[1].y - bisector.points[0].y;
    let bisector_len = (bisector_dx * bisector_dx + bisector_dy * bisector_dy).sqrt();
    let start_dx = start.x - vertex.x;
    let start_dy = start.y - vertex.y;
    let start_len = (start_dx * start_dx + start_dy * start_dy).sqrt();
    let end_dx = end.x - vertex.x;
    let end_dy = end.y - vertex.y;
    let end_len = (end_dx * end_dx + end_dy * end_dy).sqrt();

    let distance = ((vertex.x - bisector.points[0].x) * bisector_dy
        - (vertex.y - bisector.points[0].y) * bisector_dx)
        .abs()
        / bisector_len;
    assert!(
        distance < 1e-6,
        "expected bisector ray to pass through the vertex, distance={distance}"
    );

    let start_alignment =
        (start_dx * bisector_dx + start_dy * bisector_dy) / (start_len * bisector_len);
    let end_alignment = (end_dx * bisector_dx + end_dy * bisector_dy) / (end_len * bisector_len);
    assert!(
        (start_alignment - end_alignment).abs() < 1e-6,
        "expected equal angles to both rays, got start={start_alignment} end={end_alignment}"
    );
}

#[test]
fn preserves_three_point_arc_gsp() {
    let scene = fixture_scene(include_bytes!(
        "../../../tests/fixtures/gsp/static/three_point_arc.gsp"
    ));

    assert_eq!(scene.points.len(), 3, "expected three defining points");
    assert_eq!(scene.arcs.len(), 1, "expected one three-point arc");
    assert!(
        scene.lines.is_empty(),
        "expected arc fixture not to fall back to a line"
    );

    let arc = &scene.arcs[0];
    assert_eq!(arc.color, [0, 128, 0, 255]);
    assert!(
        arc.points
            .iter()
            .zip(scene.points.iter())
            .all(|(arc_point, scene_point)| {
                (arc_point.x - scene_point.position.x).abs() < 1e-6
                    && (arc_point.y - scene_point.position.y).abs() < 1e-6
            }),
        "expected arc to preserve the three source points"
    );
}

#[test]
fn preserves_arc_on_circle_gsp() {
    let scene = fixture_scene(include_bytes!(
        "../../../tests/fixtures/gsp/static/arc_on_circle.gsp"
    ));

    assert_eq!(scene.circles.len(), 1, "expected one supporting circle");
    assert!(
        scene.circles[0].dashed,
        "expected supporting circle to render dashed"
    );
    assert_eq!(scene.arcs.len(), 1, "expected one arc on the source circle");
    assert_eq!(
        scene.points.len(),
        4,
        "expected center, radius, and two arc endpoints"
    );

    let arc = &scene.arcs[0];
    let start = &scene.points[2].position;
    let end = &scene.points[3].position;
    let midpoint = &arc.points[1];
    let center = &scene.circles[0].center;
    let radius = ((scene.circles[0].radius_point.x - center.x).powi(2)
        + (scene.circles[0].radius_point.y - center.y).powi(2))
    .sqrt();
    let start_angle = (-(start.y - center.y)).atan2(start.x - center.x);
    let end_angle = (-(end.y - center.y)).atan2(end.x - center.x);
    let midpoint_angle = (-(midpoint.y - center.y)).atan2(midpoint.x - center.x);
    let ccw_span = (end_angle - start_angle).rem_euclid(std::f64::consts::TAU);
    let ccw_mid = (midpoint_angle - start_angle).rem_euclid(std::f64::consts::TAU);

    assert!((arc.points[0].x - start.x).abs() < 1e-6 && (arc.points[0].y - start.y).abs() < 1e-6);
    assert!((arc.points[2].x - end.x).abs() < 1e-6 && (arc.points[2].y - end.y).abs() < 1e-6);
    assert!(
        ((((midpoint.x - center.x).powi(2) + (midpoint.y - center.y).powi(2)).sqrt()) - radius)
            .abs()
            < 1e-6,
        "expected synthesized midpoint to remain on the source circle"
    );
    assert!(
        (ccw_mid - ccw_span * 0.5).abs() < 1e-6,
        "expected synthesized midpoint to bisect the counterclockwise sweep"
    );
}

#[test]
fn preserves_point_on_circle_arc_gsp() {
    let scene = fixture_scene(include_bytes!(
        "../../../tests/fixtures/gsp/point_on_arc1.gsp"
    ));

    assert_eq!(scene.arcs.len(), 1, "expected one arc on the source circle");
    assert_eq!(
        scene.points.len(),
        5,
        "expected center, radius, arc endpoints, and one constrained point"
    );
    assert!(scene.points.iter().any(|point| matches!(
        point.constraint,
        ScenePointConstraint::OnCircleArc {
            center_index: 0,
            start_index: 2,
            end_index: 3,
            t,
        } if (t - 0.2648281634562194).abs() < 1e-9
    )));
}

#[test]
fn preserves_parameter_controlled_arc_on_circle_gsp() {
    let scene = fixture_scene(include_bytes!(
        "../../../tests/fixtures/gsp/static/value_point_arc_on_circle.gsp"
    ));

    assert!(
        scene.circles.iter().any(|circle| matches!(
            circle.binding,
            Some(crate::runtime::scene::ShapeBinding::PointRadiusCircle { .. })
        )),
        "expected the supporting payload circle to remain exported"
    );
    assert_eq!(
        scene.arcs.len(),
        1,
        "expected one arc driven by parameter points"
    );
    assert_eq!(
        scene.parameters.len(),
        2,
        "expected both arc endpoint parameters to remain interactive"
    );
    assert_eq!(scene.parameters[0].name, "t₁");
    assert!((scene.parameters[0].value - 1.3).abs() < 0.001);
    assert_eq!(scene.parameters[1].name, "t₂");
    assert!((scene.parameters[1].value - 0.4).abs() < 0.001);
    assert_eq!(
        scene.points.len(),
        6,
        "expected center, radius point, two parameter-controlled arc endpoints, and two legacy slider source points"
    );
    assert_eq!(
        scene
            .points
            .iter()
            .filter(|point| matches!(
                (&point.binding, &point.constraint),
                (
                    Some(ScenePointBinding::Parameter { .. }),
                    ScenePointConstraint::Free
                )
            ))
            .count(),
        2,
        "expected both payload slider source points to remain visible"
    );

    let arc = &scene.arcs[0];
    assert!(
        arc.center.is_some(),
        "expected arc-on-circle export to preserve the source center"
    );
    assert!(
        arc.counterclockwise,
        "expected circle arc to preserve sweep direction"
    );
    assert!(
        (arc.points[0].x - scene.points[2].position.x).abs() < 1e-6
            && (arc.points[0].y - scene.points[2].position.y).abs() < 1e-6
            && (arc.points[2].x - scene.points[3].position.x).abs() < 1e-6
            && (arc.points[2].y - scene.points[3].position.y).abs() < 1e-6,
        "expected arc endpoints to stay attached to the parameter-controlled points"
    );
    assert!(
        (scene.points[2].position.x - scene.points[3].position.x).abs() > 1e-6
            || (scene.points[2].position.y - scene.points[3].position.y).abs() > 1e-6,
        "expected distinct start and end points from the two payload values"
    );
}

#[test]
fn uses_document_canvas_bounds_for_rich_text_triangle_centers_layout() {
    let scene = fixture_scene(include_bytes!(
        "../../../tests/fixtures/未实现的系统功能/三角形的四心.gsp"
    ));

    assert_eq!(scene.bounds.min_x, 0.0);
    assert_eq!(scene.bounds.min_y, 0.0);
    assert_eq!(scene.bounds.max_x, 1850.0);
    assert_eq!(scene.bounds.max_y, 915.0);
    assert!(
        scene
            .labels
            .iter()
            .any(|label| label.text == "三角形的四心"),
        "expected the document title label to still be present"
    );
}

#[test]
fn preserves_point_hidden_gsp() {
    let scene = fixture_scene(include_bytes!(
        "../../../tests/fixtures/gsp/static/point_hidden.gsp"
    ));

    assert_eq!(scene.points.len(), 1, "expected one point in the fixture");
    assert!(
        !scene.points[0].visible,
        "expected fixture point to inherit hidden state from source metadata"
    );
    assert!(scene.lines.is_empty());
    assert!(scene.labels.is_empty());
}

#[test]
fn preserves_hidden_ray_gsp() {
    let scene = fixture_scene(include_bytes!(
        "../../../tests/fixtures/gsp/static/hide_ray.gsp"
    ));

    assert_eq!(scene.lines.len(), 2, "expected two rays in the fixture");
    assert!(
        scene.lines.iter().any(|line| !line.visible),
        "expected one ray to inherit hidden state from the source payload"
    );
    assert!(
        scene.lines.iter().any(|line| line.visible),
        "expected the visible ray to remain interactive in the exported scene"
    );
    assert!(
        scene.lines.iter().all(|line| matches!(
            line.binding,
            Some(crate::runtime::scene::LineBinding::Ray { .. })
        )),
        "expected both extracted line bindings to remain rays"
    );
}

#[test]
fn preserves_circle_center_radius_gsp() {
    let scene = fixture_scene(include_bytes!(
        "../../../tests/fixtures/gsp/circle_center_radius.gsp"
    ));

    assert!(matches!(
        scene.circles[0].binding,
        Some(crate::runtime::scene::ShapeBinding::SegmentRadiusCircle { .. })
    ));
    assert_eq!(scene.lines.len(), 1, "expected one segment");
    assert_eq!(scene.points.len(), 3, "expected three visible points");

    let circle = &scene.circles[0];
    assert!((circle.center.x - 348.0).abs() < 1e-6);
    assert!((circle.center.y - 177.0).abs() < 1e-6);
    assert!(matches!(
        circle.binding,
        Some(crate::runtime::scene::ShapeBinding::SegmentRadiusCircle {
            center_index: 2,
            line_start_index: 0,
            line_end_index: 1,
        })
    ));

    let radius = ((circle.radius_point.x - circle.center.x).powi(2)
        + (circle.radius_point.y - circle.center.y).powi(2))
    .sqrt();
    assert!(
        (radius - ((85.0_f64).powi(2) + 1.0_f64).sqrt()).abs() < 1e-6,
        "expected circle radius to match the referenced segment length"
    );
}

#[test]
fn preserves_circle_inner_fill_gsp() {
    let Some(data) = fixture_bytes("tests/fixtures/gsp/static/circle_inner.gsp") else {
        return;
    };
    let scene = fixture_scene(&data);

    assert!(
        scene.circles.iter().any(|circle| matches!(
            circle.binding,
            Some(crate::runtime::scene::ShapeBinding::PointRadiusCircle { .. })
        )),
        "expected the payload circle to remain exported"
    );
    let circle = &scene.circles[0];
    assert_eq!(
        circle.fill_color,
        Some([255, 255, 0, 127]),
        "expected circle interior payload to preserve its fill color"
    );
    assert!(matches!(
        circle.binding,
        Some(crate::runtime::scene::ShapeBinding::PointRadiusCircle {
            center_index: 0,
            radius_index: 1,
        })
    ));
}

#[test]
fn preserves_circle_system_bindings_for_inrm_fixture() {
    let scene = fixture_scene(include_bytes!(
        "../../../tests/fixtures/未实现/圆系(inRm).gsp"
    ));

    assert_eq!(
        scene.circles.len(),
        21,
        "expected source plus iterated payload circles"
    );
    assert_eq!(scene.polygons.len(), 1, "expected one payload polygon");
    assert_eq!(
        scene.points.len(),
        29,
        "expected base, iterated helper points, and the legacy parameter source point to export"
    );
    assert!(
        scene.points.iter().any(|point| matches!(
            point.constraint,
            ScenePointConstraint::OnPolygonBoundary { .. }
        )),
        "expected polygon-boundary helper point to stay exported for dependent bindings"
    );
    assert!(matches!(
        scene.circles[0].binding,
        Some(crate::runtime::scene::ShapeBinding::SegmentRadiusCircle { .. })
    ));
    assert!(matches!(
        scene.polygons[0].binding,
        Some(crate::runtime::scene::ShapeBinding::PointPolygon { .. })
    ));
}

#[test]
fn preserves_point_segment_value_segment_point_gsp() {
    let scene = fixture_scene(include_bytes!(
        "../../../tests/fixtures/gsp/static/point_segment_value_segment_point.gsp"
    ));

    assert_eq!(scene.lines.len(), 2, "expected two segments");
    let texts = scene
        .labels
        .iter()
        .map(|label| label.text.as_str())
        .collect::<Vec<_>>();
    assert!(
        texts.contains(&"C在AB上的t值 = 0.72"),
        "expected measured segment parameter label, got {texts:?}"
    );
    assert_eq!(
        scene.parameters.len(),
        0,
        "expected derived value, not slider parameter"
    );
    assert_eq!(
        scene
            .points
            .iter()
            .filter(|point| matches!(point.constraint, ScenePointConstraint::OnSegment { .. }))
            .count(),
        2,
        "expected measured point plus derived segment point"
    );
    assert!(
        scene
            .points
            .iter()
            .any(|point| matches!(point.constraint, ScenePointConstraint::OnCircle { .. })),
        "expected derived circle point"
    );
    assert!(scene.points.iter().any(|point| matches!(
        point.constraint,
        ScenePointConstraint::OnPolygonBoundary { .. }
    )));
}

#[test]
fn preserves_circle_parameter_label_in_circle_point_value_gsp() {
    let scene = fixture_scene(include_bytes!(
        "../../../tests/fixtures/gsp/static/circle_point_value.gsp"
    ));

    assert_eq!(scene.circles.len(), 1, "expected one circle");
    assert_eq!(
        scene.points.len(),
        3,
        "expected center, radius point, and constrained point"
    );
    let texts = scene
        .labels
        .iter()
        .map(|label| label.text.as_str())
        .collect::<Vec<_>>();
    assert!(
        texts.contains(&"A"),
        "expected point label A, got {texts:?}"
    );
    assert!(
        texts.contains(&"B"),
        "expected point label B, got {texts:?}"
    );
    assert!(
        texts.contains(&"C"),
        "expected point label C, got {texts:?}"
    );
    assert!(
        texts.contains(&"C在⊙AB上的值 = 0.38"),
        "expected circle parameter label, got {texts:?}"
    );
}

#[test]
fn preserves_parameter_controlled_point_on_segment_gsp() {
    let scene = fixture_scene(include_bytes!(
        "../../../tests/fixtures/gsp/static/point_on_segment.gsp"
    ));

    assert_eq!(scene.lines.len(), 1, "expected one segment");
    assert_eq!(
        scene.points.len(),
        4,
        "expected endpoints, the legacy slider source point, and the controlled point"
    );
    assert_eq!(scene.parameters.len(), 1, "expected t parameter");
    assert_eq!(scene.parameters[0].name, "t₁");
    assert!((scene.parameters[0].value - 0.7).abs() < 0.001);
    assert!(
        scene.points.iter().any(|point| {
            point.visible
                && matches!(
                    point.binding,
                    Some(ScenePointBinding::Parameter { ref name }) if name == "t₁"
                )
        }),
        "expected the payload slider source point to remain visible"
    );
    assert!(scene.points.iter().any(|point| {
        matches!(
            point.constraint,
            ScenePointConstraint::OnSegment { t, .. } if (t - 0.7).abs() < 0.001
        )
    }));
}

#[test]
fn preserves_parameter_controlled_point_on_poly_gsp() {
    let scene = fixture_scene(include_bytes!(
        "../../../tests/fixtures/gsp/static/point_on_poly.gsp"
    ));

    assert_eq!(scene.polygons.len(), 1, "expected one polygon");
    assert_eq!(
        scene.points.len(),
        5,
        "expected polygon vertices, the legacy slider source point, and the controlled point"
    );
    assert_eq!(scene.parameters.len(), 1, "expected t parameter");
    assert_eq!(scene.parameters[0].name, "t₁");
    assert!(
        scene.points.iter().any(|point| {
            point.visible
                && matches!(
                    point.binding,
                    Some(ScenePointBinding::Parameter { ref name }) if name == "t₁"
                )
        }),
        "expected the payload slider source point to remain visible"
    );
    assert!(scene.points.iter().any(|point| matches!(
        point.constraint,
        ScenePointConstraint::OnPolygonBoundary { .. }
    )));
}

#[test]
fn preserves_parameter_controlled_point_on_circle_gsp() {
    let scene = fixture_scene(include_bytes!(
        "../../../tests/fixtures/gsp/static/point_on_circle.gsp"
    ));

    assert!(
        scene.circles.iter().any(|circle| matches!(
            circle.binding,
            Some(crate::runtime::scene::ShapeBinding::PointRadiusCircle { .. })
        )),
        "expected the payload circle to remain exported"
    );
    assert_eq!(
        scene.points.len(),
        4,
        "expected circle points, the legacy slider source point, and the controlled point"
    );
    assert_eq!(scene.parameters.len(), 1, "expected t parameter");
    assert_eq!(scene.parameters[0].name, "t₁");
    assert!(
        scene.points.iter().any(|point| {
            point.visible
                && matches!(
                    point.binding,
                    Some(ScenePointBinding::Parameter { ref name }) if name == "t₁"
                )
        }),
        "expected the payload slider source point to remain visible"
    );
    assert!(
        scene
            .points
            .iter()
            .any(|point| matches!(point.constraint, ScenePointConstraint::OnCircle { .. }))
    );
}

#[test]
fn preserves_coordinate_point_in_cood_gsp() {
    let scene = fixture_scene(include_bytes!(
        "../../../tests/fixtures/gsp/static/cood.gsp"
    ));

    assert!(scene.graph_mode, "expected graph scene");
    assert_eq!(scene.parameters.len(), 1, "expected t parameter");
    assert_eq!(scene.parameters[0].name, "t₁");
    assert!((scene.parameters[0].value - 0.01).abs() < 0.001);
    assert!(
        scene.points.iter().any(|point| {
            point.binding.as_ref().is_some_and(|binding| {
                matches!(
                    binding,
                    ScenePointBinding::Coordinate { name, .. } if name == "t₁"
                )
            })
        }),
        "expected coordinate-controlled point"
    );
    assert!(scene.points.iter().any(|point| {
        (point.position.x - 0.01).abs() < 0.001 && (point.position.y - 1.01).abs() < 0.001
    }));
}

#[test]
fn preserves_coordinate_trace_in_cood_trace_gsp() {
    let scene = fixture_scene(include_bytes!(
        "../../../tests/fixtures/gsp/static/cood-trace.gsp"
    ));

    assert!(scene.graph_mode, "expected graph scene");
    assert_eq!(scene.parameters.len(), 1, "expected t parameter");
    assert_eq!(scene.parameters[0].name, "t₁");
    assert!(
        scene.lines.iter().any(|line| {
            line.points.len() > 100
                && line
                    .points
                    .first()
                    .is_some_and(|point| point.x.abs() < 0.001)
                && line
                    .points
                    .first()
                    .is_some_and(|point| (point.y - 1.0).abs() < 0.001)
                && line
                    .points
                    .last()
                    .is_some_and(|point| (point.x - 1.0).abs() < 0.001)
                && line
                    .points
                    .last()
                    .is_some_and(|point| (point.y - 2.0).abs() < 0.001)
        }),
        "expected sampled coordinate trace line"
    );
}

#[test]
fn does_not_synthesize_graph_calibration_labels_in_cood_intersection_gsp() {
    let scene = fixture_scene(include_bytes!(
        "../../../tests/fixtures/gsp/insection/cood.gsp"
    ));

    assert!(
        scene
            .labels
            .iter()
            .all(|label| label.text != "37.80" && label.text != "37.8"),
        "expected no synthesized graph calibration labels, got {:?}",
        scene
            .labels
            .iter()
            .map(|label| label.text.as_str())
            .collect::<Vec<_>>()
    );
    assert!(
        scene.points.iter().any(|point| {
            point.visible
                && point.draggable
                && (point.position.x - 1.0).abs() < 1e-6
                && point.position.y.abs() < 1e-6
                && matches!(
                    point.binding,
                    Some(crate::runtime::scene::ScenePointBinding::GraphCalibration)
                )
        }),
        "expected visible interactive graph calibration point at (1,0), got {:?}",
        scene
            .points
            .iter()
            .map(|point| (
                point.position.x,
                point.position.y,
                point.visible,
                point.draggable,
                point.binding.as_ref().map(|binding| format!("{binding:?}"))
            ))
            .collect::<Vec<_>>()
    );
    assert!(
        scene.points.iter().any(|point| {
            !point.visible
                && point.draggable
                && point.position.x.abs() < 1e-6
                && (point.position.y - 1.0).abs() < 1e-6
                && matches!(
                    point.binding,
                    Some(crate::runtime::scene::ScenePointBinding::GraphCalibration)
                )
        }),
        "expected hidden interactive graph calibration point at (0,1), got {:?}",
        scene
            .points
            .iter()
            .map(|point| (
                point.position.x,
                point.position.y,
                point.visible,
                point.draggable,
                point.binding.as_ref().map(|binding| format!("{binding:?}"))
            ))
            .collect::<Vec<_>>()
    );
}

#[test]
fn preserves_coordinate_trace_intersection_in_cood_intersection_gsp() {
    let scene = fixture_scene(include_bytes!(
        "../../../tests/fixtures/gsp/insection/cood_intersection.gsp"
    ));

    assert!(scene.graph_mode, "expected graph scene");
    assert_eq!(
        scene.points.len(),
        7,
        "expected source, derived, intersection points, and the legacy parameter source point"
    );
    assert!(scene.lines.iter().any(|line| {
        matches!(
            line.binding,
            Some(crate::runtime::scene::LineBinding::CoordinateTrace { point_index, .. })
                if point_index == 4
        )
    }));
    assert!(scene.points.iter().any(|point| {
        matches!(
            point.binding,
            Some(crate::runtime::scene::ScenePointBinding::CoordinateSource {
                ref name,
                ..
            }) if name == "t₁"
        ) && (point.position.x - 4.021666666666667).abs() < 1e-6
            && (point.position.y - 4.021666666666667).abs() < 1e-6
    }));
    assert!(scene.points.iter().any(|point| {
        matches!(
            point.constraint,
            crate::runtime::scene::ScenePointConstraint::LineTraceIntersection {
                point_index,
                ..
            } if point_index == 4
        ) && point.position.x.abs() < 1e-6
            && point.position.y.abs() < 1e-6
    }));
}

#[test]
fn preserves_coordinate_trace_intersection_in_cood_intersection_y_gsp() {
    let scene = fixture_scene(include_bytes!(
        "../../../tests/fixtures/gsp/insection/cood_intersection_y.gsp"
    ));

    assert!(scene.graph_mode, "expected graph scene");
    assert_eq!(
        scene.points.len(),
        7,
        "expected source, derived, intersection points, and the legacy parameter source point"
    );
    assert!(scene.lines.iter().any(|line| {
        matches!(
            line.binding,
            Some(crate::runtime::scene::LineBinding::CoordinateTrace { point_index, .. })
                if point_index == 4
        )
    }));
    assert!(scene.points.iter().any(|point| {
        matches!(
            point.binding,
            Some(crate::runtime::scene::ScenePointBinding::CoordinateSource {
                ref name,
                axis: crate::runtime::scene::CoordinateAxis::Horizontal,
                ..
            }) if name == "t₁"
        ) && (point.position.x - -2.0427083333333336).abs() < 1e-6
            && (point.position.y - -2.8839583333333336).abs() < 1e-6
    }));
    assert!(scene.points.iter().any(|point| {
        matches!(
            point.constraint,
            crate::runtime::scene::ScenePointConstraint::LineTraceIntersection {
                point_index, ..
            } if point_index == 4
        ) && point.position.x.abs() < 1e-6
            && point.position.y.abs() < 1e-6
    }));
}

#[test]
fn preserves_coordinate_trace_intersection_in_cood_intersection_xy_gsp() {
    let scene = fixture_scene(include_bytes!(
        "../../../tests/fixtures/gsp/insection/cood_intersection_xy.gsp"
    ));

    assert!(scene.graph_mode, "expected graph scene");
    assert_eq!(
        scene.points.len(),
        7,
        "expected source, derived, intersection points, and the legacy parameter source point"
    );
    assert!(scene.lines.iter().any(|line| {
        matches!(
            line.binding,
            Some(crate::runtime::scene::LineBinding::CoordinateTrace { point_index, .. })
                if point_index == 4
        )
    }));
    assert!(scene.points.iter().any(|point| {
        matches!(
            point.binding,
            Some(crate::runtime::scene::ScenePointBinding::CoordinateSource2d {
                ref x_name,
                ref y_name,
                ..
            }) if x_name == "t₁" && y_name == "t₁"
        ) && (point.position.x - -0.5345833333333322).abs() < 1e-6
            && (point.position.y - 2.5345833333333334).abs() < 1e-6
    }));
    assert!(scene.points.iter().any(|point| {
        matches!(
            point.constraint,
            crate::runtime::scene::ScenePointConstraint::LineTraceIntersection {
                point_index, ..
            } if point_index == 4
        ) && point.position.x.abs() < 1e-6
            && (point.position.y - 3.069166666666897).abs() < 1e-6
    }));
}

#[test]
fn preserves_midpoint_binding_and_trace_in_trace_gsp() {
    let scene = fixture_scene(include_bytes!("../../../tests/fixtures/gsp/trace.gsp"));

    let midpoint_index = scene
        .points
        .iter()
        .enumerate()
        .find_map(|(index, point)| match (&point.constraint, &point.binding) {
            (
                ScenePointConstraint::OnSegment {
                    start_index,
                    end_index,
                    t,
                },
                Some(ScenePointBinding::Midpoint {
                    start_index: binding_start,
                    end_index: binding_end,
                }),
            ) if *start_index == 4
                && *end_index == 0
                && *binding_start == 4
                && *binding_end == 0
                && (*t - 0.5).abs() < 1e-9 =>
            {
                Some(index)
            }
            _ => None,
        })
        .expect("expected derived midpoint point");
    assert!(scene.points[midpoint_index].visible);

    assert!(
        scene.lines.iter().any(|line| {
            if line.points.len() < 100 {
                return false;
            }
            let first = line.points.first().expect("non-empty line");
            let last = line.points.last().expect("non-empty line");
            ((first.x - 846.5).abs() < 0.01
                && (first.y - 480.0).abs() < 0.01
                && (last.x - 766.0).abs() < 0.01
                && (last.y - 359.25).abs() < 0.01)
                || ((last.x - 846.5).abs() < 0.01
                    && (last.y - 480.0).abs() < 0.01
                    && (first.x - 766.0).abs() < 0.01
                    && (first.y - 359.25).abs() < 0.01)
        }),
        "expected sampled midpoint trace line"
    );
}

#[test]
fn preserves_parameter_driven_point_iteration_family() {
    let scene = fixture_scene(include_bytes!(
        "../../../tests/fixtures/gsp/static/简单迭代/原象点初象点深度5迭代.gsp"
    ));

    assert_eq!(scene.parameters.len(), 1, "expected n parameter");
    assert_eq!(scene.parameters[0].name, "n");
    assert_eq!(
        scene.point_iterations.len(),
        1,
        "expected one point iteration family"
    );
    match &scene.point_iterations[0] {
        PointIterationFamily::Offset {
            seed_index,
            depth,
            parameter_name,
            ..
        } => {
            assert_eq!(
                *seed_index, 1,
                "expected initial image point as iteration seed"
            );
            assert_eq!(*depth, 5, "expected exported depth");
            assert_eq!(parameter_name.as_deref(), Some("n"));
        }
        family => panic!("expected offset iteration family, got {family:?}"),
    }
    assert_eq!(
        scene.points.len(),
        8,
        "expected original point, initial point, 5 iterates, and the legacy parameter source point"
    );
}

#[test]
fn preserves_linear_intersection_points_in_insection_fixtures() {
    for (name, data) in [
        (
            "segment",
            include_bytes!("../../../tests/fixtures/gsp/insection/segment_insection.gsp")
                .as_slice(),
        ),
        (
            "line",
            include_bytes!("../../../tests/fixtures/gsp/insection/line_insection.gsp").as_slice(),
        ),
        (
            "ray",
            include_bytes!("../../../tests/fixtures/gsp/insection/ray_insection.gsp").as_slice(),
        ),
    ] {
        let scene = fixture_scene(data);

        assert_eq!(
            scene.points.len(),
            5,
            "expected derived intersection point for {name}"
        );
        assert!(scene.points.iter().any(|point| {
            matches!(
                point.constraint,
                ScenePointConstraint::LineIntersection { .. }
            )
        }));
        assert!(
            scene.points.iter().any(|point| {
                (point.position.x - 416.3160761196899).abs() < 1e-6
                    && (point.position.y - 321.2222079835971).abs() < 1e-6
            }),
            "expected derived intersection coordinates for {name}"
        );
    }
}

#[test]
fn preserves_circle_circle_intersection_points() {
    let scene = fixture_scene(include_bytes!(
        "../../../tests/fixtures/gsp/insection/circle_circle_insection.gsp"
    ));

    assert_eq!(
        scene.points.len(),
        6,
        "expected both circle-circle intersections"
    );
    assert!(scene.points.iter().any(|point| {
        matches!(
            point.constraint,
            ScenePointConstraint::CircleCircleIntersection { .. }
        )
    }));
    assert!(scene.points.iter().any(|point| {
        (point.position.x - 421.3993346591643).abs() < 1e-6
            && (point.position.y - 189.66291724683578).abs() < 1e-6
    }));
    assert!(scene.points.iter().any(|point| {
        (point.position.x - 445.71654184257966).abs() < 1e-6
            && (point.position.y - 470.02601183209464).abs() < 1e-6
    }));
}

#[test]
fn preserves_two_circle_intersection_inrm_fixture_interactivity() {
    let scene = fixture_scene(include_bytes!(
        "../../../tests/fixtures/未实现/(inRm)两圆之交.gsp"
    ));

    assert_eq!(scene.circles.len(), 4, "expected four source circles");
    assert_eq!(
        scene.polygons.len(),
        2,
        "expected two circular segments that make up the lens"
    );
    assert_eq!(
        scene.lines.len(),
        7,
        "expected five source helper lines plus two live circular-segment boundaries"
    );
    assert_eq!(
        scene.points.len(),
        14,
        "expected source points plus derived circle intersections"
    );
    assert_eq!(
        scene
            .circles
            .iter()
            .filter(|circle| matches!(
                circle.binding,
                Some(crate::runtime::scene::ShapeBinding::PointRadiusCircle { .. })
            ))
            .count(),
        4,
        "expected every payload circle to keep its live center/radius binding"
    );
    assert_eq!(
        scene
            .circles
            .iter()
            .filter(|circle| circle.fill_color.is_some())
            .count(),
        0,
        "expected duplicate helper circles to avoid rendering full-disk fills"
    );
    assert_eq!(
        scene
            .polygons
            .iter()
            .filter(|polygon| matches!(
                polygon.binding,
                Some(crate::runtime::scene::ShapeBinding::ArcBoundaryPolygon { .. })
            ))
            .count(),
        2,
        "expected both circular segments to stay interactive"
    );
    assert_eq!(
        scene
            .lines
            .iter()
            .filter(|line| matches!(line.binding, Some(LineBinding::Segment { .. })))
            .count(),
        2,
        "expected both payload segments to stay interactive"
    );
    assert_eq!(
        scene
            .lines
            .iter()
            .filter(|line| matches!(line.binding, Some(LineBinding::PerpendicularLine { .. })))
            .count(),
        2,
        "expected both payload perpendicular helpers to stay interactive"
    );
    assert_eq!(
        scene
            .lines
            .iter()
            .filter(|line| matches!(line.binding, Some(LineBinding::Line { .. })))
            .count(),
        1,
        "expected the payload baseline to stay interactive"
    );

    let circle_circle_points = scene
        .points
        .iter()
        .filter(|point| {
            matches!(
                point.constraint,
                ScenePointConstraint::CircleCircleIntersection { .. }
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        circle_circle_points.len(),
        2,
        "expected both circle-circle variants to stay exported"
    );
    assert!(circle_circle_points.iter().all(|point| {
        (point.position.x - 327.0).abs() < 1e-6 && (point.position.y - 275.0).abs() < 1e-6
    }));
    assert_eq!(
        scene
            .points
            .iter()
            .filter(|point| {
                matches!(
                    point.constraint,
                    ScenePointConstraint::LineCircleIntersection { .. }
                )
            })
            .count(),
        8,
        "expected all derived line-circle intersection helpers to stay live"
    );
}

#[test]
fn preserves_cans_in_container_inrm_fixture_interactivity() {
    let Some(data) = fixture_bytes("tests/fixtures/未实现/(inRm)容器中的罐头.gsp") else {
        return;
    };
    let scene = fixture_scene(&data);

    assert_eq!(
        scene.lines.len(),
        13,
        "expected source guide lines to export"
    );
    assert_eq!(
        scene.circles.len(),
        38,
        "expected the payload can circles to export"
    );
    assert_eq!(
        scene.points.len(),
        40,
        "expected helper points to stay exported"
    );
    assert_eq!(
        scene
            .circles
            .iter()
            .filter(|circle| matches!(
                circle.binding,
                Some(crate::runtime::scene::ShapeBinding::SegmentRadiusCircle { .. })
            ))
            .count(),
        38,
        "expected every payload circle to keep its live segment-radius binding"
    );
    assert_eq!(
        scene.circles.iter().filter(|circle| circle.visible).count(),
        24,
        "expected the visible can circles to remain rendered"
    );
    assert_eq!(
        scene.points.iter().filter(|point| point.visible).count(),
        3,
        "expected the payload draggable points to stay visible"
    );
    assert_eq!(
        scene
            .points
            .iter()
            .filter(|point| matches!(point.constraint, ScenePointConstraint::OnSegment { .. }))
            .count(),
        2,
        "expected both payload slider points to remain segment constrained"
    );
    assert_eq!(
        scene
            .points
            .iter()
            .filter(|point| matches!(point.constraint, ScenePointConstraint::Offset { .. }))
            .count(),
        1,
        "expected the offset helper point to stay live"
    );
    assert_eq!(
        scene
            .points
            .iter()
            .filter(|point| matches!(point.binding, Some(ScenePointBinding::Scale { .. })))
            .count(),
        4,
        "expected scale-derived helper points to preserve their bindings"
    );
    assert_eq!(
        scene
            .points
            .iter()
            .filter(|point| matches!(point.binding, Some(ScenePointBinding::Rotate { .. })))
            .count(),
        1,
        "expected the rotated helper point to preserve its binding"
    );
    assert_eq!(
        scene
            .points
            .iter()
            .filter(|point| matches!(point.binding, Some(ScenePointBinding::Translate { .. })))
            .count(),
        5,
        "expected translated helper points to preserve their bindings"
    );
    assert!(
        scene
            .labels
            .iter()
            .any(|label| label.visible && label.text == "M"),
        "expected the payload midpoint label to stay visible"
    );
}

#[test]
fn preserves_line_circle_intersection_points() {
    let scene = fixture_scene(include_bytes!(
        "../../../tests/fixtures/gsp/insection/circle_insection.gsp"
    ));

    assert_eq!(
        scene.points.len(),
        5,
        "expected derived line-circle intersection"
    );
    assert!(scene.points.iter().any(|point| {
        matches!(
            point.constraint,
            ScenePointConstraint::LineCircleIntersection { .. }
        )
    }));
    assert!(scene.points.iter().any(|point| {
        (point.position.x - 167.5150597569313).abs() < 1e-6
            && (point.position.y - 204.5902707856141).abs() < 1e-6
    }));
}

#[test]
fn preserves_perpendicular_intersection_points_in_perp_fixture() {
    let scene = fixture_scene(include_bytes!("../../../tests/fixtures/gsp/perp.gsp"));

    let intersection = scene
        .points
        .iter()
        .find(|point| {
            matches!(
                point.constraint,
                ScenePointConstraint::LineIntersection {
                    left: LineConstraint::Segment { .. } | LineConstraint::Line { .. },
                    right: LineConstraint::PerpendicularLine {
                        through_index: 2,
                        ..
                    },
                }
            )
        })
        .expect("expected reactive intersection point bound to the perpendicular line");

    assert!(
        (intersection.position.x - 867.3347427619169).abs() < 1e-6
            && (intersection.position.y - 469.9559050197873).abs() < 1e-6,
        "expected foot-of-perpendicular coordinates, got {:?}",
        intersection.position
    );
}

#[test]
fn preserves_circle_y_intersection_points() {
    let scene = fixture_scene(include_bytes!(
        "../../../tests/fixtures/gsp/circle_y_intersection.gsp"
    ));

    assert!(scene.points.iter().any(|point| {
        point.visible
            && (point.position.x - 1.0).abs() < 1e-6
            && (point.position.y - 0.0).abs() < 1e-6
            && matches!(
                point.binding,
                Some(crate::runtime::scene::ScenePointBinding::GraphCalibration)
            )
    }));
    assert!(scene.labels.iter().any(|label| label.text == "G"));
    assert!(
        scene.points.iter().any(|point| {
            matches!(
                point.constraint,
                ScenePointConstraint::LineCircleIntersection { .. }
            ) && (point.position.x - 0.0).abs() < 1e-6
                && (point.position.y - 1.0).abs() < 1e-6
        }),
        "expected y-axis circle intersection point, got {:?}",
        scene
            .points
            .iter()
            .map(|point| (&point.position.x, &point.position.y, &point.constraint))
            .collect::<Vec<_>>()
    );
}

#[test]
fn preserves_three_point_arc_intersection_points() {
    let scene = fixture_scene(include_bytes!(
        "../../../tests/fixtures/gsp/static/three_point_arc_intersection.gsp"
    ));

    assert_eq!(
        scene.points.len(),
        7,
        "expected original arc control points plus one derived intersection"
    );
    assert!(
        scene.points.iter().any(|point| {
            matches!(
                point.constraint,
                ScenePointConstraint::CircularIntersection { .. }
            ) && (point.position.x - 471.96614672487107).abs() < 1e-6
                && (point.position.y - 484.54842372244576).abs() < 1e-6
        }),
        "expected reactive arc intersection, got {:?}",
        scene
            .points
            .iter()
            .map(|point| (&point.position.x, &point.position.y, &point.constraint))
            .collect::<Vec<_>>()
    );
}

#[test]
fn preserves_non_graph_parameter_and_expression_labels_in_iteration_fixture() {
    let scene = fixture_scene(include_bytes!(
        "../../../tests/fixtures/gsp/static/简单迭代/原象点和参数初象点和数值深度5迭代.gsp"
    ));

    let parameter_names = scene
        .parameters
        .iter()
        .map(|parameter| parameter.name.as_str())
        .collect::<Vec<_>>();
    assert!(
        parameter_names.contains(&"n"),
        "expected n parameter, got {parameter_names:?}"
    );
    assert!(
        parameter_names.contains(&"a"),
        "expected a parameter, got {parameter_names:?}"
    );
    assert!(scene.labels.iter().any(|label| {
        matches!(
            label.binding,
            Some(TextLabelBinding::ParameterValue { ref name }) if name == "a"
        )
    }));
    assert!(scene.labels.iter().any(|label| {
        matches!(
            label.binding,
            Some(TextLabelBinding::PointExpressionValue {
                ref parameter_name,
                ..
            }) if parameter_name == "a"
        )
    }));
    assert!(scene.labels.iter().any(|label| {
        matches!(
            label.binding,
            Some(TextLabelBinding::ExpressionValue {
                ref parameter_name,
                ref expr_label,
                ..
            }) if parameter_name == "a" && expr_label == "a + 1"
        )
    }));
    assert!(scene.point_iterations.iter().any(|family| {
        matches!(
            family,
            PointIterationFamily::Offset {
                dx,
                dy,
                parameter_name,
                ..
            } if parameter_name.as_deref() == Some("n")
                && (*dx - 37.79527559055118).abs() < 1e-6
                && dy.abs() < 1e-6
        )
    }));
    assert!(scene.label_iterations.iter().any(|family| {
        matches!(
            family,
            LabelIterationFamily::PointExpression {
                parameter_name,
                depth_parameter_name,
                ..
            } if parameter_name == "a" && depth_parameter_name.as_deref() == Some("n")
        )
    }));
}

#[test]
fn preserves_default_depth_non_graph_iteration_fixture() {
    let scene = fixture_scene(include_bytes!(
        "../../../tests/fixtures/gsp/static/简单迭代/原象点和参数初象点和数值默认深度迭代.gsp"
    ));

    let parameter_names = scene
        .parameters
        .iter()
        .map(|parameter| parameter.name.as_str())
        .collect::<Vec<_>>();
    assert_eq!(parameter_names, vec!["a"]);
    assert!(scene.point_iterations.iter().any(|family| {
        matches!(
            family,
            PointIterationFamily::RotateChain {
                seed_index,
                center_index,
                angle_degrees,
                depth,
            } if *seed_index == 2
                && *center_index == 0
                && (*angle_degrees - 30.0).abs() < 1e-6
                && *depth == 3
        )
    }));
    assert!(scene.label_iterations.iter().any(|family| {
        matches!(
            family,
            LabelIterationFamily::PointExpression {
                parameter_name,
                depth,
                depth_parameter_name,
                ..
            } if parameter_name == "a" && *depth == 3 && depth_parameter_name.is_none()
        )
    }));
}

#[test]
fn preserves_carried_segment_default_depth_iteration_fixture() {
    let scene = fixture_scene(include_bytes!(
        "../../../tests/fixtures/gsp/static/简单迭代/原象点初象携带线段默认深度3迭代.gsp"
    ));

    assert_eq!(
        scene.lines.len(),
        4,
        "expected original segment plus three carried copies"
    );
    assert_eq!(
        scene.points.len(),
        5,
        "expected original point, seed point, and three iterates"
    );
    let starts = scene
        .lines
        .iter()
        .map(|line| line.points.first().cloned().expect("segment start"))
        .collect::<Vec<_>>();
    assert!(
        starts
            .iter()
            .any(|point| { (point.x - 168.0).abs() < 1e-6 && (point.y - 376.0).abs() < 1e-6 })
    );
    assert!(starts.iter().any(|point| {
        (point.x - 205.79527559055117).abs() < 1e-6 && (point.y - 338.20472440944883).abs() < 1e-6
    }));
    assert!(starts.iter().any(|point| {
        (point.x - 243.59055118110234).abs() < 1e-6 && (point.y - 300.40944881889766).abs() < 1e-6
    }));
    assert!(starts.iter().any(|point| {
        (point.x - 281.3858267716535).abs() < 1e-6 && (point.y - 262.6141732283465).abs() < 1e-6
    }));
}

#[test]
fn preserves_carried_polygon_iteration_fixture() {
    let scene = fixture_scene(include_bytes!(
        "../../../tests/fixtures/gsp/static/简单迭代/原象点初象携带多边形双映射深度4迭代.gsp"
    ));

    assert_eq!(
        scene.polygons.len(),
        15,
        "expected triangular lattice of seed polygon plus carried copies"
    );
    assert!(
        scene.lines.len() <= 1,
        "expected polygon edges to stay suppressed apart from any standalone parameter-control helper geometry"
    );
    assert!(
        scene
            .parameters
            .iter()
            .any(|parameter| parameter.name == "n")
    );
    assert!(
        scene.line_iterations.is_empty(),
        "expected carried polygon fixture to avoid duplicate line iteration metadata"
    );
    assert_eq!(scene.polygon_iterations.len(), 1);
    assert!(scene.polygon_iterations.iter().any(|family| {
        matches!(
            family,
            PolygonIterationFamily::Translate {
                parameter_name,
                depth,
                vertex_indices,
                secondary_dx,
                secondary_dy,
                dx,
                dy,
                ..
            } if parameter_name.as_deref() == Some("n")
                && *depth == 4
                && *vertex_indices == vec![0, 2, 1]
                && secondary_dx.is_some()
                && secondary_dy.is_some()
                && dx.abs() < 1e-6
                && (*dy + 37.79527559055118).abs() < 1e-6
        )
    }));
    assert_eq!(
        scene.points.len(),
        4,
        "expected base point, two mapped vertices, and the legacy parameter source point"
    );
    assert!(matches!(
        scene.points[1].constraint,
        ScenePointConstraint::Offset {
            origin_index: 0,
            dx,
            dy,
        } if (dx - 37.79527559055118).abs() < 1e-6
            && (dy + 37.79527559055118).abs() < 1e-6
    ));
    assert!(matches!(
        scene.points[2].constraint,
        ScenePointConstraint::Offset {
            origin_index: 0,
            dx,
            dy,
        } if dx.abs() < 1e-6 && (dy + 37.79527559055118).abs() < 1e-6
    ));
    let first_vertices = scene
        .polygons
        .iter()
        .map(|polygon| polygon.points.first().cloned().expect("polygon vertex"))
        .collect::<Vec<_>>();
    assert!(
        first_vertices
            .iter()
            .any(|point| { (point.x - 168.0).abs() < 1e-6 && (point.y - 376.0).abs() < 1e-6 })
    );
    assert!(first_vertices.iter().any(|point| {
        (point.x - 168.0).abs() < 1e-6 && (point.y - 338.20472440944883).abs() < 1e-6
    }));
    assert!(first_vertices.iter().any(|point| {
        (point.x - 168.0).abs() < 1e-6 && (point.y - 300.40944881889766).abs() < 1e-6
    }));
    assert!(first_vertices.iter().any(|point| {
        (point.x - 168.0).abs() < 1e-6 && (point.y - 262.6141732283465).abs() < 1e-6
    }));
    assert!(first_vertices.iter().any(|point| {
        (point.x - 168.0).abs() < 1e-6 && (point.y - 224.81889763779532).abs() < 1e-6
    }));
    assert!(first_vertices.iter().any(|point| {
        (point.x - 205.79527559055117).abs() < 1e-6 && (point.y - 300.40944881889766).abs() < 1e-6
    }));
    assert!(first_vertices.iter().any(|point| {
        (point.x - 243.59055118110234).abs() < 1e-6 && (point.y - 262.6141732283465).abs() < 1e-6
    }));
}

#[test]
fn preserves_default_depth_point_iteration_family() {
    let scene = fixture_scene(include_bytes!(
        "../../../tests/fixtures/gsp/static/简单迭代/原象点初象点默认深度3迭代.gsp"
    ));

    assert!(
        scene.parameters.is_empty(),
        "expected default-depth fixture without editable parameters"
    );
    assert_eq!(
        scene.point_iterations.len(),
        1,
        "expected one default-depth point iteration family"
    );
    match &scene.point_iterations[0] {
        PointIterationFamily::Offset {
            seed_index,
            depth,
            parameter_name,
            ..
        } => {
            assert_eq!(
                *seed_index, 1,
                "expected initial image point as iteration seed"
            );
            assert_eq!(*depth, 3, "expected default depth of three");
            assert_eq!(parameter_name, &None);
        }
        family => panic!("expected offset iteration family, got {family:?}"),
    }
    assert_eq!(
        scene.points.len(),
        5,
        "expected original point, initial point, and three default iterates"
    );
    assert!(
        matches!(
            scene.points[1].constraint,
            ScenePointConstraint::Offset {
                origin_index: 0,
                dx,
                dy
            } if (dx - 37.79527559055118).abs() < 1e-6
                && (dy + 37.79527559055118).abs() < 1e-6
        ),
        "expected legacy initial image point to preserve its 1cm horizontal and vertical offsets"
    );
}

#[test]
fn does_not_treat_triangle_point_labels_as_iteration_parameters() {
    let scene = fixture_scene(include_bytes!(
        "../../../tests/fixtures/gsp/static/简单迭代/三角形.gsp"
    ));

    assert!(
        scene.parameters.is_empty(),
        "expected no editable parameters in triangle fixture"
    );
    assert_eq!(scene.line_iterations.len(), 3);
    assert!(
        scene
            .line_iterations
            .iter()
            .all(|family| matches!(family, LineIterationFamily::Affine { .. }))
    );
}

#[test]
fn preserves_midpoint_triangle_iteration_geometry() {
    let scene = fixture_scene(include_bytes!(
        "../../../tests/fixtures/gsp/static/简单迭代/三角形.gsp"
    ));

    assert!(scene.lines.iter().any(|line| {
        line.points.len() == 2
            && (line.points[0].x - 751.0).abs() < 0.01
            && (line.points[0].y - 467.5).abs() < 0.01
            && (line.points[1].x - 853.0).abs() < 0.01
            && (line.points[1].y - 319.5).abs() < 0.01
    }));
    assert!(
        !scene.lines.iter().any(|line| {
            line.points.len() == 2
                && (line.points[0].x - 367.0).abs() < 0.01
                && (line.points[0].y - 786.0).abs() < 0.01
        }),
        "expected midpoint recursion, not translated copies"
    );
}

#[test]
fn preserves_regular_polygon_iteration_without_carried_duplicates() {
    let scene = fixture_scene(include_bytes!(
        "../../../tests/fixtures/gsp/static/简单迭代/迭代正多边形.gsp"
    ));

    assert_eq!(scene.parameters.len(), 1, "expected editable n parameter");
    assert_eq!(scene.parameters[0].name, "n");
    assert_eq!(
        scene.lines.len(),
        1,
        "expected the payload's first related edge to stay serialized as the iteration source"
    );
    assert_eq!(
        scene
            .lines
            .iter()
            .filter(|line| line.debug.as_ref().is_some_and(|debug| debug.group_ordinal == 7))
            .count(),
        1,
        "expected the serialized source edge to come from payload segment #7"
    );
    assert!(
        scene.line_iterations.iter().any(|family| matches!(
            family,
            LineIterationFamily::Rotate {
                source_index,
                parameter_name,
                depth_parameter_name,
                depth,
                ..
            } if *source_index == 0
                && parameter_name.as_deref() == Some("n")
                && depth_parameter_name.is_none()
                && *depth == 4
        )),
        "expected regular polygon iteration to export the payload source edge plus a rotate family for the carried copies"
    );
    assert_eq!(
        scene.line_iterations.len(),
        1,
        "expected one canonical rotate family for the regular polygon payload"
    );
}

#[test]
fn preserves_scaled_point_and_single_parameter_label_in_scale_gsp() {
    let scene = fixture_scene(include_bytes!(
        "../../../tests/fixtures/gsp/static/scale.gsp"
    ));

    assert_eq!(
        scene.circles.len(),
        2,
        "expected original and scaled circle"
    );
    assert!(scene.circles.iter().any(|circle| matches!(
        circle.binding,
        Some(crate::runtime::scene::ShapeBinding::ScaleCircle { .. })
    )));
    assert_eq!(
        scene.polygons.len(),
        2,
        "expected original and scaled polygon"
    );
    let texts = scene
        .labels
        .iter()
        .map(|label| label.text.as_str())
        .collect::<Vec<_>>();
    assert!(
        texts.contains(&"A"),
        "expected point label A, got {texts:?}"
    );
    assert!(
        texts.contains(&"B"),
        "expected point label B, got {texts:?}"
    );
    assert!(
        texts.contains(&"C"),
        "expected point label C, got {texts:?}"
    );
    assert!(
        scene.points.len() >= 3,
        "expected source point, center point, and transformed point"
    );
    assert!(scene.points.iter().any(|point| {
        matches!(
            point.binding,
            Some(ScenePointBinding::Scale { factor, .. }) if (factor - 1.0 / 3.0).abs() < 0.0001
        )
    }));
}

#[test]
fn preserves_reflection_point_circle_and_polygon_gsp() {
    let scene = fixture_scene(include_bytes!(
        "../../../tests/fixtures/gsp/static/reflection.gsp"
    ));

    assert_eq!(
        scene.circles.len(),
        2,
        "expected original and reflected circle"
    );
    assert_eq!(
        scene.polygons.len(),
        2,
        "expected original and reflected polygon"
    );
    assert!(
        scene
            .points
            .iter()
            .any(|point| matches!(point.binding, Some(ScenePointBinding::Reflect { .. })))
    );
    assert!(scene.circles.iter().any(|circle| matches!(
        circle.binding,
        Some(crate::runtime::scene::ShapeBinding::ReflectCircle { .. })
    )));
    assert!(scene.polygons.iter().any(|polygon| matches!(
        polygon.binding,
        Some(crate::runtime::scene::ShapeBinding::ReflectPolygon { .. })
    )));
}

#[test]
fn preserves_reflected_circle_across_constructed_perpendicular_line() {
    let scene = fixture_scene(include_bytes!("../../../tests/fixtures/bug/镜像圆.gsp"));

    assert_eq!(
        scene.circles.len(),
        2,
        "expected original and reflected circles"
    );
    assert!(scene.circles.iter().any(|circle| matches!(
        circle.binding,
        Some(crate::runtime::scene::ShapeBinding::ReflectCircle {
            line_index: Some(_),
            ..
        })
    )));
}

#[test]
fn preserves_translated_triangle_segments_in_congruent_triangle_fixture() {
    let scene = fixture_scene(include_bytes!(
        "../../../tests/fixtures/gsp/两个三角形标记全等.gsp"
    ));

    assert_eq!(
        scene.lines.len(),
        16,
        "expected source and translated edges plus angle and segment congruence markers"
    );
    assert_eq!(
        scene
            .lines
            .iter()
            .filter(|line| matches!(line.binding, Some(LineBinding::TranslateLine { .. })))
            .count(),
        3,
        "expected the translated triangle to contribute three translated segment bindings"
    );
    assert_eq!(
        scene
            .lines
            .iter()
            .filter(|line| matches!(line.binding, Some(LineBinding::AngleMarker { .. })))
            .count(),
        4,
        "expected four reactive angle markers"
    );
    assert_eq!(
        scene
            .lines
            .iter()
            .filter(|line| matches!(line.binding, Some(LineBinding::SegmentMarker { .. })))
            .count(),
        6,
        "expected six segment congruence markers from payload"
    );
    assert!(scene.lines.iter().any(|line| {
        matches!(
            line.binding,
            Some(LineBinding::TranslateLine {
                vector_start_index: 0,
                vector_end_index: 3,
                ..
            })
        ) && line.points.len() == 2
            && (line.points[0].x - 298.0).abs() < 1e-6
            && (line.points[0].y - 237.0).abs() < 1e-6
            && (line.points[1].x - 467.0).abs() < 1e-6
            && (line.points[1].y - 250.0).abs() < 1e-6
    }));
    assert!(scene.lines.iter().any(|line| matches!(
        line.binding,
        Some(LineBinding::SegmentMarker {
            marker_class: 3,
            ..
        })
    )));
    let perpendicular_marker = scene
        .lines
        .iter()
        .find(|line| {
            matches!(
                line.binding,
                Some(LineBinding::SegmentMarker {
                    start_index: 0,
                    end_index: 1,
                    marker_class: 1,
                    ..
                })
            )
        })
        .expect("expected segment marker on translated base edge");
    let marker_dx = perpendicular_marker.points[1].x - perpendicular_marker.points[0].x;
    let marker_dy = perpendicular_marker.points[1].y - perpendicular_marker.points[0].y;
    let segment_dx = scene.points[1].position.x - scene.points[0].position.x;
    let segment_dy = scene.points[1].position.y - scene.points[0].position.y;
    assert!(
        (marker_dx * segment_dx + marker_dy * segment_dy).abs() < 1e-6,
        "expected segment marker to be perpendicular to its host segment"
    );
    assert!(
        scene.labels.iter().any(|label| label.text == "B'"),
        "expected translated point label B'"
    );
    assert!(
        scene.labels.iter().any(|label| label.text == "C'"),
        "expected translated point label C'"
    );
}

#[test]
fn preserves_point_label_in_point_label_gsp() {
    let scene = fixture_scene(include_bytes!(
        "../../../tests/fixtures/gsp/static/point_label.gsp"
    ));

    assert!(
        scene.labels.iter().any(|label| label.text == "A"),
        "expected point label A, got {:?}",
        scene
            .labels
            .iter()
            .map(|label| &label.text)
            .collect::<Vec<_>>()
    );
}

#[test]
fn preserves_point_and_segment_labels_in_segment_label_gsp() {
    let scene = fixture_scene(include_bytes!(
        "../../../tests/fixtures/gsp/static/segment_label.gsp"
    ));

    let texts = scene
        .labels
        .iter()
        .map(|label| label.text.as_str())
        .collect::<Vec<_>>();
    assert!(
        texts.contains(&"A"),
        "expected point label A, got {texts:?}"
    );
    assert!(
        texts.contains(&"B"),
        "expected point label B, got {texts:?}"
    );
    assert!(
        texts.contains(&"j"),
        "expected segment label j, got {texts:?}"
    );
}

#[test]
fn preserves_angle_marker_label_in_angle_marker_label_gsp() {
    let scene = fixture_scene(include_bytes!(
        "../../../tests/fixtures/gsp/static/angle_marker_label.gsp"
    ));

    let texts = scene
        .labels
        .iter()
        .map(|label| label.text.as_str())
        .collect::<Vec<_>>();
    assert!(
        texts.contains(&"42.5"),
        "expected payload-backed angle marker label, got {texts:?}"
    );
    assert!(
        scene
            .lines
            .iter()
            .any(|line| matches!(line.binding, Some(LineBinding::AngleMarker { .. }))),
        "expected angle marker to stay interactive"
    );
    assert!(scene.labels.iter().any(|label| matches!(
        label.binding,
        Some(TextLabelBinding::AngleMarkerValue {
            start_index: 1,
            vertex_index: 0,
            end_index: 2,
            decimals: 1,
        })
    )));
}

#[test]
fn preserves_visible_and_hidden_ray_labels_from_payload() {
    let scene = fixture_scene(include_bytes!(
        "../../../tests/fixtures/gsp/static/ray_label_hide.gsp"
    ));

    assert_eq!(
        scene.labels.len(),
        2,
        "expected both ray labels in the scene"
    );
    assert!(
        scene
            .labels
            .iter()
            .any(|label| label.text == "j" && label.visible),
        "expected ray label j to remain visible"
    );
    assert!(
        scene
            .labels
            .iter()
            .any(|label| label.text == "k" && !label.visible),
        "expected ray label k to remain hidden based on the 0x07d5 payload flag"
    );
    assert!(
        scene.lines.iter().all(|line| line.visible),
        "expected hidden state to apply to the label only, not the ray geometry"
    );
}

#[test]
fn keeps_control_labels_in_non_graph_sample() {
    let scene = fixture_scene(include_bytes!(
        "../../../tests/Samples/个人专栏/潘建平作品/加油潘建平老师.gsp"
    ));

    assert!(
        scene.labels.iter().any(|label| label.text.contains("单价")),
        "expected UI text label from rich text payload, got {:?}",
        scene
            .labels
            .iter()
            .map(|label| label.text.as_str())
            .collect::<Vec<_>>()
    );
}
