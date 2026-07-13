pub(crate) const RECORD_INDEXED_PATH_A: u32 = 0x07d2;
pub(crate) const RECORD_INDEXED_PATH_B: u32 = 0x07d3;
pub(crate) const RECORD_BINDING_PAYLOAD: u32 = 0x07d3;
pub(crate) const RECORD_LABEL_AUX: u32 = 0x07d5;
pub(crate) const RECORD_LABEL_VISIBILITY: u32 = 0x07d5;
pub(crate) const RECORD_PATH_POINT_AUX: u32 = 0x07d8;
pub(crate) const RECORD_BBOX_A: u32 = 0x0898;
pub(crate) const RECORD_POINT_F64_PAIR: u32 = 0x0899;
pub(crate) const RECORD_BBOX_B: u32 = 0x08a2;
pub(crate) const RECORD_BBOX_C: u32 = 0x08a3;
pub(crate) const RECORD_IMAGE_TRANSFORM: u32 = 0x08a8;
pub(crate) use crate::format::{
    RECORD_ITERATION_DEFINITION, RECORD_RICH_TEXT, RECORD_VALUE_TABLE_LAYOUT,
};
pub(crate) const RECORD_RICH_TEXT_MAGIC: u32 = RECORD_RICH_TEXT;
pub(crate) const RECORD_ACTION_BUTTON_PAYLOAD: u32 = 0x0906;
pub(crate) const RECORD_FUNCTION_EXPR_PAYLOAD: u32 = 0x0907;
pub(crate) const RECORD_FUNCTION_PLOT_DESCRIPTOR: u32 = 0x0902;
pub(crate) const RECORD_ACTION_AUX: u32 = 0x0903;
pub(crate) const RECORD_IMAGE_SIZE: u32 = 0x090c;
pub(crate) const RECORD_ANGLE_MARKER_CLASS: u32 = 0x090e;
pub(crate) const RECORD_SEGMENT_MARKER_PAYLOAD: u32 = 0x090f;
pub(crate) const RECORD_IMAGE_RESOURCE: u32 = 0x1f44;

pub(crate) const EXPRESSION_TRANSFORM_SCALE_CLASS: u16 = 0;
pub(crate) const EXPRESSION_TRANSFORM_ROTATE_CLASS: u16 = 1;
pub(crate) const EXPRESSION_TRANSFORM_MARKED_SCALE_CLASS: u16 = 5;
pub(crate) const EXPRESSION_TRANSFORM_CALCULATED_SCALE_CLASS: u16 = 7;

pub(crate) const FUNCTION_EXPR_MARKER_A: [u16; 2] = [0x0094, 0x0001];
pub(crate) const FUNCTION_EXPR_MARKER_B: [u16; 2] = [0x00a0, 0x0001];
pub(crate) const EXPR_OP_ADD: u16 = 0x1000;
pub(crate) const EXPR_OP_SUB: u16 = 0x1001;
pub(crate) const EXPR_OP_MUL: u16 = 0x1002;
pub(crate) const EXPR_OP_DIV: u16 = 0x1003;
pub(crate) const EXPR_OP_POW: u16 = 0x1004;
pub(crate) const EXPR_PI_WORD: u16 = 0x000d;
pub(crate) const EXPR_PI_SUFFIX: u16 = 0x0100;
pub(crate) const EXPR_VARIABLE_WORD: u16 = 0x000f;
pub(crate) const EXPR_VARIABLE_SUFFIX: u16 = 0x000c;
pub(crate) const EXPR_PARAMETER_MASK: u16 = 0xfff0;
pub(crate) const EXPR_PARAMETER_PREFIX: u16 = 0x6000;
