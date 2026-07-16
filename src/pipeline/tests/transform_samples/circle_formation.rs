use super::*;

#[test]
fn exports_circle_formation_fixture_with_rotation_iteration() {
    let scene = fixture_scene(
        include_bytes!("../../../../tests/fixtures/未实现的系统功能/圆的形成.gsp"),
        "circle-formation fixture should compile",
    );
    let parameters = scene["parameters"]
        .as_array()
        .expect("scene parameters should be an array");
    assert_eq!(parameters.len(), 1, "expected a single live t₂ parameter");
    assert_eq!(parameters[0]["name"].as_str(), Some("t₂"));
    assert_eq!(parameters[0]["value"].as_f64(), Some(5.0));

    let lines = scene["lines"]
        .as_array()
        .expect("scene lines should be an array");
    assert_eq!(
        lines.len(),
        1,
        "expected the payload's first related edge to remain serialized as the rotation source"
    );
    assert_eq!(
        lines[0]["debug"]["groupOrdinal"].as_u64(),
        Some(11),
        "expected the serialized source edge to come from payload segment #11"
    );
    let line_iterations = scene["lineIterations"]
        .as_array()
        .expect("scene line iterations should be an array");
    assert_eq!(
        line_iterations
            .iter()
            .filter(|family| family["kind"].as_str() == Some("rotate"))
            .count(),
        1,
        "expected one canonical serialized rotate family"
    );
    assert!(
        line_iterations.iter().any(|family| {
            family["kind"].as_str() == Some("rotate")
                && family["sourceIndex"].as_u64() == Some(0)
                && family["parameterName"].as_str() == Some("t₂")
                && family["depthParameterName"].as_str() == Some("t₃")
                && family["depth"].as_u64() == Some(4)
        }),
        "expected the regular-polygon segment iteration family to serialize the payload source edge and depth driver"
    );
    let labels = scene["labels"]
        .as_array()
        .expect("scene labels should be an array");
    assert!(
        labels.iter().any(|label| {
            label["text"].as_str() == Some("2*180 / t₂ = 72.00°")
                && label["binding"]["kind"].as_str() == Some("expression-value")
                && label["binding"]["parameterName"].as_str() == Some("t₂")
                && label["binding"]["exprLabel"].as_str() == Some("2*180 / t₂")
                && label["binding"]["expr"]["expr"]["lhs"]["rhs"]["kind"].as_str()
                    == Some("pi-angle")
        }),
        "expected the circle-formation angle label to preserve the payload calculation"
    );
    let iteration_tables = scene["iterationTables"]
        .as_array()
        .expect("scene iteration tables should be an array");
    assert_eq!(
        iteration_tables.len(),
        1,
        "expected one visible iteration table"
    );
    assert_eq!(iteration_tables[0]["exprLabel"].as_str(), Some("t₁ + 1"));
    assert_eq!(iteration_tables[0]["parameterName"].as_str(), Some("t₁"));
    assert_eq!(
        iteration_tables[0]["depthParameterName"].as_str(),
        Some("t₃")
    );
    assert_eq!(iteration_tables[0]["x"].as_f64(), Some(322.0));
    assert_eq!(iteration_tables[0]["y"].as_f64(), Some(481.0));
    assert_eq!(iteration_tables[0]["depth"].as_u64(), Some(4));
}

#[test]
fn exports_regular_polygon_fixture_with_interactive_seed_segment() {
    let scene = fixture_scene(
        include_bytes!("../../../../tests/fixtures/gsp/static/简单迭代/迭代正多边形.gsp"),
        "regular polygon fixture should compile",
    );

    let lines = scene["lines"]
        .as_array()
        .expect("scene lines should be an array");
    let interactive_segment = lines
        .iter()
        .find(|line| line["binding"]["kind"].as_str() == Some("segment"))
        .expect("expected the payload source edge to remain interactive");
    let start_index = interactive_segment["binding"]["startIndex"]
        .as_u64()
        .and_then(|index| usize::try_from(index).ok())
        .expect("segment start index should serialize as usize");
    let end_index = interactive_segment["binding"]["endIndex"]
        .as_u64()
        .and_then(|index| usize::try_from(index).ok())
        .expect("segment end index should serialize as usize");

    let points = scene["points"]
        .as_array()
        .expect("scene points should be an array");
    assert_eq!(
        points
            .get(start_index)
            .and_then(|point| point["draggable"].as_bool()),
        Some(true),
        "expected the payload seed vertex to remain draggable"
    );
    assert!(
        points.get(end_index).is_some_and(|point| {
            point["binding"]["kind"].as_str() == Some("matrix-apply")
                && point["binding"]["sourceIndex"].as_u64() == Some(start_index as u64)
                && point["binding"]["matrixApply"][0]["kind"].as_str() == Some("rotate")
                && point["binding"]["matrixApply"][0]["parameterName"].as_str() == Some("n")
                && point["binding"]["matrixApply"][0]["angleDegrees"]
                    .as_f64()
                    .is_some_and(|value| (value - 72.0).abs() < 0.01)
                && point["binding"]["matrixApply"][0]["angleExpr"].is_object()
        }),
        "expected the payload rotated endpoint to remain a live rotate-bound point"
    );
    let labels = scene["labels"]
        .as_array()
        .expect("scene labels should be an array");
    assert!(
        labels.iter().any(|label| {
            label["visible"].as_bool() == Some(true)
                && label["text"].as_str() == Some("360° / n = 72.00°")
                && label["binding"]["kind"].as_str() == Some("expression-value")
                && label["binding"]["parameterName"].as_str() == Some("n")
                && label["binding"]["exprLabel"].as_str() == Some("360° / n")
        }),
        "expected the payload angle label to remain bound to the regular-polygon angle expression"
    );
}

#[test]
fn exports_circle_formation_fixture_iteration_table_against_sequence_value() {
    let scene = fixture_scene(
        include_bytes!("../../../../tests/fixtures/圆的形成.gsp"),
        "circle-formation fixture should compile",
    );
    let iteration_tables = scene["iterationTables"]
        .as_array()
        .expect("scene iteration tables should be an array");
    assert_eq!(
        iteration_tables.len(),
        1,
        "expected one visible iteration table"
    );
    assert_eq!(iteration_tables[0]["exprLabel"].as_str(), Some("t₁ + 1"));
    assert_eq!(
        iteration_tables[0]["parameterName"].as_str(),
        Some("t₁"),
        "expected the iteration table to track the sequence value instead of the root control parameter"
    );
    assert_eq!(
        iteration_tables[0]["depthParameterName"].as_str(),
        Some("t₃"),
        "expected the iteration depth to follow the payload's derived iteration count"
    );
}

#[test]
fn exports_circle_formation_fixture_with_non_draggable_rotate_points() {
    let scene = fixture_scene(
        include_bytes!("../../../../tests/fixtures/圆的形成.gsp"),
        "circle-formation fixture should compile",
    );

    let points = scene["points"]
        .as_array()
        .expect("scene points should be an array");
    let rotate_points = points
        .iter()
        .filter(|point| {
            point["binding"]["kind"].as_str() == Some("matrix-apply")
                && point["binding"]["matrixApply"][0]["kind"].as_str() == Some("rotate")
        })
        .collect::<Vec<_>>();

    assert!(
        !rotate_points.is_empty(),
        "expected the payload to export rotate-bound polygon vertices"
    );
    assert!(
        rotate_points
            .iter()
            .all(|point| point["draggable"].as_bool() == Some(false)),
        "expected rotate-bound vertices to stay derived instead of becoming draggable handles"
    );
}

#[test]
fn exports_circle_formation_fixture_without_static_duplicate_labels() {
    let scene = fixture_scene(
        include_bytes!("../../../../tests/fixtures/圆的形成.gsp"),
        "circle-formation fixture should compile",
    );
    let labels = scene["labels"]
        .as_array()
        .expect("scene labels should be an array");
    assert!(
        labels.iter().all(|label| {
            !label["visible"].as_bool().unwrap_or(false) || !label["binding"].is_null()
        }),
        "expected visible labels in the circle-formation fixture to stay payload-bound and interactive"
    );
}
