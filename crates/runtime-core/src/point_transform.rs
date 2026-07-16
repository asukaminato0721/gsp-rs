use std::collections::BTreeMap;

use serde::Deserialize;

use crate::line_constraint::{LineConstraint, LineConstraintResolver};
use crate::{AffineMatrix, LineKind, Point, measured_rotation_radians, project_to_line_like};

#[derive(Clone, Copy, Deserialize)]
struct ScenePoint {
    x: f64,
    y: f64,
}

impl From<ScenePoint> for Point {
    fn from(point: ScenePoint) -> Self {
        Self {
            x: point.x,
            y: point.y,
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct InverseTransformInput {
    world: Point,
    points: Vec<ScenePoint>,
    #[serde(default)]
    parameters: BTreeMap<String, f64>,
    #[serde(rename = "matrixApply")]
    matrix_apply: Vec<PointTransform>,
}

#[derive(Clone, Deserialize)]
#[serde(tag = "kind")]
enum PointTransform {
    #[serde(rename = "translate")]
    Translate {
        #[serde(rename = "vectorStartIndex")]
        vector_start_index: usize,
        #[serde(rename = "vectorEndIndex")]
        vector_end_index: usize,
    },
    #[serde(rename = "reflect")]
    Reflect {
        #[serde(rename = "lineStartIndex")]
        line_start_index: usize,
        #[serde(rename = "lineEndIndex")]
        line_end_index: usize,
    },
    #[serde(rename = "reflect-constraint")]
    ReflectConstraint { line: LineConstraint },
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
        #[serde(rename = "angleParameterPointIndex")]
        angle_parameter_point_index: Option<usize>,
        #[serde(rename = "angleParameterStartIndex")]
        angle_parameter_start_index: Option<usize>,
        #[serde(rename = "angleParameterEndIndex")]
        angle_parameter_end_index: Option<usize>,
        #[serde(rename = "angleParameterScale")]
        angle_parameter_scale: Option<f64>,
    },
    #[serde(rename = "scale")]
    Scale {
        #[serde(rename = "centerIndex")]
        center_index: usize,
        factor: f64,
        #[serde(rename = "parameterName")]
        parameter_name: Option<String>,
        #[serde(rename = "factorExpr")]
        factor_expr: Option<serde_json::Value>,
        #[serde(rename = "factorParameterPointIndex")]
        factor_parameter_point_index: Option<usize>,
        #[serde(rename = "factorParameterStartIndex")]
        factor_parameter_start_index: Option<usize>,
        #[serde(rename = "factorParameterEndIndex")]
        factor_parameter_end_index: Option<usize>,
    },
    #[serde(other)]
    Unsupported,
}

pub fn inverse_point_transform_json(bytes: &[u8]) -> Result<Option<Point>, serde_json::Error> {
    let input = serde_json::from_slice::<InverseTransformInput>(bytes)?;
    Ok(InverseResolver {
        points: &input.points,
        parameters: &input.parameters,
    }
    .inverse_matrix_apply(&input.matrix_apply, input.world))
}

struct InverseResolver<'a> {
    points: &'a [ScenePoint],
    parameters: &'a BTreeMap<String, f64>,
}

impl InverseResolver<'_> {
    fn point(&self, index: usize) -> Option<Point> {
        self.points.get(index).copied().map(Point::from)
    }

    fn inverse_matrix_apply(&self, transforms: &[PointTransform], world: Point) -> Option<Point> {
        transforms
            .iter()
            .try_fold(AffineMatrix::IDENTITY, |combined, transform| {
                Some(combined.then(self.transform_matrix(transform)?))
            })?
            .inverse()
            .map(|matrix| matrix.apply(world))
    }

    fn transform_matrix(&self, transform: &PointTransform) -> Option<AffineMatrix> {
        Some(match transform {
            PointTransform::Translate {
                vector_start_index,
                vector_end_index,
            } => {
                let start = self.point(*vector_start_index)?;
                let end = self.point(*vector_end_index)?;
                AffineMatrix::translation(end.x - start.x, end.y - start.y)
            }
            PointTransform::Reflect {
                line_start_index,
                line_end_index,
            } => AffineMatrix::reflection(
                self.point(*line_start_index)?,
                self.point(*line_end_index)?,
            )?,
            PointTransform::ReflectConstraint { line } => {
                let resolved = LineConstraintResolver {
                    point: &|index| self.point(index),
                    parameters: self.parameters,
                }
                .resolve(line)?;
                AffineMatrix::reflection(resolved.start, resolved.end)?
            }
            PointTransform::Rotate { center_index, .. } => AffineMatrix::rotation(
                self.point(*center_index)?,
                self.rotation_degrees(transform)?.to_radians(),
            ),
            PointTransform::Scale { center_index, .. } => {
                AffineMatrix::scale(self.point(*center_index)?, self.scale_factor(transform)?)
            }
            PointTransform::Unsupported => return None,
        })
    }

    fn rotation_degrees(&self, transform: &PointTransform) -> Option<f64> {
        let PointTransform::Rotate {
            angle_degrees,
            parameter_name,
            angle_expr,
            angle_start_index,
            angle_vertex_index,
            angle_end_index,
            angle_parameter_point_index,
            angle_parameter_start_index,
            angle_parameter_end_index,
            angle_parameter_scale,
            ..
        } = transform
        else {
            return None;
        };
        if let (Some(point_index), Some(start_index), Some(end_index)) = (
            angle_parameter_point_index,
            angle_parameter_start_index,
            angle_parameter_end_index,
        ) {
            return Some(
                project_to_line_like(
                    self.point(*point_index)?,
                    self.point(*start_index)?,
                    self.point(*end_index)?,
                    LineKind::Segment,
                )?
                .t * angle_parameter_scale.unwrap_or(1.0),
            );
        }
        if let (Some(start_index), Some(vertex_index), Some(end_index)) =
            (angle_start_index, angle_vertex_index, angle_end_index)
        {
            return Some(
                measured_rotation_radians(
                    self.point(*start_index)?,
                    self.point(*vertex_index)?,
                    self.point(*end_index)?,
                )?
                .to_degrees(),
            );
        }
        if let Some(expr) = angle_expr {
            return self.evaluate(expr);
        }
        if let Some(name) = parameter_name {
            return self.parameters.get(name).copied();
        }
        angle_degrees.is_finite().then_some(*angle_degrees)
    }

    fn scale_factor(&self, transform: &PointTransform) -> Option<f64> {
        let PointTransform::Scale {
            factor,
            parameter_name,
            factor_expr,
            factor_parameter_point_index,
            factor_parameter_start_index,
            factor_parameter_end_index,
            ..
        } = transform
        else {
            return None;
        };
        if let (Some(point_index), Some(start_index), Some(end_index)) = (
            factor_parameter_point_index,
            factor_parameter_start_index,
            factor_parameter_end_index,
        ) && let Some(projection) = project_to_line_like(
            self.point(*point_index)?,
            self.point(*start_index)?,
            self.point(*end_index)?,
            LineKind::Segment,
        ) {
            return Some(projection.t);
        }
        if let Some(expr) = factor_expr {
            return self.evaluate(expr);
        }
        if let Some(name) = parameter_name {
            return self.parameters.get(name).copied();
        }
        factor.is_finite().then_some(*factor)
    }

    fn evaluate(&self, encoded_expr: &serde_json::Value) -> Option<f64> {
        let encoded = serde_json::to_vec(encoded_expr).ok()?;
        let expr = crate::parse_expression_json(&encoded).ok()?;
        crate::evaluate_expr(&expr, 0.0, self.parameters).filter(|value| value.is_finite())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inverts_payload_defined_point_transforms() {
        let translated = br#"{
          "world":{"x":5,"y":7},
          "points":[{"x":1,"y":2},{"x":4,"y":6}],
          "matrixApply":[{"kind":"translate","vectorStartIndex":0,"vectorEndIndex":1}]
        }"#;
        assert_eq!(
            inverse_point_transform_json(translated).unwrap(),
            Some(Point { x: 2.0, y: 3.0 })
        );

        let rotated = br#"{
          "world":{"x":0,"y":-2},
          "points":[{"x":0,"y":0}],
          "matrixApply":[{"kind":"rotate","centerIndex":0,"angleDegrees":90,"parameterName":null}]
        }"#;
        let source = inverse_point_transform_json(rotated).unwrap().unwrap();
        assert!((source.x - 2.0).abs() < 1e-9);
        assert!(source.y.abs() < 1e-9);

        let scaled = br#"{
          "world":{"x":4,"y":2},
          "points":[{"x":0,"y":0}],
          "matrixApply":[{"kind":"scale","centerIndex":0,"factor":2,"parameterName":null}]
        }"#;
        assert_eq!(
            inverse_point_transform_json(scaled).unwrap(),
            Some(Point { x: 2.0, y: 1.0 })
        );

        let composed = br#"{
          "world":{"x":0,"y":-3},
          "points":[{"x":0,"y":0},{"x":1,"y":0}],
          "matrixApply":[
            {"kind":"translate","vectorStartIndex":0,"vectorEndIndex":1},
            {"kind":"rotate","centerIndex":0,"angleDegrees":90,"parameterName":null}
          ]
        }"#;
        let source = inverse_point_transform_json(composed).unwrap().unwrap();
        assert!((source.x - 2.0).abs() < 1e-9);
        assert!(source.y.abs() < 1e-9);

        let reflected_by_transformed_line = br#"{
          "world":{"x":3,"y":4},
          "points":[{"x":0,"y":0},{"x":0,"y":1}],
          "matrixApply":[{
            "kind":"reflect-constraint",
            "line":{
              "kind":"matrix-apply",
              "source":{"kind":"line","startIndex":0,"endIndex":1},
              "matrixApply":[
                {"kind":"translate-delta","dx":2,"dy":0},
                {"kind":"rotate","centerIndex":0,"angleDegrees":90}
              ]
            }
          }]
        }"#;
        assert_eq!(
            inverse_point_transform_json(reflected_by_transformed_line).unwrap(),
            Some(Point { x: 3.0, y: -8.0 })
        );
    }
}
