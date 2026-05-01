use super::*;

#[test]
fn exports_circle_system_fixture_with_live_parameter_and_bindings() {
    let scene = fixture_scene(
        include_bytes!("../../../../tests/fixtures/未实现/圆系(inRm).gsp"),
        "circle-system fixture should compile",
    );
    let parameters = scene["parameters"]
        .as_array()
        .expect("scene parameters should be an array");
    assert_eq!(parameters.len(), 1, "expected one live n parameter");
    assert_eq!(parameters[0]["name"].as_str(), Some("n"));
    assert_eq!(parameters[0]["value"].as_f64(), Some(20.0));

    let lines = scene["lines"]
        .as_array()
        .expect("scene lines should be an array");
    assert!(
        lines
            .iter()
            .any(|line| line["binding"]["kind"].as_str() == Some("segment")),
        "expected the payload segment to stay interactive"
    );
    assert!(
        lines
            .iter()
            .any(|line| line["binding"]["kind"].as_str() == Some("ray")),
        "expected the payload ray to stay interactive"
    );

    let points = scene["points"]
        .as_array()
        .expect("scene points should be an array");
    assert!(
        points.iter().any(|point| {
            point["binding"]["kind"].as_str() == Some("derived")
                && point["binding"]["transform"]["kind"].as_str() == Some("rotate")
        }),
        "expected the rotated payload point to keep its live binding"
    );
    assert!(
        points.iter().any(|point| {
            point["binding"]["kind"].as_str() == Some("derived")
                && point["binding"]["transform"]["kind"].as_str() == Some("scale")
        }),
        "expected the scaled payload point to keep its live binding"
    );

    let labels = scene["labels"]
        .as_array()
        .expect("scene labels should be an array");
    assert!(labels.iter().any(|label| {
        label["binding"]["kind"].as_str() == Some("parameter-value")
            && label["text"].as_str() == Some("n = 20")
    }));
    assert!(labels.iter().any(|label| {
        label["binding"]["kind"].as_str() == Some("polygon-boundary-parameter")
            && label["text"].as_str() == Some("m = 0.95")
    }));
    assert!(labels.iter().any(|label| {
        label["binding"]["kind"].as_str() == Some("expression-value")
            && label["binding"]["parameterName"].as_str() == Some("m")
            && label["text"].as_str() == Some("(m在ABB''B'上的值 + 1) / n = 0.05")
    }));
    let circles = scene["circles"]
        .as_array()
        .expect("scene circles should be an array");
    assert_eq!(
        circles.len(),
        21,
        "expected source plus iterated payload circles"
    );
    assert!(
        circles
            .iter()
            .any(|circle| circle["binding"]["kind"].as_str() == Some("segment-radius-circle")),
        "expected the payload circle to keep its live center/radius-segment binding"
    );
    let circle_iterations = scene["circleIterations"]
        .as_array()
        .expect("scene circle iterations should be an array");
    assert_eq!(
        circle_iterations.len(),
        1,
        "expected one live circle iteration family"
    );
    assert_eq!(circle_iterations[0]["depth"].as_u64(), Some(20));
    let polygons = scene["polygons"]
        .as_array()
        .expect("scene polygons should be an array");
    assert!(
        polygons
            .iter()
            .any(|polygon| polygon["binding"]["kind"].as_str() == Some("point-polygon")),
        "expected the payload polygon to stay interactive"
    );
    assert!(
        scene["points"]
            .as_array()
            .expect("scene points should be an array")
            .iter()
            .any(|point| point["constraint"]["kind"].as_str() == Some("polygon-boundary")),
        "expected the payload boundary point to remain live"
    );
}
