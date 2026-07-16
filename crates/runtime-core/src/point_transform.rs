use std::collections::BTreeMap;

use serde::Deserialize;

use crate::{
    AffineMatrix, LineKind, Point, angle_bisector_direction, measured_rotation_radians,
    project_to_line_like,
};

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

#[derive(Clone, Deserialize)]
#[serde(tag = "kind")]
enum LineConstraint {
    #[serde(rename = "segment")]
    Segment {
        #[serde(rename = "startIndex")]
        start_index: usize,
        #[serde(rename = "endIndex")]
        end_index: usize,
    },
    #[serde(rename = "line")]
    Line {
        #[serde(rename = "startIndex")]
        start_index: usize,
        #[serde(rename = "endIndex")]
        end_index: usize,
    },
    #[serde(rename = "ray")]
    Ray {
        #[serde(rename = "startIndex")]
        start_index: usize,
        #[serde(rename = "endIndex")]
        end_index: usize,
    },
    #[serde(rename = "perpendicular-line")]
    Perpendicular {
        #[serde(rename = "throughIndex")]
        through_index: usize,
        #[serde(rename = "lineStartIndex")]
        line_start_index: usize,
        #[serde(rename = "lineEndIndex")]
        line_end_index: usize,
    },
    #[serde(rename = "parallel-line")]
    Parallel {
        #[serde(rename = "throughIndex")]
        through_index: usize,
        #[serde(rename = "lineStartIndex")]
        line_start_index: usize,
        #[serde(rename = "lineEndIndex")]
        line_end_index: usize,
    },
    #[serde(rename = "perpendicular-to")]
    PerpendicularTo {
        #[serde(rename = "throughIndex")]
        through_index: usize,
        line: Box<LineConstraint>,
    },
    #[serde(rename = "parallel-to")]
    ParallelTo {
        #[serde(rename = "throughIndex")]
        through_index: usize,
        line: Box<LineConstraint>,
    },
    #[serde(rename = "angle-bisector-ray")]
    AngleBisector {
        #[serde(rename = "startIndex")]
        start_index: usize,
        #[serde(rename = "vertexIndex")]
        vertex_index: usize,
        #[serde(rename = "endIndex")]
        end_index: usize,
    },
    #[serde(rename = "translated")]
    Translated {
        line: Box<LineConstraint>,
        #[serde(rename = "vectorStartIndex")]
        vector_start_index: usize,
        #[serde(rename = "vectorEndIndex")]
        vector_end_index: usize,
    },
    #[serde(rename = "translated-delta")]
    TranslatedDelta {
        line: Box<LineConstraint>,
        dx: f64,
        dy: f64,
    },
    #[serde(rename = "reflected")]
    Reflected {
        line: Box<LineConstraint>,
        axis: Box<LineConstraint>,
    },
    #[serde(rename = "rotated")]
    Rotated {
        line: Box<LineConstraint>,
        #[serde(rename = "centerIndex")]
        center_index: usize,
        #[serde(rename = "angleDegrees")]
        angle_degrees: f64,
    },
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
                let (start, end, _) = self.line_geometry(line)?;
                AffineMatrix::reflection(start, end)?
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

    fn line_geometry(&self, line: &LineConstraint) -> Option<(Point, Point, LineKind)> {
        match line {
            LineConstraint::Segment {
                start_index,
                end_index,
            } => Some((
                self.point(*start_index)?,
                self.point(*end_index)?,
                LineKind::Segment,
            )),
            LineConstraint::Line {
                start_index,
                end_index,
            } => Some((
                self.point(*start_index)?,
                self.point(*end_index)?,
                LineKind::Line,
            )),
            LineConstraint::Ray {
                start_index,
                end_index,
            } => Some((
                self.point(*start_index)?,
                self.point(*end_index)?,
                LineKind::Ray,
            )),
            LineConstraint::Perpendicular {
                through_index,
                line_start_index,
                line_end_index,
            } => self.perpendicular_line(
                self.point(*through_index)?,
                self.point(*line_start_index)?,
                self.point(*line_end_index)?,
            ),
            LineConstraint::Parallel {
                through_index,
                line_start_index,
                line_end_index,
            } => self.parallel_line(
                self.point(*through_index)?,
                self.point(*line_start_index)?,
                self.point(*line_end_index)?,
            ),
            LineConstraint::PerpendicularTo {
                through_index,
                line,
            } => {
                let (start, end, _) = self.line_geometry(line)?;
                self.perpendicular_line(self.point(*through_index)?, start, end)
            }
            LineConstraint::ParallelTo {
                through_index,
                line,
            } => {
                let (start, end, _) = self.line_geometry(line)?;
                self.parallel_line(self.point(*through_index)?, start, end)
            }
            LineConstraint::AngleBisector {
                start_index,
                vertex_index,
                end_index,
            } => {
                let start = self.point(*start_index)?;
                let vertex = self.point(*vertex_index)?;
                let end = self.point(*end_index)?;
                let direction = angle_bisector_direction(start, vertex, end)?;
                Some((
                    vertex,
                    Point {
                        x: vertex.x + direction.x,
                        y: vertex.y + direction.y,
                    },
                    LineKind::Ray,
                ))
            }
            LineConstraint::Translated {
                line,
                vector_start_index,
                vector_end_index,
            } => {
                let (start, end, kind) = self.line_geometry(line)?;
                let vector_start = self.point(*vector_start_index)?;
                let vector_end = self.point(*vector_end_index)?;
                let matrix = AffineMatrix::translation(
                    vector_end.x - vector_start.x,
                    vector_end.y - vector_start.y,
                );
                Some((matrix.apply(start), matrix.apply(end), kind))
            }
            LineConstraint::TranslatedDelta { line, dx, dy } => {
                let (start, end, kind) = self.line_geometry(line)?;
                let matrix = AffineMatrix::translation(*dx, *dy);
                Some((matrix.apply(start), matrix.apply(end), kind))
            }
            LineConstraint::Reflected { line, axis } => {
                let (start, end, kind) = self.line_geometry(line)?;
                let (axis_start, axis_end, _) = self.line_geometry(axis)?;
                let matrix = AffineMatrix::reflection(axis_start, axis_end)?;
                Some((matrix.apply(start), matrix.apply(end), kind))
            }
            LineConstraint::Rotated {
                line,
                center_index,
                angle_degrees,
            } => {
                let (start, end, kind) = self.line_geometry(line)?;
                let center = self.point(*center_index)?;
                let matrix = AffineMatrix::rotation(center, angle_degrees.to_radians());
                Some((matrix.apply(start), matrix.apply(end), kind))
            }
        }
    }

    fn perpendicular_line(
        &self,
        through: Point,
        start: Point,
        end: Point,
    ) -> Option<(Point, Point, LineKind)> {
        let dx = end.x - start.x;
        let dy = end.y - start.y;
        (dx.hypot(dy) > 1e-9).then_some((
            through,
            Point {
                x: through.x - dy,
                y: through.y + dx,
            },
            LineKind::Line,
        ))
    }

    fn parallel_line(
        &self,
        through: Point,
        start: Point,
        end: Point,
    ) -> Option<(Point, Point, LineKind)> {
        let dx = end.x - start.x;
        let dy = end.y - start.y;
        (dx.hypot(dy) > 1e-9).then_some((
            through,
            Point {
                x: through.x + dx,
                y: through.y + dy,
            },
            LineKind::Line,
        ))
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
    }
}
