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
    let runtime_sources = include_str!(concat!(env!("OUT_DIR"), "/viewer-runtime.js"));

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

    let allowed_missing = BTreeSet::new();
    assert_eq!(
        runtime_missing, allowed_missing,
        "exported payload kinds should have explicit JS runtime coverage unless intentionally static",
    );
}

#[test]
fn circle_constraint_runtime_has_single_resolver_implementation() {
    let runtime_sources = include_str!(concat!(env!("OUT_DIR"), "/viewer-runtime.js"));
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
fn viewer_does_not_infer_payload_links_from_geometry() {
    let viewer = include_str!("../../html/runtime/viewer.ts");

    assert!(!viewer.contains("attachPointRef"));
    assert!(!viewer.contains("attachLabelAnchor"));
    assert!(!viewer.contains("pointMatchTolerance"));
    assert!(!viewer.contains("labelAttachDistance"));
    assert!(viewer.contains("function explicitLabelAnchor("));
}

#[test]
fn runtime_has_no_synthetic_animation_or_empty_module_fallbacks() {
    let overlay = include_str!("../../html/runtime/viewer_overlay.ts");
    let render = include_str!("../../html/runtime/viewer_render_basic.ts");
    let scene = include_str!("../../html/runtime/viewer_scene_basic.ts");
    let dynamics = include_str!("../../html/runtime/viewer_dynamics.ts");

    assert!(!overlay.contains("Math.random"));
    assert!(!overlay.contains("Math.sin(state.t)"));
    assert!(!overlay.contains("const durationMs"));
    assert!(!overlay.contains("dt / 700"));
    assert!(overlay.contains("point.animation.speed"));
    assert!(!render.contains("function drawImages(_env"));
    assert!(!render.contains("function findHitLabel()"));
    assert!(!scene.contains("function lineLineIntersection()"));
    assert_eq!(
        scene.matches("function resolveAngleMarkerPoints(").count()
            + dynamics
                .matches("function resolveAngleMarkerPoints(")
                .count(),
        1,
    );
}

#[test]
fn runtime_validates_json_and_uses_strict_runtime_shapes() {
    let document = include_str!("../../html/runtime/viewer_app_document.ts");
    let types = include_str!("../../html/viewer_types.d.ts");

    assert!(document.contains("function assertSceneData("));
    assert!(!document.contains("raw as SceneData"));
    assert!(!types.contains("type RuntimeLineJson = Partial<"));
    assert!(!types.contains("type RuntimePolygonJson = Partial<"));
    assert!(!types.contains("type RuntimeCircleJson = Partial<"));
}

#[test]
fn compiler_has_no_fixed_spectrum_viewport_or_color_guessing() {
    let build = include_str!("../../runtime/extract/build.rs");
    let trace = include_str!("../../runtime/extract/trace.rs");
    let bindings = include_str!("../../runtime/extract/bindings.rs");

    assert!(!build.contains("800.0"));
    assert!(!trace.contains("viewport_width"));
    assert!(!bindings.contains("color_distance"));
    assert!(bindings.contains("COLORIZED_RGB_OPCODE => Some(PayloadColorModel::Rgb)"));
    assert!(bindings.contains("COLORIZED_HSV_OPCODE => Some(PayloadColorModel::Hsv)"));
}

#[test]
fn compiler_does_not_synthesize_function_axes_or_plot_labels() {
    let shapes = include_str!("../../runtime/extract/shapes.rs");
    let labels = include_str!("../../runtime/extract/labels.rs");
    let functions = include_str!("../../runtime/functions.rs");

    assert!(!shapes.contains("synthesize_function_axes"));
    assert!(!labels.contains("synthesize_function_labels"));
    assert!(!functions.contains("synthesize_function_axes"));
    assert!(!functions.contains("synthesize_function_labels"));
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
