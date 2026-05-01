use super::*;

#[test]
fn exports_multiline_text_into_html() {
    let html = fixture_html(
        include_bytes!("../../../../tests/fixtures/gsp/多行文本.gsp"),
        "multiline text fixture should compile",
    );

    assert!(html.contains(
        "\"text\":\"线段中垂线\\n垂线\\n平行线\\n直角三角形\\n点的轨迹\\n圆上的弧\\n过三点的弧\""
    ));
}

#[test]
fn exports_hidden_point_fixture_into_html() {
    let html = fixture_html(
        include_bytes!("../../../../tests/fixtures/gsp/static/point_hidden.gsp"),
        "hidden-point fixture should compile",
    );

    assert!(html.contains(
        "\"points\":[{\"x\":323.0,\"y\":217.0,\"color\":[255,0,0,255],\"visible\":false"
    ));
    assert!(html.contains("\"lines\":[]"));
}

#[test]
fn exports_hidden_ray_fixture_into_html() {
    let scene = fixture_scene(
        include_bytes!("../../../../tests/fixtures/gsp/static/hide_ray.gsp"),
        "hidden-ray fixture should compile",
    );
    let lines = scene["lines"]
        .as_array()
        .expect("scene lines should be an array");

    assert_eq!(lines.len(), 2, "expected two rays in the exported scene");
    assert!(
        lines
            .iter()
            .any(|line| line["visible"].as_bool() == Some(false)),
        "expected one exported ray to stay hidden from the source payload"
    );
    assert!(
        lines
            .iter()
            .any(|line| line["visible"].as_bool() == Some(true)),
        "expected one exported ray to stay visible"
    );
    assert!(
        lines
            .iter()
            .all(|line| line["binding"]["kind"].as_str() == Some("ray")),
        "expected both exported line bindings to remain rays"
    );
}

#[test]
fn exports_angle_marker_label_fixture_into_html() {
    let scene = fixture_scene(
        include_bytes!("../../../../tests/fixtures/gsp/static/angle_marker_label.gsp"),
        "angle-marker-label fixture should compile",
    );
    let labels = scene["labels"]
        .as_array()
        .expect("scene labels should be an array");
    assert!(
        labels
            .iter()
            .any(|label| label["text"].as_str() == Some("42.5")),
        "expected exported labels to include the payload angle marker label"
    );
    assert!(
        scene["lines"].as_array().is_some_and(|lines| lines
            .iter()
            .any(|line| line["binding"]["kind"].as_str() == Some("angle-marker"))),
        "expected exported angle marker to stay interactive"
    );
    assert!(labels.iter().any(|label| {
        label["binding"]["kind"].as_str() == Some("angle-marker-value")
            && label["binding"]["startIndex"].as_u64() == Some(1)
            && label["binding"]["vertexIndex"].as_u64() == Some(0)
            && label["binding"]["endIndex"].as_u64() == Some(2)
            && label["binding"]["decimals"].as_u64() == Some(1)
    }));
}

#[test]
fn exports_ray_label_hide_fixture_into_html() {
    let scene = fixture_scene(
        include_bytes!("../../../../tests/fixtures/gsp/static/ray_label_hide.gsp"),
        "ray-label-hide fixture should compile",
    );
    let labels = scene["labels"]
        .as_array()
        .expect("scene labels should be an array");
    assert_eq!(
        labels.len(),
        2,
        "expected both payload ray labels to export"
    );
    assert!(labels.iter().any(|label| {
        label["text"].as_str() == Some("j") && label["visible"].as_bool() == Some(true)
    }));
    assert!(labels.iter().any(|label| {
        label["text"].as_str() == Some("k") && label["visible"].as_bool() == Some(false)
    }));
}

#[test]
fn html_viewer_preserves_label_visibility_flags() {
    let html = fixture_html(
        include_bytes!("../../../../tests/fixtures/gsp/static/ray_label_hide.gsp"),
        "ray-label-hide fixture should compile to html",
    );

    assert!(
        html.contains("\"text\":\"k\"") && html.contains("\"visible\":false"),
        "expected scene JSON embedded in html to preserve the hidden ray label"
    );
    assert!(
        html.contains("visible: label.visible !== false"),
        "expected bundled viewer runtime to hydrate label visibility from the source scene"
    );
}
