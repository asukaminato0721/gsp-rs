use super::*;

#[test]
fn exports_scaled_circle_intersections_fixture_with_live_constraints() {
    let Some(data) = fixture_bytes("tests/fixtures/bug/圆的伸缩变换.gsp") else {
        return;
    };
    let scene = fixture_scene(&data, "scaled-circle intersection fixture should compile");
    let bounds = &scene["bounds"];
    assert!(
        bounds["minX"].as_f64().is_some_and(|min_x| min_x < 832.0)
            && bounds["minY"].as_f64().is_some_and(|min_y| min_y < 373.0),
        "expected the viewport to include the live circle intersections"
    );
    let circles = scene["circles"]
        .as_array()
        .expect("scene circles should be an array");
    assert_eq!(
        circles.len(),
        3,
        "expected both payload circles plus the scaled circle"
    );
    assert!(
        circles.iter().any(|circle| {
            circle["binding"]["kind"].as_str() == Some("derived")
                && circle["binding"]["transform"]["kind"].as_str() == Some("scale")
        }),
        "expected the scaled payload circle to keep its live binding"
    );

    let constrained_points = scene["points"]
        .as_array()
        .expect("scene points should be an array")
        .iter()
        .filter(|point| point["constraint"]["kind"].as_str() == Some("circular-intersection"))
        .collect::<Vec<_>>();
    assert_eq!(
        constrained_points.len(),
        2,
        "expected both payload circle intersections to stay live"
    );
    assert!(constrained_points.iter().all(|point| {
        (point["constraint"]["left"]["kind"].as_str() == Some("derived")
            && point["constraint"]["left"]["transform"]["kind"].as_str() == Some("scale"))
            || (point["constraint"]["right"]["kind"].as_str() == Some("derived")
                && point["constraint"]["right"]["transform"]["kind"].as_str() == Some("scale"))
    }));

    let html = fixture_html(&data, "scaled-circle intersection fixture should compile");
    assert!(
        html.contains("function circleCircleIntersection("),
        "expected live circular intersections to pull in the intersections runtime"
    );
}

#[test]
fn exports_nested_scaled_reflected_circle_fixture_with_live_constraints() {
    let Some(data) = fixture_bytes("tests/fixtures/bug/圆的伸缩变换1.gsp") else {
        return;
    };
    let scene = fixture_scene(
        &data,
        "nested scaled reflected-circle fixture should compile",
    );
    let circles = scene["circles"]
        .as_array()
        .expect("scene circles should be an array");
    assert_eq!(
        circles.len(),
        3,
        "expected original, reflected, and scaled-reflected circles"
    );
    assert!(
        circles.iter().any(|circle| {
            circle["binding"]["kind"].as_str() == Some("derived")
                && circle["binding"]["transform"]["kind"].as_str() == Some("reflect")
        }),
        "expected the reflected payload circle to keep its live binding"
    );
    assert!(
        circles.iter().any(|circle| {
            circle["binding"]["kind"].as_str() == Some("derived")
                && circle["binding"]["transform"]["kind"].as_str() == Some("scale")
        }),
        "expected the scaled payload circle to keep its live binding"
    );

    let constrained_points = scene["points"]
        .as_array()
        .expect("scene points should be an array")
        .iter()
        .filter(|point| point["constraint"]["kind"].as_str() == Some("circular-intersection"))
        .collect::<Vec<_>>();
    assert_eq!(
        constrained_points.len(),
        2,
        "expected both nested circle intersections to stay live"
    );
    assert!(constrained_points.iter().all(|point| {
        (point["constraint"]["left"]["kind"].as_str() == Some("derived")
            && point["constraint"]["left"]["transform"]["kind"].as_str() == Some("scale"))
            || (point["constraint"]["right"]["kind"].as_str() == Some("derived")
                && point["constraint"]["right"]["transform"]["kind"].as_str() == Some("scale"))
    }));
}

#[test]
fn exports_translation_fixture_with_live_circle_and_intersection() {
    let Some(data) = fixture_bytes("tests/fixtures/gsp/static/translation.gsp") else {
        return;
    };
    let scene = fixture_scene(&data, "translation fixture should compile");
    let circles = scene["circles"]
        .as_array()
        .expect("scene circles should be an array");
    assert_eq!(circles.len(), 2, "expected original and translated circles");
    assert!(
        circles.iter().any(|circle| {
            circle["binding"]["kind"].as_str() == Some("derived")
                && circle["binding"]["transform"]["kind"].as_str() == Some("translate-delta")
        }),
        "expected the translated payload circle to keep its live binding"
    );

    let constrained_point_count = scene["points"]
        .as_array()
        .expect("scene points should be an array")
        .iter()
        .filter(|point| {
            matches!(
                point["constraint"]["kind"].as_str(),
                Some("circular-intersection") | Some("circle-circle-intersection")
            )
        })
        .count();
    assert_eq!(
        constrained_point_count, 1,
        "expected the translated-circle intersection point to stay live"
    );

    let html = fixture_html(&data, "translation fixture should compile");
    assert!(
        html.contains("\"kind\":\"derived\"") && html.contains("\"kind\":\"translate-delta\""),
        "expected the translated circle binding to be embedded in the html scene payload"
    );
    assert!(
        html.contains("constraint.kind === \"derived\""),
        "expected the static circular constraint runtime to resolve translated circles"
    );
    assert!(
        html.contains("function circleCircleIntersection("),
        "expected the intersection runtime to stay available for the translated circles"
    );
}
