use super::build_scene;
use crate::format::GspFile;
use crate::runtime::scene::{
    LabelIterationFamily, LineBinding, PointIterationFamily, ScenePointBinding,
    ScenePointConstraint, TextLabelBinding,
};

#[test]
fn builds_function_plot_for_f_gsp() {
    let data = include_bytes!("../../../../f.gsp");
    let file = GspFile::parse(data).expect("fixture parses");
    let scene = build_scene(&file);

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
    assert!(scene.bounds.min_x < -30.0);
    assert!(scene.bounds.max_y > 100.0);
    assert_eq!(scene.labels.len(), 1);
    assert_eq!(
        scene.labels[0].text,
        "f(x) = |x| + √x + ln(x) + log(x) + sgn(x) + round(x) + trunc(x)"
    );
}

#[test]
fn preserves_constrained_points_in_edge_gsp() {
    let data = include_bytes!("../../../../edge.gsp");
    let file = GspFile::parse(data).expect("fixture parses");
    let scene = build_scene(&file);

    assert_eq!(scene.circles.len(), 2);
    assert_eq!(scene.points.len(), 11);
    assert!(scene.points.iter().any(|point| {
        matches!(
            point.constraint,
            ScenePointConstraint::OnCircle {
                center_index: 0,
                radius_index: 1,
                ..
            }
        )
    }));
    assert!(scene.points.iter().any(|point| {
        matches!(
            point.constraint,
            ScenePointConstraint::OnPolygonBoundary {
                ref vertex_indices,
                ..
            } if vertex_indices == &vec![2, 6, 3, 4]
        )
    }));
    assert!(scene.points.iter().any(|point| {
        (point.position.x + 9.17159).abs() < 0.01 && (point.position.y - 5.598877).abs() < 0.01
    }));
    assert!(scene.points.iter().any(|point| {
        (point.position.x + 4.956433).abs() < 0.01 && (point.position.y - 1.163518).abs() < 0.01
    }));
    assert!(
        scene
            .points
            .iter()
            .any(|point| { matches!(point.constraint, ScenePointConstraint::OnPolyline { .. }) })
    );
    assert_eq!(
        scene
            .labels
            .iter()
            .map(|label| label.text.as_str())
            .collect::<Vec<_>>(),
        vec![
            "a = 3.00",
            "b = 1.00",
            "f(x) = x + a*sin(x) + b",
            "f'(x) = 1 + a*cos(x)",
        ]
    );
}

#[test]
fn preserves_translated_points_in_point_translation_gsp() {
    let data = include_bytes!("../../../tests/fixtures/gsp/static/point_translation.gsp");
    let file = GspFile::parse(data).expect("fixture parses");
    let scene = build_scene(&file);

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
fn preserves_polygon_in_poly_gsp() {
    let data = include_bytes!("../../../tests/fixtures/gsp/static/poly.gsp");
    let file = GspFile::parse(data).expect("fixture parses");
    let scene = build_scene(&file);

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
fn preserves_polygon_boundary_point_in_poly_point_gsp() {
    let data = include_bytes!("../../../tests/fixtures/gsp/static/poly_point.gsp");
    let file = GspFile::parse(data).expect("fixture parses");
    let scene = build_scene(&file);

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
fn preserves_polygon_labels_in_poly_point_with_val_gsp() {
    let data = include_bytes!("../../../tests/fixtures/gsp/static/poly_point_with_val.gsp");
    let file = GspFile::parse(data).expect("fixture parses");
    let scene = build_scene(&file);

    assert_eq!(scene.polygons.len(), 1, "expected a single polygon");
    assert_eq!(
        scene.points.len(),
        5,
        "expected four vertices and one constrained point"
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
        texts.contains(&"D"),
        "expected point label D, got {texts:?}"
    );
    assert!(
        texts.contains(&"E"),
        "expected constrained point label E, got {texts:?}"
    );
    assert!(
        texts.contains(&"E在ABCD上的t值 = 0.58"),
        "expected polygon parameter label, got {texts:?}"
    );
}

#[test]
fn preserves_segment_parameter_label_in_segment_point_value_gsp() {
    let data = include_bytes!("../../../tests/fixtures/gsp/static/segment_point_value.gsp");
    let file = GspFile::parse(data).expect("fixture parses");
    let scene = build_scene(&file);

    assert_eq!(scene.lines.len(), 1, "expected one segment");
    assert_eq!(
        scene.points.len(),
        3,
        "expected two endpoints and one constrained point"
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
        texts.contains(&"C在AB上的t值 = 0.51"),
        "expected segment parameter label, got {texts:?}"
    );
}

#[test]
fn preserves_line_gsp() {
    let data = include_bytes!("../../../tests/fixtures/gsp/static/line.gsp");
    let file = GspFile::parse(data).expect("fixture parses");
    let scene = build_scene(&file);

    assert_eq!(scene.lines.len(), 1, "expected one line");
    assert_eq!(scene.points.len(), 2, "expected two defining points");
    let line = &scene.lines[0];
    assert!(matches!(line.binding, Some(LineBinding::Line { .. })));
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
    let data = include_bytes!("../../../tests/fixtures/gsp/static/ray.gsp");
    let file = GspFile::parse(data).expect("fixture parses");
    let scene = build_scene(&file);

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
fn preserves_point_segment_value_segment_point_gsp() {
    let data =
        include_bytes!("../../../tests/fixtures/gsp/static/point_segment_value_segment_point.gsp");
    let file = GspFile::parse(data).expect("fixture parses");
    let scene = build_scene(&file);

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
fn preserves_circle_parameter_label_in_circle_point_value_gsp() {
    let data = include_bytes!("../../../tests/fixtures/gsp/static/circle_point_value.gsp");
    let file = GspFile::parse(data).expect("fixture parses");
    let scene = build_scene(&file);

    assert_eq!(scene.circles.len(), 1, "expected one circle");
    assert_eq!(
        scene.points.len(),
        3,
        "expected center, radius point, and constrained point"
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
        texts.contains(&"C在⊙AB上的值 = 0.38"),
        "expected circle parameter label, got {texts:?}"
    );
}

#[test]
fn preserves_parameter_controlled_point_on_segment_gsp() {
    let data = include_bytes!("../../../tests/fixtures/gsp/static/point_on_segment.gsp");
    let file = GspFile::parse(data).expect("fixture parses");
    let scene = build_scene(&file);

    assert_eq!(scene.lines.len(), 1, "expected one segment");
    assert_eq!(
        scene.points.len(),
        3,
        "expected endpoints plus controlled point"
    );
    assert_eq!(scene.parameters.len(), 1, "expected t parameter");
    assert_eq!(scene.parameters[0].name, "t₁");
    assert!((scene.parameters[0].value - 0.01).abs() < 0.001);
    assert!(scene.points.iter().any(|point| {
        matches!(
            point.constraint,
            ScenePointConstraint::OnSegment { t, .. } if (t - 0.01).abs() < 0.001
        )
    }));
}

#[test]
fn preserves_parameter_controlled_point_on_poly_gsp() {
    let data = include_bytes!("../../../tests/fixtures/gsp/static/point_on_poly.gsp");
    let file = GspFile::parse(data).expect("fixture parses");
    let scene = build_scene(&file);

    assert_eq!(scene.polygons.len(), 1, "expected one polygon");
    assert_eq!(
        scene.points.len(),
        4,
        "expected polygon vertices plus controlled point"
    );
    assert_eq!(scene.parameters.len(), 1, "expected t parameter");
    assert_eq!(scene.parameters[0].name, "t₁");
    assert!(scene.points.iter().any(|point| matches!(
        point.constraint,
        ScenePointConstraint::OnPolygonBoundary { .. }
    )));
}

#[test]
fn preserves_parameter_controlled_point_on_circle_gsp() {
    let data = include_bytes!("../../../tests/fixtures/gsp/static/point_on_circle.gsp");
    let file = GspFile::parse(data).expect("fixture parses");
    let scene = build_scene(&file);

    assert_eq!(scene.circles.len(), 1, "expected one circle");
    assert_eq!(
        scene.points.len(),
        3,
        "expected circle points plus controlled point"
    );
    assert_eq!(scene.parameters.len(), 1, "expected t parameter");
    assert_eq!(scene.parameters[0].name, "t₁");
    assert!(
        scene
            .points
            .iter()
            .any(|point| matches!(point.constraint, ScenePointConstraint::OnCircle { .. }))
    );
}

#[test]
fn preserves_coordinate_point_in_cood_gsp() {
    let data = include_bytes!("../../../tests/fixtures/gsp/static/cood.gsp");
    let file = GspFile::parse(data).expect("fixture parses");
    let scene = build_scene(&file);

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
fn preserves_coordinate_trace_in_cood_trace_gsp() {
    let data = include_bytes!("../../../tests/fixtures/gsp/static/cood-trace.gsp");
    let file = GspFile::parse(data).expect("fixture parses");
    let scene = build_scene(&file);

    assert!(scene.graph_mode, "expected graph scene");
    assert_eq!(scene.parameters.len(), 1, "expected t parameter");
    assert_eq!(scene.parameters[0].name, "t₁");
    assert!(
        scene.lines.iter().any(|line| {
            line.points.len() > 100
                && line
                    .points
                    .first()
                    .is_some_and(|point| point.x.abs() < 0.001)
                && line
                    .points
                    .first()
                    .is_some_and(|point| (point.y - 1.0).abs() < 0.001)
                && line
                    .points
                    .last()
                    .is_some_and(|point| (point.x - 1.0).abs() < 0.001)
                && line
                    .points
                    .last()
                    .is_some_and(|point| (point.y - 2.0).abs() < 0.001)
        }),
        "expected sampled coordinate trace line"
    );
}

#[test]
fn preserves_parameter_driven_point_iteration_family() {
    let data =
        include_bytes!("../../../tests/fixtures/gsp/static/简单迭代/原象点初象点深度5迭代.gsp");
    let file = GspFile::parse(data).expect("fixture parses");
    let scene = build_scene(&file);

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
        7,
        "expected original point, initial point, and 5 iterates"
    );
}

#[test]
fn preserves_non_graph_parameter_and_expression_labels_in_iteration_fixture() {
    let data = include_bytes!(
        "../../../tests/fixtures/gsp/static/简单迭代/原象点和参数初象点和数值深度5迭代.gsp"
    );
    let file = GspFile::parse(data).expect("fixture parses");
    let scene = build_scene(&file);

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
    let data = include_bytes!(
        "../../../tests/fixtures/gsp/static/简单迭代/原象点和参数初象点和数值默认深度迭代.gsp"
    );
    let file = GspFile::parse(data).expect("fixture parses");
    let scene = build_scene(&file);

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
    let data = include_bytes!(
        "../../../tests/fixtures/gsp/static/简单迭代/原象点初象携带线段默认深度3迭代.gsp"
    );
    let file = GspFile::parse(data).expect("fixture parses");
    let scene = build_scene(&file);

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
    let data = include_bytes!(
        "../../../tests/fixtures/gsp/static/简单迭代/原象点初象携带多边形双映射深度4迭代.gsp"
    );
    let file = GspFile::parse(data).expect("fixture parses");
    let scene = build_scene(&file);

    assert_eq!(
        scene.polygons.len(),
        15,
        "expected triangular lattice of seed polygon plus carried copies"
    );
    assert_eq!(
        scene.lines.len(),
        45,
        "expected triangular lattice of carried line copies"
    );
    assert!(
        scene
            .parameters
            .iter()
            .any(|parameter| parameter.name == "n")
    );
    assert_eq!(scene.line_iterations.len(), 3);
    assert!(scene.line_iterations.iter().all(|family| {
        family.parameter_name.as_deref() == Some("n")
            && family.depth == 4
            && family.secondary_dx.is_some()
            && family.secondary_dy.is_some()
            && (family.dy + 37.79527559055118).abs() < 1e-6
    }));
    assert_eq!(scene.polygon_iterations.len(), 1);
    assert!(scene.polygon_iterations.iter().any(|family| {
        family.parameter_name.as_deref() == Some("n")
            && family.depth == 4
            && family.vertex_indices == vec![0, 2, 1]
            && family.secondary_dx.is_some()
            && family.secondary_dy.is_some()
            && family.dx.abs() < 1e-6
            && (family.dy + 37.79527559055118).abs() < 1e-6
    }));
    assert_eq!(
        scene.points.len(),
        3,
        "expected base point plus two mapped vertices"
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
    let data =
        include_bytes!("../../../tests/fixtures/gsp/static/简单迭代/原象点初象点默认深度3迭代.gsp");
    let file = GspFile::parse(data).expect("fixture parses");
    let scene = build_scene(&file);

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
fn preserves_scaled_point_and_single_parameter_label_in_scale_gsp() {
    let data = include_bytes!("../../../tests/fixtures/gsp/static/scale.gsp");
    let file = GspFile::parse(data).expect("fixture parses");
    let scene = build_scene(&file);

    assert_eq!(
        scene.circles.len(),
        2,
        "expected original and scaled circle"
    );
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
fn preserves_reflection_point_circle_and_polygon_gsp() {
    let data = include_bytes!("../../../tests/fixtures/gsp/static/reflection.gsp");
    let file = GspFile::parse(data).expect("fixture parses");
    let scene = build_scene(&file);

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
        Some(crate::runtime::scene::ShapeBinding::ReflectCircle { .. })
    )));
    assert!(scene.polygons.iter().any(|polygon| matches!(
        polygon.binding,
        Some(crate::runtime::scene::ShapeBinding::ReflectPolygon { .. })
    )));
}

#[test]
fn preserves_translation_and_right_angle_rotation_in_transform_fixture() {
    let data = include_bytes!("../../../tests/fixtures/gsp/static/平移旋转缩放轴对称.gsp");
    let file = GspFile::parse(data).expect("fixture parses");
    let scene = build_scene(&file);

    assert!(
        scene
            .points
            .iter()
            .any(|point| matches!(point.binding, Some(ScenePointBinding::Translate { .. })))
    );
    assert!(scene.polygons.iter().any(|polygon| matches!(
        polygon.binding,
        Some(crate::runtime::scene::ShapeBinding::TranslatePolygon { .. })
    )));
    assert!(scene.polygons.iter().any(|polygon| matches!(
        polygon.binding,
        Some(crate::runtime::scene::ShapeBinding::RotatePolygon { angle_degrees, .. })
            if (angle_degrees - 90.0).abs() < 1e-3
    )));
}

#[test]
fn preserves_reflect_then_translate_in_translation_fixture() {
    let data = include_bytes!("../../../tests/fixtures/gsp/static/平移.gsp");
    let file = GspFile::parse(data).expect("fixture parses");
    let scene = build_scene(&file);

    let reflected_points = scene
        .points
        .iter()
        .enumerate()
        .filter_map(|(index, point)| match point.binding {
            Some(ScenePointBinding::Reflect { .. }) => Some(index),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(reflected_points.len(), 2, "expected reflected A' and B'");

    assert!(scene.polygons.iter().any(|polygon| matches!(
        polygon.binding,
        Some(crate::runtime::scene::ShapeBinding::TranslatePolygon {
            vector_start_index,
            vector_end_index,
            ..
        }) if reflected_points.contains(&vector_start_index)
            && reflected_points.contains(&vector_end_index)
    )));
}

#[test]
fn preserves_point_label_in_point_label_gsp() {
    let data = include_bytes!("../../../tests/fixtures/gsp/static/point_label.gsp");
    let file = GspFile::parse(data).expect("fixture parses");
    let scene = build_scene(&file);

    assert!(
        scene.labels.iter().any(|label| label.text == "A"),
        "expected point label A, got {:?}",
        scene
            .labels
            .iter()
            .map(|label| &label.text)
            .collect::<Vec<_>>()
    );
}

#[test]
fn preserves_point_and_segment_labels_in_segment_label_gsp() {
    let data = include_bytes!("../../../tests/fixtures/gsp/static/segment_label.gsp");
    let file = GspFile::parse(data).expect("fixture parses");
    let scene = build_scene(&file);

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
        texts.contains(&"j"),
        "expected segment label j, got {texts:?}"
    );
}

#[test]
fn keeps_control_labels_in_non_graph_sample() {
    let data = include_bytes!("../../../../Samples/个人专栏/潘建平作品/加油潘建平老师.gsp");
    let file = GspFile::parse(data).expect("fixture parses");
    let scene = build_scene(&file);

    assert!(
        scene.labels.iter().any(|label| label.text.contains("单价")),
        "expected UI text label from rich text payload, got {:?}",
        scene
            .labels
            .iter()
            .map(|label| label.text.as_str())
            .collect::<Vec<_>>()
    );
}
