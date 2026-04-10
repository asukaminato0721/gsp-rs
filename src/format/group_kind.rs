macro_rules! define_group_kinds {
    ($($name:ident = $value:literal,)+) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub enum GroupKind {
            $($name,)+
            Unknown(u16),
        }

        impl From<u16> for GroupKind {
            fn from(value: u16) -> Self {
                match value {
                    $($value => Self::$name,)+
                    other => Self::Unknown(other),
                }
            }
        }

        impl GroupKind {
            pub fn raw(self) -> u16 {
                match self {
                    $(Self::$name => $value,)+
                    Self::Unknown(other) => other,
                }
            }

            pub fn is_line_like(self) -> bool {
                matches!(self, Self::Segment | Self::Line | Self::Ray)
            }

            pub fn is_rendered_line_group(self) -> bool {
                matches!(
                    self,
                    Self::Segment | Self::AngleMarker | Self::LineKind5 | Self::LineKind6 | Self::LineKind7
                )
            }

            pub fn is_coordinate_object(self) -> bool {
                matches!(
                    self,
                    Self::CoordinatePoint
                        | Self::CoordinateExpressionPoint
                        | Self::CoordinateExpressionPointAlt
                        | Self::Unknown(20)
                        | Self::CoordinateTrace
                )
            }

            pub fn is_iteration_helper(self) -> bool {
                matches!(
                    self,
                    Self::AffineIteration
                        | Self::IterationBinding
                        | Self::RegularPolygonIteration
                        | Self::IterationExpressionHelper
                )
            }

            pub fn is_carried_iteration(self) -> bool {
                matches!(self, Self::AffineIteration | Self::RegularPolygonIteration)
            }

            pub fn is_graph_calibration(self) -> bool {
                matches!(self, Self::GraphCalibrationX | Self::GraphCalibrationY)
            }

            pub fn is_graph_object(self) -> bool {
                matches!(
                    self,
                    Self::GraphObject40
                        | Self::GraphCalibrationX
                        | Self::GraphCalibrationY
                        | Self::MeasurementLine
                        | Self::AxisLine
                )
            }

            pub fn is_point_constraint(self) -> bool {
                matches!(self, Self::PointConstraint | Self::PathPoint)
            }
        }
    };
}

define_group_kinds! {
    Point = 0,
    Midpoint = 1,
    Segment = 2,
    Circle = 3,
    CircleCenterRadius = 4,
    LineKind5 = 5,
    LineKind6 = 6,
    LineKind7 = 7,
    Polygon = 8,
    LinearIntersectionPoint = 9,
    CircleInterior = 10,
    IntersectionPoint1 = 11,
    IntersectionPoint2 = 12,
    CircleCircleIntersectionPoint1 = 13,
    CircleCircleIntersectionPoint2 = 14,
    PointConstraint = 15,
    Translation = 16,
    CartesianOffsetPoint = 17,
    CoordinateExpressionPoint = 18,
    CoordinateExpressionPointAlt = 19,
    PolarOffsetPoint = 21,
    DerivedSegment24 = 24,
    CustomTransformPoint = 26,
    Rotation = 27,
    ParameterRotation = 29,
    Scale = 30,
    Reflection = 34,
    PointTrace = 35,
    GraphObject40 = 40,
    FunctionExpr = 48,
    Kind51 = 51,
    GraphCalibrationX = 52,
    GraphCalibrationY = 54,
    MeasurementLine = 58,
    AxisLine = 61,
    ActionButton = 62,
    Line = 63,
    Ray = 64,
    OffsetAnchor = 67,
    CoordinatePoint = 69,
    FunctionPlot = 72,
    ButtonLabel = 73,
    DerivedSegment75 = 75,
    AffineIteration = 76,
    IterationBinding = 77,
    DerivativeFunction = 78,
    ArcOnCircle = 79,
    CenterArc = 80,
    ThreePointArc = 81,
    SectorBoundary = 82,
    CircularSegmentBoundary = 83,
    RegularPolygonIteration = 89,
    LabelIterationSeed = 90,
    IterationExpressionHelper = 92,
    ParameterAnchor = 94,
    ParameterControlledPoint = 95,
    CoordinateTrace = 97,
    CoordinateTraceIntersectionPoint = 98,
    CustomTransformTrace = 102,
    AngleMarker = 113,
    PathPoint = 123,
    SegmentMarker = 121,
}

#[cfg(test)]
mod tests {
    use super::GroupKind;

    #[test]
    fn round_trips_known_and_unknown_kind_ids() {
        assert_eq!(GroupKind::from(0), GroupKind::Point);
        assert_eq!(GroupKind::Point.raw(), 0);
        assert_eq!(GroupKind::from(92), GroupKind::IterationExpressionHelper);
        assert_eq!(GroupKind::IterationExpressionHelper.raw(), 92);
        assert_eq!(GroupKind::from(121), GroupKind::SegmentMarker);
        assert_eq!(GroupKind::SegmentMarker.raw(), 121);
        assert_eq!(GroupKind::from(999), GroupKind::Unknown(999));
        assert_eq!(GroupKind::Unknown(999).raw(), 999);
    }

    #[test]
    fn categorizes_common_group_kind_families() {
        assert!(GroupKind::Segment.is_line_like());
        assert!(GroupKind::Ray.is_line_like());
        assert!(!GroupKind::AngleMarker.is_line_like());

        assert!(GroupKind::AngleMarker.is_rendered_line_group());
        assert!(GroupKind::LineKind6.is_rendered_line_group());
        assert!(!GroupKind::Line.is_rendered_line_group());

        assert!(GroupKind::CoordinateTrace.is_coordinate_object());
        assert!(GroupKind::RegularPolygonIteration.is_iteration_helper());
        assert!(GroupKind::IterationExpressionHelper.is_iteration_helper());
        assert!(GroupKind::AffineIteration.is_carried_iteration());
        assert!(GroupKind::GraphCalibrationX.is_graph_calibration());
        assert!(GroupKind::MeasurementLine.is_graph_object());
    }
}
