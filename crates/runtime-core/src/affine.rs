use crate::Point;

const INVERSE_EPSILON: f64 = 1e-12;

/// A two-dimensional affine transform using the same six coefficients as a
/// homogeneous 3x3 matrix whose final row is `[0, 0, 1]`.
#[derive(Debug, Clone, Copy, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AffineMatrix {
    pub xx: f64,
    pub xy: f64,
    pub yx: f64,
    pub yy: f64,
    pub tx: f64,
    pub ty: f64,
}

impl AffineMatrix {
    pub const IDENTITY: Self = Self {
        xx: 1.0,
        xy: 0.0,
        yx: 0.0,
        yy: 1.0,
        tx: 0.0,
        ty: 0.0,
    };

    pub fn translation(dx: f64, dy: f64) -> Self {
        Self {
            tx: dx,
            ty: dy,
            ..Self::IDENTITY
        }
    }

    pub fn rotation(center: Point, radians: f64) -> Self {
        let (sin, cos) = radians.sin_cos();
        Self {
            xx: cos,
            xy: sin,
            yx: -sin,
            yy: cos,
            tx: center.x - cos * center.x - sin * center.y,
            ty: center.y + sin * center.x - cos * center.y,
        }
    }

    pub fn scale(center: Point, factor: f64) -> Self {
        Self {
            xx: factor,
            xy: 0.0,
            yx: 0.0,
            yy: factor,
            tx: center.x * (1.0 - factor),
            ty: center.y * (1.0 - factor),
        }
    }

    pub fn reflection(line_start: Point, line_end: Point) -> Option<Self> {
        let dx = line_end.x - line_start.x;
        let dy = line_end.y - line_start.y;
        let length_squared = dx * dx + dy * dy;
        if !length_squared.is_finite() || length_squared <= INVERSE_EPSILON {
            return None;
        }
        let xx = (dx * dx - dy * dy) / length_squared;
        let xy = 2.0 * dx * dy / length_squared;
        Some(Self {
            xx,
            xy,
            yx: xy,
            yy: -xx,
            tx: line_start.x - xx * line_start.x - xy * line_start.y,
            ty: line_start.y - xy * line_start.x + xx * line_start.y,
        })
    }

    pub fn apply(self, point: Point) -> Point {
        Point {
            x: self.xx * point.x + self.xy * point.y + self.tx,
            y: self.yx * point.x + self.yy * point.y + self.ty,
        }
    }

    pub fn determinant(self) -> f64 {
        self.xx * self.yy - self.xy * self.yx
    }

    /// Returns the transform that applies `self` first and `next` second.
    pub fn then(self, next: Self) -> Self {
        Self {
            xx: next.xx * self.xx + next.xy * self.yx,
            xy: next.xx * self.xy + next.xy * self.yy,
            yx: next.yx * self.xx + next.yy * self.yx,
            yy: next.yx * self.xy + next.yy * self.yy,
            tx: next.xx * self.tx + next.xy * self.ty + next.tx,
            ty: next.yx * self.tx + next.yy * self.ty + next.ty,
        }
    }

    pub fn inverse(self) -> Option<Self> {
        let determinant = self.determinant();
        if !determinant.is_finite() || determinant.abs() <= INVERSE_EPSILON {
            return None;
        }
        let xx = self.yy / determinant;
        let xy = -self.xy / determinant;
        let yx = -self.yx / determinant;
        let yy = self.xx / determinant;
        Some(Self {
            xx,
            xy,
            yx,
            yy,
            tx: -(xx * self.tx + xy * self.ty),
            ty: -(yx * self.tx + yy * self.ty),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn composition_and_inverse_preserve_apply_order() {
        let point = Point { x: 2.0, y: 1.0 };
        let translate = AffineMatrix::translation(3.0, -2.0);
        let rotate = AffineMatrix::rotation(Point { x: 0.0, y: 0.0 }, 90_f64.to_radians());
        let combined = translate.then(rotate);

        let composed = combined.apply(point);
        let sequential = rotate.apply(translate.apply(point));
        assert!((composed.x - sequential.x).abs() < 1e-12);
        assert!((composed.y - sequential.y).abs() < 1e-12);
        let restored = combined.inverse().unwrap().apply(combined.apply(point));
        assert!((restored.x - point.x).abs() < 1e-12);
        assert!((restored.y - point.y).abs() < 1e-12);
    }

    #[test]
    fn reflection_has_negative_orientation_and_is_its_own_inverse() {
        let reflection =
            AffineMatrix::reflection(Point { x: 0.0, y: -1.0 }, Point { x: 0.0, y: 1.0 }).unwrap();
        assert!(reflection.determinant() < 0.0);
        assert_eq!(
            reflection.apply(Point { x: 2.0, y: 3.0 }),
            Point { x: -2.0, y: 3.0 }
        );
        assert_eq!(reflection.inverse(), Some(reflection));
    }
}
