use super::test_support::{
    derive_expression_label_parameters, fixture_bytes, fixture_log, fixture_scene,
};
use crate::runtime::functions::evaluate_expr_with_parameters;
use crate::runtime::scene::{
    ButtonAction, LabelIterationFamily, LineBinding, LineIterationFamily, PointIterationFamily,
    PolygonIterationFamily, RichTextExpressionValue, ScenePointBinding, ScenePointConstraint,
    TextLabelBinding,
};
use std::collections::BTreeMap;

#[test]
fn circle_to_square_sample_keeps_live_calculations_and_bidirectional_sector_iterations() {
    let data = include_bytes!("../../../tests/Samples/个人专栏/李章博作品/割圆为方（李章博）.gsp");
    let file = crate::format::GspFile::parse(data).expect("fixture parses");
    let groups = file.object_groups();
    assert!(crate::runtime::functions::function_expr_uses_degree_units(
        &file, &groups, &groups[3]
    ));
    assert!(crate::runtime::functions::function_expr_uses_degree_units(
        &file,
        &groups,
        &groups[16]
    ));
    let scene = fixture_scene(data);

    let angle_step = scene
        .labels
        .iter()
        .find(|label| {
            label
                .debug
                .as_ref()
                .is_some_and(|debug| debug.group_ordinal == 4)
        })
        .expect("expected the payload angle-step calculation #4");
    let Some(TextLabelBinding::ExpressionValue {
        parameter_name,
        expr: angle_step_expr,
        ..
    }) = angle_step.binding.as_ref()
    else {
        panic!("expected angle step to carry an expression binding");
    };
    assert_eq!(parameter_name, "n");
    let parameters = BTreeMap::from([("n".to_string(), 10.0), ("b".to_string(), 0.5)]);
    let angle_step_value = evaluate_expr_with_parameters(angle_step_expr, 0.0, &parameters)
        .expect("expected angle step to evaluate");
    assert!((angle_step_value - 9.0).abs() < 1e-9);
    let transition_angle = scene
        .labels
        .iter()
        .find(|label| {
            label
                .debug
                .as_ref()
                .is_some_and(|debug| debug.group_ordinal == 17)
        })
        .expect("expected the payload transition-angle calculation #17");
    let Some(TextLabelBinding::ExpressionValue {
        parameter_name,
        expr: transition_angle_expr,
        ..
    }) = transition_angle.binding.as_ref()
    else {
        panic!("expected transition angle to carry an expression binding");
    };
    assert_eq!(parameter_name, "b");
    let transition_angle_value =
        evaluate_expr_with_parameters(transition_angle_expr, 0.0, &parameters)
            .expect("expected transition angle to evaluate");
    assert!((transition_angle_value + 2.25).abs() < 1e-9);

    assert_eq!(
        scene.polygons.len(),
        40,
        "expected four seeds and 36 iterated sectors"
    );
    let seed_colors = scene.polygons[..4]
        .iter()
        .map(|polygon| polygon.color)
        .collect::<Vec<_>>();
    assert_eq!(
        seed_colors,
        vec![
            [255, 0, 0, 127],
            [0, 128, 0, 127],
            [0, 128, 0, 127],
            [255, 0, 0, 127],
        ],
        "expected transformed sectors to retain their own payload styles"
    );
    assert_eq!(scene.polygon_iterations.len(), 4);
    let mut inverse_count = 0;
    let mut source_indices = scene
        .polygon_iterations
        .iter()
        .map(|family| match family {
            PolygonIterationFamily::Similarity {
                source_index,
                source_start_index,
                source_end_index,
                target_start_index,
                target_end_index,
                depth,
                depth_expr,
                inverse,
                ..
            } => {
                assert_eq!(*depth, 9);
                assert_ne!(source_start_index, source_end_index);
                assert_ne!(target_start_index, target_end_index);
                let live_depth = evaluate_expr_with_parameters(
                    depth_expr.as_ref().expect("expected live n - 1 depth"),
                    0.0,
                    &BTreeMap::from([("n".to_string(), 6.0)]),
                );
                assert_eq!(live_depth, Some(5.0));
                inverse_count += usize::from(*inverse);
                *source_index
            }
            other => panic!("expected similarity iteration, got {other:?}"),
        })
        .collect::<Vec<_>>();
    source_indices.sort_unstable();
    assert_eq!(source_indices, vec![0, 1, 2, 3]);
    assert_eq!(
        inverse_count, 2,
        "expected one inverse branch above and below"
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
fn exports_factorial_rich_label_as_live_iteration_state() {
    let Some(data) = fixture_bytes("tests/Samples/未分类档/N!.gsp") else {
        return;
    };
    let scene = fixture_scene(&data);
    let label = scene
        .labels
        .iter()
        .find(|label| label.text == "6!=720")
        .expect("expected factorial result label");
    let Some(TextLabelBinding::RichTextExpressionValues { refs, .. }) = label.binding.as_ref()
    else {
        panic!("expected factorial label to carry rich-text value refs");
    };
    assert!(
        refs.iter().any(|reference| matches!(
            &reference.value,
            RichTextExpressionValue::Parameter { name } if name == "n"
        )),
        "expected the first rich-text slot to stay linked to parameter n"
    );
    assert!(
        refs.iter().any(|reference| matches!(
            &reference.value,
            RichTextExpressionValue::IterationState {
                state_parameter_names,
                target_parameter_name,
                depth_expr: Some(_),
                ..
            } if state_parameter_names == &vec!["t₁".to_string(), "s".to_string()]
                && target_parameter_name == "s"
        )),
        "expected the second rich-text slot to read the iterated y-state"
    );
    let table = scene
        .iteration_tables
        .first()
        .expect("expected iteration expression helper table");
    assert_eq!(
        table
            .columns
            .iter()
            .map(|column| column.expr_label.as_str())
            .collect::<Vec<_>>(),
        vec!["t₁ + 1", "s*(t₁ + 1)"],
        "expected the payload helper table to preserve both expression columns"
    );
    assert!(
        table.depth_expr.is_some(),
        "expected the payload helper table depth to use n - 1 rather than raw n"
    );
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
        3,
        "expected all three payload point bindings to become interpreted point traces"
    );
    assert!(
        scene.point_iterations.iter().any(|family| matches!(
            family,
            PointIterationFamily::Interpreted {
                depth_parameter_name: Some(depth_parameter_name),
                ..
            } if depth_parameter_name == "t[7]"
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
    let PointIterationFamily::Interpreted {
        point_index,
        states,
        depth,
        depth_parameter_name,
        ..
    } = &scene.point_iterations[0];
    assert_eq!(
        *point_index, 1,
        "expected initial image point as iteration seed"
    );
    assert_eq!(*depth, 5, "expected exported depth");
    assert_eq!(depth_parameter_name.as_deref(), Some("n"));
    assert_eq!(states.len(), 1);
    assert_eq!(states[0].source_group_ordinal, 1);
    assert_eq!(states[0].image_group_ordinal, 2);
    assert_eq!(
        scene.points.len(),
        3,
        "expected original point, initial point, and the legacy parameter source point"
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
            PointIterationFamily::Interpreted {
                depth_parameter_name,
                states,
                ..
            } if depth_parameter_name.as_deref() == Some("n")
                && states.len() == 2
                && states[0].source_group_ordinal == 1
                && states[0].image_group_ordinal == 5
                && states[1].source_group_ordinal == 3
                && states[1].image_group_ordinal == 4
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
            PointIterationFamily::Interpreted {
                point_index,
                states,
                depth,
                ..
            } if *point_index == 2
                && states.len() == 2
                && states[0].source_group_ordinal == 4
                && states[0].image_group_ordinal == 5
                && states[1].source_group_ordinal == 2
                && states[1].image_group_ordinal == 3
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
        2,
        "expected the payload's original point and initial image; later images are interpreted"
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
    let PointIterationFamily::Interpreted {
        point_index,
        states,
        depth,
        depth_parameter_name,
        ..
    } = &scene.point_iterations[0];
    assert_eq!(
        *point_index, 1,
        "expected initial image point as iteration seed"
    );
    assert_eq!(*depth, 3, "expected default depth of three");
    assert_eq!(depth_parameter_name, &None);
    assert_eq!(states.len(), 1);
    assert_eq!(
        scene.points.len(),
        2,
        "expected only the payload's original point and initial image point"
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
