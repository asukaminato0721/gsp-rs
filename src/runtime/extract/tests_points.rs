use super::build_scene_checked;
use super::test_support::{fixture_bytes, fixture_log, fixture_scene, function_expr_has_unary};
use crate::format::GspFile;
use crate::runtime::functions::UnaryFunction;
use crate::runtime::scene::{
    ButtonAction, CircularConstraint, LineBinding, LineConstraint, LineTransformBinding,
    ScenePointBinding, ScenePointConstraint, ShapeBinding, TextLabelBinding,
};

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
            Some(LineBinding::DerivedTransform {
                source_index,
                transform: LineTransformBinding::Rotate(binding),
            }) if *source_index == source_bl_line_index
                && binding.center_index == center_point_index
                && binding.angle_start_index == Some(point_index_for_group(16))
                && binding.angle_vertex_index == Some(point_index_for_group(15))
                && binding.angle_end_index == Some(point_index_for_group(18))
        ),
        "expected #32 to rotate #29 from measured angle #30"
    );
    for (label_ordinal, (start_index, end_index)) in
        [(12, segment12), (13, segment23), (14, segment01)]
    {
        assert!(
            matches!(
                &label_for_group(label_ordinal).binding,
                Some(TextLabelBinding::SegmentProjectionParameter {
                    point_index,
                    start_index: actual_start_index,
                    end_index: actual_end_index,
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
                    ScenePointBinding::Coordinate { name, .. } if name == "t₁"
                )
            })
        }),
        "expected coordinate-controlled point"
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
