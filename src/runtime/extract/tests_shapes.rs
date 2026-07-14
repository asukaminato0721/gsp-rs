use super::test_support::{fixture_bytes, fixture_log, fixture_scene, function_expr_has_parameter};
use crate::runtime::scene::{
    ArcBinding, ButtonAction, LineBinding, LineConstraint, ScenePointBinding, ScenePointConstraint,
    ShapeBinding, TextLabelBinding,
};

#[test]
fn cylinder_family_center_arc_payload_parents_are_preserved() {
    let file = crate::format::GspFile::parse(include_bytes!(
        "../../../tests/Samples/个人专栏/孟令岩作品/※圆柱、圆锥、圆台的展开与形成20131012（孟令岩）.gsp"
    ))
    .expect("fixture parses");
    let page = &file.page_files()[3];
    let scene = super::build::build_scene_checked(page).expect("page builds");
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
            .unwrap_or_else(|| panic!("missing scene point for group #{ordinal}"))
    };
    let point = |ordinal| &scene.points[point_index(ordinal)];

    assert!(matches!(
        point(13).binding,
        Some(ScenePointBinding::Rotate {
            source_index,
            center_index,
            ref angle_expr,
            ref angle_parameter_group_ordinals,
            ..
        }) if source_index == point_index(1)
            && center_index == point_index(6)
            && angle_expr.is_some()
            && angle_parameter_group_ordinals.values().copied().collect::<Vec<_>>() == [10, 11]
    ));
    assert!(matches!(
        point(15).binding,
        Some(ScenePointBinding::Rotate {
            source_index,
            center_index,
            angle_expr: None,
            ..
        }) if source_index == point_index(13) && center_index == point_index(6)
    ));
    assert!(matches!(
        point(17).constraint,
        ScenePointConstraint::OnCircleArc {
            center_index,
            start_index,
            end_index,
            ..
        } if center_index == point_index(6)
            && start_index == point_index(14)
            && end_index == point_index(15)
    ));
    assert!(matches!(
        point(30).binding,
        Some(ScenePointBinding::Rotate {
            source_index,
            center_index,
            angle_start_index: Some(angle_start_index),
            angle_vertex_index: Some(angle_vertex_index),
            angle_end_index: Some(angle_end_index),
            ..
        }) if source_index == point_index(20)
            && center_index == point_index(6)
            && angle_start_index == point_index(1)
            && angle_vertex_index == point_index(6)
            && angle_end_index == point_index(17)
    ));

    for (ordinal, center, start, end) in [(16, 6, 14, 15), (18, 6, 1, 13), (38, 6, 20, 30)] {
        let arc = scene
            .arcs
            .iter()
            .find(|arc| {
                arc.debug
                    .as_ref()
                    .is_some_and(|debug| debug.group_ordinal == ordinal)
            })
            .unwrap_or_else(|| panic!("missing scene arc for group #{ordinal}"));
        assert!(matches!(
            arc.binding,
            Some(ArcBinding::CenterArc {
                center_index,
                start_index,
                end_index,
            }) if center_index == point_index(center)
                && start_index == point_index(start)
                && end_index == point_index(end)
        ));
    }
}

#[test]
fn cylinder_sector_angle_uses_circle_circumference_payload() {
    let file = crate::format::GspFile::parse(include_bytes!(
        "../../../tests/Samples/个人专栏/孟令岩作品/※圆柱、圆锥、圆台的展开与形成20131012（孟令岩）.gsp"
    ))
    .expect("fixture parses");
    let page = &file.page_files()[4];
    let groups = page.object_groups();
    assert_eq!(
        super::graph_object_circle_measurement_kind(page, &groups[43]),
        Some(super::GraphObjectCircleMeasurementKind::Circumference)
    );
    let angle_expr =
        crate::runtime::functions::try_decode_function_expr(page, &groups, &groups[44])
            .expect("sector angle expression");
    assert!(function_expr_has_parameter(&angle_expr, "11.47"));
    assert!(function_expr_has_parameter(&angle_expr, "h"));
    assert_eq!(
        crate::runtime::functions::function_parameter_group_ordinals(page, &groups, &groups[44],),
        std::collections::BTreeMap::from([("11.47".to_string(), 44), ("h".to_string(), 19)])
    );

    let scene = super::build::build_scene_checked(page).expect("page builds");
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
            .unwrap_or_else(|| panic!("missing scene point for group #{ordinal}"))
    };
    for ordinal in [46, 49, 55, 104, 106, 111] {
        point_index(ordinal);
    }
    assert!(matches!(
        scene.points[point_index(46)].binding,
        Some(ScenePointBinding::Rotate {
            source_index,
            center_index,
            ref angle_expr,
            ref angle_parameter_group_ordinals,
            ..
        }) if source_index == point_index(1)
            && center_index == point_index(39)
            && angle_expr.is_some()
            && angle_parameter_group_ordinals.get("11.47") == Some(&44)
            && angle_parameter_group_ordinals.get("h") == Some(&19)
    ));
    assert!(matches!(
        scene.points[point_index(49)].constraint,
        ScenePointConstraint::OnCircleArc {
            center_index,
            start_index,
            end_index,
            ..
        } if center_index == point_index(39)
            && start_index == point_index(1)
            && end_index == point_index(46)
    ));

    for (ordinal, center, start, end) in [
        (48, 39, 1, 46),
        (56, 49, 55, 53),
        (105, 39, 87, 104),
        (112, 106, 111, 109),
    ] {
        let arc = scene
            .arcs
            .iter()
            .find(|arc| {
                arc.debug
                    .as_ref()
                    .is_some_and(|debug| debug.group_ordinal == ordinal)
            })
            .unwrap_or_else(|| panic!("missing scene arc for group #{ordinal}"));
        assert!(matches!(
            arc.binding,
            Some(ArcBinding::CenterArc {
                center_index,
                start_index,
                end_index,
            }) if center_index == point_index(center)
                && start_index == point_index(start)
                && end_index == point_index(end)
        ));
    }
}

#[test]
fn derived_endpoint_accepts_the_exact_twenty_four_byte_payload() {
    let data = fixture_bytes("tests/Samples/个人专栏/孟令岩作品/整体面积三例（孟令岩）.gsp")
        .expect("fixture");
    let document = crate::format::GspFile::parse(&data).expect("fixture parses");
    let page = &document.page_files()[1];
    let groups = page.object_groups();
    let point_map = super::points::collect_point_objects(page, &groups);
    let analysis = super::analysis::analyze_scene(page, &groups, &point_map);
    let endpoint = &groups[170];
    let payload = endpoint
        .records
        .iter()
        .find(|record| record.record_type == crate::runtime::payload_consts::RECORD_BINDING_PAYLOAD)
        .expect("group #171 binding payload")
        .payload(&page.data);
    assert_eq!(payload.len(), 24);

    let binding = super::points::decode_derived_polar_endpoint_binding(
        page,
        &groups,
        endpoint,
        &analysis.raw_anchors,
    )
    .expect("the 24-byte derived endpoint payload decodes");
    assert_eq!(binding.center_group_index, 167);
    assert!((binding.distance_value - 0.3).abs() < 1e-12);
}

#[test]
fn measured_radius_circle_from_exam_sample_is_live() {
    let data = fixture_bytes("tests/Samples/个人专栏/侯仰顺作品/09年潍坊17题答案（蚂蚁制作）.gsp")
        .expect("exam fixture");
    let scene = fixture_scene(&data);

    let endpoint = scene
        .points
        .iter()
        .find(|point| {
            point
                .debug
                .as_ref()
                .is_some_and(|debug| debug.group_ordinal == 8)
        })
        .expect("measured endpoint #8 is materialized");
    assert!(matches!(
        endpoint.binding,
        Some(ScenePointBinding::PolarOffset {
            ref distance_parameter_group_ordinals,
            ..
        }) if distance_parameter_group_ordinals.values().copied().eq([6])
    ));

    let circle = scene
        .circles
        .iter()
        .find(|circle| {
            circle
                .debug
                .as_ref()
                .is_some_and(|debug| debug.group_ordinal == 13)
        })
        .expect("measured-radius circle #13");
    assert!(matches!(
        circle.binding,
        Some(ShapeBinding::SegmentRadiusCircle { .. })
    ));

    let intersection = scene
        .points
        .iter()
        .find(|point| {
            point
                .debug
                .as_ref()
                .is_some_and(|debug| debug.group_ordinal == 14)
        })
        .expect("line-circle intersection #14");
    assert!(matches!(
        intersection.constraint,
        ScenePointConstraint::LineCircularIntersection { variant: 0, .. }
    ));
}

#[test]
fn rolling_circle_decodes_grouped_rotation_and_boundary_expressions() {
    let data = fixture_bytes("tests/Samples/个人专栏/方小庆作品/圆的滚动全解(inRm).gsp")
        .expect("rolling-circle fixture");
    let file = crate::format::GspFile::parse(&data).expect("fixture parses");

    let sixth = &file.page_files()[5];
    let sixth_groups = sixth.object_groups();
    for ordinal in [56, 57] {
        crate::runtime::functions::try_decode_function_expr_with_inlined_refs(
            sixth,
            &sixth_groups,
            &sixth_groups[ordinal - 1],
        )
        .unwrap_or_else(|error| panic!("page 6 group #{ordinal} must decode: {error}"));
    }

    let eighth = &file.page_files()[7];
    let eighth_groups = eighth.object_groups();
    for ordinal in [28, 36, 49, 50, 53, 61, 145, 155] {
        crate::runtime::functions::try_decode_function_expr_with_inlined_refs(
            eighth,
            &eighth_groups,
            &eighth_groups[ordinal - 1],
        )
        .unwrap_or_else(|error| panic!("page 8 group #{ordinal} must decode: {error}"));
    }
}

#[test]
fn rolling_circle_point_trace_parameter_is_live() {
    let data = fixture_bytes("tests/Samples/个人专栏/方小庆作品/圆的滚动全解(inRm).gsp")
        .expect("rolling-circle fixture");
    let file = crate::format::GspFile::parse(&data).expect("fixture parses");
    let scene = super::build::build_scene_checked(&file.page_files()[3]).expect("page 4 scene");
    let point = scene
        .points
        .iter()
        .find(|point| {
            point
                .debug
                .as_ref()
                .is_some_and(|debug| debug.group_ordinal == 26)
        })
        .expect("group #26 point");
    assert!(matches!(
        point.constraint,
        ScenePointConstraint::OnPolyline {
            function_key: 18,
            ..
        }
    ));
}

#[test]
fn cylinder_net_exports_live_translation_parallel_trace_and_move_buttons() {
    let path = "tests/Samples/个人专栏/侯仰顺作品/圆柱侧面展开图(蚂蚁制作).gsp";
    let Some(data) = fixture_bytes(path) else {
        return;
    };
    let log = fixture_log(&data, path);
    assert!(log.contains("问题数量: 0"));
    assert!(log.contains("{7} Translation/FixedAngle/MarkedDistance(6,5,0)"));
    let scene = fixture_scene(&data);
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
            .expect("expected payload point")
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
            .expect("expected payload line")
    };
    assert!(matches!(
        &point_for_group(7).binding,
        Some(ScenePointBinding::PolarOffset { distance_expr, .. })
            if function_expr_has_parameter(distance_expr, "A底面大小")
    ));
    assert!(scene.circles.iter().any(|circle| {
        circle
            .debug
            .as_ref()
            .is_some_and(|debug| debug.group_ordinal == 17)
            && matches!(
                circle.binding,
                Some(ShapeBinding::ExpressionRadiusCircle { .. })
            )
    }));
    assert!(matches!(
        point_for_group(102).constraint,
        ScenePointConstraint::OnCircularConstraint { .. }
    ));
    assert!(matches!(
        point_for_group(24).constraint,
        ScenePointConstraint::OnLineConstraint { t, .. }
            if (t - 0.452_707).abs() < 1e-6
    ));
    assert!(matches!(
        point_for_group(111).constraint,
        ScenePointConstraint::OnPolyline {
            function_key: 108,
            ..
        }
    ));
    assert!(matches!(
        point_for_group(113).constraint,
        ScenePointConstraint::LineTraceIntersection { variant: 1, .. }
    ));
    assert!(scene.lines.iter().any(|line| {
        line.debug
            .as_ref()
            .is_some_and(|debug| debug.group_ordinal == 112)
            && matches!(line.binding, Some(LineBinding::ParallelLine { .. }))
    }));
    assert!(scene.lines.iter().any(|line| {
        line.debug
            .as_ref()
            .is_some_and(|debug| debug.group_ordinal == 108)
            && matches!(line.binding, Some(LineBinding::PointTrace { .. }))
    }));
    for (ordinal, color) in [
        (74, [102, 102, 178, 255]),
        (75, [192, 192, 192, 255]),
        (92, [192, 192, 192, 255]),
        (115, [255, 255, 0, 255]),
    ] {
        let trace = line_for_group(ordinal);
        assert!(matches!(
            trace.binding,
            Some(LineBinding::SegmentTrace { .. })
        ));
        assert_eq!(trace.color, color);
    }
    let segment_trace_paint_order = scene
        .lines
        .iter()
        .filter(|line| matches!(line.binding, Some(LineBinding::SegmentTrace { .. })))
        .filter_map(|line| line.debug.as_ref().map(|debug| debug.group_ordinal))
        .collect::<Vec<_>>();
    assert_eq!(segment_trace_paint_order, [92, 75, 74, 115]);
    assert_eq!(
        scene
            .buttons
            .iter()
            .filter(|button| matches!(
                button.action,
                ButtonAction::MovePoint { .. } | ButtonAction::MovePoints { .. }
            ))
            .count(),
        2
    );
}

#[test]
fn hejixu_fold2_exports_marked_ratio_dilation_and_reflection() {
    let Some(data) = fixture_bytes("tests/Samples/个人专栏/贺基旭作品/翻折2(hjx4882).gsp")
    else {
        return;
    };
    let scene = fixture_scene(&data);

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
    let f_index = point_index_for_group(9);
    let g_index = point_index_for_group(11);
    let h_index = point_index_for_group(12);

    assert!(
        matches!(
            scene.points[f_index].binding,
            Some(ScenePointBinding::ScaleByRatio {
                source_index: 3,
                center_index: 1,
                ratio_origin_index: 1,
                ratio_denominator_index: 3,
                ratio_numerator_index: 4,
                signed: false,
                clamp_to_unit: true,
            })
        ),
        "expected F from Dilation/MarkedRatio(4,2,8) to stay live as a scale-by-ratio point"
    );
    assert!(
        (scene.points[f_index].position.x - 628.795).abs() < 0.01
            && (scene.points[f_index].position.y - 242.205).abs() < 0.01,
        "expected F to match the initial clamped marked-ratio dilation position"
    );
    assert!(
        matches!(
            scene.points[h_index].binding,
            Some(ScenePointBinding::Reflect {
                source_index,
                line_start_index: 0,
                line_end_index: 4,
            }) if source_index == f_index
        ),
        "expected H to reflect F across segment AE"
    );
    assert!(
        scene.polygons.iter().any(|polygon| matches!(
            &polygon.binding,
            Some(ShapeBinding::PointPolygon { vertex_indices })
                if vertex_indices == &vec![0, g_index, h_index, 4]
        )),
        "expected folded polygon A-G-H-E to stay linked to reflected points"
    );
    assert!(
        scene.labels.iter().any(|label| {
            label.text == "F"
                && matches!(
                    label.binding,
                    Some(TextLabelBinding::PointAnchor {
                        point_index,
                        ..
                    }) if point_index == f_index
                )
        }),
        "expected label F to bind to the marked-ratio dilation point"
    );
    assert!(
        scene.labels.iter().any(|label| {
            label.text == "(BE/BD) = 1"
                && label.visible
                && label.anchor.x <= 20.0
                && label.anchor.y <= 60.0
                && label.rich_markup.as_deref().is_some_and(|markup| {
                    markup.contains("</<H")
                        && markup.contains("<TxBE>")
                        && markup.contains("<TxBD>")
                        && markup.contains("<Tx = 1>")
                })
                && matches!(
                    label.binding,
                    Some(TextLabelBinding::PointDistanceRatioValue {
                        origin_index: 1,
                        denominator_index: 3,
                        numerator_index: 4,
                        clamp_to_unit: true,
                        ..
                    })
                )
        }),
        "expected the clamped BE/BD ratio measurement label at the left-top payload anchor"
    );
}

#[test]
fn preserves_circular_segment_boundary_point_interactivity() {
    let scene = fixture_scene(include_bytes!(
        "../../../tests/fixtures/未实现的系统功能/弓形周界动点.gsp"
    ));

    assert_eq!(
        scene.polygons.len(),
        1,
        "expected one filled circular segment"
    );
    assert!(matches!(
        scene.polygons[0].binding,
        Some(crate::runtime::scene::ShapeBinding::ArcBoundaryPolygon { .. })
    ));

    let boundary_point = scene
        .points
        .iter()
        .find(|point| matches!(point.constraint, ScenePointConstraint::OnPolyline { .. }))
        .expect("expected boundary point constrained to rendered perimeter");
    match &boundary_point.constraint {
        ScenePointConstraint::OnPolyline {
            points,
            segment_index,
            t,
            ..
        } => {
            assert!(points.len() >= 4, "expected sampled boundary polyline");
            assert!(
                *segment_index < points.len() - 1,
                "segment index should reference a valid boundary segment"
            );
            assert!(
                (0.0..=1.0).contains(t),
                "polyline parameter should stay normalized"
            );
        }
        _ => unreachable!(),
    }
    assert!(
        scene.lines.iter().any(|line| line.points.len() >= 4),
        "expected perimeter shape to be rendered as an interactive polyline"
    );
    assert!(
        scene
            .lines
            .iter()
            .any(|line| matches!(line.binding, Some(LineBinding::ArcBoundary { .. }))),
        "expected boundary line to stay payload-bound for reactive updates"
    );
}

#[test]
fn preserves_polygon_in_poly_gsp() {
    let scene = fixture_scene(include_bytes!(
        "../../../tests/fixtures/gsp/static/poly.gsp"
    ));

    assert_eq!(scene.polygons.len(), 1, "expected a single polygon");
    assert_eq!(
        scene.polygons[0].points.len(),
        4,
        "expected polygon to keep its four vertices"
    );
    assert_eq!(
        scene.polygons[0].color,
        [255, 128, 0, 127],
        "expected polygon fill opacity from source style metadata"
    );
    assert_eq!(scene.points.len(), 4, "expected four visible points");
    assert!(
        scene
            .points
            .iter()
            .all(|point| matches!(point.constraint, ScenePointConstraint::Free)),
        "expected polygon vertices to stay free points"
    );
}

#[test]
fn preserves_explicit_polygon_edge_segments_from_basic_shapes_htm() {
    let scene = fixture_scene(include_bytes!(
        "../../../tests/fixtures/gsp/static/基本图形.gsp"
    ));

    for ordinal in 14..=18 {
        assert!(
            scene.lines.iter().any(|line| {
                matches!(line.binding, Some(LineBinding::Segment { .. }))
                    && line
                        .debug
                        .as_ref()
                        .is_some_and(|debug| debug.group_ordinal == ordinal)
            }),
            "expected explicit htm segment #{ordinal} to stay exported"
        );
    }
}

#[test]
fn preserves_polygon_boundary_point_in_poly_point_gsp() {
    let scene = fixture_scene(include_bytes!(
        "../../../tests/fixtures/gsp/static/poly_point.gsp"
    ));

    assert_eq!(scene.polygons.len(), 1, "expected a single polygon");
    assert_eq!(
        scene.points.len(),
        5,
        "expected four vertices and one constrained point"
    );
    assert_eq!(
        scene
            .points
            .iter()
            .filter(|point| matches!(point.constraint, ScenePointConstraint::Free))
            .count(),
        4,
        "expected four free polygon vertices"
    );
    assert!(scene.points.iter().any(|point| {
        matches!(
            point.constraint,
            ScenePointConstraint::OnPolygonBoundary {
                ref vertex_indices,
                edge_index: 2,
                t,
            } if vertex_indices == &vec![0, 1, 2, 3] && (t - 0.4450450665338869).abs() < 0.001
        )
    }));
    assert!(scene.points.iter().any(|point| {
        (point.position.x - 487.23).abs() < 0.05 && (point.position.y - 262.28).abs() < 0.05
    }));
}

#[test]
fn preserves_line_gsp() {
    let scene = fixture_scene(include_bytes!(
        "../../../tests/fixtures/gsp/static/line.gsp"
    ));

    assert_eq!(scene.lines.len(), 1, "expected one line");
    assert_eq!(scene.points.len(), 2, "expected two defining points");
    let line = &scene.lines[0];
    assert!(matches!(
        line.binding,
        Some(LineBinding::Line { .. } | LineBinding::Segment { .. })
    ));
    let min_x = line
        .points
        .iter()
        .map(|point| point.x)
        .fold(f64::INFINITY, f64::min);
    let max_x = line
        .points
        .iter()
        .map(|point| point.x)
        .fold(f64::NEG_INFINITY, f64::max);
    assert!((min_x - scene.bounds.min_x).abs() < 1e-3);
    assert!((max_x - scene.bounds.max_x).abs() < 1e-3);
}

#[test]
fn preserves_ray_gsp() {
    let scene = fixture_scene(include_bytes!("../../../tests/fixtures/gsp/static/ray.gsp"));

    assert_eq!(scene.lines.len(), 1, "expected one ray");
    assert_eq!(scene.points.len(), 2, "expected two defining points");
    let line = &scene.lines[0];
    assert!(matches!(line.binding, Some(LineBinding::Ray { .. })));
    let max_x = line
        .points
        .iter()
        .map(|point| point.x)
        .fold(f64::NEG_INFINITY, f64::max);
    assert!((max_x - scene.bounds.max_x).abs() < 1e-3);
    assert!(
        line.points
            .iter()
            .any(|point| (point.x - scene.points[0].position.x).abs() < 1e-3),
        "expected ray to include its start point"
    );
}

#[test]
fn preserves_perpendicular_gsp() {
    let scene = fixture_scene(include_bytes!(
        "../../../tests/fixtures/gsp/static/perpendicular.gsp"
    ));

    assert_eq!(
        scene.lines.len(),
        2,
        "expected base segment and perpendicular line"
    );
    assert_eq!(scene.points.len(), 2, "expected two defining points");

    let base = scene
        .lines
        .iter()
        .find(|line| matches!(line.binding, Some(LineBinding::Segment { .. })))
        .expect("expected source segment");
    let perpendicular = scene
        .lines
        .iter()
        .find(|line| matches!(line.binding, Some(LineBinding::PerpendicularLine { .. })))
        .expect("expected synthesized perpendicular line");

    let base_dx = base.points[1].x - base.points[0].x;
    let base_dy = base.points[1].y - base.points[0].y;
    let perp_dx = perpendicular.points[1].x - perpendicular.points[0].x;
    let perp_dy = perpendicular.points[1].y - perpendicular.points[0].y;
    let base_len = (base_dx * base_dx + base_dy * base_dy).sqrt();
    let perp_len = (perp_dx * perp_dx + perp_dy * perp_dy).sqrt();
    let dot = base_dx * perp_dx + base_dy * perp_dy;

    assert!(
        (dot / (base_len * perp_len)).abs() < 1e-6,
        "expected perpendicular directions, got base=({base_dx},{base_dy}) and line=({perp_dx},{perp_dy})"
    );

    let through = &scene.points[1].position;
    let distance = ((through.x - perpendicular.points[0].x) * perp_dy
        - (through.y - perpendicular.points[0].y) * perp_dx)
        .abs()
        / perp_len;
    assert!(
        distance < 1e-6,
        "expected perpendicular line to pass through point B, distance={distance}"
    );
}

#[test]
fn preserves_parallel_gsp() {
    let scene = fixture_scene(include_bytes!("../../../tests/fixtures/gsp/parallel.gsp"));

    assert_eq!(
        scene.lines.len(),
        2,
        "expected base segment and parallel line"
    );
    assert_eq!(
        scene.points.len(),
        3,
        "expected two base points plus through point"
    );

    let base = scene
        .lines
        .iter()
        .find(|line| matches!(line.binding, Some(LineBinding::Segment { .. })))
        .expect("expected source segment");
    let parallel = scene
        .lines
        .iter()
        .find(|line| matches!(line.binding, Some(LineBinding::ParallelLine { .. })))
        .expect("expected synthesized parallel line");

    let base_dx = base.points[1].x - base.points[0].x;
    let base_dy = base.points[1].y - base.points[0].y;
    let parallel_dx = parallel.points[1].x - parallel.points[0].x;
    let parallel_dy = parallel.points[1].y - parallel.points[0].y;
    let base_len = (base_dx * base_dx + base_dy * base_dy).sqrt();
    let parallel_len = (parallel_dx * parallel_dx + parallel_dy * parallel_dy).sqrt();
    let cross = base_dx * parallel_dy - base_dy * parallel_dx;

    assert!(
        (cross / (base_len * parallel_len)).abs() < 1e-6,
        "expected parallel directions, got base=({base_dx},{base_dy}) and line=({parallel_dx},{parallel_dy})"
    );

    let through = &scene.points[2].position;
    let distance = ((through.x - parallel.points[0].x) * parallel_dy
        - (through.y - parallel.points[0].y) * parallel_dx)
        .abs()
        / parallel_len;
    assert!(
        distance < 1e-6,
        "expected parallel line to pass through point C, distance={distance}"
    );
}

#[test]
fn preserves_perpendicular_segment_fixture_as_line_with_perp_segment() {
    let scene = fixture_scene(include_bytes!("../../../tests/fixtures/gsp/垂线段.gsp"));

    assert_eq!(
        scene.points.len(),
        4,
        "expected three free points and the foot point"
    );
    assert_eq!(
        scene.lines.len(),
        3,
        "expected a base line, one perpendicular segment, and the right-angle marker"
    );
    assert!(
        !scene.lines.iter().any(|line| {
            matches!(
                line.binding,
                Some(
                    LineBinding::Segment {
                        start_index: 0,
                        end_index: 3,
                    } | LineBinding::Segment {
                        start_index: 3,
                        end_index: 0,
                    }
                )
            )
        }),
        "expected the base helper segment to stay suppressed"
    );

    let foot = &scene.points[3];
    assert!(
        matches!(
            foot.constraint,
            ScenePointConstraint::LineIntersection {
                left: LineConstraint::Line {
                    start_index: 0,
                    end_index: 2,
                },
                right: LineConstraint::PerpendicularLine {
                    through_index: 1,
                    line_start_index: 0,
                    line_end_index: 2,
                },
            }
        ),
        "expected the foot point to stay constrained by the payload line and perpendicular segment"
    );
    assert!(
        foot.binding.is_none(),
        "expected the foot point to use the live constraint"
    );

    assert!(scene.lines.iter().any(|line| {
        matches!(
            line.binding,
            Some(
                LineBinding::Segment {
                    start_index: 1,
                    end_index: 3,
                } | LineBinding::Segment {
                    start_index: 3,
                    end_index: 1,
                }
            )
        )
    }));
    assert!(scene.lines.iter().any(|line| {
        matches!(
            line.binding,
            Some(
                LineBinding::Line {
                    start_index: 0,
                    end_index: 2,
                } | LineBinding::Line {
                    start_index: 2,
                    end_index: 0,
                }
            )
        )
    }));
    assert!(scene.lines.iter().any(|line| {
        matches!(
            line.binding,
            Some(LineBinding::AngleMarker {
                vertex_index: 3,
                ..
            })
        )
    }));
}

#[test]
fn preserves_nested_perpendicular_parallel_bindings_in_pert_vert_gsp() {
    let scene = fixture_scene(include_bytes!("../../../tests/fixtures/gsp/pert_vert.gsp"));

    assert_eq!(
        scene.lines.len(),
        4,
        "expected base line, bisector, and marker strokes"
    );
    assert_eq!(
        scene.points.len(),
        4,
        "expected free anchor point plus midpoint construction"
    );

    let base_index = scene
        .lines
        .iter()
        .position(|line| matches!(line.binding, Some(LineBinding::Segment { .. })))
        .expect("expected source segment");
    let main_perpendicular_index = scene
        .lines
        .iter()
        .position(|line| {
            matches!(
                line.binding,
                Some(LineBinding::PerpendicularLine {
                    through_index: 3,
                    line_index: Some(0),
                    ..
                })
            )
        })
        .expect("expected midpoint perpendicular line bound to the source segment");
    assert_eq!(main_perpendicular_index, 1);
    assert_eq!(base_index, 0);

    assert!(scene.lines.iter().any(|line| {
        matches!(
            line.binding,
            Some(LineBinding::PerpendicularLine {
                through_index: 1,
                line_index: Some(1),
                ..
            })
        )
    }));
    assert!(scene.lines.iter().any(|line| {
        matches!(
            line.binding,
            Some(LineBinding::ParallelLine {
                through_index: 1,
                line_index: Some(1),
                ..
            })
        )
    }));
}

#[test]
fn preserves_bisector_gsp() {
    let scene = fixture_scene(include_bytes!(
        "../../../tests/fixtures/gsp/static/bisector.gsp"
    ));

    assert_eq!(scene.lines.len(), 1, "expected one angle bisector");
    assert_eq!(scene.points.len(), 3, "expected three defining points");

    let bisector = scene
        .lines
        .iter()
        .find(|line| matches!(line.binding, Some(LineBinding::AngleBisectorRay { .. })))
        .expect("expected synthesized angle bisector ray");

    let start = &scene.points[0].position;
    let vertex = &scene.points[1].position;
    let end = &scene.points[2].position;
    assert!(
        (bisector.points[0].x - vertex.x).abs() < 1e-6
            && (bisector.points[0].y - vertex.y).abs() < 1e-6,
        "expected bisector ray to start at the vertex"
    );
    let bisector_dx = bisector.points[1].x - bisector.points[0].x;
    let bisector_dy = bisector.points[1].y - bisector.points[0].y;
    let bisector_len = (bisector_dx * bisector_dx + bisector_dy * bisector_dy).sqrt();
    let start_dx = start.x - vertex.x;
    let start_dy = start.y - vertex.y;
    let start_len = (start_dx * start_dx + start_dy * start_dy).sqrt();
    let end_dx = end.x - vertex.x;
    let end_dy = end.y - vertex.y;
    let end_len = (end_dx * end_dx + end_dy * end_dy).sqrt();

    let distance = ((vertex.x - bisector.points[0].x) * bisector_dy
        - (vertex.y - bisector.points[0].y) * bisector_dx)
        .abs()
        / bisector_len;
    assert!(
        distance < 1e-6,
        "expected bisector ray to pass through the vertex, distance={distance}"
    );

    let start_alignment =
        (start_dx * bisector_dx + start_dy * bisector_dy) / (start_len * bisector_len);
    let end_alignment = (end_dx * bisector_dx + end_dy * bisector_dy) / (end_len * bisector_len);
    assert!(
        (start_alignment - end_alignment).abs() < 1e-6,
        "expected equal angles to both rays, got start={start_alignment} end={end_alignment}"
    );
}

#[test]
fn preserves_three_point_arc_gsp() {
    let scene = fixture_scene(include_bytes!(
        "../../../tests/fixtures/gsp/static/three_point_arc.gsp"
    ));

    assert_eq!(scene.points.len(), 3, "expected three defining points");
    assert_eq!(scene.arcs.len(), 1, "expected one three-point arc");
    assert!(
        scene.lines.is_empty(),
        "expected arc fixture not to fall back to a line"
    );

    let arc = &scene.arcs[0];
    assert!(matches!(
        arc.binding,
        Some(crate::runtime::scene::ArcBinding::ThreePointArc {
            start_index: 0,
            mid_index: 1,
            end_index: 2,
        })
    ));
    assert_eq!(arc.color, [0, 128, 0, 255]);
    assert!(
        arc.points
            .iter()
            .zip(scene.points.iter())
            .all(|(arc_point, scene_point)| {
                (arc_point.x - scene_point.position.x).abs() < 1e-6
                    && (arc_point.y - scene_point.position.y).abs() < 1e-6
            }),
        "expected arc to preserve the three source points"
    );
}

#[test]
fn preserves_arc_on_circle_gsp() {
    let scene = fixture_scene(include_bytes!(
        "../../../tests/fixtures/gsp/static/arc_on_circle.gsp"
    ));

    assert_eq!(scene.circles.len(), 1, "expected one supporting circle");
    assert!(
        scene.circles[0].dashed,
        "expected supporting circle to render dashed"
    );
    assert_eq!(scene.arcs.len(), 1, "expected one arc on the source circle");
    assert_eq!(
        scene.points.len(),
        4,
        "expected center, radius, and two arc endpoints"
    );

    let arc = &scene.arcs[0];
    assert!(matches!(
        arc.binding,
        Some(crate::runtime::scene::ArcBinding::CircleArc {
            circle_index: 0,
            start_index: 2,
            end_index: 3,
        })
    ));
    let start = &scene.points[2].position;
    let end = &scene.points[3].position;
    let midpoint = &arc.points[1];
    let center = &scene.circles[0].center;
    let radius = ((scene.circles[0].radius_point.x - center.x).powi(2)
        + (scene.circles[0].radius_point.y - center.y).powi(2))
    .sqrt();
    let start_angle = (-(start.y - center.y)).atan2(start.x - center.x);
    let end_angle = (-(end.y - center.y)).atan2(end.x - center.x);
    let midpoint_angle = (-(midpoint.y - center.y)).atan2(midpoint.x - center.x);
    let ccw_span = (end_angle - start_angle).rem_euclid(std::f64::consts::TAU);
    let ccw_mid = (midpoint_angle - start_angle).rem_euclid(std::f64::consts::TAU);

    assert!((arc.points[0].x - start.x).abs() < 1e-6 && (arc.points[0].y - start.y).abs() < 1e-6);
    assert!((arc.points[2].x - end.x).abs() < 1e-6 && (arc.points[2].y - end.y).abs() < 1e-6);
    assert!(
        ((((midpoint.x - center.x).powi(2) + (midpoint.y - center.y).powi(2)).sqrt()) - radius)
            .abs()
            < 1e-6,
        "expected synthesized midpoint to remain on the source circle"
    );
    assert!(
        (ccw_mid - ccw_span * 0.5).abs() < 1e-6,
        "expected synthesized midpoint to bisect the counterclockwise sweep"
    );
}

#[test]
fn preserves_point_on_circle_arc_gsp() {
    let scene = fixture_scene(include_bytes!(
        "../../../tests/fixtures/gsp/point_on_arc1.gsp"
    ));

    assert_eq!(scene.arcs.len(), 1, "expected one arc on the source circle");
    assert_eq!(
        scene.points.len(),
        5,
        "expected center, radius, arc endpoints, and one constrained point"
    );
    assert!(scene.points.iter().any(|point| matches!(
        point.constraint,
        ScenePointConstraint::OnCircleArc {
            center_index: 0,
            start_index: 2,
            end_index: 3,
            t,
        } if (t - 0.2648281634562194).abs() < 1e-9
    )));
}

#[test]
fn preserves_parameter_controlled_arc_on_circle_gsp() {
    let scene = fixture_scene(include_bytes!(
        "../../../tests/fixtures/gsp/static/value_point_arc_on_circle.gsp"
    ));

    assert!(
        scene.circles.iter().any(|circle| matches!(
            circle.binding,
            Some(crate::runtime::scene::ShapeBinding::PointRadiusCircle { .. })
        )),
        "expected the supporting payload circle to remain exported"
    );
    assert_eq!(
        scene.arcs.len(),
        1,
        "expected one arc driven by parameter points"
    );
    assert_eq!(
        scene.parameters.len(),
        2,
        "expected both arc endpoint parameters to remain interactive"
    );
    assert_eq!(scene.parameters[0].name, "t₁");
    assert!((scene.parameters[0].value - 1.3).abs() < 0.001);
    assert_eq!(scene.parameters[1].name, "t₂");
    assert!((scene.parameters[1].value - 0.4).abs() < 0.001);
    assert_eq!(
        scene.points.len(),
        6,
        "expected center, radius point, two parameter-controlled arc endpoints, and two legacy slider source points"
    );
    assert_eq!(
        scene
            .points
            .iter()
            .filter(|point| matches!(
                (&point.binding, &point.constraint),
                (
                    Some(ScenePointBinding::Parameter { .. }),
                    ScenePointConstraint::Free
                )
            ))
            .count(),
        2,
        "expected both payload slider source points to remain visible"
    );

    let arc = &scene.arcs[0];
    assert!(
        arc.center.is_some(),
        "expected arc-on-circle export to preserve the source center"
    );
    assert!(
        arc.counterclockwise,
        "expected circle arc to preserve sweep direction"
    );
    assert!(
        (arc.points[0].x - scene.points[2].position.x).abs() < 1e-6
            && (arc.points[0].y - scene.points[2].position.y).abs() < 1e-6
            && (arc.points[2].x - scene.points[3].position.x).abs() < 1e-6
            && (arc.points[2].y - scene.points[3].position.y).abs() < 1e-6,
        "expected arc endpoints to stay attached to the parameter-controlled points"
    );
    assert!(
        (scene.points[2].position.x - scene.points[3].position.x).abs() > 1e-6
            || (scene.points[2].position.y - scene.points[3].position.y).abs() > 1e-6,
        "expected distinct start and end points from the two payload values"
    );
}

#[test]
fn preserves_center_arc_with_parameter_controlled_center() {
    let Some(data) = fixture_bytes("tests/Samples/个人专栏/侯仰顺作品/滑块(蚂蚁).gsp")
    else {
        return;
    };
    let file = crate::format::GspFile::parse(&data).expect("fixture parses");
    let groups = file.object_groups();
    let point_map = super::collect_point_objects(&file, &groups);
    let anchors = super::collect_raw_object_anchors(&file, &groups, &point_map, None);
    let decoded_expr =
        crate::runtime::functions::try_decode_parameter_control_expr(&file, &groups, &groups[26]);
    if let Err(error) = decoded_expr {
        panic!("parameter-control expression #27 must decode: {error:?}");
    }
    let decoded_center =
        super::try_decode_parameter_controlled_point(&file, &groups, &groups[27], &anchors);
    if let Err(error) = decoded_center {
        panic!("parameter-controlled center #28 must decode: {error:?}");
    }
    let scene = fixture_scene(&data);
    let center_index = scene
        .points
        .iter()
        .position(|point| {
            point
                .debug
                .as_ref()
                .is_some_and(|debug| debug.group_ordinal == 28)
        })
        .expect("parameter-controlled center #28 must remain in the dependency graph");
    let start_index = scene
        .points
        .iter()
        .position(|point| {
            point
                .debug
                .as_ref()
                .is_some_and(|debug| debug.group_ordinal == 64)
        })
        .expect("arc start #64");
    let end_index = scene
        .points
        .iter()
        .position(|point| {
            point
                .debug
                .as_ref()
                .is_some_and(|debug| debug.group_ordinal == 68)
        })
        .expect("arc end #68");
    let arc = scene
        .arcs
        .iter()
        .find(|arc| {
            arc.debug
                .as_ref()
                .is_some_and(|debug| debug.group_ordinal == 69)
        })
        .expect("center arc #69");
    assert!(matches!(
        arc.binding,
        Some(crate::runtime::scene::ArcBinding::CenterArc {
            center_index: actual_center,
            start_index: actual_start,
            end_index: actual_end,
        }) if actual_center == center_index
            && actual_start == start_index
            && actual_end == end_index
    ));
}

#[test]
fn preserves_center_arc_with_function_rotation_endpoints() {
    let Some(data) = fixture_bytes("tests/Samples/个人专栏/况永胜作品/分数的魔变.gsp")
    else {
        return;
    };
    let file = crate::format::GspFile::parse(&data).expect("fixture parses");
    let groups = file.object_groups();
    for (ordinal, expected) in [
        (12usize, "trunc(t₁ - 0) / 分母"),
        (13, "t₁ - m₃*分母"),
        (22, "360 / 分母"),
        (23, "360 / 分母*t₁ - m₃*分母"),
    ] {
        let decoded = crate::runtime::functions::try_decode_function_expr(
            &file,
            &groups,
            &groups[ordinal - 1],
        )
        .unwrap_or_else(|error| panic!("function expression #{ordinal}: {error:?}"));
        assert_eq!(
            crate::runtime::functions::function_expr_label(decoded),
            expected
        );
    }
    let fractional_step =
        crate::runtime::functions::try_decode_function_expr(&file, &groups, &groups[11])
            .expect("fractional rotation step expression");
    assert_eq!(
        crate::runtime::functions::evaluate_expr_with_parameters(
            &fractional_step,
            0.0,
            &std::collections::BTreeMap::from([("t₁".to_string(), 6.0005)]),
        ),
        Some(5.0 / 6.0),
        "the payload's 0.001 offset must not be rounded out of the calculation"
    );
    let scene = fixture_scene(&data);
    let point_index = |ordinal| {
        scene.points.iter().position(|point| {
            point
                .debug
                .as_ref()
                .is_some_and(|debug| debug.group_ordinal == ordinal)
        })
    };
    for ordinal in [18usize, 19, 20, 21, 40, 41] {
        assert!(
            point_index(ordinal).is_some(),
            "derived point #{ordinal} must remain in the object graph"
        );
    }
    assert!(matches!(
        scene.points[point_index(18).expect("expression rotation #18")].binding,
        Some(ScenePointBinding::Rotate { .. })
    ));
    let arc = scene
        .arcs
        .iter()
        .find(|arc| {
            arc.debug
                .as_ref()
                .is_some_and(|debug| debug.group_ordinal == 42)
        })
        .expect("center arc #42");
    assert!(matches!(
        arc.binding,
        Some(crate::runtime::scene::ArcBinding::CenterArc { .. })
    ));
}

#[test]
fn preserves_hjx_arc_unfold_function_rotation_and_visible_arc() {
    let Some(data) =
        fixture_bytes("tests/Samples/个人专栏/贺基旭作品/20180905圆弧的展开(hjx4882).gsp")
    else {
        return;
    };
    let scene = fixture_scene(&data);

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
    let rotated = point_by_ordinal(11);
    let Some(ScenePointBinding::Rotate {
        source_index,
        center_index,
        angle_degrees,
        angle_expr: Some(angle_expr),
        ..
    }) = &rotated.binding
    else {
        panic!("expected H to be a live function-rotation point");
    };
    assert_eq!(
        scene.points[*source_index]
            .debug
            .as_ref()
            .unwrap()
            .group_ordinal,
        7
    );
    assert_eq!(
        scene.points[*center_index]
            .debug
            .as_ref()
            .unwrap()
            .group_ordinal,
        10
    );
    assert!(function_expr_has_parameter(angle_expr, "m₂"));
    assert!((*angle_degrees - 89.3967911216063).abs() < 1e-9);
    assert!(
        (rotated.position.x - 887.5801047234683).abs() < 1e-6
            && (rotated.position.y - 333.1981957068289).abs() < 1e-6,
        "expected H to be placed by the decoded (BM/BA)*360 degree rotation"
    );

    let arc = scene
        .arcs
        .iter()
        .find(|arc| {
            arc.debug
                .as_ref()
                .is_some_and(|debug| debug.group_ordinal == 12)
        })
        .expect("expected center arc #12");
    assert!(arc.visible, "the payload CenterArc #12 should render");
    assert_eq!(arc.color, [255, 0, 0, 255]);
    assert!((arc.points[2].x - rotated.position.x).abs() < 1e-6);
    assert!((arc.points[2].y - rotated.position.y).abs() < 1e-6);
}

#[test]
fn preserves_hidden_ray_gsp() {
    let scene = fixture_scene(include_bytes!(
        "../../../tests/fixtures/gsp/static/hide_ray.gsp"
    ));

    assert_eq!(scene.lines.len(), 2, "expected two rays in the fixture");
    assert!(
        scene.lines.iter().any(|line| !line.visible),
        "expected one ray to inherit hidden state from the source payload"
    );
    assert!(
        scene.lines.iter().any(|line| line.visible),
        "expected the visible ray to remain interactive in the exported scene"
    );
    assert!(
        scene.lines.iter().all(|line| matches!(
            line.binding,
            Some(crate::runtime::scene::LineBinding::Ray { .. })
        )),
        "expected both extracted line bindings to remain rays"
    );
}

#[test]
fn preserves_circle_center_radius_gsp() {
    let scene = fixture_scene(include_bytes!(
        "../../../tests/fixtures/gsp/circle_center_radius.gsp"
    ));

    assert!(matches!(
        scene.circles[0].binding,
        Some(crate::runtime::scene::ShapeBinding::SegmentRadiusCircle { .. })
    ));
    assert_eq!(scene.lines.len(), 1, "expected one segment");
    assert_eq!(scene.points.len(), 3, "expected three visible points");

    let circle = &scene.circles[0];
    assert!((circle.center.x - 348.0).abs() < 1e-6);
    assert!((circle.center.y - 201.0).abs() < 1e-6);
    assert!(matches!(
        circle.binding,
        Some(crate::runtime::scene::ShapeBinding::SegmentRadiusCircle {
            center_index: 2,
            line_start_index: 0,
            line_end_index: 1,
        })
    ));

    let radius = ((circle.radius_point.x - circle.center.x).powi(2)
        + (circle.radius_point.y - circle.center.y).powi(2))
    .sqrt();
    assert!(
        (radius - ((85.0_f64).powi(2) + 1.0_f64).sqrt()).abs() < 1e-6,
        "expected circle radius to match the referenced segment length"
    );
}

#[test]
fn chen_faquan_taiji_trace_exports_parameter_radius_circle() {
    let Some(data) = fixture_bytes("tests/Samples/个人专栏/陈发铨作品/太极图整体轨迹(一线天).gsp")
    else {
        return;
    };
    let scene = fixture_scene(&data);

    let circle = scene
        .circles
        .iter()
        .find(|circle| {
            circle
                .debug
                .as_ref()
                .is_some_and(|debug| debug.group_ordinal == 16)
        })
        .expect("expected payload #16 Circle by radius to export");
    assert!(
        circle.visible,
        "expected the .htm-visible radius circle to render"
    );
    assert!(matches!(
        &circle.binding,
        Some(ShapeBinding::ParameterRadiusCircle {
            center_index: 4,
            parameter_name,
            raw_per_unit,
        }) if parameter_name == "R"
            && (*raw_per_unit - crate::runtime::DEFAULT_GRAPH_RAW_PER_UNIT).abs() < 1e-9
    ));
    let radius = ((circle.radius_point.x - circle.center.x).powi(2)
        + (circle.radius_point.y - circle.center.y).powi(2))
    .sqrt();
    assert!(
        (radius - 4.0 * crate::runtime::DEFAULT_GRAPH_RAW_PER_UNIT).abs() < 1e-6,
        "expected radius to come from the R parameter value"
    );
}

#[test]
fn preserves_circle_inner_fill_gsp() {
    let Some(data) = fixture_bytes("tests/fixtures/gsp/static/circle_inner.gsp") else {
        return;
    };
    let scene = fixture_scene(&data);

    assert!(
        scene.circles.iter().any(|circle| matches!(
            circle.binding,
            Some(crate::runtime::scene::ShapeBinding::PointRadiusCircle { .. })
        )),
        "expected the payload circle to remain exported"
    );
    let circle = &scene.circles[0];
    assert_eq!(
        circle.fill_color,
        Some([255, 255, 0, 127]),
        "expected circle interior payload to preserve its fill color"
    );
    assert!(
        circle.fill_visible,
        "expected visible circle interior payload to render even when tracked separately"
    );
    assert!(matches!(
        circle.binding,
        Some(crate::runtime::scene::ShapeBinding::PointRadiusCircle {
            center_index: 0,
            radius_index: 1,
        })
    ));
}

#[test]
fn preserves_point_segment_value_segment_point_gsp() {
    let scene = fixture_scene(include_bytes!(
        "../../../tests/fixtures/gsp/static/point_segment_value_segment_point.gsp"
    ));

    assert_eq!(scene.lines.len(), 2, "expected two segments");
    let texts = scene
        .labels
        .iter()
        .map(|label| label.text.as_str())
        .collect::<Vec<_>>();
    assert!(
        texts.contains(&"C在AB上的t值 = 0.72"),
        "expected measured segment parameter label, got {texts:?}"
    );
    assert_eq!(
        scene.parameters.len(),
        0,
        "expected derived value, not slider parameter"
    );
    assert_eq!(
        scene
            .points
            .iter()
            .filter(|point| matches!(point.constraint, ScenePointConstraint::OnSegment { .. }))
            .count(),
        2,
        "expected measured point plus derived segment point"
    );
    assert!(
        scene
            .points
            .iter()
            .any(|point| matches!(point.constraint, ScenePointConstraint::OnCircle { .. })),
        "expected derived circle point"
    );
    assert!(scene.points.iter().any(|point| matches!(
        point.constraint,
        ScenePointConstraint::OnPolygonBoundary { .. }
    )));
}

#[test]
fn preserves_scaled_point_and_single_parameter_label_in_scale_gsp() {
    let scene = fixture_scene(include_bytes!(
        "../../../tests/fixtures/gsp/static/scale.gsp"
    ));

    assert_eq!(
        scene.circles.len(),
        2,
        "expected original and scaled circle"
    );
    assert!(scene.circles.iter().any(|circle| matches!(
        circle.binding,
        Some(crate::runtime::scene::ShapeBinding::DerivedTransform {
            transform: crate::runtime::scene::ShapeTransformBinding::Scale(..),
            ..
        })
    )));
    assert_eq!(
        scene.polygons.len(),
        2,
        "expected original and scaled polygon"
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
        scene.points.len() >= 3,
        "expected source point, center point, and transformed point"
    );
    assert!(scene.points.iter().any(|point| {
        matches!(
            point.binding,
            Some(ScenePointBinding::Scale { factor, .. }) if (factor - 1.0 / 3.0).abs() < 0.0001
        )
    }));
}

#[test]
fn preserves_translated_circle_and_intersection_in_translation_gsp() {
    let scene = fixture_scene(include_bytes!(
        "../../../tests/fixtures/gsp/static/translation.gsp"
    ));

    assert_eq!(
        scene.circles.len(),
        2,
        "expected original and translated circles"
    );
    assert!(scene.circles.iter().any(|circle| matches!(
        circle.binding,
        Some(crate::runtime::scene::ShapeBinding::DerivedTransform {
            transform: crate::runtime::scene::ShapeTransformBinding::TranslateDelta { .. },
            ..
        })
    )));
    let constrained_point_count = scene
        .points
        .iter()
        .filter(|point| {
            matches!(
                point.constraint,
                crate::runtime::scene::ScenePointConstraint::CircularIntersection { .. }
                    | crate::runtime::scene::ScenePointConstraint::CircleCircleIntersection { .. }
            )
        })
        .count();
    assert_eq!(
        constrained_point_count, 1,
        "expected the translated-circle intersection point to stay live"
    );
}

#[test]
fn preserves_reflection_point_circle_and_polygon_gsp() {
    let scene = fixture_scene(include_bytes!(
        "../../../tests/fixtures/gsp/static/reflection.gsp"
    ));

    assert_eq!(
        scene.circles.len(),
        2,
        "expected original and reflected circle"
    );
    assert_eq!(
        scene.polygons.len(),
        2,
        "expected original and reflected polygon"
    );
    assert!(
        scene
            .points
            .iter()
            .any(|point| matches!(point.binding, Some(ScenePointBinding::Reflect { .. })))
    );
    assert!(scene.circles.iter().any(|circle| matches!(
        circle.binding,
        Some(crate::runtime::scene::ShapeBinding::DerivedTransform {
            transform: crate::runtime::scene::ShapeTransformBinding::Reflect(..),
            ..
        })
    )));
    assert!(scene.polygons.iter().any(|polygon| matches!(
        polygon.binding,
        Some(crate::runtime::scene::ShapeBinding::DerivedTransform {
            transform: crate::runtime::scene::ShapeTransformBinding::Reflect(..),
            ..
        })
    )));
}

#[test]
fn preserves_reflected_circle_across_constructed_perpendicular_line() {
    let scene = fixture_scene(include_bytes!("../../../tests/fixtures/bug/镜像圆.gsp"));

    assert_eq!(
        scene.circles.len(),
        2,
        "expected original and reflected circles"
    );
    assert!(scene.circles.iter().any(|circle| matches!(
        circle.binding,
        Some(crate::runtime::scene::ShapeBinding::DerivedTransform {
            transform: crate::runtime::scene::ShapeTransformBinding::Reflect(
                crate::runtime::scene::AxisBinding {
                    line_index: Some(_),
                    ..
                }
            ),
            ..
        })
    )));
}

#[test]
fn preserves_translated_triangle_segments_in_congruent_triangle_fixture() {
    let scene = fixture_scene(include_bytes!(
        "../../../tests/fixtures/gsp/两个三角形标记全等.gsp"
    ));

    assert_eq!(
        scene.lines.len(),
        16,
        "expected source and translated edges plus angle and segment congruence markers"
    );
    assert_eq!(
        scene
            .lines
            .iter()
            .filter(|line| matches!(
                line.binding,
                Some(LineBinding::DerivedTransform {
                    transform: crate::runtime::scene::LineTransformBinding::Translate { .. },
                    ..
                })
            ))
            .count(),
        3,
        "expected the translated triangle to contribute three translated segment bindings"
    );
    assert_eq!(
        scene
            .lines
            .iter()
            .filter(|line| matches!(line.binding, Some(LineBinding::AngleMarker { .. })))
            .count(),
        4,
        "expected four reactive angle markers"
    );
    assert_eq!(
        scene
            .lines
            .iter()
            .filter(|line| matches!(line.binding, Some(LineBinding::SegmentMarker { .. })))
            .count(),
        6,
        "expected six segment congruence markers from payload"
    );
    assert!(scene.lines.iter().any(|line| {
        matches!(
            line.binding,
            Some(LineBinding::DerivedTransform {
                transform: crate::runtime::scene::LineTransformBinding::Translate {
                    vector_start_index: 0,
                    vector_end_index: 3,
                },
                ..
            })
        ) && line.points.len() == 2
            && (line.points[0].x - 298.0).abs() < 1e-6
            && (line.points[0].y - 237.0).abs() < 1e-6
            && (line.points[1].x - 467.0).abs() < 1e-6
            && (line.points[1].y - 250.0).abs() < 1e-6
    }));
    assert!(scene.lines.iter().any(|line| matches!(
        line.binding,
        Some(LineBinding::SegmentMarker {
            marker_class: 3,
            ..
        })
    )));
    let perpendicular_marker = scene
        .lines
        .iter()
        .find(|line| {
            matches!(
                line.binding,
                Some(LineBinding::SegmentMarker {
                    start_index: 0,
                    end_index: 1,
                    marker_class: 1,
                    ..
                })
            )
        })
        .expect("expected segment marker on translated base edge");
    let marker_dx = perpendicular_marker.points[1].x - perpendicular_marker.points[0].x;
    let marker_dy = perpendicular_marker.points[1].y - perpendicular_marker.points[0].y;
    let segment_dx = scene.points[1].position.x - scene.points[0].position.x;
    let segment_dy = scene.points[1].position.y - scene.points[0].position.y;
    assert!(
        (marker_dx * segment_dx + marker_dy * segment_dy).abs() < 1e-6,
        "expected segment marker to be perpendicular to its host segment"
    );
    assert!(
        scene.labels.iter().any(|label| label.text == "B'"),
        "expected translated point label B'"
    );
    assert!(
        scene.labels.iter().any(|label| label.text == "C'"),
        "expected translated point label C'"
    );
}

#[test]
fn decodes_bug_fixture_angle_marker_class_from_low_word() {
    let scene = fixture_scene(include_bytes!("../../../tests/fixtures/bug/测试10.gsp"));

    let angle_marker = scene
        .lines
        .iter()
        .find(|line| matches!(line.binding, Some(LineBinding::AngleMarker { .. })))
        .expect("expected payload-backed angle marker");

    assert!(matches!(
        angle_marker.binding,
        Some(LineBinding::AngleMarker {
            marker_class: 1,
            ..
        })
    ));
}
