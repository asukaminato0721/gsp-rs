use super::*;

#[test]
fn exports_scaled_circle_intersections_fixture_with_live_constraints() {
    let Some(output) = standard_fixture_output(
        "scaled-circle-intersections",
        "tests/fixtures/bug/圆的伸缩变换.gsp",
    ) else {
        return;
    };
    let scene = &output.scene;
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
            circle["binding"]["kind"].as_str() == Some("matrix-apply")
                && circle["binding"]["matrixApply"][0]["kind"].as_str() == Some("scale")
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

    assert!(
        output.html.contains("function circleCircleIntersection("),
        "expected live circular intersections to pull in the intersections runtime"
    );
    assert!(output.payload_log.contains("问题数量: 0"));
    assert!(output.payload_log.contains("伸缩"));
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
            circle["binding"]["kind"].as_str() == Some("matrix-apply")
                && circle["binding"]["matrixApply"][0]["kind"].as_str() == Some("reflect")
        }),
        "expected the reflected payload circle to keep its live binding"
    );
    assert!(
        circles.iter().any(|circle| {
            circle["binding"]["kind"].as_str() == Some("matrix-apply")
                && circle["binding"]["matrixApply"][0]["kind"].as_str() == Some("scale")
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
fn exports_kaleidoscope_nested_reflected_polygons() {
    let Some(data) = fixture_bytes("tests/Samples/未分类档/万花筒.gsp") else {
        return;
    };
    let scene = fixture_scene(&data, "kaleidoscope fixture should compile");
    let polygons = scene["polygons"]
        .as_array()
        .expect("scene polygons should be an array");
    assert_eq!(
        polygons.len(),
        287,
        "expected seven seed polygons plus every polygon Reflection() from the .htm construction"
    );
    for ordinal in [54, 61, 89, 131, 337] {
        let polygon = polygons
            .iter()
            .find(|polygon| polygon["debug"]["groupOrdinal"].as_u64() == Some(ordinal))
            .unwrap_or_else(|| panic!("expected reflected polygon #{ordinal} to export"));
        assert_eq!(
            polygon["binding"]["kind"].as_str(),
            Some("matrix-apply"),
            "expected reflected polygon #{ordinal} to keep a live transform binding"
        );
        assert_eq!(
            polygon["binding"]["matrixApply"][0]["kind"].as_str(),
            Some("reflect"),
            "expected reflected polygon #{ordinal} to stay reflection-bound"
        );
    }

    let lines = scene["lines"]
        .as_array()
        .expect("scene lines should be an array");
    for ordinal in [5, 6, 53] {
        assert!(
            lines.iter().any(|line| {
                line["debug"]["groupOrdinal"].as_u64() == Some(ordinal)
                    && line["binding"]["kind"].as_str() == Some("matrix-apply")
                    && line["binding"]["matrixApply"][0]["kind"].as_str() == Some("rotate")
            }),
            "expected rotated axis segment #{ordinal} to export for reflection axes"
        );
    }
    let transformed_midpoint = scene["points"]
        .as_array()
        .and_then(|points| {
            points
                .iter()
                .find(|point| point["debug"]["groupOrdinal"].as_u64() == Some(8))
        })
        .expect("expected midpoint #8 of rotated segment #5 to export");
    assert_eq!(
        transformed_midpoint["constraint"]["kind"].as_str(),
        Some("line-constraint")
    );
    assert_eq!(
        transformed_midpoint["constraint"]["line"]["kind"].as_str(),
        Some("matrix-apply")
    );

    let buttons = scene["buttons"]
        .as_array()
        .expect("scene buttons should be an array");
    let button = buttons
        .iter()
        .find(|button| button["debug"]["groupOrdinal"].as_u64() == Some(52))
        .expect("expected AnimateButton #52 to export");
    assert_eq!(button["text"].as_str(), Some("动画点"));
    assert_eq!(
        button["action"]["kind"].as_str(),
        Some("animate-points"),
        "expected the .htm AnimateButton to keep all animated point refs"
    );
    let animated_ordinals = button["action"]["targets"]
        .as_array()
        .expect("animate-points should carry target definitions")
        .iter()
        .map(|target| {
            let index = target["pointIndex"]
                .as_u64()
                .expect("point index should be numeric") as usize;
            scene["points"][index]["debug"]["groupOrdinal"]
                .as_u64()
                .expect("animated point should keep its payload ordinal")
        })
        .collect::<Vec<_>>();
    assert_eq!(
        animated_ordinals,
        vec![
            10, 19, 21, 23, 14, 46, 36, 44, 12, 45, 35, 34, 27, 40, 26, 38, 49, 39, 25, 48, 50, 16,
            30, 31, 32, 18, 17
        ],
        "expected all point refs from AnimateButton #52 to match the .htm order"
    );
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
            circle["binding"]["kind"].as_str() == Some("matrix-apply")
                && circle["binding"]["matrixApply"][0]["kind"].as_str() == Some("translate-delta")
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
        html.contains("\"kind\":\"matrix-apply\"")
            && html.contains("\"matrixApply\":[{\"kind\":\"translate-delta\""),
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
