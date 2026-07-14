use std::path::Path;

use gsp_rs::pipeline::compile_file_to_scene_json;
use serde_json::Value;

fn compile_fixture(path: &str) -> Value {
    let json = compile_file_to_scene_json(Path::new(path), 960, 640).unwrap();
    serde_json::from_str(&json).unwrap()
}

fn operation_kind<'a>(scene: &'a Value, id: &str) -> Option<&'a str> {
    scene["objectGraph"]["nodes"]
        .as_array()?
        .iter()
        .find(|node| node["id"] == id)?["definition"]["op"]["kind"]
        .as_str()
}

fn object_id_for_group(scene: &Value, collection: &str, prefix: &str, ordinal: u64) -> String {
    let index = scene[collection]
        .as_array()
        .unwrap_or_else(|| panic!("missing {collection} collection"))
        .iter()
        .position(|object| object["debug"]["groupOrdinal"].as_u64() == Some(ordinal))
        .unwrap_or_else(|| panic!("missing {collection} object for group #{ordinal}"));
    format!("{prefix}:{index}")
}

fn assert_no_graph_validation_errors(scene: &Value) {
    assert!(
        scene["objectGraph"]["pendingOperations"]
            .as_array()
            .expect("pending operation list")
            .iter()
            .all(|pending| !pending
                .as_str()
                .is_some_and(|pending| pending.starts_with("graph-validation:")))
    );
}

#[test]
fn parameter_controlled_locus_point_uses_its_payload_expression_parents() {
    let document = compile_fixture("tests/Samples/个人专栏/孟令岩作品/勾股定理小题.gsp");
    let scene = &document["pages"][0]["scene"];
    let point = object_id_for_group(scene, "points", "point", 32);
    let source = object_id_for_group(scene, "points", "point", 29);
    let ratio = object_id_for_group(scene, "labels", "scalar:label", 28);
    assert_eq!(operation_kind(scene, &point), Some("point-on-polyline"));
    let nested_trace = object_id_for_group(scene, "lines", "line", 39);
    assert_eq!(operation_kind(scene, &nested_trace), Some("point-trace"));
    assert!(
        scene["objectGraph"]["pendingOperations"]
            .as_array()
            .unwrap()
            .iter()
            .all(|pending| !pending
                .as_str()
                .is_some_and(|pending| pending.starts_with(&format!("{point}:"))))
    );

    let scalar_id = format!("scalar:{point}:constraint-parameter");
    let scalar = scene["objectGraph"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["id"] == scalar_id)
        .expect("the locus parameter is evaluated from its payload expression");
    assert_eq!(
        scalar["definition"]["op"]["parameter_names"],
        serde_json::json!(["F", "1"])
    );
    assert_eq!(scalar["definition"]["parents"][1], ratio);
    let source_parameter = scalar["definition"]["parents"][0]
        .as_str()
        .expect("point parameter parent");
    let source_parameter_node = scene["objectGraph"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["id"] == source_parameter)
        .expect("the F parent is derived from the locus point");
    assert_eq!(
        source_parameter_node["definition"]["parents"],
        serde_json::json!([format!("control:{source}:t")])
    );
}

#[test]
fn nested_measurements_drive_rotation_trace_and_controlled_point() {
    let document = compile_fixture("tests/Samples/个人专栏/孟令岩作品/勾股定理小题.gsp");
    let scene = &document["pages"][1]["scene"];
    assert_eq!(
        scene["objectGraph"]["pendingOperations"],
        serde_json::json!([])
    );

    let parameter_rotation = object_id_for_group(scene, "points", "point", 55);
    let linear_intersection = object_id_for_group(scene, "points", "point", 59);
    let first_expression_rotation = object_id_for_group(scene, "points", "point", 60);
    let second_expression_rotation = object_id_for_group(scene, "points", "point", 68);
    let controlled_trace_point = object_id_for_group(scene, "points", "point", 85);
    assert_eq!(
        operation_kind(scene, &parameter_rotation),
        Some("rotate-point-degrees")
    );
    assert_eq!(
        operation_kind(scene, &linear_intersection),
        Some("line-intersection")
    );
    assert_eq!(
        operation_kind(scene, &first_expression_rotation),
        Some("rotate-point-degrees")
    );
    assert_eq!(
        operation_kind(scene, &second_expression_rotation),
        Some("rotate-point-degrees")
    );
    assert_eq!(
        operation_kind(scene, &controlled_trace_point),
        Some("point-on-polyline")
    );

    let measured_p_arrow = object_id_for_group(scene, "labels", "scalar:label", 53);
    let measured_bc = object_id_for_group(scene, "labels", "scalar:label", 36);
    let rotation_scalar_id = format!("scalar:{parameter_rotation}:rotation-degrees");
    let rotation_scalar = scene["objectGraph"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["id"] == rotation_scalar_id)
        .expect("parameter rotation scalar");
    let parents = rotation_scalar["definition"]["parents"].as_array().unwrap();
    assert!(parents.contains(&Value::String(measured_p_arrow)));
    assert!(parents.contains(&Value::String(measured_bc)));
}

#[test]
fn angle_rotated_segment_intersects_legacy_radius_circle() {
    let document = compile_fixture("tests/Samples/个人专栏/孟令岩作品/勾股定理小题.gsp");
    let scene = &document["pages"][2]["scene"];
    assert_eq!(
        scene["objectGraph"]["pendingOperations"],
        serde_json::json!([])
    );

    let intersection = object_id_for_group(scene, "points", "point", 30);
    assert_eq!(
        operation_kind(scene, &intersection),
        Some("line-circle-intersection")
    );

    let line_id = format!("domain:{intersection}:line");
    let circle_id = format!("domain:{intersection}:circle");
    assert_eq!(
        operation_kind(scene, &line_id),
        Some("rotate-shape-degrees")
    );
    assert_eq!(
        operation_kind(scene, &circle_id),
        Some("circle-by-segment-radius")
    );

    let center = object_id_for_group(scene, "points", "point", 1);
    let source_circle_center = object_id_for_group(scene, "points", "point", 24);
    let circle = scene["objectGraph"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["id"] == circle_id)
        .expect("legacy radius circle domain");
    assert_eq!(
        circle["definition"]["parents"],
        serde_json::json!([center, source_circle_center, center])
    );

    let angle_scalar_id = format!("scalar:{line_id}:rotation-degrees");
    assert_eq!(
        operation_kind(scene, &angle_scalar_id),
        Some("measured-rotation-degrees")
    );
}

#[test]
fn coordinate_value_parent_drives_the_payload_vector_translation() {
    let document = compile_fixture("tests/Samples/个人专栏/向忠作品/y=Asin(wx+v).gsp");
    for (page_index, page) in document["pages"].as_array().unwrap().iter().enumerate() {
        assert_eq!(
            page["scene"]["objectGraph"]["geometryComplete"],
            true,
            "page {} pending: {}",
            page_index + 1,
            page["scene"]["objectGraph"]["pendingOperations"]
        );
        assert_eq!(
            page["scene"]["objectGraph"]["pendingOperations"],
            serde_json::json!([])
        );
    }

    let scene = &document["pages"][1]["scene"];
    let translated = object_id_for_group(scene, "points", "point", 252);
    let source = object_id_for_group(scene, "points", "point", 251);
    let vector_start = object_id_for_group(scene, "points", "point", 1);
    assert_eq!(operation_kind(scene, &translated), Some("translate-point"));
    let node = scene["objectGraph"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["id"] == translated)
        .unwrap();
    assert_eq!(node["definition"]["parents"][0], source);
    assert_eq!(node["definition"]["parents"][1], vector_start);
    assert_eq!(node["definition"]["parents"][2], source);
}

#[test]
fn coordinate_expression_parameter_drives_the_full_hebei_construction() {
    let scene = compile_fixture("tests/Samples/个人专栏/周维波作品/2010年河北25题1.gsp");
    assert_eq!(scene["objectGraph"]["geometryComplete"], true);
    assert_eq!(
        scene["objectGraph"]["pendingOperations"],
        serde_json::json!([])
    );

    for (ordinal, kind) in [
        (4, "point-scaled-offset"),
        (20, "point-on-line"),
        (23, "point-on-polygon-boundary"),
        (24, "rotate-point-degrees"),
        (33, "line-intersection"),
        (34, "line-intersection"),
    ] {
        let point_id = object_id_for_group(&scene, "points", "point", ordinal);
        assert_eq!(operation_kind(&scene, &point_id), Some(kind));
    }

    let a = scene["points"]
        .as_array()
        .unwrap()
        .iter()
        .find(|point| point["debug"]["groupOrdinal"] == 1)
        .unwrap();
    let b = scene["points"]
        .as_array()
        .unwrap()
        .iter()
        .find(|point| point["debug"]["groupOrdinal"] == 4)
        .unwrap();
    let expected_height = 3.0_f64.sqrt() * 3.0 * 37.795_275_590_551_18;
    assert!((b["x"].as_f64().unwrap() - a["x"].as_f64().unwrap()).abs() < 1e-9);
    assert!((b["y"].as_f64().unwrap() - a["y"].as_f64().unwrap() - expected_height).abs() < 1e-6);
}

#[test]
fn directed_angle_anchor_and_reflected_arc_are_fully_table_driven() {
    let scene =
        compile_fixture("tests/Samples/个人专栏/周维波作品/角平分线的尺规作图（雪山飞狐）.gsp");
    assert_eq!(scene["objectGraph"]["geometryComplete"], true);
    assert_eq!(
        scene["objectGraph"]["pendingOperations"],
        serde_json::json!([])
    );

    let anchor_id = object_id_for_group(&scene, "points", "point", 12);
    assert_eq!(
        operation_kind(&scene, &anchor_id),
        Some("directed-angle-anchor")
    );

    let reflected_arc_id = object_id_for_group(&scene, "arcs", "arc", 61);
    assert_eq!(
        operation_kind(&scene, &reflected_arc_id),
        Some("reflect-shape-across-line")
    );

    let controlled_point_id = object_id_for_group(&scene, "points", "point", 71);
    assert_eq!(
        operation_kind(&scene, &controlled_point_id),
        Some("point-on-arc")
    );
    assert_eq!(
        operation_kind(&scene, &format!("domain:{controlled_point_id}")),
        Some("reflect-shape-across-line")
    );

    let result_arc_id = object_id_for_group(&scene, "arcs", "arc", 72);
    assert_eq!(operation_kind(&scene, &result_arc_id), Some("center-arc"));
    let result_arc_node = scene["objectGraph"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["id"] == result_arc_id)
        .unwrap();
    assert!(
        result_arc_node["definition"]["parents"]
            .as_array()
            .unwrap()
            .iter()
            .any(|parent| parent == &controlled_point_id)
    );

    let expression_circle_intersection_id = object_id_for_group(&scene, "points", "point", 45);
    assert_eq!(
        operation_kind(&scene, &expression_circle_intersection_id),
        Some("line-circle-intersection")
    );
    let distance_scalar_id = object_id_for_group(&scene, "labels", "scalar:label", 42);
    let radius_scalar_id =
        format!("scalar:domain:{expression_circle_intersection_id}:circle:radius");
    let radius_node = scene["objectGraph"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["id"] == radius_scalar_id)
        .expect("expression-radius scalar node");
    assert_eq!(
        radius_node["definition"]["parents"],
        serde_json::json!([distance_scalar_id])
    );

    let expression_rotation_id = object_id_for_group(&scene, "points", "point", 68);
    assert_eq!(
        operation_kind(&scene, &expression_rotation_id),
        Some("rotate-point-degrees")
    );
    let parameter_anchor_scalar_id = object_id_for_group(&scene, "labels", "scalar:label", 65);
    let rotation_scalar_id = format!("scalar:{expression_rotation_id}:rotation-degrees");
    let rotation_scalar_node = scene["objectGraph"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["id"] == rotation_scalar_id)
        .expect("expression-rotation scalar node");
    assert_eq!(
        rotation_scalar_node["definition"]["parents"],
        serde_json::json!([parameter_anchor_scalar_id])
    );

    let final_intersection_id = object_id_for_group(&scene, "points", "point", 87);
    assert_eq!(
        operation_kind(&scene, &final_intersection_id),
        Some("line-intersection")
    );
}

#[test]
fn point_on_rotated_circle_drives_the_complete_gear_chain() {
    let scene = compile_fixture("tests/Samples/个人专栏/况永胜作品/转动的齿轮 (1).gsp");
    assert_eq!(scene["objectGraph"]["geometryComplete"], true);
    assert_eq!(
        scene["objectGraph"]["pendingOperations"],
        serde_json::json!([])
    );

    let constrained_point_id = object_id_for_group(&scene, "points", "point", 48);
    assert_eq!(
        operation_kind(&scene, &constrained_point_id),
        Some("point-on-circle")
    );
    assert_eq!(
        operation_kind(&scene, &format!("domain:{constrained_point_id}")),
        Some("rotate-shape-degrees")
    );

    let measured_radius_circle_id = object_id_for_group(&scene, "circles", "circle", 50);
    assert_eq!(
        operation_kind(&scene, &measured_radius_circle_id),
        Some("circle-by-segment-radius")
    );

    let intersection_id = object_id_for_group(&scene, "points", "point", 51);
    assert_eq!(
        operation_kind(&scene, &intersection_id),
        Some("line-circle-intersection")
    );
}

#[test]
fn polygon_boundary_intersection_drives_the_sphere_construction() {
    let scene = compile_fixture("tests/Samples/个人专栏/方小庆作品/三维球(inRm).gsp");
    assert_eq!(scene["objectGraph"]["geometryComplete"], true);
    assert_eq!(
        scene["objectGraph"]["pendingOperations"],
        serde_json::json!([])
    );

    let boundary_id = object_id_for_group(&scene, "points", "point", 73);
    assert_eq!(
        operation_kind(&scene, &boundary_id),
        Some("line-polyline-intersection")
    );
    let perpendicular_id = object_id_for_group(&scene, "lines", "line", 74);
    assert_eq!(
        operation_kind(&scene, &perpendicular_id),
        Some("perpendicular-line")
    );
    for ordinal in [75, 76] {
        let intersection_id = object_id_for_group(&scene, "points", "point", ordinal);
        assert_eq!(
            operation_kind(&scene, &intersection_id),
            Some("line-circle-intersection")
        );
    }
}

#[test]
fn coordinate_trace_intersection_is_a_complete_typed_graph() {
    let scene = compile_fixture("tests/fixtures/gsp/insection/cood_intersection.gsp");
    assert_eq!(scene["objectGraph"]["geometryComplete"], true);
    assert_eq!(
        operation_kind(&scene, "point:4"),
        Some("point-scaled-offset")
    );
    assert_eq!(operation_kind(&scene, "line:2"), Some("coordinate-trace"));
    assert_eq!(
        operation_kind(&scene, "point:5"),
        Some("line-polyline-intersection")
    );
}

#[test]
fn non_graph_coordinate_trace_drives_points_lines_and_intersections() {
    let document = compile_fixture("tests/Samples/个人专栏/向忠作品/平面截圆柱面的展开图.gsp");
    let scene = &document["pages"][1]["scene"];
    assert_eq!(scene["objectGraph"]["geometryComplete"], true);
    assert_eq!(
        scene["objectGraph"]["pendingOperations"],
        serde_json::json!([])
    );

    let trace_id = object_id_for_group(scene, "lines", "line", 57);
    assert_eq!(operation_kind(scene, &trace_id), Some("point-trace"));

    let trace_point_id = object_id_for_group(scene, "points", "point", 90);
    assert_eq!(
        operation_kind(scene, &trace_point_id),
        Some("point-on-polyline")
    );

    let parallel_id = object_id_for_group(scene, "lines", "line", 91);
    assert_eq!(operation_kind(scene, &parallel_id), Some("parallel-line"));

    let intersection_id = object_id_for_group(scene, "points", "point", 92);
    assert_eq!(
        operation_kind(scene, &intersection_id),
        Some("line-polyline-intersection")
    );
}

#[test]
fn derived_parameter_points_share_the_source_segment_parameter() {
    let scene = compile_fixture("tests/fixtures/gsp/static/point_segment_value_segment_point.gsp");
    assert_eq!(scene["objectGraph"]["geometryComplete"], true);
    assert_eq!(operation_kind(&scene, "point:5"), Some("point-on-line"));
    assert_eq!(
        operation_kind(&scene, "point:8"),
        Some("point-on-circle-parameter")
    );
    assert_eq!(
        operation_kind(&scene, "point:14"),
        Some("point-on-polygon-boundary")
    );
}

#[test]
fn segment_markers_are_derived_from_their_host_endpoints() {
    let scene = compile_fixture("tests/fixtures/gsp/两个三角形标记全等.gsp");
    assert_eq!(scene["objectGraph"]["geometryComplete"], true);
    for index in 10..=15 {
        assert_eq!(
            operation_kind(&scene, &format!("line:{index}")),
            Some("segment-marker")
        );
    }
}

#[test]
fn parametric_curve_is_sampled_by_the_operation_table() {
    let scene = compile_fixture("tests/fixtures/gsp/static/parameter_curve2.gsp");
    assert_eq!(scene["objectGraph"]["geometryComplete"], true);
    assert_eq!(operation_kind(&scene, "line:3"), Some("parametric-curve"));
}

#[test]
fn three_point_arc_intersection_uses_arc_parents() {
    let scene = compile_fixture("tests/fixtures/gsp/static/three_point_arc_intersection.gsp");
    assert_eq!(scene["objectGraph"]["geometryComplete"], true);
    assert_eq!(
        operation_kind(&scene, "point:6"),
        Some("circle-circle-intersection")
    );
}

#[test]
fn arc_boundary_point_uses_the_derived_boundary_line() {
    let scene = compile_fixture("tests/fixtures/未实现的系统功能/弓形周界动点.gsp");
    assert_eq!(scene["objectGraph"]["geometryComplete"], true);
    assert_eq!(operation_kind(&scene, "point:3"), Some("point-on-polyline"));
}

#[test]
fn function_plot_line_depends_on_expression_parameters() {
    let scene = compile_fixture("tests/fixtures/未实现的系统功能/函数.gsp");
    assert_eq!(scene["objectGraph"]["geometryComplete"], true);
    assert_eq!(operation_kind(&scene, "line:3"), Some("function-plot"));
}

#[test]
fn nested_function_plot_uses_live_parameters_and_payload_defaults() {
    let document = compile_fixture("tests/Samples/个人专栏/向忠作品/正态分布.gsp");
    let scene = if document.get("objectGraph").is_some() {
        &document
    } else {
        &document["pages"][0]["scene"]
    };
    assert_no_graph_validation_errors(scene);
    let line_id = format!(
        "line:{}",
        scene["functions"][0]["lineIndex"]
            .as_u64()
            .expect("normal density plot line")
    );
    assert_eq!(operation_kind(scene, &line_id), Some("function-plot"));
    let node = scene["objectGraph"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["id"] == line_id)
        .unwrap();
    assert!(
        node["definition"]["parents"]
            .as_array()
            .unwrap()
            .iter()
            .any(|parent| parent == "parameter:μ")
    );
    assert!(
        node["definition"]["parents"]
            .as_array()
            .unwrap()
            .iter()
            .any(|parent| parent == "parameter:σ")
    );
    let expression = &node["definition"]["op"]["expression"];
    let serialized = serde_json::to_string(expression).unwrap();
    assert!(serialized.contains("pi-constant"));
    assert!(serialized.contains("euler-constant"));

    let integral_trace_id = object_id_for_group(scene, "lines", "line", 152);
    assert_eq!(
        operation_kind(scene, &integral_trace_id),
        Some("point-trace"),
        "the hidden GraphDistanceValue parent must keep the integral trace live"
    );
}

#[test]
fn coordinate_point_uses_its_payload_scalar_objects_as_graph_parents() {
    let scene = compile_fixture("tests/Samples/个人专栏/孟令岩作品/mly习作-五角星出水导函数.gsp");
    assert_no_graph_validation_errors(&scene);

    let point_id = object_id_for_group(&scene, "points", "point", 439);
    let point_node = scene["objectGraph"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["id"] == point_id)
        .unwrap();
    let polygon_area_label_index = scene["labels"]
        .as_array()
        .unwrap()
        .iter()
        .position(|label| label["debug"]["groupOrdinal"] == 438)
        .expect("polygon-area scalar parent");
    let coordinate_y_id = format!("scalar:{point_id}:coordinate-y");
    let coordinate_y_node = scene["objectGraph"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["id"] == coordinate_y_id)
        .unwrap();
    assert_eq!(
        coordinate_y_node["definition"]["parents"][0],
        format!("scalar:label:{polygon_area_label_index}")
    );
    assert_eq!(
        point_node["definition"]["op"]["kind"],
        "point-offset-by-scalars"
    );
    let trace_id = object_id_for_group(&scene, "lines", "line", 464);
    assert_eq!(operation_kind(&scene, &trace_id), Some("point-trace"));
}

#[test]
fn transformed_line_constraints_remain_nested_graph_parents() {
    let document = compile_fixture("tests/Samples/个人专栏/孟令岩作品/勾股定理小题.gsp");
    let scene = &document["pages"][2]["scene"];
    assert_no_graph_validation_errors(scene);

    let parallel_intersection_id = object_id_for_group(scene, "points", "point", 106);
    assert_eq!(
        operation_kind(scene, &parallel_intersection_id),
        Some("line-intersection")
    );
    let arc_intersection_id = object_id_for_group(scene, "points", "point", 109);
    assert_eq!(
        operation_kind(scene, &arc_intersection_id),
        Some("line-circle-intersection")
    );
    let trace_id = object_id_for_group(scene, "lines", "line", 111);
    assert_eq!(operation_kind(scene, &trace_id), Some("point-trace"));
}

#[test]
fn hidden_function_intersection_builds_its_own_typed_plot_domain() {
    let document = compile_fixture("tests/Samples/个人专栏/向忠作品/指数函数的图象和性质.gsp");
    let scene = &document["pages"][0]["scene"];
    assert_no_graph_validation_errors(scene);
    let point_id = object_id_for_group(scene, "points", "point", 323);
    assert_eq!(
        operation_kind(scene, &point_id),
        Some("line-polyline-intersection")
    );
    assert_eq!(
        operation_kind(scene, &format!("domain:{point_id}:function")),
        Some("function-plot")
    );
}

#[test]
fn point_trace_evaluates_the_target_point_dependency_program() {
    let scene = compile_fixture("tests/fixtures/gsp/trace.gsp");
    assert_eq!(scene["objectGraph"]["geometryComplete"], true);
    assert_eq!(operation_kind(&scene, "line:5"), Some("point-trace"));
    let operation = scene["objectGraph"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["id"] == "line:5")
        .unwrap()["definition"]["op"]
        .clone();
    assert_eq!(operation["program"]["targetId"], "point:5");
    assert_eq!(operation["driver"]["kind"], "scalar");
}

#[test]
fn polygon_boundary_driver_traces_the_complete_live_boundary() {
    let document = compile_fixture("tests/Samples/个人专栏/贺基旭作品/轨迹(hjx4882).gsp");
    let scene = &document["pages"][0]["scene"];
    let driver_id = object_id_for_group(scene, "points", "point", 7);
    let trace_id = object_id_for_group(scene, "lines", "line", 22);
    assert_eq!(
        operation_kind(scene, &driver_id),
        Some("point-on-polygon-boundary")
    );
    assert_eq!(
        operation_kind(scene, &format!("control:{driver_id}:boundary")),
        Some("polygon-boundary-parameter")
    );
    assert_eq!(operation_kind(scene, &trace_id), Some("point-trace"));
    let trace = scene["objectGraph"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["id"] == trace_id)
        .unwrap();
    assert_eq!(
        trace["definition"]["op"]["driver"]["source_id"],
        format!("control:{driver_id}:boundary")
    );
    assert!(
        !scene["objectGraph"]["pendingOperations"]
            .as_array()
            .unwrap()
            .iter()
            .any(|operation| operation
                .as_str()
                .is_some_and(|operation| operation.starts_with("line:")))
    );
}

#[test]
fn boundary_length_endpoint_is_derived_from_its_live_arc() {
    let scene = compile_fixture("tests/Samples/个人专栏/李有贵作品/圆弧展开(yougui).gsp");
    assert_eq!(scene["objectGraph"]["geometryComplete"], true);
    assert_eq!(
        scene["objectGraph"]["pendingOperations"],
        serde_json::json!([])
    );
    let endpoint_id = object_id_for_group(&scene, "points", "point", 10);
    assert_eq!(
        operation_kind(&scene, &endpoint_id),
        Some("point-scaled-offset")
    );
    assert_eq!(
        operation_kind(&scene, &format!("domain:{endpoint_id}:boundary-length")),
        Some("center-arc")
    );
    assert_eq!(
        operation_kind(&scene, &format!("scalar:{endpoint_id}:boundary-length")),
        Some("arc-length")
    );
    let rotated_arc_id = object_id_for_group(&scene, "arcs", "arc", 19);
    assert_eq!(operation_kind(&scene, &rotated_arc_id), Some("center-arc"));
}

#[test]
fn custom_transform_trace_runs_the_payload_target_dependency_program() {
    let scene = compile_fixture("tests/Samples/个人专栏/高峻清作品/正方体的三维全展开(gjq).gsp");
    assert_eq!(scene["objectGraph"]["geometryComplete"], true);
    assert_eq!(
        scene["objectGraph"]["pendingOperations"],
        serde_json::json!([])
    );
    let trace_id = object_id_for_group(&scene, "lines", "line", 33);
    let target_id = object_id_for_group(&scene, "points", "point", 26);
    assert_eq!(operation_kind(&scene, &trace_id), Some("point-trace"));
    let trace = scene["objectGraph"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["id"] == trace_id)
        .unwrap();
    assert_eq!(trace["definition"]["op"]["program"]["targetId"], target_id);
    assert_eq!(trace["definition"]["op"]["driver"]["kind"], "circle");
}

#[test]
fn function_point_drives_the_complete_point_and_segment_trace_chain() {
    let scene = compile_fixture("tests/Samples/个人专栏/贺基旭作品/y=x^2的轴对称性(hjx4882).gsp");
    assert_eq!(scene["objectGraph"]["geometryComplete"], true);
    assert_eq!(
        scene["objectGraph"]["pendingOperations"],
        serde_json::json!([])
    );

    let function_line_id = format!(
        "line:{}",
        scene["functions"][0]["lineIndex"]
            .as_u64()
            .expect("function plot line index")
    );
    let function_point_id = object_id_for_group(&scene, "points", "point", 10);
    assert_eq!(
        operation_kind(&scene, &function_line_id),
        Some("function-plot")
    );
    assert_eq!(
        operation_kind(&scene, &function_point_id),
        Some("point-on-polyline")
    );
    let function_point = scene["objectGraph"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["id"] == function_point_id)
        .unwrap();
    assert_eq!(function_point["definition"]["parents"][0], function_line_id);

    for ordinal in [18, 24, 22] {
        let trace_id = object_id_for_group(&scene, "lines", "line", ordinal);
        assert_eq!(operation_kind(&scene, &trace_id), Some("point-trace"));
    }
    let segment_trace_id = object_id_for_group(&scene, "lines", "line", 21);
    assert_eq!(
        operation_kind(&scene, &segment_trace_id),
        Some("zip-point-traces")
    );
    let segment_trace = scene["objectGraph"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["id"] == segment_trace_id)
        .unwrap();
    assert_eq!(
        segment_trace["definition"]["parents"],
        serde_json::json!([
            format!("{segment_trace_id}:start-trace"),
            format!("{segment_trace_id}:end-trace")
        ])
    );
    let start_trace = scene["objectGraph"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["id"] == format!("{segment_trace_id}:start-trace"))
        .unwrap();
    assert!(
        start_trace["definition"]["op"]["program"]["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|node| node["definition"]["op"]["kind"] == "point-on-generated-trace")
    );
}

#[test]
fn circle_iteration_uses_live_boundary_parameters() {
    let scene = compile_fixture("tests/fixtures/未实现/圆系(inRm).gsp");
    assert_eq!(scene["objectGraph"]["geometryComplete"], true);
    assert_eq!(
        operation_kind(&scene, "circle-iteration:0"),
        Some("circle-iteration")
    );
    assert_eq!(
        operation_kind(&scene, "scalar:circle-iteration:0:next"),
        Some("polygon-boundary-parameter-from-point")
    );
}

#[test]
fn parameter_anchor_expression_keeps_center_arc_live() {
    let scene = compile_fixture("tests/Samples/个人专栏/侯仰顺作品/滑块(蚂蚁).gsp");
    assert_eq!(scene["objectGraph"]["geometryComplete"], true);
    assert_eq!(
        operation_kind(&scene, "scalar:point:15:constraint-parameter"),
        Some("evaluate-expression")
    );
    assert_eq!(
        operation_kind(&scene, "point:15"),
        Some("point-on-circle-parameter")
    );
    assert_eq!(operation_kind(&scene, "arc:2"), Some("center-arc"));
    let arc = scene["objectGraph"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["id"] == "arc:2")
        .unwrap();
    assert_eq!(arc["definition"]["parents"][0], "point:15");
}

#[test]
fn function_rotation_chain_keeps_its_center_arc_live() {
    let scene = compile_fixture("tests/Samples/个人专栏/况永胜作品/分数的魔变.gsp");
    assert_eq!(scene["objectGraph"]["geometryComplete"], true);

    let expected_point_ops = [
        (18, "rotate-point-degrees"),
        (19, "translate-point"),
        (20, "translate-point"),
        (21, "scale-point-by-scalar"),
        (40, "rotate-point-degrees"),
        (41, "rotate-point-degrees"),
    ];
    for (ordinal, expected_op) in expected_point_ops {
        let id = object_id_for_group(&scene, "points", "point", ordinal);
        assert_eq!(operation_kind(&scene, &id), Some(expected_op));
    }

    let arc_id = object_id_for_group(&scene, "arcs", "arc", 42);
    assert_eq!(operation_kind(&scene, &arc_id), Some("center-arc"));
    let center_id = object_id_for_group(&scene, "points", "point", 21);
    let start_id = object_id_for_group(&scene, "points", "point", 40);
    let end_id = object_id_for_group(&scene, "points", "point", 41);
    let arc = scene["objectGraph"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["id"] == arc_id)
        .unwrap();
    assert_eq!(
        arc["definition"]["parents"],
        serde_json::json!([center_id, start_id, end_id])
    );
}

#[test]
fn parameterized_point_iterations_run_typed_dependency_programs() {
    let scene =
        compile_fixture("tests/Samples/个人专栏/李章博作品/动画演示立体几何轨迹形成（李章博）.gsp");
    assert_eq!(scene["objectGraph"]["geometryComplete"], true);
    let expected_targets = [
        object_id_for_group(&scene, "points", "point", 138),
        object_id_for_group(&scene, "points", "point", 145),
    ];
    for (index, target_id) in expected_targets.into_iter().enumerate() {
        let id = format!("point-iteration:{index}");
        assert_eq!(
            operation_kind(&scene, &id),
            Some("parameterized-point-iteration")
        );
        let node = scene["objectGraph"]["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|node| node["id"] == id)
            .unwrap();
        assert_eq!(node["definition"]["op"]["program"]["targetId"], target_id);
        let parents = node["definition"]["parents"].as_array().unwrap();
        assert!(parents.iter().any(|parent| parent == "parameter:t[10]"));
        assert!(parents.iter().any(|parent| {
            parent
                .as_str()
                .is_some_and(|parent| parent == format!("scalar:{id}:depth"))
        }));
    }
}

#[test]
fn spectrum_polygon_colors_depend_on_the_arc_control_point() {
    let scene = compile_fixture("tests/Samples/个人专栏/高峻清作品/勾股树开花（gjq）.gsp");
    assert_eq!(scene["objectGraph"]["geometryComplete"], true);
    assert_eq!(
        scene["objectGraph"]["pendingOperations"],
        serde_json::json!([])
    );
    for index in 0..5 {
        let id = format!("polygon-color:{index}");
        assert_eq!(operation_kind(&scene, &id), Some("spectrum-color"));
        let node = scene["objectGraph"]["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|node| node["id"] == id)
            .unwrap();
        assert_eq!(
            node["definition"]["parents"],
            serde_json::json!([format!("{id}:value")])
        );
        assert_eq!(operation_kind(&scene, &format!("{id}:value")), Some("copy"));
    }
}

#[test]
fn unnamed_parameter_anchors_keep_the_payload_expression_dependency() {
    let scene = compile_fixture("tests/Samples/个人专栏/钟科作品/正N边形内滚动（颗粒）.gsp");
    assert_no_graph_validation_errors(&scene);

    for (point_ordinal, anchor_ordinal) in [(40, 14), (72, 67)] {
        let point_id = object_id_for_group(&scene, "points", "point", point_ordinal);
        assert_eq!(operation_kind(&scene, &point_id), Some("point-on-line"));
        let scalar_id = format!("scalar:{point_id}:constraint-parameter");
        assert_eq!(
            operation_kind(&scene, &scalar_id),
            Some("evaluate-expression")
        );
        let node = scene["objectGraph"]["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|node| node["id"] == scalar_id)
            .unwrap();
        assert!(
            node["definition"]["op"]["parameter_names"]
                .as_array()
                .unwrap()
                .iter()
                .any(|name| name == &format!("__param_anchor_{anchor_ordinal}"))
        );
        assert!(
            node["definition"]["parents"]
                .as_array()
                .unwrap()
                .iter()
                .any(|parent| parent == &format!("{scalar_id}:source:1"))
        );
    }
}

#[test]
fn points_on_reflected_and_rotated_rays_keep_arc_dependencies_live() {
    let scene =
        compile_fixture("tests/Samples/个人专栏/孟令岩作品/投骰子模拟试验（1）（石岩）.gsp");
    assert_eq!(scene["objectGraph"]["geometryComplete"], true);
    assert_eq!(
        scene["objectGraph"]["pendingOperations"],
        serde_json::json!([])
    );

    for ordinal in [71, 77, 340] {
        let point_id = object_id_for_group(&scene, "points", "point", ordinal);
        assert_eq!(operation_kind(&scene, &point_id), Some("point-on-line"));
    }
    let rotated_point_id = object_id_for_group(&scene, "points", "point", 340);
    assert_eq!(
        operation_kind(&scene, &format!("domain:{rotated_point_id}")),
        Some("rotate-shape-degrees")
    );
    for ordinal in [84, 86, 347] {
        let arc_id = object_id_for_group(&scene, "arcs", "arc", ordinal);
        assert_eq!(operation_kind(&scene, &arc_id), Some("center-arc"));
    }
}

#[test]
fn point_on_point_trace_uses_the_trace_as_its_typed_domain() {
    let scene = compile_fixture("tests/Samples/个人专栏/孙禄京作品/路段长度演示.gsp");
    assert_eq!(scene["objectGraph"]["geometryComplete"], true);
    assert_eq!(
        scene["objectGraph"]["pendingOperations"],
        serde_json::json!([])
    );

    let trace_id = object_id_for_group(&scene, "lines", "line", 293);
    let point_id = object_id_for_group(&scene, "points", "point", 294);
    assert_eq!(operation_kind(&scene, &trace_id), Some("point-trace"));
    assert_eq!(operation_kind(&scene, &point_id), Some("point-on-polyline"));
    let point_node = scene["objectGraph"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["id"] == point_id)
        .unwrap();
    assert_eq!(point_node["definition"]["parents"][0], trace_id);
}

#[test]
fn point_trace_can_override_a_derived_constraint_parameter() {
    let scene = compile_fixture("tests/Samples/个人专栏/方小庆作品/FlyingCar(inRm).gsp");
    assert_no_graph_validation_errors(&scene);
    let hidden_trace_id = object_id_for_group(&scene, "lines", "line", 85);
    assert_eq!(
        operation_kind(&scene, &hidden_trace_id),
        Some("point-trace")
    );
    for ordinal in [86, 103] {
        let intersection_id = object_id_for_group(&scene, "points", "point", ordinal);
        assert_eq!(
            operation_kind(&scene, &intersection_id),
            Some("line-polyline-intersection")
        );
        let intersection = scene["objectGraph"]["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|node| node["id"] == intersection_id)
            .unwrap();
        assert_eq!(intersection["definition"]["parents"][1], hidden_trace_id);
    }
    let trace_id = object_id_for_group(&scene, "lines", "line", 114);
    assert_eq!(operation_kind(&scene, &trace_id), Some("point-trace"));

    let trace_node = scene["objectGraph"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["id"] == trace_id)
        .unwrap();
    let operation = &trace_node["definition"]["op"];
    assert_eq!(
        operation["driver"]["source_id"],
        "scalar:point:22:derived-parameter"
    );
    let driver_node = operation["program"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["id"] == "scalar:point:22:derived-parameter")
        .unwrap();
    assert_eq!(driver_node["definition"]["kind"], "source");
}

#[test]
fn unnamed_parameter_anchors_are_derived_without_self_cycles() {
    let document = compile_fixture("tests/Samples/个人专栏/孟令岩作品/整体面积三例（孟令岩）.gsp");
    let pages = document["pages"].as_array().unwrap();
    assert_eq!(pages.len(), 3);

    for page in pages {
        let scene = &page["scene"];
        assert_no_graph_validation_errors(scene);
        let anchors = scene["labels"]
            .as_array()
            .unwrap()
            .iter()
            .enumerate()
            .filter(|(_, label)| label["debug"]["groupKind"] == "ParameterAnchor")
            .map(|(index, _)| format!("scalar:label:{index}"))
            .collect::<Vec<_>>();
        assert!(!anchors.is_empty());
        for anchor_id in anchors {
            let node = scene["objectGraph"]["nodes"]
                .as_array()
                .unwrap()
                .iter()
                .find(|node| node["id"] == anchor_id)
                .expect("parameter anchor graph node");
            assert_eq!(node["definition"]["kind"], "derived");
            assert_eq!(node["definition"]["op"]["kind"], "point-line-parameter");
            assert!(
                node["definition"]["parents"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .all(|parent| parent != &anchor_id)
            );
        }
    }
}

#[test]
fn repeated_line_collection_keeps_the_first_payload_object_index() {
    let scene = compile_fixture("tests/Samples/个人专栏/孙禄京作品/数轴上的π值演示.gsp");
    let source_id = object_id_for_group(&scene, "lines", "line", 98);
    let rotated_id = object_id_for_group(&scene, "lines", "line", 99);
    assert_eq!(
        operation_kind(&scene, &rotated_id),
        Some("rotate-shape-degrees")
    );
    let rotated = scene["objectGraph"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["id"] == rotated_id)
        .unwrap();
    assert_eq!(rotated["definition"]["parents"][0], source_id);
    assert!(
        !scene["objectGraph"]["pendingOperations"]
            .as_array()
            .unwrap()
            .iter()
            .any(|pending| pending
                .as_str()
                .is_some_and(|pending| pending.starts_with("graph-validation:")))
    );
}

#[test]
fn marked_distance_endpoint_depends_on_the_live_distance_measurement() {
    let scene = compile_fixture("tests/Samples/个人专栏/周维波作品/相切圆（飞狐制作）.gsp");
    assert_eq!(scene["objectGraph"]["geometryComplete"], true);
    assert_eq!(
        scene["objectGraph"]["pendingOperations"],
        serde_json::json!([])
    );
    let endpoint_id = object_id_for_group(&scene, "points", "point", 5);
    let distance_label_id = object_id_for_group(&scene, "labels", "scalar:label", 4);
    assert_eq!(
        operation_kind(&scene, &endpoint_id),
        Some("point-scaled-offset")
    );
    let distance_id = format!("scalar:{endpoint_id}:distance");
    assert_eq!(
        operation_kind(&scene, &distance_id),
        Some("evaluate-expression")
    );
    let distance = scene["objectGraph"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["id"] == distance_id)
        .unwrap();
    assert_eq!(distance["definition"]["parents"][0], distance_label_id);
}

#[test]
fn initially_degenerate_constructed_lines_remain_typed_and_live() {
    let overlap = compile_fixture("tests/Samples/个人专栏/钟科作品/重叠面积（颗粒）.gsp");
    assert_no_graph_validation_errors(&overlap);
    let perpendicular_id = object_id_for_group(&overlap, "lines", "line", 39);
    assert_eq!(
        operation_kind(&overlap, &perpendicular_id),
        Some("perpendicular-line")
    );

    let rolling = compile_fixture("tests/Samples/个人专栏/高峻清作品/两正多边形(内互滚)(gjq).gsp");
    assert_no_graph_validation_errors(&rolling);
    let bisector_id = object_id_for_group(&rolling, "lines", "line", 50);
    assert_eq!(
        operation_kind(&rolling, &bisector_id),
        Some("angle-bisector-ray")
    );
}

#[test]
fn plotted_function_parameters_and_intersections_use_exact_graph_parents() {
    let document =
        compile_fixture("tests/Samples/个人专栏/郑飞宇作品/正弦型函数图像变换(修正颜色).gsp");
    for page in document["pages"].as_array().unwrap() {
        assert_eq!(page["scene"]["objectGraph"]["geometryComplete"], true);
        assert_eq!(
            page["scene"]["objectGraph"]["pendingOperations"],
            serde_json::json!([])
        );
    }

    let first = &document["pages"][0]["scene"];
    assert_eq!(operation_kind(first, "line:24"), Some("function-plot"));
    assert_eq!(operation_kind(first, "line:25"), Some("function-plot"));
    assert_eq!(
        operation_kind(first, "point:23"),
        Some("line-polyline-intersection")
    );
    let fourth = &document["pages"][3]["scene"];
    assert_eq!(
        operation_kind(fourth, "point:44"),
        Some("line-polyline-intersection")
    );
}

#[test]
fn rolling_circle_all_pages_have_typed_arcs() {
    let document = compile_fixture("tests/Samples/个人专栏/方小庆作品/圆的滚动全解(inRm).gsp");
    for (page_index, page) in document["pages"].as_array().unwrap().iter().enumerate() {
        let scene = &page["scene"];
        if scene["objectGraph"]["geometryComplete"] != true {
            eprintln!(
                "page {} pending={} arcs={}",
                page_index + 1,
                scene["objectGraph"]["pendingOperations"],
                scene["arcs"]
            );
        }
        assert_eq!(scene["objectGraph"]["geometryComplete"], true);
        assert_eq!(
            scene["objectGraph"]["pendingOperations"],
            serde_json::json!([])
        );
    }

    let fourth = &document["pages"][3]["scene"];
    let polygon_point = object_id_for_group(fourth, "points", "point", 26);
    assert_eq!(
        operation_kind(fourth, &polygon_point),
        Some("point-on-polyline")
    );
    let fourth_arc = object_id_for_group(fourth, "arcs", "arc", 33);
    assert_eq!(operation_kind(fourth, &fourth_arc), Some("center-arc"));

    let sixth = &document["pages"][5]["scene"];
    let expression_arc = object_id_for_group(sixth, "arcs", "arc", 60);
    assert_eq!(operation_kind(sixth, &expression_arc), Some("center-arc"));

    let eighth = &document["pages"][7]["scene"];
    let angle_a = object_id_for_group(eighth, "labels", "scalar:label", 21);
    let alias_a = object_id_for_group(eighth, "labels", "scalar:label", 23);
    let normalized_a = object_id_for_group(eighth, "labels", "scalar:label", 29);
    assert_eq!(operation_kind(eighth, &alias_a), Some("copy"));
    assert_eq!(
        eighth["objectGraph"]["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|node| node["id"] == alias_a)
            .unwrap()["definition"]["parents"][0],
        angle_a
    );
    assert!(
        eighth["objectGraph"]["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|node| node["id"] == normalized_a)
            .unwrap()["definition"]["parents"]
            .as_array()
            .unwrap()
            .iter()
            .any(|parent| parent == &alias_a)
    );
    for (ordinal, kind) in [
        (124, "center-arc"),
        (125, "center-arc"),
        (157, "circle-arc"),
    ] {
        let arc = object_id_for_group(eighth, "arcs", "arc", ordinal);
        assert_eq!(operation_kind(eighth, &arc), Some(kind));
    }
    let rotated_endpoint = object_id_for_group(eighth, "points", "point", 156);
    assert_eq!(
        operation_kind(eighth, &rotated_endpoint),
        Some("rotate-point-degrees")
    );
}

#[test]
fn legacy_calculate_arcs_and_fixed_endpoint_segment_traces_are_typed() {
    let document = compile_fixture("tests/Samples/个人专栏/方小庆作品/(inRm)圆柱圆锥展开.gsp");
    for page in document["pages"].as_array().unwrap() {
        assert_eq!(page["scene"]["objectGraph"]["geometryComplete"], true);
        assert_eq!(
            page["scene"]["objectGraph"]["pendingOperations"],
            serde_json::json!([])
        );
    }

    let scene = &document["pages"][4]["scene"];
    for (ordinal, kind) in [
        (15, "circle-arc"),
        (27, "three-point-arc"),
        (39, "circle-arc"),
    ] {
        let arc_id = object_id_for_group(scene, "arcs", "arc", ordinal);
        assert_eq!(operation_kind(scene, &arc_id), Some(kind));
    }
    let trace_id = object_id_for_group(scene, "lines", "line", 35);
    assert_eq!(operation_kind(scene, &trace_id), Some("zip-point-traces"));
    assert_eq!(
        operation_kind(scene, &format!("{trace_id}:start-trace")),
        Some("repeat-point")
    );
    assert_eq!(
        operation_kind(scene, &format!("{trace_id}:end-trace")),
        Some("point-trace")
    );
}

#[test]
fn hidden_offset_anchor_keeps_parameter_rotated_arc_live() {
    let scene = compile_fixture("tests/Samples/个人专栏/孟令岩作品/认识π.gsp");
    assert_eq!(scene["objectGraph"]["geometryComplete"], true);
    assert_eq!(
        scene["objectGraph"]["pendingOperations"],
        serde_json::json!([])
    );

    let anchor_id = object_id_for_group(&scene, "points", "point", 48);
    let constrained_id = object_id_for_group(&scene, "points", "point", 50);
    let rotated_id = object_id_for_group(&scene, "points", "point", 55);
    let arc_id = object_id_for_group(&scene, "arcs", "arc", 56);
    assert_eq!(operation_kind(&scene, &anchor_id), Some("point-offset"));
    assert_eq!(
        operation_kind(&scene, &constrained_id),
        Some("point-on-line")
    );
    assert_eq!(
        operation_kind(&scene, &rotated_id),
        Some("rotate-point-degrees")
    );
    assert_eq!(operation_kind(&scene, &arc_id), Some("center-arc"));
}

#[test]
fn trace_intersections_depend_on_the_payload_trace_object() {
    let document = compile_fixture("tests/Samples/个人专栏/方小庆作品/(inRm)圆柱圆锥展开.gsp");
    let scene = &document["pages"][0]["scene"];
    assert_eq!(scene["objectGraph"]["geometryComplete"], true);
    assert_eq!(
        scene["objectGraph"]["pendingOperations"],
        serde_json::json!([])
    );

    let trace_id = object_id_for_group(scene, "lines", "line", 24);
    assert_eq!(operation_kind(scene, &trace_id), Some("point-trace"));
    for ordinal in [28, 34] {
        let point_id = object_id_for_group(scene, "points", "point", ordinal);
        assert_eq!(
            operation_kind(scene, &point_id),
            Some("line-polyline-intersection")
        );
        let node = scene["objectGraph"]["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|node| node["id"] == point_id)
            .unwrap();
        assert_eq!(node["definition"]["parents"][1], trace_id);
    }
}

#[test]
fn rotor_expressions_with_the_same_display_name_use_exact_payload_parents() {
    let scene = compile_fixture("tests/Samples/个人专栏/周维波作品/三角转子的滚动-雪山飞狐.gsp");
    assert_eq!(scene["objectGraph"]["geometryComplete"], true);
    assert_eq!(
        scene["objectGraph"]["pendingOperations"],
        serde_json::json!([])
    );

    let node = |id: &str| {
        scene["objectGraph"]["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|node| node["id"] == id)
            .unwrap_or_else(|| panic!("missing object graph node {id}"))
    };
    for (expression_group, parameter_group) in [(11, 10), (23, 17), (27, 18)] {
        let expression_id = object_id_for_group(&scene, "labels", "scalar:label", expression_group);
        let parameter_id = object_id_for_group(&scene, "labels", "scalar:label", parameter_group);
        assert_eq!(
            operation_kind(&scene, &expression_id),
            Some("evaluate-expression")
        );
        assert_eq!(
            node(&expression_id)["definition"]["parents"][0],
            parameter_id,
            "expression group #{expression_group} must use payload parent group #{parameter_group}"
        );
    }

    let scaled_point = object_id_for_group(&scene, "points", "point", 12);
    let scale_scalar = format!("scalar:{scaled_point}:scale-factor");
    let ray_parameter = object_id_for_group(&scene, "labels", "scalar:label", 10);
    assert_eq!(
        operation_kind(&scene, &scaled_point),
        Some("scale-point-by-scalar")
    );
    assert_eq!(
        node(&scale_scalar)["definition"]["parents"][0],
        ray_parameter
    );

    for ordinal in [36, 37, 38] {
        let arc_id = object_id_for_group(&scene, "arcs", "arc", ordinal);
        assert_eq!(operation_kind(&scene, &arc_id), Some("center-arc"));
    }
}
