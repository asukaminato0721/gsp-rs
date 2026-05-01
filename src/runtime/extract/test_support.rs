use super::context::SceneContext;
use super::{
    analyze_scene, build_scene_checked, collect_buttons, collect_point_objects,
    collect_scene_labels, collect_scene_shapes, collect_visible_points_checked,
    remap_scene_bindings, render_payload_log,
};
use crate::format::GspFile;
use crate::runtime::functions::{
    FunctionAst, FunctionExpr, UnaryFunction, evaluate_expr_with_parameters, function_expr_label,
};
use crate::runtime::scene::{Scene, SceneButton, TextLabelBinding};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

pub(super) fn fixture_scene(data: &[u8]) -> Scene {
    let file = GspFile::parse(data).expect("fixture parses");
    build_scene_checked(&file).expect("scene builds")
}

pub(super) fn fixture_log(data: &[u8], source_path: &str) -> String {
    let file = GspFile::parse(data).expect("fixture parses");
    render_payload_log(Path::new(source_path), &file)
}

pub(super) fn fixture_bytes(path: &str) -> Option<Vec<u8>> {
    fs::read(path).ok()
}

pub(super) fn derive_expression_label_parameters(
    scene: &Scene,
    seed: BTreeMap<String, f64>,
) -> BTreeMap<String, f64> {
    let mut parameters = seed;
    for _ in 0..scene.labels.len().max(16) {
        let mut changed = false;
        for label in &scene.labels {
            let (result_name, expr_label, expr) = match label.binding.as_ref() {
                Some(TextLabelBinding::ExpressionValue {
                    result_name,
                    expr_label,
                    expr,
                    ..
                })
                | Some(TextLabelBinding::PointBoundExpressionValue {
                    result_name,
                    expr_label,
                    expr,
                    ..
                }) => (result_name, expr_label, expr),
                _ => continue,
            };
            let Some(value) = evaluate_expr_with_parameters(expr, 0.0, &parameters) else {
                continue;
            };
            let mut names = vec![expr_label.clone(), function_expr_label(expr.clone())];
            if let Some(result_name) = result_name {
                names.push(result_name.clone());
            }
            for name in names {
                if parameters.get(&name).copied() != Some(value) {
                    parameters.insert(name, value);
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
    }
    parameters
}

pub(super) fn function_expr_has_unary(expr: &FunctionExpr, op: UnaryFunction) -> bool {
    match expr {
        FunctionExpr::Parsed(ast) => function_ast_has_unary(ast, op),
        _ => false,
    }
}

fn function_ast_has_unary(ast: &FunctionAst, op: UnaryFunction) -> bool {
    match ast {
        FunctionAst::Unary { op: ast_op, expr } => {
            *ast_op == op || function_ast_has_unary(expr, op)
        }
        FunctionAst::Binary { lhs, rhs, .. } => {
            function_ast_has_unary(lhs, op) || function_ast_has_unary(rhs, op)
        }
        _ => false,
    }
}

pub(super) fn function_expr_has_parameter(expr: &FunctionExpr, expected: &str) -> bool {
    match expr {
        FunctionExpr::Parsed(ast) => function_ast_has_parameter(ast, expected),
        _ => false,
    }
}

fn function_ast_has_parameter(ast: &FunctionAst, expected: &str) -> bool {
    match ast {
        FunctionAst::Parameter(name, _) => name == expected,
        FunctionAst::Unary { expr, .. } => function_ast_has_parameter(expr, expected),
        FunctionAst::Binary { lhs, rhs, .. } => {
            function_ast_has_parameter(lhs, expected) || function_ast_has_parameter(rhs, expected)
        }
        _ => false,
    }
}

pub(super) fn fixture_buttons_without_validation(data: &[u8]) -> Vec<SceneButton> {
    let file = GspFile::parse(data).expect("fixture parses");
    let groups = file.object_groups();
    let context = SceneContext::new(&file, &groups);
    let point_map = collect_point_objects(&file, &groups);
    let analysis = analyze_scene(&file, &groups, &context, &point_map);
    let mut shapes = collect_scene_shapes(&file, &groups, &point_map, &analysis);
    let (_, image_group_to_index) =
        super::images::collect_scene_images(&file, &groups, &analysis.graph_ref);
    let (_, label_group_to_index, _) =
        collect_scene_labels(&file, &groups, &context, &analysis, &shapes);
    let (_, group_to_point_index) = collect_visible_points_checked(
        &file,
        &groups,
        &point_map,
        &analysis.raw_anchors,
        &analysis.graph_ref,
    )
    .expect("visible points build");
    let (binding_maps, _, _) = remap_scene_bindings(
        &file,
        &groups,
        &analysis.raw_anchors,
        &group_to_point_index,
        &mut shapes,
    );
    let (buttons, _) = collect_buttons(
        &file,
        &groups,
        &analysis.raw_anchors,
        super::buttons::ButtonIndexLookups {
            label_group_to_index: &label_group_to_index,
            image_group_to_index: &image_group_to_index,
            group_to_point_index: &group_to_point_index,
            line_group_to_index: &binding_maps.line_group_to_index,
            circle_group_to_index: &binding_maps.circle_group_to_index,
            polygon_group_to_index: &binding_maps.polygon_group_to_index,
        },
    );
    buttons
}

pub(super) fn fixture_labels_without_validation(
    data: &[u8],
) -> Vec<crate::runtime::scene::TextLabel> {
    let file = GspFile::parse(data).expect("fixture parses");
    let groups = file.object_groups();
    let context = SceneContext::new(&file, &groups);
    let point_map = collect_point_objects(&file, &groups);
    let analysis = analyze_scene(&file, &groups, &context, &point_map);
    let mut shapes = collect_scene_shapes(&file, &groups, &point_map, &analysis);
    let (_, image_group_to_index) =
        super::images::collect_scene_images(&file, &groups, &analysis.graph_ref);
    let (labels, label_group_to_index, _) =
        collect_scene_labels(&file, &groups, &context, &analysis, &shapes);
    let (_, group_to_point_index) = collect_visible_points_checked(
        &file,
        &groups,
        &point_map,
        &analysis.raw_anchors,
        &analysis.graph_ref,
    )
    .expect("visible points build");
    let (binding_maps, _, _) = remap_scene_bindings(
        &file,
        &groups,
        &analysis.raw_anchors,
        &group_to_point_index,
        &mut shapes,
    );
    let _ = collect_buttons(
        &file,
        &groups,
        &analysis.raw_anchors,
        super::buttons::ButtonIndexLookups {
            label_group_to_index: &label_group_to_index,
            image_group_to_index: &image_group_to_index,
            group_to_point_index: &group_to_point_index,
            line_group_to_index: &binding_maps.line_group_to_index,
            circle_group_to_index: &binding_maps.circle_group_to_index,
            polygon_group_to_index: &binding_maps.polygon_group_to_index,
        },
    );
    labels
}

pub(super) fn fixture_images_without_validation(
    data: &[u8],
) -> Vec<crate::runtime::scene::SceneImage> {
    let file = GspFile::parse(data).expect("fixture parses");
    let groups = file.object_groups();
    let context = SceneContext::new(&file, &groups);
    let point_map = collect_point_objects(&file, &groups);
    let analysis = analyze_scene(&file, &groups, &context, &point_map);
    super::images::collect_scene_images(&file, &groups, &analysis.graph_ref).0
}

pub(super) fn assert_supported_sample_log(path: &str) {
    let Some(data) = fixture_bytes(path) else {
        return;
    };
    let log = fixture_log(&data, path);
    assert!(
        log.contains("问题数量: 0"),
        "expected {path} to stop reporting unsupported helper payloads, got:\n{log}"
    );
    assert!(
        log.contains("未发现不支持的载荷。"),
        "expected {path} to have a clean payload log"
    );
}
