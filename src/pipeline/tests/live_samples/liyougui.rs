use super::*;

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
                || point["binding"]["kind"].as_str() == Some("constraint-parameter-from-point-expr")
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
