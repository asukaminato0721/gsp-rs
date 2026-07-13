use super::*;

#[test]
fn exports_polar_function_fixture_into_html() {
    let html = fixture_html(
        include_bytes!("../../../../tests/fixtures/未实现的系统功能/极坐标.gsp"),
        "polar fixture should compile",
    );

    assert!(html.contains("\"plotMode\":\"polar\""));
    assert!(html.contains("\"name\":\"g\""));
    assert!(!html.contains("\"text\":\"r = 1 + cos(θ)\""));
    assert!(html.contains("\"x\":-0.24999414519673077"));
}

#[test]
fn exports_parameterized_function_fixture_with_unique_parameters() {
    let scene = fixture_scene(
        include_bytes!("../../../../tests/fixtures/未实现的系统功能/函数.gsp"),
        "parameterized function fixture should compile",
    );
    assert_eq!(scene["piMode"].as_bool(), Some(false));
    assert_eq!(scene["savedViewport"].as_bool(), Some(true));
    let parameters = scene["parameters"]
        .as_array()
        .expect("scene parameters should be an array");
    let parameter_names = parameters
        .iter()
        .map(|parameter| {
            parameter["name"]
                .as_str()
                .expect("parameter name should be a string")
        })
        .collect::<Vec<_>>();
    assert_eq!(parameter_names, vec!["a", "b", "c"]);
    assert!(
        parameters
            .iter()
            .all(|parameter| parameter["labelIndex"].is_null())
    );

    let functions = scene["functions"]
        .as_array()
        .expect("scene functions should be an array");
    assert_eq!(functions.len(), 1);
    assert_eq!(functions[0]["name"].as_str(), Some("f"));
    assert_eq!(functions[0]["lineIndex"].as_u64(), Some(3));
    assert!(scene["labels"].as_array().is_some_and(Vec::is_empty));
    assert_eq!(
        functions[0]["expr"]["expr"]["kind"].as_str(),
        Some("binary")
    );
    assert_eq!(functions[0]["expr"]["expr"]["op"].as_str(), Some("add"));
    assert_eq!(
        functions[0]["expr"]["expr"]["lhs"]["kind"].as_str(),
        Some("binary")
    );
    assert_eq!(
        functions[0]["expr"]["expr"]["lhs"]["op"].as_str(),
        Some("add")
    );
}
