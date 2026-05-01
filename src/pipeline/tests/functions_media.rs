use super::*;

#[test]
fn exports_draw_function_fixture_with_payload_linked_labels() {
    let scene = fixture_scene(
        include_bytes!("../../../tests/fixtures/未实现的系统功能/绘图函数.gsp"),
        "draw function fixture should compile",
    );
    let images = scene["images"]
        .as_array()
        .expect("scene images should be an array");
    assert_eq!(images.len(), 1);
    assert_eq!(images[0]["screenSpace"].as_bool(), Some(true));
    assert!(
        images[0]["src"]
            .as_str()
            .is_some_and(|src| src.starts_with("data:image/png;base64,")),
        "expected embedded png data url"
    );
    assert_eq!(images[0]["topLeft"]["x"].as_f64(), Some(95.0));
    assert_eq!(images[0]["topLeft"]["y"].as_f64(), Some(198.0));
    assert_eq!(images[0]["bottomRight"]["x"].as_f64(), Some(536.0));
    assert_eq!(images[0]["bottomRight"]["y"].as_f64(), Some(273.0));
}

#[test]
fn exports_parameter_curve_fixture_as_parametric_line() {
    let scene = fixture_scene(
        include_bytes!("../../../tests/fixtures/gsp/static/parameter_curve.gsp"),
        "parameter curve fixture should compile",
    );

    assert_eq!(scene["lines"].as_array().map(Vec::len), Some(0));
    assert_eq!(scene["parameters"].as_array().map(Vec::len), Some(0));
    assert_eq!(scene["points"].as_array().map(Vec::len), Some(0));
    let definitions = scene["functionDefinitions"]
        .as_array()
        .expect("scene function definitions should be an array");
    assert_eq!(definitions.len(), 1);
    assert_eq!(definitions[0]["name"].as_str(), Some("f"));
    assert_eq!(definitions[0]["expr"]["kind"].as_str(), Some("parsed"));
    assert_eq!(
        definitions[0]["expr"]["expr"]["kind"].as_str(),
        Some("unary")
    );
    assert_eq!(definitions[0]["expr"]["expr"]["op"].as_str(), Some("sin"));
    let labels = scene["labels"]
        .as_array()
        .expect("scene labels should be an array");
    assert_eq!(labels.len(), 1);
    assert_eq!(labels[0]["text"].as_str(), Some("f(x) = sin(x)"));
    assert_eq!(labels[0]["visible"].as_bool(), Some(true));
}

#[test]
fn exports_parameter_curve1_fixture_with_two_standalone_function_definitions() {
    let scene = fixture_scene(
        include_bytes!("../../../tests/fixtures/gsp/static/parameter_curve1.gsp"),
        "parameter curve1 fixture should compile",
    );

    assert_eq!(scene["parameters"].as_array().map(Vec::len), Some(0));
    assert_eq!(scene["points"].as_array().map(Vec::len), Some(0));
    let definitions = scene["functionDefinitions"]
        .as_array()
        .expect("scene function definitions should be an array");
    assert_eq!(definitions.len(), 2);
    assert_eq!(definitions[0]["name"].as_str(), Some("f"));
    assert_eq!(definitions[1]["name"].as_str(), Some("h"));
    assert_eq!(
        definitions[1]["expr"]["expr"]["kind"].as_str(),
        Some("unary")
    );
    assert_eq!(definitions[1]["expr"]["expr"]["op"].as_str(), Some("cos"));

    let labels = scene["labels"]
        .as_array()
        .expect("scene labels should be an array");
    assert_eq!(labels.len(), 2);
    assert_eq!(labels[0]["text"].as_str(), Some("f(x) = sin(x)"));
    assert_eq!(labels[1]["text"].as_str(), Some("h(x) = cos(2*x)"));
    assert!(
        labels
            .iter()
            .all(|label| label["visible"].as_bool() == Some(true))
    );
    assert_eq!(labels[0]["debug"]["groupOrdinal"].as_u64(), Some(1));
    assert_eq!(labels[1]["debug"]["groupOrdinal"].as_u64(), Some(2));
}

#[test]
fn exports_parameter_curve2_fixture_with_functions_and_parametric_curve() {
    let scene = fixture_scene(
        include_bytes!("../../../tests/fixtures/gsp/static/parameter_curve2.gsp"),
        "parameter curve2 fixture should compile",
    );

    assert_eq!(scene["parameters"].as_array().map(Vec::len), Some(0));
    let definitions = scene["functionDefinitions"]
        .as_array()
        .expect("scene function definitions should be an array");
    assert_eq!(definitions.len(), 2);
    assert_eq!(definitions[0]["name"].as_str(), Some("f"));
    assert_eq!(definitions[1]["name"].as_str(), Some("h"));

    let lines = scene["lines"]
        .as_array()
        .expect("scene lines should be an array");
    let parametric_line = lines
        .iter()
        .find(|line| line["binding"]["kind"].as_str() == Some("parametric-curve"))
        .expect("expected parametric curve line binding");
    assert!(
        parametric_line["points"]
            .as_array()
            .is_some_and(|points| points.len() > 2),
        "expected sampled parametric curve points"
    );
    let x_min = parametric_line["binding"]["xMin"]
        .as_f64()
        .expect("parametric curve xMin should be a number");
    let x_max = parametric_line["binding"]["xMax"]
        .as_f64()
        .expect("parametric curve xMax should be a number");
    assert!(
        (x_max - x_min - std::f64::consts::TAU).abs() < 1e-6,
        "expected periodic parametric curve domain to reduce to one shared period"
    );
    assert_eq!(
        parametric_line["binding"]["yExpr"]["expr"]["kind"].as_str(),
        Some("unary")
    );
    assert_eq!(
        parametric_line["binding"]["yExpr"]["expr"]["op"].as_str(),
        Some("cos")
    );

    let labels = scene["labels"]
        .as_array()
        .expect("scene labels should be an array");
    assert_eq!(labels.len(), 2);
    assert_eq!(labels[0]["text"].as_str(), Some("f(x) = sin(x)"));
    assert_eq!(labels[1]["text"].as_str(), Some("h(x) = cos(2*x)"));
    assert!(
        labels
            .iter()
            .all(|label| label["visible"].as_bool() == Some(true))
    );
    assert_eq!(labels[0]["debug"]["groupOrdinal"].as_u64(), Some(1));
    assert_eq!(labels[1]["debug"]["groupOrdinal"].as_u64(), Some(2));
}

#[test]
fn exports_insert_image_fixture() {
    let scene = fixture_scene(
        include_bytes!("../../../tests/fixtures/未实现的系统功能/插入图片.gsp"),
        "insert image fixture should compile",
    );
    let images = scene["images"]
        .as_array()
        .expect("scene images should be an array");
    assert_eq!(images.len(), 1);
    assert_eq!(images[0]["screenSpace"].as_bool(), Some(true));
    assert!(
        images[0]["src"]
            .as_str()
            .is_some_and(|src| src.starts_with("data:image/png;base64,")),
        "expected embedded png data url"
    );
    assert_eq!(images[0]["topLeft"]["x"].as_f64(), Some(118.0));
    assert_eq!(images[0]["topLeft"]["y"].as_f64(), Some(112.0));
    assert_eq!(images[0]["bottomRight"]["x"].as_f64(), Some(373.0));
    assert_eq!(images[0]["bottomRight"]["y"].as_f64(), Some(270.0));
}
