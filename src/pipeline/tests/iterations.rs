use super::*;

#[test]
fn exports_point_iteration_metadata_into_html() {
    let html = fixture_html(
        include_bytes!("../../../tests/fixtures/gsp/static/简单迭代/原象点初象点深度5迭代.gsp"),
        "iteration fixture should compile",
    );

    assert!(html.contains("\"pointIterations\":["));
    assert!(html.contains("\"kind\":\"interpreted\""));
    assert!(html.contains("\"depthParameterName\":\"n\""));
}

#[test]
fn exports_non_graph_iteration_parameters_and_expression_bindings_into_html() {
    let html = fixture_html(
        include_bytes!(
            "../../../tests/fixtures/gsp/static/简单迭代/原象点和参数初象点和数值深度5迭代.gsp"
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
        include_bytes!("../../../tests/fixtures/gsp/static/简单迭代/原象点初象点默认深度3迭代.gsp"),
        "default iteration fixture should compile",
    );

    assert!(html.contains("\"pointIterations\":["));
    assert!(html.contains("\"depth\":3"));
}

#[test]
fn exports_default_depth_non_graph_iteration_fixture_metadata() {
    let html = fixture_html(
        include_bytes!(
            "../../../tests/fixtures/gsp/static/简单迭代/原象点和参数初象点和数值默认深度迭代.gsp"
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
        include_bytes!("../../../tests/fixtures/未实现的系统功能/parameter.gsp"),
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
fn static_fixture_embeds_full_viewer_runtime() {
    let html = fixture_html(
        include_bytes!("../../../tests/fixtures/gsp/static/point.gsp"),
        "static point fixture should compile to html",
    );

    assert!(html.contains("viewer-runtime: full"));
    assert!(html.contains("function sampleDynamicFunction("));
    assert!(html.contains("function drawCircles(env)"));
    assert!(html.contains("function circleArcControlPoints("));
}

#[test]
fn parameter_fixture_embeds_full_viewer_runtime() {
    let html = fixture_html(
        include_bytes!("../../../tests/fixtures/未实现的系统功能/parameter.gsp"),
        "parameter fixture should compile to html",
    );

    assert!(html.contains("viewer-runtime: full"));
    assert!(
        html.contains("function sampleDynamicFunction("),
        "parameter fixture should keep the full dynamics runtime"
    );
}

#[test]
fn hot_text_fixture_uses_full_overlay_runtime() {
    let html = fixture_html(
        include_bytes!("../../../tests/fixtures/gsp/热文本.gsp"),
        "hot text fixture should compile to html",
    );

    assert!(html.contains("viewer-runtime: full"));
    assert!(
        html.contains("function renderRichMarkupNodes("),
        "hot text fixture should keep the full overlay runtime"
    );
}

#[test]
fn circle_arc_fixture_uses_circular_scene_runtime() {
    let html = fixture_html(
        include_bytes!("../../../tests/fixtures/gsp/static/arc_on_circle.gsp"),
        "arc on circle fixture should compile to html",
    );

    assert!(html.contains("viewer-runtime: full"));
    assert!(
        html.contains("function circleArcControlPoints("),
        "arc-on-circle fixture should include the circular scene addon"
    );
}

#[test]
fn coordinate_trace_intersection_fixture_uses_trace_and_intersection_scene_runtime() {
    let html = fixture_html(
        include_bytes!("../../../tests/fixtures/gsp/insection/cood_intersection.gsp"),
        "coordinate trace intersection fixture should compile to html",
    );

    assert!(html.contains("viewer-runtime: full"));
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
fn coordinate_trace_intersection_fixture_has_three_level_artifacts() {
    let fixture = FixtureArtifacts::new(
        "coordinate-trace-intersection",
        "cood_intersection.gsp",
        include_bytes!("../../../tests/fixtures/gsp/insection/cood_intersection.gsp"),
    );

    let output = fixture.compile_standard_with_outputs();

    assert!(output.payload_log.contains("问题数量: 0"));
    assert!(output.payload_log.contains("Construction VALUE"));
    assert!(output.payload_log.contains("坐标点"));
    assert!(output.payload_log.contains("坐标轨迹"));

    let lines = output.scene["lines"]
        .as_array()
        .expect("debug scene should export lines");
    assert!(
        lines.iter().any(|line| {
            line["binding"]["kind"].as_str() == Some("coordinate-trace")
                || line["binding"]["kind"].as_str() == Some("point-trace")
        }),
        "debug scene should preserve the trace binding"
    );
    let points = output.scene["points"]
        .as_array()
        .expect("debug scene should export points");
    assert!(
        points
            .iter()
            .any(|point| point["constraint"]["kind"].as_str() == Some("line-trace-intersection")),
        "debug scene should preserve the line-trace intersection constraint"
    );

    assert!(output.html.contains("viewer-runtime: full"));
    assert!(
        output
            .html
            .contains("function sampleCoordinateTracePoints(")
    );
    assert!(output.html.contains("function lineCircleIntersection("));
}

#[test]
fn exports_carried_polygon_iteration_metadata_into_html() {
    let html = fixture_html(
        include_bytes!(
            "../../../tests/fixtures/gsp/static/简单迭代/原象点初象携带多边形双映射深度4迭代.gsp"
        ),
        "carried polygon iteration fixture should compile",
    );

    assert!(html.contains("\"lineIterations\":[]"));
    assert!(html.contains("\"polygonIterations\":["));
    assert!(html.contains("\"parameterName\":\"n\""));
    assert!(html.contains("\"vertexIndices\":[0,2,1]"));
    assert!(html.contains("\"secondaryDx\":37.79527559055118"));
}
