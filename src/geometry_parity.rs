use crate::format::PointRecord;
use crate::runtime::geometry::{
    Bounds, clip_line_to_bounds, clip_ray_to_bounds, lerp_point, point_on_circle_arc,
    point_on_three_point_arc, point_on_three_point_arc_complement, reflect_across_line,
    rotate_around, scale_around, three_point_arc_geometry,
};
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

pub const GEOMETRY_PARITY_VECTORS_FILE: &str = "geometry_parity_vectors.json";

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeometryParityVectors {
    version: u32,
    lerp_point: Vec<LerpCase>,
    rotate_around: Vec<RotateCase>,
    scale_around: Vec<ScaleCase>,
    reflect_across_line: Vec<ReflectCase>,
    clip_line_to_bounds: Vec<ClipCase>,
    clip_ray_to_bounds: Vec<ClipCase>,
    three_point_arc_geometry: Vec<ArcGeometryCase>,
    point_on_three_point_arc: Vec<ArcPointCase>,
    point_on_three_point_arc_complement: Vec<ArcPointCase>,
    point_on_circle_arc: Vec<CircleArcPointCase>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ParityPoint {
    x: f64,
    y: f64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ParityBounds {
    min_x: f64,
    max_x: f64,
    min_y: f64,
    max_y: f64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct LerpCase {
    name: &'static str,
    start: ParityPoint,
    end: ParityPoint,
    t: f64,
    expected: ParityPoint,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RotateCase {
    name: &'static str,
    point: ParityPoint,
    center: ParityPoint,
    radians: f64,
    expected: ParityPoint,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ScaleCase {
    name: &'static str,
    point: ParityPoint,
    center: ParityPoint,
    factor: f64,
    expected: ParityPoint,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ReflectCase {
    name: &'static str,
    point: ParityPoint,
    line_start: ParityPoint,
    line_end: ParityPoint,
    expected: Option<ParityPoint>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ClipCase {
    name: &'static str,
    start: ParityPoint,
    end: ParityPoint,
    bounds: ParityBounds,
    expected: Option<[ParityPoint; 2]>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ArcGeometryCase {
    name: &'static str,
    start: ParityPoint,
    mid: ParityPoint,
    end: ParityPoint,
    expected: Option<ArcGeometrySnapshot>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ArcGeometrySnapshot {
    center: ParityPoint,
    radius: f64,
    start_angle: f64,
    end_angle: f64,
    counterclockwise: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ArcPointCase {
    name: String,
    start: ParityPoint,
    mid: ParityPoint,
    end: ParityPoint,
    t: f64,
    expected: Option<ParityPoint>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CircleArcPointCase {
    name: String,
    center: ParityPoint,
    start: ParityPoint,
    end: ParityPoint,
    t: f64,
    expected: Option<ParityPoint>,
}

pub fn export_geometry_parity_vectors(output_dir: &Path) -> std::io::Result<PathBuf> {
    fs::create_dir_all(output_dir)?;
    let output_path = output_dir.join(GEOMETRY_PARITY_VECTORS_FILE);
    let vectors =
        serde_json::to_vec_pretty(&geometry_parity_vectors()).expect("geometry vectors serialize");
    fs::write(&output_path, vectors)?;
    Ok(output_path)
}

pub fn geometry_parity_vectors() -> GeometryParityVectors {
    let bounds = Bounds {
        min_x: -5.0,
        max_x: 5.0,
        min_y: -3.0,
        max_y: 3.0,
    };

    let arc_upper = (point(-1.0, 0.0), point(0.0, -1.0), point(1.0, 0.0));
    let arc_lower = (point(-1.0, 0.0), point(0.0, 1.0), point(1.0, 0.0));
    let arc_skew = (point(2.0, 1.0), point(4.0, -1.0), point(6.0, 1.0));
    let arc_degenerate = (point(0.0, 0.0), point(1.0, 1.0), point(2.0, 2.0));

    GeometryParityVectors {
        version: 1,
        lerp_point: vec![
            LerpCase {
                name: "midpoint-origin",
                start: json_point(&point(0.0, 0.0)),
                end: json_point(&point(10.0, -10.0)),
                t: 0.5,
                expected: json_point(&lerp_point(&point(0.0, 0.0), &point(10.0, -10.0), 0.5)),
            },
            LerpCase {
                name: "outside-segment-left",
                start: json_point(&point(-3.0, 7.0)),
                end: json_point(&point(9.0, 1.0)),
                t: -0.25,
                expected: json_point(&lerp_point(&point(-3.0, 7.0), &point(9.0, 1.0), -0.25)),
            },
            LerpCase {
                name: "outside-segment-right",
                start: json_point(&point(2.0, -4.0)),
                end: json_point(&point(8.0, 14.0)),
                t: 1.25,
                expected: json_point(&lerp_point(&point(2.0, -4.0), &point(8.0, 14.0), 1.25)),
            },
        ],
        rotate_around: vec![
            RotateCase {
                name: "quarter-turn-origin",
                point: json_point(&point(3.0, 1.0)),
                center: json_point(&point(0.0, 0.0)),
                radians: std::f64::consts::FRAC_PI_2,
                expected: json_point(&rotate_around(
                    &point(3.0, 1.0),
                    &point(0.0, 0.0),
                    std::f64::consts::FRAC_PI_2,
                )),
            },
            RotateCase {
                name: "half-turn-offset-center",
                point: json_point(&point(4.0, -2.0)),
                center: json_point(&point(1.0, 3.0)),
                radians: std::f64::consts::PI,
                expected: json_point(&rotate_around(
                    &point(4.0, -2.0),
                    &point(1.0, 3.0),
                    std::f64::consts::PI,
                )),
            },
            RotateCase {
                name: "negative-sixty",
                point: json_point(&point(-2.0, 5.0)),
                center: json_point(&point(2.5, -1.5)),
                radians: -std::f64::consts::PI / 3.0,
                expected: json_point(&rotate_around(
                    &point(-2.0, 5.0),
                    &point(2.5, -1.5),
                    -std::f64::consts::PI / 3.0,
                )),
            },
        ],
        scale_around: vec![
            ScaleCase {
                name: "double-from-origin",
                point: json_point(&point(2.0, -3.0)),
                center: json_point(&point(0.0, 0.0)),
                factor: 2.0,
                expected: json_point(&scale_around(&point(2.0, -3.0), &point(0.0, 0.0), 2.0)),
            },
            ScaleCase {
                name: "collapse-to-center",
                point: json_point(&point(8.0, 4.0)),
                center: json_point(&point(1.0, -2.0)),
                factor: 0.0,
                expected: json_point(&scale_around(&point(8.0, 4.0), &point(1.0, -2.0), 0.0)),
            },
            ScaleCase {
                name: "mirror-through-center",
                point: json_point(&point(-4.0, 7.0)),
                center: json_point(&point(3.0, -1.0)),
                factor: -1.0,
                expected: json_point(&scale_around(&point(-4.0, 7.0), &point(3.0, -1.0), -1.0)),
            },
        ],
        reflect_across_line: vec![
            ReflectCase {
                name: "horizontal-axis",
                point: json_point(&point(2.0, 3.0)),
                line_start: json_point(&point(-5.0, 0.0)),
                line_end: json_point(&point(5.0, 0.0)),
                expected: reflect_across_line(
                    &point(2.0, 3.0),
                    &point(-5.0, 0.0),
                    &point(5.0, 0.0),
                )
                .as_ref()
                .map(json_point),
            },
            ReflectCase {
                name: "vertical-offset-axis",
                point: json_point(&point(4.0, -2.0)),
                line_start: json_point(&point(1.0, -5.0)),
                line_end: json_point(&point(1.0, 5.0)),
                expected: reflect_across_line(
                    &point(4.0, -2.0),
                    &point(1.0, -5.0),
                    &point(1.0, 5.0),
                )
                .as_ref()
                .map(json_point),
            },
            ReflectCase {
                name: "diagonal-axis",
                point: json_point(&point(3.0, 1.0)),
                line_start: json_point(&point(0.0, 0.0)),
                line_end: json_point(&point(5.0, 5.0)),
                expected: reflect_across_line(&point(3.0, 1.0), &point(0.0, 0.0), &point(5.0, 5.0))
                    .as_ref()
                    .map(json_point),
            },
            ReflectCase {
                name: "degenerate-axis",
                point: json_point(&point(7.0, -4.0)),
                line_start: json_point(&point(1.0, 1.0)),
                line_end: json_point(&point(1.0, 1.0)),
                expected: reflect_across_line(
                    &point(7.0, -4.0),
                    &point(1.0, 1.0),
                    &point(1.0, 1.0),
                )
                .as_ref()
                .map(json_point),
            },
        ],
        clip_line_to_bounds: vec![
            ClipCase {
                name: "diagonal-across-box",
                start: json_point(&point(-10.0, -10.0)),
                end: json_point(&point(10.0, 10.0)),
                bounds: json_bounds(&bounds),
                expected: clip_line_to_bounds(&point(-10.0, -10.0), &point(10.0, 10.0), &bounds)
                    .map(json_segment),
            },
            ClipCase {
                name: "vertical-through-box",
                start: json_point(&point(1.0, -10.0)),
                end: json_point(&point(1.0, 10.0)),
                bounds: json_bounds(&bounds),
                expected: clip_line_to_bounds(&point(1.0, -10.0), &point(1.0, 10.0), &bounds)
                    .map(json_segment),
            },
            ClipCase {
                name: "outside-parallel-no-hit",
                start: json_point(&point(10.0, 0.0)),
                end: json_point(&point(10.0, 1.0)),
                bounds: json_bounds(&bounds),
                expected: clip_line_to_bounds(&point(10.0, 0.0), &point(10.0, 1.0), &bounds)
                    .map(json_segment),
            },
            ClipCase {
                name: "degenerate-line",
                start: json_point(&point(2.0, 2.0)),
                end: json_point(&point(2.0, 2.0)),
                bounds: json_bounds(&bounds),
                expected: clip_line_to_bounds(&point(2.0, 2.0), &point(2.0, 2.0), &bounds)
                    .map(json_segment),
            },
        ],
        clip_ray_to_bounds: vec![
            ClipCase {
                name: "inside-to-right",
                start: json_point(&point(0.0, 0.0)),
                end: json_point(&point(1.0, 0.0)),
                bounds: json_bounds(&bounds),
                expected: clip_ray_to_bounds(&point(0.0, 0.0), &point(1.0, 0.0), &bounds)
                    .map(json_segment),
            },
            ClipCase {
                name: "outside-left-through-box",
                start: json_point(&point(-10.0, 0.0)),
                end: json_point(&point(-9.0, 0.0)),
                bounds: json_bounds(&bounds),
                expected: clip_ray_to_bounds(&point(-10.0, 0.0), &point(-9.0, 0.0), &bounds)
                    .map(json_segment),
            },
            ClipCase {
                name: "outside-right-away-from-box",
                start: json_point(&point(10.0, 0.0)),
                end: json_point(&point(11.0, 0.0)),
                bounds: json_bounds(&bounds),
                expected: clip_ray_to_bounds(&point(10.0, 0.0), &point(11.0, 0.0), &bounds)
                    .map(json_segment),
            },
            ClipCase {
                name: "degenerate-ray",
                start: json_point(&point(2.0, -1.0)),
                end: json_point(&point(2.0, -1.0)),
                bounds: json_bounds(&bounds),
                expected: clip_ray_to_bounds(&point(2.0, -1.0), &point(2.0, -1.0), &bounds)
                    .map(json_segment),
            },
        ],
        three_point_arc_geometry: vec![
            arc_geometry_case("upper-semicircle", &arc_upper.0, &arc_upper.1, &arc_upper.2),
            arc_geometry_case("lower-semicircle", &arc_lower.0, &arc_lower.1, &arc_lower.2),
            arc_geometry_case("skew-arc", &arc_skew.0, &arc_skew.1, &arc_skew.2),
            arc_geometry_case(
                "degenerate-collinear",
                &arc_degenerate.0,
                &arc_degenerate.1,
                &arc_degenerate.2,
            ),
        ],
        point_on_three_point_arc: arc_point_cases(
            "arc",
            [
                ("upper-semicircle", &arc_upper),
                ("lower-semicircle", &arc_lower),
                ("skew-arc", &arc_skew),
                ("degenerate-collinear", &arc_degenerate),
            ],
            false,
        ),
        point_on_three_point_arc_complement: arc_point_cases(
            "arc-complement",
            [
                ("upper-semicircle", &arc_upper),
                ("lower-semicircle", &arc_lower),
                ("skew-arc", &arc_skew),
                ("degenerate-collinear", &arc_degenerate),
            ],
            true,
        ),
        point_on_circle_arc: circle_arc_point_cases(),
    }
}

fn arc_point_cases(
    prefix: &str,
    arcs: [(&'static str, &(PointRecord, PointRecord, PointRecord)); 4],
    complement: bool,
) -> Vec<ArcPointCase> {
    let ts = [0.0, 0.25, 0.5, 0.75, 1.0];
    let mut cases = Vec::new();
    for (name, (start, mid, end)) in arcs {
        for t in ts {
            let expected = if complement {
                point_on_three_point_arc_complement(start, mid, end, t)
            } else {
                point_on_three_point_arc(start, mid, end, t)
            };
            cases.push(ArcPointCase {
                name: format!("{prefix}:{name}:t={t:.2}"),
                start: json_point(start),
                mid: json_point(mid),
                end: json_point(end),
                t,
                expected: expected.as_ref().map(json_point),
            });
        }
    }
    cases
}

fn circle_arc_point_cases() -> Vec<CircleArcPointCase> {
    let cases = [
        (
            "quarter-turn",
            point(0.0, 0.0),
            point(3.0, 0.0),
            point(0.0, -3.0),
        ),
        (
            "three-quarter-turn",
            point(2.0, 1.0),
            point(5.0, 1.0),
            point(2.0, 4.0),
        ),
        (
            "degenerate-radius",
            point(1.0, 1.0),
            point(1.0, 1.0),
            point(1.0, 1.0),
        ),
    ];
    let ts = [0.0, 0.25, 0.5, 0.75, 1.0];
    let mut output = Vec::new();
    for (name, center, start, end) in cases {
        for t in ts {
            output.push(CircleArcPointCase {
                name: format!("circle-arc:{name}:t={t:.2}"),
                center: json_point(&center),
                start: json_point(&start),
                end: json_point(&end),
                t,
                expected: point_on_circle_arc(&center, &start, &end, t)
                    .as_ref()
                    .map(json_point),
            });
        }
    }
    output
}

fn arc_geometry_case(
    name: &'static str,
    start: &PointRecord,
    mid: &PointRecord,
    end: &PointRecord,
) -> ArcGeometryCase {
    ArcGeometryCase {
        name,
        start: json_point(start),
        mid: json_point(mid),
        end: json_point(end),
        expected: three_point_arc_geometry(start, mid, end)
            .as_ref()
            .map(|geometry| ArcGeometrySnapshot {
                center: json_point(&geometry.center),
                radius: geometry.radius,
                start_angle: geometry.start_angle,
                end_angle: geometry.end_angle,
                counterclockwise: geometry.counterclockwise,
            }),
    }
}

fn point(x: f64, y: f64) -> PointRecord {
    PointRecord { x, y }
}

fn json_point(point: &PointRecord) -> ParityPoint {
    ParityPoint {
        x: point.x,
        y: point.y,
    }
}

fn json_bounds(bounds: &Bounds) -> ParityBounds {
    ParityBounds {
        min_x: bounds.min_x,
        max_x: bounds.max_x,
        min_y: bounds.min_y,
        max_y: bounds.max_y,
    }
}

fn json_segment(segment: [PointRecord; 2]) -> [ParityPoint; 2] {
    [json_point(&segment[0]), json_point(&segment[1])]
}
