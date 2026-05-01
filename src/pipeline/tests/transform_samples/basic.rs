use super::*;

#[test]
fn exports_translated_triangle_segments_into_html() {
    let html = fixture_html(
        include_bytes!("../../../../tests/fixtures/gsp/两个三角形标记全等.gsp"),
        "congruent triangle fixture should compile",
    );

    assert!(html.contains("\"kind\":\"derived\""));
    assert!(html.contains("\"kind\":\"angle-marker\""));
    assert!(html.contains("\"kind\":\"segment-marker\""));
    assert!(html.contains(
        "\"transform\":{\"kind\":\"translate\",\"vectorStartIndex\":0,\"vectorEndIndex\":3}"
    ));
    assert!(html.contains("\"text\":\"B'\""));
    assert!(html.contains("\"text\":\"C'\""));
}

#[test]
fn exports_circular_segment_boundary_fixture_with_polyline_constraint() {
    let scene = fixture_scene(
        include_bytes!("../../../../tests/fixtures/未实现的系统功能/弓形周界动点.gsp"),
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
            polygons
                .iter()
                .any(|polygon| polygon["binding"]["kind"].as_str() == Some("arc-boundary-polygon"))
        }),
        "expected the circular segment fill to export as a live boundary polygon"
    );
}

#[test]
fn exports_custom_transform_fixture_with_interactive_point_binding() {
    let html = fixture_html(
        include_bytes!("../../../../tests/fixtures/未实现的系统功能/自定义变换.gsp"),
        "custom transform fixture should compile",
    );

    assert!(html.contains("\"text\":\"Q\""));
    assert!(html.contains("\"kind\":\"custom-transform\""));
    assert!(html.contains("\"sourceIndex\":2"));
    assert!(html.contains("\"name\":\"P\""));
    assert!(html.contains("1厘米"));
    assert!(html.contains("100°"));
}
