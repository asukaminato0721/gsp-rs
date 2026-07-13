//! Canonical mathematical semantics shared by the native parser and browser runtime.

mod geometry;

pub use geometry::{
    Bounds, LineKind, Point, Projection, ThreePointArcGeometry, angle_bisector_direction,
    circle_arc_control_points, circle_circle_intersections, clip_line_to_bounds,
    clip_ray_to_bounds, lerp_point, line_circle_intersections, line_line_intersection,
    measured_rotation_radians, normalize_angle_delta, point_circle_tangents, point_on_circle_arc,
    point_on_three_point_arc, point_on_three_point_arc_complement, project_to_circle_arc,
    project_to_line_like, project_to_three_point_arc, reflect_across_line, rotate_around,
    scale_around, scale_by_three_point_ratio, three_point_arc_geometry,
};

use std::collections::BTreeMap;

#[cfg(any(test, target_arch = "wasm32"))]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(any(test, target_arch = "wasm32"), derive(Deserialize))]
#[cfg_attr(any(test, target_arch = "wasm32"), serde(rename_all = "lowercase"))]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Pow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(any(test, target_arch = "wasm32"), derive(Deserialize))]
#[cfg_attr(any(test, target_arch = "wasm32"), serde(rename_all = "lowercase"))]
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

fn evaluate_ast(expr: &FunctionAst, x: f64, parameters: &BTreeMap<String, f64>) -> Option<f64> {
    let value = match expr {
        FunctionAst::Variable => x,
        FunctionAst::Constant(value) => *value,
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
        FunctionAst::Variable | FunctionAst::Constant(_) | FunctionAst::Parameter(_, _) => false,
    }
}

#[cfg(any(test, target_arch = "wasm32"))]
#[derive(Deserialize)]
#[serde(untagged)]
enum ExpressionInput {
    Expression(WireFunctionExpr),
    Ast(WireFunctionAst),
}

#[cfg(any(test, target_arch = "wasm32"))]
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

#[cfg(any(test, target_arch = "wasm32"))]
#[derive(Deserialize)]
#[serde(tag = "kind")]
enum WireFunctionAst {
    #[serde(rename = "variable")]
    Variable,
    #[serde(rename = "constant")]
    Constant { value: f64 },
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

#[cfg(any(test, target_arch = "wasm32"))]
impl From<WireFunctionExpr> for FunctionExpr {
    fn from(value: WireFunctionExpr) -> Self {
        match value {
            WireFunctionExpr::Constant { value } => Self::Constant(value),
            WireFunctionExpr::Identity => Self::Identity,
            WireFunctionExpr::Parsed { expr } => Self::Parsed(expr.into()),
        }
    }
}

#[cfg(any(test, target_arch = "wasm32"))]
impl From<WireFunctionAst> for FunctionAst {
    fn from(value: WireFunctionAst) -> Self {
        match value {
            WireFunctionAst::Variable => Self::Variable,
            WireFunctionAst::Constant { value } => Self::Constant(value),
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

#[cfg(any(test, target_arch = "wasm32"))]
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
        Bounds, FunctionAst, FunctionExpr, LineKind, Point, Projection, angle_bisector_direction,
        circle_arc_control_points, circle_circle_intersections, clip_line_to_bounds,
        clip_ray_to_bounds, evaluate_expr, lerp_point, line_circle_intersections,
        line_line_intersection, measured_rotation_radians, normalize_angle_delta,
        parse_expression_json, point_circle_tangents, point_on_circle_arc,
        point_on_three_point_arc, point_on_three_point_arc_complement, project_to_circle_arc,
        project_to_line_like, project_to_three_point_arc, reflect_across_line, rotate_around,
        scale_around, scale_by_three_point_ratio, three_point_arc_geometry,
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
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn gsp_runtime_abi_version() -> u32 {
        3
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

    fn with_expression<T>(handle: u32, f: impl FnOnce(&CompiledExpression) -> T) -> Option<T> {
        if handle == 0 {
            return None;
        }
        EXPRESSIONS.with_borrow(|expressions| expressions.get((handle - 1) as usize).map(f))
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
