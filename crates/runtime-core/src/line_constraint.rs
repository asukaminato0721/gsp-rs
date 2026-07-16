use std::collections::{BTreeMap, BTreeSet};

use serde::Deserialize;

use crate::{
    AffineMatrix, LineKind, Point, angle_bisector_direction, evaluate_expr,
    measured_rotation_radians, parse_expression_json,
};

#[derive(Clone, Deserialize)]
#[serde(tag = "kind")]
pub(crate) enum LineConstraint {
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
    #[serde(rename = "angle-bisector-ray")]
    AngleBisector {
        #[serde(rename = "startIndex")]
        start_index: usize,
        #[serde(rename = "vertexIndex")]
        vertex_index: usize,
        #[serde(rename = "endIndex")]
        end_index: usize,
    },
    #[serde(rename = "matrix-apply")]
    MatrixApply {
        source: Box<LineConstraint>,
        #[serde(rename = "matrixApply")]
        matrix_apply: Vec<LineConstraintMatrix>,
    },
}

#[derive(Clone, Deserialize)]
#[serde(tag = "kind")]
pub(crate) enum LineConstraintMatrix {
    #[serde(rename = "translate-vector")]
    TranslateVector {
        #[serde(rename = "vectorStartIndex")]
        vector_start_index: usize,
        #[serde(rename = "vectorEndIndex")]
        vector_end_index: usize,
    },
    #[serde(rename = "translate-delta")]
    TranslateDelta { dx: f64, dy: f64 },
    #[serde(rename = "reflect")]
    Reflect { axis: Box<LineConstraint> },
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

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ResolvedLineConstraint {
    pub start: Point,
    pub end: Point,
    pub kind: LineKind,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ResolveLineConstraintInput {
    constraint: LineConstraint,
    points: BTreeMap<usize, Point>,
    #[serde(default)]
    parameters: BTreeMap<String, f64>,
}

pub fn line_constraint_point_indices_json(bytes: &[u8]) -> Result<Vec<usize>, serde_json::Error> {
    let constraint = serde_json::from_slice::<LineConstraint>(bytes)?;
    let mut indices = BTreeSet::new();
    constraint.collect_point_indices(&mut indices);
    Ok(indices.into_iter().collect())
}

pub fn resolve_line_constraint_json(
    bytes: &[u8],
) -> Result<Option<ResolvedLineConstraint>, serde_json::Error> {
    let input = serde_json::from_slice::<ResolveLineConstraintInput>(bytes)?;
    Ok(LineConstraintResolver {
        point: &|index| input.points.get(&index).copied(),
        parameters: &input.parameters,
    }
    .resolve(&input.constraint))
}

pub(crate) struct LineConstraintResolver<'a> {
    pub point: &'a dyn Fn(usize) -> Option<Point>,
    pub parameters: &'a BTreeMap<String, f64>,
}

impl LineConstraint {
    fn collect_point_indices(&self, indices: &mut BTreeSet<usize>) {
        match self {
            Self::Segment {
                start_index,
                end_index,
            }
            | Self::Line {
                start_index,
                end_index,
            }
            | Self::Ray {
                start_index,
                end_index,
            } => indices.extend([*start_index, *end_index]),
            Self::AngleBisector {
                start_index,
                vertex_index,
                end_index,
            } => indices.extend([*start_index, *vertex_index, *end_index]),
            Self::MatrixApply {
                source,
                matrix_apply,
            } => {
                source.collect_point_indices(indices);
                for matrix in matrix_apply {
                    matrix.collect_point_indices(indices);
                }
            }
        }
    }
}

impl LineConstraintMatrix {
    fn collect_point_indices(&self, indices: &mut BTreeSet<usize>) {
        match self {
            Self::TranslateVector {
                vector_start_index,
                vector_end_index,
            } => indices.extend([*vector_start_index, *vector_end_index]),
            Self::TranslateDelta { .. } => {}
            Self::Reflect { axis } => axis.collect_point_indices(indices),
            Self::Rotate {
                center_index,
                angle_start_index,
                angle_vertex_index,
                angle_end_index,
                ..
            } => {
                indices.insert(*center_index);
                indices.extend(
                    [*angle_start_index, *angle_vertex_index, *angle_end_index]
                        .into_iter()
                        .flatten(),
                );
            }
            Self::RotateSourcePoint { .. } => {}
            Self::TranslateSourcePoint { target_index, .. } => {
                indices.insert(*target_index);
            }
        }
    }
}

impl LineConstraintResolver<'_> {
    pub fn resolve(&self, line: &LineConstraint) -> Option<ResolvedLineConstraint> {
        let resolved = match line {
            LineConstraint::Segment {
                start_index,
                end_index,
            } => ResolvedLineConstraint {
                start: self.point(*start_index)?,
                end: self.point(*end_index)?,
                kind: LineKind::Segment,
            },
            LineConstraint::Line {
                start_index,
                end_index,
            } => ResolvedLineConstraint {
                start: self.point(*start_index)?,
                end: self.point(*end_index)?,
                kind: LineKind::Line,
            },
            LineConstraint::Ray {
                start_index,
                end_index,
            } => ResolvedLineConstraint {
                start: self.point(*start_index)?,
                end: self.point(*end_index)?,
                kind: LineKind::Ray,
            },
            LineConstraint::AngleBisector {
                start_index,
                vertex_index,
                end_index,
            } => {
                let vertex = self.point(*vertex_index)?;
                let direction = angle_bisector_direction(
                    self.point(*start_index)?,
                    vertex,
                    self.point(*end_index)?,
                )?;
                ResolvedLineConstraint {
                    start: vertex,
                    end: Point {
                        x: vertex.x + direction.x,
                        y: vertex.y + direction.y,
                    },
                    kind: LineKind::Ray,
                }
            }
            LineConstraint::MatrixApply {
                source,
                matrix_apply,
            } => {
                let mut resolved = self.resolve(source)?;
                for transform in matrix_apply {
                    let matrix = self.matrix(transform, resolved)?;
                    resolved.start = matrix.apply(resolved.start);
                    resolved.end = matrix.apply(resolved.end);
                    if matches!(
                        transform,
                        LineConstraintMatrix::RotateSourcePoint { .. }
                            | LineConstraintMatrix::TranslateSourcePoint { .. }
                    ) {
                        resolved.kind = LineKind::Line;
                    }
                }
                resolved
            }
        };
        ((resolved.end.x - resolved.start.x).hypot(resolved.end.y - resolved.start.y) > 1e-9)
            .then_some(resolved)
    }

    fn point(&self, index: usize) -> Option<Point> {
        (self.point)(index)
    }

    fn matrix(
        &self,
        transform: &LineConstraintMatrix,
        source: ResolvedLineConstraint,
    ) -> Option<AffineMatrix> {
        Some(match transform {
            LineConstraintMatrix::TranslateVector {
                vector_start_index,
                vector_end_index,
            } => {
                let start = self.point(*vector_start_index)?;
                let end = self.point(*vector_end_index)?;
                AffineMatrix::translation(end.x - start.x, end.y - start.y)
            }
            LineConstraintMatrix::TranslateDelta { dx, dy } => AffineMatrix::translation(*dx, *dy),
            LineConstraintMatrix::Reflect { axis } => {
                let axis = self.resolve(axis)?;
                AffineMatrix::reflection(axis.start, axis.end)?
            }
            LineConstraintMatrix::Rotate {
                center_index,
                angle_degrees,
                parameter_name,
                angle_expr,
                angle_start_index,
                angle_vertex_index,
                angle_end_index,
            } => AffineMatrix::rotation(
                self.point(*center_index)?,
                self.rotation_degrees(
                    *angle_degrees,
                    parameter_name.as_deref(),
                    angle_expr.as_ref(),
                    *angle_start_index,
                    *angle_vertex_index,
                    *angle_end_index,
                )?
                .to_radians(),
            ),
            LineConstraintMatrix::RotateSourcePoint {
                source_point_index,
                angle_degrees,
            } => AffineMatrix::rotation(
                [source.start, source.end]
                    .get(*source_point_index)
                    .copied()?,
                angle_degrees.to_radians(),
            ),
            LineConstraintMatrix::TranslateSourcePoint {
                source_point_index,
                target_index,
            } => {
                let source = [source.start, source.end]
                    .get(*source_point_index)
                    .copied()?;
                let target = self.point(*target_index)?;
                AffineMatrix::translation(target.x - source.x, target.y - source.y)
            }
        })
    }

    fn rotation_degrees(
        &self,
        angle_degrees: f64,
        parameter_name: Option<&str>,
        angle_expr: Option<&serde_json::Value>,
        angle_start_index: Option<usize>,
        angle_vertex_index: Option<usize>,
        angle_end_index: Option<usize>,
    ) -> Option<f64> {
        if let (Some(start), Some(vertex), Some(end)) =
            (angle_start_index, angle_vertex_index, angle_end_index)
        {
            return measured_rotation_radians(
                self.point(start)?,
                self.point(vertex)?,
                self.point(end)?,
            )
            .map(f64::to_degrees);
        }
        if let Some(encoded_expr) = angle_expr {
            let encoded = serde_json::to_vec(encoded_expr).ok()?;
            let expression = parse_expression_json(&encoded).ok()?;
            return evaluate_expr(&expression, 0.0, self.parameters);
        }
        if let Some(name) = parameter_name {
            return self
                .parameters
                .get(name)
                .copied()
                .filter(|value| value.is_finite());
        }
        angle_degrees.is_finite().then_some(angle_degrees)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_recursive_matrix_line_constraint() {
        let input = br#"{
          "constraint": {
            "kind":"matrix-apply",
            "source":{"kind":"line","startIndex":0,"endIndex":1},
            "matrixApply":[
              {"kind":"translate-vector","vectorStartIndex":2,"vectorEndIndex":3},
              {"kind":"reflect","axis":{"kind":"line","startIndex":4,"endIndex":5}}
            ]
          },
          "points": {
            "0":{"x":0,"y":0}, "1":{"x":1,"y":0},
            "2":{"x":0,"y":0}, "3":{"x":0,"y":2},
            "4":{"x":0,"y":0}, "5":{"x":0,"y":1}
          }
        }"#;
        let line = resolve_line_constraint_json(input).unwrap().unwrap();
        assert_eq!(line.kind, LineKind::Line);
        assert_eq!(line.start, Point { x: 0.0, y: 2.0 });
        assert_eq!(line.end, Point { x: -1.0, y: 2.0 });
    }

    #[test]
    fn collects_nested_constraint_point_indices() {
        let constraint = br#"{
          "kind":"matrix-apply",
          "source":{"kind":"segment","startIndex":7,"endIndex":2},
          "matrixApply":[{
            "kind":"reflect",
            "axis":{
              "kind":"matrix-apply",
              "source":{"kind":"line","startIndex":1,"endIndex":3},
              "matrixApply":[{
                "kind":"translate-source-point",
                "sourcePointIndex":0,
                "targetIndex":5
              }]
            }
          }]
        }"#;
        assert_eq!(
            line_constraint_point_indices_json(constraint).unwrap(),
            vec![1, 2, 3, 5, 7]
        );
    }
}
