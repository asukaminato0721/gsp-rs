use super::*;

#[test]
fn rejects_unnamed1_instead_of_exporting_an_incomplete_geometry_runtime() {
    let data = include_bytes!("../../../tests/fixtures/未实现的系统功能/未命名1.gsp");
    let error = fixture_scene_error(data);
    assert!(error.contains("does not produce a complete object graph"));
    assert!(error.contains("point:20:point-binding"));
}

#[test]
fn exports_segment_intersection_fixture_into_html() {
    let scene = fixture_scene(
        include_bytes!("../../../tests/fixtures/gsp/insection/segment_insection.gsp"),
        "segment intersection fixture should compile",
    );

    let points = scene["points"]
        .as_array()
        .expect("scene points should be an array");
    let intersection = points
        .iter()
        .find(|point| point["constraint"]["kind"].as_str() == Some("line-intersection"))
        .expect("expected segment intersection point");
    assert_eq!(
        intersection["constraint"]["left"]["kind"].as_str(),
        Some("segment")
    );
    assert_eq!(
        intersection["constraint"]["right"]["kind"].as_str(),
        Some("segment")
    );
    assert_eq!(intersection["x"].as_f64(), Some(416.3160761196899));
    assert_eq!(intersection["y"].as_f64(), Some(345.2222079835971));
}

#[test]
fn exports_segment_circle_intersection_fixture_into_html() {
    let scene = fixture_scene(
        include_bytes!("../../../tests/fixtures/gsp/insection/circle_insection.gsp"),
        "segment-circle intersection fixture should compile",
    );

    let points = scene["points"]
        .as_array()
        .expect("scene points should be an array");
    let intersection = points
        .iter()
        .find(|point| point["constraint"]["kind"].as_str() == Some("line-circle-intersection"))
        .expect("expected segment-circle intersection point");
    assert_eq!(
        intersection["constraint"]["line"]["kind"].as_str(),
        Some("segment")
    );
    assert_eq!(intersection["x"].as_f64(), Some(566.0581863195608));
    assert_eq!(intersection["y"].as_f64(), Some(417.2769704284295));
}

#[test]
fn exports_perpendicular_intersection_fixture_into_html() {
    let html = fixture_html(
        include_bytes!("../../../tests/fixtures/gsp/perp.gsp"),
        "perp fixture should compile",
    );

    assert!(html.contains("\"x\":867.3347427619246"));
    assert!(html.contains("\"y\":469.95590501978756"));
    assert!(html.contains("\"kind\":\"line-intersection\""));
    assert!(html.contains("\"right\":{\"kind\":\"matrix-apply\""));
    assert!(html.contains(
        "\"kind\":\"rotate-source-point\",\"sourcePointIndex\":0,\"angleDegrees\":-90.0"
    ));
    assert!(html.contains("\"targetIndex\":2"));
}

#[test]
fn exports_perpendicular_segment_fixture_into_html() {
    let html = fixture_html(
        include_bytes!("../../../tests/fixtures/gsp/垂线段.gsp"),
        "perpendicular segment fixture should compile",
    );

    assert!(html.contains("\"constraint\":{\"kind\":\"line-intersection\""));
    assert!(html.contains("\"right\":{\"kind\":\"matrix-apply\""));
    assert!(html.contains("\"targetIndex\":1"));
    assert!(!html.contains("\"kind\":\"perpendicular-line\""));
    assert!(!html.contains("\"kind\":\"segment\",\"startIndex\":0,\"endIndex\":3"));
}

#[test]
fn exports_coordinate_trace_intersection_fixture_into_html() {
    let html = fixture_html(
        include_bytes!("../../../tests/fixtures/gsp/insection/cood_intersection.gsp"),
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
        include_bytes!("../../../tests/fixtures/gsp/insection/cood_intersection_y.gsp"),
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
        include_bytes!("../../../tests/fixtures/gsp/insection/cood_intersection_xy.gsp"),
        "coordinate trace xy intersection fixture should compile",
    );

    assert!(html.contains("\"kind\":\"coordinate-trace\""));
    assert!(html.contains("\"kind\":\"coordinate-source-2d\""));
    assert!(html.contains("\"kind\":\"line-trace-intersection\""));
    assert!(html.contains("\"y\":3.069166666666"));
}
