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
    let runtime_sources = [
        include_str!("../../html/viewer.js"),
        include_str!("../../html/viewer_drag.js"),
        include_str!("../../html/viewer_drag_pan.js"),
        include_str!("../../html/viewer_dynamics.js"),
        include_str!("../../html/viewer_overlay.js"),
        include_str!("../../html/viewer_render_basic.js"),
        include_str!("../../html/viewer_render_circular.js"),
        include_str!("../../html/viewer_render_hotspots.js"),
        include_str!("../../html/viewer_render_images.js"),
        include_str!("../../html/viewer_render_labels.js"),
        include_str!("../../html/viewer_render_polygons.js"),
        include_str!("../../html/viewer_render_tables.js"),
        include_str!("../../html/viewer_scene_basic.js"),
        include_str!("../../html/viewer_scene_circular.js"),
        include_str!("../../html/viewer_scene_intersections.js"),
        include_str!("../../html/viewer_scene_trace.js"),
    ]
    .join("\n");

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
fn compiles_legacy_arc_measure_helper_fixtures() {
    let Some(clock) = fixture_bytes("tests/Samples/个人专栏/侯仰顺作品/时钟.gsp") else {
        return;
    };
    let clock_scene = fixture_scene_json(&clock, "clock fixture should compile");
    assert!(clock_scene.contains("\"points\": ["));

    let Some(rolling) = fixture_bytes("tests/Samples/个人专栏/况永胜作品/正多边形在圆外滚动.gsp")
    else {
        return;
    };
    let rolling_scene = fixture_scene_json(&rolling, "rolling polygon fixture should compile");
    assert!(rolling_scene.contains("\"lines\": ["));
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
