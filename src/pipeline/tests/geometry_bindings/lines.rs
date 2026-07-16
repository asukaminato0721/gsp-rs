use super::*;

#[test]
fn exports_perpendicular_line_binding_into_html() {
    let html = fixture_html(
        include_bytes!("../../../../tests/fixtures/gsp/static/perpendicular.gsp"),
        "perpendicular fixture should compile",
    );

    assert!(html.contains("\"kind\":\"matrix-apply\",\"sourceIndex\":0"));
    assert!(html.contains(
        "\"kind\":\"rotate-source-point\",\"sourcePointIndex\":0,\"angleDegrees\":-90.0"
    ));
    assert!(
        html.contains(
            "\"kind\":\"translate-source-point\",\"sourcePointIndex\":0,\"targetIndex\":1"
        )
    );
    assert!(!html.contains("\"kind\":\"perpendicular-line\""));
}

#[test]
fn exports_parallel_line_binding_into_html() {
    let html = fixture_html(
        include_bytes!("../../../../tests/fixtures/gsp/parallel.gsp"),
        "parallel fixture should compile",
    );

    assert!(html.contains("\"kind\":\"matrix-apply\",\"sourceIndex\":0"));
    assert!(
        html.contains(
            "\"kind\":\"translate-source-point\",\"sourcePointIndex\":0,\"targetIndex\":2"
        )
    );
    assert!(!html.contains("\"kind\":\"parallel-line\""));
}

#[test]
fn exports_angle_bisector_ray_binding_into_html() {
    let html = fixture_html(
        include_bytes!("../../../../tests/fixtures/gsp/static/bisector.gsp"),
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
        include_bytes!("../../../../tests/fixtures/gsp/pert_vert.gsp"),
        "pert_vert fixture should compile",
    );

    assert!(html.contains("\"targetIndex\":3"));
    assert!(html.matches("\"targetIndex\":1").count() >= 2);
    assert!(html.matches("\"kind\":\"rotate-source-point\"").count() >= 2);
    assert!(!html.contains("\"kind\":\"perpendicular-line\""));
    assert!(!html.contains("\"kind\":\"parallel-line\""));
}
