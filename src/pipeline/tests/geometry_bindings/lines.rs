use super::*;

#[test]
fn exports_perpendicular_line_binding_into_html() {
    let html = fixture_html(
        include_bytes!("../../../../tests/fixtures/gsp/static/perpendicular.gsp"),
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
        include_bytes!("../../../../tests/fixtures/gsp/parallel.gsp"),
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

    assert!(html.contains("\"kind\":\"perpendicular-line\",\"throughIndex\":3"));
    assert!(html.contains("\"kind\":\"perpendicular-line\",\"throughIndex\":1"));
    assert!(html.contains("\"kind\":\"parallel-line\",\"throughIndex\":1"));
    assert!(html.contains("\"lineIndex\":1"));
}
