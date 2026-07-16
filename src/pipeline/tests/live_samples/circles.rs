use super::*;

#[test]
fn exports_two_circle_intersection_inrm_fixture_with_live_bindings() {
    let scene = fixture_scene(
        include_bytes!("../../../../tests/fixtures/未实现/(inRm)两圆之交.gsp"),
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
    assert_eq!(
        circles
            .iter()
            .filter(|circle| !circle["fillColor"].is_null())
            .count(),
        2,
        "expected both Circle interior objects declared by the HTM payload"
    );

    let polygons = scene["polygons"]
        .as_array()
        .expect("scene polygons should be an array");
    assert_eq!(
        polygons.len(),
        2,
        "expected the payload circular segments that make up the lens"
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
    assert_eq!(
        lines
            .iter()
            .filter(|line| line["binding"]["kind"].as_str() == Some("arc-boundary"))
            .count(),
        2,
        "expected both payload circular-segment boundaries to stay interactive"
    );

    let points = scene["points"]
        .as_array()
        .expect("scene points should be an array");
    let circle_circle_points = points
        .iter()
        .filter(|point| point["constraint"]["kind"].as_str() == Some("circle-circle-intersection"))
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
    let Some(data) = fixture_bytes("tests/fixtures/未实现/(inRm)容器中的罐头.gsp") else {
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
            .filter(|point| {
                point["binding"]["kind"].as_str() == Some("derived")
                    && point["binding"]["matrixApply"][0]["kind"].as_str() == Some("scale")
            })
            .count(),
        4,
        "expected scale-derived helper points to preserve their bindings"
    );
    assert_eq!(
        points
            .iter()
            .filter(|point| {
                point["binding"]["kind"].as_str() == Some("derived")
                    && point["binding"]["matrixApply"][0]["kind"].as_str() == Some("rotate")
            })
            .count(),
        1,
        "expected the rotated helper point to preserve its binding"
    );
    assert_eq!(
        points
            .iter()
            .filter(|point| {
                point["binding"]["kind"].as_str() == Some("derived")
                    && point["binding"]["matrixApply"][0]["kind"].as_str() == Some("translate")
            })
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
