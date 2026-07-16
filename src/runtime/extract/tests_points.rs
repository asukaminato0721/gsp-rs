use super::analysis::analyze_scene;
use super::build_scene_checked;
use super::points::{
    collect_point_objects, decode_directed_angle_anchor_binding, decode_expression_scale_binding,
    try_decode_parameter_controlled_point, try_decode_point_constraint,
};
use super::test_support::{fixture_bytes, fixture_log, fixture_scene, function_expr_has_unary};
use crate::format::GspFile;
use crate::runtime::functions::{FunctionExpr, UnaryFunction};
use crate::runtime::scene::{
    ArcBinding, ArcConstraint, ButtonAction, CircularConstraint, ColorBinding,
    GeometryTransformBinding, IterationTableValueBinding, LineBinding, LineConstraint,
    LineLikeKind, ScenePointBinding, ScenePointConstraint, SceneScalarBinding, ShapeBinding,
    TextLabelBinding,
};
use gsp_runtime_core::ObjectOp;
use gsp_runtime_core::object_graph::ObjectDefinition;

#[test]
fn point_trace_constraint_keeps_its_payload_parameter() {
    let data = fixture_bytes("tests/Samples/个人专栏/向忠作品/n叶草系列迭代.gsp")
        .expect("n-leaf iteration fixture");
    let document = GspFile::parse(&data).expect("n-leaf iteration fixture parses");
    let page = &document.page_files()[0];
    let scene = build_scene_checked(page).expect("n-leaf iteration scene builds");
    assert!(
        scene.lines.iter().any(|line| {
            line.debug
                .as_ref()
                .is_some_and(|debug| debug.group_ordinal == 43)
                && matches!(line.binding, Some(LineBinding::PointTrace { .. }))
        }),
        "line ordinals: {:?}; point ordinals: {:?}",
        scene
            .lines
            .iter()
            .filter_map(|line| line.debug.as_ref().map(|debug| debug.group_ordinal))
            .collect::<Vec<_>>(),
        scene
            .points
            .iter()
            .filter_map(|point| point.debug.as_ref().map(|debug| debug.group_ordinal))
            .collect::<Vec<_>>(),
    );
    let point = scene
        .points
        .iter()
        .find(|point| {
            point
                .debug
                .as_ref()
                .is_some_and(|debug| debug.group_ordinal == 44)
        })
        .expect("point constrained to trace #43");
    match &point.constraint {
        ScenePointConstraint::OnPolyline {
            function_key,
            parameter,
            ..
        } => {
            assert_eq!(*function_key, 43);
            assert!(parameter.is_finite());
            assert!((0.0..=1.0).contains(parameter));
        }
        other => panic!("expected payload point-trace constraint, got {other:?}"),
    }
}

#[test]
fn triangle_rolling_trace_parameter_keeps_its_construction_chain() {
    let data =
        fixture_bytes("tests/Samples/个人专栏/贺基旭作品/圆在三角形边上滚动（成品 By hjx4882).gsp")
            .expect("triangle rolling fixture");
    let file = GspFile::parse(&data).expect("triangle rolling fixture parses");
    let groups = file.object_groups();
    let point_map = collect_point_objects(&file, &groups);
    let analysis = analyze_scene(&file, &groups, &point_map);
    for ordinal in 24usize..=29 {
        crate::runtime::functions::try_decode_function_expr(&file, &groups, &groups[ordinal - 1])
            .unwrap_or_else(|error| panic!("length scalar #{ordinal}: {error:?}"));
    }
    for ordinal in 33usize..=38 {
        crate::runtime::functions::try_decode_function_expr(&file, &groups, &groups[ordinal - 1])
            .unwrap_or_else(|error| panic!("segment expression #{ordinal}: {error:?}"));
    }
    for ordinal in 45usize..=49 {
        try_decode_parameter_controlled_point(
            &file,
            &groups,
            &groups[ordinal - 1],
            &analysis.raw_anchors,
        )
        .unwrap_or_else(|error| panic!("parameter point #{ordinal} must decode: {error:?}"));
    }
    try_decode_parameter_controlled_point(&file, &groups, &groups[62], &analysis.raw_anchors)
        .unwrap_or_else(|error| panic!("parameter point #63 must decode: {error:?}"));
    for ordinal in [64usize, 65, 66] {
        crate::runtime::functions::try_decode_function_expr(&file, &groups, &groups[ordinal - 1])
            .unwrap_or_else(|error| panic!("rotation expression #{ordinal}: {error:?}"));
    }

    let scene = build_scene_checked(&file).expect("triangle rolling scene builds");
    assert!(
        scene.lines.iter().any(|line| {
            line.debug
                .as_ref()
                .is_some_and(|debug| debug.group_ordinal == 74)
                && matches!(line.binding, Some(LineBinding::PointTrace { .. }))
        }),
        "point ordinals: {:?}",
        scene
            .points
            .iter()
            .filter_map(|point| point.debug.as_ref().map(|debug| debug.group_ordinal))
            .collect::<Vec<_>>()
    );
    assert!(matches!(
        scene.points.iter().find(|point| point
            .debug
            .as_ref()
            .is_some_and(|debug| debug.group_ordinal == 78)),
        Some(crate::runtime::scene::ScenePoint {
            constraint: ScenePointConstraint::OnPolyline {
                function_key: 74,
                ..
            },
            ..
        })
    ));
}

#[test]
fn quadrilateral_rolling_translations_keep_rotation_parents() {
    let data = fixture_bytes("tests/Samples/个人专栏/方小庆作品/圆在四边形上滚动(inRm).gsp")
        .expect("quadrilateral rolling fixture");
    let file = GspFile::parse(&data).expect("quadrilateral rolling fixture parses");
    let groups = file.object_groups();
    assert_eq!(
        super::decode::graph_object_circle_measurement_kind(&file, &groups[13]),
        Some(super::decode::GraphObjectCircleMeasurementKind::Circumference),
        "circle measurement payload: {:?}",
        groups[13]
            .records
            .iter()
            .map(|record| (record.record_type, record.payload(&file.data).to_vec()))
            .collect::<Vec<_>>()
    );
    for ordinal in [8usize, 14, 19] {
        crate::runtime::functions::try_decode_function_expr(&file, &groups, &groups[ordinal - 1])
            .unwrap_or_else(|error| panic!("scalar #{ordinal} must decode: {error:?}"));
    }
    let scene = build_scene_checked(&file).expect("quadrilateral rolling scene builds");

    for ordinal in [20, 37, 38, 39, 40, 44, 45, 46] {
        let point = scene
            .points
            .iter()
            .find(|point| {
                point
                    .debug
                    .as_ref()
                    .is_some_and(|debug| debug.group_ordinal == ordinal)
            })
            .unwrap_or_else(|| panic!("rotation parent #{ordinal} must be materialized"));
        assert!(matches!(
            point.binding,
            Some(ScenePointBinding::Rotate { .. })
        ));
    }
    for ordinal in 47..=54 {
        let point = scene
            .points
            .iter()
            .find(|point| {
                point
                    .debug
                    .as_ref()
                    .is_some_and(|debug| debug.group_ordinal == ordinal)
            })
            .unwrap_or_else(|| panic!("translated point #{ordinal}"));
        assert!(
            matches!(point.binding, Some(ScenePointBinding::Translate { .. })),
            "translated point #{ordinal}: {:?}",
            point.binding
        );
    }
}

#[test]
fn trajectory_polygon_parameter_restores_line_arc_intersection_chain() {
    let data = fixture_bytes("tests/Samples/个人专栏/贺基旭作品/轨迹(hjx4882).gsp")
        .expect("trajectory fixture");
    let document = GspFile::parse(&data).expect("trajectory fixture parses");
    let pages = document.page_files();
    let page = &pages[0];
    let groups = page.object_groups();
    let point_map = collect_point_objects(page, &groups);
    let analysis = analyze_scene(page, &groups, &point_map);
    crate::runtime::functions::try_decode_parameter_control_expr(page, &groups, &groups[9])
        .unwrap_or_else(|error| panic!("parameter expression #10 must decode: {error:?}"));
    let parameter_point =
        try_decode_parameter_controlled_point(page, &groups, &groups[10], &analysis.raw_anchors)
            .unwrap_or_else(|error| {
                panic!("parameter point #11 must decode from #9/#10: {error:?}")
            });
    assert!(matches!(
        parameter_point.constraint,
        super::points::RawPointConstraint::PolygonBoundary { .. }
    ));

    let scene = build_scene_checked(page).expect("first trajectory page builds");
    let point = |ordinal| {
        scene.points.iter().find(|point| {
            point
                .debug
                .as_ref()
                .is_some_and(|debug| debug.group_ordinal == ordinal)
        })
    };
    assert!(matches!(
        point(11).map(|point| &point.constraint),
        Some(ScenePointConstraint::OnPolygonBoundary { .. })
    ));
    assert!(matches!(
        point(19).map(|point| &point.constraint),
        Some(ScenePointConstraint::LineCircularIntersection { .. })
    ));
}

#[test]
fn walking_person_multi_parent_coordinate_restores_the_intersection_chain() {
    let data = fixture_bytes("tests/Samples/个人专栏/方小庆作品/步行拄拐人(inRm).gsp")
        .expect("walking-person fixture");
    let file = GspFile::parse(&data).expect("walking-person fixture parses");
    let groups = file.object_groups();
    let point_map = collect_point_objects(&file, &groups);
    let analysis = analyze_scene(&file, &groups, &point_map);
    let coordinate = super::points::decode_coordinate_point(
        &file,
        &groups,
        &groups[23],
        &analysis.raw_anchors,
        &analysis.graph_ref,
    )
    .expect("the payload-defined horizontal coordinate decodes");
    let anchor = analysis.raw_anchors[23]
        .as_ref()
        .expect("coordinate anchor");
    assert!((coordinate.position.x - anchor.x).abs() < 1e-12);
    assert!((coordinate.position.y - anchor.y).abs() < 1e-12);
    let scene = build_scene_checked(&file).expect("walking-person scene builds");
    let point = |ordinal| {
        scene.points.iter().find(|point| {
            point
                .debug
                .as_ref()
                .is_some_and(|debug| debug.group_ordinal == ordinal)
        })
    };
    assert!(matches!(
        point(24).and_then(|point| point.binding.as_ref()),
        Some(ScenePointBinding::CoordinateSource {
            parameter_group_ordinals,
            axis: crate::runtime::scene::CoordinateAxis::Horizontal,
            ..
        }) if parameter_group_ordinals
            == &std::collections::BTreeMap::from([
                ("OM".to_string(), 15),
                ("st".to_string(), 18),
            ])
    ));
    assert!(matches!(
        point(29).map(|point| &point.constraint),
        Some(ScenePointConstraint::LineIntersection { .. })
    ));
    assert!(matches!(
        point(98).map(|point| &point.constraint),
        Some(ScenePointConstraint::CircularIntersection { .. })
    ));
}

#[test]
fn parameter_coordinate_restores_isochronous_circle_intersections() {
    let data = fixture_bytes("tests/Samples/个人专栏/庞坤生作品/等时圆.gsp")
        .expect("isochronous-circle fixture");
    let file = GspFile::parse(&data).expect("fixture parses");
    let groups = file.object_groups();
    let point_map = collect_point_objects(&file, &groups);
    let analysis = analyze_scene(&file, &groups, &point_map);
    let coordinate = super::points::decode_coordinate_point(
        &file,
        &groups,
        &groups[3],
        &analysis.raw_anchors,
        &analysis.graph_ref,
    )
    .expect("parameter-controlled coordinate decodes");
    let anchor = analysis.raw_anchors[3]
        .as_ref()
        .expect("parameter-controlled coordinate anchor");
    assert!((coordinate.position.x - anchor.x).abs() < 1e-12);
    assert!((coordinate.position.y - anchor.y).abs() < 1e-12);

    let scene = build_scene_checked(&file).expect("isochronous-circle scene builds");
    let point = |ordinal| {
        scene.points.iter().find(|point| {
            point
                .debug
                .as_ref()
                .is_some_and(|debug| debug.group_ordinal == ordinal)
        })
    };
    assert!(matches!(
        point(4).and_then(|point| point.binding.as_ref()),
        Some(ScenePointBinding::CoordinateSource {
            parameter_group_ordinals,
            axis: crate::runtime::scene::CoordinateAxis::Vertical,
            ..
        }) if parameter_group_ordinals
            == &std::collections::BTreeMap::from([("圆半径R".to_string(), 2)])
    ));
    assert!(matches!(
        point(10).map(|point| &point.constraint),
        Some(ScenePointConstraint::LineCircularIntersection { .. })
    ));
    assert!(matches!(
        point(52).map(|point| &point.constraint),
        Some(ScenePointConstraint::LineIntersection { .. })
    ));
}

#[test]
fn fixed_translated_line_restores_running_person_dependency_chain() {
    let data = fixture_bytes("tests/Samples/个人专栏/方小庆作品/(inRm)跑步人.gsp")
        .expect("running-person fixture");
    let file = GspFile::parse(&data).expect("fixture parses");
    let scene = build_scene_checked(&file).expect("running-person scene builds");
    let point = |ordinal| {
        scene.points.iter().find(|point| {
            point
                .debug
                .as_ref()
                .is_some_and(|debug| debug.group_ordinal == ordinal)
        })
    };
    assert!(matches!(
        point(42).map(|point| &point.constraint),
        Some(ScenePointConstraint::OnLineConstraint {
            line: crate::runtime::scene::LineConstraint::MatrixApply { .. },
            ..
        })
    ));
    assert!(matches!(
        point(45).and_then(|point| point.binding.as_ref()),
        Some(ScenePointBinding::ScaleByRatio { .. })
    ));
    assert!(matches!(
        point(124).map(|point| &point.constraint),
        Some(ScenePointConstraint::CircularIntersection { variant: 1, .. })
    ));
    assert!(matches!(
        point(129),
        Some(crate::runtime::scene::ScenePoint {
            binding: Some(ScenePointBinding::ConstraintParameterFromPointExpr { .. }),
            constraint: ScenePointConstraint::OnPolyline { .. },
            ..
        })
    ));
}

#[test]
fn polar_offset_line_host_passes_payload_validation() {
    let data = fixture_bytes("tests/Samples/个人专栏/贺基旭作品/任意长方体展开(hjx4882).gsp")
        .expect("cuboid-unfolding fixture");
    let file = GspFile::parse(&data).expect("fixture parses");
    let scene = build_scene_checked(&file).expect("polar-offset line host builds");
    assert!(scene.lines.iter().any(|line| {
        line.debug.as_ref().is_some_and(|debug| {
            debug.group_ordinal == 23
                && debug.group_kind == crate::format::GroupKind::PerpendicularLine
        })
    }));
}

#[test]
fn fixed_coordinate_point_restores_heart_curve_transform_chain() {
    let Some(data) = fixture_bytes("tests/Samples/未分类档/心脏线.gsp") else {
        return;
    };
    let file = GspFile::parse(&data).expect("heart curve fixture parses");
    let groups = file.object_groups();
    let point_map = collect_point_objects(&file, &groups);
    let analysis = analyze_scene(&file, &groups, &point_map);
    let coordinate = super::points::decode_coordinate_point(
        &file,
        &groups,
        &groups[8],
        &analysis.raw_anchors,
        &analysis.graph_ref,
    )
    .expect("the payload-defined fixed coordinate point decodes");
    let anchor = analysis.raw_anchors[8]
        .as_ref()
        .expect("fixed coordinate point has a payload-derived anchor");
    assert!((coordinate.position.x - anchor.x).abs() < 1e-12);
    assert!((coordinate.position.y - anchor.y).abs() < 1e-12);

    let scene = build_scene_checked(&file).expect("heart curve fixture builds");
    let point_index = |ordinal| {
        scene
            .points
            .iter()
            .position(|point| {
                point
                    .debug
                    .as_ref()
                    .is_some_and(|debug| debug.group_ordinal == ordinal)
            })
            .unwrap_or_else(|| panic!("point group #{ordinal}"))
    };
    let origin = point_index(1);
    let center = point_index(9);
    let circle_point = point_index(11);
    let first_translation = point_index(13);
    let second_translation = point_index(14);
    let parameter_rotation = point_index(17);
    let fixed_rotation = point_index(20);
    let arc_point = point_index(23);

    assert!(matches!(
        scene.points[center].binding,
        Some(ScenePointBinding::CoordinateSource2d {
            source_index,
            ref x_expr,
            ref y_expr,
            ..
        }) if source_index == origin
            && matches!(x_expr, FunctionExpr::Constant(x) if (*x + 0.5).abs() < 1e-12)
            && matches!(y_expr, FunctionExpr::Constant(y) if y.abs() < 1e-12)
    ));
    assert!(matches!(
        scene.points[circle_point].constraint,
        ScenePointConstraint::OnCircle {
            center_index,
            radius_index,
            ..
        } if center_index == center && radius_index == origin
    ));
    assert!(matches!(
        scene.points[first_translation].binding,
        Some(ScenePointBinding::Translate {
            source_index,
            vector_start_index,
            vector_end_index,
        }) if source_index == circle_point
            && vector_start_index == center
            && vector_end_index == circle_point
    ));
    assert!(matches!(
        scene.points[second_translation].binding,
        Some(ScenePointBinding::Translate {
            source_index,
            vector_start_index,
            vector_end_index,
        }) if source_index == origin
            && vector_start_index == center
            && vector_end_index == first_translation
    ));
    assert!(matches!(
        scene.points[parameter_rotation].binding,
        Some(ScenePointBinding::Rotate {
            source_index,
            center_index,
            angle_start_index: Some(angle_start_index),
            angle_vertex_index: Some(angle_vertex_index),
            angle_end_index: Some(angle_end_index),
            ..
        }) if source_index == circle_point
            && center_index == first_translation
            && angle_start_index == origin
            && angle_vertex_index == center
            && angle_end_index == circle_point
    ));
    assert!(matches!(
        scene.points[fixed_rotation].binding,
        Some(ScenePointBinding::Rotate {
            source_index,
            center_index,
            angle_degrees,
            ..
        }) if source_index == parameter_rotation
            && center_index == first_translation
            && (angle_degrees - 359.0).abs() < 1e-12
    ));
    assert!(matches!(
        scene.points[arc_point].constraint,
        ScenePointConstraint::OnCircleArc {
            center_index,
            start_index,
            end_index,
            ..
        } if center_index == first_translation
            && start_index == parameter_rotation
            && end_index == fixed_rotation
    ));
    let arc = scene
        .arcs
        .iter()
        .find(|arc| {
            arc.debug
                .as_ref()
                .is_some_and(|debug| debug.group_ordinal == 22)
        })
        .expect("center arc #22");
    assert!(matches!(
        arc.binding,
        Some(ArcBinding::CenterArc {
            center_index,
            start_index,
            end_index,
        }) if center_index == first_translation
            && start_index == parameter_rotation
            && end_index == fixed_rotation
    ));
}

#[test]
fn polygon_rolling_translation_keeps_its_expression_and_measurement_parent_chain() {
    let Some(data) = fixture_bytes("tests/Samples/个人专栏/阮国祥作品/多边形在多边形上的滚动.gsp")
    else {
        return;
    };
    let file = GspFile::parse(&data).expect("polygon rolling fixture parses");
    let groups = file.object_groups();
    let point_map = collect_point_objects(&file, &groups);
    let analysis = analyze_scene(&file, &groups, &point_map);
    assert!(crate::runtime::functions::function_expr_uses_degree_units(
        &file,
        &groups,
        &groups[24]
    ));
    let (distance_expr, parameters, _) = super::points::expression_runtime_context(
        &file,
        &groups,
        &groups[24],
        &analysis.raw_anchors,
    )
    .expect("group #25 decodes a / tan((360 / n2) / 2) from the payload");
    let distance =
        crate::runtime::functions::evaluate_expr_with_parameters(&distance_expr, 0.0, &parameters)
            .expect("the payload angle unit makes the tangent argument 30 degrees");
    assert!((distance - 121.91182810124567).abs() < 1e-9);
    let endpoint = super::points::decode_derived_polar_endpoint_binding(
        &file,
        &groups,
        &groups[25],
        &analysis.raw_anchors,
    )
    .expect("derived endpoint #26 uses the live distance expression");
    assert_eq!(endpoint.center_group_index, 12);
    assert!((endpoint.distance_value - distance).abs() < 1e-9);

    let measured_distance =
        crate::runtime::functions::try_decode_function_expr(&file, &groups, &groups[17])
            .expect("measured value #18 is a scalar expression");
    assert!(matches!(
        measured_distance,
        FunctionExpr::Parsed(crate::runtime::functions::FunctionAst::Parameter(ref name, value))
            if name == "m[6]" && (value - 3.724583333333335).abs() < 1e-12
    ));

    let scene = build_scene_checked(&file).expect("polygon rolling fixture builds");
    let point_index = |ordinal| {
        scene
            .points
            .iter()
            .position(|point| {
                point
                    .debug
                    .as_ref()
                    .is_some_and(|debug| debug.group_ordinal == ordinal)
            })
            .unwrap_or_else(|| panic!("point group #{ordinal}"))
    };
    let parameter_rotation = point_index(38);
    let measured_transform = point_index(39);
    let parameter_point = point_index(68);
    let second_rotation = point_index(72);
    let third_rotation = point_index(73);
    let translation = point_index(74);

    assert!(matches!(
        scene.points[measured_transform].binding,
        Some(ScenePointBinding::PolarTransform {
            source_index,
            distance_scale,
            ref distance_expr,
            ..
        }) if source_index == parameter_rotation
            && (distance_scale - crate::runtime::DEFAULT_GRAPH_RAW_PER_UNIT).abs() < 1e-12
            && matches!(distance_expr,
                FunctionExpr::Parsed(crate::runtime::functions::FunctionAst::Parameter(name, value))
                    if name == "m[6]" && (*value - 3.724583333333335).abs() < 1e-12)
    ));
    assert!(matches!(
        scene.points[parameter_point].constraint,
        ScenePointConstraint::OnSegment {
            start_index,
            end_index,
            ..
        } if start_index == parameter_rotation && end_index == measured_transform
    ));
    assert!(matches!(
        scene.points[second_rotation].binding,
        Some(ScenePointBinding::Rotate { center_index, .. })
            if center_index == parameter_rotation
    ));
    assert!(matches!(
        scene.points[third_rotation].binding,
        Some(ScenePointBinding::Rotate {
            source_index,
            center_index,
            ..
        }) if source_index == second_rotation && center_index == parameter_rotation
    ));
    assert!(matches!(
        scene.points[translation].binding,
        Some(ScenePointBinding::Translate {
            source_index,
            vector_start_index,
            vector_end_index,
        }) if source_index == parameter_rotation
            && vector_start_index == third_rotation
            && vector_end_index == parameter_point
    ));
    let trace = scene
        .lines
        .iter()
        .find(|line| {
            line.debug
                .as_ref()
                .is_some_and(|debug| debug.group_ordinal == 75)
        })
        .expect("custom transform trace #75");
    assert!(matches!(
        trace.binding,
        Some(LineBinding::CustomTransformTrace { point_index, .. })
            if point_index == translation
    ));
}

#[test]
fn moon_center_arcs_keep_parameter_point_endpoint_chains() {
    let Some(data) = fixture_bytes(
        "tests/Samples/个人专栏/庞坤生作品/月球的公转和自转（为何看不到月球背面）.gsp",
    ) else {
        return;
    };
    let file = GspFile::parse(&data).expect("moon fixture parses");
    for (page_index, page) in file.page_files().iter().enumerate() {
        let scene = build_scene_checked(page).expect("moon page builds");
        let point_index = |ordinal| {
            scene
                .points
                .iter()
                .position(|point| {
                    point
                        .debug
                        .as_ref()
                        .is_some_and(|debug| debug.group_ordinal == ordinal)
                })
                .unwrap_or_else(|| panic!("page {} point group #{ordinal}", page_index + 1))
        };
        let assert_polar_offset = |ordinal, source_ordinal, value: f64| {
            assert!(matches!(
                scene.points[point_index(ordinal)].binding,
                Some(ScenePointBinding::PolarOffset {
                    source_index,
                    ref distance_expr,
                    ..
                }) if source_index == point_index(source_ordinal)
                    && matches!(distance_expr, FunctionExpr::Parsed(
                        crate::runtime::functions::FunctionAst::Parameter(_, actual)
                    ) if (*actual - value).abs() < 1e-12)
            ));
        };
        let assert_rotation = |ordinal, source_ordinal, center_ordinal| {
            assert!(matches!(
                scene.points[point_index(ordinal)].binding,
                Some(ScenePointBinding::Rotate {
                    source_index,
                    center_index,
                    ..
                }) if source_index == point_index(source_ordinal)
                    && center_index == point_index(center_ordinal)
            ));
        };
        let assert_arc = |ordinal, center_ordinal, start_ordinal, end_ordinal| {
            let arc = scene
                .arcs
                .iter()
                .find(|arc| {
                    arc.debug
                        .as_ref()
                        .is_some_and(|debug| debug.group_ordinal == ordinal)
                })
                .unwrap_or_else(|| panic!("page {} arc group #{ordinal}", page_index + 1));
            assert!(matches!(
                arc.binding,
                Some(ArcBinding::CenterArc {
                    center_index,
                    start_index,
                    end_index,
                }) if center_index == point_index(center_ordinal)
                    && start_index == point_index(start_ordinal)
                    && end_index == point_index(end_ordinal)
            ));
        };

        assert_polar_offset(5, 1, 5.9);
        match page_index {
            0 => {
                assert_polar_offset(14, 5, 0.7);
                assert_polar_offset(15, 5, 0.7);
                assert_rotation(16, 5, 1);
                assert_rotation(17, 15, 1);
                assert_rotation(18, 14, 1);
                assert_arc(20, 16, 17, 18);
                assert_arc(21, 16, 18, 17);
            }
            1 => {
                assert_rotation(22, 5, 1);
                assert_polar_offset(24, 22, 0.7);
                assert_polar_offset(25, 22, 0.7);
                assert_arc(26, 22, 24, 25);
                assert_arc(28, 22, 25, 24);
            }
            _ => panic!("unexpected moon fixture page {}", page_index + 1),
        }
    }
}

#[test]
fn half_sector_pages_keep_scale_translation_arc_and_intersection_chains() {
    let Some(data) = fixture_bytes("tests/Samples/热研系列/滚动系列/半圆扇形滚动操作详解.gsp")
    else {
        return;
    };
    let file = GspFile::parse(&data).expect("half-sector fixture parses");
    for (page_index, page) in file.page_files().iter().enumerate() {
        let scene = build_scene_checked(page)
            .unwrap_or_else(|error| panic!("half-sector page {} builds: {error}", page_index + 1));
        let point_index = |ordinal| {
            scene
                .points
                .iter()
                .position(|point| {
                    point
                        .debug
                        .as_ref()
                        .is_some_and(|debug| debug.group_ordinal == ordinal)
                })
                .unwrap_or_else(|| panic!("page {} point group #{ordinal}", page_index + 1))
        };
        let assert_translation = |ordinal, source_ordinal, start_ordinal, end_ordinal| {
            assert!(matches!(
                scene.points[point_index(ordinal)].binding,
                Some(ScenePointBinding::Translate {
                    source_index,
                    vector_start_index,
                    vector_end_index,
                }) if source_index == point_index(source_ordinal)
                    && vector_start_index == point_index(start_ordinal)
                    && vector_end_index == point_index(end_ordinal)
            ));
        };
        let assert_arc = |ordinal, center_ordinal, start_ordinal, end_ordinal| {
            let arc = scene
                .arcs
                .iter()
                .find(|arc| {
                    arc.debug
                        .as_ref()
                        .is_some_and(|debug| debug.group_ordinal == ordinal)
                })
                .unwrap_or_else(|| panic!("page {} arc group #{ordinal}", page_index + 1));
            assert!(matches!(
                arc.binding,
                Some(ArcBinding::CenterArc {
                    center_index,
                    start_index,
                    end_index,
                }) if center_index == point_index(center_ordinal)
                    && start_index == point_index(start_ordinal)
                    && end_index == point_index(end_ordinal)
            ));
        };

        assert!(matches!(
            scene.points[point_index(6)].binding,
            Some(ScenePointBinding::Scale {
                source_index,
                center_index,
                factor,
                ref parameter_name,
                factor_expr: Some(_),
                ..
            }) if source_index == point_index(4)
                && center_index == point_index(1)
                && (factor - 2.0).abs() < 1e-12
                && parameter_name.as_deref() == Some("m₃")
        ));
        assert_translation(7, 6, 4, 1);
        assert!(matches!(
            scene.points[point_index(12)].binding,
            Some(ScenePointBinding::Scale {
                source_index,
                center_index,
                factor_expr: Some(_),
                ..
            }) if source_index == point_index(6) && center_index == point_index(1)
        ));
        assert_translation(16, 15, 1, 4);
        assert_translation(17, 15, 4, 1);
        assert_arc(18, 15, 16, 17);

        if page_index == 1 {
            assert!(matches!(
                scene.points[point_index(24)].constraint,
                ScenePointConstraint::LineIntersection { .. }
            ));
            assert!(matches!(
                scene.points[point_index(26)].constraint,
                ScenePointConstraint::LineCircularIntersection { .. }
            ));
            assert_arc(31, 26, 28, 29);
        }
    }
}

#[test]
fn fraction_arc_keeps_non_finite_expression_parents_symbolic() {
    let Some(data) = fixture_bytes("tests/Samples/个人专栏/钟科作品/分数有意义（颗粒）.gsp")
    else {
        return;
    };
    let file = GspFile::parse(&data).expect("fraction fixture parses");
    let groups = file.object_groups();
    crate::runtime::functions::try_decode_function_expr(&file, &groups, &groups[68])
        .expect("group #69 remains a symbolic expression when group #33 is initially non-finite");
    assert_eq!(
        crate::runtime::functions::function_parameter_group_ordinals(&file, &groups, &groups[68])
            .get("分母₃"),
        Some(&33)
    );

    let scene = build_scene_checked(&file).expect("fraction fixture builds");
    let point_index = |ordinal| {
        scene
            .points
            .iter()
            .position(|point| {
                point
                    .debug
                    .as_ref()
                    .is_some_and(|debug| debug.group_ordinal == ordinal)
            })
            .unwrap_or_else(|| panic!("point group #{ordinal}"))
    };
    assert!(matches!(
        scene.points[point_index(92)].binding,
        Some(ScenePointBinding::Rotate {
            source_index,
            center_index,
            angle_expr: Some(_),
            ref angle_parameter_group_ordinals,
            ..
        }) if source_index == point_index(68)
            && center_index == point_index(67)
            && angle_parameter_group_ordinals.get("分母₃") == Some(&33)
    ));
    assert!(matches!(
        scene.points[point_index(93)].binding,
        Some(ScenePointBinding::Rotate {
            source_index,
            center_index,
            angle_start_index: Some(angle_start_index),
            angle_vertex_index: Some(angle_vertex_index),
            angle_end_index: Some(angle_end_index),
            ..
        }) if source_index == point_index(68)
            && center_index == point_index(67)
            && angle_start_index == point_index(92)
            && angle_vertex_index == point_index(67)
            && angle_end_index == point_index(68)
    ));
    assert!(matches!(
        scene.points[point_index(94)].binding,
        Some(ScenePointBinding::Rotate {
            source_index,
            center_index,
            angle_expr: Some(_),
            ..
        }) if source_index == point_index(93) && center_index == point_index(67)
    ));
    let arc = scene
        .arcs
        .iter()
        .find(|arc| {
            arc.debug
                .as_ref()
                .is_some_and(|debug| debug.group_ordinal == 95)
        })
        .expect("center arc #95");
    assert!(matches!(
        arc.binding,
        Some(ArcBinding::CenterArc {
            center_index,
            start_index,
            end_index,
        }) if center_index == point_index(67)
            && start_index == point_index(93)
            && end_index == point_index(94)
    ));
}

#[test]
fn arbitrary_sector_arc_uses_the_measured_arc_angle_as_a_scalar_parent() {
    let Some(data) = fixture_bytes("tests/Samples/个人专栏/高峻清作品/任意角扇形的滚动(gjq).gsp")
    else {
        return;
    };
    let file = GspFile::parse(&data).expect("arbitrary-sector fixture parses");
    let scene = build_scene_checked(&file).expect("arbitrary-sector fixture builds");
    let point_index = |ordinal| {
        scene
            .points
            .iter()
            .position(|point| {
                point
                    .debug
                    .as_ref()
                    .is_some_and(|debug| debug.group_ordinal == ordinal)
            })
            .unwrap_or_else(|| panic!("point group #{ordinal}"))
    };
    assert!(matches!(
        scene.points[point_index(49)].binding,
        Some(ScenePointBinding::Rotate {
            source_index,
            center_index,
            angle_expr: Some(_),
            ref angle_parameter_group_ordinals,
            ..
        }) if source_index == point_index(48)
            && center_index == point_index(46)
            && angle_parameter_group_ordinals.get("__arc_angle_7") == Some(&7)
    ));
    for (ordinal, source_ordinal, center_ordinal) in [
        (51, 46, 49),
        (52, 48, 49),
        (54, 49, 51),
        (55, 52, 51),
        (57, 51, 55),
        (58, 54, 55),
    ] {
        assert!(matches!(
            scene.points[point_index(ordinal)].binding,
            Some(ScenePointBinding::Rotate {
                source_index,
                center_index,
                angle_expr: Some(_),
                ..
            }) if source_index == point_index(source_ordinal)
                && center_index == point_index(center_ordinal)
        ));
    }
    let arc = scene
        .arcs
        .iter()
        .find(|arc| {
            arc.debug
                .as_ref()
                .is_some_and(|debug| debug.group_ordinal == 61)
        })
        .expect("center arc #61");
    assert!(matches!(
        arc.binding,
        Some(ArcBinding::CenterArc {
            center_index,
            start_index,
            end_index,
        }) if center_index == point_index(57)
            && start_index == point_index(55)
            && end_index == point_index(58)
    ));
}

#[test]
fn decodes_involute_parameter_rotation_chain() {
    let Some(data) =
        fixture_bytes("tests/Samples/个人专栏/周维波作品/正n边形的渐开线（雪山飞狐）.gsp")
    else {
        return;
    };
    let file = GspFile::parse(&data).expect("involute fixture parses");
    let groups = file.object_groups();
    let point_map = collect_point_objects(&file, &groups);
    let analysis = analyze_scene(&file, &groups, &point_map);
    let binding =
        decode_expression_scale_binding(&file, &groups, &groups[17], &analysis.raw_anchors)
            .expect("the paired HTM defines group #18 as Dilation/MarkedRatio");
    assert_eq!(binding.source_group_index, 0);
    assert_eq!(binding.center_group_index, 15);
    assert!((binding.factor - 3.0_f64.sqrt().recip()).abs() < 1e-12);
}

#[test]
fn circular_arc_parameter_anchor_drives_expression_rotation() {
    let data = fixture_bytes("tests/Samples/个人专栏/方小庆作品/(inRm)圆柱圆锥展开.gsp")
        .expect("cylinder-cone fixture");
    let file = GspFile::parse(&data).expect("cylinder-cone fixture parses");
    let page = &file.page_files()[0];
    let scene = build_scene_checked(page).expect("first page builds");
    let point = |ordinal| {
        scene
            .points
            .iter()
            .find(|point| {
                point
                    .debug
                    .as_ref()
                    .is_some_and(|debug| debug.group_ordinal == ordinal)
            })
            .unwrap_or_else(|| panic!("missing derived point #{ordinal}"))
    };
    assert!(
        matches!(
            &point(22).binding,
            Some(ScenePointBinding::Rotate {
                angle_degrees,
                angle_expr: Some(_),
                angle_parameter_group_ordinals,
                ..
            }) if (*angle_degrees - 149.366_407_980_351_34).abs() < 1e-9
                && angle_parameter_group_ordinals.get("m") == Some(&19)
        ),
        "group #22 binding: {:?}",
        point(22).binding
    );
    assert!(matches!(
        point(23).binding,
        Some(ScenePointBinding::Translate { .. })
    ));
}

#[test]
fn measurement_line_parameter_point_drives_vector_translation() {
    let data = fixture_bytes("tests/Samples/未分类档/平移正弦线作正弦函数图像.gsp")
        .expect("translated-sine fixture");
    let file = GspFile::parse(&data).expect("translated-sine fixture parses");
    let scene = build_scene_checked(&file).expect("translated-sine fixture builds");
    let point = |ordinal| {
        scene
            .points
            .iter()
            .find(|point| {
                point
                    .debug
                    .as_ref()
                    .is_some_and(|debug| debug.group_ordinal == ordinal)
            })
            .unwrap_or_else(|| panic!("missing derived point #{ordinal}"))
    };
    assert!(matches!(
        point(18).constraint,
        ScenePointConstraint::OnSegment { .. }
    ));
    assert!(matches!(
        point(24).binding,
        Some(ScenePointBinding::Translate { .. })
    ));
}

#[test]
fn initially_undefined_polar_endpoint_keeps_scale_translation_chain() {
    let data = fixture_bytes(
        "tests/Samples/个人专栏/孙禄京作品/温州市龙湾区实验中学适应性测试试题(孙禄京).gsp",
    )
    .expect("conditional-dilation fixture");
    let file = GspFile::parse(&data).expect("conditional-dilation fixture parses");
    let scene = build_scene_checked(&file).expect("conditional-dilation fixture builds");
    let point = |ordinal| {
        scene
            .points
            .iter()
            .find(|point| {
                point
                    .debug
                    .as_ref()
                    .is_some_and(|debug| debug.group_ordinal == ordinal)
            })
            .unwrap_or_else(|| panic!("missing derived point #{ordinal}"))
    };
    assert!(matches!(
        point(65).binding,
        Some(ScenePointBinding::PolarOffset { .. })
    ));
    assert!(matches!(
        point(66).binding,
        Some(ScenePointBinding::Scale { factor, .. }) if (factor - 1.0).abs() < 1e-12
    ));
    assert!(matches!(
        point(69).binding,
        Some(ScenePointBinding::Translate { .. })
    ));
}

#[test]
fn initially_undefined_rotated_ray_keeps_circle_intersection_chain() {
    let data = fixture_bytes("tests/Samples/未分类档/圆内点的弹性束缚 (3).gsp")
        .expect("elastic-circle fixture");
    let file = GspFile::parse(&data).expect("elastic-circle fixture parses");
    let scene = build_scene_checked(&file).expect("elastic-circle fixture builds");
    let rotated_ray = scene
        .lines
        .iter()
        .find(|line| {
            line.debug
                .as_ref()
                .is_some_and(|debug| debug.group_ordinal == 14)
        })
        .expect("rotated ray #14");
    assert!(matches!(
        &rotated_ray.binding,
        Some(LineBinding::MatrixApply { matrices, .. })
            if matches!(matrices.as_slice(), [GeometryTransformBinding::Rotate(binding)]
                if binding.angle_degrees == 0.0 && binding.angle_expr.is_some())
    ));
    let intersection = scene
        .points
        .iter()
        .find(|point| {
            point
                .debug
                .as_ref()
                .is_some_and(|debug| debug.group_ordinal == 15)
        })
        .expect("intersection #15");
    assert!(matches!(
        intersection.constraint,
        ScenePointConstraint::LineCircularIntersection { variant: 1, .. }
    ));
}

#[test]
fn boundary_curve_length_radius_keeps_circle_intersection_chain() {
    let data = fixture_bytes("tests/Samples/热研系列/滚动系列/三角车轮.gsp")
        .expect("triangle-wheel fixture");
    let file = GspFile::parse(&data).expect("triangle-wheel fixture parses");
    let scene = build_scene_checked(&file).expect("triangle-wheel fixture builds");
    let circle = scene
        .circles
        .iter()
        .find(|circle| {
            circle
                .debug
                .as_ref()
                .is_some_and(|debug| debug.group_ordinal == 29)
        })
        .expect("arc-length radius circle #29");
    assert!(matches!(
        &circle.binding,
        Some(ShapeBinding::ExpressionRadiusCircle {
            parameter_group_ordinals,
            ..
        }) if parameter_group_ordinals.values().copied().eq([24])
    ));
    let intersection = scene
        .points
        .iter()
        .find(|point| {
            point
                .debug
                .as_ref()
                .is_some_and(|debug| debug.group_ordinal == 30)
        })
        .expect("second circle intersection #30");
    assert!(matches!(
        intersection.constraint,
        ScenePointConstraint::LineCircularIntersection { variant: 1, .. }
    ));
}

#[test]
fn parameter_anchor_on_arc_keeps_its_scalar_parent_chain() {
    let data = fixture_bytes("tests/Samples/个人专栏/向忠作品/正弦波·音乐【电子琴】.gsp")
        .expect("electronic-keyboard fixture");
    let file = GspFile::parse(&data).expect("electronic-keyboard fixture parses");
    let scene = build_scene_checked(&file).expect("electronic-keyboard fixture builds");
    let scalar = scene
        .scalars
        .iter()
        .find(|scalar| scalar.group_ordinal == 62)
        .expect("parameter anchor #62");
    assert!(matches!(
        scalar.binding,
        SceneScalarBinding::PointArcParameter { .. }
    ));
}

#[test]
fn point_grid_boundary_payload_keeps_polygon_parent_chain() {
    let data = fixture_bytes("tests/Samples/个人专栏/向忠作品/点阵的局部放大.gsp")
        .expect("point-grid fixture");
    let file = GspFile::parse(&data).expect("point-grid fixture parses");
    let groups = file.object_groups();
    let point_map = collect_point_objects(&file, &groups);
    let analysis = analyze_scene(&file, &groups, &point_map);
    let polygon_point = try_decode_point_constraint(
        &file,
        &groups,
        &groups[20],
        Some(&analysis.raw_anchors),
        &analysis.graph_ref,
    )
    .unwrap_or_else(|error| panic!("polygon path point #21 must decode: {error:?}"));
    assert!(matches!(
        polygon_point,
        super::points::RawPointConstraint::PolygonBoundaryParameter { parameter, .. }
            if parameter == 0.0
    ));
    let scene = build_scene_checked(&file).expect("point-grid fixture builds");
    let point = |ordinal| {
        scene
            .points
            .iter()
            .find(|point| {
                point
                    .debug
                    .as_ref()
                    .is_some_and(|debug| debug.group_ordinal == ordinal)
            })
            .unwrap_or_else(|| panic!("missing point group #{ordinal}"))
    };
    assert!(matches!(
        point(20).constraint,
        ScenePointConstraint::OnPolygonBoundaryParameter { parameter, .. }
            if parameter == 0.5
    ));
    assert!(matches!(
        point(21).constraint,
        ScenePointConstraint::OnPolygonBoundaryParameter { parameter, .. }
            if parameter == 0.0
    ));
    assert!(matches!(
        point(35).constraint,
        ScenePointConstraint::LinePolygonIntersection { .. }
    ));
    assert!(scene.scalars.iter().any(|scalar| {
        scalar.group_ordinal == 36
            && matches!(
                scalar.binding,
                SceneScalarBinding::PointPolygonParameter { .. }
            )
    }));
}

#[test]
fn translated_point_keeps_function_rotation_parent_chain() {
    let Some(data) = fixture_bytes("tests/Samples/个人专栏/潘建平作品/40牛潘建平老师.gsp")
    else {
        return;
    };
    let file = GspFile::parse(&data).expect("translation fixture parses");
    let scene = build_scene_checked(&file).expect("translation fixture builds");
    let point_index = |ordinal| {
        scene
            .points
            .iter()
            .position(|point| {
                point
                    .debug
                    .as_ref()
                    .is_some_and(|debug| debug.group_ordinal == ordinal)
            })
            .unwrap_or_else(|| panic!("point group #{ordinal}"))
    };
    let source = point_index(11);
    let center = point_index(1);
    let rotated = point_index(12);
    let vector_end = point_index(13);
    let translated = point_index(14);
    assert!(matches!(
        scene.points[rotated].binding,
        Some(ScenePointBinding::Rotate {
            source_index,
            center_index,
            angle_expr: Some(_),
            ..
        }) if source_index == source && center_index == center
    ));
    assert!(matches!(
        scene.points[translated].binding,
        Some(ScenePointBinding::Translate {
            source_index,
            vector_start_index,
            vector_end_index,
        }) if source_index == rotated
            && vector_start_index == center
            && vector_end_index == vector_end
    ));
}

#[test]
fn transformed_line_intersections_keep_nested_line_parents() {
    let Some(data) = fixture_bytes("tests/Samples/个人专栏/钮炳坤作品/椭球（钮炳坤老师）.gsp")
    else {
        return;
    };
    let file = GspFile::parse(&data).expect("ellipsoid fixture parses");
    let scene = build_scene_checked(&file).expect("ellipsoid fixture builds");
    let point_index = |ordinal| {
        scene
            .points
            .iter()
            .position(|point| {
                point
                    .debug
                    .as_ref()
                    .is_some_and(|debug| debug.group_ordinal == ordinal)
            })
            .unwrap_or_else(|| panic!("point group #{ordinal}"))
    };
    let translated_circle_point = point_index(13);
    let circle_center = point_index(6);
    let circle_radius = point_index(8);
    let vector_end = point_index(5);
    assert!(matches!(
        scene.points[translated_circle_point].constraint,
        ScenePointConstraint::OnCircularConstraint {
            circle: CircularConstraint::VectorTranslateCircle {
                ref source,
                vector_start_index,
                vector_end_index,
            },
            ..
        } if matches!(
            source.as_ref(),
            CircularConstraint::Circle {
                center_index,
                radius_index,
            } if *center_index == circle_center && *radius_index == circle_radius
        ) && vector_start_index == circle_center && vector_end_index == vector_end
    ));
    for ordinal in [26, 27, 29, 30, 34, 38] {
        assert!(matches!(
            scene.points[point_index(ordinal)].constraint,
            ScenePointConstraint::LineIntersection { .. }
        ));
    }
}

#[test]
fn projected_coordinate_points_keep_their_payload_parent_chain() {
    let Some(data) = fixture_bytes("tests/Samples/个人专栏/况永胜作品/正方体的展开（3D效果）.gsp")
    else {
        return;
    };
    let file = GspFile::parse(&data).expect("cube fixture parses");
    let scene = build_scene_checked(&file).expect("cube fixture builds");
    let point_index = |ordinal| {
        scene
            .points
            .iter()
            .position(|point| {
                point
                    .debug
                    .as_ref()
                    .is_some_and(|debug| debug.group_ordinal == ordinal)
            })
            .unwrap_or_else(|| panic!("point group #{ordinal}"))
    };
    for (ordinal, parent_ordinals) in [(23, [15, 22, 10, 11, 15]), (26, [15, 25, 10, 11, 15])] {
        let point = &scene.points[point_index(ordinal)];
        assert!(matches!(
            &point.binding,
            Some(ScenePointBinding::ProjectedCoordinate {
                source_index,
                parent_group_ordinals,
                source_parent: 0,
            }) if *source_index == point_index(parent_ordinals[0])
                && parent_group_ordinals == &parent_ordinals
        ));
    }
}

#[test]
fn sliding_polygon_third_page_trace_chain_uses_payload_objects() {
    let Some(data) =
        fixture_bytes("tests/Samples/个人专栏/方小庆作品/多边形沿两定点滑动(inRm).gsp")
    else {
        return;
    };
    let document = GspFile::parse(&data).expect("sliding polygon fixture parses");
    let page = &document.page_files()[2];
    let scene = build_scene_checked(page).expect("sliding polygon page 3 builds");
    let point = |ordinal| {
        scene
            .points
            .iter()
            .find(|point| {
                point
                    .debug
                    .as_ref()
                    .is_some_and(|debug| debug.group_ordinal == ordinal)
            })
            .unwrap_or_else(|| panic!("point group #{ordinal}"))
    };
    for ordinal in [25, 38] {
        assert!(matches!(
            point(ordinal).constraint,
            ScenePointConstraint::LineTraceIntersection { .. }
        ));
    }
    assert!(matches!(
        point(31).constraint,
        ScenePointConstraint::CircularTraceIntersection {
            circle: CircularConstraint::CircleArc { .. },
            ..
        }
    ));
    for ordinal in [27, 28, 29] {
        assert!(matches!(
            point(ordinal).constraint,
            ScenePointConstraint::LineCircularIntersection { .. }
                | ScenePointConstraint::LineCircleIntersection { .. }
        ));
    }
    assert!(
        matches!(point(33).binding, Some(ScenePointBinding::Rotate { .. })),
        "group #33 binding: {:?}",
        point(33).binding
    );
    assert!(
        matches!(point(34).binding, Some(ScenePointBinding::Translate { .. })),
        "group #34 binding: {:?}",
        point(34).binding
    );
    assert!(scene.lines.iter().any(|line| {
        line.debug
            .as_ref()
            .is_some_and(|debug| debug.group_ordinal == 37)
            && matches!(line.binding, Some(LineBinding::PointTrace { .. }))
    }));
}

#[test]
fn ellipse_trace_intersection_chain_uses_payload_objects() {
    let Some(data) = fixture_bytes("tests/Samples/个人专栏/向忠作品/椭圆的判定实验.gsp")
    else {
        return;
    };
    let file = GspFile::parse(&data).expect("ellipse fixture parses");
    let groups = file.object_groups();
    let point_map = collect_point_objects(&file, &groups);
    let analysis = analyze_scene(&file, &groups, &point_map);
    let rotation = super::points::decode_expression_rotation_binding(
        &file,
        &groups,
        &groups[104],
        &analysis.raw_anchors,
    )
    .expect("group #105 carries the calculated-rotation payload class");
    assert_eq!(rotation.source_group_index, 103);
    assert_eq!(rotation.center_group_index, 23);
    assert_eq!(rotation.parameter_name.as_deref(), Some("拖我"));
    assert_eq!(rotation.angle_degrees, 0.0);
    let scale =
        decode_expression_scale_binding(&file, &groups, &groups[161], &analysis.raw_anchors)
            .expect("group #162 preserves its marked-ratio expression outside the initial domain");
    assert_eq!(scale.source_group_index, 23);
    assert_eq!(scale.center_group_index, 158);
    assert_eq!(scale.factor, 1.0);

    let scene = build_scene_checked(&file).expect("ellipse fixture builds");
    let point = |ordinal| {
        scene
            .points
            .iter()
            .find(|point| {
                point
                    .debug
                    .as_ref()
                    .is_some_and(|debug| debug.group_ordinal == ordinal)
            })
            .unwrap_or_else(|| panic!("missing derived point #{ordinal}"))
    };
    assert!(matches!(
        point(105).binding,
        Some(ScenePointBinding::Rotate {
            angle_expr: Some(_),
            ..
        })
    ));
    assert!(
        matches!(
            point(119).constraint,
            ScenePointConstraint::LineTraceIntersection { .. }
        ),
        "group #119 constraint: {:?}, binding: {:?}",
        point(119).constraint,
        point(119).binding
    );
    assert!(matches!(
        point(138).constraint,
        ScenePointConstraint::CircularTraceIntersection { .. }
    ));
    assert!(matches!(
        point(162).binding,
        Some(ScenePointBinding::Scale {
            factor_expr: Some(_),
            ..
        })
    ));
    for ordinal in [164, 170] {
        assert!(matches!(
            point(ordinal).constraint,
            ScenePointConstraint::LineTraceIntersection { .. }
        ));
    }
}

#[test]
fn angle_bisector_fixture_decodes_its_angle_anchor_and_reflected_arc_from_payload() {
    let Some(data) =
        fixture_bytes("tests/Samples/个人专栏/周维波作品/角平分线的尺规作图（雪山飞狐）.gsp")
    else {
        return;
    };
    let file = GspFile::parse(&data).expect("angle-bisector fixture parses");
    let groups = file.object_groups();
    let anchor = decode_directed_angle_anchor_binding(&file, &groups[11])
        .expect("group #12 is the four-parent directed-angle anchor");
    assert_eq!(
        [
            anchor.first_start_group_index,
            anchor.first_end_group_index,
            anchor.second_start_group_index,
            anchor.second_end_group_index,
        ],
        [0, 3, 0, 9]
    );
    assert!((anchor.distance - 18.89763779527559).abs() < 1e-12);
    assert_eq!(anchor.parameter, 0.5);

    let scene = build_scene_checked(&file).expect("angle-bisector scene builds");
    let controlled = scene
        .points
        .iter()
        .find(|point| {
            point
                .debug
                .as_ref()
                .is_some_and(|debug| debug.group_ordinal == 71)
        })
        .expect("point #71 on the reflected arc");
    assert!(matches!(
        controlled.constraint,
        ScenePointConstraint::OnArcConstraint {
            arc: ArcConstraint::Reflected { .. },
            ..
        }
    ));
    let reflected_arc = scene
        .arcs
        .iter()
        .find(|arc| {
            arc.debug
                .as_ref()
                .is_some_and(|debug| debug.group_ordinal == 61)
        })
        .expect("reflected arc #61 is materialized");
    assert!(matches!(
        reflected_arc.binding,
        Some(ArcBinding::MatrixApply { .. })
    ));

    let expression_circle_intersection = scene
        .points
        .iter()
        .find(|point| {
            point
                .debug
                .as_ref()
                .is_some_and(|debug| debug.group_ordinal == 45)
        })
        .expect("intersection #45 uses the expression-radius circle");
    assert!(matches!(
        expression_circle_intersection.constraint,
        ScenePointConstraint::LineCircularIntersection {
            circle: CircularConstraint::ExpressionRadiusCircle {
                ref parameter_group_ordinals,
                ..
            },
            variant: 1,
            ..
        } if parameter_group_ordinals.get("DC") == Some(&42)
    ));

    let expression_rotation = scene
        .points
        .iter()
        .find(|point| {
            point
                .debug
                .as_ref()
                .is_some_and(|debug| debug.group_ordinal == 68)
        })
        .expect("point #68 uses the parameter-anchor expression rotation");
    assert!(matches!(
        expression_rotation.binding,
        Some(ScenePointBinding::Rotate {
            ref angle_parameter_group_ordinals,
            ..
        }) if angle_parameter_group_ordinals.get("E") == Some(&65)
    ));

    let final_intersection = scene
        .points
        .iter()
        .find(|point| {
            point
                .debug
                .as_ref()
                .is_some_and(|debug| debug.group_ordinal == 87)
        })
        .expect("point #87 remains a derived line intersection");
    assert!(matches!(
        final_intersection.constraint,
        ScenePointConstraint::LineIntersection { .. }
    ));
}

#[test]
fn rolling_sector_parameter_anchor_drives_hidden_arc_center() {
    let Some(data) = fixture_bytes("tests/Samples/个人专栏/方小庆作品/扇形滚动(inRm).gsp")
    else {
        return;
    };
    let document = GspFile::parse(&data).expect("rolling-sector fixture parses");
    let pages = document.page_files();
    assert_eq!(pages.len(), 4);
    let file = &pages[0];
    let groups = file.object_groups();
    let point_map = collect_point_objects(file, &groups);
    let analysis = analyze_scene(file, &groups, &point_map);
    let controlled =
        try_decode_parameter_controlled_point(file, &groups, &groups[13], &analysis.raw_anchors)
            .expect("payload point #14 decodes from ParameterAnchor #13");
    assert_eq!(controlled.source_point_group_index, Some(4));
    assert!(matches!(
        controlled.constraint,
        super::points::RawPointConstraint::ConstructedLine {
            host_group_index: 11,
            line_like_kind: LineLikeKind::Segment,
            ..
        }
    ));
    let scene = build_scene_checked(file).expect("rolling-sector first page builds");
    assert!(scene.points.iter().any(|point| {
        point
            .debug
            .as_ref()
            .is_some_and(|debug| debug.group_ordinal == 14)
    }));

    for (page_index, page) in pages.iter().enumerate() {
        let scene = build_scene_checked(page)
            .unwrap_or_else(|error| panic!("rolling-sector page {}: {error:#}", page_index + 1));
        assert!(
            scene.arcs.iter().all(|arc| arc.binding.is_some()),
            "every arc on rolling-sector page {} must retain exact point parents",
            page_index + 1
        );
        if let Some(boundary_point_ordinal) = [None, Some(18), Some(14), Some(26)][page_index] {
            let point = scene
                .points
                .iter()
                .find(|point| {
                    point.debug.as_ref().is_some_and(|debug| {
                        debug.group_ordinal == boundary_point_ordinal
                    })
                })
                .unwrap_or_else(|| {
                    panic!(
                        "rolling-sector page {} boundary-length coordinate point #{boundary_point_ordinal}",
                        page_index + 1
                    )
                });
            assert!(matches!(
                point.binding,
                Some(ScenePointBinding::BoundaryLengthOffset { .. })
            ));
        }
        if let Some(controlled_ordinal) = [None, Some(28), None, Some(34)][page_index] {
            let point = scene
                .points
                .iter()
                .find(|point| {
                    point
                        .debug
                        .as_ref()
                        .is_some_and(|debug| debug.group_ordinal == controlled_ordinal)
                })
                .unwrap_or_else(|| {
                    panic!(
                        "rolling-sector page {} translated-line control point #{controlled_ordinal}",
                        page_index + 1
                    )
                });
            assert!(matches!(
                point.binding,
                Some(ScenePointBinding::DerivedParameter { .. })
            ));
            assert!(matches!(
                point.constraint,
                ScenePointConstraint::OnLineConstraint { .. }
            ));
        }
    }
}

#[test]
fn expression_transform_kind_follows_payload_value_class() {
    let rolling = GspFile::parse(include_bytes!(
        "../../../tests/Samples/热研系列/滚动系列/正Ｎ边形真滚1.gsp"
    ))
    .expect("rolling fixture parses");
    let rolling_scene = build_scene_checked(&rolling).expect("rolling scene builds");
    let rolling_point = rolling_scene
        .points
        .iter()
        .find(|point| {
            point
                .debug
                .as_ref()
                .is_some_and(|debug| debug.group_ordinal == 9)
        })
        .expect("rolling point #9");
    assert!(matches!(
        rolling_point.binding,
        Some(ScenePointBinding::Rotate { .. })
    ));

    let solid = GspFile::parse(include_bytes!(
        "../../../tests/Samples/个人专栏/李章博作品/动画演示立体几何轨迹形成（李章博）.gsp"
    ))
    .expect("solid-geometry fixture parses");
    let solid_scene = build_scene_checked(&solid).expect("solid-geometry scene builds");
    for ordinal in [107, 113] {
        let point = solid_scene
            .points
            .iter()
            .find(|point| {
                point
                    .debug
                    .as_ref()
                    .is_some_and(|debug| debug.group_ordinal == ordinal)
            })
            .unwrap_or_else(|| panic!("solid-geometry point #{ordinal}"));
        assert!(matches!(
            point.binding,
            Some(ScenePointBinding::Scale { .. })
        ));
    }

    let pythagorean = GspFile::parse(include_bytes!(
        "../../../tests/Samples/个人专栏/孟令岩作品/勾股定理小题.gsp"
    ))
    .expect("pythagorean fixture parses");
    let page = &pythagorean.page_files()[1];
    let scene = build_scene_checked(page).expect("pythagorean page builds");
    for ordinal in [60, 68] {
        let point = scene
            .points
            .iter()
            .find(|point| {
                point
                    .debug
                    .as_ref()
                    .is_some_and(|debug| debug.group_ordinal == ordinal)
            })
            .unwrap_or_else(|| panic!("pythagorean rotation point #{ordinal}"));
        assert!(matches!(
            point.binding,
            Some(ScenePointBinding::Rotate { .. })
        ));
    }
}

#[test]
fn refraction_sample_uses_raw_translation_offsets_and_live_iteration_depth() {
    let Some(data) = fixture_bytes("tests/Samples/个人专栏/侯仰顺作品/光的折射(蚂蚁制作).gsp")
    else {
        return;
    };
    let scene = fixture_scene(&data);

    assert_eq!(scene.background_color, Some([253, 224, 181, 255]));
    assert!(
        scene.lines.iter().all(|line| {
            !(line.debug.is_none() && line.color == [30, 30, 30, 255] && line.points.len() == 7)
        }),
        "the refraction payload does not define a synthetic hexagon"
    );
    let title = scene
        .labels
        .iter()
        .find(|label| {
            label
                .debug
                .as_ref()
                .is_some_and(|debug| debug.group_ordinal == 126)
        })
        .expect("expected rich-text title #126");
    assert_eq!(title.color, [0, 0, 255, 255]);
    assert_eq!(title.font_size, Some(24.0));
    assert_eq!(title.font_family.as_deref(), Some("Times New Roman"));
    assert!(title.screen_space);
    assert!(
        title
            .rich_markup
            .as_deref()
            .is_some_and(|markup| markup.contains("SP2#30R1G81L1"))
    );
    let refractive_index_label = scene
        .labels
        .iter()
        .find(|label| {
            label
                .debug
                .as_ref()
                .is_some_and(|debug| debug.group_ordinal == 18)
        })
        .expect("expected refractive-index parameter label #18");
    assert!(!refractive_index_label.visible);
    assert_eq!(refractive_index_label.text, "折射率n = 1.64");
    assert!(matches!(
        refractive_index_label.binding,
        Some(TextLabelBinding::LineProjectionParameter {
            line_kind: LineLikeKind::Ray,
            ..
        })
    ));
    let value_table = scene
        .iteration_tables
        .iter()
        .find(|table| {
            table
                .debug
                .as_ref()
                .is_some_and(|debug| debug.group_ordinal == 83)
        })
        .expect("expected payload value table #83");
    assert!(!value_table.show_index && value_table.anchor_at_top && value_table.depth == 0);
    assert_eq!((value_table.anchor.x, value_table.anchor.y), (247.0, 178.0));
    assert_eq!(
        value_table
            .columns
            .iter()
            .map(|column| column.expr_label.as_str())
            .collect::<Vec<_>>(),
        vec!["入射角θ₁", "折射角θ₂", "sinθ₁/sinθ₂"]
    );
    assert!(matches!(
        value_table.columns[0].value_binding,
        Some(IterationTableValueBinding::AngleMarker { .. })
    ));
    assert!(matches!(
        value_table.columns[1].value_binding,
        Some(IterationTableValueBinding::AngleMarker { .. })
    ));
    assert!(value_table.columns[2].value_binding.is_none());

    let offset = |ordinal| {
        let point = scene
            .points
            .iter()
            .find(|point| {
                point
                    .debug
                    .as_ref()
                    .is_some_and(|debug| debug.group_ordinal == ordinal)
            })
            .unwrap_or_else(|| panic!("expected translated point #{ordinal}"));
        match point.constraint {
            ScenePointConstraint::Offset { dx, dy, .. } => (dx, dy),
            ref constraint => {
                panic!("expected offset constraint for #{ordinal}, got {constraint:?}")
            }
        }
    };

    let (dx, dy) = offset(2);
    assert!((dx - 453.54330708661416).abs() < 1e-9 && dy.abs() < 1e-9);
    let (dx, dy) = offset(3);
    assert!(dx.abs() < 1e-9 && (dy - 340.1574803149606).abs() < 1e-9);
    let (dx, dy) = offset(21);
    assert!((dx - 7.637795448303222).abs() < 1e-9 && dy.abs() < 1e-9);
    let (dx, dy) = offset(120);
    assert!((dx + 7.559055118110236).abs() < 1e-9);
    assert!((dy - 18.89763779527559).abs() < 1e-9);

    let medium_polygon = scene
        .polygons
        .iter()
        .find(|polygon| {
            polygon
                .debug
                .as_ref()
                .is_some_and(|debug| debug.group_ordinal == 12)
        })
        .expect("expected medium polygon #12");
    assert!(matches!(
        medium_polygon.color_binding,
        Some(ColorBinding::Spectrum {
            point_index: 8,
            base_value,
            period,
            base_color: [0, 128, 0, 99],
        }) if (base_value - 1.640416666666667).abs() < 1e-9
            && (period - 1.0).abs() < 1e-9
    ));

    let refracted_intersection = scene
        .points
        .iter()
        .find(|point| {
            point
                .debug
                .as_ref()
                .is_some_and(|debug| debug.group_ordinal == 68)
        })
        .expect("expected refracted-ray intersection #68");
    assert!(matches!(
        refracted_intersection.constraint,
        ScenePointConstraint::LineIntersection {
            left: LineConstraint::Ray { .. },
            right: LineConstraint::ParallelLine { .. },
        }
    ));

    assert!(scene.parameters.iter().any(|parameter| {
        parameter.name == "光线条数" && parameter.visible && (parameter.value - 8.0).abs() < 1e-9
    }));
    assert_eq!(scene.line_iterations.len(), 3);
    assert!(scene.line_iterations.iter().all(|family| matches!(
        family,
        crate::runtime::scene::LineIterationFamily::Translate {
            depth: 7,
            depth_expr: Some(_),
            vector_start_index: Some(6),
            vector_end_index: Some(12),
            dx,
            dy,
            ..
        } if (*dx - 21.0).abs() < 1e-9 && dy.abs() < 1e-9
    )));
    assert_eq!(scene.polygon_iterations.len(), 3);
    assert_eq!(scene.polygons.len(), 32);
    assert_eq!(
        scene
            .polygon_iterations
            .iter()
            .map(|family| match family {
                crate::runtime::scene::PolygonIterationFamily::Translate { color, .. } => *color,
                family => panic!("expected translated arrow iteration, got {family:?}"),
            })
            .collect::<Vec<_>>(),
        vec![[255, 0, 0, 255], [255, 0, 255, 255], [0, 0, 255, 255]]
    );
    assert!(scene.polygon_iterations.iter().all(|family| matches!(
        family,
        crate::runtime::scene::PolygonIterationFamily::Translate {
            depth: 7,
            depth_expr: Some(_),
            vector_start_index: Some(6),
            vector_end_index: Some(12),
            secondary_dx: None,
            secondary_dy: None,
            dx,
            dy,
            ..
        } if (*dx - 21.0).abs() < 1e-9 && dy.abs() < 1e-9
    )));
    assert!(scene.buttons.iter().any(|button| {
        button.text == "隐藏反射光线"
            && matches!(
                &button.action,
                ButtonAction::ShowHideVisibility {
                    line_iteration_indices,
                    polygon_iteration_indices,
                    ..
                } if line_iteration_indices.len() == 1 && polygon_iteration_indices.len() == 1
            )
    }));
}

#[test]
fn moving_equilateral_triangle_preserves_payload_parameter_chain() {
    let Some(data) = fixture_bytes(
        "tests/Samples/个人专栏/侯仰顺作品/参数的应用-正三角形在正方形内滑动【蚂蚁制作】.gsp",
    ) else {
        return;
    };
    let file = GspFile::parse(&data).expect("fixture parses");
    let groups = file.object_groups();
    let point_map = collect_point_objects(&file, &groups);
    let analysis = analyze_scene(&file, &groups, &point_map);
    crate::runtime::functions::try_decode_parameter_control_expr(&file, &groups, &groups[11])
        .expect("parameter control expression #12 decodes");
    let controlled =
        try_decode_parameter_controlled_point(&file, &groups, &groups[12], &analysis.raw_anchors)
            .expect("payload point #13 should decode from expression #12 on polygon #9");
    assert!(matches!(
        controlled.constraint,
        super::points::RawPointConstraint::PolygonBoundary { .. }
    ));
    assert!(controlled.source_expr_absolute_parameter);

    let scene = build_scene_checked(&file).expect("scene builds");
    assert_eq!(scene.background_color, Some([255, 255, 255, 255]));
    let point_for_group = |ordinal| {
        scene
            .points
            .iter()
            .find(|point| {
                point
                    .debug
                    .as_ref()
                    .is_some_and(|debug| debug.group_ordinal == ordinal)
            })
            .unwrap_or_else(|| panic!("expected point #{ordinal}"))
    };
    let e = point_for_group(10);
    assert!(e.visible && e.draggable);
    let f = point_for_group(13);
    assert!(f.visible);
    assert!(matches!(
        f.constraint,
        ScenePointConstraint::OnPolygonBoundary { .. }
    ));
    assert!(matches!(
        f.binding,
        Some(ScenePointBinding::ConstraintParameterFromPointExpr { .. })
    ));
    let g = point_for_group(15);
    assert!(g.visible);
    assert!(matches!(g.binding, Some(ScenePointBinding::Rotate { .. })));
    let side_length = |left: &crate::format::PointRecord, right: &crate::format::PointRecord| {
        (left.x - right.x).hypot(left.y - right.y)
    };
    let square_side = side_length(&point_for_group(1).position, &point_for_group(2).position);
    for triangle_side in [
        side_length(&e.position, &f.position),
        side_length(&f.position, &g.position),
        side_length(&g.position, &e.position),
    ] {
        assert!((triangle_side - square_side).abs() < 1e-6);
    }
    for ordinal in [14, 16, 17] {
        assert!(scene.lines.iter().any(|line| {
            line.debug
                .as_ref()
                .is_some_and(|debug| debug.group_ordinal == ordinal)
                && line.binding.is_some()
        }));
    }
    assert!(scene.lines.iter().any(|line| {
        line.debug
            .as_ref()
            .is_some_and(|debug| debug.group_ordinal == 18)
            && matches!(line.binding, Some(LineBinding::PointTrace { .. }))
    }));
}

#[test]
fn builds_unnamed1_fixture_with_live_angle_rotation_points() {
    let data = include_bytes!("../../../tests/fixtures/未实现的系统功能/未命名1.gsp");
    let scene = fixture_scene(data);
    let log = fixture_log(data, "tests/fixtures/未实现的系统功能/未命名1.gsp");

    assert!(log.contains("问题数量: 0"));
    assert!(
        !log.contains("对象类型 28 还没有实现"),
        "expected angle-defined rotation helpers to stop being reported"
    );
    assert!(
        log.contains("将 点 #13 围绕 #10 按 #1、#6、#8 所成角旋转得到的点"),
        "expected the payload log to describe the recovered type-28 helper"
    );
    assert!(
        scene
            .points
            .iter()
            .filter(|point| {
                matches!(
                    point.binding,
                    Some(ScenePointBinding::Rotate {
                        angle_start_index: Some(_),
                        angle_vertex_index: Some(_),
                        angle_end_index: Some(_),
                        ..
                    })
                )
            })
            .count()
            >= 3,
        "expected the three type-28 helper points to export as live angle-rotation bindings"
    );
}

#[test]
fn triangle_angle_sum_fixture_keeps_measured_angle_rotation_live() {
    let Some(data) = fixture_bytes("tests/Samples/未分类档/三角形内角和定理.gsp")
    else {
        return;
    };
    let scene = fixture_scene(&data);
    let log = fixture_log(&data, "tests/Samples/未分类档/三角形内角和定理.gsp");

    assert!(log.contains("问题数量: 0"));
    assert!(
        log.contains("#31 = 参数旋转对象，按载荷顺序引用 #27、#21、#30"),
        "expected the payload log to keep the measured-angle parameter rotation"
    );

    let point_index_for_group = |ordinal| {
        scene
            .points
            .iter()
            .position(|point| {
                point
                    .debug
                    .as_ref()
                    .is_some_and(|debug| debug.group_ordinal == ordinal)
            })
            .expect("expected point for payload group")
    };
    let label_for_group = |ordinal| {
        scene
            .labels
            .iter()
            .find(|label| {
                label
                    .debug
                    .as_ref()
                    .is_some_and(|debug| debug.group_ordinal == ordinal)
            })
            .expect("expected label for payload group")
    };
    let arc_point_index = point_index_for_group(27);
    let center_point_index = point_index_for_group(21);
    let rotated_point_index = point_index_for_group(31);
    let intersection_index = point_index_for_group(46);
    let h_index = point_index_for_group(11);
    let segment01 = (point_index_for_group(1), point_index_for_group(6));
    let segment12 = (point_index_for_group(6), point_index_for_group(7));
    let segment23 = (point_index_for_group(7), point_index_for_group(4));

    assert!(
        matches!(
            scene.points[rotated_point_index].binding,
            Some(ScenePointBinding::Rotate {
                angle_start_index: Some(_),
                angle_vertex_index: Some(_),
                angle_end_index: Some(_),
                ..
            })
        ),
        "expected #31 to rotate from measured angle #30 instead of becoming static"
    );
    assert!(
        matches!(
            scene.points[intersection_index].constraint,
            ScenePointConstraint::LineCircularIntersection {
                circle: CircularConstraint::SegmentRadiusCircle {
                    center_index: _,
                    line_start_index: _,
                    line_end_index: _,
                },
                ..
            }
        ),
        "expected #46 to stay tied to circle #44, whose radius comes from measured BC"
    );
    assert!(
        scene.lines.iter().any(|line| {
            line.debug
                .as_ref()
                .is_some_and(|debug| debug.group_ordinal == 38)
                && matches!(
                    line.binding,
                    Some(LineBinding::AngleMarker {
                        start_index,
                        vertex_index,
                        end_index,
                        ..
                    }) if start_index == arc_point_index
                        && vertex_index == center_point_index
                        && end_index == point_index_for_group(31)
                )
        }),
        "expected #38 to bind to the measured-angle-rotated point"
    );
    let rotated_bc_line_index = scene
        .lines
        .iter()
        .position(|line| {
            line.debug
                .as_ref()
                .is_some_and(|debug| debug.group_ordinal == 32)
        })
        .expect("expected #32 rotated pink BC segment");
    let source_bl_line_index = scene
        .lines
        .iter()
        .position(|line| {
            line.debug
                .as_ref()
                .is_some_and(|debug| debug.group_ordinal == 29)
        })
        .expect("expected #29 source segment");
    assert!(
        matches!(
            &scene.lines[rotated_bc_line_index].binding,
            Some(LineBinding::MatrixApply {
                source_index,
                matrices,
            }) if *source_index == source_bl_line_index
                && matches!(matrices.as_slice(), [GeometryTransformBinding::Rotate(binding)]
                    if binding.center_index == center_point_index
                        && binding.angle_start_index == Some(point_index_for_group(16))
                        && binding.angle_vertex_index == Some(point_index_for_group(15))
                        && binding.angle_end_index == Some(point_index_for_group(18)))
        ),
        "expected #32 to rotate #29 from measured angle #30"
    );
    for (label_ordinal, (start_index, end_index)) in
        [(12, segment12), (13, segment23), (14, segment01)]
    {
        assert!(
            matches!(
                &label_for_group(label_ordinal).binding,
                Some(TextLabelBinding::LineProjectionParameter {
                    point_index,
                    start_index: actual_start_index,
                    end_index: actual_end_index,
                    line_kind: LineLikeKind::Segment,
                    ..
                }) if (*point_index, *actual_start_index, *actual_end_index)
                    == (h_index, start_index, end_index)
            ),
            "expected #{label_ordinal} to project H onto its referenced segment"
        );
    }
    for (point_ordinal, (start_index, end_index)) in
        [(21, segment23), (27, segment12), (28, segment01)]
    {
        let point_index = point_index_for_group(point_ordinal);
        assert!(
            matches!(
                &scene.points[point_index].binding,
                Some(ScenePointBinding::DerivedParameter {
                    source_index,
                    parameter_start_index: Some(actual_start_index),
                    parameter_end_index: Some(actual_end_index),
                }) if (*source_index, *actual_start_index, *actual_end_index)
                    == (h_index, start_index, end_index)
            ),
            "expected #{point_ordinal} to follow H projected onto its referenced segment"
        );
    }
    assert!(
        scene.circles.iter().any(|circle| {
            circle
                .debug
                .as_ref()
                .is_some_and(|debug| debug.group_ordinal == 44)
                && matches!(
                    circle.binding,
                    Some(ShapeBinding::SegmentRadiusCircle { .. })
                )
        }),
        "expected #44 to export as a live segment-radius circle"
    );
}

#[test]
fn three_circle_rolling_fixture_keeps_animate_buttons_and_measured_rotations_live() {
    let Some(data) = fixture_bytes("tests/Samples/热研系列/滚动系列/三圆滚动.gsp")
    else {
        return;
    };
    let scene = fixture_scene(&data);
    let log = fixture_log(&data, "tests/Samples/热研系列/滚动系列/三圆滚动.gsp");

    assert!(log.contains("问题数量: 0"));
    assert!(
        log.contains("#17 = 按钮，关联 #16，动作类型是 (2, 0)，名称“内圆”")
            && log.contains("#29 = 按钮，关联 #28，动作类型是 (2, 0)，名称“外圆”"),
        "expected the payload log to keep both JavaSketchpad animate buttons"
    );

    let point_index_for_group = |ordinal| {
        scene
            .points
            .iter()
            .position(|point| {
                point
                    .debug
                    .as_ref()
                    .is_some_and(|debug| debug.group_ordinal == ordinal)
            })
            .expect("expected point for payload group")
    };
    let line_for_group = |ordinal| {
        scene
            .lines
            .iter()
            .find(|line| {
                line.debug
                    .as_ref()
                    .is_some_and(|debug| debug.group_ordinal == ordinal)
            })
            .expect("expected line for payload group")
    };

    let inner_driver = point_index_for_group(16);
    let outer_driver = point_index_for_group(28);
    let inner_button = scene
        .buttons
        .iter()
        .find(|button| button.text == "内圆")
        .expect("expected inner rolling animate button");
    let outer_button = scene
        .buttons
        .iter()
        .find(|button| button.text == "外圆")
        .expect("expected outer rolling animate button");
    assert!(matches!(
        inner_button.action,
        ButtonAction::AnimatePoint { point_index, .. } if point_index == inner_driver
    ));
    assert!(matches!(
        outer_button.action,
        ButtonAction::AnimatePoint { point_index, .. } if point_index == outer_driver
    ));

    for ordinal in [21, 25, 33, 37] {
        let point = &scene.points[point_index_for_group(ordinal)];
        assert!(
            matches!(
                point.binding,
                Some(ScenePointBinding::Rotate {
                    angle_expr: Some(_),
                    ..
                })
            ),
            "expected #{ordinal} to keep its calculated measured-rotation binding"
        );
    }

    for ordinal in [24, 36] {
        let point = &scene.points[point_index_for_group(ordinal)];
        assert!(
            matches!(
                point.constraint,
                ScenePointConstraint::LineCircularIntersection {
                    circle: CircularConstraint::SegmentRadiusCircle { .. },
                    ..
                }
            ),
            "expected #{ordinal} to stay linked to the measured rolling-circle intersection"
        );
    }

    let main_center = &scene.points[point_index_for_group(6)];
    let inner_center = &scene.points[point_index_for_group(24)];
    let outer_center = &scene.points[point_index_for_group(36)];
    assert!(
        inner_center.position.x < main_center.position.x
            && inner_center.position.y < main_center.position.y,
        "expected the inner rolling circle to start above-left of the main circle, got inner=({}, {}) main=({}, {})",
        inner_center.position.x,
        inner_center.position.y,
        main_center.position.x,
        main_center.position.y
    );
    assert!(
        outer_center.position.x < main_center.position.x
            && outer_center.position.y > main_center.position.y,
        "expected the outer rolling circle to start below-left of the main circle, got outer=({}, {}) main=({}, {})",
        outer_center.position.x,
        outer_center.position.y,
        main_center.position.x,
        main_center.position.y
    );

    let inner_spoke = line_for_group(26);
    assert!(inner_spoke.visible);
    assert!(matches!(
        inner_spoke.binding,
        Some(LineBinding::Segment {
            start_index,
            end_index,
        }) if start_index == point_index_for_group(24) && end_index == point_index_for_group(25)
    ));

    let outer_spoke = line_for_group(38);
    assert!(outer_spoke.visible);
    assert!(matches!(
        outer_spoke.binding,
        Some(LineBinding::Segment {
            start_index,
            end_index,
        }) if start_index == point_index_for_group(36) && end_index == point_index_for_group(37)
    ));
}

#[test]
fn preserves_points_defined_by_path_value_fixture() {
    let scene = fixture_scene(include_bytes!(
        "../../../tests/fixtures/未实现的系统功能/给定的数值在路径上绘制点.gsp"
    ));

    assert_eq!(
        scene.points.len(),
        6,
        "expected A/B/D/E plus constrained C/F"
    );
    assert!(
        scene
            .points
            .iter()
            .any(|point| matches!(point.constraint, ScenePointConstraint::OnCircle { .. })),
        "expected one point constrained by the circle path payload"
    );
    assert!(
        scene
            .points
            .iter()
            .any(|point| matches!(point.constraint, ScenePointConstraint::OnSegment { .. })),
        "expected one point constrained by the segment path payload"
    );
    let labels = scene
        .labels
        .iter()
        .map(|label| label.text.as_str())
        .collect::<Vec<_>>();
    assert!(
        labels.contains(&"C"),
        "expected path-defined point label C, got {labels:?}"
    );
    assert!(
        labels.contains(&"F"),
        "expected path-defined point label F, got {labels:?}"
    );
}

#[test]
fn preserves_translated_points_in_point_translation_gsp() {
    let scene = fixture_scene(include_bytes!(
        "../../../tests/fixtures/gsp/static/point_translation.gsp"
    ));

    assert_eq!(
        scene.points.len(),
        2,
        "expected base point and translated point"
    );
    let origin = scene
        .points
        .iter()
        .find(|point| matches!(point.constraint, ScenePointConstraint::Free))
        .expect("expected free origin point");
    let translated = scene
        .points
        .iter()
        .find(|point| matches!(point.constraint, ScenePointConstraint::Offset { .. }))
        .expect("expected translated offset point");

    match translated.constraint {
        ScenePointConstraint::Offset {
            origin_index,
            dx,
            dy,
        } => {
            assert_eq!(origin_index, 0);
            assert!(
                dx.abs() < 0.001,
                "expected 90-degree translation to keep x constant, got dx={dx}"
            );
            assert!(
                dy < 0.0,
                "expected upward translation in raw coordinates, got dy={dy}"
            );
            assert!(
                (translated.position.x - (origin.position.x + dx)).abs() < 0.001
                    && (translated.position.y - (origin.position.y + dy)).abs() < 0.001,
                "expected translated point to preserve offset from origin: origin={:?}, translated={:?}",
                origin.position,
                translated.position
            );
        }
        _ => panic!("expected offset constraint"),
    }
}

#[test]
fn preserves_point_hidden_gsp() {
    let scene = fixture_scene(include_bytes!(
        "../../../tests/fixtures/gsp/static/point_hidden.gsp"
    ));

    assert_eq!(scene.points.len(), 1, "expected one point in the fixture");
    assert!(
        !scene.points[0].visible,
        "expected fixture point to inherit hidden state from source metadata"
    );
    assert!(scene.lines.is_empty());
    assert!(scene.labels.is_empty());
}

#[test]
fn preserves_parameter_controlled_point_on_segment_gsp() {
    let scene = fixture_scene(include_bytes!(
        "../../../tests/fixtures/gsp/static/point_on_segment.gsp"
    ));

    assert_eq!(scene.lines.len(), 1, "expected one segment");
    assert_eq!(
        scene.points.len(),
        4,
        "expected endpoints, the legacy slider source point, and the controlled point"
    );
    assert_eq!(scene.parameters.len(), 1, "expected t parameter");
    assert_eq!(scene.parameters[0].name, "t₁");
    assert!((scene.parameters[0].value - 0.7).abs() < 0.001);
    assert!(
        scene.points.iter().any(|point| {
            point.visible
                && matches!(
                    point.binding,
                    Some(ScenePointBinding::Parameter { ref name }) if name == "t₁"
                )
        }),
        "expected the payload slider source point to remain visible"
    );
    assert!(scene.points.iter().any(|point| {
        matches!(
            point.constraint,
            ScenePointConstraint::OnSegment { t, .. } if (t - 0.7).abs() < 0.001
        )
    }));

    let controlled_index = scene
        .points
        .iter()
        .position(|point| {
            matches!(point.binding, Some(ScenePointBinding::Parameter { ref name }) if name == "t₁")
                && matches!(point.constraint, ScenePointConstraint::OnSegment { .. })
        })
        .expect("expected the parameter-controlled point on the segment");
    let controlled_node = scene
        .object_graph
        .nodes
        .iter()
        .find(|node| node.id == format!("point:{controlled_index}"))
        .expect("controlled point belongs to the object graph");
    assert!(matches!(
        &controlled_node.definition,
        ObjectDefinition::Derived {
            op: ObjectOp::PointOnLine,
            parents,
        } if parents.iter().any(|parent| parent == "parameter:t₁")
    ));
}

#[test]
fn preserves_parameter_controlled_point_on_poly_gsp() {
    let scene = fixture_scene(include_bytes!(
        "../../../tests/fixtures/gsp/static/point_on_poly.gsp"
    ));

    assert_eq!(scene.polygons.len(), 1, "expected one polygon");
    assert_eq!(
        scene.points.len(),
        5,
        "expected polygon vertices, the legacy slider source point, and the controlled point"
    );
    assert_eq!(scene.parameters.len(), 1, "expected t parameter");
    assert_eq!(scene.parameters[0].name, "t₁");
    assert!(
        scene.points.iter().any(|point| {
            point.visible
                && matches!(
                    point.binding,
                    Some(ScenePointBinding::Parameter { ref name }) if name == "t₁"
                )
        }),
        "expected the payload slider source point to remain visible"
    );
    assert!(scene.points.iter().any(|point| matches!(
        point.constraint,
        ScenePointConstraint::OnPolygonBoundary { .. }
    )));
}

#[test]
fn preserves_parameter_controlled_point_on_circle_gsp() {
    let scene = fixture_scene(include_bytes!(
        "../../../tests/fixtures/gsp/static/point_on_circle.gsp"
    ));

    assert!(
        scene.circles.iter().any(|circle| matches!(
            circle.binding,
            Some(crate::runtime::scene::ShapeBinding::PointRadiusCircle { .. })
        )),
        "expected the payload circle to remain exported"
    );
    assert_eq!(
        scene.points.len(),
        4,
        "expected circle points, the legacy slider source point, and the controlled point"
    );
    assert_eq!(scene.parameters.len(), 1, "expected t parameter");
    assert_eq!(scene.parameters[0].name, "t₁");
    assert!(
        scene.points.iter().any(|point| {
            point.visible
                && matches!(
                    point.binding,
                    Some(ScenePointBinding::Parameter { ref name }) if name == "t₁"
                )
        }),
        "expected the payload slider source point to remain visible"
    );
    assert!(
        scene
            .points
            .iter()
            .any(|point| matches!(point.constraint, ScenePointConstraint::OnCircle { .. }))
    );
}

#[test]
fn preserves_coordinate_point_in_cood_gsp() {
    let scene = fixture_scene(include_bytes!(
        "../../../tests/fixtures/gsp/static/cood.gsp"
    ));

    assert!(scene.graph_mode, "expected graph scene");
    assert_eq!(scene.parameters.len(), 1, "expected t parameter");
    assert_eq!(scene.parameters[0].name, "t₁");
    assert!((scene.parameters[0].value - 0.01).abs() < 0.001);
    assert!(
        scene.points.iter().any(|point| {
            point.binding.as_ref().is_some_and(|binding| {
                matches!(
                    binding,
                    ScenePointBinding::CoordinateSource2d {
                        x_name,
                        y_name,
                        x_scalar_group_ordinal: Some(1),
                        y_scalar_group_ordinal: Some(2),
                        ..
                    } if x_name == "t₁" && y_name == "t₁ + 1"
                )
            })
        }),
        "expected the coordinate point to retain both payload scalar parents"
    );
    assert!(scene.points.iter().any(|point| {
        (point.position.x - 0.01).abs() < 0.001 && (point.position.y - 1.01).abs() < 0.001
    }));
}

#[test]
fn preserves_linear_intersection_points_in_insection_fixtures() {
    for (name, data, expected_right_kind) in [
        (
            "segment",
            include_bytes!("../../../tests/fixtures/gsp/insection/segment_insection.gsp")
                .as_slice(),
            crate::runtime::scene::LineLikeKind::Segment,
        ),
        (
            "line",
            include_bytes!("../../../tests/fixtures/gsp/insection/line_insection.gsp").as_slice(),
            crate::runtime::scene::LineLikeKind::Line,
        ),
        (
            "ray",
            include_bytes!("../../../tests/fixtures/gsp/insection/ray_insection.gsp").as_slice(),
            crate::runtime::scene::LineLikeKind::Ray,
        ),
    ] {
        let scene = fixture_scene(data);

        assert_eq!(
            scene.points.len(),
            5,
            "expected derived intersection point for {name}"
        );
        assert!(scene.points.iter().any(|point| match expected_right_kind {
            crate::runtime::scene::LineLikeKind::Segment => matches!(
                point.constraint,
                ScenePointConstraint::LineIntersection {
                    left: LineConstraint::Segment { .. },
                    right: LineConstraint::Segment { .. },
                }
            ),
            crate::runtime::scene::LineLikeKind::Line => matches!(
                point.constraint,
                ScenePointConstraint::LineIntersection {
                    left: LineConstraint::Segment { .. },
                    right: LineConstraint::Line { .. },
                }
            ),
            crate::runtime::scene::LineLikeKind::Ray => matches!(
                point.constraint,
                ScenePointConstraint::LineIntersection {
                    left: LineConstraint::Segment { .. },
                    right: LineConstraint::Ray { .. },
                }
            ),
        }));
        assert!(
            scene.points.iter().any(|point| {
                (point.position.x - 416.3160761196899).abs() < 1e-6
                    && (point.position.y - 345.2222079835971).abs() < 1e-6
            }),
            "expected derived intersection coordinates for {name}"
        );
    }
}

#[test]
fn preserves_circle_circle_intersection_points() {
    let scene = fixture_scene(include_bytes!(
        "../../../tests/fixtures/gsp/insection/circle_circle_insection.gsp"
    ));

    assert_eq!(
        scene.points.len(),
        6,
        "expected both circle-circle intersections"
    );
    assert!(scene.points.iter().any(|point| {
        matches!(
            point.constraint,
            ScenePointConstraint::CircleCircleIntersection { .. }
        )
    }));
    assert!(scene.points.iter().any(|point| {
        (point.position.x - 421.3993346591643).abs() < 1e-6
            && (point.position.y - 213.66291724683578).abs() < 1e-6
    }));
    assert!(scene.points.iter().any(|point| {
        (point.position.x - 445.71654184257966).abs() < 1e-6
            && (point.position.y - 494.02601183209464).abs() < 1e-6
    }));
}

#[test]
fn preserves_line_circle_intersection_points() {
    let scene = fixture_scene(include_bytes!(
        "../../../tests/fixtures/gsp/insection/circle_insection.gsp"
    ));

    assert_eq!(
        scene.points.len(),
        5,
        "expected derived line-circle intersection"
    );
    assert!(scene.points.iter().any(|point| {
        matches!(
            point.constraint,
            ScenePointConstraint::LineCircleIntersection {
                line: LineConstraint::Segment { .. },
                ..
            }
        )
    }));
    assert!(scene.points.iter().any(|point| {
        (point.position.x - 566.0581863195608).abs() < 1e-6
            && (point.position.y - 417.2769704284295).abs() < 1e-6
    }));
}

#[test]
fn preserves_perpendicular_intersection_points_in_perp_fixture() {
    let scene = fixture_scene(include_bytes!("../../../tests/fixtures/gsp/perp.gsp"));

    let intersection = scene
        .points
        .iter()
        .find(|point| {
            matches!(
                point.constraint,
                ScenePointConstraint::LineIntersection {
                    left: LineConstraint::Segment { .. } | LineConstraint::Line { .. },
                    right: LineConstraint::PerpendicularLine {
                        through_index: 2,
                        ..
                    },
                }
            )
        })
        .expect("expected reactive intersection point bound to the perpendicular line");

    assert!(
        (intersection.position.x - 867.3347427619169).abs() < 1e-6
            && (intersection.position.y - 469.9559050197873).abs() < 1e-6,
        "expected foot-of-perpendicular coordinates, got {:?}",
        intersection.position
    );
}

#[test]
fn preserves_circle_y_intersection_points() {
    let scene = fixture_scene(include_bytes!(
        "../../../tests/fixtures/gsp/circle_y_intersection.gsp"
    ));

    assert!(scene.points.iter().any(|point| {
        point.visible
            && (point.position.x - 1.0).abs() < 1e-6
            && (point.position.y - 0.0).abs() < 1e-6
            && matches!(
                point.binding,
                Some(crate::runtime::scene::ScenePointBinding::GraphCalibration)
            )
    }));
    assert!(scene.labels.iter().any(|label| label.text == "G"));
    assert!(
        scene.points.iter().any(|point| {
            matches!(
                point.constraint,
                ScenePointConstraint::LineCircleIntersection { .. }
            ) && (point.position.x - 0.0).abs() < 1e-6
                && (point.position.y + 1.0).abs() < 1e-6
        }),
        "expected y-axis circle intersection point, got {:?}",
        scene
            .points
            .iter()
            .map(|point| (&point.position.x, &point.position.y, &point.constraint))
            .collect::<Vec<_>>()
    );
}

#[test]
fn preserves_three_point_arc_intersection_points() {
    let scene = fixture_scene(include_bytes!(
        "../../../tests/fixtures/gsp/static/three_point_arc_intersection.gsp"
    ));

    assert_eq!(
        scene.points.len(),
        7,
        "expected original arc control points plus one derived intersection"
    );
    assert!(
        scene.points.iter().any(|point| {
            matches!(
                point.constraint,
                ScenePointConstraint::CircularIntersection { .. }
            ) && (point.position.x - 471.96614672487107).abs() < 1e-6
                && (point.position.y - 484.54842372244576).abs() < 1e-6
        }),
        "expected reactive arc intersection, got {:?}",
        scene
            .points
            .iter()
            .map(|point| (&point.position.x, &point.position.y, &point.constraint))
            .collect::<Vec<_>>()
    );
}

#[test]
fn preserves_cans_in_container_inrm_fixture_interactivity() {
    let Some(data) = fixture_bytes("tests/fixtures/未实现/(inRm)容器中的罐头.gsp") else {
        return;
    };
    let scene = fixture_scene(&data);

    assert_eq!(
        scene.lines.len(),
        13,
        "expected source guide lines to export"
    );
    assert_eq!(
        scene.circles.len(),
        38,
        "expected the payload can circles to export"
    );
    assert_eq!(
        scene.points.len(),
        40,
        "expected helper points to stay exported"
    );
    assert_eq!(
        scene
            .circles
            .iter()
            .filter(|circle| matches!(
                circle.binding,
                Some(crate::runtime::scene::ShapeBinding::SegmentRadiusCircle { .. })
            ))
            .count(),
        38,
        "expected every payload circle to keep its live segment-radius binding"
    );
    assert_eq!(
        scene.circles.iter().filter(|circle| circle.visible).count(),
        24,
        "expected the visible can circles to remain rendered"
    );
    assert_eq!(
        scene.points.iter().filter(|point| point.visible).count(),
        3,
        "expected the payload draggable points to stay visible"
    );
    assert_eq!(
        scene
            .points
            .iter()
            .filter(|point| matches!(point.constraint, ScenePointConstraint::OnSegment { .. }))
            .count(),
        2,
        "expected both payload slider points to remain segment constrained"
    );
    assert_eq!(
        scene
            .points
            .iter()
            .filter(|point| matches!(point.constraint, ScenePointConstraint::Offset { .. }))
            .count(),
        1,
        "expected the offset helper point to stay live"
    );
    assert_eq!(
        scene
            .points
            .iter()
            .filter(|point| matches!(point.binding, Some(ScenePointBinding::Scale { .. })))
            .count(),
        4,
        "expected scale-derived helper points to preserve their bindings"
    );
    assert_eq!(
        scene
            .points
            .iter()
            .filter(|point| matches!(point.binding, Some(ScenePointBinding::Rotate { .. })))
            .count(),
        1,
        "expected the rotated helper point to preserve its binding"
    );
    assert_eq!(
        scene
            .points
            .iter()
            .filter(|point| matches!(point.binding, Some(ScenePointBinding::Translate { .. })))
            .count(),
        5,
        "expected translated helper points to preserve their bindings"
    );
    assert!(
        scene
            .labels
            .iter()
            .any(|label| label.visible && label.text == "M"),
        "expected the payload midpoint label to stay visible"
    );
}

#[test]
fn exports_test10_marked_ratio_scale_and_reference_geometry() {
    let data = include_bytes!("../../../tests/fixtures/bug/测试10.gsp");
    let file = GspFile::parse(data).expect("fixture parses");
    let scene = build_scene_checked(&file).expect("scene builds");

    let point_by_ordinal = |ordinal| {
        scene
            .points
            .iter()
            .find(|point| {
                point
                    .debug
                    .as_ref()
                    .is_some_and(|debug| debug.group_ordinal == ordinal)
            })
            .expect("expected point ordinal")
    };

    let b = point_by_ordinal(6);
    match &b.binding {
        Some(ScenePointBinding::Scale {
            factor,
            factor_expr: Some(factor_expr),
            parameter_name,
            factor_parameter_point_index,
            factor_parameter_start_index,
            factor_parameter_end_index,
            source_index,
            center_index,
            ..
        }) => {
            assert_eq!((*source_index, *center_index), (2, 1));
            assert_eq!(parameter_name, &None);
            assert_eq!(factor_parameter_point_index, &None);
            assert_eq!(factor_parameter_start_index, &None);
            assert_eq!(factor_parameter_end_index, &None);
            assert!((factor - 6.0_f64.sqrt()).abs() < 1e-12);
            assert!(function_expr_has_unary(factor_expr, UnaryFunction::Sqrt));
        }
        other => panic!("expected marked-ratio scale binding for group #6, got {other:?}"),
    }

    assert!((b.position.x - 943.2872383623309).abs() < 1e-9);
    assert!((b.position.y - 309.37346662472527).abs() < 1e-9);

    let d0 = point_by_ordinal(18);
    assert!((d0.position.x - 538.0).abs() < 1e-9);
    assert!((d0.position.y - 428.0).abs() < 1e-9);

    let e = point_by_ordinal(20);
    assert!((e.position.x - 714.459085867259).abs() < 1e-9);
    assert!((e.position.y - 647.5843289845315).abs() < 1e-9);

    let e0 = point_by_ordinal(24);
    assert!((e0.position.x - 486.0).abs() < 1e-9);
    assert!((e0.position.y - 551.0).abs() < 1e-9);

    assert!(
        scene.points.iter().any(|point| matches!(
            point.binding,
            Some(ScenePointBinding::Scale {
                factor_expr: Some(_),
                source_index,
                center_index,
                ..
            }) if source_index == 2 && center_index == 1
        )),
        "expected live marked-ratio scale point for group #6"
    );

    assert!(
        scene.arcs.iter().filter(|arc| arc.visible).all(|arc| !arc
            .debug
            .as_ref()
            .is_some_and(|debug| matches!(debug.group_ordinal, 49 | 51))),
        "expected the style-c hidden three-point arcs omitted by the reference htm to stay non-rendering"
    );
    assert!(
        scene.labels.iter().any(|label| label.text == "⇒△CBD∼△CEB"),
        "expected rich-text symbols from the reference htm label"
    );
    assert!(
        scene
            .labels
            .iter()
            .any(|label| label.text == "⇒(BC^2)=CD*CE"),
        "expected rich-text operator symbols from the reference htm label"
    );
}

#[test]
fn exports_three_moving_point_fixture_parameter_rotation_and_buttons() {
    let scene = fixture_scene(include_bytes!(
        "../../../tests/fixtures/bug/三动点最小值_20260419_123930.gsp"
    ));

    let parameter = scene
        .parameters
        .iter()
        .find(|parameter| parameter.name == "t₁")
        .expect("expected t₁ angle parameter from htm");
    assert_eq!(parameter.unit.as_deref(), Some("degree"));
    assert!((parameter.value - 60.0).abs() < 1e-6);

    let rotated_c = scene
        .points
        .iter()
        .find(|point| {
            point
                .debug
                .as_ref()
                .is_some_and(|debug| debug.group_ordinal == 4)
        })
        .expect("expected htm Rotation/MeasuredAngle object #4 to export as point C");
    assert!((rotated_c.position.x - 673.4418668596801).abs() < 1e-6);
    assert!((rotated_c.position.y - 608.7880361833353).abs() < 1e-6);
    assert!(matches!(
        rotated_c.binding,
        Some(ScenePointBinding::Rotate {
            source_index: 1,
            center_index: 0,
            angle_degrees,
            parameter_name: Some(ref name),
            ..
        }) if (angle_degrees - 60.0).abs() < 1e-6 && name == "t₁"
    ));

    let mut move_button_ordinals = scene
        .buttons
        .iter()
        .filter(|button| matches!(button.action, ButtonAction::MovePoint { .. }))
        .filter_map(|button| button.debug.as_ref().map(|debug| debug.group_ordinal))
        .collect::<Vec<_>>();
    move_button_ordinals.sort_unstable();
    assert_eq!(move_button_ordinals, vec![39, 42, 43]);

    let measurement_labels = scene
        .labels
        .iter()
        .filter_map(|label| {
            label
                .debug
                .as_ref()
                .map(|debug| (debug.group_ordinal, label.text.as_str(), &label.binding))
        })
        .filter(|(ordinal, _, _)| matches!(ordinal, 15 | 31 | 34 | 35 | 36 | 44 | 45 | 46))
        .collect::<Vec<_>>();
    assert_eq!(
        measurement_labels
            .iter()
            .map(|(ordinal, _, _)| *ordinal)
            .collect::<Vec<_>>(),
        vec![15, 31, 34, 35, 36, 44, 45, 46],
        "expected all htm measurement readouts to export"
    );
    assert!(measurement_labels.iter().any(|(_, text, binding)| {
        text.starts_with("∠BAC = ")
            && matches!(binding, Some(TextLabelBinding::PointAngleValue { .. }))
    }));
    assert!(measurement_labels.iter().any(|(_, text, binding)| {
        text.starts_with("△ABC的面积 = ")
            && matches!(binding, Some(TextLabelBinding::PolygonAreaValue { .. }))
    }));
    assert!(measurement_labels.iter().any(|(_, text, binding)| {
        text.starts_with("MN = ")
            && matches!(binding, Some(TextLabelBinding::PointDistanceValue { .. }))
    }));
    assert!(measurement_labels.iter().any(|(_, text, binding)| {
        text.starts_with("AE / BC*3 = ")
            && matches!(binding, Some(TextLabelBinding::ExpressionValue { .. }))
    }));
}

#[test]
fn ellipse_polygon_rolling_keeps_marked_translation_and_intersection_chain() {
    let Some(data) = fixture_bytes("tests/Samples/热研系列/滚动系列/椭圆在正多边形上的滚动.gsp")
    else {
        return;
    };
    let file = GspFile::parse(&data).expect("ellipse rolling fixture parses");
    let scene = build_scene_checked(&file).expect("ellipse rolling fixture builds");
    let point = |ordinal| {
        scene
            .points
            .iter()
            .find(|point| {
                point
                    .debug
                    .as_ref()
                    .is_some_and(|debug| debug.group_ordinal == ordinal)
            })
            .unwrap_or_else(|| panic!("missing point group #{ordinal}"))
    };
    let line = |ordinal| {
        scene
            .lines
            .iter()
            .find(|line| {
                line.debug
                    .as_ref()
                    .is_some_and(|debug| debug.group_ordinal == ordinal)
            })
            .unwrap_or_else(|| panic!("missing line group #{ordinal}"))
    };

    match &point(69).binding {
        Some(ScenePointBinding::MarkedAngleTranslation {
            target_index,
            angle_start_index,
            angle_vertex_index,
            angle_end_index,
            distance,
            distance_parameter_group_ordinals,
            ..
        }) => {
            assert_eq!(
                (
                    *target_index,
                    *angle_start_index,
                    *angle_vertex_index,
                    *angle_end_index
                ),
                (0, 3, 0, 5)
            );
            assert!((distance - 47.01831435133081).abs() < 1e-9);
            assert_eq!(
                distance_parameter_group_ordinals,
                &std::collections::BTreeMap::from([("a".to_string(), 66), ("b".to_string(), 67)])
            );
        }
        other => panic!("expected marked-angle translation for group #69, got {other:?}"),
    }
    assert!(matches!(
        point(86).binding,
        Some(ScenePointBinding::DerivedParameter { .. })
    ));
    for ordinal in [105, 108] {
        assert!(matches!(
            point(ordinal).constraint,
            ScenePointConstraint::LineCircularIntersection { .. }
        ));
    }
    assert!(matches!(
        point(119).constraint,
        ScenePointConstraint::LineIntersection { .. }
    ));
    for ordinal in [85, 101, 110] {
        assert!(matches!(
            &line(ordinal).binding,
            Some(LineBinding::MatrixApply { matrices, .. })
                if matches!(matrices.as_slice(), [GeometryTransformBinding::Rotate(_)])
        ));
    }
    assert!(matches!(
        &line(102).binding,
        Some(LineBinding::MatrixApply { matrices, .. })
            if matches!(matrices.as_slice(), [GeometryTransformBinding::Reflect(_)])
    ));
    for ordinal in [87, 118] {
        assert!(matches!(
            line(ordinal).binding,
            Some(LineBinding::PerpendicularLine { .. })
        ));
    }
}

#[test]
fn neon_light_polygon_chain_builds_a_complete_object_graph() {
    let data = fixture_bytes("tests/Samples/个人专栏/方小庆作品/霓虹灯问题(inRm).gsp")
        .expect("neon-light fixture");
    let file = GspFile::parse(&data).expect("neon-light fixture parses");
    let scene = build_scene_checked(&file).expect("neon-light scene builds");
    let point_index = |group_ordinal| {
        scene
            .points
            .iter()
            .position(|point| {
                point
                    .debug
                    .as_ref()
                    .is_some_and(|debug| debug.group_ordinal == group_ordinal)
            })
            .unwrap_or_else(|| panic!("point group #{group_ordinal}"))
    };
    let point = |group_ordinal| &scene.points[point_index(group_ordinal)];

    for group_ordinal in [25, 26, 28, 33, 35] {
        assert!(matches!(
            point(group_ordinal).constraint,
            ScenePointConstraint::OnPolygonShapeBoundary { .. }
        ));
    }
    for (group_ordinal, angle_point_ordinal) in [(39, 33), (41, 35)] {
        let Some(ScenePointBinding::Rotate {
            source_index,
            center_index,
            angle_parameter_point_index,
            angle_parameter_start_index,
            angle_parameter_end_index,
            angle_parameter_scale,
            ..
        }) = &point(group_ordinal).binding
        else {
            panic!("point group #{group_ordinal} should retain its parameter rotation");
        };
        assert_eq!(*source_index, point_index(38));
        assert_eq!(*center_index, point_index(37));
        assert_eq!(
            *angle_parameter_point_index,
            Some(point_index(angle_point_ordinal))
        );
        assert_eq!(*angle_parameter_start_index, Some(point_index(25)));
        assert_eq!(*angle_parameter_end_index, Some(point_index(26)));
        assert_eq!(
            *angle_parameter_scale,
            Some(std::f64::consts::TAU.to_degrees())
        );
    }
    let polygon = scene
        .polygons
        .iter()
        .find(|polygon| {
            polygon
                .debug
                .as_ref()
                .is_some_and(|debug| debug.group_ordinal == 44)
        })
        .expect("neon polygon #44");
    assert!(matches!(
        &polygon.binding,
        Some(ShapeBinding::PointPolygon { vertex_indices })
            if *vertex_indices == [39, 41, 42, 43].map(point_index)
    ));
    assert!(
        scene.object_graph.geometry_complete,
        "pending: {:?}",
        scene.object_graph.pending_operations
    );
    assert!(scene.object_graph.pending_operations.is_empty());
}
