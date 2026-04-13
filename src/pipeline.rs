use crate::export::html::{
    render_scene_json, render_standalone_html_document, write_standalone_html,
};
use crate::gsp;
use crate::runtime::build_scene_checked;
use crate::runtime::render_payload_log;
use miette::{IntoDiagnostic, Result, WrapErr, miette};
use std::fs;
use std::path::Path;

pub fn compile_file_to_html(
    gsp_path: &Path,
    html_path: &Path,
    width: u32,
    height: u32,
) -> Result<()> {
    let data = fs::read(gsp_path)
        .into_diagnostic()
        .wrap_err_with(|| format!("failed to read {}", gsp_path.display()))?;
    match compile_bytes_to_html_file(&data, html_path, width, height) {
        Ok(()) => {
            write_payload_log(gsp_path, &data)?;
            Ok(())
        }
        Err(error) => {
            let log_path = write_payload_log(gsp_path, &data).ok().flatten();
            if let Some(log_path) = log_path {
                Err(miette!("{error}\npayload log: {}", log_path.display()))
            } else {
                Err(error)
            }
        }
    }
}

pub fn compile_file_to_scene_json(gsp_path: &Path, width: u32, height: u32) -> Result<String> {
    let data = fs::read(gsp_path)
        .into_diagnostic()
        .wrap_err_with(|| format!("failed to read {}", gsp_path.display()))?;
    compile_bytes_to_scene_json(&data, width, height)
}

pub fn compile_bytes_to_html_file(
    data: &[u8],
    html_path: &Path,
    width: u32,
    height: u32,
) -> Result<()> {
    let html = compile_bytes_to_html_document(data, width, height)?;
    write_standalone_html(html_path, &html).map_err(|error| miette!("{error}"))
}

pub fn compile_bytes_to_html_document(data: &[u8], width: u32, height: u32) -> Result<String> {
    let file = gsp::parse(data).map_err(miette::Report::new)?;
    let scene = build_scene_checked(&file)
        .map_err(|error| miette!("{error:#}"))
        .wrap_err("failed to build scene from parsed payload")?;
    let document_layout = is_document_layout(&file, &scene);
    let (width, height) = export_dimensions(&file, &scene, width, height);
    Ok(render_standalone_html_document(
        &scene,
        width,
        height,
        document_layout,
    ))
}

pub fn compile_bytes_to_scene_json(data: &[u8], width: u32, height: u32) -> Result<String> {
    let file = gsp::parse(data).map_err(miette::Report::new)?;
    let scene = build_scene_checked(&file)
        .map_err(|error| miette!("{error:#}"))
        .wrap_err("failed to build scene from parsed payload")?;
    let (width, height) = export_dimensions(&file, &scene, width, height);
    Ok(render_scene_json(&scene, width, height, true))
}

fn write_payload_log(gsp_path: &Path, data: &[u8]) -> Result<Option<std::path::PathBuf>> {
    let file = gsp::parse(data).map_err(miette::Report::new)?;
    let log_body = render_payload_log(gsp_path, &file);
    let log_path = gsp_path.with_extension("log");
    fs::write(&log_path, log_body)
        .into_diagnostic()
        .wrap_err_with(|| format!("failed to write {}", log_path.display()))?;
    Ok(Some(log_path))
}

fn export_dimensions(
    file: &crate::format::GspFile,
    scene: &crate::runtime::scene::Scene,
    fallback_width: u32,
    fallback_height: u32,
) -> (u32, u32) {
    if is_document_layout(file, scene)
        && let Some((width, height)) = file.document_canvas_size()
    {
        return (width, height);
    }
    (fallback_width, fallback_height)
}

fn is_document_layout(file: &crate::format::GspFile, scene: &crate::runtime::scene::Scene) -> bool {
    !scene.graph_mode
        && file.object_groups().iter().any(|group| {
            group
                .records
                .iter()
                .any(|record| record.record_type == 0x08fc)
        })
}

#[cfg(test)]
mod tests {
    use super::{
        compile_bytes_to_html_document, compile_bytes_to_scene_json, compile_file_to_html,
    };
    use insta::assert_snapshot;
    use serde_json::Value;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    const FIXTURE_WIDTH: u32 = 800;
    const FIXTURE_HEIGHT: u32 = 600;

    fn fixture_html(data: &[u8], message: &str) -> String {
        compile_bytes_to_html_document(data, FIXTURE_WIDTH, FIXTURE_HEIGHT).expect(message)
    }

    fn fixture_scene_json(data: &[u8], message: &str) -> String {
        compile_bytes_to_scene_json(data, FIXTURE_WIDTH, FIXTURE_HEIGHT).expect(message)
    }

    fn fixture_scene(data: &[u8], message: &str) -> Value {
        serde_json::from_str(&fixture_scene_json(data, message))
            .expect("scene json should be valid json")
    }

    fn fixture_bytes(path: &str) -> Option<Vec<u8>> {
        fs::read(path).ok()
    }

    #[test]
    fn compiles_fixture_into_standalone_html() {
        let html = fixture_html(
            include_bytes!("../tests/fixtures/gsp/static/point.gsp"),
            "fixture should compile",
        );

        assert!(html.contains("<!doctype html>"));
        assert!(html.contains("<svg id=\"view\""));
        assert!(html.contains("Generated by gsp-rs"));
        assert!(html.contains("application/json"));
        assert!(html.contains("toggle-debug"));
        assert!(html.contains("window.gspDebug"));
    }

    #[test]
    fn svg_runtime_keeps_geometry_and_grid_layers_inside_the_same_stage() {
        let html = fixture_html(
            include_bytes!("../tests/fixtures/gsp/static/point.gsp"),
            "fixture should compile",
        );

        assert!(html.contains("<g id=\"grid-layer\"></g>"));
        assert!(html.contains("<g id=\"scene-layer\"></g>"));
        assert!(html.contains("viewBox=\"0 0 800 600\""));
    }

    #[test]
    fn compiles_fixture_and_also_writes_payload_log() {
        let temp_root = unique_test_dir("payload-log-success");
        fs::create_dir_all(&temp_root).expect("temporary directory should be creatable");

        let gsp_path = temp_root.join("point.gsp");
        let html_path = temp_root.join("point.html");
        let log_path = temp_root.join("point.log");
        fs::write(
            &gsp_path,
            include_bytes!("../tests/fixtures/gsp/static/point.gsp"),
        )
        .expect("fixture gsp should be writable");

        compile_file_to_html(&gsp_path, &html_path, FIXTURE_WIDTH, FIXTURE_HEIGHT)
            .expect("fixture should compile to html");

        assert!(html_path.exists(), "expected html output to be written");
        assert!(log_path.exists(), "expected payload log to be written");

        let log = fs::read_to_string(&log_path).expect("payload log should be readable");
        assert!(log.contains("载荷说明"));
        assert!(log.contains("问题数量: 0"));
        assert!(log.contains("构造步骤"));
        assert!(log.contains("1. #1 = 自由点。"));

        let _ = fs::remove_dir_all(&temp_root);
    }

    fn unique_test_dir(prefix: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should move forward")
            .as_nanos();
        std::env::temp_dir().join(format!("gsp-rs-{prefix}-{unique}"))
    }

    #[test]
    fn exports_scene_json_for_console_debugging() {
        let scene_json = fixture_scene_json(
            include_bytes!("../tests/fixtures/gsp/static/point.gsp"),
            "fixture should compile",
        );

        assert!(scene_json.contains("\n  \"width\": 800,"));
        assert!(scene_json.contains("\"points\": ["));
    }

    #[test]
    fn snapshots_point_fixture_scene_json() {
        let scene_json = fixture_scene_json(
            include_bytes!("../tests/fixtures/gsp/static/point.gsp"),
            "fixture should compile",
        );

        assert_snapshot!("point_fixture_scene_json", scene_json);
    }

    #[test]
    fn exports_segment_intersection_fixture_into_html() {
        let html = fixture_html(
            include_bytes!("../tests/fixtures/gsp/insection/segment_insection.gsp"),
            "segment intersection fixture should compile",
        );

        assert!(html.contains("\"x\":416.3160761196899"));
        assert!(html.contains("\"y\":321.2222079835971"));
        assert!(html.contains("\"kind\":\"line-intersection\""));
    }

    #[test]
    fn exports_perpendicular_intersection_fixture_into_html() {
        let html = fixture_html(
            include_bytes!("../tests/fixtures/gsp/perp.gsp"),
            "perp fixture should compile",
        );

        assert!(html.contains("\"x\":867.3347427619169"));
        assert!(html.contains("\"y\":469.9559050197873"));
        assert!(html.contains("\"kind\":\"line-intersection\""));
        assert!(html.contains("\"right\":{\"kind\":\"perpendicular-line\",\"throughIndex\":2"));
    }

    #[test]
    fn exports_coordinate_trace_intersection_fixture_into_html() {
        let html = fixture_html(
            include_bytes!("../tests/fixtures/gsp/insection/cood_intersection.gsp"),
            "coordinate trace intersection fixture should compile",
        );

        assert!(html.contains("\"kind\":\"coordinate-trace\""));
        assert!(html.contains("\"kind\":\"coordinate-source\""));
        assert!(html.contains("\"kind\":\"line-trace-intersection\""));
        assert!(html.contains("\"parameterName\":\"t₁\""));
        assert!(html.contains("\"x\":0.0,\"y\":0.0"));
    }

    #[test]
    fn exports_coordinate_trace_intersection_y_fixture_into_html() {
        let html = fixture_html(
            include_bytes!("../tests/fixtures/gsp/insection/cood_intersection_y.gsp"),
            "coordinate trace y intersection fixture should compile",
        );

        assert!(html.contains("\"kind\":\"coordinate-trace\""));
        assert!(html.contains("\"kind\":\"coordinate-source\""));
        assert!(html.contains("\"axis\":\"horizontal\""));
        assert!(html.contains("\"kind\":\"line-trace-intersection\""));
        assert!(html.contains("\"x\":0.0,\"y\":0.0"));
    }

    #[test]
    fn exports_coordinate_trace_intersection_xy_fixture_into_html() {
        let html = fixture_html(
            include_bytes!("../tests/fixtures/gsp/insection/cood_intersection_xy.gsp"),
            "coordinate trace xy intersection fixture should compile",
        );

        assert!(html.contains("\"kind\":\"coordinate-trace\""));
        assert!(html.contains("\"kind\":\"coordinate-source-2d\""));
        assert!(html.contains("\"kind\":\"line-trace-intersection\""));
        assert!(html.contains("\"y\":3.069166666666897"));
    }

    #[test]
    fn exports_point_iteration_metadata_into_html() {
        let html = fixture_html(
            include_bytes!("../tests/fixtures/gsp/static/简单迭代/原象点初象点深度5迭代.gsp"),
            "iteration fixture should compile",
        );

        assert!(html.contains("\"pointIterations\":["));
        assert!(html.contains("\"parameterName\":\"n\""));
    }

    #[test]
    fn exports_non_graph_iteration_parameters_and_expression_bindings_into_html() {
        let html = fixture_html(
            include_bytes!(
                "../tests/fixtures/gsp/static/简单迭代/原象点和参数初象点和数值深度5迭代.gsp"
            ),
            "non-graph iteration fixture should compile",
        );

        assert!(html.contains("\"name\":\"n\""));
        assert!(html.contains("\"name\":\"a\""));
        assert!(html.contains("\"kind\":\"parameter-value\",\"name\":\"a\""));
        assert!(html.contains("\"kind\":\"point-expression-value\""));
        assert!(html.contains("\"parameterName\":\"a\""));
        assert!(html.contains("\"pointIndex\":1"));
        assert!(html.contains("\"kind\":\"expression-value\",\"parameterName\":\"a\""));
        assert!(html.contains("\"labelIterations\":["));
    }

    #[test]
    fn exports_default_depth_iteration_metadata_into_html() {
        let html = fixture_html(
            include_bytes!("../tests/fixtures/gsp/static/简单迭代/原象点初象点默认深度3迭代.gsp"),
            "default iteration fixture should compile",
        );

        assert!(html.contains("\"pointIterations\":["));
        assert!(html.contains("\"depth\":3"));
    }

    #[test]
    fn exports_default_depth_non_graph_iteration_fixture_metadata() {
        let html = fixture_html(
            include_bytes!(
                "../tests/fixtures/gsp/static/简单迭代/原象点和参数初象点和数值默认深度迭代.gsp"
            ),
            "default non-graph iteration fixture should compile",
        );

        assert!(html.contains("\"name\":\"a\""));
        assert!(html.contains("\"pointIterations\":["));
        assert!(html.contains("\"labelIterations\":["));
        assert!(html.contains("\"depth\":3"));
        assert!(!html.contains("\"depthParameterName\":\"B\""));
    }

    #[test]
    fn exports_standalone_parameter_controls_into_html() {
        let scene = fixture_scene(
            include_bytes!("../tests/fixtures/未实现的系统功能/parameter.gsp"),
            "standalone parameter fixture should compile",
        );
        let parameters = scene["parameters"]
            .as_array()
            .expect("scene parameters should be an array");
        assert_eq!(parameters.len(), 3);
        assert_eq!(parameters[0]["name"].as_str(), Some("t₁"));
        assert_eq!(parameters[0]["value"].as_f64(), Some(1.0));
        assert_eq!(parameters[0]["unit"].as_str(), Some("degree"));
        assert_eq!(parameters[1]["name"].as_str(), Some("t₂"));
        assert_eq!(parameters[1]["value"].as_f64(), Some(1.0));
        assert_eq!(parameters[1]["unit"].as_str(), Some("cm"));
        assert_eq!(parameters[2]["name"].as_str(), Some("t₃"));
        assert_eq!(parameters[2]["value"].as_f64(), Some(1.0));
        assert_eq!(parameters[2]["unit"], Value::Null);
        let labels = scene["labels"]
            .as_array()
            .expect("scene labels should be an array");
        assert_eq!(labels[0]["text"].as_str(), Some("t₁ = 1.00°"));
        assert_eq!(labels[0]["visible"].as_bool(), Some(true));
        assert_eq!(labels[1]["text"].as_str(), Some("t₂ = 1.00 cm"));
        assert_eq!(labels[1]["visible"].as_bool(), Some(true));
        assert_eq!(labels[2]["text"].as_str(), Some("t₃ = 1.00"));
        assert_eq!(labels[2]["visible"].as_bool(), Some(true));
    }

    #[test]
    fn static_fixture_uses_stub_viewer_dynamics_runtime() {
        let html = fixture_html(
            include_bytes!("../tests/fixtures/gsp/static/point.gsp"),
            "static point fixture should compile to html",
        );

        assert!(html.contains(
            "viewer-runtime: scene=basic; render=basic; overlay=stub; drag=full; dynamics=stub"
        ));
        assert!(
            !html.contains("function sampleDynamicFunction("),
            "static fixture should not embed the full dynamics runtime"
        );
        assert!(
            !html.contains("function drawCircles(env)"),
            "static point fixture should not embed the full render runtime"
        );
        assert!(
            !html.contains("function circleArcControlPoints("),
            "static point fixture should not embed the full scene runtime"
        );
    }

    #[test]
    fn parameter_fixture_uses_full_viewer_dynamics_runtime() {
        let html = fixture_html(
            include_bytes!("../tests/fixtures/未实现的系统功能/parameter.gsp"),
            "parameter fixture should compile to html",
        );

        assert!(
            html.contains("viewer-runtime: ") && html.contains("dynamics=full"),
            "parameter fixture should keep the full dynamics runtime profile"
        );
        assert!(
            html.contains("function sampleDynamicFunction("),
            "parameter fixture should keep the full dynamics runtime"
        );
    }

    #[test]
    fn hot_text_fixture_uses_full_overlay_runtime() {
        let html = fixture_html(
            include_bytes!("../tests/fixtures/gsp/热文本.gsp"),
            "hot text fixture should compile to html",
        );

        assert!(html.contains("viewer-runtime: scene="));
        assert!(html.contains("overlay=full;"));
        assert!(
            html.contains("function renderRichMarkupNodes("),
            "hot text fixture should keep the full overlay runtime"
        );
    }

    #[test]
    fn circle_arc_fixture_uses_circular_scene_runtime() {
        let html = fixture_html(
            include_bytes!("../tests/fixtures/gsp/static/arc_on_circle.gsp"),
            "arc on circle fixture should compile to html",
        );

        assert!(html.contains("viewer-runtime: scene=basic+circular; render=basic+circular;"));
        assert!(
            html.contains("function circleArcControlPoints("),
            "arc-on-circle fixture should include the circular scene addon"
        );
    }

    #[test]
    fn coordinate_trace_intersection_fixture_uses_trace_and_intersection_scene_runtime() {
        let html = fixture_html(
            include_bytes!("../tests/fixtures/gsp/insection/cood_intersection.gsp"),
            "coordinate trace intersection fixture should compile to html",
        );

        assert!(html.contains("trace+intersections"));
        assert!(
            html.contains("function sampleCoordinateTracePoints("),
            "coordinate trace intersection fixture should include the trace scene addon"
        );
        assert!(
            html.contains("function lineCircleIntersection("),
            "coordinate trace intersection fixture should include the intersections scene addon"
        );
    }

    #[test]
    fn exports_carried_polygon_iteration_metadata_into_html() {
        let html = fixture_html(
            include_bytes!(
                "../tests/fixtures/gsp/static/简单迭代/原象点初象携带多边形双映射深度4迭代.gsp"
            ),
            "carried polygon iteration fixture should compile",
        );

        assert!(html.contains("\"lineIterations\":[]"));
        assert!(html.contains("\"polygonIterations\":["));
        assert!(html.contains("\"parameterName\":\"n\""));
        assert!(html.contains("\"vertexIndices\":[0,2,1]"));
        assert!(html.contains("\"secondaryDx\":37.79527559055118"));
    }

    #[test]
    fn exports_perpendicular_line_binding_into_html() {
        let html = fixture_html(
            include_bytes!("../tests/fixtures/gsp/static/perpendicular.gsp"),
            "perpendicular fixture should compile",
        );

        assert!(html.contains("\"kind\":\"perpendicular-line\""));
        assert!(html.contains("\"throughIndex\":1"));
        assert!(html.contains("\"lineStartIndex\":0"));
        assert!(html.contains("\"lineEndIndex\":1"));
    }

    #[test]
    fn exports_parallel_line_binding_into_html() {
        let html = fixture_html(
            include_bytes!("../tests/fixtures/gsp/parallel.gsp"),
            "parallel fixture should compile",
        );

        assert!(html.contains("\"kind\":\"parallel-line\""));
        assert!(html.contains("\"throughIndex\":2"));
        assert!(html.contains("\"lineStartIndex\":0"));
        assert!(html.contains("\"lineEndIndex\":1"));
    }

    #[test]
    fn exports_angle_bisector_ray_binding_into_html() {
        let html = fixture_html(
            include_bytes!("../tests/fixtures/gsp/static/bisector.gsp"),
            "bisector fixture should compile",
        );

        assert!(html.contains("\"kind\":\"angle-bisector-ray\""));
        assert!(html.contains("\"startIndex\":0"));
        assert!(html.contains("\"vertexIndex\":1"));
        assert!(html.contains("\"endIndex\":2"));
    }

    #[test]
    fn exports_nested_perpendicular_parallel_marker_bindings_into_html() {
        let html = fixture_html(
            include_bytes!("../tests/fixtures/gsp/pert_vert.gsp"),
            "pert_vert fixture should compile",
        );

        assert!(html.contains("\"kind\":\"perpendicular-line\",\"throughIndex\":3"));
        assert!(html.contains("\"kind\":\"perpendicular-line\",\"throughIndex\":1"));
        assert!(html.contains("\"kind\":\"parallel-line\",\"throughIndex\":1"));
        assert!(html.contains("\"lineIndex\":1"));
    }

    #[test]
    fn exports_three_point_arc_into_html() {
        let html = fixture_html(
            include_bytes!("../tests/fixtures/gsp/static/three_point_arc.gsp"),
            "three-point arc fixture should compile",
        );

        assert!(html.contains("\"arcs\":["));
        assert!(html.contains("\"color\":[0,128,0,255]"));
        assert!(html.contains("\"points\":[{\"x\":323.0,\"y\":217.0}"));
    }

    #[test]
    fn exports_three_point_arc_point_constraint_into_html() {
        let html = fixture_html(
            include_bytes!("../tests/fixtures/gsp/static/three_point_arc_point.gsp"),
            "three-point arc point fixture should compile",
        );

        assert!(html.contains("\"kind\":\"arc\""));
        assert!(html.contains("\"startIndex\":0"));
        assert!(html.contains("\"midIndex\":1"));
        assert!(html.contains("\"endIndex\":2"));
    }

    #[test]
    fn exports_arc_on_circle_into_html() {
        let html = fixture_html(
            include_bytes!("../tests/fixtures/gsp/static/arc_on_circle.gsp"),
            "arc-on-circle fixture should compile",
        );

        assert!(html.contains("\"arcs\":["));
        assert!(html.contains("\"dashed\":true"));
        assert!(html.contains("\"counterclockwise\":true"));
        assert!(html.contains("\"points\":[{\"x\":411.18946322164174"));
    }

    #[test]
    fn exports_point_on_circle_arc_constraint_into_html() {
        let html = fixture_html(
            include_bytes!("../tests/fixtures/gsp/point_on_arc1.gsp"),
            "point-on-circle-arc fixture should compile",
        );

        assert!(html.contains("\"kind\":\"circle-arc\""));
        assert!(html.contains("\"centerIndex\":0"));
        assert!(html.contains("\"startIndex\":2"));
        assert!(html.contains("\"endIndex\":3"));
    }

    #[test]
    fn exports_parameter_controlled_arc_on_circle_into_html() {
        let html = fixture_html(
            include_bytes!("../tests/fixtures/gsp/static/value_point_arc_on_circle.gsp"),
            "parameter-controlled arc-on-circle fixture should compile",
        );

        assert!(html.contains("\"arcs\":["));
        assert!(html.contains("\"counterclockwise\":true"));
        assert!(html.contains("\"name\":\"t₁\""));
        assert!(html.contains("\"name\":\"t₂\""));
    }

    #[test]
    fn exports_three_point_arc_intersection_into_html() {
        let html = fixture_html(
            include_bytes!("../tests/fixtures/gsp/static/three_point_arc_intersection.gsp"),
            "three-point arc intersection fixture should compile",
        );

        assert!(html.contains("\"kind\":\"circular-intersection\""));
        assert!(html.contains("\"left\":{\"kind\":\"three-point-arc\""));
        assert!(html.contains("\"right\":{\"kind\":\"three-point-arc\""));
    }

    #[test]
    fn exports_circle_center_radius_into_html() {
        let html = fixture_html(
            include_bytes!("../tests/fixtures/gsp/circle_center_radius.gsp"),
            "circle-center-radius fixture should compile",
        );

        assert!(html.contains("\"circles\":[{\"center\":{\"x\":348.0,\"y\":177.0}"));
        assert!(html.contains("\"kind\":\"segment-radius-circle\""));
        assert!(html.contains(
            "\"lines\":[{\"points\":[{\"x\":318.0,\"y\":391.0},{\"x\":403.0,\"y\":390.0}]"
        ));
    }

    #[test]
    fn exports_circle_inner_fill_into_html() {
        let Some(data) = fixture_bytes("tests/fixtures/gsp/static/circle_inner.gsp") else {
            return;
        };
        let html = fixture_html(&data, "circle-inner fixture should compile");

        assert!(html.contains("\"circles\":["));
        assert!(html.contains("\"fillColor\":[255,255,0,127]"));
        assert!(html.contains("\"kind\":\"point-radius-circle\""));
    }

    #[test]
    fn exports_multiline_text_into_html() {
        let html = fixture_html(
            include_bytes!("../tests/fixtures/gsp/多行文本.gsp"),
            "multiline text fixture should compile",
        );

        assert!(html.contains(
            "\"text\":\"线段中垂线\\n垂线\\n平行线\\n直角三角形\\n点的轨迹\\n圆上的弧\\n过三点的弧\""
        ));
    }

    #[test]
    fn exports_hidden_point_fixture_into_html() {
        let html = fixture_html(
            include_bytes!("../tests/fixtures/gsp/static/point_hidden.gsp"),
            "hidden-point fixture should compile",
        );

        assert!(html.contains(
            "\"points\":[{\"x\":323.0,\"y\":217.0,\"color\":[255,0,0,255],\"visible\":false"
        ));
        assert!(html.contains("\"lines\":[]"));
    }

    #[test]
    fn exports_hidden_ray_fixture_into_html() {
        let scene = fixture_scene(
            include_bytes!("../tests/fixtures/gsp/static/hide_ray.gsp"),
            "hidden-ray fixture should compile",
        );
        let lines = scene["lines"]
            .as_array()
            .expect("scene lines should be an array");

        assert_eq!(lines.len(), 2, "expected two rays in the exported scene");
        assert!(
            lines
                .iter()
                .any(|line| line["visible"].as_bool() == Some(false)),
            "expected one exported ray to stay hidden from the source payload"
        );
        assert!(
            lines
                .iter()
                .any(|line| line["visible"].as_bool() == Some(true)),
            "expected one exported ray to stay visible"
        );
        assert!(
            lines
                .iter()
                .all(|line| line["binding"]["kind"].as_str() == Some("ray")),
            "expected both exported line bindings to remain rays"
        );
    }

    #[test]
    fn exports_angle_marker_label_fixture_into_html() {
        let scene = fixture_scene(
            include_bytes!("../tests/fixtures/gsp/static/angle_marker_label.gsp"),
            "angle-marker-label fixture should compile",
        );
        let labels = scene["labels"]
            .as_array()
            .expect("scene labels should be an array");
        assert!(
            labels
                .iter()
                .any(|label| label["text"].as_str() == Some("42.5")),
            "expected exported labels to include the payload angle marker label"
        );
        assert!(
            scene["lines"].as_array().is_some_and(|lines| lines
                .iter()
                .any(|line| line["binding"]["kind"].as_str() == Some("angle-marker"))),
            "expected exported angle marker to stay interactive"
        );
        assert!(labels.iter().any(|label| {
            label["binding"]["kind"].as_str() == Some("angle-marker-value")
                && label["binding"]["startIndex"].as_u64() == Some(1)
                && label["binding"]["vertexIndex"].as_u64() == Some(0)
                && label["binding"]["endIndex"].as_u64() == Some(2)
                && label["binding"]["decimals"].as_u64() == Some(1)
        }));
    }

    #[test]
    fn exports_ray_label_hide_fixture_into_html() {
        let scene = fixture_scene(
            include_bytes!("../tests/fixtures/gsp/static/ray_label_hide.gsp"),
            "ray-label-hide fixture should compile",
        );
        let labels = scene["labels"]
            .as_array()
            .expect("scene labels should be an array");
        assert_eq!(
            labels.len(),
            2,
            "expected both payload ray labels to export"
        );
        assert!(labels.iter().any(|label| {
            label["text"].as_str() == Some("j") && label["visible"].as_bool() == Some(true)
        }));
        assert!(labels.iter().any(|label| {
            label["text"].as_str() == Some("k") && label["visible"].as_bool() == Some(false)
        }));
    }

    #[test]
    fn html_viewer_preserves_label_visibility_flags() {
        let html = fixture_html(
            include_bytes!("../tests/fixtures/gsp/static/ray_label_hide.gsp"),
            "ray-label-hide fixture should compile to html",
        );

        assert!(
            html.contains("\"text\":\"k\"") && html.contains("\"visible\":false"),
            "expected scene JSON embedded in html to preserve the hidden ray label"
        );
        assert!(
            html.contains("visible: label.visible !== false"),
            "expected bundled viewer runtime to hydrate label visibility from the source scene"
        );
    }

    #[test]
    fn exports_polar_function_fixture_into_html() {
        let html = fixture_html(
            include_bytes!("../tests/fixtures/未实现的系统功能/极坐标.gsp"),
            "polar fixture should compile",
        );

        assert!(html.contains("\"plotMode\":\"polar\""));
        assert!(html.contains("\"text\":\"r = 1 + cos(θ)\""));
        assert!(html.contains("\"name\":\"g\""));
        assert!(html.contains("\"x\":-0.24999414519673077"));
    }

    #[test]
    fn exports_parameterized_function_fixture_with_unique_parameters() {
        let scene = fixture_scene(
            include_bytes!("../tests/fixtures/未实现的系统功能/函数.gsp"),
            "parameterized function fixture should compile",
        );
        assert_eq!(scene["piMode"].as_bool(), Some(false));
        assert_eq!(scene["savedViewport"].as_bool(), Some(true));
        let parameters = scene["parameters"]
            .as_array()
            .expect("scene parameters should be an array");
        let parameter_names = parameters
            .iter()
            .map(|parameter| {
                parameter["name"]
                    .as_str()
                    .expect("parameter name should be a string")
            })
            .collect::<Vec<_>>();
        assert_eq!(parameter_names, vec!["a", "b", "c"]);
        assert!(
            parameters
                .iter()
                .all(|parameter| parameter["labelIndex"].as_u64().is_some()),
            "graph parameters should keep label bindings for interactive updates"
        );

        let functions = scene["functions"]
            .as_array()
            .expect("scene functions should be an array");
        assert_eq!(functions.len(), 1);
        assert_eq!(functions[0]["name"].as_str(), Some("f"));
        assert_eq!(functions[0]["lineIndex"].as_u64(), Some(3));
        assert_eq!(
            scene["labels"][3]["text"].as_str(),
            Some("f(x) = a*x^2 + b*x + c")
        );
        assert_eq!(
            functions[0]["expr"]["expr"]["kind"].as_str(),
            Some("binary")
        );
        assert_eq!(functions[0]["expr"]["expr"]["op"].as_str(), Some("add"));
        assert_eq!(
            functions[0]["expr"]["expr"]["lhs"]["kind"].as_str(),
            Some("binary")
        );
        assert_eq!(
            functions[0]["expr"]["expr"]["lhs"]["op"].as_str(),
            Some("add")
        );
    }

    #[test]
    fn exports_draw_function_fixture_with_payload_linked_labels() {
        let scene = fixture_scene(
            include_bytes!("../tests/fixtures/未实现的系统功能/绘图函数.gsp"),
            "draw function fixture should compile",
        );
        let images = scene["images"]
            .as_array()
            .expect("scene images should be an array");
        assert_eq!(images.len(), 1);
        assert_eq!(images[0]["screenSpace"].as_bool(), Some(true));
        assert!(
            images[0]["src"]
                .as_str()
                .is_some_and(|src| src.starts_with("data:image/png;base64,")),
            "expected embedded png data url"
        );
        assert_eq!(images[0]["topLeft"]["x"].as_f64(), Some(95.0));
        assert_eq!(images[0]["topLeft"]["y"].as_f64(), Some(198.0));
        assert_eq!(images[0]["bottomRight"]["x"].as_f64(), Some(536.0));
        assert_eq!(images[0]["bottomRight"]["y"].as_f64(), Some(273.0));
    }

    #[test]
    fn exports_insert_image_fixture() {
        let scene = fixture_scene(
            include_bytes!("../tests/fixtures/未实现的系统功能/插入图片.gsp"),
            "insert image fixture should compile",
        );
        let images = scene["images"]
            .as_array()
            .expect("scene images should be an array");
        assert_eq!(images.len(), 1);
        assert_eq!(images[0]["screenSpace"].as_bool(), Some(true));
        assert!(
            images[0]["src"]
                .as_str()
                .is_some_and(|src| src.starts_with("data:image/png;base64,")),
            "expected embedded png data url"
        );
        assert_eq!(images[0]["topLeft"]["x"].as_f64(), Some(118.0));
        assert_eq!(images[0]["topLeft"]["y"].as_f64(), Some(112.0));
        assert_eq!(images[0]["bottomRight"]["x"].as_f64(), Some(373.0));
        assert_eq!(images[0]["bottomRight"]["y"].as_f64(), Some(270.0));
    }

    #[test]
    fn exports_translated_triangle_segments_into_html() {
        let html = fixture_html(
            include_bytes!("../tests/fixtures/gsp/两个三角形标记全等.gsp"),
            "congruent triangle fixture should compile",
        );

        assert!(html.contains("\"kind\":\"translate-line\""));
        assert!(html.contains("\"kind\":\"angle-marker\""));
        assert!(html.contains("\"kind\":\"segment-marker\""));
        assert!(html.contains("\"vectorStartIndex\":0,\"vectorEndIndex\":3"));
        assert!(html.contains("\"text\":\"B'\""));
        assert!(html.contains("\"text\":\"C'\""));
    }

    #[test]
    fn exports_circular_segment_boundary_fixture_with_polyline_constraint() {
        let scene = fixture_scene(
            include_bytes!("../tests/fixtures/未实现的系统功能/弓形周界动点.gsp"),
            "circular segment boundary fixture should compile",
        );
        assert!(
            scene["points"].as_array().is_some_and(|points| points
                .iter()
                .any(|point| point["constraint"]["kind"].as_str() == Some("polyline"))),
            "expected a live point constrained to the boundary perimeter"
        );
        assert!(
            scene["polygons"].as_array().is_some_and(|polygons| {
                polygons.iter().any(|polygon| {
                    polygon["binding"]["kind"].as_str() == Some("arc-boundary-polygon")
                })
            }),
            "expected the circular segment fill to export as a live boundary polygon"
        );
    }

    #[test]
    fn exports_custom_transform_fixture_with_interactive_point_binding() {
        let html = fixture_html(
            include_bytes!("../tests/fixtures/未实现的系统功能/自定义变换.gsp"),
            "custom transform fixture should compile",
        );

        assert!(html.contains("\"text\":\"Q\""));
        assert!(html.contains("\"kind\":\"custom-transform\""));
        assert!(html.contains("\"sourceIndex\":2"));
        assert!(html.contains("\"name\":\"P\""));
        assert!(html.contains("1厘米"));
        assert!(html.contains("100°"));
    }

    #[test]
    fn exports_circle_formation_fixture_with_rotation_iteration() {
        let scene = fixture_scene(
            include_bytes!("../tests/fixtures/未实现的系统功能/圆的形成.gsp"),
            "circle-formation fixture should compile",
        );
        let parameters = scene["parameters"]
            .as_array()
            .expect("scene parameters should be an array");
        assert_eq!(parameters.len(), 1, "expected a single live t₂ parameter");
        assert_eq!(parameters[0]["name"].as_str(), Some("t₂"));
        assert_eq!(parameters[0]["value"].as_f64(), Some(5.0));

        let lines = scene["lines"]
            .as_array()
            .expect("scene lines should be an array");
        assert!(
            lines
                .iter()
                .any(|line| line["binding"]["kind"].as_str() == Some("rotate-edge")),
            "expected regular-polygon iteration edges to stay interactive"
        );
        assert_eq!(
            lines
                .iter()
                .filter(|line| line["binding"]["kind"].as_str() == Some("rotate-edge"))
                .count(),
            5,
            "expected five interactive polygon edges for the default pentagon"
        );
        let line_iterations = scene["lineIterations"]
            .as_array()
            .expect("scene line iterations should be an array");
        assert_eq!(
            line_iterations
                .iter()
                .filter(|family| family["kind"].as_str() == Some("rotate"))
                .count(),
            1,
            "expected one canonical serialized rotate family"
        );
        assert!(
            line_iterations.iter().any(|family| {
                family["kind"].as_str() == Some("rotate")
                    && family["parameterName"].as_str() == Some("t₂")
                    && family["depth"].as_u64() == Some(5)
            }),
            "expected the regular-polygon segment iteration family to be serialized into html payload"
        );
        let iteration_tables = scene["iterationTables"]
            .as_array()
            .expect("scene iteration tables should be an array");
        assert_eq!(
            iteration_tables.len(),
            1,
            "expected one visible iteration table"
        );
        assert_eq!(iteration_tables[0]["exprLabel"].as_str(), Some("t₁ + 1"));
        assert_eq!(iteration_tables[0]["parameterName"].as_str(), Some("t₁"));
        assert_eq!(
            iteration_tables[0]["depthParameterName"].as_str(),
            Some("t₂")
        );
        assert_eq!(iteration_tables[0]["x"].as_f64(), Some(322.0));
        assert_eq!(iteration_tables[0]["y"].as_f64(), Some(481.0));
        assert_eq!(iteration_tables[0]["depth"].as_u64(), Some(4));
    }

    #[test]
    fn exports_circle_formation_fixture_iteration_table_against_sequence_value() {
        let scene = fixture_scene(
            include_bytes!("../tests/fixtures/圆的形成.gsp"),
            "circle-formation fixture should compile",
        );
        let iteration_tables = scene["iterationTables"]
            .as_array()
            .expect("scene iteration tables should be an array");
        assert_eq!(
            iteration_tables.len(),
            1,
            "expected one visible iteration table"
        );
        assert_eq!(iteration_tables[0]["exprLabel"].as_str(), Some("t₁ + 1"));
        assert_eq!(
            iteration_tables[0]["parameterName"].as_str(),
            Some("t₁"),
            "expected the iteration table to track the sequence value instead of the root control parameter"
        );
        assert_eq!(
            iteration_tables[0]["depthParameterName"].as_str(),
            Some("t₂"),
            "expected the iteration depth to remain controlled by the editable polygon-side parameter"
        );
    }

    #[test]
    fn exports_circle_system_fixture_with_live_parameter_and_bindings() {
        let scene = fixture_scene(
            include_bytes!("../tests/fixtures/未实现/圆系(inRm).gsp"),
            "circle-system fixture should compile",
        );
        let parameters = scene["parameters"]
            .as_array()
            .expect("scene parameters should be an array");
        assert_eq!(parameters.len(), 1, "expected one live n parameter");
        assert_eq!(parameters[0]["name"].as_str(), Some("n"));
        assert_eq!(parameters[0]["value"].as_f64(), Some(2.0));

        let lines = scene["lines"]
            .as_array()
            .expect("scene lines should be an array");
        assert!(
            lines
                .iter()
                .any(|line| line["binding"]["kind"].as_str() == Some("segment")),
            "expected the payload segment to stay interactive"
        );
        assert!(
            lines
                .iter()
                .any(|line| line["binding"]["kind"].as_str() == Some("ray")),
            "expected the payload ray to stay interactive"
        );

        let points = scene["points"]
            .as_array()
            .expect("scene points should be an array");
        assert!(
            points
                .iter()
                .any(|point| point["binding"]["kind"].as_str() == Some("rotate")),
            "expected the rotated payload point to keep its live binding"
        );
        assert!(
            points
                .iter()
                .any(|point| point["binding"]["kind"].as_str() == Some("scale")),
            "expected the scaled payload point to keep its live binding"
        );

        let labels = scene["labels"]
            .as_array()
            .expect("scene labels should be an array");
        assert!(labels.iter().any(|label| {
            label["binding"]["kind"].as_str() == Some("parameter-value")
                && label["text"].as_str() == Some("n = 2.00")
        }));
        assert!(labels.iter().any(|label| {
            label["binding"]["kind"].as_str() == Some("polygon-boundary-parameter")
                && label["text"].as_str() == Some("m = 0.95")
        }));
        assert!(labels.iter().any(|label| {
            label["binding"]["kind"].as_str() == Some("polygon-boundary-expression")
                && label["text"].as_str() == Some("1 / n = 0.50")
        }));
        let circles = scene["circles"]
            .as_array()
            .expect("scene circles should be an array");
        assert_eq!(
            circles.len(),
            21,
            "expected source plus iterated payload circles"
        );
        assert!(
            circles
                .iter()
                .any(|circle| circle["binding"]["kind"].as_str() == Some("segment-radius-circle")),
            "expected the payload circle to keep its live center/radius-segment binding"
        );
        let circle_iterations = scene["circleIterations"]
            .as_array()
            .expect("scene circle iterations should be an array");
        assert_eq!(
            circle_iterations.len(),
            1,
            "expected one live circle iteration family"
        );
        assert_eq!(circle_iterations[0]["depth"].as_u64(), Some(20));
        let polygons = scene["polygons"]
            .as_array()
            .expect("scene polygons should be an array");
        assert!(
            polygons
                .iter()
                .any(|polygon| polygon["binding"]["kind"].as_str() == Some("point-polygon")),
            "expected the payload polygon to stay interactive"
        );
        assert!(
            scene["points"]
                .as_array()
                .expect("scene points should be an array")
                .iter()
                .any(|point| point["constraint"]["kind"].as_str() == Some("polygon-boundary")),
            "expected the payload boundary point to remain live"
        );
    }

    #[test]
    fn exports_two_circle_intersection_inrm_fixture_with_live_bindings() {
        let scene = fixture_scene(
            include_bytes!("../tests/fixtures/未实现/(inRm)两圆之交.gsp"),
            "two-circle-intersection fixture should compile",
        );
        let circles = scene["circles"]
            .as_array()
            .expect("scene circles should be an array");
        assert_eq!(circles.len(), 4, "expected four payload circles");
        assert!(
            circles
                .iter()
                .all(|circle| circle["binding"]["kind"].as_str() == Some("point-radius-circle")),
            "expected every exported circle to keep its live point-radius binding"
        );
        assert!(
            circles.iter().all(|circle| circle["fillColor"].is_null()),
            "expected helper duplicate circles to avoid exporting full-disk fills"
        );

        let polygons = scene["polygons"]
            .as_array()
            .expect("scene polygons should be an array");
        assert_eq!(
            polygons.len(),
            2,
            "expected the lens to export as two circular segments"
        );
        assert!(
            polygons
                .iter()
                .all(|polygon| polygon["binding"]["kind"].as_str() == Some("arc-boundary-polygon")),
            "expected both filled polygons to stay bound to their source circular segments"
        );

        let lines = scene["lines"]
            .as_array()
            .expect("scene lines should be an array");
        assert_eq!(
            lines
                .iter()
                .filter(|line| line["binding"]["kind"].as_str() == Some("segment"))
                .count(),
            2,
            "expected both exported segments to remain interactive"
        );
        assert_eq!(
            lines
                .iter()
                .filter(|line| line["binding"]["kind"].as_str() == Some("perpendicular-line"))
                .count(),
            2,
            "expected both perpendicular helpers to remain interactive"
        );
        assert_eq!(
            lines
                .iter()
                .filter(|line| line["binding"]["kind"].as_str() == Some("line"))
                .count(),
            1,
            "expected the baseline to remain interactive"
        );

        let points = scene["points"]
            .as_array()
            .expect("scene points should be an array");
        let circle_circle_points = points
            .iter()
            .filter(|point| {
                point["constraint"]["kind"].as_str() == Some("circle-circle-intersection")
            })
            .collect::<Vec<_>>();
        assert_eq!(
            circle_circle_points.len(),
            2,
            "expected both circle-circle intersection variants"
        );
        assert!(circle_circle_points.iter().all(|point| {
            point["x"]
                .as_f64()
                .is_some_and(|x| (x - 327.0).abs() < 1e-6)
                && point["y"]
                    .as_f64()
                    .is_some_and(|y| (y - 275.0).abs() < 1e-6)
        }));
        assert_eq!(
            points
                .iter()
                .filter(|point| point["constraint"]["kind"].as_str()
                    == Some("line-circle-intersection"))
                .count(),
            8,
            "expected all derived line-circle intersections to stay live"
        );
    }

    #[test]
    fn exports_cans_in_container_inrm_fixture_with_live_bindings() {
        let Some(data) = fixture_bytes("tests/fixtures/未实现/(inRm)容器中的罐头.gsp")
        else {
            return;
        };
        let scene = fixture_scene(&data, "cans-in-container fixture should compile");
        let circles = scene["circles"]
            .as_array()
            .expect("scene circles should be an array");
        assert_eq!(circles.len(), 38, "expected payload circles to export");
        assert!(
            circles
                .iter()
                .all(|circle| circle["binding"]["kind"].as_str() == Some("segment-radius-circle")),
            "expected every exported circle to keep its live segment-radius binding"
        );
        assert_eq!(
            circles
                .iter()
                .filter(|circle| circle["visible"] == true)
                .count(),
            24,
            "expected the visible can circles to remain rendered"
        );

        let points = scene["points"]
            .as_array()
            .expect("scene points should be an array");
        assert_eq!(points.len(), 40, "expected helper points to stay exported");
        assert_eq!(
            points
                .iter()
                .filter(|point| point["visible"] == true)
                .count(),
            3,
            "expected payload draggable points to stay visible"
        );
        assert_eq!(
            points
                .iter()
                .filter(|point| point["constraint"]["kind"].as_str() == Some("segment"))
                .count(),
            2,
            "expected both slider points to remain segment constrained"
        );
        assert_eq!(
            points
                .iter()
                .filter(|point| point["constraint"]["kind"].as_str() == Some("offset"))
                .count(),
            1,
            "expected the offset helper point to stay live"
        );
        assert_eq!(
            points
                .iter()
                .filter(|point| point["binding"]["kind"].as_str() == Some("scale"))
                .count(),
            4,
            "expected scale-derived helper points to preserve their bindings"
        );
        assert_eq!(
            points
                .iter()
                .filter(|point| point["binding"]["kind"].as_str() == Some("rotate"))
                .count(),
            1,
            "expected the rotated helper point to preserve its binding"
        );
        assert_eq!(
            points
                .iter()
                .filter(|point| point["binding"]["kind"].as_str() == Some("translate"))
                .count(),
            5,
            "expected translated helper points to preserve their bindings"
        );
        assert!(
            scene["labels"]
                .as_array()
                .expect("scene labels should be an array")
                .iter()
                .any(|label| label["visible"] == true && label["text"].as_str() == Some("M")),
            "expected the payload midpoint label to stay visible"
        );
    }

    #[test]
    fn exports_ant_fixture_with_two_axis_line_iterations() {
        let scene = fixture_scene(
            include_bytes!("../tests/fixtures/bug/迭代方法2(蚂蚁).gsp"),
            "ant fixture should compile",
        );
        let line_iterations = scene["lineIterations"]
            .as_array()
            .expect("scene line iterations should be an array");
        assert_eq!(
            line_iterations.len(),
            4,
            "expected the four seed edges to iterate"
        );
        assert_eq!(
            line_iterations
                .iter()
                .filter(|family| family["kind"].as_str() == Some("translate"))
                .count(),
            3,
            "expected three translational seed-edge families"
        );
        assert!(
            line_iterations
                .iter()
                .filter(|family| family["kind"].as_str() == Some("translate"))
                .all(|family| {
                    family["parameterName"].as_str() == Some("n")
                        && family["dx"].as_f64() == Some(-62.0)
                        && family["dy"].as_f64() == Some(-36.0)
                        && family["secondaryDx"].as_f64() == Some(47.0)
                        && family["secondaryDy"].as_f64() == Some(-52.0)
                        && family["bidirectional"].as_bool() == Some(true)
                        && family["depth"].as_u64() == Some(3)
                })
        );
        assert!(line_iterations.iter().any(|family| {
            family["kind"].as_str() == Some("branching")
                && family["parameterName"].as_str() == Some("n")
                && family["depth"].as_u64() == Some(3)
                && family["targetSegments"]
                    .as_array()
                    .is_some_and(|segments| segments.len() == 2)
        }));
        let points = scene["points"]
            .as_array()
            .expect("scene points should be an array");
        assert!(
            points
                .iter()
                .filter(|point| point["visible"].as_bool() == Some(true))
                .all(|point| {
                    point["binding"]["kind"].as_str() == Some("parameter")
                        && point["constraint"].is_null()
                }),
            "expected only standalone payload parameter controls to remain visible when helper point markers are omitted"
        );
    }

    #[test]
    fn exports_crescent_trace_inrm_fixture_with_live_trace_bindings() {
        let Some(data) = fixture_bytes("tests/fixtures/未实现/月牙形轨迹(inRm).gsp") else {
            return;
        };
        let scene = fixture_scene(&data, "crescent-trace fixture should compile");

        let points = scene["points"]
            .as_array()
            .expect("scene points should be an array");
        assert!(
            points
                .iter()
                .any(|point| point["binding"]["kind"].as_str() == Some("scale-by-ratio")),
            "expected the ratio-scaled point to keep a live binding"
        );

        let lines = scene["lines"]
            .as_array()
            .expect("scene lines should be an array");
        assert!(
            lines
                .iter()
                .any(|line| line["binding"]["kind"].as_str() == Some("point-trace")),
            "expected the payload trace to keep a live trace binding"
        );
        assert!(
            lines
                .iter()
                .any(|line| line["binding"]["kind"].as_str() == Some("segment")),
            "expected the payload segment to remain interactive"
        );
    }

    #[test]
    fn exports_changing_polyline_lyg_fixture_with_live_ray_and_iterations() {
        let Some(data) = fixture_bytes("tests/Samples/个人专栏/李有贵作品/变化的折线（lyg).gsp")
        else {
            return;
        };
        let scene = fixture_scene(&data, "changing polyline fixture should compile");

        let points = scene["points"]
            .as_array()
            .expect("scene points should be an array");
        assert!(
            points
                .iter()
                .any(|point| point["constraint"]["kind"].as_str() == Some("ray")),
            "expected the payload draggable anchor to stay constrained to its source ray"
        );
        assert!(
            points.iter().any(|point| {
                point["binding"]["kind"].as_str() == Some("derived-parameter-expr")
                    || point["binding"]["kind"].as_str() == Some("constraint-parameter-expr")
                    || point["binding"]["kind"].as_str()
                        == Some("constraint-parameter-from-point-expr")
            }),
            "expected the payload parameter-controlled helper point to stay live"
        );

        let labels = scene["labels"]
            .as_array()
            .expect("scene labels should be an array");
        assert!(
            labels
                .iter()
                .any(|label| label["binding"]["kind"].as_str() == Some("segment-parameter")),
            "expected the payload ray anchor label to export as a live parameter label"
        );
        assert!(
            labels.iter().any(|label| {
                label["binding"]["kind"].as_str() == Some("expression-value")
                    && label["text"].as_str() == Some("P - trunc(P) = 0.02")
                    && label["richMarkup"].as_str().is_some()
            }),
            "expected the payload fractional expression to stay live beside the iterated geometry"
        );
        assert!(
            labels.iter().any(|label| {
                label["binding"]["kind"].as_str() == Some("expression-value")
                    && label["text"]
                        .as_str()
                        .is_some_and(|text| text.ends_with("= 未定义"))
            }),
            "expected the payload undefined distance expression to remain visible"
        );

        let lines = scene["lines"]
            .as_array()
            .expect("scene lines should be an array");
        assert!(
            lines
                .iter()
                .any(|line| line["binding"]["kind"].as_str() == Some("ray")),
            "expected the payload source ray to remain interactive"
        );

        let line_iterations = scene["lineIterations"]
            .as_array()
            .expect("scene line iterations should be an array");
        assert_eq!(
            line_iterations.len(),
            2,
            "expected both payload seed segments to export as carried line families"
        );
        assert!(line_iterations.iter().all(|family| {
            family["kind"].as_str() == Some("translate")
                && family["depth"].as_u64() == Some(8)
                && family["parameterName"].is_null()
        }));
    }

    #[test]
    fn exports_non_iterated_changing_polyline_lyg1_fixture_calculation_labels() {
        let Some(data) = fixture_bytes("tests/Samples/个人专栏/李有贵作品/变化的折线（lyg)1.gsp")
        else {
            return;
        };
        let scene = fixture_scene(&data, "changing polyline calc fixture should compile");

        let labels = scene["labels"]
            .as_array()
            .expect("scene labels should be an array");
        assert!(
            labels.iter().any(|label| {
                label["binding"]["kind"].as_str() == Some("expression-value")
                    && label["text"].as_str() == Some("m₁ - trunc(m₁) = 0.61")
                    && label["richMarkup"].as_str().is_some()
            }),
            "expected the payload fractional-part expression label to export"
        );
        assert!(
            labels.iter().any(|label| {
                label["binding"]["kind"].as_str() == Some("expression-value")
                    && label["text"]
                        .as_str()
                        .is_some_and(|text| text.ends_with("= 未定义"))
            }),
            "expected the payload undefined distance expression label to export"
        );
    }

    #[test]
    fn exports_chessboard_yougui_fixture_with_live_segment_parameter_binding() {
        let Some(data) = fixture_bytes("tests/Samples/个人专栏/李有贵作品/棋盘（有贵）.gsp")
        else {
            return;
        };
        let scene = fixture_scene(&data, "chessboard yougui fixture should compile");

        let points = scene["points"]
            .as_array()
            .expect("scene points should be an array");
        assert!(
            points
                .iter()
                .any(|point| point["constraint"]["kind"].as_str() == Some("ray")),
            "expected the payload draggable anchors to stay constrained to their source rays"
        );
        assert!(
            points.iter().any(|point| {
                point["binding"]["kind"].as_str() == Some("constraint-parameter-expr")
                    || point["binding"]["kind"].as_str()
                        == Some("constraint-parameter-from-point-expr")
            }),
            "expected the payload seed-square controls to stay bound to their source expressions"
        );

        let lines = scene["lines"]
            .as_array()
            .expect("scene lines should be an array");
        assert!(
            lines
                .iter()
                .any(|line| line["binding"]["kind"].as_str() == Some("segment")),
            "expected the payload board edges to remain interactive segments"
        );

        let labels = scene["labels"]
            .as_array()
            .expect("scene labels should be an array");
        assert!(
            labels
                .iter()
                .any(|label| label["binding"]["kind"].as_str() == Some("segment-parameter")),
            "expected the measured segment helper to export as a live parameter label"
        );
        assert!(
            labels.iter().any(|label| {
                label["binding"]["kind"].as_str() == Some("expression-value")
                    && label["binding"]["resultName"].as_str() == Some("n")
            }),
            "expected the named payload expression label to expose a derived runtime parameter"
        );

        let polygons = scene["polygons"]
            .as_array()
            .expect("scene polygons should be an array");
        assert!(
            polygons
                .iter()
                .any(|polygon| polygon["binding"]["kind"].as_str() == Some("point-polygon")),
            "expected the payload polygon to keep its live point binding"
        );
        assert_eq!(
            polygons.len(),
            41,
            "expected the payload chessboard to export the seed plus 40 iterated dark squares"
        );
        let mut row_counts = std::collections::BTreeMap::<i64, usize>::new();
        for polygon in polygons {
            let y = polygon["points"][0]["y"]
                .as_f64()
                .expect("polygon point y should be numeric");
            let bucket = (y * 1000.0).round() as i64;
            *row_counts.entry(bucket).or_default() += 1;
        }
        let counts = row_counts.into_values().collect::<Vec<_>>();
        assert!(
            counts.starts_with(&[5, 4, 5, 4]),
            "expected the payload chessboard rows to alternate 5/4 dark cells, got {counts:?}"
        );
        let polygon_iterations = scene["polygonIterations"]
            .as_array()
            .expect("scene polygon iterations should be an array");
        assert!(
            polygon_iterations.iter().any(|family| {
                family["kind"].as_str() == Some("coordinate-grid")
                    && family["parameterName"].as_str() == Some("t₁")
            }),
            "expected the payload chessboard copies to rebuild from a live coordinate-grid family"
        );
    }

    #[test]
    fn exports_three_parameter_color_fixture_with_live_fill_bindings() {
        let Some(data) =
            fixture_bytes("tests/Samples/个人专栏/侯仰顺作品/三个参数控制颜色(蚂蚁).gsp")
        else {
            return;
        };
        let scene = fixture_scene(&data, "three-parameter color fixture should compile");

        let circles = scene["circles"]
            .as_array()
            .expect("scene circles should be an array");
        assert_eq!(circles.len(), 2, "expected both payload circles");
        assert_eq!(
            circles[0]["fillColorBinding"]["kind"].as_str(),
            Some("rgb"),
            "expected the first circle interior to keep its RGB payload binding"
        );
        assert_eq!(
            circles[0]["fillColorBinding"]["redPointIndex"].as_u64(),
            Some(4)
        );
        assert_eq!(
            circles[0]["fillColorBinding"]["greenPointIndex"].as_u64(),
            Some(5)
        );
        assert_eq!(
            circles[0]["fillColorBinding"]["bluePointIndex"].as_u64(),
            Some(6)
        );
        assert_eq!(
            circles[1]["fillColorBinding"]["kind"].as_str(),
            Some("hsb"),
            "expected the second circle interior to keep its HSB payload binding"
        );
        assert_eq!(
            circles[1]["fillColorBinding"]["huePointIndex"].as_u64(),
            Some(11)
        );
        assert_eq!(
            circles[1]["fillColorBinding"]["saturationPointIndex"].as_u64(),
            Some(12)
        );
        assert_eq!(
            circles[1]["fillColorBinding"]["brightnessPointIndex"].as_u64(),
            Some(13)
        );

        let labels = scene["labels"]
            .as_array()
            .expect("scene labels should be an array");
        let label_color = |text: &str| {
            labels
                .iter()
                .find(|label| label["text"].as_str() == Some(text))
                .and_then(|label| label["color"].as_array())
                .cloned()
                .expect("expected fixture label color to be exported")
        };
        assert_eq!(
            label_color("红"),
            vec![
                Value::from(255),
                Value::from(0),
                Value::from(0),
                Value::from(255)
            ],
            "expected the red payload label to keep its text color"
        );
        assert_eq!(
            label_color("绿"),
            vec![
                Value::from(0),
                Value::from(128),
                Value::from(0),
                Value::from(255)
            ],
            "expected the green payload label to keep its text color"
        );
        assert_eq!(
            label_color("蓝"),
            vec![
                Value::from(0),
                Value::from(0),
                Value::from(255),
                Value::from(255)
            ],
            "expected the blue payload label to keep its text color"
        );
        assert_eq!(
            label_color("色调"),
            vec![
                Value::from(0),
                Value::from(0),
                Value::from(255),
                Value::from(255)
            ],
            "expected the hue payload label to keep its blue text color"
        );
        assert_eq!(
            label_color("饱和度"),
            vec![
                Value::from(0),
                Value::from(0),
                Value::from(255),
                Value::from(255)
            ],
            "expected the saturation payload label to keep its blue text color"
        );
        assert_eq!(
            label_color("亮度"),
            vec![
                Value::from(0),
                Value::from(0),
                Value::from(255),
                Value::from(255)
            ],
            "expected the brightness payload label to keep its blue text color"
        );

        let visible_label = |text: &str| {
            labels
                .iter()
                .find(|label| label["text"].as_str() == Some(text))
                .and_then(|label| label["visible"].as_bool())
                .expect("expected fixture label visibility to be exported")
        };
        assert!(
            visible_label("红 = 0.28"),
            "expected the red segment parameter label to use the concise named form"
        );
        assert!(
            visible_label("绿 = 0.48"),
            "expected the green segment parameter label to use the concise named form"
        );
        assert!(
            visible_label("蓝 = 0.79"),
            "expected the blue segment parameter label to use the concise named form"
        );
        assert!(
            visible_label("色调 = 0.19"),
            "expected the hue segment parameter label to use the concise named form"
        );
        assert!(
            visible_label("饱和度 = 0.54"),
            "expected the saturation segment parameter label to use the concise named form"
        );
        assert!(
            visible_label("亮度 = 0.77"),
            "expected the brightness segment parameter label to use the concise named form"
        );
        assert!(
            labels
                .iter()
                .all(|label| label["text"].as_str() != Some("红在AB上的t值 = 0.28")),
            "expected the verbose red segment helper label to be omitted when the anchor is named"
        );
        assert!(
            labels
                .iter()
                .all(|label| label["text"].as_str() != Some("色调在FG上的t值 = 0.19")),
            "expected the verbose hue segment helper label to be omitted when the anchor is named"
        );
    }
}
