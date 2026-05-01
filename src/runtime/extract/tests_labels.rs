use super::test_support::{fixture_bytes, fixture_scene};
use crate::runtime::scene::{LineBinding, TextLabelBinding};

#[test]
fn exports_one_dragon_fixture_against_javasketch_visibility() {
    let Some(data) = fixture_bytes("tests/Samples/个人专栏/李章博作品/一条龙.gsp")
    else {
        return;
    };
    let scene = fixture_scene(&data);

    assert_eq!(
        scene.points.iter().filter(|point| point.visible).count(),
        2,
        "expected the two red Intersect1/Intersect2 dot points from the paired .htm to stay visible"
    );
    assert!(
        scene
            .buttons
            .iter()
            .filter(|button| {
                button.debug.as_ref().is_some_and(|debug| {
                    matches!(debug.group_ordinal, 10 | 11 | 12 | 14 | 19 | 21 | 26 | 28)
                })
            })
            .all(|button| !button.visible),
        "expected hidden action buttons to remain callable but not rendered as visible controls"
    );
    assert!(
        scene.buttons.iter().any(|button| button
            .debug
            .as_ref()
            .is_some_and(|debug| debug.group_ordinal == 33)
            && button.visible),
        "expected the top-level visible sequence button to remain interactive"
    );
}

#[test]
fn uses_document_canvas_bounds_for_rich_text_triangle_centers_layout() {
    let scene = fixture_scene(include_bytes!(
        "../../../tests/fixtures/未实现的系统功能/三角形的四心.gsp"
    ));

    assert_eq!(scene.bounds.min_x, 0.0);
    assert_eq!(scene.bounds.min_y, 0.0);
    assert_eq!(scene.bounds.max_x, 1850.0);
    assert_eq!(scene.bounds.max_y, 915.0);
    assert!(
        scene
            .labels
            .iter()
            .any(|label| label.text == "三角形的四心"),
        "expected the document title label to still be present"
    );
}

#[test]
fn exports_lizhangbo_exponent_calculator_visible_parameters() {
    let Some(data) = fixture_bytes("tests/Samples/个人专栏/李章博作品/指数计算器（李章博）.gsp")
    else {
        return;
    };
    let scene = fixture_scene(&data);

    let visible_parameters = scene
        .parameters
        .iter()
        .filter(|parameter| parameter.visible)
        .map(|parameter| (parameter.name.as_str(), parameter.value))
        .collect::<Vec<_>>();

    assert_eq!(visible_parameters, vec![("底数", 2.0), ("指数", 200.0)]);
    assert!(
        scene
            .parameters
            .iter()
            .any(|parameter| parameter.name == "a[101]" && !parameter.visible),
        "expected internal digit-carry parameters to stay hidden"
    );
}

#[test]
fn preserves_polygon_labels_in_poly_point_with_val_gsp() {
    let scene = fixture_scene(include_bytes!(
        "../../../tests/fixtures/gsp/static/poly_point_with_val.gsp"
    ));

    assert_eq!(scene.polygons.len(), 1, "expected a single polygon");
    assert_eq!(
        scene.points.len(),
        5,
        "expected four vertices and one constrained point"
    );
    let texts = scene
        .labels
        .iter()
        .map(|label| label.text.as_str())
        .collect::<Vec<_>>();
    assert!(
        texts.contains(&"A"),
        "expected point label A, got {texts:?}"
    );
    assert!(
        texts.contains(&"B"),
        "expected point label B, got {texts:?}"
    );
    assert!(
        texts.contains(&"C"),
        "expected point label C, got {texts:?}"
    );
    assert!(
        texts.contains(&"D"),
        "expected point label D, got {texts:?}"
    );
    assert!(
        texts.contains(&"E"),
        "expected constrained point label E, got {texts:?}"
    );
    assert!(
        texts.contains(&"E在ABCD上的值 = 0.58"),
        "expected polygon parameter label, got {texts:?}"
    );
}

#[test]
fn preserves_segment_parameter_label_in_segment_point_value_gsp() {
    let scene = fixture_scene(include_bytes!(
        "../../../tests/fixtures/gsp/static/segment_point_value.gsp"
    ));

    assert_eq!(scene.lines.len(), 1, "expected one segment");
    assert_eq!(
        scene.points.len(),
        3,
        "expected two endpoints and one constrained point"
    );
    let texts = scene
        .labels
        .iter()
        .map(|label| label.text.as_str())
        .collect::<Vec<_>>();
    assert!(
        texts.contains(&"A"),
        "expected point label A, got {texts:?}"
    );
    assert!(
        texts.contains(&"B"),
        "expected point label B, got {texts:?}"
    );
    assert!(
        texts.contains(&"C"),
        "expected point label C, got {texts:?}"
    );
    assert!(
        texts.contains(&"C在AB上的t值 = 0.51"),
        "expected segment parameter label, got {texts:?}"
    );
}

#[test]
fn preserves_circle_parameter_label_in_circle_point_value_gsp() {
    let scene = fixture_scene(include_bytes!(
        "../../../tests/fixtures/gsp/static/circle_point_value.gsp"
    ));

    assert_eq!(scene.circles.len(), 1, "expected one circle");
    assert_eq!(
        scene.points.len(),
        3,
        "expected center, radius point, and constrained point"
    );
    let texts = scene
        .labels
        .iter()
        .map(|label| label.text.as_str())
        .collect::<Vec<_>>();
    assert!(
        texts.contains(&"A"),
        "expected point label A, got {texts:?}"
    );
    assert!(
        texts.contains(&"B"),
        "expected point label B, got {texts:?}"
    );
    assert!(
        texts.contains(&"C"),
        "expected point label C, got {texts:?}"
    );
    assert!(
        texts.contains(&"C在⊙AB上的值 = 0.38"),
        "expected circle parameter label, got {texts:?}"
    );
}

#[test]
fn preserves_triangle_centers_fixture_point_labels_and_black_text_color() {
    let scene = fixture_scene(include_bytes!(
        "../../../tests/fixtures/未实现的系统功能/三角形的四心.gsp"
    ));

    for name in ["D", "E", "F", "G", "H", "I", "O"] {
        assert!(
            scene
                .labels
                .iter()
                .any(|label| label.visible && label.text == name),
            "expected visible payload point label {name}"
        );
    }

    for name in ["A", "B", "C", "D", "E", "F", "G", "H", "I", "O"] {
        assert!(
            scene
                .labels
                .iter()
                .filter(|label| label.text == name)
                .all(|label| label.color == [30, 30, 30, 255]),
            "expected payload point label {name} to keep black text color"
        );
    }
}

#[test]
fn preserves_point_label_in_point_label_gsp() {
    let scene = fixture_scene(include_bytes!(
        "../../../tests/fixtures/gsp/static/point_label.gsp"
    ));

    assert!(
        scene.labels.iter().any(|label| label.text == "A"),
        "expected point label A, got {:?}",
        scene
            .labels
            .iter()
            .map(|label| &label.text)
            .collect::<Vec<_>>()
    );
}

#[test]
fn preserves_point_and_segment_labels_in_segment_label_gsp() {
    let scene = fixture_scene(include_bytes!(
        "../../../tests/fixtures/gsp/static/segment_label.gsp"
    ));

    let texts = scene
        .labels
        .iter()
        .map(|label| label.text.as_str())
        .collect::<Vec<_>>();
    assert!(
        texts.contains(&"A"),
        "expected point label A, got {texts:?}"
    );
    assert!(
        texts.contains(&"B"),
        "expected point label B, got {texts:?}"
    );
    assert!(
        texts.contains(&"j"),
        "expected segment label j, got {texts:?}"
    );
}

#[test]
fn preserves_angle_marker_label_in_angle_marker_label_gsp() {
    let scene = fixture_scene(include_bytes!(
        "../../../tests/fixtures/gsp/static/angle_marker_label.gsp"
    ));

    let texts = scene
        .labels
        .iter()
        .map(|label| label.text.as_str())
        .collect::<Vec<_>>();
    assert!(
        texts.contains(&"42.5"),
        "expected payload-backed angle marker label, got {texts:?}"
    );
    assert!(
        scene
            .lines
            .iter()
            .any(|line| matches!(line.binding, Some(LineBinding::AngleMarker { .. }))),
        "expected angle marker to stay interactive"
    );
    assert!(scene.labels.iter().any(|label| matches!(
        label.binding,
        Some(TextLabelBinding::AngleMarkerValue {
            start_index: 1,
            vertex_index: 0,
            end_index: 2,
            decimals: 1,
        })
    )));
}

#[test]
fn preserves_visible_and_hidden_ray_labels_from_payload() {
    let scene = fixture_scene(include_bytes!(
        "../../../tests/fixtures/gsp/static/ray_label_hide.gsp"
    ));

    assert_eq!(
        scene.labels.len(),
        2,
        "expected both ray labels in the scene"
    );
    assert!(
        scene
            .labels
            .iter()
            .any(|label| label.text == "j" && label.visible),
        "expected ray label j to remain visible"
    );
    assert!(
        scene
            .labels
            .iter()
            .any(|label| label.text == "k" && !label.visible),
        "expected ray label k to remain hidden based on the 0x07d5 payload flag"
    );
    assert!(
        scene.lines.iter().all(|line| line.visible),
        "expected hidden state to apply to the label only, not the ray geometry"
    );
}

#[test]
fn keeps_control_labels_in_non_graph_sample() {
    let scene = fixture_scene(include_bytes!(
        "../../../tests/Samples/个人专栏/潘建平作品/加油潘建平老师.gsp"
    ));

    assert!(
        scene.labels.iter().any(|label| label.text.contains("单价")),
        "expected UI text label from rich text payload, got {:?}",
        scene
            .labels
            .iter()
            .map(|label| label.text.as_str())
            .collect::<Vec<_>>()
    );
}
