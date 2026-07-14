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

#[test]
fn n_leaf_trace_point_is_derived_from_trace_and_payload_parameter() {
    let scene = compile_fixture("tests/Samples/个人专栏/向忠作品/n叶草系列迭代.gsp");
    assert_eq!(
        scene["objectGraph"]["geometryComplete"], true,
        "pending: {}",
        scene["objectGraph"]["pendingOperations"]
    );
    assert_eq!(
        scene["objectGraph"]["pendingOperations"],
        serde_json::json!([])
    );

    let point = object_id_for_group(&scene, "points", "point", 44);
    let point_node = scene["objectGraph"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["id"] == point)
        .expect("trace-constrained point node");
    let trace_parent = point_node["definition"]["parents"][0]
        .as_str()
        .expect("trace parent");
    assert_eq!(operation_kind(&scene, &point), Some("point-on-polyline"));
    assert_eq!(operation_kind(&scene, trace_parent), Some("point-trace"));
    assert!(
        point_node["definition"]["parents"]
            .as_array()
            .expect("trace point parents")
            .iter()
            .any(|parent| parent == &serde_json::json!(format!("control:{point}:t")))
    );
}

#[test]
fn triangle_rolling_trace_point_is_derived_from_measured_length_chain() {
    let scene = compile_fixture(
        "tests/Samples/个人专栏/贺基旭作品/圆在三角形边上滚动（成品 By hjx4882).gsp",
    );
    assert_eq!(
        scene["objectGraph"]["geometryComplete"], true,
        "pending: {}",
        scene["objectGraph"]["pendingOperations"]
    );
    assert_eq!(
        scene["objectGraph"]["pendingOperations"],
        serde_json::json!([])
    );

    let first_partition = object_id_for_group(&scene, "points", "point", 45);
    let translated_driver = object_id_for_group(&scene, "points", "point", 63);
    let trace = object_id_for_group(&scene, "lines", "line", 74);
    let trace_point = object_id_for_group(&scene, "points", "point", 78);
    assert_eq!(
        operation_kind(&scene, &first_partition),
        Some("point-on-line")
    );
    assert_eq!(
        operation_kind(&scene, &translated_driver),
        Some("point-on-line")
    );
    assert_eq!(operation_kind(&scene, &trace), Some("point-trace"));
    assert_eq!(
        operation_kind(&scene, &trace_point),
        Some("point-on-polyline")
    );
}

#[test]
fn quadrilateral_rolling_uses_rotation_scalars_and_translation_parents() {
    let scene = compile_fixture("tests/Samples/个人专栏/方小庆作品/圆在四边形上滚动(inRm).gsp");
    assert_eq!(
        scene["objectGraph"]["geometryComplete"], true,
        "pending: {}",
        scene["objectGraph"]["pendingOperations"]
    );
    assert_eq!(
        scene["objectGraph"]["pendingOperations"],
        serde_json::json!([])
    );

    for ordinal in [20, 37, 38, 39, 40, 44, 45, 46] {
        let point = object_id_for_group(&scene, "points", "point", ordinal);
        assert_eq!(
            operation_kind(&scene, &point),
            Some("rotate-point-degrees"),
            "rotation point #{ordinal}"
        );
    }
    for ordinal in 47..=54 {
        let point = object_id_for_group(&scene, "points", "point", ordinal);
        assert_eq!(
            operation_kind(&scene, &point),
            Some("translate-point"),
            "translation point #{ordinal}"
        );
    }

    assert_eq!(
        operation_kind(&scene, "scalar:group:41"),
        Some("polygon-boundary-parameter-from-point")
    );
    let rotation_scalar = scene["objectGraph"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["id"] == "scalar:point:18:rotation-degrees")
        .expect("polygon-parameter rotation scalar");
    assert_eq!(
        rotation_scalar["definition"]["parents"],
        serde_json::json!(["scalar:group:41"])
    );
}

#[test]
fn trajectory_polygon_parameter_and_line_arc_intersection_are_table_driven() {
    let document = compile_fixture("tests/Samples/个人专栏/贺基旭作品/轨迹(hjx4882).gsp");
    for page in document["pages"].as_array().expect("trajectory pages") {
        assert_eq!(page["scene"]["objectGraph"]["geometryComplete"], true);
        assert_eq!(
            page["scene"]["objectGraph"]["pendingOperations"],
            serde_json::json!([])
        );
    }

    let scene = &document["pages"][0]["scene"];
    let parameter_point = object_id_for_group(scene, "points", "point", 11);
    let intersection = object_id_for_group(scene, "points", "point", 19);
    assert_eq!(
        operation_kind(scene, &parameter_point),
        Some("point-on-polygon-boundary")
    );
    assert_eq!(
        operation_kind(scene, &intersection),
        Some("line-circle-intersection")
    );

    let parameter_id = format!("scalar:{parameter_point}:constraint-parameter");
    let parameter = scene["objectGraph"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["id"] == parameter_id)
        .expect("relative polygon parameter expression");
    assert_eq!(parameter["definition"]["op"]["kind"], "wrap-unit-scalar");
    assert_eq!(
        parameter["definition"]["parents"],
        serde_json::json!([format!("{parameter_id}:sum")])
    );
    let parameter_sum = scene["objectGraph"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["id"] == format!("{parameter_id}:sum"))
        .expect("base parameter plus expression offset");
    assert_eq!(
        parameter_sum["definition"]["parents"]
            .as_array()
            .expect("base parameter plus expression offset")
            .len(),
        2
    );
    assert_eq!(
        operation_kind(scene, &format!("{parameter_id}:base")),
        Some("polygon-boundary-parameter-from-point")
    );
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

#[test]
fn walking_person_coordinate_expression_keeps_all_payload_scalar_parents() {
    let scene = compile_fixture("tests/Samples/个人专栏/方小庆作品/步行拄拐人(inRm).gsp");
    assert_eq!(scene["objectGraph"]["geometryComplete"], true);
    assert_eq!(
        scene["objectGraph"]["pendingOperations"],
        serde_json::json!([])
    );

    let point = |ordinal| object_id_for_group(&scene, "points", "point", ordinal);
    let coordinate = point(24);
    let intersection = point(29);
    let final_intersection = point(98);
    assert_eq!(
        operation_kind(&scene, &coordinate),
        Some("point-scaled-offset")
    );
    assert_eq!(
        operation_kind(&scene, &intersection),
        Some("line-intersection")
    );
    assert_eq!(
        operation_kind(&scene, &final_intersection),
        Some("circle-circle-intersection")
    );

    let coordinate_json = scene["points"]
        .as_array()
        .unwrap()
        .iter()
        .find(|point| point["debug"]["groupOrdinal"] == 24)
        .expect("coordinate point #24");
    assert_eq!(
        coordinate_json["binding"]["parameterGroupOrdinals"],
        serde_json::json!({ "OM": 15, "st": 18 })
    );
    let offset = format!("scalar:{coordinate}:coordinate-offset");
    let offset_node = scene["objectGraph"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["id"] == offset)
        .expect("coordinate expression node");
    assert_eq!(
        offset_node["definition"]["op"]["parameter_names"],
        serde_json::json!(["OM", "st"])
    );
    assert_eq!(
        offset_node["definition"]["parents"]
            .as_array()
            .expect("exact scalar parents")
            .len(),
        2
    );
}

#[test]
fn parameter_coordinate_feeds_isochronous_circle_intersections() {
    let scene = compile_fixture("tests/Samples/个人专栏/庞坤生作品/等时圆.gsp");
    assert_eq!(scene["objectGraph"]["geometryComplete"], true);
    assert_eq!(
        scene["objectGraph"]["pendingOperations"],
        serde_json::json!([])
    );
    let point = |ordinal| object_id_for_group(&scene, "points", "point", ordinal);
    assert_eq!(
        operation_kind(&scene, &point(4)),
        Some("point-scaled-offset")
    );
    assert_eq!(
        operation_kind(&scene, &point(10)),
        Some("line-circle-intersection")
    );
    assert_eq!(
        operation_kind(&scene, &point(52)),
        Some("line-intersection")
    );
    let coordinate = scene["points"]
        .as_array()
        .unwrap()
        .iter()
        .find(|point| point["debug"]["groupOrdinal"] == 4)
        .expect("coordinate point #4");
    assert_eq!(
        coordinate["binding"]["parameterGroupOrdinals"],
        serde_json::json!({ "圆半径R": 2 })
    );
}

#[test]
fn fixed_translated_line_keeps_running_person_table_driven() {
    let scene = compile_fixture("tests/Samples/个人专栏/方小庆作品/(inRm)跑步人.gsp");
    assert_eq!(scene["objectGraph"]["geometryComplete"], true);
    assert_eq!(
        scene["objectGraph"]["pendingOperations"],
        serde_json::json!([])
    );
    let point = |ordinal| object_id_for_group(&scene, "points", "point", ordinal);
    assert_eq!(operation_kind(&scene, &point(42)), Some("point-on-line"));
    assert_eq!(operation_kind(&scene, &point(49)), Some("point-on-line"));
    assert_eq!(
        operation_kind(&scene, &point(45)),
        Some("scale-point-by-ratio")
    );
    assert_eq!(
        operation_kind(&scene, &point(124)),
        Some("circle-circle-intersection")
    );
    assert_eq!(
        operation_kind(&scene, &point(129)),
        Some("point-on-polyline")
    );

    let translated_point = scene["points"]
        .as_array()
        .unwrap()
        .iter()
        .find(|point| point["debug"]["groupOrdinal"] == 42)
        .expect("point on the translated segment");
    assert_eq!(
        translated_point["constraint"]["line"]["kind"],
        "translated-delta"
    );
}

#[test]
fn ellipse_on_ellipse_ignores_embedded_tool_sections() {
    let scene = compile_fixture("tests/Samples/热研系列/滚动系列/椭圆在椭圆上滚动.gsp");
    assert_eq!(scene["objectGraph"]["geometryComplete"], true);
    assert_eq!(
        scene["objectGraph"]["pendingOperations"],
        serde_json::json!([])
    );
    for collection in ["points", "lines", "circles", "arcs", "polygons", "labels"] {
        for object in scene[collection].as_array().into_iter().flatten() {
            assert!(
                object["debug"]["groupOrdinal"]
                    .as_u64()
                    .is_none_or(|ordinal| ordinal <= 139),
                "embedded tool object leaked into {collection}: {}",
                object["debug"]
            );
        }
    }
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
fn payload_alias_point_keeps_every_ordered_parent_in_the_object_graph() {
    let document = compile_fixture("tests/Samples/个人专栏/况永胜作品/正方体的展开（3D效果）.gsp");
    let scene = document
        .get("pages")
        .and_then(Value::as_array)
        .and_then(|pages| pages.first())
        .and_then(|page| page.get("scene"))
        .unwrap_or(&document);
    assert_eq!(scene["objectGraph"]["geometryComplete"], true);

    let alias = object_id_for_group(scene, "points", "point", 23);
    let parent = |ordinal| object_id_for_group(scene, "points", "point", ordinal);
    let node = scene["objectGraph"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["id"] == alias)
        .expect("payload alias node");
    assert_eq!(
        node["definition"]["op"]["kind"],
        "projected-coordinate-point"
    );
    assert_eq!(node["definition"]["op"]["source_parent"], 0);
    assert_eq!(
        node["definition"]["parents"],
        serde_json::json!([parent(15), parent(22), parent(10), parent(11), parent(15),])
    );
}

#[test]
fn projected_coordinate_points_keep_mixed_payload_parents_and_feed_translations() {
    let scene = compile_fixture("tests/Samples/个人专栏/高峻清作品/正n棱柱的三视图3(gjq).gsp");
    assert_eq!(scene["objectGraph"]["geometryComplete"], true);
    assert_eq!(
        scene["objectGraph"]["pendingOperations"],
        serde_json::json!([])
    );

    let point = |ordinal| object_id_for_group(&scene, "points", "point", ordinal);
    let line = |ordinal| object_id_for_group(&scene, "lines", "line", ordinal);
    let nodes = scene["objectGraph"]["nodes"].as_array().unwrap();
    let node = |id: &str| {
        nodes
            .iter()
            .find(|node| node["id"] == id)
            .unwrap_or_else(|| panic!("missing graph node {id}"))
    };

    let projected = point(58);
    assert_eq!(
        operation_kind(&scene, &projected),
        Some("projected-coordinate-point")
    );
    assert_eq!(
        node(&projected)["definition"]["parents"],
        serde_json::json!([
            point(57),
            point(37),
            point(15),
            point(16),
            point(17),
            point(55),
            line(56),
            point(57),
        ])
    );

    for (translated_ordinal, source_ordinal) in [
        (72, 66),
        (73, 62),
        (76, 69),
        (79, 58),
        (80, 63),
        (82, 59),
        (83, 60),
    ] {
        let translated = point(translated_ordinal);
        assert_eq!(operation_kind(&scene, &translated), Some("translate-point"));
        assert_eq!(
            node(&translated)["definition"]["parents"],
            serde_json::json!([point(source_ordinal), point(54), point(52)])
        );
    }
}

#[test]
fn projected_coordinate_program_materializes_unrendered_payload_parents() {
    let scene = compile_fixture("tests/Samples/热研系列/滚动系列/椭圆在正多边形上的滚动.gsp");
    let projected = object_id_for_group(&scene, "points", "point", 42);
    let nodes = scene["objectGraph"]["nodes"].as_array().unwrap();
    let node = |id: &str| {
        nodes
            .iter()
            .find(|node| node["id"] == id)
            .unwrap_or_else(|| panic!("missing graph node {id}"))
    };
    assert_eq!(
        operation_kind(&scene, &projected),
        Some("projected-coordinate-point")
    );
    assert_eq!(
        operation_kind(&scene, "payload-group:30"),
        Some("select-parent")
    );
    assert_eq!(
        operation_kind(&scene, "payload-group:31"),
        Some("select-parent")
    );
    assert_eq!(
        node("payload-group:31")["definition"]["parents"],
        serde_json::json!([
            object_id_for_group(&scene, "points", "point", 29),
            "payload-group:30",
        ])
    );
    assert_eq!(
        node(&projected)["definition"]["parents"],
        serde_json::json!([
            object_id_for_group(&scene, "points", "point", 32),
            object_id_for_group(&scene, "points", "point", 19),
            object_id_for_group(&scene, "points", "point", 21),
            object_id_for_group(&scene, "labels", "scalar:label", 22),
            object_id_for_group(&scene, "labels", "scalar:label", 23),
            object_id_for_group(&scene, "points", "point", 25),
            object_id_for_group(&scene, "points", "point", 26),
            object_id_for_group(&scene, "labels", "scalar:label", 27),
            object_id_for_group(&scene, "points", "point", 29),
            "payload-group:30",
            "payload-group:31",
            object_id_for_group(&scene, "points", "point", 32),
        ])
    );
}

#[test]
fn fraction_arc_keeps_its_symbolic_rotation_parent_chain() {
    let scene = compile_fixture("tests/Samples/个人专栏/钟科作品/分数有意义（颗粒）.gsp");
    assert_eq!(scene["objectGraph"]["geometryComplete"], true);
    assert_eq!(
        scene["objectGraph"]["pendingOperations"],
        serde_json::json!([])
    );
    let point = |ordinal| object_id_for_group(&scene, "points", "point", ordinal);
    let arc = object_id_for_group(&scene, "arcs", "arc", 95);
    let scalar = object_id_for_group(&scene, "labels", "scalar:label", 33);
    let nodes = scene["objectGraph"]["nodes"].as_array().unwrap();
    let node = |id: &str| {
        nodes
            .iter()
            .find(|node| node["id"] == id)
            .unwrap_or_else(|| panic!("missing graph node {id}"))
    };

    for ordinal in [92, 94] {
        let rotated = point(ordinal);
        assert_eq!(
            operation_kind(&scene, &rotated),
            Some("rotate-point-degrees")
        );
        let angle = format!("scalar:{rotated}:rotation-degrees");
        assert_eq!(operation_kind(&scene, &angle), Some("evaluate-expression"));
        assert_eq!(
            node(&angle)["definition"]["parents"],
            serde_json::json!([scalar])
        );
    }
    assert_eq!(operation_kind(&scene, &arc), Some("center-arc"));
    assert_eq!(
        node(&arc)["definition"]["parents"],
        serde_json::json!([point(67), point(93), point(94)])
    );
}

#[test]
fn arbitrary_sector_arc_uses_the_arc_measurement_scalar_program() {
    let scene = compile_fixture("tests/Samples/个人专栏/高峻清作品/任意角扇形的滚动(gjq).gsp");
    assert_eq!(scene["objectGraph"]["geometryComplete"], true);
    assert_eq!(
        scene["objectGraph"]["pendingOperations"],
        serde_json::json!([])
    );
    let point = |ordinal| object_id_for_group(&scene, "points", "point", ordinal);
    let arc = object_id_for_group(&scene, "arcs", "arc", 61);
    let nodes = scene["objectGraph"]["nodes"].as_array().unwrap();
    let node = |id: &str| {
        nodes
            .iter()
            .find(|node| node["id"] == id)
            .unwrap_or_else(|| panic!("missing graph node {id}"))
    };

    let measured_rotation = point(49);
    let angle = format!("scalar:{measured_rotation}:rotation-degrees");
    assert_eq!(
        operation_kind(&scene, &measured_rotation),
        Some("rotate-point-degrees")
    );
    assert_eq!(operation_kind(&scene, &angle), Some("evaluate-expression"));
    assert_eq!(
        node(&angle)["definition"]["parents"],
        serde_json::json!(["scalar:group:7"])
    );
    assert_eq!(operation_kind(&scene, &arc), Some("center-arc"));
    assert_eq!(
        node(&arc)["definition"]["parents"],
        serde_json::json!([point(57), point(55), point(58)])
    );
}

#[test]
fn sliding_polygon_circular_trace_chain_is_table_driven() {
    let document =
        compile_fixture("tests/Samples/个人专栏/方小庆作品/多边形沿两定点滑动(inRm).gsp");
    let scene = &document["pages"][2]["scene"];
    let intersection = object_id_for_group(scene, "points", "point", 31);
    let trace = object_id_for_group(scene, "lines", "line", 21);
    let translated = object_id_for_group(scene, "points", "point", 34);
    let traced = object_id_for_group(scene, "lines", "line", 37);
    assert_eq!(
        operation_kind(scene, &intersection),
        Some("circular-polyline-intersection")
    );
    let intersection_node = scene["objectGraph"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["id"] == intersection)
        .expect("circular trace intersection node");
    assert_eq!(
        intersection_node["definition"]["parents"],
        serde_json::json!([format!("domain:{intersection}:circle"), trace])
    );
    assert_eq!(operation_kind(scene, &translated), Some("translate-point"));
    assert_eq!(operation_kind(scene, &traced), Some("point-trace"));
    assert_eq!(scene["objectGraph"]["geometryComplete"], true);
    assert_eq!(
        scene["objectGraph"]["pendingOperations"],
        serde_json::json!([])
    );
}

#[test]
fn colorized_spectrum_lines_are_table_driven_from_the_trace() {
    let scene =
        compile_fixture("tests/Samples/个人专栏/贺基旭作品/20171231抛物线的光学性质_hjx4882.gsp");
    assert_eq!(scene["objectGraph"]["geometryComplete"], true);
    assert_eq!(
        scene["objectGraph"]["pendingOperations"],
        serde_json::json!([])
    );
    let line = object_id_for_group(&scene, "lines", "line", 30);
    assert_eq!(
        operation_kind(&scene, &line),
        Some("colorized-spectrum-line")
    );
    let node = scene["objectGraph"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["id"] == line)
        .expect("colorized spectrum line node");
    assert_eq!(node["definition"]["op"]["step_index"], 0);
    assert_eq!(node["definition"]["parents"][0], "line:1");
    assert_eq!(node["definition"]["parents"][1], "line:12");
    assert_eq!(node["definition"]["parents"][3], "parameter:N");
}

#[test]
fn ellipse_trace_intersection_chain_is_table_driven() {
    let scene = compile_fixture("tests/Samples/个人专栏/向忠作品/椭圆的判定实验.gsp");
    assert_eq!(scene["objectGraph"]["geometryComplete"], true);
    assert_eq!(
        scene["objectGraph"]["pendingOperations"],
        serde_json::json!([])
    );

    let rotation = object_id_for_group(&scene, "points", "point", 105);
    let first_intersection = object_id_for_group(&scene, "points", "point", 119);
    let arc_intersection = object_id_for_group(&scene, "points", "point", 138);
    let undefined_initial_scale = object_id_for_group(&scene, "points", "point", 162);
    assert_eq!(
        operation_kind(&scene, &rotation),
        Some("rotate-point-degrees")
    );
    assert_eq!(
        operation_kind(&scene, &first_intersection),
        Some("line-polyline-intersection")
    );
    assert_eq!(
        operation_kind(&scene, &arc_intersection),
        Some("circular-polyline-intersection")
    );
    assert_eq!(
        operation_kind(&scene, &undefined_initial_scale),
        Some("scale-point-by-scalar")
    );

    let nodes = scene["objectGraph"]["nodes"].as_array().unwrap();
    let node = |id: &str| {
        nodes
            .iter()
            .find(|node| node["id"] == id)
            .unwrap_or_else(|| panic!("missing object graph node {id}"))
    };
    let first_trace = object_id_for_group(&scene, "lines", "line", 103);
    assert_eq!(
        node(&first_intersection)["definition"]["parents"][1],
        first_trace
    );
    assert_eq!(
        node(&arc_intersection)["definition"]["parents"][1],
        first_trace
    );

    let second_trace = object_id_for_group(&scene, "lines", "line", 163);
    for ordinal in [164, 170] {
        let point = object_id_for_group(&scene, "points", "point", ordinal);
        assert_eq!(
            operation_kind(&scene, &point),
            Some("line-polyline-intersection")
        );
        assert_eq!(node(&point)["definition"]["parents"][1], second_trace);
    }
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
    let source_trace = object_id_for_group(scene, "lines", "line", 21);
    assert_eq!(
        source_parameter_node["definition"]["op"]["kind"],
        "polyline-parameter-from-point"
    );
    assert_eq!(
        source_parameter_node["definition"]["parents"],
        serde_json::json!([source_trace, source])
    );
}

#[test]
fn polygon_parameter_point_uses_the_exact_anchor_scalar_parent() {
    let scene = compile_fixture("tests/Samples/个人专栏/侯仰顺作品/10福建宁德26题2(蚂蚁).gsp");
    assert_eq!(scene["objectGraph"]["geometryComplete"], true);

    let point = object_id_for_group(&scene, "points", "point", 17);
    let anchor_point = object_id_for_group(&scene, "points", "point", 3);
    let segment_start = object_id_for_group(&scene, "points", "point", 11);
    let segment_end = object_id_for_group(&scene, "points", "point", 9);
    let anchor_scalar = object_id_for_group(&scene, "labels", "scalar:label", 14);
    assert_eq!(
        operation_kind(&scene, &point),
        Some("point-on-polygon-boundary")
    );
    assert_eq!(
        operation_kind(&scene, &anchor_scalar),
        Some("point-line-parameter")
    );
    let anchor_node = scene["objectGraph"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["id"] == anchor_scalar)
        .unwrap();
    assert_eq!(
        anchor_node["definition"]["parents"],
        serde_json::json!([anchor_point, segment_start, segment_end])
    );

    let parameter_scalar = format!("scalar:{point}:constraint-parameter");
    let parameter_node = scene["objectGraph"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["id"] == parameter_scalar)
        .unwrap();
    assert_eq!(
        parameter_node["definition"]["parents"],
        serde_json::json!([anchor_scalar])
    );
}

#[test]
fn parameter_angle_expression_keeps_the_refraction_intersection_live() {
    let scene = compile_fixture("tests/Samples/个人专栏/侯仰顺作品/光的折射(蚂蚁制作).gsp");
    assert_eq!(scene["objectGraph"]["geometryComplete"], true);

    let rotated = object_id_for_group(&scene, "points", "point", 62);
    let translated = object_id_for_group(&scene, "points", "point", 63);
    let intersection = object_id_for_group(&scene, "points", "point", 66);
    let source = object_id_for_group(&scene, "points", "point", 36);
    let center = object_id_for_group(&scene, "points", "point", 40);
    let translated_source = object_id_for_group(&scene, "points", "point", 20);
    let refractive_index = object_id_for_group(&scene, "labels", "scalar:label", 18);
    let angle = format!("scalar:{rotated}:rotation-degrees");

    assert_eq!(
        operation_kind(&scene, &rotated),
        Some("rotate-point-degrees")
    );
    assert_eq!(operation_kind(&scene, &translated), Some("translate-point"));
    assert_eq!(
        operation_kind(&scene, &intersection),
        Some("line-circle-intersection")
    );
    let nodes = scene["objectGraph"]["nodes"].as_array().unwrap();
    let angle_node = nodes.iter().find(|node| node["id"] == angle).unwrap();
    assert_eq!(
        angle_node["definition"]["parents"],
        serde_json::json!([refractive_index])
    );
    let rotated_node = nodes.iter().find(|node| node["id"] == rotated).unwrap();
    assert_eq!(
        rotated_node["definition"]["parents"],
        serde_json::json!([source, center, angle])
    );
    let translated_node = nodes.iter().find(|node| node["id"] == translated).unwrap();
    assert_eq!(
        translated_node["definition"]["parents"],
        serde_json::json!([translated_source, rotated, center])
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
fn legacy_coordinate_helpers_keep_piecewise_star_intersections_live() {
    let scene = compile_fixture("tests/Samples/个人专栏/孟令岩作品/mly习作-五角星出水导函数.gsp");
    assert_eq!(
        scene["objectGraph"]["pendingOperations"],
        serde_json::json!([])
    );

    let helper = object_id_for_group(&scene, "points", "point", 245);
    let intersection = object_id_for_group(&scene, "points", "point", 248);
    assert_eq!(operation_kind(&scene, &helper), Some("point-scaled-offset"));
    assert_eq!(
        operation_kind(&scene, &intersection),
        Some("line-circle-intersection")
    );

    let parameter_scalar = "scalar:group:142";
    assert_eq!(
        operation_kind(&scene, parameter_scalar),
        Some("point-line-parameter")
    );
    let parameter_point = object_id_for_group(&scene, "points", "point", 141);
    let parameter_start = object_id_for_group(&scene, "points", "point", 138);
    let parameter_end = object_id_for_group(&scene, "points", "point", 139);
    let parameter_node = scene["objectGraph"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["id"] == parameter_scalar)
        .expect("parameter anchor scalar");
    assert_eq!(
        parameter_node["definition"]["parents"],
        serde_json::json!([parameter_point, parameter_start, parameter_end])
    );

    let expression = object_id_for_group(&scene, "labels", "scalar:label", 244);
    let nested_expression = object_id_for_group(&scene, "labels", "scalar:label", 233);
    let expression_node = scene["objectGraph"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["id"] == expression)
        .expect("piecewise helper expression");
    assert_eq!(
        expression_node["definition"]["op"]["parameter_names"],
        serde_json::json!(["__param_anchor_142", "__ratio_value_231*__param_anchor_107"])
    );
    assert_eq!(
        expression_node["definition"]["parents"],
        serde_json::json!([parameter_scalar, nested_expression])
    );

    let ratio = object_id_for_group(&scene, "labels", "scalar:label", 231);
    let ratio_origin = object_id_for_group(&scene, "points", "point", 170);
    let ratio_denominator = object_id_for_group(&scene, "points", "point", 230);
    let ratio_numerator = object_id_for_group(&scene, "points", "point", 180);
    assert_eq!(operation_kind(&scene, &ratio), Some("point-distance-ratio"));
    let ratio_node = scene["objectGraph"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["id"] == ratio)
        .expect("ratio scalar");
    assert_eq!(
        ratio_node["definition"]["parents"],
        serde_json::json!([ratio_origin, ratio_denominator, ratio_numerator])
    );
}

#[test]
fn circle_intersection_parameter_anchors_are_graph_scalars() {
    let scene = compile_fixture("tests/Samples/个人专栏/侯仰顺作品/滑块(蚂蚁).gsp");
    assert_eq!(
        scene["objectGraph"]["pendingOperations"],
        serde_json::json!([])
    );

    let scalar = "scalar:group:25";
    let circle = format!("domain:{scalar}:circle");
    let point = object_id_for_group(&scene, "points", "point", 23);
    let center = object_id_for_group(&scene, "points", "point", 9);
    let radius = object_id_for_group(&scene, "points", "point", 13);
    assert_eq!(operation_kind(&scene, scalar), Some("circle-parameter"));
    assert_eq!(operation_kind(&scene, &circle), Some("circle-by-points"));

    let nodes = scene["objectGraph"]["nodes"].as_array().unwrap();
    let circle_node = nodes
        .iter()
        .find(|node| node["id"] == circle)
        .expect("parameter host circle");
    assert_eq!(
        circle_node["definition"]["parents"],
        serde_json::json!([center, radius])
    );
    let scalar_node = nodes
        .iter()
        .find(|node| node["id"] == scalar)
        .expect("circle parameter scalar");
    assert_eq!(
        scalar_node["definition"]["parents"],
        serde_json::json!([point, circle])
    );

    let expression = object_id_for_group(&scene, "labels", "scalar:label", 27);
    let expression_node = nodes
        .iter()
        .find(|node| node["id"] == expression)
        .expect("parameter-controlled circle expression");
    let parents = expression_node["definition"]["parents"].as_array().unwrap();
    assert!(parents.contains(&Value::String(scalar.to_string())));
    assert!(parents.contains(&Value::String("scalar:group:26".to_string())));
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

    let measurement_parameter = "scalar:group:128";
    assert_eq!(
        operation_kind(scene, measurement_parameter),
        Some("point-line-parameter")
    );
    let parameter_point = object_id_for_group(scene, "points", "point", 114);
    let measurement_origin = object_id_for_group(scene, "points", "point", 13);
    let measurement_end = object_id_for_group(scene, "points", "point", 122);
    let parameter_node = scene["objectGraph"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["id"] == measurement_parameter)
        .unwrap();
    assert_eq!(
        parameter_node["definition"]["parents"],
        serde_json::json!([parameter_point, measurement_origin, measurement_end])
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

    let trace_parameter = "scalar:group:88";
    assert_eq!(
        operation_kind(&scene, trace_parameter),
        Some("polyline-parameter-from-point")
    );
    let parameter_point = object_id_for_group(&scene, "points", "point", 87);
    let parameter_node = scene["objectGraph"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["id"] == trace_parameter)
        .unwrap();
    assert_eq!(
        parameter_node["definition"]["parents"],
        serde_json::json!([trace_id, parameter_point])
    );
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
fn fixed_coordinate_root_keeps_heart_curve_transform_chain_table_driven() {
    let scene = compile_fixture("tests/Samples/未分类档/心脏线.gsp");
    assert_eq!(scene["objectGraph"]["geometryComplete"], true);
    assert_eq!(
        scene["objectGraph"]["pendingOperations"],
        serde_json::json!([])
    );

    let point = |ordinal| object_id_for_group(&scene, "points", "point", ordinal);
    let origin = point(1);
    let center = point(9);
    let circle_point = point(11);
    let first_translation = point(13);
    let second_translation = point(14);
    let parameter_rotation = point(17);
    let fixed_rotation = point(20);
    let arc_point = point(23);
    let arc = object_id_for_group(&scene, "arcs", "arc", 22);

    for (id, op) in [
        (&center, "point-offset-by-scalars"),
        (&circle_point, "point-on-circle"),
        (&first_translation, "translate-point"),
        (&second_translation, "translate-point"),
        (&parameter_rotation, "rotate-point-degrees"),
        (&fixed_rotation, "rotate-point-degrees"),
        (&arc_point, "point-on-arc"),
        (&arc, "center-arc"),
    ] {
        assert_eq!(operation_kind(&scene, id), Some(op), "object {id}");
    }

    let node = |id: &str| {
        scene["objectGraph"]["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|node| node["id"] == id)
            .unwrap_or_else(|| panic!("object graph node {id}"))
    };
    assert_eq!(
        node(&first_translation)["definition"]["parents"],
        serde_json::json!([circle_point, center, circle_point])
    );
    assert_eq!(
        node(&second_translation)["definition"]["parents"],
        serde_json::json!([origin, center, first_translation])
    );
    assert_eq!(
        node(&parameter_rotation)["definition"]["parents"],
        serde_json::json!([
            circle_point,
            first_translation,
            format!("scalar:{parameter_rotation}:rotation-degrees")
        ])
    );
    assert_eq!(
        node(&fixed_rotation)["definition"]["parents"],
        serde_json::json!([
            parameter_rotation,
            first_translation,
            format!("scalar:{fixed_rotation}:rotation-degrees")
        ])
    );
    assert_eq!(
        node(&arc)["definition"]["parents"],
        serde_json::json!([first_translation, parameter_rotation, fixed_rotation])
    );
    assert_eq!(
        node(&format!("domain:{arc_point}"))["definition"]["parents"],
        node(&arc)["definition"]["parents"]
    );
}

#[test]
fn polygon_rolling_translation_and_trace_use_the_complete_parent_program() {
    let scene = compile_fixture("tests/Samples/个人专栏/阮国祥作品/多边形在多边形上的滚动.gsp");
    assert_eq!(scene["objectGraph"]["geometryComplete"], true);
    assert_eq!(
        scene["objectGraph"]["pendingOperations"],
        serde_json::json!([])
    );

    let source = object_id_for_group(&scene, "points", "point", 38);
    let vector_start = object_id_for_group(&scene, "points", "point", 73);
    let vector_end = object_id_for_group(&scene, "points", "point", 68);
    let translated = object_id_for_group(&scene, "points", "point", 74);
    let trace = object_id_for_group(&scene, "lines", "line", 75);
    assert_eq!(operation_kind(&scene, &translated), Some("translate-point"));
    assert_eq!(operation_kind(&scene, &trace), Some("point-trace"));

    let node = |id: &str| {
        scene["objectGraph"]["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|node| node["id"] == id)
            .unwrap_or_else(|| panic!("object graph node {id}"))
    };
    assert_eq!(
        node(&translated)["definition"]["parents"],
        serde_json::json!([source, vector_start, vector_end])
    );
    assert_eq!(
        node(&trace)["definition"]["op"]["program"]["targetId"],
        translated
    );
}

#[test]
fn moon_center_arcs_use_parameter_root_endpoint_chains_on_both_pages() {
    let document = compile_fixture(
        "tests/Samples/个人专栏/庞坤生作品/月球的公转和自转（为何看不到月球背面）.gsp",
    );
    let pages = document["pages"].as_array().expect("two-page document");
    assert_eq!(pages.len(), 2);
    for (page_index, page) in pages.iter().enumerate() {
        let scene = &page["scene"];
        assert_eq!(scene["objectGraph"]["geometryComplete"], true);
        assert_eq!(
            scene["objectGraph"]["pendingOperations"],
            serde_json::json!([])
        );
        let point = |ordinal| object_id_for_group(scene, "points", "point", ordinal);
        let node = |id: &str| {
            scene["objectGraph"]["nodes"]
                .as_array()
                .unwrap()
                .iter()
                .find(|node| node["id"] == id)
                .unwrap_or_else(|| panic!("page {} object graph node {id}", page_index + 1))
        };
        let expected = match page_index {
            0 => [(20, 16, 17, 18), (21, 16, 18, 17)],
            1 => [(26, 22, 24, 25), (28, 22, 25, 24)],
            _ => unreachable!(),
        };
        for (arc_ordinal, center, start, end) in expected {
            let arc = object_id_for_group(scene, "arcs", "arc", arc_ordinal);
            assert_eq!(operation_kind(scene, &arc), Some("center-arc"));
            assert_eq!(
                node(&arc)["definition"]["parents"],
                serde_json::json!([point(center), point(start), point(end)])
            );
        }
    }
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
fn circle_measurement_function_rotations_keep_center_arcs_table_driven() {
    let document = compile_fixture(
        "tests/Samples/个人专栏/孟令岩作品/※圆柱、圆锥、圆台的展开与形成20131012（孟令岩）.gsp",
    );
    let scene = &document["pages"][3]["scene"];
    let point = |ordinal| object_id_for_group(scene, "points", "point", ordinal);
    let arc = |ordinal| object_id_for_group(scene, "arcs", "arc", ordinal);
    let nodes = scene["objectGraph"]["nodes"].as_array().unwrap();
    let node = |id: &str| {
        nodes
            .iter()
            .find(|node| node["id"] == id)
            .unwrap_or_else(|| panic!("missing object graph node {id}"))
    };

    for ordinal in [13, 15, 30] {
        assert_eq!(
            operation_kind(scene, &point(ordinal)),
            Some("rotate-point-degrees")
        );
    }
    assert_eq!(operation_kind(scene, &point(17)), Some("point-on-arc"));

    let expression_rotation = point(13);
    let expression_scalar = format!("scalar:{expression_rotation}:rotation-degrees");
    assert_eq!(
        operation_kind(scene, &expression_scalar),
        Some("evaluate-expression")
    );
    assert_eq!(
        node(&expression_scalar)["definition"]["parents"]
            .as_array()
            .expect("expression parents")
            .len(),
        2,
        "the radius and distance payload helpers must both remain live parents"
    );

    let measured_rotation = point(30);
    let measured_scalar = format!("scalar:{measured_rotation}:rotation-degrees");
    assert_eq!(
        operation_kind(scene, &measured_scalar),
        Some("measured-rotation-degrees")
    );
    assert_eq!(
        node(&measured_scalar)["definition"]["parents"],
        serde_json::json!([point(1), point(6), point(17)])
    );

    for (arc_ordinal, center, start, end) in [(16, 6, 14, 15), (18, 6, 1, 13), (38, 6, 20, 30)] {
        let arc_id = arc(arc_ordinal);
        assert_eq!(operation_kind(scene, &arc_id), Some("center-arc"));
        assert_eq!(
            node(&arc_id)["definition"]["parents"],
            serde_json::json!([point(center), point(start), point(end)])
        );
    }

    for id in [
        point(13),
        point(15),
        point(17),
        point(30),
        arc(16),
        arc(18),
        arc(38),
    ] {
        assert!(
            scene["objectGraph"]["pendingOperations"]
                .as_array()
                .expect("pending operations")
                .iter()
                .all(|pending| !pending
                    .as_str()
                    .is_some_and(|pending| pending.starts_with(&id))),
            "{id} must not fall back to a pending source"
        );
    }

    let scene = &document["pages"][4]["scene"];
    let point = |ordinal| object_id_for_group(scene, "points", "point", ordinal);
    let arc = |ordinal| object_id_for_group(scene, "arcs", "arc", ordinal);
    let circle = |ordinal| object_id_for_group(scene, "circles", "circle", ordinal);
    let nodes = scene["objectGraph"]["nodes"].as_array().unwrap();
    let node = |id: &str| {
        nodes
            .iter()
            .find(|node| node["id"] == id)
            .unwrap_or_else(|| panic!("missing object graph node {id}"))
    };

    let circumference = "scalar:group:44";
    let radius = "scalar:group:44:radius";
    assert_eq!(operation_kind(scene, circumference), Some("scale-scalar"));
    assert_eq!(operation_kind(scene, radius), Some("circular-radius"));
    assert_eq!(
        node(radius)["definition"]["parents"],
        serde_json::json!([circle(37)])
    );

    for (ordinal, kind) in [
        (46, "rotate-point-degrees"),
        (49, "point-on-arc"),
        (55, "rotate-point-degrees"),
        (104, "rotate-point-degrees"),
        (106, "point-on-arc"),
        (111, "rotate-point-degrees"),
    ] {
        assert_eq!(operation_kind(scene, &point(ordinal)), Some(kind));
    }
    let angle = format!("scalar:{}:rotation-degrees", point(46));
    assert_eq!(operation_kind(scene, &angle), Some("evaluate-expression"));
    assert!(
        node(&angle)["definition"]["parents"]
            .as_array()
            .expect("sector angle parents")
            .iter()
            .any(|parent| parent == circumference)
    );

    for (arc_ordinal, center, start, end) in [
        (48, 39, 1, 46),
        (56, 49, 55, 53),
        (105, 39, 87, 104),
        (112, 106, 111, 109),
    ] {
        let arc_id = arc(arc_ordinal);
        assert_eq!(operation_kind(scene, &arc_id), Some("center-arc"));
        assert_eq!(
            node(&arc_id)["definition"]["parents"],
            serde_json::json!([point(center), point(start), point(end)])
        );
    }
    assert!(
        scene["objectGraph"]["pendingOperations"]
            .as_array()
            .expect("pending operations")
            .iter()
            .all(|pending| !pending
                .as_str()
                .is_some_and(|pending| pending.starts_with("arc:")))
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
                .any(|parent| parent == &format!("scalar:group:{anchor_ordinal}"))
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
                "page {} pending={}",
                page_index + 1,
                scene["objectGraph"]["pendingOperations"],
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
    let polygon_parameter = "scalar:group:46";
    assert_eq!(
        operation_kind(eighth, polygon_parameter),
        Some("polygon-boundary-parameter-from-point")
    );
    let polygon_parameter_node = eighth["objectGraph"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["id"] == polygon_parameter)
        .unwrap();
    let polygon_parameter_parents =
        [13, 12, 11, 10, 12].map(|ordinal| object_id_for_group(eighth, "points", "point", ordinal));
    assert_eq!(
        polygon_parameter_node["definition"]["parents"],
        serde_json::json!(polygon_parameter_parents)
    );
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
    for (page_index, page) in document["pages"].as_array().unwrap().iter().enumerate() {
        assert_eq!(
            page["scene"]["objectGraph"]["geometryComplete"], true,
            "page {page_index} pending: {}",
            page["scene"]["objectGraph"]["pendingOperations"]
        );
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
    assert_eq!(
        scene["objectGraph"]["geometryComplete"], true,
        "pending: {}",
        scene["objectGraph"]["pendingOperations"]
    );
    assert_eq!(
        scene["objectGraph"]["pendingOperations"],
        serde_json::json!([])
    );

    let rotation_id = object_id_for_group(scene, "points", "point", 22);
    let translation_id = object_id_for_group(scene, "points", "point", 23);
    assert_eq!(
        operation_kind(scene, &rotation_id),
        Some("rotate-point-degrees")
    );
    assert_eq!(
        operation_kind(scene, &translation_id),
        Some("translate-point")
    );
    let parameter_id = "scalar:group:19";
    assert_eq!(
        operation_kind(scene, parameter_id),
        Some("arc-parameter-from-point")
    );
    let parameter_node = scene["objectGraph"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["id"] == parameter_id)
        .expect("arc parameter node");
    assert_eq!(
        parameter_node["definition"]["parents"],
        serde_json::json!([
            object_id_for_group(scene, "arcs", "arc", 11),
            object_id_for_group(scene, "points", "point", 13)
        ])
    );
    let angle_id = format!("scalar:{rotation_id}:rotation-degrees");
    let angle_node = scene["objectGraph"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["id"] == angle_id)
        .expect("arc-parameter rotation scalar");
    assert_eq!(
        angle_node["definition"]["parents"],
        serde_json::json!([parameter_id])
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

#[test]
fn direct_polar_transform_drives_the_circle_arc_construction() {
    let scene = compile_fixture("tests/Samples/个人专栏/方小庆作品/化圆为方详解(inRm).gsp");
    assert_eq!(scene["objectGraph"]["geometryComplete"], true);
    assert_eq!(
        scene["objectGraph"]["pendingOperations"],
        serde_json::json!([])
    );

    let source = object_id_for_group(&scene, "points", "point", 35);
    let transformed = object_id_for_group(&scene, "points", "point", 42);
    let midpoint = object_id_for_group(&scene, "points", "point", 44);
    let intersection = object_id_for_group(&scene, "points", "point", 47);
    let arc = object_id_for_group(&scene, "arcs", "arc", 49);
    let distance = format!("scalar:{transformed}:distance");
    let angle = format!("scalar:{transformed}:angle");
    let nodes = scene["objectGraph"]["nodes"].as_array().unwrap();
    let node = |id: &str| {
        nodes
            .iter()
            .find(|node| node["id"] == id)
            .unwrap_or_else(|| panic!("missing object graph node {id}"))
    };

    assert_eq!(
        operation_kind(&scene, &transformed),
        Some("point-polar-offset")
    );
    assert_eq!(
        node(&transformed)["definition"]["parents"],
        serde_json::json!([source, distance, angle])
    );
    assert_eq!(
        operation_kind(&scene, &distance),
        Some("evaluate-expression")
    );
    assert_eq!(operation_kind(&scene, &angle), Some("evaluate-expression"));
    assert_eq!(operation_kind(&scene, &midpoint), Some("midpoint"));
    assert_eq!(
        operation_kind(&scene, &intersection),
        Some("line-circle-intersection")
    );
    assert_eq!(operation_kind(&scene, &arc), Some("circle-arc"));

    let point = |ordinal: u64| {
        scene["points"]
            .as_array()
            .unwrap()
            .iter()
            .find(|point| point["debug"]["groupOrdinal"].as_u64() == Some(ordinal))
            .unwrap_or_else(|| panic!("missing scene point for group #{ordinal}"))
    };
    let source_point = point(35);
    let transformed_point = point(42);
    let dx = transformed_point["x"].as_f64().unwrap() - source_point["x"].as_f64().unwrap();
    let dy = transformed_point["y"].as_f64().unwrap() - source_point["y"].as_f64().unwrap();
    assert!(
        (dx - 1.820_933_542_503_012_9).abs() < 1e-12,
        "unexpected direct polar x offset: {dx}"
    );
    assert!(dy.abs() < 1e-12, "unexpected direct polar y offset: {dy}");
}

#[test]
fn parameter_rotated_endpoints_keep_the_involute_center_arc_live() {
    let scene =
        compile_fixture("tests/Samples/个人专栏/周维波作品/正n边形的渐开线（雪山飞狐）.gsp");
    let point_ordinals = scene["points"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|point| point["debug"]["groupOrdinal"].as_u64())
        .collect::<Vec<_>>();
    assert!(
        [21, 26]
            .iter()
            .all(|ordinal| point_ordinals.contains(ordinal)),
        "missing parameter-rotation endpoints: {point_ordinals:?}"
    );
    assert_eq!(scene["objectGraph"]["geometryComplete"], true);
    let arc = object_id_for_group(&scene, "arcs", "arc", 27);
    assert_eq!(operation_kind(&scene, &arc), Some("center-arc"));
}

#[test]
fn square_wheel_boundary_intersection_drives_its_point_trace() {
    let scene = compile_fixture("tests/Samples/个人专栏/侯仰顺作品/方形车轮(蚂蚁).gsp");
    let point_ordinals = scene["points"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|point| point["debug"]["groupOrdinal"].as_u64())
        .collect::<Vec<_>>();
    assert!(
        [12, 13, 14, 15, 23]
            .iter()
            .all(|ordinal| point_ordinals.contains(ordinal)),
        "missing square-wheel dependency points: {point_ordinals:?}"
    );
    assert_eq!(scene["objectGraph"]["geometryComplete"], true);
    let intersection = object_id_for_group(&scene, "points", "point", 23);
    let trace = object_id_for_group(&scene, "lines", "line", 24);
    assert_eq!(
        operation_kind(&scene, &intersection),
        Some("line-polyline-intersection")
    );
    assert_eq!(operation_kind(&scene, &trace), Some("point-trace"));
}

#[test]
fn clock_time_scalars_use_their_exact_arc_angle_parents() {
    let scene = compile_fixture("tests/Samples/个人专栏/侯仰顺作品/时钟(蚂蚁制作).gsp");
    let reflected_arc = object_id_for_group(&scene, "arcs", "arc", 355);
    assert_eq!(
        operation_kind(&scene, &reflected_arc),
        Some("reflect-shape-across-line")
    );

    let nodes = scene["objectGraph"]["nodes"].as_array().unwrap();
    for (source_group, result_group) in [(95, 100), (97, 101), (99, 102)] {
        let source = format!("scalar:group:{source_group}");
        let result = object_id_for_group(&scene, "labels", "scalar:label", result_group);
        assert_eq!(operation_kind(&scene, &source), Some("arc-angle-degrees"));
        let result_node = nodes.iter().find(|node| node["id"] == result).unwrap();
        assert_eq!(
            result_node["definition"]["parents"],
            serde_json::json!([source]),
            "clock result group #{result_group} must use arc-angle group #{source_group}"
        );
    }
    assert_eq!(scene["objectGraph"]["geometryComplete"], true);
}

#[test]
fn meng_clock_iterations_are_table_driven() {
    let document = compile_fixture("tests/Samples/个人专栏/孟令岩作品/时钟.gsp");
    for (page_index, page) in document["pages"].as_array().unwrap().iter().enumerate() {
        let scene = &page["scene"];
        if scene["objectGraph"]["geometryComplete"] != true {
            eprintln!(
                "page={} pending={}",
                page_index + 1,
                scene["objectGraph"]["pendingOperations"],
            );
        }
        assert_eq!(scene["objectGraph"]["geometryComplete"], true);
    }

    let second = &document["pages"][1]["scene"];
    assert_eq!(
        operation_kind(second, "line-iteration:0"),
        Some("line-affine-iteration")
    );
    let iteration = second["objectGraph"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["id"] == "line-iteration:0")
        .unwrap();
    assert_eq!(
        iteration["definition"]["op"]["target_handles"],
        serde_json::json!([
            { "kind": "fixed", "point": { "x": 507.0, "y": 60.00000000000006 } },
            { "kind": "parent-point" },
            { "kind": "parent-point" }
        ])
    );
    assert_eq!(
        iteration["definition"]["parents"].as_array().unwrap().len(),
        8
    );
}

#[test]
fn li_circle_to_square_keeps_the_payload_colors_and_iteration_origins() {
    let document = compile_fixture("tests/Samples/个人专栏/李章博作品/割圆为方（李章博）.gsp");
    let scene = document
        .get("pages")
        .and_then(Value::as_array)
        .and_then(|pages| pages.first())
        .map(|page| &page["scene"])
        .unwrap_or(&document);
    assert_eq!(scene["objectGraph"]["geometryComplete"], true);

    let polygons = scene["polygons"].as_array().unwrap();
    assert_eq!(
        polygons
            .iter()
            .take(4)
            .map(|polygon| polygon["color"].clone())
            .collect::<Vec<_>>(),
        vec![
            serde_json::json!([255, 0, 0, 127]),
            serde_json::json!([0, 128, 0, 127]),
            serde_json::json!([0, 128, 0, 127]),
            serde_json::json!([255, 0, 0, 127]),
        ]
    );

    let iterations = scene["polygonIterations"].as_array().unwrap();
    assert_eq!(iterations.len(), 4);
    assert_eq!(
        iterations
            .iter()
            .map(|family| (
                family["sourceIndex"].as_u64().unwrap(),
                family["inverse"].as_bool().unwrap(),
                family["color"].clone(),
            ))
            .collect::<Vec<_>>(),
        vec![
            (1, true, serde_json::json!([0, 128, 0, 127])),
            (2, false, serde_json::json!([0, 128, 0, 127])),
            (3, true, serde_json::json!([255, 0, 0, 127])),
            (0, false, serde_json::json!([255, 0, 0, 127])),
        ]
    );
    for (index, family) in iterations.iter().enumerate() {
        assert_eq!(family["depth"], 9);
        assert_eq!(family["sourceStartIndex"], 7);
        assert_eq!(family["sourceEndIndex"], 8);
        assert_eq!(family["targetStartIndex"], 9);
        assert_eq!(family["targetEndIndex"], 11);
        assert_eq!(
            operation_kind(scene, &format!("polygon-iteration:{index}")),
            Some("similarity-polygon-iteration")
        );
        let depth = format!("scalar:polygon-iteration:{index}:depth");
        assert_eq!(operation_kind(scene, &depth), Some("evaluate-expression"));
        let depth_node = scene["objectGraph"]["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|node| node["id"] == depth)
            .unwrap();
        assert_eq!(
            depth_node["definition"]["parents"],
            serde_json::json!(["parameter:n"])
        );
    }
}

#[test]
fn translated_point_uses_its_exact_payload_parent_chain() {
    let document = compile_fixture("tests/Samples/个人专栏/潘建平作品/40牛潘建平老师.gsp");
    let scene = &document["pages"][0]["scene"];
    assert_eq!(scene["objectGraph"]["geometryComplete"], true);
    assert_eq!(
        scene["objectGraph"]["pendingOperations"],
        serde_json::json!([])
    );

    let rotated = object_id_for_group(scene, "points", "point", 12);
    let translated = object_id_for_group(scene, "points", "point", 14);
    let source = object_id_for_group(scene, "points", "point", 11);
    let vector_start = object_id_for_group(scene, "points", "point", 1);
    let vector_end = object_id_for_group(scene, "points", "point", 13);
    assert_eq!(
        operation_kind(scene, &rotated),
        Some("rotate-point-degrees")
    );
    assert_eq!(operation_kind(scene, &translated), Some("translate-point"));
    let translated_node = scene["objectGraph"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["id"] == translated)
        .expect("translated point graph node");
    assert_eq!(
        translated_node["definition"]["parents"],
        serde_json::json!([rotated, vector_start, vector_end])
    );
    let rotated_node = scene["objectGraph"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["id"] == rotated)
        .expect("rotated source graph node");
    assert_eq!(rotated_node["definition"]["parents"][0], source);
}

#[test]
fn half_sector_pages_keep_scale_translation_intersection_and_arc_programs() {
    let document = compile_fixture("tests/Samples/热研系列/滚动系列/半圆扇形滚动操作详解.gsp");
    let pages = document["pages"].as_array().expect("two-page fixture");
    assert_eq!(pages.len(), 2);

    for (page_index, page) in pages.iter().enumerate() {
        let scene = &page["scene"];
        assert_eq!(scene["objectGraph"]["geometryComplete"], true);
        assert_eq!(
            scene["objectGraph"]["pendingOperations"],
            serde_json::json!([])
        );
        let point = |ordinal| object_id_for_group(scene, "points", "point", ordinal);
        let arc = |ordinal| object_id_for_group(scene, "arcs", "arc", ordinal);
        let node = |id: &str| {
            scene["objectGraph"]["nodes"]
                .as_array()
                .expect("graph nodes")
                .iter()
                .find(|node| node["id"] == id)
                .unwrap_or_else(|| panic!("page {} graph node {id}", page_index + 1))
        };

        let scaled = point(6);
        assert_eq!(
            operation_kind(scene, &scaled),
            Some("scale-point-by-scalar")
        );
        assert_eq!(
            node(&scaled)["definition"]["parents"],
            serde_json::json!([point(4), point(1), format!("scalar:{scaled}:scale-factor")])
        );
        let factor = format!("scalar:{scaled}:scale-factor");
        assert_eq!(operation_kind(scene, &factor), Some("evaluate-expression"));
        assert_eq!(
            node(&factor)["definition"]["parents"],
            serde_json::json!(["parameter:m₃"])
        );
        assert_eq!(
            node(&factor)["definition"]["op"]["expression"]["default"],
            2.0
        );

        for (ordinal, source, vector_start, vector_end) in
            [(7, 6, 4, 1), (16, 15, 1, 4), (17, 15, 4, 1)]
        {
            let translated = point(ordinal);
            assert_eq!(operation_kind(scene, &translated), Some("translate-point"));
            assert_eq!(
                node(&translated)["definition"]["parents"],
                serde_json::json!([point(source), point(vector_start), point(vector_end)])
            );
        }

        let first_arc = arc(18);
        assert_eq!(operation_kind(scene, &first_arc), Some("center-arc"));
        assert_eq!(
            node(&first_arc)["definition"]["parents"],
            serde_json::json!([point(15), point(16), point(17)])
        );

        if page_index == 1 {
            let line_intersection = point(24);
            let circular_intersection = point(26);
            assert_eq!(
                operation_kind(scene, &line_intersection),
                Some("line-intersection")
            );
            assert_eq!(
                operation_kind(scene, &circular_intersection),
                Some("line-circle-intersection")
            );
            let final_arc = arc(31);
            assert_eq!(operation_kind(scene, &final_arc), Some("center-arc"));
            assert_eq!(
                node(&final_arc)["definition"]["parents"],
                serde_json::json!([point(26), point(28), point(29)])
            );
        }
    }
}

#[test]
fn vector_translated_circle_keeps_both_vector_points_as_graph_parents() {
    let scene = compile_fixture("tests/Samples/个人专栏/钮炳坤作品/椭球（钮炳坤老师）.gsp");
    assert_eq!(scene["objectGraph"]["geometryComplete"], true);
    assert_eq!(
        scene["objectGraph"]["pendingOperations"],
        serde_json::json!([])
    );

    let constrained_point = object_id_for_group(&scene, "points", "point", 13);
    let vector_start = object_id_for_group(&scene, "points", "point", 6);
    let vector_end = object_id_for_group(&scene, "points", "point", 5);
    let domain = format!("domain:{constrained_point}");
    assert_eq!(
        operation_kind(&scene, &constrained_point),
        Some("point-on-circle")
    );
    assert_eq!(operation_kind(&scene, &domain), Some("translate-shape"));
    let domain_node = scene["objectGraph"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["id"] == domain)
        .expect("translated circle domain node");
    assert_eq!(
        domain_node["definition"]["parents"],
        serde_json::json!([format!("{domain}:source"), vector_start, vector_end,])
    );
}

#[test]
fn ellipse_polygon_rolling_is_an_exact_table_driven_program() {
    let scene = compile_fixture("tests/Samples/热研系列/滚动系列/椭圆在正多边形上的滚动.gsp");
    assert_eq!(scene["objectGraph"]["geometryComplete"], true);
    assert_eq!(
        scene["objectGraph"]["pendingOperations"],
        serde_json::json!([])
    );
    let point = |ordinal| object_id_for_group(&scene, "points", "point", ordinal);
    let line = |ordinal| object_id_for_group(&scene, "lines", "line", ordinal);
    let nodes = scene["objectGraph"]["nodes"].as_array().unwrap();
    let node = |id: &str| {
        nodes
            .iter()
            .find(|node| node["id"] == id)
            .unwrap_or_else(|| panic!("missing graph node {id}"))
    };

    let focus = point(69);
    let distance = format!("scalar:{focus}:marked-angle-translation-distance");
    assert_eq!(
        operation_kind(&scene, &focus),
        Some("marked-angle-translation-point")
    );
    assert_eq!(
        node(&focus)["definition"]["parents"],
        serde_json::json!([point(2), point(7), point(2), point(10), distance])
    );
    assert_eq!(
        operation_kind(&scene, &distance),
        Some("evaluate-expression")
    );

    for (ordinal, op) in [
        (85, "rotate-shape-degrees"),
        (87, "perpendicular-line"),
        (101, "rotate-shape-degrees"),
        (102, "reflect-shape-across-line"),
        (110, "rotate-shape-degrees"),
        (118, "perpendicular-line"),
    ] {
        assert_eq!(operation_kind(&scene, &line(ordinal)), Some(op));
    }
    for ordinal in [105, 108] {
        assert_eq!(
            operation_kind(&scene, &point(ordinal)),
            Some("line-circle-intersection")
        );
    }
    assert_eq!(
        operation_kind(&scene, &point(119)),
        Some("line-intersection")
    );
    assert_eq!(
        node(&line(102))["definition"]["parents"],
        serde_json::json!([line(101), line(87)])
    );
}

#[test]
fn measurement_line_parameter_point_translation_is_table_driven() {
    let scene = compile_fixture("tests/Samples/未分类档/平移正弦线作正弦函数图像.gsp");
    assert_eq!(
        scene["objectGraph"]["geometryComplete"], true,
        "pending: {}",
        scene["objectGraph"]["pendingOperations"]
    );
    let point = |ordinal| object_id_for_group(&scene, "points", "point", ordinal);
    assert_eq!(operation_kind(&scene, &point(18)), Some("point-on-line"));
    assert_eq!(operation_kind(&scene, &point(24)), Some("translate-point"));
    let translated = scene["objectGraph"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["id"] == point(24))
        .expect("translated point node");
    assert_eq!(
        translated["definition"]["parents"],
        serde_json::json!([point(19), point(21), point(18)])
    );
}

#[test]
fn initially_undefined_polar_center_keeps_scale_translation_program() {
    let scene = compile_fixture(
        "tests/Samples/个人专栏/孙禄京作品/温州市龙湾区实验中学适应性测试试题(孙禄京).gsp",
    );
    assert_eq!(
        scene["objectGraph"]["geometryComplete"], true,
        "pending: {}",
        scene["objectGraph"]["pendingOperations"]
    );
    let point = |ordinal| object_id_for_group(&scene, "points", "point", ordinal);
    assert_eq!(
        operation_kind(&scene, &point(65)),
        Some("point-scaled-offset")
    );
    assert_eq!(
        operation_kind(&scene, &point(66)),
        Some("scale-point-by-scalar")
    );
    assert_eq!(operation_kind(&scene, &point(69)), Some("translate-point"));
    let nodes = scene["objectGraph"]["nodes"].as_array().unwrap();
    let node = |id: &str| {
        nodes
            .iter()
            .find(|node| node["id"] == id)
            .unwrap_or_else(|| panic!("missing graph node {id}"))
    };
    assert_eq!(
        node(&point(69))["definition"]["parents"],
        serde_json::json!([point(66), point(62), point(2)])
    );
    let distance = format!("scalar:{}:distance", point(65));
    assert_eq!(
        operation_kind(&scene, &distance),
        Some("evaluate-expression")
    );
}

#[test]
fn initially_undefined_rotated_ray_intersection_is_table_driven() {
    let scene = compile_fixture("tests/Samples/未分类档/圆内点的弹性束缚 (3).gsp");
    assert_eq!(
        scene["objectGraph"]["geometryComplete"], true,
        "pending: {}",
        scene["objectGraph"]["pendingOperations"]
    );
    let line = object_id_for_group(&scene, "lines", "line", 14);
    let intersection = object_id_for_group(&scene, "points", "point", 15);
    assert_eq!(operation_kind(&scene, &line), Some("rotate-shape-degrees"));
    assert_eq!(
        operation_kind(&scene, &intersection),
        Some("line-circle-intersection")
    );
    let angle = format!("scalar:{line}:rotation-degrees");
    let ca = object_id_for_group(&scene, "labels", "scalar:label", 12);
    let nodes = scene["objectGraph"]["nodes"].as_array().unwrap();
    let angle_node = nodes
        .iter()
        .find(|node| node["id"] == angle)
        .expect("rotation angle node");
    assert_eq!(
        angle_node["definition"]["parents"],
        serde_json::json!(["parameter:半径", ca])
    );
}

#[test]
fn boundary_curve_length_radius_circle_is_table_driven() {
    let scene = compile_fixture("tests/Samples/热研系列/滚动系列/三角车轮.gsp");
    assert_eq!(
        scene["objectGraph"]["geometryComplete"], true,
        "pending: {}",
        scene["objectGraph"]["pendingOperations"]
    );
    let arc = object_id_for_group(&scene, "arcs", "arc", 9);
    let circle = object_id_for_group(&scene, "circles", "circle", 29);
    let intersection = object_id_for_group(&scene, "points", "point", 30);
    let scalar = "scalar:group:24";
    assert_eq!(operation_kind(&scene, scalar), Some("arc-length"));
    assert_eq!(operation_kind(&scene, &circle), Some("circle-by-radius"));
    assert_eq!(
        operation_kind(&scene, &intersection),
        Some("line-circle-intersection")
    );
    let nodes = scene["objectGraph"]["nodes"].as_array().unwrap();
    let node = |id: &str| {
        nodes
            .iter()
            .find(|node| node["id"] == id)
            .unwrap_or_else(|| panic!("missing graph node {id}"))
    };
    assert_eq!(
        node(scalar)["definition"]["parents"],
        serde_json::json!([arc])
    );
    let radius = format!("scalar:{circle}:radius");
    assert_eq!(
        node(&radius)["definition"]["parents"],
        serde_json::json!([scalar])
    );
    assert_eq!(
        node(&circle)["definition"]["parents"],
        serde_json::json!([object_id_for_group(&scene, "points", "point", 28), radius])
    );
}

#[test]
fn parameter_anchor_on_arc_is_table_driven() {
    let scene = compile_fixture("tests/Samples/个人专栏/向忠作品/正弦波·音乐【电子琴】.gsp");
    assert_eq!(
        scene["objectGraph"]["geometryComplete"], true,
        "pending: {}",
        scene["objectGraph"]["pendingOperations"]
    );
    let point = object_id_for_group(&scene, "points", "point", 13);
    let arc = object_id_for_group(&scene, "arcs", "arc", 56);
    let scalar = "scalar:group:62";
    assert_eq!(
        operation_kind(&scene, scalar),
        Some("arc-parameter-from-point")
    );
    let node = scene["objectGraph"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["id"] == scalar)
        .expect("arc parameter scalar node");
    assert_eq!(
        node["definition"]["parents"],
        serde_json::json!([arc, point])
    );
}

#[test]
fn normalized_polygon_path_point_keeps_boundary_intersection_chain() {
    let scene = compile_fixture("tests/Samples/个人专栏/向忠作品/点阵的局部放大.gsp");
    assert_eq!(
        scene["objectGraph"]["geometryComplete"], true,
        "pending: {}",
        scene["objectGraph"]["pendingOperations"]
    );
    let point = |ordinal| object_id_for_group(&scene, "points", "point", ordinal);
    let polygon = |ordinal| object_id_for_group(&scene, "polygons", "polygon", ordinal);
    let nodes = scene["objectGraph"]["nodes"].as_array().unwrap();
    let node = |id: &str| {
        nodes
            .iter()
            .find(|node| node["id"] == id)
            .unwrap_or_else(|| panic!("missing graph node {id}"))
    };

    for (ordinal, parameter) in [(20, 0.5), (21, 0.0)] {
        let id = point(ordinal);
        assert_eq!(
            operation_kind(&scene, &id),
            Some("point-on-polygon-boundary")
        );
        assert_eq!(
            node(&format!("control:{id}:boundary"))["definition"]["kind"],
            "source"
        );
        let source = scene["objectGraph"]["sources"]
            .as_array()
            .unwrap()
            .iter()
            .find(|source| source["id"] == format!("control:{id}:boundary"))
            .expect("normalized polygon parameter source");
        assert_eq!(source["value"]["value"], parameter);
    }
    let intersection = point(35);
    assert_eq!(
        operation_kind(&scene, &intersection),
        Some("line-polyline-intersection")
    );
    assert_eq!(operation_kind(&scene, &polygon(34)), Some("polygon"));
    let scalar = "scalar:group:36";
    assert_eq!(
        operation_kind(&scene, scalar),
        Some("polygon-boundary-parameter-from-point")
    );
    assert_eq!(
        node(scalar)["definition"]["parents"],
        serde_json::json!([point(21), point(26), point(28), point(30), intersection])
    );
}
