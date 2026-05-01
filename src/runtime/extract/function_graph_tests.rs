use super::analysis::analyze_scene;
use super::points::collect_point_objects;
use super::test_support::{fixture_bytes, fixture_log, fixture_scene};
use crate::format::GspFile;
use crate::runtime::functions::{BinaryOp, FunctionAst, FunctionExpr, UnaryFunction};
use crate::runtime::scene::{
    ButtonAction, LineBinding, LineIterationFamily, ScenePointBinding, ScenePointConstraint,
    TextLabelBinding,
};

#[test]
fn preserves_parabola_locus_with_constructed_line_driver() {
    let Some(data) =
        fixture_bytes("tests/Samples/个人专栏/贺基旭作品/20171231抛物线的光学性质_hjx4882.gsp")
    else {
        return;
    };
    let scene = fixture_scene(&data);
    let trace_line = scene
        .lines
        .iter()
        .find(|line| {
            line.debug
                .as_ref()
                .is_some_and(|debug| debug.group_ordinal == 11)
                && matches!(line.binding, Some(LineBinding::PointTrace { .. }))
        })
        .expect("expected payload #11 Locus to export as a point trace");
    assert!(
        trace_line.points.len() >= 100,
        "expected the parabola locus to keep its sampled payload curve"
    );
    let (min_x, max_x, min_y, max_y) = trace_line.points.iter().fold(
        (
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::INFINITY,
            f64::NEG_INFINITY,
        ),
        |(min_x, max_x, min_y, max_y), point| {
            (
                min_x.min(point.x),
                max_x.max(point.x),
                min_y.min(point.y),
                max_y.max(point.y),
            )
        },
    );
    assert!(
        max_x - min_x > 250.0 && max_y - min_y > 300.0,
        "expected the constructed-line locus parameter to use the payload host scale"
    );
    let Some(LineBinding::PointTrace {
        point_index,
        driver_index,
        ..
    }) = trace_line.binding
    else {
        unreachable!();
    };
    assert!(
        matches!(
            scene.points[driver_index].constraint,
            ScenePointConstraint::OnLineConstraint { .. }
        ),
        "expected the locus driver to remain constrained to the constructed perpendicular line"
    );
    assert_ne!(
        point_index, driver_index,
        "expected the traced intersection and driver point to stay distinct"
    );
    assert!(
        scene.points.iter().any(|point| matches!(
            &point.constraint,
            ScenePointConstraint::OnPolyline {
                function_key,
                points,
                ..
            } if *function_key == 11 && points.len() == trace_line.points.len()
        )),
        "expected the point on Locus #11 to stay constrained to the live trace polyline"
    );
    assert!(
        scene
            .parameters
            .iter()
            .any(|parameter| parameter.name == "N" && (parameter.value - 28.0).abs() < 1e-6),
        "expected payload parameter N to decode as 28 for the spectrum iteration"
    );
    assert!(
        scene.labels.iter().any(|label| {
            label.text == "5在L₁上的值 = 0.03"
                && matches!(
                    label.binding,
                    Some(TextLabelBinding::PolylineParameter { point_index: 8, .. })
                )
        }),
        "expected the point-on-locus parameter label to follow the payload ParameterAnchor"
    );
    assert!(
        scene
            .labels
            .iter()
            .any(|label| label.text == "5在L₁上的值 + 1 / N = 0.06"),
        "expected the dependent calculation label to use the point-on-locus parameter name"
    );
    assert!(
        scene
            .labels
            .iter()
            .any(|label| label.text == "t₁ + 0.1 = 0.10"),
        "expected the htm-style decimal calculation t₁ + 0.1 to stay decoded"
    );
    let spectrum_line_count = scene
        .lines
        .iter()
        .filter(|line| {
            line.debug
                .as_ref()
                .is_some_and(|debug| matches!(debug.group_ordinal, 30 | 31))
        })
        .count();
    assert!(
        spectrum_line_count >= 56,
        "expected both Colorized_Spectrum derived segments to expand across N steps"
    );
    assert!(
        scene.lines.iter().any(|line| {
            line.debug
                .as_ref()
                .is_some_and(|debug| debug.group_ordinal == 30)
                && matches!(
                    line.binding,
                    Some(LineBinding::ColorizedSpectrum {
                        point_index: 8,
                        trace_endpoint_index: 1,
                        depth_parameter_name: Some(ref name),
                        ray: false,
                        ..
                    }) if name == "N"
                )
        }),
        "expected the segment Colorized_Spectrum payload to stay bound to point 4"
    );
    assert!(
        scene.lines.iter().any(|line| {
            line.debug
                .as_ref()
                .is_some_and(|debug| debug.group_ordinal == 31)
                && matches!(
                    line.binding,
                    Some(LineBinding::ColorizedSpectrum {
                        point_index: 8,
                        depth_parameter_name: Some(ref name),
                        ray: true,
                        ..
                    }) if name == "N"
                )
        }),
        "expected the ray Colorized_Spectrum payload to stay bound to its live ray direction"
    );
}

#[test]
fn preserves_binary_tree_multimap_iteration() {
    let Some(data) = fixture_bytes("../Samples/个人专栏/方小庆作品/二叉树(inRm).gsp")
    else {
        return;
    };
    let scene = fixture_scene(&data);
    assert_eq!(
        scene.line_iterations.len(),
        1,
        "expected one recursive line family for the binary tree payload"
    );
    let LineIterationFamily::Branching {
        target_segments,
        parameter_name,
        depth,
        ..
    } = &scene.line_iterations[0]
    else {
        panic!("expected binary tree iteration to export branching segment handles");
    };
    assert_eq!(
        target_segments.len(),
        2,
        "expected the payload to produce two child segment maps"
    );
    assert_eq!(parameter_name.as_deref(), Some("n"));
    assert_eq!(*depth, 7, "expected depth to stay driven by payload n");
    assert_eq!(
        scene.lines.len(),
        255,
        "expected one seed segment plus 2^1..2^7 recursive branches"
    );
    assert!(
        scene
            .line_iterations
            .iter()
            .all(|family| !matches!(family, LineIterationFamily::Affine { .. })),
        "expected the binary tree payload to avoid the carried affine fallback"
    );
    assert!(
        scene.points.iter().take(2).all(|point| point.draggable),
        "expected the free endpoints to remain interactive"
    );
    assert!(
        scene.points.iter().any(|point| {
            point.visible
                && matches!(
                    point.binding,
                    Some(ScenePointBinding::Parameter { ref name }) if name == "n"
                )
        }),
        "expected the legacy n parameter control point to stay visible in the exported scene"
    );
}

#[test]
fn builds_polygon_exterior_angle_sample_with_kind_41_helpers() {
    let Some(data) = fixture_bytes("tests/Samples/个人专栏/王伟君作品/多边形外角和(王伟君).gsp")
    else {
        return;
    };
    let scene = fixture_scene(&data);

    assert!(
        scene.points.iter().filter(|point| point.draggable).count() >= 7,
        "expected the exterior-angle sample to keep its seed vertices interactive"
    );
    assert!(
        scene
            .labels
            .iter()
            .any(|label| label.text.contains("∠1") || label.text.contains("360")),
        "expected the sample labels driven by kind 41 helpers to remain exported"
    );
}

#[test]
fn resolves_unknown_59_measurement_helper_in_statistics_sample() {
    let Some(data) = fixture_bytes("tests/Samples/工具例说/14 统计工具-统计工具示例.gsp")
    else {
        return;
    };
    let file = GspFile::parse(&data).expect("sample parses");
    let groups = file.object_groups();
    let point_map = collect_point_objects(&file, &groups);
    let anchors = crate::runtime::extract::shapes::collect_raw_object_anchors(
        &file, &groups, &point_map, None,
    );
    let helper = groups
        .iter()
        .find(|group| group.ordinal == 14)
        .expect("expected unknown 59 helper");

    let (start, end) = crate::runtime::extract::points::resolve_line_like_points_raw(
        &file, &groups, &anchors, helper,
    )
    .expect("expected unknown 59 helper to resolve as a line-like object");

    assert!(
        ((end.x - start.x).powi(2) + (end.y - start.y).powi(2)).sqrt() > 1.0,
        "expected unknown 59 helper to expose a non-degenerate measurement segment"
    );
}

#[test]
fn resolves_unknown_88_iteration_point_alias_in_statistics_sample() {
    let Some(data) = fixture_bytes("tests/Samples/工具例说/14 统计工具-统计工具示例.gsp")
    else {
        return;
    };
    let file = GspFile::parse(&data).expect("sample parses");
    let groups = file.object_groups();
    let point_map = collect_point_objects(&file, &groups);
    let anchors = crate::runtime::extract::shapes::collect_raw_object_anchors(
        &file, &groups, &point_map, None,
    );
    let helper = groups
        .iter()
        .find(|group| group.ordinal == 58)
        .expect("expected unknown 88 helper");

    let alias = crate::runtime::extract::points::decode_iteration_binding_point_alias_raw(
        &file, &groups, helper, &anchors,
    )
    .expect("expected unknown 88 helper to resolve as an iteration point alias");

    assert!(
        alias.position.x.is_finite() && alias.position.y.is_finite(),
        "expected unknown 88 helper to resolve to a concrete point"
    );
}

#[test]
fn builds_square_area_invariance_sample_with_graph_helper_stack() {
    let Some(data) = fixture_bytes("tests/Samples/个人专栏/向忠作品/正方形总面积不变.gsp")
    else {
        return;
    };
    let scene = fixture_scene(&data);

    assert!(
        !scene.points.is_empty(),
        "expected the graph-helper sample to export points"
    );
    assert!(
        !scene.point_iterations.is_empty() || !scene.polygon_iterations.is_empty(),
        "expected the graph-helper sample to export iteration-driven geometry"
    );
}

#[test]
fn builds_point_cood_expr_fixture_with_two_parameter_coordinate_binding() {
    let Some(data) = fixture_bytes("tests/fixtures/gsp/point_cood_expr.gsp") else {
        return;
    };
    let file = GspFile::parse(&data).expect("fixture parses");
    let groups = file.object_groups();
    let point_map = collect_point_objects(&file, &groups);
    let analysis = analyze_scene(&file, &groups, &point_map);
    let helper_group = groups.get(7).expect("group #8");
    let helper_path = super::find_indexed_path(&file, helper_group).expect("helper path");
    let parameter_group = groups
        .get(helper_path.refs[0].saturating_sub(1))
        .expect("parameter group");
    let parameter_value =
        super::try_decode_parameter_control_value_for_group(&file, &groups, parameter_group)
            .expect("parameter value");
    assert!((parameter_value - 1.0).abs() < 1e-6, "expected t₁ = 1");
    assert!(
        analysis
            .raw_anchors
            .get(1)
            .and_then(|point| point.as_ref())
            .is_some(),
        "expected origin anchor point to exist"
    );
    let helper_point = super::points::decode_coordinate_point(
        &file,
        &groups,
        helper_group,
        &analysis.raw_anchors,
        &analysis.graph_ref,
    );
    assert!(
        helper_point.is_some(),
        "expected legacy helper group #8 to decode as a coordinate point"
    );
    let scene = fixture_scene(&data);
    let log = fixture_log(&data, "tests/fixtures/gsp/point_cood_expr.gsp");

    assert!(log.contains("问题数量: 0"));
    let point = scene
        .points
        .iter()
        .find(|point| {
            matches!(
                point.binding,
                Some(ScenePointBinding::CoordinateSource2d {
                    ref x_name,
                    ref y_name,
                    ..
                }) if x_name == "t₂" && y_name == "t₁"
            )
        })
        .expect("expected 2d coordinate point driven by both parameter controls");
    assert!(
        point.visible,
        "expected exported coordinate point to stay visible"
    );
    assert!(
        point.draggable,
        "expected coordinate point to stay interactive"
    );

    let helper = scene
        .points
        .iter()
        .find(|point| {
            matches!(
                point.binding,
                Some(ScenePointBinding::CoordinateSource {
                    ref name,
                    axis: crate::runtime::scene::CoordinateAxis::Vertical,
                    ..
                }) if name == "t₁"
            ) && (point.position.x.abs() < 1e-6)
                && ((point.position.y - 1.0).abs() < 1e-6)
        })
        .expect("expected legacy parameter helper point at (0,1)");
    assert!(helper.visible, "expected helper point to stay visible");
    assert!(
        helper.draggable,
        "expected helper point to stay interactive"
    );
}

#[test]
fn builds_music_fixture_with_play_button() {
    let scene = fixture_scene(include_bytes!("../../../tests/fixtures/gsp/music.gsp"));

    assert!(scene.graph_mode, "expected graph scene");
    assert_eq!(scene.buttons.len(), 1, "expected one music button");
    match &scene.buttons[0].action {
        ButtonAction::PlayFunction { function_key } => assert_eq!(*function_key, 7),
        action => panic!("expected play-function action, got {action:?}"),
    }
    assert!(
        !scene.lines.is_empty(),
        "expected the music fixture to export its function plot"
    );
    assert_eq!(scene.functions.len(), 1, "expected one exported function");
    assert_eq!(scene.functions[0].name, "f");
    assert_eq!(
        scene.functions[0].expr,
        FunctionExpr::Parsed(FunctionAst::Binary {
            lhs: Box::new(FunctionAst::Constant(5.0)),
            op: BinaryOp::Mul,
            rhs: Box::new(FunctionAst::Unary {
                op: UnaryFunction::Sin,
                expr: Box::new(FunctionAst::Binary {
                    lhs: Box::new(FunctionAst::Constant(25.0)),
                    op: BinaryOp::Mul,
                    rhs: Box::new(FunctionAst::Variable),
                }),
            }),
        })
    );
    assert!(
        scene
            .labels
            .iter()
            .any(|label| label.text == "f(x) = 5*sin(25*x)"),
        "expected the music fixture label to expose the recovered legacy function expression"
    );
}

#[test]
fn builds_music1_fixture_with_legacy_frequency_expr() {
    let Some(data) = fixture_bytes("tests/fixtures/gsp/music1.gsp") else {
        return;
    };
    let scene = fixture_scene(&data);

    assert_eq!(scene.functions.len(), 1, "expected one exported function");
    assert_eq!(
        scene.functions[0].expr,
        FunctionExpr::Parsed(FunctionAst::Binary {
            lhs: Box::new(FunctionAst::Constant(5.0)),
            op: BinaryOp::Mul,
            rhs: Box::new(FunctionAst::Unary {
                op: UnaryFunction::Sin,
                expr: Box::new(FunctionAst::Binary {
                    lhs: Box::new(FunctionAst::Constant(24.0)),
                    op: BinaryOp::Mul,
                    rhs: Box::new(FunctionAst::Variable),
                }),
            }),
        })
    );
}

#[test]
fn yx2_axis_symmetry_honors_function_definition_visibility() {
    let scene = fixture_scene(include_bytes!(
        "../../../tests/Samples/个人专栏/贺基旭作品/y=x^2的轴对称性(hjx4882).gsp"
    ));

    assert_eq!(scene.functions.len(), 1, "expected one plotted function");
    assert_eq!(scene.functions[0].key, 8);
    assert!(
        scene.labels.iter().any(|label| {
            label.visible
                && label.text == "y = a*(x^2)"
                && label
                    .debug
                    .as_ref()
                    .is_some_and(|debug| debug.group_ordinal == 25)
        }),
        "expected visible payload function definition label #25"
    );
    assert!(
        !scene.labels.iter().any(|label| {
            label.visible
                && matches!(
                    label.binding,
                    Some(TextLabelBinding::FunctionLabel {
                        function_key: 8,
                        derivative: false
                    })
                )
        }),
        "hidden plotted helper function #8 must not get a synthesized visible label"
    );
    let segment_trace_count = scene
        .lines
        .iter()
        .filter(|line| {
            line.visible
                && line.points.len() == 2
                && line
                    .debug
                    .as_ref()
                    .is_some_and(|debug| debug.group_ordinal == 21)
        })
        .count();
    assert!(
        segment_trace_count >= 500,
        "expected visible line trace #21, got {segment_trace_count} sampled segments"
    );
    assert!(
        scene.lines.iter().any(|line| {
            line.visible
                && line.points.len() >= 100
                && line
                    .debug
                    .as_ref()
                    .is_some_and(|debug| debug.group_ordinal == 22)
                && matches!(line.binding, Some(LineBinding::PointTrace { .. }))
        }),
        "expected visible point trace #22"
    );
    assert!(
        scene.lines.iter().any(|line| {
            line.visible
                && line.points.len() >= 100
                && line
                    .debug
                    .as_ref()
                    .is_some_and(|debug| debug.group_ordinal == 24)
                && matches!(line.binding, Some(LineBinding::PointTrace { .. }))
        }),
        "expected visible function/reflection point trace #24"
    );
    assert!(
        scene.buttons.iter().any(|button| button.visible
            && matches!(button.action, ButtonAction::Sequence { .. })
            && button
                .debug
                .as_ref()
                .is_some_and(|debug| debug.group_ordinal == 27)),
        "expected the payload-only sequence button to stay visible and interactive"
    );
}

#[test]
fn builds_xy_coordinate_fixture_with_live_coordinate_label() {
    let scene = fixture_scene(include_bytes!("../../../tests/fixtures/gsp/xy_cood.gsp"));

    assert!(
        scene.labels.iter().any(|label| {
            label.text == "B: (-9.82, 5.93)"
                && matches!(
                    &label.binding,
                    Some(crate::runtime::scene::TextLabelBinding::PointCoordinateValue {
                        point_name,
                        ..
                    }) if point_name == "B"
                )
        }),
        "expected xy_cood fixture to export a live coordinate readout label"
    );
    assert!(
        scene.labels.iter().any(|label| {
            label.text == "x = -9.82"
                && matches!(
                    &label.binding,
                    Some(TextLabelBinding::PointAxisValue { name, .. }) if name == "x"
                )
                && ((label.anchor.x + 9.82).abs() > 0.1 || (label.anchor.y - 5.93).abs() > 0.1)
        }),
        "expected xy_cood fixture to export the x coordinate helper label"
    );
    assert!(
        scene.labels.iter().any(|label| {
            label.text == "y = 5.93"
                && matches!(
                    &label.binding,
                    Some(TextLabelBinding::PointAxisValue { name, .. }) if name == "y"
                )
                && ((label.anchor.x + 9.82).abs() > 0.1 || (label.anchor.y - 5.93).abs() > 0.1)
        }),
        "expected xy_cood fixture to export the y coordinate helper label"
    );
    assert!(
        scene.labels.iter().any(|label| {
            label.text == "BC = 8.29 厘米"
                && matches!(
                    &label.binding,
                    Some(TextLabelBinding::PointDistanceValue {
                        name,
                        value_suffix,
                        ..
                    }) if name == "BC" && value_suffix == " 厘米"
                )
        }),
        "expected xy_cood fixture to export the distance helper label"
    );
}

#[test]
fn simple_coordinate_sample_follows_exported_axis_coordinate_system() {
    let Some(data) = fixture_bytes("tests/Samples/简易数轴与坐标系/最简坐标系/样本1.gsp")
    else {
        return;
    };
    let scene = fixture_scene(&data);

    assert!(
        !scene.graph_mode,
        "hidden CoordSysByAxes scaffolding must not render the viewer grid"
    );
    assert!(
        scene.y_up,
        "expected hidden coordinate system to map y upward"
    );
    let point = scene
        .points
        .iter()
        .find(|point| {
            point.visible
                && point.color == [255, 0, 0, 255]
                && (point.position.x - 2.51).abs() < 0.01
                && (point.position.y - 2.86).abs() < 0.01
        })
        .expect("expected visible point A");
    assert!((point.position.x - 2.51).abs() < 0.01);
    assert!((point.position.y - 2.86).abs() < 0.01);
    assert!(
        scene.labels.iter().any(|label| {
            label.visible
                && label.text == "A: (2.51, 2.86)"
                && matches!(
                    label.binding,
                    Some(TextLabelBinding::PointCoordinateValue { .. })
                )
        }),
        "expected coordinate readout from the exported Coordinates(42,12,...) object"
    );
    assert!(
        scene.labels.iter().any(|label| {
            label.visible && label.screen_space && label.text == "※标准化\n※控制点"
        }),
        "expected button label to follow the exported screen-space button placement"
    );
}

#[test]
fn simple_coordinate_sample_exports_left_calculation_labels() {
    let Some(data) = fixture_bytes("tests/Samples/简易数轴与坐标系/最简坐标系/样本2.gsp")
    else {
        return;
    };
    let scene = fixture_scene(&data);

    for expected in ["Xmax = 3", "Xmin = -2", "Ymax = 3", "Ymin = -2"] {
        assert!(
            scene.labels.iter().any(|label| {
                label.visible
                    && label.screen_space
                    && label.text == expected
                    && label.anchor.x < 20.0
                    && label.anchor.y < 110.0
                    && matches!(label.binding, Some(TextLabelBinding::PointAxisValue { .. }))
            }),
            "expected visible left-side calculation label {expected}"
        );
    }

    for (prefix, x, y, axis) in [
        (
            "Xmax",
            3.513_541_666_666_668,
            0.0,
            crate::runtime::scene::CoordinateAxis::Horizontal,
        ),
        (
            "Xmin",
            -2.613_958_333_333_335,
            0.0,
            crate::runtime::scene::CoordinateAxis::Horizontal,
        ),
        (
            "Ymax",
            0.0,
            3.698_750_000_000_001_8,
            crate::runtime::scene::CoordinateAxis::Vertical,
        ),
        (
            "Ymin",
            0.0,
            -2.455_208_333_333_335,
            crate::runtime::scene::CoordinateAxis::Vertical,
        ),
    ] {
        let label = scene
            .labels
            .iter()
            .find(|label| label.text.starts_with(prefix))
            .expect("expected axis calculation label");
        let Some(TextLabelBinding::PointAxisValue {
            point_index,
            axis: label_axis,
            origin_index,
            x_unit_index,
            y_unit_index,
            ..
        }) = label.binding
        else {
            panic!("expected {prefix} to be bound to a point axis");
        };
        assert_eq!(label_axis, axis);
        assert_eq!(origin_index, Some(0));
        assert_eq!(x_unit_index, Some(1));
        assert_eq!(y_unit_index, Some(2));
        let point = &scene.points[point_index];
        assert!((point.position.x - x).abs() < 1e-6);
        assert!((point.position.y - y).abs() < 1e-6);
    }

    for (x, y) in [(3.513_541_666_666_668, 0.0), (0.0, 3.698_750_000_000_001_8)] {
        assert!(
            scene.points.iter().any(|point| {
                point.visible
                    && point.draggable
                    && (point.position.x - x).abs() < 1e-6
                    && (point.position.y - y).abs() < 1e-6
                    && matches!(point.binding, Some(ScenePointBinding::GraphCalibration))
                    && matches!(
                        point.constraint,
                        ScenePointConstraint::Offset {
                            origin_index: 3,
                            ..
                        }
                    )
            }),
            "expected visible arrow control graph calibration point at ({x},{y})"
        );
    }

    for (x, y) in [(-2.613_958_333_333_335, 0.0), (0.0, -2.455_208_333_333_335)] {
        assert!(
            scene.points.iter().any(|point| {
                point.visible
                    && point.draggable
                    && (point.position.x - x).abs() < 1e-6
                    && (point.position.y - y).abs() < 1e-6
                    && matches!(point.binding, Some(ScenePointBinding::Rotate { .. }))
            }),
            "expected visible constructed arrow endpoint at ({x},{y})"
        );
    }
}

#[test]
fn builds_function_plot_for_f_gsp() {
    let Some(data) = fixture_bytes("f.gsp") else {
        return;
    };
    let scene = fixture_scene(&data);

    assert!(scene.graph_mode);
    assert!(
        scene.lines.iter().any(|line| {
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
            min_x <= 0.1 && max_x > 30.0
        }),
        "expected a non-degenerate function plot spanning the graph domain"
    );
    assert!(scene.bounds.min_x < -9.0);
    assert!(scene.bounds.max_y > 14.0);
    assert_eq!(scene.labels.len(), 1);
    assert_eq!(
        scene.labels[0]
            .text
            .strip_prefix("q(x) = ")
            .or_else(|| scene.labels[0].text.strip_prefix("f(x) = ")),
        Some("|x| + √x + ln(x) + log(x) + sgn(x) + round(x) + trunc(x)")
    );
}

#[test]
fn calibration_only_geometry_fixture_does_not_enable_graph_mode() {
    let Some(data) = fixture_bytes("tests/fixtures/bug/20260421角平分线的作用.gsp") else {
        return;
    };
    let scene = fixture_scene(&data);

    assert!(
        !scene.graph_mode,
        "a lone graph calibration helper should not enable graph mode"
    );
    assert!(
        !scene.lines.is_empty(),
        "expected geometry lines to remain exported"
    );
    let point_a = scene
        .points
        .iter()
        .find(|point| {
            point
                .debug
                .as_ref()
                .is_some_and(|debug| debug.group_ordinal == 1)
        })
        .expect("expected source point A");
    assert!(
        point_a.visible,
        "the reference htm declares A as Point(...)[label('A'),dot]"
    );
    let point_o = scene
        .points
        .iter()
        .find(|point| {
            point
                .debug
                .as_ref()
                .is_some_and(|debug| debug.group_ordinal == 8)
        })
        .expect("expected point O on the sector boundary");
    assert!(
        point_o.visible && !point_o.draggable,
        "payload places O at the sector boundary center, so it should behave as the derived circumcenter"
    );
    assert!(
        matches!(point_o.constraint, ScenePointConstraint::Free),
        "the exported O point should be driven by its circumcenter binding, not by a draggable static polyline"
    );
    let Some(ScenePointBinding::Circumcenter {
        start_index,
        mid_index,
        end_index,
    }) = &point_o.binding
    else {
        panic!("expected O to carry a circumcenter binding");
    };
    let circumcenter_ordinals = [*start_index, *mid_index, *end_index].map(|point_index| {
        scene.points[point_index]
            .debug
            .as_ref()
            .unwrap()
            .group_ordinal
    });
    assert_eq!(
        circumcenter_ordinals,
        [2, 5, 1],
        "O should be the center of the payload three-point arc through B, C, and A"
    );
    let visible_arc_ordinals = scene
        .arcs
        .iter()
        .filter(|arc| arc.visible)
        .filter_map(|arc| arc.debug.as_ref().map(|debug| debug.group_ordinal))
        .collect::<Vec<_>>();
    assert!(
        visible_arc_ordinals.contains(&6) && visible_arc_ordinals.contains(&9),
        "the source payload's two black arcs should render as the circumcircle outline"
    );
    let problem_text = scene
        .labels
        .iter()
        .find(|label| label.text.starts_with("如图，O是△ABC的外接圆"))
        .expect("expected the rich problem statement label");
    assert!(problem_text.visible, "expected problem text to be visible");
    assert!(
        problem_text.screen_space,
        "problem text should stay anchored in document screen space"
    );
    assert_eq!(problem_text.anchor.x, 38.0);
    assert_eq!(problem_text.anchor.y, 24.0);
}
