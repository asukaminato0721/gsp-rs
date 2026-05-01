use super::*;

#[test]
fn exports_three_point_arc_into_html() {
    let html = fixture_html(
        include_bytes!("../../../../tests/fixtures/gsp/static/three_point_arc.gsp"),
        "three-point arc fixture should compile",
    );

    assert!(html.contains("\"arcs\":["));
    assert!(html.contains("\"color\":[0,128,0,255]"));
    assert!(html.contains("\"points\":[{\"x\":323.0,\"y\":217.0}"));
}

#[test]
fn exports_three_point_arc_point_constraint_into_html() {
    let html = fixture_html(
        include_bytes!("../../../../tests/fixtures/gsp/static/three_point_arc_point.gsp"),
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
        include_bytes!("../../../../tests/fixtures/gsp/static/arc_on_circle.gsp"),
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
        include_bytes!("../../../../tests/fixtures/gsp/point_on_arc1.gsp"),
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
        include_bytes!("../../../../tests/fixtures/gsp/static/value_point_arc_on_circle.gsp"),
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
        include_bytes!("../../../../tests/fixtures/gsp/static/three_point_arc_intersection.gsp"),
        "three-point arc intersection fixture should compile",
    );

    assert!(html.contains("\"kind\":\"circular-intersection\""));
    assert!(html.contains("\"left\":{\"kind\":\"three-point-arc\""));
    assert!(html.contains("\"right\":{\"kind\":\"three-point-arc\""));
}

#[test]
fn exports_circle_center_radius_into_html() {
    let html = fixture_html(
        include_bytes!("../../../../tests/fixtures/gsp/circle_center_radius.gsp"),
        "circle-center-radius fixture should compile",
    );

    assert!(html.contains("\"circles\":[{\"center\":{\"x\":348.0,\"y\":201.0}"));
    assert!(html.contains("\"kind\":\"segment-radius-circle\""));
    assert!(
        html.contains(
            "\"lines\":[{\"points\":[{\"x\":318.0,\"y\":415.0},{\"x\":403.0,\"y\":414.0}]"
        )
    );
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
