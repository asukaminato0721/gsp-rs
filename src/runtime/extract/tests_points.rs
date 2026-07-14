use super::analysis::analyze_scene;
use super::build_scene_checked;
use super::points::{
    collect_point_objects, decode_directed_angle_anchor_binding,
    try_decode_parameter_controlled_point,
};
use super::test_support::{fixture_bytes, fixture_log, fixture_scene, function_expr_has_unary};
use crate::format::GspFile;
use crate::runtime::functions::UnaryFunction;
use crate::runtime::scene::{
    ArcBinding, ArcConstraint, ButtonAction, CircularConstraint, ColorBinding,
    IterationTableValueBinding, LineBinding, LineConstraint, LineLikeKind, LineTransformBinding,
    ScenePointBinding, ScenePointConstraint, ShapeBinding, TextLabelBinding,
};

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
        Some(ArcBinding::DerivedTransform { .. })
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
    let file = GspFile::parse(&data).expect("rolling-sector fixture parses");
    let groups = file.object_groups();
    let point_map = collect_point_objects(&file, &groups);
    let analysis = analyze_scene(&file, &groups, &point_map);
    let controlled =
        try_decode_parameter_controlled_point(&file, &groups, &groups[13], &analysis.raw_anchors)
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
    let scene = build_scene_checked(&file).expect("rolling-sector scene builds");
    assert!(scene.points.iter().any(|point| {
        point
            .debug
            .as_ref()
            .is_some_and(|debug| debug.group_ordinal == 14)
    }));

    let pages = file.page_files();
    assert_eq!(pages.len(), 4);
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
