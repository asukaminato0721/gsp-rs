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
fn preserves_multiline_text_labels() {
    let data = include_bytes!("../../../tests/fixtures/gsp/多行文本.gsp");
    let file = GspFile::parse(data).expect("fixture parses");
    let scene = build_scene(&file);

    assert_eq!(scene.labels.len(), 1);
    assert_eq!(
        scene.labels[0].text,
        "线段中垂线\n垂线\n平行线\n直角三角形\n点的轨迹\n圆上的弧\n过三点的弧"
    );
}

#[test]
fn preserves_hot_text_actions_in_rich_text_gsp() {
    let data = include_bytes!("../../../tests/fixtures/gsp/热文本.gsp");
    let file = GspFile::parse(data).expect("fixture parses");
    let scene = build_scene(&file);

    let rich_label = scene
        .labels
        .iter()
        .find(|label| label.text.contains("BAC"))
        .expect("expected hot text label");
    assert_eq!(rich_label.text, "在ACB中，CA=AB，BAC=CBA");
    assert_eq!(
        rich_label
            .hotspots
            .iter()
            .map(|hotspot| hotspot.text.as_str())
            .collect::<Vec<_>>(),
        vec!["ACB", "CA", "AB", "BAC", "CBA"]
    );
    assert!(matches!(
        rich_label.hotspots[0].action,
        crate::runtime::scene::TextLabelHotspotAction::Polygon { .. }
    ));
    assert!(matches!(
        rich_label.hotspots[1].action,
        crate::runtime::scene::TextLabelHotspotAction::Segment { .. }
    ));
    assert!(matches!(
        rich_label.hotspots[2].action,
        crate::runtime::scene::TextLabelHotspotAction::Segment { .. }
    ));
    assert!(matches!(
        rich_label.hotspots[3].action,
        crate::runtime::scene::TextLabelHotspotAction::AngleMarker { .. }
    ));
    assert!(matches!(
        rich_label.hotspots[4].action,
        crate::runtime::scene::TextLabelHotspotAction::AngleMarker { .. }
    ));
    assert_eq!(scene.buttons.len(), 1, "expected linked action button");
    assert_eq!(scene.buttons[0].text, "隐藏三角形 ACB");
}

#[test]
fn preserves_info_gsp_button_and_hidden_point() {
    let data = include_bytes!("../../../tests/fixtures/gsp/info.gsp");
    let file = GspFile::parse(data).expect("fixture parses");
    let scene = build_scene(&file);

    assert_eq!(scene.points.len(), 1);
    assert!(!scene.points[0].visible, "point should start hidden");
    assert_eq!(scene.labels.len(), 1, "expected linked text label");
    assert_eq!(scene.labels[0].text, "显示点");
    assert_eq!(scene.labels[0].hotspots.len(), 1);
    assert!(matches!(
        scene.labels[0].hotspots[0].action,
        crate::runtime::scene::TextLabelHotspotAction::Button { button_index: 0 }
    ));
    assert_eq!(scene.buttons.len(), 1);
    assert_eq!(scene.buttons[0].text, "显示点");
    match &scene.buttons[0].action {
        crate::runtime::scene::ButtonAction::ShowHideVisibility {
            point_indices,
            line_indices,
            circle_indices,
            polygon_indices,
        } => {
            assert_eq!(point_indices, &vec![0]);
            assert!(line_indices.is_empty());
            assert!(circle_indices.is_empty());
            assert!(polygon_indices.is_empty());
        }
        other => panic!("unexpected button action: {other:?}"),
    }
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
fn preserves_perpendicular_gsp() {
    let data = include_bytes!("../../../tests/fixtures/gsp/static/perpendicular.gsp");
    let file = GspFile::parse(data).expect("fixture parses");
    let scene = build_scene(&file);

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
    let data = include_bytes!("../../../tests/fixtures/gsp/parallel.gsp");
    let file = GspFile::parse(data).expect("fixture parses");
    let scene = build_scene(&file);

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
fn preserves_perpendicular_bisector_midpoint_gsp() {
    let data = include_bytes!("../../../tests/fixtures/gsp/中垂线.gsp");
    let file = GspFile::parse(data).expect("fixture parses");
    let scene = build_scene(&file);

    assert_eq!(
        scene.lines.len(),
        2,
        "expected source segment and perpendicular bisector"
    );
    assert_eq!(
        scene.points.len(),
        3,
        "expected endpoints plus visible midpoint"
    );

    let midpoint_index = scene
        .points
        .iter()
        .enumerate()
        .find_map(|(index, point)| match point.constraint {
            ScenePointConstraint::OnSegment {
                start_index,
                end_index,
                t,
            } if start_index == 0 && end_index == 1 && (t - 0.5).abs() < 1e-9 => Some(index),
            _ => None,
        })
        .expect("expected midpoint point on the source segment");
    assert!(
        scene.points[midpoint_index].visible,
        "expected midpoint to be rendered as a visible point"
    );

    let perpendicular = scene
        .lines
        .iter()
        .find_map(|line| match line.binding {
            Some(LineBinding::PerpendicularLine {
                through_index,
                line_start_index,
                line_end_index,
                ..
            }) => Some((through_index, line_start_index, line_end_index)),
            _ => None,
        })
        .expect("expected synthesized perpendicular line");

    assert_eq!(perpendicular.0, midpoint_index);
    assert_eq!(perpendicular.1, Some(0));
    assert_eq!(perpendicular.2, Some(1));
}

#[test]
fn preserves_nested_perpendicular_parallel_bindings_in_pert_vert_gsp() {
    let data = include_bytes!("../../../tests/fixtures/gsp/pert_vert.gsp");
    let file = GspFile::parse(data).expect("fixture parses");
    let scene = build_scene(&file);

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
fn preserves_nested_line_indices_in_basic_shapes_gsp() {
    let data = include_bytes!("../../../tests/fixtures/gsp/基本图形.gsp");
    let file = GspFile::parse(data).expect("fixture parses");
    let scene = build_scene(&file);

    let base_segment_index = scene
        .lines
        .iter()
        .position(|line| {
            matches!(
                line.binding,
                Some(LineBinding::Segment {
                    start_index: 8,
                    end_index: 0,
                })
            )
        })
        .expect("expected base segment for midpoint construction");

    let midpoint_perpendicular_index = scene
        .lines
        .iter()
        .position(|line| {
            matches!(
                line.binding,
                Some(LineBinding::PerpendicularLine {
                    through_index: 9,
                    line_start_index: Some(8),
                    line_end_index: Some(0),
                    line_index: Some(_),
                })
            )
        })
        .expect("expected midpoint perpendicular line");

    assert_eq!(midpoint_perpendicular_index, base_segment_index + 1);
    assert!(matches!(
        scene.lines[midpoint_perpendicular_index].binding,
        Some(LineBinding::PerpendicularLine {
            line_index: Some(line_index),
            ..
        }) if line_index == base_segment_index
    ));

    let nested_host_index = midpoint_perpendicular_index;
    assert!(scene.lines.iter().any(|line| {
        matches!(
            line.binding,
            Some(LineBinding::PerpendicularLine {
                through_index: 1,
                line_start_index: Some(9),
                line_end_index: None,
                line_index: Some(line_index),
            }) if line_index == nested_host_index
        )
    }));
    assert!(scene.lines.iter().any(|line| {
        matches!(
            line.binding,
            Some(LineBinding::ParallelLine {
                through_index: 1,
                line_start_index: Some(9),
                line_end_index: None,
                line_index: Some(line_index),
            }) if line_index == nested_host_index
        )
    }));
}

#[test]
fn preserves_bisector_gsp() {
    let data = include_bytes!("../../../tests/fixtures/gsp/static/bisector.gsp");
    let file = GspFile::parse(data).expect("fixture parses");
    let scene = build_scene(&file);

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
    let data = include_bytes!("../../../tests/fixtures/gsp/static/three_point_arc.gsp");
    let file = GspFile::parse(data).expect("fixture parses");
    let scene = build_scene(&file);

    assert_eq!(scene.points.len(), 3, "expected three defining points");
    assert_eq!(scene.arcs.len(), 1, "expected one three-point arc");
    assert!(
        scene.lines.is_empty(),
        "expected arc fixture not to fall back to a line"
    );

    let arc = &scene.arcs[0];
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
fn preserves_three_point_arc_point_gsp() {
    let data = include_bytes!("../../../tests/fixtures/gsp/static/three_point_arc_point.gsp");
    let file = GspFile::parse(data).expect("fixture parses");
    let scene = build_scene(&file);

    assert_eq!(scene.arcs.len(), 1, "expected one three-point arc");
    assert_eq!(
        scene.points.len(),
        4,
        "expected three defining points and one constrained point"
    );
    assert!(scene.points.iter().any(|point| matches!(
        point.constraint,
        ScenePointConstraint::OnArc {
            start_index: 0,
            mid_index: 1,
            end_index: 2,
            t,
        } if (t - 0.201784919136623).abs() < 1e-9
    )));
}

#[test]
fn preserves_arc_on_circle_gsp() {
    let data = include_bytes!("../../../tests/fixtures/gsp/static/arc_on_circle.gsp");
    let file = GspFile::parse(data).expect("fixture parses");
    let scene = build_scene(&file);

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
    let data = include_bytes!("../../../tests/fixtures/gsp/point_on_arc1.gsp");
    let file = GspFile::parse(data).expect("fixture parses");
    let scene = build_scene(&file);

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
fn preserves_center_arc_and_point_on_arc_in_unimplemented_fixture() {
    let data = include_bytes!("../../../tests/fixtures/gsp/未实现1(1).gsp");
    let file = GspFile::parse(data).expect("fixture parses");
    let scene = build_scene(&file);

    assert_eq!(scene.circles.len(), 1, "expected one circle");
    assert_eq!(scene.arcs.len(), 1, "expected one center-based arc");
    assert_eq!(
        scene.points.len(),
        5,
        "expected base points plus constrained arc point"
    );
    assert!(scene.points.iter().any(|point| matches!(
        point.constraint,
        ScenePointConstraint::OnCircleArc {
            center_index: 0,
            start_index: 1,
            end_index: 3,
            ..
        }
    )));
}

#[test]
fn preserves_angle_sign_gsp() {
    let data = include_bytes!("../../../tests/fixtures/gsp/angle-sign.gsp");
    let file = GspFile::parse(data).expect("fixture parses");
    let scene = build_scene(&file);

    assert_eq!(
        scene.lines.len(),
        3,
        "expected angle rays plus synthesized angle marker"
    );
    assert_eq!(
        scene.points.len(),
        4,
        "expected anchor, ray endpoint, and marker points"
    );
    assert_eq!(scene.labels.len(), 1, "expected one point label");
    assert_eq!(scene.labels[0].text, "A");

    assert!(matches!(
        scene.points[0].constraint,
        ScenePointConstraint::Free
    ));
    assert!(matches!(
        scene.points[1].constraint,
        ScenePointConstraint::Free
    ));
    assert!(matches!(
        scene.points[2].binding,
        Some(ScenePointBinding::Rotate {
            source_index: 1,
            center_index: 0,
            angle_degrees,
            parameter_name: None,
        }) if (angle_degrees - 90.0).abs() < 1e-6
    ));
    assert!(
        !scene.points[2].visible,
        "expected intermediate rotated helper point to stay hidden"
    );
    assert!(matches!(
        scene.points[3].binding,
        Some(ScenePointBinding::Scale {
            source_index: 2,
            center_index: 0,
            factor,
        }) if (factor - 1.5).abs() < 1e-6
    ));
    assert!(
        scene.points[3].visible,
        "expected scaled endpoint to remain visible"
    );

    assert!(matches!(
        scene.lines[0].binding,
        Some(LineBinding::Segment {
            start_index: 0,
            end_index: 1,
        })
    ));
    assert!(matches!(
        scene.lines[1].binding,
        Some(LineBinding::Segment {
            start_index: 3,
            end_index: 0,
        })
    ));

    let marker = scene
        .lines
        .iter()
        .find(|line| {
            matches!(line.binding, Some(LineBinding::AngleMarker { .. })) && line.points.len() == 3
        })
        .expect("expected reactive angle marker polyline");

    let anchor = &scene.points[0].position;
    let base = &scene.points[1].position;
    let rotated = &scene.points[2].position;
    let scaled = &scene.points[3].position;

    let base_dx = base.x - anchor.x;
    let base_dy = base.y - anchor.y;
    let rotated_dx = rotated.x - anchor.x;
    let rotated_dy = rotated.y - anchor.y;
    assert!(
        (base_dx * rotated_dx + base_dy * rotated_dy).abs() < 1e-6,
        "expected rotated point to form a right angle with the base segment"
    );

    let rotated_len = (rotated_dx * rotated_dx + rotated_dy * rotated_dy).sqrt();
    let scaled_dx = scaled.x - anchor.x;
    let scaled_dy = scaled.y - anchor.y;
    let scaled_len = (scaled_dx * scaled_dx + scaled_dy * scaled_dy).sqrt();
    assert!(
        (scaled_len - rotated_len * 1.5).abs() < 1e-6,
        "expected scaled point to extend the rotated marker arm"
    );

    assert!(
        marker.points[0].x > anchor.x
            && marker.points[0].x < base.x
            && (marker.points[0].y - anchor.y).abs() < 1e-6,
        "expected marker to start partway along the horizontal ray"
    );
    assert!(
        (marker.points[2].x - anchor.x).abs() < 1e-6
            && marker.points[2].y < anchor.y
            && marker.points[2].y > rotated.y,
        "expected marker to end partway along the vertical ray"
    );
    assert!(
        (marker.points[1].x - marker.points[0].x).abs() < 1e-6
            && (marker.points[1].y - marker.points[2].y).abs() < 1e-6,
        "expected marker corner to form the missing square corner"
    );

    let marker_dx = marker.points[0].x - anchor.x;
    let marker_dy = anchor.y - marker.points[2].y;
    assert!(
        (marker_dx - marker_dy).abs() < 1e-6,
        "expected marker arms to use the same inset length"
    );
}

#[test]
fn preserves_point_hidden_gsp() {
    let data = include_bytes!("../../../tests/fixtures/gsp/static/point_hidden.gsp");
    let file = GspFile::parse(data).expect("fixture parses");
    let scene = build_scene(&file);

    assert_eq!(scene.points.len(), 1, "expected one point in the fixture");
    assert!(
        !scene.points[0].visible,
        "expected fixture point to inherit hidden state from source metadata"
    );
    assert!(scene.lines.is_empty());
    assert!(scene.labels.is_empty());
}

#[test]
fn preserves_circle_center_radius_gsp() {
    let data = include_bytes!("../../../tests/fixtures/gsp/circle_center_radius.gsp");
    let file = GspFile::parse(data).expect("fixture parses");
    let scene = build_scene(&file);

    assert_eq!(scene.circles.len(), 1, "expected one circle");
    assert_eq!(scene.lines.len(), 1, "expected one segment");
    assert_eq!(scene.points.len(), 3, "expected three visible points");

    let circle = &scene.circles[0];
    assert!((circle.center.x - 348.0).abs() < 1e-6);
    assert!((circle.center.y - 177.0).abs() < 1e-6);
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
fn preserves_midpoint_binding_and_trace_in_trace_gsp() {
    let data = include_bytes!("../../../tests/fixtures/gsp/trace.gsp");
    let file = GspFile::parse(data).expect("fixture parses");
    let scene = build_scene(&file);

    let midpoint_index = scene
        .points
        .iter()
        .enumerate()
        .find_map(|(index, point)| match (&point.constraint, &point.binding) {
            (
                ScenePointConstraint::OnSegment {
                    start_index,
                    end_index,
                    t,
                },
                Some(ScenePointBinding::Midpoint {
                    start_index: binding_start,
                    end_index: binding_end,
                }),
            ) if *start_index == 4
                && *end_index == 0
                && *binding_start == 4
                && *binding_end == 0
                && (*t - 0.5).abs() < 1e-9 =>
            {
                Some(index)
            }
            _ => None,
        })
        .expect("expected derived midpoint point");
    assert!(scene.points[midpoint_index].visible);

    assert!(
        scene.lines.iter().any(|line| {
            if line.points.len() < 100 {
                return false;
            }
            let first = line.points.first().expect("non-empty line");
            let last = line.points.last().expect("non-empty line");
            ((first.x - 846.5).abs() < 0.01
                && (first.y - 480.0).abs() < 0.01
                && (last.x - 766.0).abs() < 0.01
                && (last.y - 359.25).abs() < 0.01)
                || ((last.x - 846.5).abs() < 0.01
                    && (last.y - 480.0).abs() < 0.01
                    && (first.x - 766.0).abs() < 0.01
                    && (first.y - 359.25).abs() < 0.01)
        }),
        "expected sampled midpoint trace line"
    );
}

#[test]
fn preserves_trace1_graph_geometry_and_traces() {
    let data = include_bytes!("../../../tests/fixtures/gsp/trace1.gsp");
    let file = GspFile::parse(data).expect("fixture parses");
    let scene = build_scene(&file);

    assert!(scene.graph_mode, "expected graph scene");
    assert_eq!(
        scene.points.len(),
        6,
        "expected origin plus derived intersections"
    );
    assert_eq!(scene.circles.len(), 2, "expected two circles");
    assert_eq!(scene.polygons.len(), 2, "expected two filled polygons");
    assert!(
        scene.lines.len() >= 5,
        "expected measurement, construction, and trace lines"
    );

    let has_point = |x: f64, y: f64| {
        scene
            .points
            .iter()
            .any(|point| (point.position.x - x).abs() < 1e-6 && (point.position.y - y).abs() < 1e-6)
    };
    assert!(has_point(0.0, 0.0), "expected origin point");
    assert!(has_point(0.0, 1.0), "expected arc point");
    assert!(has_point(0.0, 2.0), "expected translated top point");
    assert!(has_point(-1.0, 1.0), "expected left trace endpoint");
    assert!(has_point(1.0, 1.0), "expected right trace endpoint");

    assert!(
        scene.lines.iter().any(|line| {
            line.points
                .first()
                .is_some_and(|point| (point.x - 0.0).abs() < 1e-6 && (point.y - 1.0).abs() < 1e-6)
                && line.points.last().is_some_and(|point| {
                    (point.x + 1.0).abs() < 1e-6 && (point.y - 1.0).abs() < 1e-6
                })
        }),
        "expected left horizontal trace"
    );
    assert!(
        scene.lines.iter().any(|line| {
            line.points
                .first()
                .is_some_and(|point| (point.x - 0.0).abs() < 1e-6 && (point.y - 1.0).abs() < 1e-6)
                && line.points.last().is_some_and(|point| {
                    (point.x - 1.0).abs() < 1e-6 && (point.y - 1.0).abs() < 1e-6
                })
        }),
        "expected right horizontal trace"
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
fn preserves_linear_intersection_points_in_insection_fixtures() {
    for (name, data) in [
        (
            "segment",
            include_bytes!("../../../tests/fixtures/gsp/insection/segment_insection.gsp")
                .as_slice(),
        ),
        (
            "line",
            include_bytes!("../../../tests/fixtures/gsp/insection/line_insection.gsp").as_slice(),
        ),
        (
            "ray",
            include_bytes!("../../../tests/fixtures/gsp/insection/ray_insection.gsp").as_slice(),
        ),
    ] {
        let file = GspFile::parse(data).expect("fixture parses");
        let scene = build_scene(&file);

        assert_eq!(
            scene.points.len(),
            5,
            "expected derived intersection point for {name}"
        );
        assert!(scene.points.iter().any(|point| {
            matches!(
                point.constraint,
                ScenePointConstraint::LineIntersection { .. }
            )
        }));
        assert!(
            scene.points.iter().any(|point| {
                (point.position.x - 416.3160761196899).abs() < 1e-6
                    && (point.position.y - 321.2222079835971).abs() < 1e-6
            }),
            "expected derived intersection coordinates for {name}"
        );
    }
}

#[test]
fn preserves_circle_circle_intersection_points() {
    let data = include_bytes!("../../../tests/fixtures/gsp/insection/circle_circle_insection.gsp");
    let file = GspFile::parse(data).expect("fixture parses");
    let scene = build_scene(&file);

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
            && (point.position.y - 189.66291724683578).abs() < 1e-6
    }));
    assert!(scene.points.iter().any(|point| {
        (point.position.x - 445.71654184257966).abs() < 1e-6
            && (point.position.y - 470.02601183209464).abs() < 1e-6
    }));
}

#[test]
fn preserves_line_circle_intersection_points() {
    let data = include_bytes!("../../../tests/fixtures/gsp/insection/circle_insection.gsp");
    let file = GspFile::parse(data).expect("fixture parses");
    let scene = build_scene(&file);

    assert_eq!(
        scene.points.len(),
        5,
        "expected derived line-circle intersection"
    );
    assert!(scene.points.iter().any(|point| {
        matches!(
            point.constraint,
            ScenePointConstraint::LineCircleIntersection { .. }
        )
    }));
    assert!(scene.points.iter().any(|point| {
        (point.position.x - 167.5150597569313).abs() < 1e-6
            && (point.position.y - 204.5902707856141).abs() < 1e-6
    }));
}

#[test]
fn preserves_circle_y_intersection_points() {
    let data = include_bytes!("../../../tests/fixtures/gsp/circle_y_intersection.gsp");
    let file = GspFile::parse(data).expect("fixture parses");
    let scene = build_scene(&file);

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
                && (point.position.y - 1.0).abs() < 1e-6
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
        0,
        "expected polygon edges to render via polygons without duplicate carried segments"
    );
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
fn does_not_treat_triangle_point_labels_as_iteration_parameters() {
    let data = include_bytes!("../../../tests/fixtures/gsp/static/简单迭代/三角形.gsp");
    let file = GspFile::parse(data).expect("fixture parses");
    let scene = build_scene(&file);

    assert!(
        scene.parameters.is_empty(),
        "expected no editable parameters in triangle fixture"
    );
    assert_eq!(scene.line_iterations.len(), 3);
    assert!(
        scene
            .line_iterations
            .iter()
            .all(|family| family.affine_source_indices.is_some()
                && family.affine_target_handles.is_some())
    );
}

#[test]
fn preserves_midpoint_triangle_iteration_geometry() {
    let data = include_bytes!("../../../tests/fixtures/gsp/static/简单迭代/三角形.gsp");
    let file = GspFile::parse(data).expect("fixture parses");
    let scene = build_scene(&file);

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
    let data = include_bytes!("../../../tests/fixtures/gsp/static/简单迭代/迭代正多边形.gsp");
    let file = GspFile::parse(data).expect("fixture parses");
    let scene = build_scene(&file);

    assert_eq!(scene.parameters.len(), 1, "expected editable n parameter");
    assert_eq!(scene.parameters[0].name, "n");
    assert_eq!(scene.lines.len(), 5, "expected five polygon edges");
    assert_eq!(
        scene
            .lines
            .iter()
            .filter(|line| matches!(line.binding, Some(LineBinding::RotateEdge { .. })))
            .count(),
        5,
        "expected all polygon edges to stay in one dynamic rotate-edge family"
    );
    assert!(
        scene
            .lines
            .iter()
            .all(|line| matches!(line.binding, Some(LineBinding::RotateEdge { .. }))),
        "expected no static seed or carried duplicate segments"
    );
    assert!(
        scene.line_iterations.is_empty(),
        "expected no carried translation metadata for regular polygon iteration"
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

    assert_eq!(
        scene.lines.len(),
        5,
        "expected the reflection axis plus four translated polygon edges"
    );
    assert_eq!(
        scene
            .lines
            .iter()
            .filter(|line| matches!(line.binding, Some(LineBinding::TranslateLine { .. })))
            .count(),
        4,
        "expected translated edges for the translated quadrilateral"
    );
    assert_eq!(scene.parameters.len(), 1, "expected one angle parameter");
    assert_eq!(scene.parameters[0].name, "t₁");
    assert!((scene.parameters[0].value - 90.0).abs() < 1e-6);
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
        &polygon.binding,
        Some(crate::runtime::scene::ShapeBinding::RotatePolygon {
            angle_degrees,
            parameter_name,
            ..
        })
            if parameter_name.as_deref() == Some("t₁")
                && (angle_degrees - 90.0).abs() < 1e-3
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
fn preserves_translated_triangle_segments_in_congruent_triangle_fixture() {
    let data = include_bytes!("../../../tests/fixtures/gsp/两个三角形标记全等.gsp");
    let file = GspFile::parse(data).expect("fixture parses");
    let scene = build_scene(&file);

    assert_eq!(
        scene.lines.len(),
        16,
        "expected source and translated edges plus angle and segment congruence markers"
    );
    assert_eq!(
        scene
            .lines
            .iter()
            .filter(|line| matches!(line.binding, Some(LineBinding::TranslateLine { .. })))
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
            Some(LineBinding::TranslateLine {
                vector_start_index: 0,
                vector_end_index: 3,
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
