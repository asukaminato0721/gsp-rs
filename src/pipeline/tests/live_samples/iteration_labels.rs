use super::*;

#[test]
fn exports_ant_fixture_with_two_axis_line_iterations() {
    let scene = fixture_scene(
        include_bytes!("../../../../tests/fixtures/bug/迭代方法2(蚂蚁).gsp"),
        "ant fixture should compile",
    );
    let line_iterations = scene["lineIterations"]
        .as_array()
        .expect("scene line iterations should be an array");
    assert_eq!(
        line_iterations.len(),
        4,
        "expected the four seed edges to stay exported as live translational families"
    );
    assert_eq!(
        line_iterations
            .iter()
            .filter(|family| family["kind"].as_str() == Some("translate"))
            .count(),
        4,
        "expected four translational seed-edge families"
    );
    assert!(line_iterations.iter().all(|family| {
        family["kind"].as_str() == Some("translate")
            && family["parameterName"].as_str() == Some("n")
            && family["dx"].as_f64() == Some(-62.0)
            && family["dy"].as_f64() == Some(-36.0)
            && family["secondaryDx"].as_f64() == Some(47.0)
            && family["secondaryDy"].as_f64() == Some(-52.0)
            && family["bidirectional"].as_bool() == Some(true)
            && family["depth"].as_u64() == Some(3)
    }));
    let points = scene["points"]
        .as_array()
        .expect("scene points should be an array");
    let visible_points = points
        .iter()
        .filter(|point| point["visible"].as_bool() == Some(true))
        .collect::<Vec<_>>();
    assert_eq!(
        visible_points.len(),
        16,
        "expected the payload seed and translated ant points to stay visible"
    );
    assert!(
        visible_points.iter().all(|point| {
            point["constraint"].is_null()
                && matches!(point["binding"]["kind"].as_str(), None | Some("derived"))
        }),
        "expected visible ant helper points to be source or derived geometry"
    );
}

#[test]
fn exports_triangle_centers_fixture_with_named_midpoints_and_black_point_labels() {
    let scene = fixture_scene(
        include_bytes!("../../../../tests/fixtures/未实现的系统功能/三角形的四心.gsp"),
        "triangle-centers fixture should compile",
    );

    let labels = scene["labels"]
        .as_array()
        .expect("scene labels should be an array");

    for name in ["D", "E", "F", "G", "H", "I", "O"] {
        assert!(
            labels.iter().any(|label| {
                label["text"].as_str() == Some(name) && label["visible"].as_bool() == Some(true)
            }),
            "expected visible payload point label {name}"
        );
    }

    let black = vec![
        Value::from(30),
        Value::from(30),
        Value::from(30),
        Value::from(255),
    ];
    for name in ["A", "B", "C", "D", "E", "F", "G", "H", "I", "O"] {
        assert!(
            labels
                .iter()
                .filter(|label| label["text"].as_str() == Some(name))
                .all(|label| label["color"].as_array() == Some(&black)),
            "expected payload point label {name} to keep black text color"
        );
    }
}

#[test]
fn exports_crescent_trace_inrm_fixture_with_live_trace_bindings() {
    let Some(data) = fixture_bytes("tests/fixtures/未实现/月牙形轨迹(inRm).gsp") else {
        return;
    };
    let scene = fixture_scene(&data, "crescent-trace fixture should compile");

    let points = scene["points"]
        .as_array()
        .expect("scene points should be an array");
    assert!(
        points
            .iter()
            .any(|point| point["binding"]["kind"].as_str() == Some("scale-by-ratio")),
        "expected the ratio-scaled point to keep a live binding"
    );

    let lines = scene["lines"]
        .as_array()
        .expect("scene lines should be an array");
    assert!(
        lines
            .iter()
            .any(|line| line["binding"]["kind"].as_str() == Some("point-trace")),
        "expected the payload trace to keep a live trace binding"
    );
    assert!(
        lines
            .iter()
            .any(|line| line["binding"]["kind"].as_str() == Some("segment")),
        "expected the payload segment to remain interactive"
    );
}
