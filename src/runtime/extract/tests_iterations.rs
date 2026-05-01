use super::test_support::{
    derive_expression_label_parameters, fixture_bytes, fixture_log, fixture_scene,
};
use crate::runtime::functions::evaluate_expr_with_parameters;
use crate::runtime::scene::{
    ButtonAction, LabelIterationFamily, LineBinding, LineIterationFamily, PointIterationFamily,
    PolygonIterationFamily, ScenePointBinding, ScenePointConstraint, TextLabelBinding,
};
use std::collections::BTreeMap;

#[test]
fn builds_equilateral_triangle_iteration_fixture_with_expression_rotation_helpers() {
    let Some(data) = fixture_bytes("tests/Samples/热研系列/迭代系列/等边三角形迭代.gsp")
    else {
        return;
    };
    let scene = fixture_scene(&data);
    let log = fixture_log(&data, "tests/Samples/热研系列/迭代系列/等边三角形迭代.gsp");

    assert!(log.contains("问题数量: 0"));
    assert!(
        scene.points.iter().any(|point| {
            matches!(
                point.binding,
                Some(ScenePointBinding::Rotate {
                    angle_expr: Some(_),
                    ..
                })
            )
        }),
        "expected the type-33 helper point to export as an expression-driven rotation"
    );
    assert!(
        scene.points.iter().any(|point| {
            !point.visible
                && matches!(
                    point.binding,
                    Some(ScenePointBinding::CoordinateSource { axis, .. })
                        if matches!(axis, crate::runtime::scene::CoordinateAxis::Horizontal)
                )
        }),
        "expected the type-23 helper point to stay exported as a hidden horizontal expression offset"
    );
}

#[test]
fn builds_spiral_arrow_iteration_fixture_with_expression_offset_seed() {
    let Some(data) =
        fixture_bytes("tests/Samples/热研系列/迭代系列/长度为1,1,2,2,3,3…的螺旋箭头迭代.gsp")
    else {
        return;
    };
    let scene = fixture_scene(&data);
    let log = fixture_log(
        &data,
        "tests/Samples/热研系列/迭代系列/长度为1,1,2,2,3,3…的螺旋箭头迭代.gsp",
    );

    assert!(log.contains("问题数量: 0"));
    assert!(
        !scene.lines.is_empty(),
        "expected the sample to stay exportable once the type-23 helper is accepted"
    );
}

#[test]
fn builds_golden_curve_iteration_fixture_without_polygon_helper_errors() {
    let Some(data) = fixture_bytes("tests/Samples/热研系列/迭代系列/黄金曲线迭代.gsp")
    else {
        return;
    };
    let _scene = fixture_scene(&data);
    let log = fixture_log(&data, "tests/Samples/热研系列/迭代系列/黄金曲线迭代.gsp");

    assert!(log.contains("问题数量: 0"));
}

#[test]
fn builds_iteration_example_fixture_without_bbox_helper_errors() {
    let Some(data) = fixture_bytes("tests/Samples/热研系列/迭代系列/迭代举例.gsp")
    else {
        return;
    };
    let _scene = fixture_scene(&data);
    let log = fixture_log(&data, "tests/Samples/热研系列/迭代系列/迭代举例.gsp");

    assert!(log.contains("问题数量: 0"));
}

#[test]
fn exports_lizhangbo_solid_geometry_parameter_buttons() {
    let Some(data) =
        fixture_bytes("tests/Samples/个人专栏/李章博作品/动画演示立体几何轨迹形成（李章博）.gsp")
    else {
        return;
    };
    let scene = fixture_scene(&data);

    let parameter_value = |name: &str| {
        scene
            .parameters
            .iter()
            .find(|parameter| parameter.name == name)
            .map(|parameter| parameter.value)
            .unwrap_or_else(|| panic!("expected parameter {name}"))
    };
    assert_eq!(parameter_value("棱长"), 6.0);
    assert_eq!(parameter_value("t[7]"), 399.0);
    assert_eq!(parameter_value("t[8]"), 0.0);
    assert_eq!(parameter_value("t[9]"), 20.0);
    assert_eq!(parameter_value("t[10]"), 0.05);
    assert_eq!(parameter_value("t[5]"), 0.0);
    let quarter_turn_parameters = derive_expression_label_parameters(
        &scene,
        BTreeMap::from([
            ("棱长".to_string(), 6.0),
            ("t[7]".to_string(), 399.0),
            ("t[8]".to_string(), 10.0),
            ("t[9]".to_string(), 20.0),
            ("t[10]".to_string(), 0.05),
            ("t[5]".to_string(), 0.0),
        ]),
    );
    let hidden_calc_value = |ordinal: usize| {
        let label = scene
            .labels
            .iter()
            .find(|label| {
                label
                    .debug
                    .as_ref()
                    .is_some_and(|debug| debug.group_ordinal == ordinal)
            })
            .unwrap_or_else(|| panic!("expected hidden calculation #{ordinal}"));
        let Some(TextLabelBinding::ExpressionValue { expr, .. }) = label.binding.as_ref() else {
            panic!("expected hidden calculation #{ordinal} to carry an expression binding");
        };
        evaluate_expr_with_parameters(expr, 0.0, &quarter_turn_parameters)
            .unwrap_or_else(|| panic!("expected hidden calculation #{ordinal} to evaluate"))
    };
    assert!(
        (hidden_calc_value(106) - 3.0 * 2.0_f64.sqrt()).abs() < 1e-6,
        "expected hidden sine calculation to use GSP degree-mode pi-angle payload"
    );
    assert!(
        (hidden_calc_value(108) - 3.0 * 2.0_f64.sqrt()).abs() < 1e-6,
        "expected hidden cosine calculation to use GSP degree-mode pi-angle payload"
    );
    assert_eq!(
        scene.line_iterations.len(),
        0,
        "the fixture uses point iteration, not carried segment iteration"
    );
    assert_eq!(
        scene.point_iterations.len(),
        2,
        "expected P and the payload-derived N translation to be exported as point traces"
    );
    assert!(
        scene.point_iterations.iter().any(|family| matches!(
            family,
            PointIterationFamily::Parameterized {
                depth_parameter_name: Some(depth_parameter_name),
                trace_parameter_name,
                ..
            } if depth_parameter_name == "t[7]" && trace_parameter_name == "t[8]"
        )),
        "expected the RegularPolygonIteration payload to export parameterized point iteration"
    );

    let reset = scene
        .buttons
        .iter()
        .find(|button| button.text == "初 始 化")
        .expect("expected initialization move button");
    match &reset.action {
        ButtonAction::SetParameter {
            parameter_name,
            value,
        } => {
            assert_eq!(parameter_name, "t[7]");
            assert_eq!(*value, 0.0);
        }
        action => panic!("expected set-parameter action, got {action:?}"),
    }

    let trace = scene
        .buttons
        .iter()
        .find(|button| button.text == "轨迹生成")
        .expect("expected trace-generation move button");
    match &trace.action {
        ButtonAction::AnimateParameter {
            parameter_name,
            target_value,
        } => {
            assert_eq!(parameter_name, "t[7]");
            assert_eq!(*target_value, 399.0);
        }
        action => panic!("expected animate-parameter action, got {action:?}"),
    }
}

#[test]
fn preserves_parameter_driven_point_iteration_family() {
    let scene = fixture_scene(include_bytes!(
        "../../../tests/fixtures/gsp/static/简单迭代/原象点初象点深度5迭代.gsp"
    ));

    assert_eq!(scene.parameters.len(), 1, "expected n parameter");
    assert_eq!(scene.parameters[0].name, "n");
    assert_eq!(
        scene.point_iterations.len(),
        1,
        "expected one point iteration family"
    );
    match &scene.point_iterations[0] {
        PointIterationFamily::Offset {
            seed_index,
            depth,
            parameter_name,
            ..
        } => {
            assert_eq!(
                *seed_index, 1,
                "expected initial image point as iteration seed"
            );
            assert_eq!(*depth, 5, "expected exported depth");
            assert_eq!(parameter_name.as_deref(), Some("n"));
        }
        family => panic!("expected offset iteration family, got {family:?}"),
    }
    assert_eq!(
        scene.points.len(),
        8,
        "expected original point, initial point, 5 iterates, and the legacy parameter source point"
    );
}

#[test]
fn preserves_non_graph_parameter_and_expression_labels_in_iteration_fixture() {
    let scene = fixture_scene(include_bytes!(
        "../../../tests/fixtures/gsp/static/简单迭代/原象点和参数初象点和数值深度5迭代.gsp"
    ));

    let parameter_names = scene
        .parameters
        .iter()
        .map(|parameter| parameter.name.as_str())
        .collect::<Vec<_>>();
    assert!(
        parameter_names.contains(&"n"),
        "expected n parameter, got {parameter_names:?}"
    );
    assert!(
        parameter_names.contains(&"a"),
        "expected a parameter, got {parameter_names:?}"
    );
    assert!(scene.labels.iter().any(|label| {
        matches!(
            label.binding,
            Some(TextLabelBinding::ParameterValue { ref name }) if name == "a"
        )
    }));
    assert!(scene.labels.iter().any(|label| {
        matches!(
            label.binding,
            Some(TextLabelBinding::PointExpressionValue {
                ref parameter_name,
                ..
            }) if parameter_name == "a"
        )
    }));
    assert!(scene.labels.iter().any(|label| {
        matches!(
            label.binding,
            Some(TextLabelBinding::ExpressionValue {
                ref parameter_name,
                ref expr_label,
                ..
            }) if parameter_name == "a" && expr_label == "a + 1"
        )
    }));
    assert!(scene.point_iterations.iter().any(|family| {
        matches!(
            family,
            PointIterationFamily::Offset {
                dx,
                dy,
                parameter_name,
                ..
            } if parameter_name.as_deref() == Some("n")
                && (*dx - 37.79527559055118).abs() < 1e-6
                && dy.abs() < 1e-6
        )
    }));
    assert!(scene.label_iterations.iter().any(|family| {
        matches!(
            family,
            LabelIterationFamily::PointExpression {
                parameter_name,
                depth_parameter_name,
                ..
            } if parameter_name == "a" && depth_parameter_name.as_deref() == Some("n")
        )
    }));
}

#[test]
fn preserves_default_depth_non_graph_iteration_fixture() {
    let scene = fixture_scene(include_bytes!(
        "../../../tests/fixtures/gsp/static/简单迭代/原象点和参数初象点和数值默认深度迭代.gsp"
    ));

    let parameter_names = scene
        .parameters
        .iter()
        .map(|parameter| parameter.name.as_str())
        .collect::<Vec<_>>();
    assert_eq!(parameter_names, vec!["a"]);
    assert!(scene.point_iterations.iter().any(|family| {
        matches!(
            family,
            PointIterationFamily::RotateChain {
                seed_index,
                center_index,
                angle_degrees,
                depth,
            } if *seed_index == 2
                && *center_index == 0
                && (*angle_degrees - 30.0).abs() < 1e-6
                && *depth == 3
        )
    }));
    assert!(scene.label_iterations.iter().any(|family| {
        matches!(
            family,
            LabelIterationFamily::PointExpression {
                parameter_name,
                depth,
                depth_parameter_name,
                ..
            } if parameter_name == "a" && *depth == 3 && depth_parameter_name.is_none()
        )
    }));
}

#[test]
fn preserves_carried_segment_default_depth_iteration_fixture() {
    let scene = fixture_scene(include_bytes!(
        "../../../tests/fixtures/gsp/static/简单迭代/原象点初象携带线段默认深度3迭代.gsp"
    ));

    assert_eq!(
        scene.lines.len(),
        4,
        "expected original segment plus three carried copies"
    );
    assert_eq!(
        scene.points.len(),
        5,
        "expected original point, seed point, and three iterates"
    );
    let starts = scene
        .lines
        .iter()
        .map(|line| line.points.first().cloned().expect("segment start"))
        .collect::<Vec<_>>();
    assert!(
        starts
            .iter()
            .any(|point| { (point.x - 168.0).abs() < 1e-6 && (point.y - 376.0).abs() < 1e-6 })
    );
    assert!(starts.iter().any(|point| {
        (point.x - 205.79527559055117).abs() < 1e-6 && (point.y - 338.20472440944883).abs() < 1e-6
    }));
    assert!(starts.iter().any(|point| {
        (point.x - 243.59055118110234).abs() < 1e-6 && (point.y - 300.40944881889766).abs() < 1e-6
    }));
    assert!(starts.iter().any(|point| {
        (point.x - 281.3858267716535).abs() < 1e-6 && (point.y - 262.6141732283465).abs() < 1e-6
    }));
}

#[test]
fn preserves_carried_polygon_iteration_fixture() {
    let scene = fixture_scene(include_bytes!(
        "../../../tests/fixtures/gsp/static/简单迭代/原象点初象携带多边形双映射深度4迭代.gsp"
    ));

    assert_eq!(
        scene.polygons.len(),
        16,
        "expected htm source polygon plus triangular lattice of carried copies"
    );
    assert!(
        scene.polygons.iter().any(|polygon| polygon
            .debug
            .as_ref()
            .is_some_and(|debug| debug.group_ordinal == 5)),
        "expected htm source polygon #5 to stay exported"
    );
    for ordinal in 6..=8 {
        assert!(
            scene.lines.iter().any(|line| {
                matches!(line.binding, Some(LineBinding::Segment { .. }))
                    && line
                        .debug
                        .as_ref()
                        .is_some_and(|debug| debug.group_ordinal == ordinal)
            }),
            "expected htm source segment #{ordinal} to stay exported"
        );
    }
    assert!(
        scene
            .parameters
            .iter()
            .any(|parameter| parameter.name == "n")
    );
    assert!(
        scene.line_iterations.is_empty(),
        "expected carried polygon fixture to avoid duplicate line iteration metadata"
    );
    assert_eq!(scene.polygon_iterations.len(), 1);
    assert!(scene.polygon_iterations.iter().any(|family| {
        matches!(
            family,
            PolygonIterationFamily::Translate {
                parameter_name,
                depth,
                vertex_indices,
                secondary_dx,
                secondary_dy,
                dx,
                dy,
                ..
            } if parameter_name.as_deref() == Some("n")
                && *depth == 4
                && *vertex_indices == vec![0, 2, 1]
                && secondary_dx.is_some()
                && secondary_dy.is_some()
                && dx.abs() < 1e-6
                && (*dy + 37.79527559055118).abs() < 1e-6
        )
    }));
    assert_eq!(
        scene.points.len(),
        4,
        "expected base point, two mapped vertices, and the legacy parameter source point"
    );
    assert!(matches!(
        scene.points[1].constraint,
        ScenePointConstraint::Offset {
            origin_index: 0,
            dx,
            dy,
        } if (dx - 37.79527559055118).abs() < 1e-6
            && (dy + 37.79527559055118).abs() < 1e-6
    ));
    assert!(matches!(
        scene.points[2].constraint,
        ScenePointConstraint::Offset {
            origin_index: 0,
            dx,
            dy,
        } if dx.abs() < 1e-6 && (dy + 37.79527559055118).abs() < 1e-6
    ));
    let first_vertices = scene
        .polygons
        .iter()
        .map(|polygon| polygon.points.first().cloned().expect("polygon vertex"))
        .collect::<Vec<_>>();
    assert!(
        first_vertices
            .iter()
            .any(|point| { (point.x - 168.0).abs() < 1e-6 && (point.y - 376.0).abs() < 1e-6 })
    );
    assert!(first_vertices.iter().any(|point| {
        (point.x - 168.0).abs() < 1e-6 && (point.y - 338.20472440944883).abs() < 1e-6
    }));
    assert!(first_vertices.iter().any(|point| {
        (point.x - 168.0).abs() < 1e-6 && (point.y - 300.40944881889766).abs() < 1e-6
    }));
    assert!(first_vertices.iter().any(|point| {
        (point.x - 168.0).abs() < 1e-6 && (point.y - 262.6141732283465).abs() < 1e-6
    }));
    assert!(first_vertices.iter().any(|point| {
        (point.x - 168.0).abs() < 1e-6 && (point.y - 224.81889763779532).abs() < 1e-6
    }));
    assert!(first_vertices.iter().any(|point| {
        (point.x - 205.79527559055117).abs() < 1e-6 && (point.y - 300.40944881889766).abs() < 1e-6
    }));
    assert!(first_vertices.iter().any(|point| {
        (point.x - 243.59055118110234).abs() < 1e-6 && (point.y - 262.6141732283465).abs() < 1e-6
    }));
}

#[test]
fn preserves_default_depth_point_iteration_family() {
    let scene = fixture_scene(include_bytes!(
        "../../../tests/fixtures/gsp/static/简单迭代/原象点初象点默认深度3迭代.gsp"
    ));

    assert!(
        scene.parameters.is_empty(),
        "expected default-depth fixture without editable parameters"
    );
    assert_eq!(
        scene.point_iterations.len(),
        1,
        "expected one default-depth point iteration family"
    );
    match &scene.point_iterations[0] {
        PointIterationFamily::Offset {
            seed_index,
            depth,
            parameter_name,
            ..
        } => {
            assert_eq!(
                *seed_index, 1,
                "expected initial image point as iteration seed"
            );
            assert_eq!(*depth, 3, "expected default depth of three");
            assert_eq!(parameter_name, &None);
        }
        family => panic!("expected offset iteration family, got {family:?}"),
    }
    assert_eq!(
        scene.points.len(),
        5,
        "expected original point, initial point, and three default iterates"
    );
    assert!(
        matches!(
            scene.points[1].constraint,
            ScenePointConstraint::Offset {
                origin_index: 0,
                dx,
                dy
            } if (dx - 37.79527559055118).abs() < 1e-6
                && (dy + 37.79527559055118).abs() < 1e-6
        ),
        "expected legacy initial image point to preserve its 1cm horizontal and vertical offsets"
    );
}

#[test]
fn does_not_treat_triangle_point_labels_as_iteration_parameters() {
    let scene = fixture_scene(include_bytes!(
        "../../../tests/fixtures/gsp/static/简单迭代/三角形.gsp"
    ));

    assert!(
        scene.parameters.is_empty(),
        "expected no editable parameters in triangle fixture"
    );
    assert_eq!(scene.line_iterations.len(), 3);
    assert!(
        scene
            .line_iterations
            .iter()
            .all(|family| matches!(family, LineIterationFamily::Affine { .. }))
    );
}

#[test]
fn preserves_midpoint_triangle_iteration_geometry() {
    let scene = fixture_scene(include_bytes!(
        "../../../tests/fixtures/gsp/static/简单迭代/三角形.gsp"
    ));

    for name in ["D", "E", "F"] {
        assert!(
            scene.labels.iter().any(|label| {
                label.text == name
                    && matches!(label.binding, Some(TextLabelBinding::PointAnchor { .. }))
            }),
            "expected midpoint label {name} to stay anchored to its point"
        );
    }

    assert!(scene.lines.iter().any(|line| {
        line.points.len() == 2
            && (line.points[0].x - 751.0).abs() < 0.01
            && (line.points[0].y - 467.5).abs() < 0.01
            && (line.points[1].x - 853.0).abs() < 0.01
            && (line.points[1].y - 319.5).abs() < 0.01
    }));
    assert!(
        !scene.lines.iter().any(|line| {
            line.points.len() == 2
                && (line.points[0].x - 367.0).abs() < 0.01
                && (line.points[0].y - 786.0).abs() < 0.01
        }),
        "expected midpoint recursion, not translated copies"
    );
}

#[test]
fn preserves_regular_polygon_iteration_without_carried_duplicates() {
    let scene = fixture_scene(include_bytes!(
        "../../../tests/fixtures/gsp/static/简单迭代/迭代正多边形.gsp"
    ));

    assert_eq!(scene.parameters.len(), 1, "expected editable n parameter");
    assert_eq!(scene.parameters[0].name, "n");
    assert_eq!(
        scene.lines.len(),
        1,
        "expected the payload's first related edge to stay serialized as the iteration source"
    );
    assert_eq!(
        scene
            .lines
            .iter()
            .filter(|line| line
                .debug
                .as_ref()
                .is_some_and(|debug| debug.group_ordinal == 7))
            .count(),
        1,
        "expected the serialized source edge to come from payload segment #7"
    );
    assert!(
        scene.line_iterations.iter().any(|family| matches!(
            family,
            LineIterationFamily::Rotate {
                source_index,
                parameter_name,
                depth_parameter_name,
                depth,
                ..
            } if *source_index == 0
                && parameter_name.as_deref() == Some("n")
                && depth_parameter_name.is_none()
                && *depth == 4
        )),
        "expected regular polygon iteration to export the payload source edge plus a rotate family for the carried copies"
    );
    assert_eq!(
        scene.line_iterations.len(),
        1,
        "expected one canonical rotate family for the regular polygon payload"
    );
    let interactive_segment = scene
        .lines
        .iter()
        .find_map(|line| match line.binding {
            Some(LineBinding::Segment {
                start_index,
                end_index,
            }) => Some((start_index, end_index)),
            _ => None,
        })
        .expect("expected the payload source edge to remain an interactive segment");
    assert!(
        scene
            .points
            .get(interactive_segment.0)
            .is_some_and(|point| point.draggable),
        "expected the payload source vertex to remain draggable"
    );
    assert!(
        matches!(
            scene
                .points
                .get(interactive_segment.1)
                .and_then(|point| point.binding.as_ref()),
            Some(ScenePointBinding::Rotate {
                source_index,
                angle_degrees,
                parameter_name,
                angle_expr,
                ..
            }) if *source_index == interactive_segment.0
                && (*angle_degrees - 72.0).abs() < 0.01
                && parameter_name.as_deref() == Some("n")
                && angle_expr.is_some()
        ),
        "expected the payload rotated endpoint to remain a live rotate-bound point"
    );
    assert!(
        scene.labels.iter().any(|label| {
            label.visible
                && label.text == "360° / n = 72.00°"
                && matches!(
                    label.binding.as_ref(),
                    Some(TextLabelBinding::ExpressionValue {
                        parameter_name,
                        expr_label,
                        ..
                    }) if parameter_name == "n" && expr_label == "360° / n"
                )
        }),
        "expected the payload angle label to stay bound to the regular-polygon rotation expression"
    );
}
