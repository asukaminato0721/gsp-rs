use std::collections::BTreeMap;

use serde::Deserialize;

use crate::{
    AffineMatrix, Point, evaluate_expr, measured_rotation_radians, parse_expression_json,
    scale_by_three_point_ratio, three_point_arc_geometry,
};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ResolveCircularCenterInput {
    constraint: CircularConstraint,
    points: Vec<Point>,
    #[serde(default)]
    lines: Vec<Option<[Point; 2]>>,
    #[serde(default)]
    parameters: BTreeMap<String, f64>,
}

#[derive(Deserialize)]
#[serde(tag = "kind")]
enum CircularConstraint {
    #[serde(rename = "circle")]
    Circle {
        #[serde(rename = "centerIndex")]
        center_index: usize,
        #[serde(rename = "radiusIndex")]
        radius_index: usize,
    },
    #[serde(rename = "segment-radius-circle")]
    SegmentRadiusCircle {
        #[serde(rename = "centerIndex")]
        center_index: usize,
        #[serde(rename = "lineStartIndex")]
        line_start_index: usize,
        #[serde(rename = "lineEndIndex")]
        line_end_index: usize,
    },
    #[serde(rename = "parameter-radius-circle")]
    ParameterRadiusCircle {
        #[serde(rename = "centerIndex")]
        center_index: usize,
        #[serde(rename = "parameterName")]
        parameter_name: String,
        #[serde(rename = "parameterValue")]
        parameter_value: f64,
        #[serde(rename = "rawPerUnit")]
        raw_per_unit: f64,
    },
    #[serde(rename = "expression-radius-circle")]
    ExpressionRadiusCircle {
        #[serde(rename = "centerIndex")]
        center_index: usize,
        expr: serde_json::Value,
        #[serde(rename = "initialValue")]
        initial_value: f64,
    },
    #[serde(rename = "matrix-apply")]
    MatrixApply {
        source: Box<CircularConstraint>,
        #[serde(rename = "matrixApply")]
        matrix_apply: Vec<GeometryTransform>,
    },
    #[serde(rename = "circle-arc")]
    CircleArc {
        #[serde(rename = "centerIndex")]
        center_index: usize,
        #[serde(rename = "startIndex")]
        start_index: usize,
        #[serde(rename = "endIndex")]
        end_index: usize,
    },
    #[serde(rename = "three-point-arc")]
    ThreePointArc {
        #[serde(rename = "startIndex")]
        start_index: usize,
        #[serde(rename = "midIndex")]
        mid_index: usize,
        #[serde(rename = "endIndex")]
        end_index: usize,
    },
}

#[derive(Deserialize)]
#[serde(tag = "kind")]
enum GeometryTransform {
    #[serde(rename = "translate")]
    Translate {
        #[serde(rename = "vectorStartIndex")]
        vector_start_index: usize,
        #[serde(rename = "vectorEndIndex")]
        vector_end_index: usize,
    },
    #[serde(rename = "translate-delta")]
    TranslateDelta { dx: f64, dy: f64 },
    #[serde(rename = "rotate")]
    Rotate {
        #[serde(rename = "centerIndex")]
        center_index: usize,
        #[serde(rename = "angleDegrees")]
        angle_degrees: f64,
        #[serde(rename = "parameterName")]
        parameter_name: Option<String>,
        #[serde(rename = "angleExpr")]
        angle_expr: Option<serde_json::Value>,
        #[serde(rename = "angleStartIndex")]
        angle_start_index: Option<usize>,
        #[serde(rename = "angleVertexIndex")]
        angle_vertex_index: Option<usize>,
        #[serde(rename = "angleEndIndex")]
        angle_end_index: Option<usize>,
    },
    #[serde(rename = "scale")]
    Scale {
        #[serde(rename = "centerIndex")]
        center_index: usize,
        factor: f64,
    },
    #[serde(rename = "scale-by-ratio")]
    ScaleByRatio {
        #[serde(rename = "centerIndex")]
        center_index: usize,
        #[serde(rename = "ratioOriginIndex")]
        ratio_origin_index: usize,
        #[serde(rename = "ratioDenominatorIndex")]
        ratio_denominator_index: usize,
        #[serde(rename = "ratioNumeratorIndex")]
        ratio_numerator_index: usize,
        signed: bool,
        #[serde(rename = "clampToUnit")]
        clamp_to_unit: bool,
    },
    #[serde(rename = "reflect")]
    Reflect {
        #[serde(rename = "lineStartIndex")]
        line_start_index: Option<usize>,
        #[serde(rename = "lineEndIndex")]
        line_end_index: Option<usize>,
        #[serde(rename = "lineIndex")]
        line_index: Option<usize>,
    },
    #[serde(rename = "rotate-source-point")]
    RotateSourcePoint {
        #[serde(rename = "sourcePointIndex")]
        source_point_index: usize,
        #[serde(rename = "angleDegrees")]
        angle_degrees: f64,
    },
    #[serde(rename = "translate-source-point")]
    TranslateSourcePoint {
        #[serde(rename = "sourcePointIndex")]
        source_point_index: usize,
        #[serde(rename = "targetIndex")]
        target_index: usize,
    },
}

pub fn resolve_circular_constraint_center_json(
    bytes: &[u8],
) -> Result<Option<Point>, serde_json::Error> {
    let input = serde_json::from_slice::<ResolveCircularCenterInput>(bytes)?;
    Ok(CircularCenterResolver {
        points: &input.points,
        lines: &input.lines,
        parameters: &input.parameters,
    }
    .resolve(&input.constraint))
}

struct CircularCenterResolver<'a> {
    points: &'a [Point],
    lines: &'a [Option<[Point; 2]>],
    parameters: &'a BTreeMap<String, f64>,
}

impl CircularCenterResolver<'_> {
    fn point(&self, index: usize) -> Option<Point> {
        self.points.get(index).copied().filter(finite_point)
    }

    fn resolve(&self, constraint: &CircularConstraint) -> Option<Point> {
        match constraint {
            CircularConstraint::Circle {
                center_index,
                radius_index,
            } => {
                self.point(*radius_index)?;
                self.point(*center_index)
            }
            CircularConstraint::SegmentRadiusCircle {
                center_index,
                line_start_index,
                line_end_index,
            } => {
                self.point(*line_start_index)?;
                self.point(*line_end_index)?;
                self.point(*center_index)
            }
            CircularConstraint::ParameterRadiusCircle {
                center_index,
                parameter_name,
                parameter_value,
                raw_per_unit,
            } => {
                let radius = self
                    .parameters
                    .get(parameter_name)
                    .copied()
                    .unwrap_or(*parameter_value)
                    .abs()
                    * raw_per_unit;
                radius
                    .is_finite()
                    .then(|| self.point(*center_index))
                    .flatten()
            }
            CircularConstraint::ExpressionRadiusCircle {
                center_index,
                expr,
                initial_value,
            } => {
                let value = self.evaluate(expr).unwrap_or(*initial_value);
                value
                    .is_finite()
                    .then(|| self.point(*center_index))
                    .flatten()
            }
            CircularConstraint::MatrixApply {
                source,
                matrix_apply,
            } => matrix_apply
                .iter()
                .try_fold(self.resolve(source)?, |center, transform| {
                    self.transform_matrix(transform, center)
                        .map(|matrix| matrix.apply(center))
                })
                .and_then(FinitePoint::finite),
            CircularConstraint::CircleArc {
                center_index,
                start_index,
                end_index,
            } => {
                self.point(*start_index)?;
                self.point(*end_index)?;
                self.point(*center_index)
            }
            CircularConstraint::ThreePointArc {
                start_index,
                mid_index,
                end_index,
            } => three_point_arc_geometry(
                self.point(*start_index)?,
                self.point(*mid_index)?,
                self.point(*end_index)?,
            )
            .map(|geometry| geometry.center),
        }
    }

    fn transform_matrix(
        &self,
        transform: &GeometryTransform,
        source_center: Point,
    ) -> Option<AffineMatrix> {
        Some(match transform {
            GeometryTransform::Translate {
                vector_start_index,
                vector_end_index,
            } => {
                let start = self.point(*vector_start_index)?;
                let end = self.point(*vector_end_index)?;
                AffineMatrix::translation(end.x - start.x, end.y - start.y)
            }
            GeometryTransform::TranslateDelta { dx, dy } => AffineMatrix::translation(*dx, *dy),
            GeometryTransform::Rotate {
                center_index,
                angle_degrees,
                parameter_name,
                angle_expr,
                angle_start_index,
                angle_vertex_index,
                angle_end_index,
            } => {
                let degrees = if let (Some(start), Some(vertex), Some(end)) =
                    (*angle_start_index, *angle_vertex_index, *angle_end_index)
                {
                    measured_rotation_radians(
                        self.point(start)?,
                        self.point(vertex)?,
                        self.point(end)?,
                    )?
                    .to_degrees()
                } else if let Some(expr) = angle_expr {
                    self.evaluate(expr)?
                } else if let Some(name) = parameter_name {
                    self.parameters.get(name).copied()?
                } else {
                    *angle_degrees
                };
                AffineMatrix::rotation(self.point(*center_index)?, degrees.to_radians())
            }
            GeometryTransform::Scale {
                center_index,
                factor,
            } => AffineMatrix::scale(self.point(*center_index)?, *factor),
            GeometryTransform::ScaleByRatio {
                center_index,
                ratio_origin_index,
                ratio_denominator_index,
                ratio_numerator_index,
                signed,
                clamp_to_unit,
            } => {
                let center = self.point(*center_index)?;
                let probe = Point {
                    x: center.x + 1.0,
                    y: center.y,
                };
                let scaled = scale_by_three_point_ratio(
                    probe,
                    center,
                    self.point(*ratio_origin_index)?,
                    self.point(*ratio_denominator_index)?,
                    self.point(*ratio_numerator_index)?,
                    *signed,
                    *clamp_to_unit,
                )?;
                AffineMatrix::scale(center, scaled.x - center.x)
            }
            GeometryTransform::Reflect {
                line_start_index,
                line_end_index,
                line_index,
            } => {
                let axis = match (*line_start_index, *line_end_index, *line_index) {
                    (Some(start), Some(end), _) => [self.point(start)?, self.point(end)?],
                    (_, _, Some(index)) => self.lines.get(index).copied().flatten()?,
                    _ => return None,
                };
                AffineMatrix::reflection(axis[0], axis[1])?
            }
            GeometryTransform::RotateSourcePoint {
                source_point_index,
                angle_degrees,
            } => {
                if *source_point_index != 0 {
                    return None;
                }
                AffineMatrix::rotation(source_center, angle_degrees.to_radians())
            }
            GeometryTransform::TranslateSourcePoint {
                source_point_index,
                target_index,
            } => {
                if *source_point_index != 0 {
                    return None;
                }
                let target = self.point(*target_index)?;
                AffineMatrix::translation(target.x - source_center.x, target.y - source_center.y)
            }
        })
    }

    fn evaluate(&self, value: &serde_json::Value) -> Option<f64> {
        let encoded = serde_json::to_vec(value).ok()?;
        let expression = parse_expression_json(&encoded).ok()?;
        evaluate_expr(&expression, 0.0, self.parameters)
    }
}

fn finite_point(point: &Point) -> bool {
    point.x.is_finite() && point.y.is_finite()
}

trait FinitePoint {
    fn finite(self) -> Option<Self>
    where
        Self: Sized;
}

impl FinitePoint for Point {
    fn finite(self) -> Option<Self> {
        finite_point(&self).then_some(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_matrix_applied_circle_center() {
        let input = br#"{
          "constraint": {
            "kind":"matrix-apply",
            "source":{"kind":"circle","centerIndex":0,"radiusIndex":1},
            "matrixApply":[{"kind":"reflect","lineStartIndex":2,"lineEndIndex":3,"lineIndex":null}]
          },
          "points":[{"x":2,"y":1},{"x":3,"y":1},{"x":0,"y":0},{"x":0,"y":1}]
        }"#;
        assert_eq!(
            resolve_circular_constraint_center_json(input).unwrap(),
            Some(Point { x: -2.0, y: 1.0 })
        );
    }
}
