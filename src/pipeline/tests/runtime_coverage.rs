use super::*;

#[test]
fn js_runtime_covers_exported_payload_kinds() {
    let generated_sources = [
        include_str!("../../html/generated/PointBindingJson.ts"),
        include_str!("../../html/generated/PointConstraintJson.ts"),
        include_str!("../../html/generated/LineBindingJson.ts"),
        include_str!("../../html/generated/LineConstraintJson.ts"),
        include_str!("../../html/generated/ShapeBindingJson.ts"),
        include_str!("../../html/generated/LabelBindingJson.ts"),
        include_str!("../../html/generated/LabelHotspotActionJson.ts"),
        include_str!("../../html/generated/ButtonActionJson.ts"),
        include_str!("../../html/generated/CircularConstraintJson.ts"),
        include_str!("../../html/generated/PointIterationJson.ts"),
        include_str!("../../html/generated/LineIterationJson.ts"),
        include_str!("../../html/generated/PolygonIterationJson.ts"),
    ];
    let runtime_sources = include_str!("../../html/generated/viewer-runtime.js");

    let exported_kinds = generated_sources
        .into_iter()
        .flat_map(collect_kind_literals)
        .collect::<BTreeSet<_>>();
    let runtime_missing = exported_kinds
        .into_iter()
        .filter(|kind| {
            !runtime_sources.contains(&format!("\"{kind}\""))
                && !runtime_sources.contains(&format!("'{kind}'"))
        })
        .collect::<BTreeSet<_>>();

    let allowed_missing = BTreeSet::from(["function-label".to_string()]);
    assert_eq!(
        runtime_missing, allowed_missing,
        "exported payload kinds should have explicit JS runtime coverage unless intentionally static",
    );
}

#[test]
fn circle_constraint_runtime_has_single_resolver_implementation() {
    let runtime_sources = include_str!("../../html/generated/viewer-runtime.js");
    let resolver_definitions = runtime_sources
        .matches("function circleFromConstraint(")
        .count();

    assert_eq!(
        resolver_definitions, 1,
        "circle constraints should be resolved by one shared scene implementation",
    );
    let scene_circular = include_str!("../../html/runtime/viewer_scene_circular.ts");
    assert!(
        !scene_circular.contains("_circleFromConstraint:"),
        "circular scene addon should not replace the shared circle constraint resolver",
    );
}

#[test]
fn compiles_fixed_coordinate_and_slope_helper_fixtures() {
    let Some(cardioid) = fixture_bytes("tests/Samples/未分类档/心脏线.gsp") else {
        return;
    };
    let cardioid_scene = fixture_scene_json(&cardioid, "cardioid fixture should compile");
    assert!(cardioid_scene.contains("\"points\": ["));

    let Some(buffon) = fixture_bytes("tests/Samples/热研系列/概率问题/蒲丰投针实验求π的近似值.gsp")
    else {
        return;
    };
    let buffon_scene = fixture_scene_json(&buffon, "buffon fixture should compile");
    assert!(buffon_scene.contains("\"labels\": ["));
}

#[test]
fn compiles_legacy_constructed_point_and_angle_rotation_fixtures() {
    let Some(rolling_ellipse) =
        fixture_bytes("tests/Samples/个人专栏/贺基旭作品/椭圆在直线上滚动(优化).gsp")
    else {
        return;
    };
    let rolling_ellipse_scene =
        fixture_scene_json(&rolling_ellipse, "rolling ellipse fixture should compile");
    assert!(rolling_ellipse_scene.contains("\"points\": ["));

    let Some(cycloid) = fixture_bytes("tests/Samples/个人专栏/方小庆作品/(inRm)摆线.gsp")
    else {
        return;
    };
    let cycloid_scene = fixture_scene_json(&cycloid, "cycloid fixture should compile");
    assert!(cycloid_scene.contains("\"lines\": ["));
}
