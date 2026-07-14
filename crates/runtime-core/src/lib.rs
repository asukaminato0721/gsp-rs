//! Canonical mathematical semantics shared by the native parser and browser runtime.

mod dependency;
mod geometry;
pub mod object_graph;
mod object_ops;
mod point_constraints;
mod scene_math;

pub use dependency::{DependencyNodeInput, DependencyPlan, DependencyPlanError};

pub use geometry::{
    Bounds, LineKind, Point, Projection, ThreePointArcGeometry, angle_bisector_direction,
    circle_arc_control_points, circle_circle_intersections, clip_line_to_bounds,
    clip_ray_to_bounds, lerp_point, line_circle_intersections, line_line_intersection,
    marked_angle_translation_point, measured_rotation_radians, normalize_angle_delta,
    point_circle_tangents, point_on_circle_arc, point_on_three_point_arc,
    point_on_three_point_arc_complement, project_to_circle_arc, project_to_line_like,
    project_to_three_point_arc, reflect_across_line, rotate_around, scale_around,
    scale_by_three_point_ratio, three_point_arc_geometry,
};
pub use object_ops::{
    AffineTargetHandle, BuiltinOperationTable, ObjectCircle, ObjectExpression,
    ObjectGraphEvaluationInput, ObjectIterationProgram, ObjectNodeValue, ObjectOp, ObjectOpError,
    ObjectProgram, ObjectSourceValue, ObjectValue, TraceDriver, evaluate_object_graph_json,
};
pub use point_constraints::{
    inverse_point_transform_json, resolve_point_constraints_json, transform_points_json,
};
pub use scene_math::{
    CoordinateTraceMode, PlotMode, affine_iteration_segment, angle_marker_points,
    branching_iteration_segments, choose_point_candidate, directed_angle_anchor,
    line_circle_intersection_candidate, line_polyline_intersection, point_angle_degrees,
    point_distance, point_distance_ratio, polygon_area, rotate_iteration_points, sample_circle_arc,
    sample_coordinate_trace, sample_custom_transform_trace, sample_expression,
    sample_parametric_curve, sample_three_point_arc, segment_marker_points,
    translation_iteration_deltas,
};

use std::collections::BTreeMap;

use serde::Deserialize;

#[derive(Debug, Clone, PartialEq)]
pub enum FunctionExpr {
    Constant(f64),
    Identity,
    SinIdentity,
    CosIdentityPlus(f64),
    TanIdentityMinus(f64),
    Parsed(FunctionAst),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Pow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum UnaryFunction {
    Sin,
    Cos,
    Tan,
    Abs,
    Sqrt,
    Ln,
    Log10,
    Sign,
    Round,
    Trunc,
}

#[derive(Debug, Clone, PartialEq)]
pub enum FunctionAst {
    Variable,
    Constant(f64),
    PiConstant,
    EulerConstant,
    PiAngle,
    Parameter(String, f64),
    Unary {
        op: UnaryFunction,
        expr: Box<FunctionAst>,
    },
    Binary {
        lhs: Box<FunctionAst>,
        op: BinaryOp,
        rhs: Box<FunctionAst>,
    },
}

/// Evaluates an expression using the same rules in native code and WebAssembly.
pub fn evaluate_expr(
    expr: &FunctionExpr,
    x: f64,
    parameters: &BTreeMap<String, f64>,
) -> Option<f64> {
    match expr {
        FunctionExpr::Constant(value) => finite(*value),
        FunctionExpr::Identity => finite(x),
        FunctionExpr::SinIdentity => finite(x.sin()),
        FunctionExpr::CosIdentityPlus(offset) => finite(x.cos() + offset),
        FunctionExpr::TanIdentityMinus(offset) => {
            guarded_tangent(x).and_then(|value| finite(value - offset))
        }
        FunctionExpr::Parsed(ast) => evaluate_ast(ast, x, parameters),
    }
}

/// Returns the parameter names referenced by an expression in payload order.
///
/// Keeping this traversal in the shared Rust core prevents the exporter and the
/// browser runtime from growing separate interpretations of the expression AST.
pub fn expression_parameter_names(expr: &FunctionExpr) -> Vec<String> {
    fn collect(
        ast: &FunctionAst,
        seen: &mut std::collections::BTreeSet<String>,
        names: &mut Vec<String>,
    ) {
        match ast {
            FunctionAst::Parameter(name, _) if seen.insert(name.clone()) => {
                names.push(name.clone())
            }
            FunctionAst::Unary { expr, .. } => collect(expr, seen, names),
            FunctionAst::Binary { lhs, rhs, .. } => {
                collect(lhs, seen, names);
                collect(rhs, seen, names);
            }
            FunctionAst::Variable
            | FunctionAst::Constant(_)
            | FunctionAst::PiConstant
            | FunctionAst::EulerConstant
            | FunctionAst::PiAngle
            | FunctionAst::Parameter(_, _) => {}
        }
    }

    let mut names = Vec::new();
    if let FunctionExpr::Parsed(ast) = expr {
        collect(ast, &mut std::collections::BTreeSet::new(), &mut names);
    }
    names
}

/// Reports whether an expression uses the payload's degree-angle constant.
pub fn expression_contains_pi_angle(expr: &FunctionExpr) -> bool {
    matches!(expr, FunctionExpr::Parsed(ast) if ast_contains_pi_angle(ast))
}

fn evaluate_ast(expr: &FunctionAst, x: f64, parameters: &BTreeMap<String, f64>) -> Option<f64> {
    let value = match expr {
        FunctionAst::Variable => x,
        FunctionAst::Constant(value) => *value,
        FunctionAst::PiConstant => std::f64::consts::PI,
        FunctionAst::EulerConstant => std::f64::consts::E,
        FunctionAst::PiAngle => 180.0,
        FunctionAst::Parameter(name, default) => *parameters.get(name).unwrap_or(default),
        FunctionAst::Unary { op, expr } => {
            let value = evaluate_ast(expr, x, parameters)?;
            let trig_value = if ast_contains_pi_angle(expr) {
                value.to_radians()
            } else {
                value
            };
            match op {
                UnaryFunction::Sin => trig_value.sin(),
                UnaryFunction::Cos => trig_value.cos(),
                UnaryFunction::Tan => guarded_tangent(trig_value)?,
                UnaryFunction::Abs => value.abs(),
                UnaryFunction::Sqrt => (value >= 0.0).then(|| value.sqrt())?,
                UnaryFunction::Ln => (value > 0.0).then(|| value.ln())?,
                UnaryFunction::Log10 => (value > 0.0).then(|| value.log10())?,
                UnaryFunction::Sign => {
                    if value > 0.0 {
                        1.0
                    } else if value < 0.0 {
                        -1.0
                    } else {
                        0.0
                    }
                }
                UnaryFunction::Round => value.round(),
                UnaryFunction::Trunc => value.trunc(),
            }
        }
        FunctionAst::Binary { lhs, op, rhs } => {
            let lhs = evaluate_ast(lhs, x, parameters)?;
            let rhs = evaluate_ast(rhs, x, parameters)?;
            match op {
                BinaryOp::Add => lhs + rhs,
                BinaryOp::Sub => lhs - rhs,
                BinaryOp::Mul => lhs * rhs,
                BinaryOp::Div => (rhs.abs() >= 1e-9).then_some(lhs / rhs)?,
                BinaryOp::Pow => lhs.powf(rhs),
            }
        }
    };
    finite(value)
}

fn finite(value: f64) -> Option<f64> {
    value.is_finite().then_some(value)
}

fn guarded_tangent(radians: f64) -> Option<f64> {
    let value = radians.tan();
    (value.is_finite() && radians.cos().abs() >= 0.04 && value.abs() <= 5.0).then_some(value)
}

fn ast_contains_pi_angle(expr: &FunctionAst) -> bool {
    match expr {
        FunctionAst::PiAngle => true,
        FunctionAst::Unary { expr, .. } => ast_contains_pi_angle(expr),
        FunctionAst::Binary { lhs, rhs, .. } => {
            ast_contains_pi_angle(lhs) || ast_contains_pi_angle(rhs)
        }
        FunctionAst::Variable
        | FunctionAst::Constant(_)
        | FunctionAst::PiConstant
        | FunctionAst::EulerConstant
        | FunctionAst::Parameter(_, _) => false,
    }
}

#[derive(Deserialize)]
#[serde(untagged)]
enum ExpressionInput {
    Expression(WireFunctionExpr),
    Ast(WireFunctionAst),
}

#[derive(Deserialize)]
#[serde(tag = "kind")]
enum WireFunctionExpr {
    #[serde(rename = "constant")]
    Constant { value: f64 },
    #[serde(rename = "identity")]
    Identity,
    #[serde(rename = "parsed")]
    Parsed { expr: WireFunctionAst },
}

#[derive(Deserialize)]
#[serde(tag = "kind")]
enum WireFunctionAst {
    #[serde(rename = "variable")]
    Variable,
    #[serde(rename = "constant")]
    Constant { value: f64 },
    #[serde(rename = "pi-constant")]
    PiConstant,
    #[serde(rename = "euler-constant")]
    EulerConstant,
    #[serde(rename = "parameter")]
    Parameter { name: String, value: f64 },
    #[serde(rename = "pi-angle")]
    PiAngle,
    #[serde(rename = "unary")]
    Unary {
        op: UnaryFunction,
        expr: Box<WireFunctionAst>,
    },
    #[serde(rename = "binary")]
    Binary {
        lhs: Box<WireFunctionAst>,
        op: BinaryOp,
        rhs: Box<WireFunctionAst>,
    },
}

impl From<WireFunctionExpr> for FunctionExpr {
    fn from(value: WireFunctionExpr) -> Self {
        match value {
            WireFunctionExpr::Constant { value } => Self::Constant(value),
            WireFunctionExpr::Identity => Self::Identity,
            WireFunctionExpr::Parsed { expr } => Self::Parsed(expr.into()),
        }
    }
}

impl From<WireFunctionAst> for FunctionAst {
    fn from(value: WireFunctionAst) -> Self {
        match value {
            WireFunctionAst::Variable => Self::Variable,
            WireFunctionAst::Constant { value } => Self::Constant(value),
            WireFunctionAst::PiConstant => Self::PiConstant,
            WireFunctionAst::EulerConstant => Self::EulerConstant,
            WireFunctionAst::Parameter { name, value } => Self::Parameter(name, value),
            WireFunctionAst::PiAngle => Self::PiAngle,
            WireFunctionAst::Unary { op, expr } => Self::Unary {
                op,
                expr: Box::new((*expr).into()),
            },
            WireFunctionAst::Binary { lhs, op, rhs } => Self::Binary {
                lhs: Box::new((*lhs).into()),
                op,
                rhs: Box::new((*rhs).into()),
            },
        }
    }
}

fn parse_expression_json(bytes: &[u8]) -> Result<FunctionExpr, serde_json::Error> {
    serde_json::from_slice::<ExpressionInput>(bytes).map(|input| match input {
        ExpressionInput::Expression(expr) => expr.into(),
        ExpressionInput::Ast(ast) => FunctionExpr::Parsed(ast.into()),
    })
}

#[cfg(target_arch = "wasm32")]
mod wasm_abi {
    use std::{
        cell::RefCell,
        collections::{BTreeMap, BTreeSet},
    };

    use super::{
        Bounds, CoordinateTraceMode, DependencyNodeInput, DependencyPlan, FunctionAst,
        FunctionExpr, LineKind, PlotMode, Point, Projection, affine_iteration_segment,
        angle_bisector_direction, branching_iteration_segments, choose_point_candidate,
        circle_arc_control_points, circle_circle_intersections, clip_line_to_bounds,
        clip_ray_to_bounds, evaluate_expr, evaluate_object_graph_json,
        inverse_point_transform_json, lerp_point, line_circle_intersection_candidate,
        line_circle_intersections, line_line_intersection, line_polyline_intersection,
        measured_rotation_radians, normalize_angle_delta, parse_expression_json,
        point_angle_degrees, point_circle_tangents, point_distance, point_distance_ratio,
        point_on_circle_arc, point_on_three_point_arc, point_on_three_point_arc_complement,
        polygon_area, project_to_circle_arc, project_to_line_like, project_to_three_point_arc,
        reflect_across_line, resolve_point_constraints_json, rotate_around,
        rotate_iteration_points, sample_circle_arc, sample_coordinate_trace,
        sample_custom_transform_trace, sample_expression, sample_parametric_curve,
        sample_three_point_arc, scale_around, scale_by_three_point_ratio, three_point_arc_geometry,
        transform_points_json, translation_iteration_deltas,
    };

    struct CompiledExpression {
        expr: FunctionExpr,
        parameter_names: Vec<String>,
        parameter_defaults: Vec<f64>,
        parameters: BTreeMap<String, f64>,
    }

    thread_local! {
        static EXPRESSIONS: RefCell<Vec<CompiledExpression>> = const { RefCell::new(Vec::new()) };
        static GEOMETRY_RESULTS: RefCell<Vec<Point>> = const { RefCell::new(Vec::new()) };
        static GEOMETRY_SCALARS: RefCell<Vec<f64>> = const { RefCell::new(Vec::new()) };
        static BATCH_RESULTS: RefCell<Vec<f64>> = const { RefCell::new(Vec::new()) };
        static DEPENDENCY_PLANS: RefCell<Vec<DependencyPlan>> = const { RefCell::new(Vec::new()) };
        static JSON_RESULT: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
        static LAST_ERROR: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn gsp_runtime_abi_version() -> u32 {
        7
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn gsp_normalize_angle_delta(from: f64, to: f64) -> f64 {
        normalize_angle_delta(from, to)
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn gsp_lerp_point(
        start_x: f64,
        start_y: f64,
        end_x: f64,
        end_y: f64,
        t: f64,
    ) -> u32 {
        write_geometry_results([lerp_point(
            Point {
                x: start_x,
                y: start_y,
            },
            Point { x: end_x, y: end_y },
            t,
        )])
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn gsp_rotate_around(
        point_x: f64,
        point_y: f64,
        center_x: f64,
        center_y: f64,
        radians: f64,
    ) -> u32 {
        write_geometry_results([rotate_around(
            Point {
                x: point_x,
                y: point_y,
            },
            Point {
                x: center_x,
                y: center_y,
            },
            radians,
        )])
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn gsp_scale_around(
        point_x: f64,
        point_y: f64,
        center_x: f64,
        center_y: f64,
        factor: f64,
    ) -> u32 {
        write_geometry_results([scale_around(
            Point {
                x: point_x,
                y: point_y,
            },
            Point {
                x: center_x,
                y: center_y,
            },
            factor,
        )])
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn gsp_reflect_across_line(
        point_x: f64,
        point_y: f64,
        start_x: f64,
        start_y: f64,
        end_x: f64,
        end_y: f64,
    ) -> u32 {
        write_geometry_results(reflect_across_line(
            Point {
                x: point_x,
                y: point_y,
            },
            Point {
                x: start_x,
                y: start_y,
            },
            Point { x: end_x, y: end_y },
        ))
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn gsp_project_to_line_like(
        point_x: f64,
        point_y: f64,
        start_x: f64,
        start_y: f64,
        end_x: f64,
        end_y: f64,
        line_kind: u32,
    ) -> u32 {
        let Some(line_kind) = line_kind_from_abi(line_kind) else {
            return write_projection(None);
        };
        write_projection(project_to_line_like(
            Point {
                x: point_x,
                y: point_y,
            },
            Point {
                x: start_x,
                y: start_y,
            },
            Point { x: end_x, y: end_y },
            line_kind,
        ))
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn gsp_angle_bisector_direction(
        start_x: f64,
        start_y: f64,
        vertex_x: f64,
        vertex_y: f64,
        end_x: f64,
        end_y: f64,
    ) -> u32 {
        write_geometry_results(angle_bisector_direction(
            Point {
                x: start_x,
                y: start_y,
            },
            Point {
                x: vertex_x,
                y: vertex_y,
            },
            Point { x: end_x, y: end_y },
        ))
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn gsp_measured_rotation_radians(
        start_x: f64,
        start_y: f64,
        vertex_x: f64,
        vertex_y: f64,
        end_x: f64,
        end_y: f64,
    ) -> f64 {
        measured_rotation_radians(
            Point {
                x: start_x,
                y: start_y,
            },
            Point {
                x: vertex_x,
                y: vertex_y,
            },
            Point { x: end_x, y: end_y },
        )
        .unwrap_or(f64::NAN)
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn gsp_scale_by_three_point_ratio(
        source_x: f64,
        source_y: f64,
        center_x: f64,
        center_y: f64,
        origin_x: f64,
        origin_y: f64,
        denominator_x: f64,
        denominator_y: f64,
        numerator_x: f64,
        numerator_y: f64,
        signed: u32,
        clamp_to_unit: u32,
    ) -> u32 {
        write_geometry_results(scale_by_three_point_ratio(
            Point {
                x: source_x,
                y: source_y,
            },
            Point {
                x: center_x,
                y: center_y,
            },
            Point {
                x: origin_x,
                y: origin_y,
            },
            Point {
                x: denominator_x,
                y: denominator_y,
            },
            Point {
                x: numerator_x,
                y: numerator_y,
            },
            signed != 0,
            clamp_to_unit != 0,
        ))
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn gsp_clip_line_to_bounds(
        start_x: f64,
        start_y: f64,
        end_x: f64,
        end_y: f64,
        min_x: f64,
        max_x: f64,
        min_y: f64,
        max_y: f64,
    ) -> u32 {
        write_geometry_results(
            clip_line_to_bounds(
                Point {
                    x: start_x,
                    y: start_y,
                },
                Point { x: end_x, y: end_y },
                Bounds {
                    min_x,
                    max_x,
                    min_y,
                    max_y,
                },
            )
            .into_iter()
            .flatten(),
        )
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn gsp_clip_ray_to_bounds(
        start_x: f64,
        start_y: f64,
        end_x: f64,
        end_y: f64,
        min_x: f64,
        max_x: f64,
        min_y: f64,
        max_y: f64,
    ) -> u32 {
        write_geometry_results(
            clip_ray_to_bounds(
                Point {
                    x: start_x,
                    y: start_y,
                },
                Point { x: end_x, y: end_y },
                Bounds {
                    min_x,
                    max_x,
                    min_y,
                    max_y,
                },
            )
            .into_iter()
            .flatten(),
        )
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn gsp_three_point_arc_geometry(
        start_x: f64,
        start_y: f64,
        mid_x: f64,
        mid_y: f64,
        end_x: f64,
        end_y: f64,
    ) -> u32 {
        let Some(geometry) = three_point_arc_geometry(
            Point {
                x: start_x,
                y: start_y,
            },
            Point { x: mid_x, y: mid_y },
            Point { x: end_x, y: end_y },
        ) else {
            return write_geometry_results(std::iter::empty());
        };
        write_geometry_scalars([
            geometry.radius,
            geometry.start_angle,
            geometry.mid_angle,
            geometry.end_angle,
            geometry.ccw_span,
            geometry.ccw_mid,
        ]);
        write_geometry_results_preserving_scalars([geometry.center])
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn gsp_point_on_three_point_arc(
        start_x: f64,
        start_y: f64,
        mid_x: f64,
        mid_y: f64,
        end_x: f64,
        end_y: f64,
        t: f64,
        complement: u32,
    ) -> u32 {
        let start = Point {
            x: start_x,
            y: start_y,
        };
        let mid = Point { x: mid_x, y: mid_y };
        let end = Point { x: end_x, y: end_y };
        let point = if complement != 0 {
            point_on_three_point_arc_complement(start, mid, end, t)
        } else {
            point_on_three_point_arc(start, mid, end, t)
        };
        write_geometry_results(point)
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn gsp_circle_arc_control_points(
        center_x: f64,
        center_y: f64,
        start_x: f64,
        start_y: f64,
        end_x: f64,
        end_y: f64,
        y_up: u32,
    ) -> u32 {
        write_geometry_results(
            circle_arc_control_points(
                Point {
                    x: center_x,
                    y: center_y,
                },
                Point {
                    x: start_x,
                    y: start_y,
                },
                Point { x: end_x, y: end_y },
                y_up != 0,
            )
            .into_iter()
            .flatten(),
        )
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn gsp_point_on_circle_arc(
        center_x: f64,
        center_y: f64,
        start_x: f64,
        start_y: f64,
        end_x: f64,
        end_y: f64,
        t: f64,
        y_up: u32,
    ) -> u32 {
        write_geometry_results(point_on_circle_arc(
            Point {
                x: center_x,
                y: center_y,
            },
            Point {
                x: start_x,
                y: start_y,
            },
            Point { x: end_x, y: end_y },
            t,
            y_up != 0,
        ))
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn gsp_project_to_three_point_arc(
        point_x: f64,
        point_y: f64,
        start_x: f64,
        start_y: f64,
        mid_x: f64,
        mid_y: f64,
        end_x: f64,
        end_y: f64,
    ) -> u32 {
        write_projection(project_to_three_point_arc(
            Point {
                x: point_x,
                y: point_y,
            },
            Point {
                x: start_x,
                y: start_y,
            },
            Point { x: mid_x, y: mid_y },
            Point { x: end_x, y: end_y },
        ))
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn gsp_project_to_circle_arc(
        point_x: f64,
        point_y: f64,
        center_x: f64,
        center_y: f64,
        start_x: f64,
        start_y: f64,
        end_x: f64,
        end_y: f64,
        y_up: u32,
    ) -> u32 {
        write_projection(project_to_circle_arc(
            Point {
                x: point_x,
                y: point_y,
            },
            Point {
                x: center_x,
                y: center_y,
            },
            Point {
                x: start_x,
                y: start_y,
            },
            Point { x: end_x, y: end_y },
            y_up != 0,
        ))
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn gsp_line_line_intersection(
        left_start_x: f64,
        left_start_y: f64,
        left_end_x: f64,
        left_end_y: f64,
        left_kind: u32,
        right_start_x: f64,
        right_start_y: f64,
        right_end_x: f64,
        right_end_y: f64,
        right_kind: u32,
    ) -> u32 {
        let (Some(left_kind), Some(right_kind)) = (
            line_kind_from_abi(left_kind),
            line_kind_from_abi(right_kind),
        ) else {
            return write_geometry_results(std::iter::empty());
        };
        write_geometry_results(line_line_intersection(
            Point {
                x: left_start_x,
                y: left_start_y,
            },
            Point {
                x: left_end_x,
                y: left_end_y,
            },
            left_kind,
            Point {
                x: right_start_x,
                y: right_start_y,
            },
            Point {
                x: right_end_x,
                y: right_end_y,
            },
            right_kind,
        ))
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn gsp_line_circle_intersections(
        start_x: f64,
        start_y: f64,
        end_x: f64,
        end_y: f64,
        line_kind: u32,
        center_x: f64,
        center_y: f64,
        radius: f64,
    ) -> u32 {
        let Some(line_kind) = line_kind_from_abi(line_kind) else {
            return write_geometry_results(std::iter::empty());
        };
        write_geometry_results(line_circle_intersections(
            Point {
                x: start_x,
                y: start_y,
            },
            Point { x: end_x, y: end_y },
            line_kind,
            Point {
                x: center_x,
                y: center_y,
            },
            radius,
        ))
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn gsp_circle_circle_intersections(
        left_x: f64,
        left_y: f64,
        left_radius: f64,
        right_x: f64,
        right_y: f64,
        right_radius: f64,
    ) -> u32 {
        write_geometry_results(circle_circle_intersections(
            Point {
                x: left_x,
                y: left_y,
            },
            left_radius,
            Point {
                x: right_x,
                y: right_y,
            },
            right_radius,
        ))
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn gsp_point_circle_tangents(
        point_x: f64,
        point_y: f64,
        center_x: f64,
        center_y: f64,
        radius: f64,
    ) -> u32 {
        write_geometry_results(point_circle_tangents(
            Point {
                x: point_x,
                y: point_y,
            },
            Point {
                x: center_x,
                y: center_y,
            },
            radius,
        ))
    }

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn gsp_compile_dependency_plan(ptr: u32, len: u32) -> u32 {
        if ptr == 0 || len == 0 {
            set_last_error("dependency plan input is empty");
            return 0;
        }
        // SAFETY: the caller owns an allocation of at least `len` bytes in this module's memory.
        let bytes = unsafe { std::slice::from_raw_parts(ptr as usize as *const u8, len as usize) };
        let nodes = match serde_json::from_slice::<Vec<DependencyNodeInput>>(bytes) {
            Ok(nodes) => nodes,
            Err(error) => {
                set_last_error(&format!("invalid dependency plan: {error}"));
                return 0;
            }
        };
        let plan = match DependencyPlan::build(&nodes) {
            Ok(plan) => plan,
            Err(error) => {
                set_last_error(&error.to_string());
                return 0;
            }
        };
        clear_last_error();
        DEPENDENCY_PLANS.with_borrow_mut(|plans| {
            plans.push(plan);
            plans.len() as u32
        })
    }

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn gsp_resolve_point_constraints(ptr: u32, len: u32) -> u32 {
        if ptr == 0 || len == 0 {
            set_last_error("point constraint input is empty");
            return write_batch_scalars(std::iter::empty());
        }
        // SAFETY: the caller owns an allocation of at least `len` bytes in this module's memory.
        let bytes = unsafe { std::slice::from_raw_parts(ptr as usize as *const u8, len as usize) };
        let results = match resolve_point_constraints_json(bytes) {
            Ok(results) => results,
            Err(error) => {
                set_last_error(&format!("invalid point constraint input: {error}"));
                return write_batch_scalars(std::iter::empty());
            }
        };
        clear_last_error();
        let count = results.len() as u32;
        write_batch_scalars(results.into_iter().flat_map(|point| match point {
            Some(point) => [point.x, point.y],
            None => [f64::NAN, f64::NAN],
        }));
        count
    }

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn gsp_evaluate_object_graph(ptr: u32, len: u32) -> u32 {
        if ptr == 0 || len == 0 {
            set_last_error("object graph input is empty");
            JSON_RESULT.with_borrow_mut(Vec::clear);
            return 0;
        }
        // SAFETY: the caller owns an allocation of at least `len` bytes in this module's memory.
        let bytes = unsafe { std::slice::from_raw_parts(ptr as usize as *const u8, len as usize) };
        match evaluate_object_graph_json(bytes) {
            Ok(encoded) => {
                clear_last_error();
                let len = encoded.len() as u32;
                JSON_RESULT.with_borrow_mut(|result| *result = encoded);
                len
            }
            Err(error) => {
                set_last_error(&error);
                JSON_RESULT.with_borrow_mut(Vec::clear);
                0
            }
        }
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn gsp_json_result_ptr() -> u32 {
        JSON_RESULT.with_borrow(|result| result.as_ptr() as usize as u32)
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn gsp_json_result_len() -> u32 {
        JSON_RESULT.with_borrow(|result| result.len() as u32)
    }

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn gsp_inverse_point_transform(ptr: u32, len: u32) -> u32 {
        if ptr == 0 || len == 0 {
            set_last_error("inverse point transform input is empty");
            return write_geometry_results(std::iter::empty());
        }
        // SAFETY: the caller owns an allocation of at least `len` bytes in this module's memory.
        let bytes = unsafe { std::slice::from_raw_parts(ptr as usize as *const u8, len as usize) };
        match inverse_point_transform_json(bytes) {
            Ok(point) => {
                clear_last_error();
                write_geometry_results(point)
            }
            Err(error) => {
                set_last_error(&format!("invalid inverse point transform input: {error}"));
                write_geometry_results(std::iter::empty())
            }
        }
    }

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn gsp_transform_points(ptr: u32, len: u32) -> u32 {
        if ptr == 0 || len == 0 {
            set_last_error("point transform input is empty");
            return write_batch_points(std::iter::empty());
        }
        // SAFETY: the caller owns an allocation of at least `len` bytes in this module's memory.
        let bytes = unsafe { std::slice::from_raw_parts(ptr as usize as *const u8, len as usize) };
        match transform_points_json(bytes) {
            Ok(Some(points)) => {
                clear_last_error();
                write_batch_points(points)
            }
            Ok(None) => {
                clear_last_error();
                write_batch_points(std::iter::empty())
            }
            Err(error) => {
                set_last_error(&format!("invalid point transform input: {error}"));
                write_batch_points(std::iter::empty())
            }
        }
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn gsp_dependency_topo_order(handle: u32) -> u32 {
        let order =
            with_dependency_plan(handle, |plan| plan.topo_order().to_vec()).unwrap_or_default();
        write_batch_scalars(order.into_iter().map(|index| index as f64))
    }

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn gsp_dependency_affected(
        handle: u32,
        roots_ptr: u32,
        roots_len: u32,
    ) -> u32 {
        if roots_ptr == 0 || roots_len == 0 {
            return write_batch_scalars(std::iter::empty());
        }
        // SAFETY: the caller owns an allocation of at least `roots_len` bytes in this module's memory.
        let bytes = unsafe {
            std::slice::from_raw_parts(roots_ptr as usize as *const u8, roots_len as usize)
        };
        let Ok(roots) = serde_json::from_slice::<Vec<String>>(bytes) else {
            return write_batch_scalars(std::iter::empty());
        };
        let affected =
            with_dependency_plan(handle, |plan| plan.affected(&roots)).unwrap_or_default();
        write_batch_scalars(affected.into_iter().map(|index| index as f64))
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn gsp_last_error_ptr() -> u32 {
        LAST_ERROR.with_borrow(|error| error.as_ptr() as usize as u32)
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn gsp_last_error_len() -> u32 {
        LAST_ERROR.with_borrow(|error| error.len() as u32)
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn gsp_sample_expression(
        handle: u32,
        x_min: f64,
        x_max: f64,
        sample_count: u32,
        plot_mode: u32,
    ) -> u32 {
        let plot_mode = match plot_mode {
            0 => PlotMode::Cartesian,
            1 => PlotMode::Polar,
            _ => return write_batch_scalars(std::iter::empty()),
        };
        let sampled = with_expression(handle, |compiled| {
            sample_expression(
                &compiled.expr,
                &compiled.parameters,
                x_min,
                x_max,
                sample_count as usize,
                plot_mode,
            )
        })
        .unwrap_or_default();
        write_optional_batch_points(sampled)
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn gsp_sample_parametric_curve(
        x_handle: u32,
        y_handle: u32,
        value_min: f64,
        value_max: f64,
        sample_count: u32,
    ) -> u32 {
        let sampled = EXPRESSIONS.with_borrow(|expressions| {
            let x_compiled = expressions.get(x_handle.saturating_sub(1) as usize)?;
            let y_compiled = expressions.get(y_handle.saturating_sub(1) as usize)?;
            Some(sample_parametric_curve(
                &x_compiled.expr,
                &y_compiled.expr,
                &x_compiled.parameters,
                &y_compiled.parameters,
                value_min,
                value_max,
                sample_count as usize,
            ))
        });
        write_batch_points(sampled.unwrap_or_default())
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn gsp_sample_coordinate_trace(
        x_handle: u32,
        y_handle: u32,
        x_parameter_index: u32,
        y_parameter_index: u32,
        source_x: f64,
        source_y: f64,
        value_min: f64,
        value_max: f64,
        sample_count: u32,
        use_midpoints: u32,
        mode: u32,
    ) -> u32 {
        let mode = match mode {
            0 => CoordinateTraceMode::Horizontal,
            1 => CoordinateTraceMode::Vertical,
            2 => CoordinateTraceMode::TwoDimensional,
            _ => return write_batch_scalars(std::iter::empty()),
        };
        let input = EXPRESSIONS.with_borrow(|expressions| {
            let x_compiled = expressions.get(x_handle.saturating_sub(1) as usize)?;
            let y_compiled = (y_handle != 0)
                .then(|| expressions.get(y_handle.saturating_sub(1) as usize))
                .flatten();
            Some((
                x_compiled.expr.clone(),
                y_compiled.map(|compiled| compiled.expr.clone()),
                x_compiled.parameters.clone(),
                y_compiled.map(|compiled| compiled.parameters.clone()),
                x_compiled
                    .parameter_names
                    .get(x_parameter_index as usize)
                    .cloned(),
                y_compiled.and_then(|compiled| {
                    compiled
                        .parameter_names
                        .get(y_parameter_index as usize)
                        .cloned()
                }),
            ))
        });
        let Some((x_expr, y_expr, x_parameters, y_parameters, x_name, y_name)) = input else {
            return write_batch_scalars(std::iter::empty());
        };
        write_batch_points(sample_coordinate_trace(
            &x_expr,
            y_expr.as_ref(),
            &x_parameters,
            y_parameters.as_ref(),
            x_name.as_deref(),
            y_name.as_deref(),
            Point {
                x: source_x,
                y: source_y,
            },
            value_min,
            value_max,
            sample_count as usize,
            use_midpoints != 0,
            mode,
        ))
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn gsp_sample_custom_transform_trace(
        distance_handle: u32,
        angle_handle: u32,
        origin_x: f64,
        origin_y: f64,
        axis_end_x: f64,
        axis_end_y: f64,
        value_min: f64,
        value_max: f64,
        trace_max: f64,
        sample_count: u32,
        distance_scale: f64,
        angle_degrees_scale: f64,
    ) -> u32 {
        let input = EXPRESSIONS.with_borrow(|expressions| {
            let distance = expressions.get(distance_handle.saturating_sub(1) as usize)?;
            let angle = expressions.get(angle_handle.saturating_sub(1) as usize)?;
            Some((
                distance.expr.clone(),
                angle.expr.clone(),
                distance.parameters.clone(),
                angle.parameters.clone(),
                distance.parameter_names.clone(),
                angle.parameter_names.clone(),
            ))
        });
        let Some((
            distance_expr,
            angle_expr,
            distance_parameters,
            angle_parameters,
            distance_names,
            angle_names,
        )) = input
        else {
            return write_batch_scalars(std::iter::empty());
        };
        write_batch_points(sample_custom_transform_trace(
            &distance_expr,
            &angle_expr,
            &distance_parameters,
            &angle_parameters,
            &distance_names,
            &angle_names,
            Point {
                x: origin_x,
                y: origin_y,
            },
            Point {
                x: axis_end_x,
                y: axis_end_y,
            },
            value_min,
            value_max,
            trace_max,
            sample_count as usize,
            distance_scale,
            angle_degrees_scale,
        ))
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn gsp_sample_circle_arc(
        center_x: f64,
        center_y: f64,
        start_x: f64,
        start_y: f64,
        end_x: f64,
        end_y: f64,
        steps: u32,
        y_up: u32,
    ) -> u32 {
        write_batch_points(
            sample_circle_arc(
                Point {
                    x: center_x,
                    y: center_y,
                },
                Point {
                    x: start_x,
                    y: start_y,
                },
                Point { x: end_x, y: end_y },
                steps as usize,
                y_up != 0,
            )
            .unwrap_or_default(),
        )
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn gsp_sample_three_point_arc(
        start_x: f64,
        start_y: f64,
        mid_x: f64,
        mid_y: f64,
        end_x: f64,
        end_y: f64,
        steps: u32,
        complement: u32,
    ) -> u32 {
        write_batch_points(
            sample_three_point_arc(
                Point {
                    x: start_x,
                    y: start_y,
                },
                Point { x: mid_x, y: mid_y },
                Point { x: end_x, y: end_y },
                steps as usize,
                complement != 0,
            )
            .unwrap_or_default(),
        )
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn gsp_translation_iteration_deltas(
        depth: u32,
        primary_dx: f64,
        primary_dy: f64,
        secondary_dx: f64,
        secondary_dy: f64,
        has_secondary: u32,
        bidirectional: u32,
        include_origin: u32,
    ) -> u32 {
        write_batch_points(translation_iteration_deltas(
            depth as usize,
            Point {
                x: primary_dx,
                y: primary_dy,
            },
            (has_secondary != 0).then_some(Point {
                x: secondary_dx,
                y: secondary_dy,
            }),
            bidirectional != 0,
            include_origin != 0,
        ))
    }

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn gsp_rotate_iteration_points(
        points_ptr: u32,
        point_count: u32,
        center_x: f64,
        center_y: f64,
        angle_radians: f64,
        depth: u32,
    ) -> u32 {
        // SAFETY: the caller owns an allocation containing `point_count` little-endian point pairs.
        let Some(points) = (unsafe { read_input_points(points_ptr, point_count) }) else {
            return write_batch_scalars(std::iter::empty());
        };
        write_batch_points(rotate_iteration_points(
            &points,
            Point {
                x: center_x,
                y: center_y,
            },
            angle_radians,
            depth as usize,
        ))
    }

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn gsp_affine_iteration_segment(
        points_ptr: u32,
        point_count: u32,
        depth: u32,
    ) -> u32 {
        // SAFETY: the caller owns an allocation containing eight little-endian point pairs.
        let Some(points) = (unsafe { read_input_points(points_ptr, point_count) }) else {
            return write_batch_scalars(std::iter::empty());
        };
        if points.len() != 8 {
            return write_batch_scalars(std::iter::empty());
        }
        write_batch_points(
            affine_iteration_segment(
                points[0],
                points[1],
                [points[2], points[3], points[4]],
                [points[5], points[6], points[7]],
                depth as usize,
            )
            .unwrap_or_default(),
        )
    }

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn gsp_branching_iteration_segments(
        points_ptr: u32,
        point_count: u32,
        depth: u32,
    ) -> u32 {
        // SAFETY: the caller owns an allocation containing seed and target point pairs.
        let Some(points) = (unsafe { read_input_points(points_ptr, point_count) }) else {
            return write_batch_scalars(std::iter::empty());
        };
        if points.len() < 4 || !(points.len() - 2).is_multiple_of(2) {
            return write_batch_scalars(std::iter::empty());
        }
        let target_segments = points[2..]
            .chunks_exact(2)
            .map(|segment| [segment[0], segment[1]])
            .collect::<Vec<_>>();
        write_batch_points(
            branching_iteration_segments(points[0], points[1], &target_segments, depth as usize)
                .unwrap_or_default(),
        )
    }

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn gsp_line_polyline_intersection(
        line_start_x: f64,
        line_start_y: f64,
        line_end_x: f64,
        line_end_y: f64,
        line_kind: u32,
        points_ptr: u32,
        point_count: u32,
        sample_hint: f64,
        variant: u32,
    ) -> u32 {
        let Some(line_kind) = line_kind_from_abi(line_kind) else {
            return write_geometry_results(std::iter::empty());
        };
        // SAFETY: the caller owns an allocation containing `point_count` little-endian point pairs.
        let Some(points) = (unsafe { read_input_points(points_ptr, point_count) }) else {
            return write_geometry_results(std::iter::empty());
        };
        write_geometry_results(line_polyline_intersection(
            Point {
                x: line_start_x,
                y: line_start_y,
            },
            Point {
                x: line_end_x,
                y: line_end_y,
            },
            line_kind,
            &points,
            sample_hint.is_finite().then_some(sample_hint),
            variant as usize,
        ))
    }

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn gsp_choose_point_candidate(
        points_ptr: u32,
        point_count: u32,
        reference_x: f64,
        reference_y: f64,
        has_reference: u32,
        variant: u32,
    ) -> u32 {
        // SAFETY: the caller owns an allocation containing `point_count` little-endian point pairs.
        let Some(points) = (unsafe { read_input_points(points_ptr, point_count) }) else {
            return write_geometry_results(std::iter::empty());
        };
        write_geometry_results(choose_point_candidate(
            &points,
            (has_reference != 0).then_some(Point {
                x: reference_x,
                y: reference_y,
            }),
            variant as usize,
        ))
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn gsp_line_circle_intersection_candidate(
        start_x: f64,
        start_y: f64,
        end_x: f64,
        end_y: f64,
        line_kind: u32,
        center_x: f64,
        center_y: f64,
        radius: f64,
        variant: u32,
    ) -> u32 {
        let Some(line_kind) = line_kind_from_abi(line_kind) else {
            return write_geometry_results(std::iter::empty());
        };
        write_geometry_results(line_circle_intersection_candidate(
            Point {
                x: start_x,
                y: start_y,
            },
            Point { x: end_x, y: end_y },
            line_kind,
            Point {
                x: center_x,
                y: center_y,
            },
            radius,
            variant as usize,
        ))
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn gsp_point_distance(
        left_x: f64,
        left_y: f64,
        right_x: f64,
        right_y: f64,
        value_scale: f64,
    ) -> f64 {
        point_distance(
            Point {
                x: left_x,
                y: left_y,
            },
            Point {
                x: right_x,
                y: right_y,
            },
            value_scale,
        )
        .unwrap_or(f64::NAN)
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn gsp_point_distance_ratio(
        origin_x: f64,
        origin_y: f64,
        denominator_x: f64,
        denominator_y: f64,
        numerator_x: f64,
        numerator_y: f64,
        clamp_to_unit: u32,
    ) -> f64 {
        point_distance_ratio(
            Point {
                x: origin_x,
                y: origin_y,
            },
            Point {
                x: denominator_x,
                y: denominator_y,
            },
            Point {
                x: numerator_x,
                y: numerator_y,
            },
            clamp_to_unit != 0,
        )
        .unwrap_or(f64::NAN)
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn gsp_point_angle_degrees(
        start_x: f64,
        start_y: f64,
        vertex_x: f64,
        vertex_y: f64,
        end_x: f64,
        end_y: f64,
    ) -> f64 {
        point_angle_degrees(
            Point {
                x: start_x,
                y: start_y,
            },
            Point {
                x: vertex_x,
                y: vertex_y,
            },
            Point { x: end_x, y: end_y },
        )
        .unwrap_or(f64::NAN)
    }

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn gsp_polygon_area(
        points_ptr: u32,
        point_count: u32,
        value_scale: f64,
    ) -> f64 {
        // SAFETY: the caller owns an allocation containing `point_count` little-endian point pairs.
        let Some(points) = (unsafe { read_input_points(points_ptr, point_count) }) else {
            return f64::NAN;
        };
        polygon_area(&points, value_scale).unwrap_or(f64::NAN)
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn gsp_batch_result_ptr() -> u32 {
        BATCH_RESULTS.with_borrow(|results| results.as_ptr() as usize as u32)
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn gsp_batch_result_len() -> u32 {
        BATCH_RESULTS.with_borrow(|results| results.len() as u32)
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn gsp_geometry_result_x(index: u32) -> f64 {
        geometry_result(index).map_or(f64::NAN, |point| point.x)
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn gsp_geometry_result_y(index: u32) -> f64 {
        geometry_result(index).map_or(f64::NAN, |point| point.y)
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn gsp_geometry_result_scalar(index: u32) -> f64 {
        GEOMETRY_SCALARS
            .with_borrow(|scalars| scalars.get(index as usize).copied().unwrap_or(f64::NAN))
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn gsp_alloc_bytes(len: u32) -> u32 {
        if len == 0 {
            return 0;
        }
        let bytes = vec![0_u8; len as usize].into_boxed_slice();
        Box::into_raw(bytes) as *mut u8 as usize as u32
    }

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn gsp_free_bytes(ptr: u32, len: u32) {
        if ptr == 0 || len == 0 {
            return;
        }
        let slice = std::ptr::slice_from_raw_parts_mut(ptr as usize as *mut u8, len as usize);
        // SAFETY: `ptr` and `len` were returned together by `gsp_alloc_bytes`.
        drop(unsafe { Box::from_raw(slice) });
    }

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn gsp_compile_expression(ptr: u32, len: u32) -> u32 {
        if ptr == 0 || len == 0 {
            return 0;
        }
        // SAFETY: the caller owns an allocation of at least `len` bytes in this module's memory.
        let bytes = unsafe { std::slice::from_raw_parts(ptr as usize as *const u8, len as usize) };
        let Ok(expr) = parse_expression_json(bytes) else {
            return 0;
        };
        let mut parameters = Vec::new();
        let mut seen = BTreeSet::new();
        collect_parameters(&expr, &mut seen, &mut parameters);
        let parameter_names = parameters
            .iter()
            .map(|(name, _)| name.clone())
            .collect::<Vec<_>>();
        let parameter_defaults = parameters
            .iter()
            .map(|(_, value)| *value)
            .collect::<Vec<_>>();
        let parameters = parameters.into_iter().collect();
        EXPRESSIONS.with_borrow_mut(|expressions| {
            expressions.push(CompiledExpression {
                expr,
                parameter_names,
                parameter_defaults,
                parameters,
            });
            expressions.len() as u32
        })
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn gsp_expression_parameter_count(handle: u32) -> u32 {
        with_expression(handle, |compiled| compiled.parameter_names.len() as u32).unwrap_or(0)
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn gsp_expression_parameter_name_ptr(handle: u32, index: u32) -> u32 {
        with_expression(handle, |compiled| {
            compiled
                .parameter_names
                .get(index as usize)
                .map_or(0, |name| name.as_bytes().as_ptr() as usize as u32)
        })
        .unwrap_or(0)
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn gsp_expression_parameter_name_len(handle: u32, index: u32) -> u32 {
        with_expression(handle, |compiled| {
            compiled
                .parameter_names
                .get(index as usize)
                .map_or(0, |name| name.len() as u32)
        })
        .unwrap_or(0)
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn gsp_expression_set_parameter(handle: u32, index: u32, value: f64) -> u32 {
        EXPRESSIONS.with_borrow_mut(|expressions| {
            let Some(compiled) = expressions.get_mut(handle.saturating_sub(1) as usize) else {
                return 0;
            };
            let Some(name) = compiled.parameter_names.get(index as usize) else {
                return 0;
            };
            let next_value = if value.is_finite() {
                value
            } else {
                compiled.parameter_defaults[index as usize]
            };
            let Some(slot) = compiled.parameters.get_mut(name) else {
                return 0;
            };
            *slot = next_value;
            1
        })
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn gsp_evaluate_expression(handle: u32, x: f64) -> f64 {
        with_expression(handle, |compiled| {
            evaluate_expr(&compiled.expr, x, &compiled.parameters).unwrap_or(f64::NAN)
        })
        .unwrap_or(f64::NAN)
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn gsp_evaluate_expression_with_driver(
        handle: u32,
        x: f64,
        driver_value: f64,
    ) -> f64 {
        with_expression(handle, |compiled| {
            let mut parameters = compiled.parameters.clone();
            for name in &compiled.parameter_names {
                parameters.insert(name.clone(), driver_value);
            }
            evaluate_expr(&compiled.expr, x, &parameters).unwrap_or(f64::NAN)
        })
        .unwrap_or(f64::NAN)
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn gsp_iterate_expression(
        handle: u32,
        parameter_index: u32,
        initial_value: f64,
        count: u32,
        x: f64,
    ) -> u32 {
        let values = EXPRESSIONS.with_borrow_mut(|expressions| {
            let Some(compiled) = expressions.get_mut(handle.saturating_sub(1) as usize) else {
                return Vec::new();
            };
            let parameter_name = compiled
                .parameter_names
                .get(parameter_index as usize)
                .cloned();
            let mut current = initial_value;
            let mut values = Vec::with_capacity(count as usize);
            for _ in 0..count {
                if let Some(name) = &parameter_name {
                    compiled.parameters.insert(name.clone(), current);
                }
                let Some(value) = evaluate_expr(&compiled.expr, x, &compiled.parameters) else {
                    break;
                };
                values.push(value);
                current = value;
            }
            values
        });
        write_batch_scalars(values)
    }

    fn with_expression<T>(handle: u32, f: impl FnOnce(&CompiledExpression) -> T) -> Option<T> {
        if handle == 0 {
            return None;
        }
        EXPRESSIONS.with_borrow(|expressions| expressions.get((handle - 1) as usize).map(f))
    }

    fn with_dependency_plan<T>(handle: u32, f: impl FnOnce(&DependencyPlan) -> T) -> Option<T> {
        if handle == 0 {
            return None;
        }
        DEPENDENCY_PLANS.with_borrow(|plans| plans.get((handle - 1) as usize).map(f))
    }

    fn set_last_error(message: &str) {
        LAST_ERROR.with_borrow_mut(|error| {
            error.clear();
            error.extend_from_slice(message.as_bytes());
        });
    }

    fn clear_last_error() {
        LAST_ERROR.with_borrow_mut(Vec::clear);
    }

    fn line_kind_from_abi(value: u32) -> Option<LineKind> {
        match value {
            0 => Some(LineKind::Segment),
            1 => Some(LineKind::Line),
            2 => Some(LineKind::Ray),
            _ => None,
        }
    }

    fn write_geometry_results(points: impl IntoIterator<Item = Point>) -> u32 {
        write_geometry_scalars(std::iter::empty());
        write_geometry_results_preserving_scalars(points)
    }

    fn write_geometry_results_preserving_scalars(points: impl IntoIterator<Item = Point>) -> u32 {
        GEOMETRY_RESULTS.with_borrow_mut(|results| {
            results.clear();
            results.extend(points.into_iter().take(3));
            results.len() as u32
        })
    }

    fn write_geometry_scalars(scalars: impl IntoIterator<Item = f64>) {
        GEOMETRY_SCALARS.with_borrow_mut(|results| {
            results.clear();
            results.extend(scalars.into_iter().take(8));
        });
    }

    fn write_projection(projection: Option<Projection>) -> u32 {
        let Some(projection) = projection else {
            return write_geometry_results(std::iter::empty());
        };
        write_geometry_scalars([projection.t, projection.distance_squared]);
        write_geometry_results_preserving_scalars([projection.projected])
    }

    fn write_batch_points(points: impl IntoIterator<Item = Point>) -> u32 {
        write_batch_scalars(points.into_iter().flat_map(|point| [point.x, point.y]))
    }

    fn write_optional_batch_points(points: impl IntoIterator<Item = Option<Point>>) -> u32 {
        write_batch_scalars(points.into_iter().flat_map(|point| match point {
            Some(point) => [point.x, point.y],
            None => [f64::NAN, f64::NAN],
        }))
    }

    fn write_batch_scalars(values: impl IntoIterator<Item = f64>) -> u32 {
        BATCH_RESULTS.with_borrow_mut(|results| {
            results.clear();
            results.extend(values);
            results.len() as u32
        })
    }

    unsafe fn read_input_points(ptr: u32, point_count: u32) -> Option<Vec<Point>> {
        if ptr == 0 || point_count == 0 {
            return None;
        }
        let byte_len = (point_count as usize).checked_mul(16)?;
        // SAFETY: upheld by the caller; bytes are decoded without relying on alignment.
        let bytes = unsafe { std::slice::from_raw_parts(ptr as usize as *const u8, byte_len) };
        Some(
            bytes
                .chunks_exact(16)
                .map(|chunk| Point {
                    x: f64::from_le_bytes(chunk[0..8].try_into().expect("point x is eight bytes")),
                    y: f64::from_le_bytes(chunk[8..16].try_into().expect("point y is eight bytes")),
                })
                .collect(),
        )
    }

    fn geometry_result(index: u32) -> Option<Point> {
        GEOMETRY_RESULTS.with_borrow(|results| results.get(index as usize).copied())
    }

    fn collect_parameters(
        expr: &FunctionExpr,
        seen: &mut BTreeSet<String>,
        parameters: &mut Vec<(String, f64)>,
    ) {
        if let FunctionExpr::Parsed(ast) = expr {
            collect_ast_parameters(ast, seen, parameters);
        }
    }

    fn collect_ast_parameters(
        expr: &FunctionAst,
        seen: &mut BTreeSet<String>,
        parameters: &mut Vec<(String, f64)>,
    ) {
        match expr {
            FunctionAst::Parameter(name, value) if seen.insert(name.clone()) => {
                parameters.push((name.clone(), *value));
            }
            FunctionAst::Unary { expr, .. } => collect_ast_parameters(expr, seen, parameters),
            FunctionAst::Binary { lhs, rhs, .. } => {
                collect_ast_parameters(lhs, seen, parameters);
                collect_ast_parameters(rhs, seen, parameters);
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_and_native_evaluation_share_tangent_guard() {
        let expr = parse_expression_json(
            br#"{"kind":"parsed","expr":{"kind":"unary","op":"tan","expr":{"kind":"variable"}}}"#,
        )
        .unwrap();
        assert_eq!(evaluate_expr(&expr, 0.0, &BTreeMap::new()), Some(0.0));
        assert_eq!(
            evaluate_expr(&expr, std::f64::consts::FRAC_PI_2, &BTreeMap::new()),
            None
        );
    }

    #[test]
    fn pi_angle_trig_input_is_degrees() {
        let expr = parse_expression_json(
            br#"{"kind":"parsed","expr":{"kind":"unary","op":"sin","expr":{"kind":"binary","lhs":{"kind":"pi-angle"},"op":"div","rhs":{"kind":"constant","value":2}}}}"#,
        )
        .unwrap();
        assert_eq!(evaluate_expr(&expr, 0.0, &BTreeMap::new()), Some(1.0));
    }
}
