use super::test_support::fixture_scene;
use crate::runtime::scene::{LineBinding, ScenePointBinding, ScenePointConstraint};

#[test]
fn preserves_coordinate_trace_in_cood_trace_gsp() {
    let scene = fixture_scene(include_bytes!(
        "../../../tests/fixtures/gsp/static/cood-trace.gsp"
    ));

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
fn does_not_synthesize_graph_calibration_labels_in_cood_intersection_gsp() {
    let scene = fixture_scene(include_bytes!(
        "../../../tests/fixtures/gsp/insection/cood.gsp"
    ));

    assert!(
        scene
            .labels
            .iter()
            .all(|label| label.text != "37.80" && label.text != "37.8"),
        "expected no synthesized graph calibration labels, got {:?}",
        scene
            .labels
            .iter()
            .map(|label| label.text.as_str())
            .collect::<Vec<_>>()
    );
    assert!(
        scene.points.iter().any(|point| {
            point.visible
                && point.draggable
                && (point.position.x - 1.0).abs() < 1e-6
                && point.position.y.abs() < 1e-6
                && matches!(
                    point.binding,
                    Some(crate::runtime::scene::ScenePointBinding::GraphCalibration)
                )
        }),
        "expected visible interactive graph calibration point at (1,0), got {:?}",
        scene
            .points
            .iter()
            .map(|point| (
                point.position.x,
                point.position.y,
                point.visible,
                point.draggable,
                point.binding.as_ref().map(|binding| format!("{binding:?}"))
            ))
            .collect::<Vec<_>>()
    );
    assert!(
        scene.points.iter().any(|point| {
            !point.visible
                && point.draggable
                && point.position.x.abs() < 1e-6
                && (point.position.y - 1.0).abs() < 1e-6
                && matches!(
                    point.binding,
                    Some(crate::runtime::scene::ScenePointBinding::GraphCalibration)
                )
        }),
        "expected hidden interactive graph calibration point at (0,1), got {:?}",
        scene
            .points
            .iter()
            .map(|point| (
                point.position.x,
                point.position.y,
                point.visible,
                point.draggable,
                point.binding.as_ref().map(|binding| format!("{binding:?}"))
            ))
            .collect::<Vec<_>>()
    );
}

#[test]
fn preserves_coordinate_trace_intersection_in_cood_intersection_gsp() {
    let scene = fixture_scene(include_bytes!(
        "../../../tests/fixtures/gsp/insection/cood_intersection.gsp"
    ));

    assert!(scene.graph_mode, "expected graph scene");
    assert_eq!(
        scene.points.len(),
        7,
        "expected source, derived, intersection points, and the legacy parameter source point"
    );
    assert!(scene.lines.iter().any(|line| {
        matches!(
            line.binding,
            Some(crate::runtime::scene::LineBinding::CoordinateTrace { point_index, .. })
                if point_index == 4
        )
    }));
    assert!(scene.points.iter().any(|point| {
        matches!(
            point.binding,
            Some(crate::runtime::scene::ScenePointBinding::CoordinateSource {
                ref name,
                ..
            }) if name == "t₁"
        ) && (point.position.x - 4.021666666666667).abs() < 1e-6
            && (point.position.y - 4.021666666666667).abs() < 1e-6
    }));
    assert!(scene.points.iter().any(|point| {
        matches!(
            point.constraint,
            crate::runtime::scene::ScenePointConstraint::LineTraceIntersection {
                point_index,
                ..
            } if point_index == 4
        ) && point.position.x.abs() < 1e-6
            && point.position.y.abs() < 1e-6
    }));
}

#[test]
fn preserves_coordinate_trace_intersection_in_cood_intersection_y_gsp() {
    let scene = fixture_scene(include_bytes!(
        "../../../tests/fixtures/gsp/insection/cood_intersection_y.gsp"
    ));

    assert!(scene.graph_mode, "expected graph scene");
    assert_eq!(
        scene.points.len(),
        7,
        "expected source, derived, intersection points, and the legacy parameter source point"
    );
    assert!(scene.lines.iter().any(|line| {
        matches!(
            line.binding,
            Some(crate::runtime::scene::LineBinding::CoordinateTrace { point_index, .. })
                if point_index == 4
        )
    }));
    assert!(scene.points.iter().any(|point| {
        matches!(
            point.binding,
            Some(crate::runtime::scene::ScenePointBinding::CoordinateSource {
                ref name,
                axis: crate::runtime::scene::CoordinateAxis::Horizontal,
                ..
            }) if name == "t₁"
        ) && (point.position.x - -2.0427083333333336).abs() < 1e-6
            && (point.position.y - -2.8839583333333336).abs() < 1e-6
    }));
    assert!(scene.points.iter().any(|point| {
        matches!(
            point.constraint,
            crate::runtime::scene::ScenePointConstraint::LineTraceIntersection {
                point_index, ..
            } if point_index == 4
        ) && point.position.x.abs() < 1e-6
            && point.position.y.abs() < 1e-6
    }));
}

#[test]
fn preserves_coordinate_trace_intersection_in_cood_intersection_xy_gsp() {
    let scene = fixture_scene(include_bytes!(
        "../../../tests/fixtures/gsp/insection/cood_intersection_xy.gsp"
    ));

    assert!(scene.graph_mode, "expected graph scene");
    assert_eq!(
        scene.points.len(),
        7,
        "expected source, derived, intersection points, and the legacy parameter source point"
    );
    assert!(scene.lines.iter().any(|line| {
        matches!(
            line.binding,
            Some(crate::runtime::scene::LineBinding::CoordinateTrace { point_index, .. })
                if point_index == 4
        )
    }));
    assert!(scene.points.iter().any(|point| {
        matches!(
            point.binding,
            Some(crate::runtime::scene::ScenePointBinding::CoordinateSource2d {
                ref x_name,
                ref y_name,
                ..
            }) if x_name == "t₁" && y_name == "t₁"
        ) && (point.position.x - -0.5345833333333322).abs() < 1e-6
            && (point.position.y - 2.5345833333333334).abs() < 1e-6
    }));
    assert!(scene.points.iter().any(|point| {
        matches!(
            point.constraint,
            crate::runtime::scene::ScenePointConstraint::LineTraceIntersection {
                point_index, ..
            } if point_index == 4
        ) && point.position.x.abs() < 1e-6
            && (point.position.y - 3.069166666666897).abs() < 1e-6
    }));
    assert!(scene.points.iter().any(|point| {
        matches!(
            point.binding,
            Some(crate::runtime::scene::ScenePointBinding::Parameter { ref name })
                if name == "t₁"
        ) && !point.visible
    }));
}

#[test]
fn preserves_midpoint_binding_and_trace_in_trace_gsp() {
    let scene = fixture_scene(include_bytes!("../../../tests/fixtures/gsp/trace.gsp"));

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
                && (first.y - 504.0).abs() < 0.01
                && (last.x - 766.0).abs() < 0.01
                && (last.y - 383.25).abs() < 0.01)
                || ((last.x - 846.5).abs() < 0.01
                    && (last.y - 504.0).abs() < 0.01
                    && (first.x - 766.0).abs() < 0.01
                    && (first.y - 383.25).abs() < 0.01)
        }),
        "expected sampled midpoint trace line"
    );
}

#[test]
fn preserves_two_circle_intersection_inrm_fixture_interactivity() {
    let scene = fixture_scene(include_bytes!(
        "../../../tests/fixtures/未实现/(inRm)两圆之交.gsp"
    ));

    assert_eq!(scene.circles.len(), 4, "expected four source circles");
    assert_eq!(
        scene.polygons.len(),
        2,
        "expected the payload circular segments that make up the lens"
    );
    assert_eq!(
        scene.lines.len(),
        7,
        "expected five source helper lines plus two live circular-segment boundaries"
    );
    assert_eq!(
        scene.points.len(),
        14,
        "expected source points plus derived circle intersections"
    );
    assert_eq!(
        scene
            .circles
            .iter()
            .filter(|circle| matches!(
                circle.binding,
                Some(crate::runtime::scene::ShapeBinding::PointRadiusCircle { .. })
            ))
            .count(),
        4,
        "expected every payload circle to keep its live center/radius binding"
    );
    assert_eq!(
        scene
            .circles
            .iter()
            .filter(|circle| circle.fill_color.is_some())
            .count(),
        0,
        "expected payload helper circle interiors not to render as full-disk circle fills"
    );
    assert_eq!(
        scene
            .polygons
            .iter()
            .filter(|polygon| matches!(
                polygon.binding,
                Some(crate::runtime::scene::ShapeBinding::ArcBoundaryPolygon { .. })
            ))
            .count(),
        2,
        "expected both circular segments to stay interactive"
    );
    assert_eq!(
        scene
            .lines
            .iter()
            .filter(|line| matches!(line.binding, Some(LineBinding::Segment { .. })))
            .count(),
        2,
        "expected both payload segments to stay interactive"
    );
    assert_eq!(
        scene
            .lines
            .iter()
            .filter(|line| matches!(line.binding, Some(LineBinding::PerpendicularLine { .. })))
            .count(),
        2,
        "expected both payload perpendicular helpers to stay interactive"
    );
    assert_eq!(
        scene
            .lines
            .iter()
            .filter(|line| matches!(line.binding, Some(LineBinding::Line { .. })))
            .count(),
        1,
        "expected the payload baseline to stay interactive"
    );
    assert_eq!(
        scene
            .lines
            .iter()
            .filter(|line| matches!(line.binding, Some(LineBinding::ArcBoundary { .. })))
            .count(),
        2,
        "expected both payload circular-segment boundaries to stay interactive"
    );

    let circle_circle_points = scene
        .points
        .iter()
        .filter(|point| {
            matches!(
                point.constraint,
                ScenePointConstraint::CircleCircleIntersection { .. }
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        circle_circle_points.len(),
        2,
        "expected both circle-circle variants to stay exported"
    );
    assert!(circle_circle_points.iter().all(|point| {
        (point.position.x - 327.0).abs() < 1e-6 && (point.position.y - 275.0).abs() < 1e-6
    }));
    assert_eq!(
        scene
            .points
            .iter()
            .filter(|point| {
                matches!(
                    point.constraint,
                    ScenePointConstraint::LineCircleIntersection { .. }
                )
            })
            .count(),
        8,
        "expected all derived line-circle intersection helpers to stay live"
    );
}
