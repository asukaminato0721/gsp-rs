mod decode;
mod eval;
mod expr;
mod plot;
mod scene;

pub(crate) use decode::{decode_function_expr, decode_function_plot_descriptor};
pub(crate) use eval::{evaluate_expr_with_parameters, sample_function_points};
pub(crate) use expr::{
    BinaryOp, FunctionExpr, FunctionPlotDescriptor, FunctionTerm, ParsedFunctionExpr,
    UnaryFunction, function_expr_label,
};
pub(crate) use plot::{
    collect_function_plot_domain, collect_function_plots, synthesize_function_axes,
    synthesize_function_labels,
};
pub(crate) use scene::{collect_scene_functions, collect_scene_parameters, function_uses_pi_scale};

#[cfg(test)]
mod tests {
    use super::*;
    use super::decode::{decode_inner_function_expr, extract_inline_function_token};
    use crate::format::GspFile;
    use std::collections::BTreeMap;

    #[test]
    fn extracts_simple_function_token() {
        assert_eq!(
            extract_inline_function_token(b"\0\0<0>\0"),
            Some("0".to_string())
        );
        assert_eq!(
            extract_inline_function_token(b"junk<x>tail"),
            Some("x".to_string())
        );
    }

    #[test]
    fn decodes_f_gsp_function_expr() {
        let data = include_bytes!("../../../f.gsp");
        let file = GspFile::parse(data).expect("fixture parses");
        let groups = file.object_groups();
        let function_group = groups
            .iter()
            .find(|group| {
                group
                    .records
                    .iter()
                    .any(|record| record.record_type == 0x0907)
            })
            .expect("function group");
        let payload = function_group
            .records
            .iter()
            .find(|record| record.record_type == 0x0907)
            .expect("0907 record")
            .payload(&file.data);
        let parameters = BTreeMap::new();
        assert_eq!(
            decode_inner_function_expr(payload, &parameters),
            Some(FunctionExpr::Parsed(ParsedFunctionExpr {
                head: FunctionTerm::UnaryX(UnaryFunction::Abs),
                tail: vec![
                    (BinaryOp::Add, FunctionTerm::UnaryX(UnaryFunction::Sqrt)),
                    (BinaryOp::Add, FunctionTerm::UnaryX(UnaryFunction::Ln)),
                    (BinaryOp::Add, FunctionTerm::UnaryX(UnaryFunction::Log10)),
                    (BinaryOp::Add, FunctionTerm::UnaryX(UnaryFunction::Sign)),
                    (BinaryOp::Add, FunctionTerm::UnaryX(UnaryFunction::Round)),
                    (BinaryOp::Add, FunctionTerm::UnaryX(UnaryFunction::Trunc)),
                ],
            }))
        );
        let expr = decode_function_expr(&file, &groups, function_group);
        assert_eq!(
            expr,
            Some(FunctionExpr::Parsed(ParsedFunctionExpr {
                head: FunctionTerm::UnaryX(UnaryFunction::Abs),
                tail: vec![
                    (BinaryOp::Add, FunctionTerm::UnaryX(UnaryFunction::Sqrt)),
                    (BinaryOp::Add, FunctionTerm::UnaryX(UnaryFunction::Ln)),
                    (BinaryOp::Add, FunctionTerm::UnaryX(UnaryFunction::Log10)),
                    (BinaryOp::Add, FunctionTerm::UnaryX(UnaryFunction::Sign)),
                    (BinaryOp::Add, FunctionTerm::UnaryX(UnaryFunction::Round)),
                    (BinaryOp::Add, FunctionTerm::UnaryX(UnaryFunction::Trunc)),
                ],
            }))
        );
    }
}
