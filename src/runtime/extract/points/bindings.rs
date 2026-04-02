use super::super::decode::decode_label_name;
use super::anchors::{
    decode_reflection_anchor_raw, reflection_line_group_indices,
    translation_point_pair_group_indices,
};
use super::constraints::{
    CoordinatePoint, ParameterControlledPoint, RawPointConstraint, decode_coordinate_point,
    decode_parameter_controlled_point, decode_point_constraint, decode_translated_point_constraint,
    regular_polygon_iteration_step,
};
use super::*;
use crate::runtime::functions::FunctionExpr;
use crate::runtime::geometry::rotate_around;
use crate::runtime::scene::{LineBinding, ShapeBinding};

#[path = "bindings/decode.rs"]
mod decode;
#[path = "bindings/iterations.rs"]
mod iterations;
#[path = "bindings/remap.rs"]
mod remap;
#[path = "bindings/visible_points.rs"]
mod visible_points;

pub(crate) use decode::{decode_parameter_rotation_binding, decode_transform_binding};
pub(crate) use iterations::collect_point_iteration_points;
pub(crate) use remap::{
    remap_circle_bindings, remap_label_bindings, remap_line_bindings, remap_polygon_bindings,
};
pub(crate) use visible_points::collect_visible_points;

pub(crate) struct TransformBinding {
    pub(crate) source_group_index: usize,
    pub(crate) center_group_index: usize,
    pub(crate) kind: TransformBindingKind,
}

pub(crate) enum TransformBindingKind {
    Rotate { angle_degrees: f64 },
    Scale { factor: f64 },
}

fn iteration_depth(file: &GspFile, group: &ObjectGroup, default_depth: usize) -> usize {
    group
        .records
        .iter()
        .find(|record| record.record_type == 0x090a)
        .map(|record| record.payload(&file.data))
        .filter(|payload| payload.len() >= 20)
        .map(|payload| read_u32(payload, 16) as usize)
        .unwrap_or(default_depth)
}

pub(crate) enum RawPointIterationFamily {
    Offset {
        seed_index: usize,
        dx: f64,
        dy: f64,
        depth: usize,
        parameter_name: Option<String>,
    },
    RotateChain {
        seed_index: usize,
        center_index: usize,
        angle_degrees: f64,
        depth: usize,
    },
    Rotate {
        source_index: usize,
        center_index: usize,
        angle_expr: FunctionExpr,
        depth: usize,
        parameter_name: Option<String>,
    },
}
