mod decode;
mod eval;
mod expr;
mod plot;
mod scene;

pub(crate) use decode::{
    evaluate_function_group_with_overrides, try_decode_function_expr,
    try_decode_function_plot_descriptor,
};
pub(crate) use eval::evaluate_expr_with_parameters;
pub(crate) use expr::{
    BinaryOp, FunctionAst, FunctionExpr, FunctionPlotDescriptor, FunctionPlotMode, UnaryFunction,
    function_expr_label,
};
pub(crate) use plot::{
    collect_function_plot_domain, collect_function_plots, sample_plot_segments,
    synthesize_function_axes, synthesize_function_labels,
};
pub(crate) use scene::{collect_scene_functions, collect_scene_parameters, function_uses_pi_scale};

#[cfg(test)]
mod tests {
    use super::decode::{extract_inline_function_token, try_decode_inner_function_expr};
    use super::*;
    use crate::format::{GspFile, read_u16};
    use std::collections::BTreeMap;

    fn payload_from_words(words: &[u16]) -> Vec<u8> {
        words
            .iter()
            .flat_map(|word| word.to_le_bytes())
            .collect::<Vec<_>>()
    }

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
            try_decode_inner_function_expr(payload, &parameters).ok(),
            Some(FunctionExpr::Parsed(FunctionAst::Binary {
                lhs: Box::new(FunctionAst::Binary {
                    lhs: Box::new(FunctionAst::Binary {
                        lhs: Box::new(FunctionAst::Binary {
                            lhs: Box::new(FunctionAst::Binary {
                                lhs: Box::new(FunctionAst::Binary {
                                    lhs: Box::new(FunctionAst::Unary {
                                        op: UnaryFunction::Abs,
                                        expr: Box::new(FunctionAst::Variable),
                                    }),
                                    op: BinaryOp::Add,
                                    rhs: Box::new(FunctionAst::Unary {
                                        op: UnaryFunction::Sqrt,
                                        expr: Box::new(FunctionAst::Variable),
                                    }),
                                }),
                                op: BinaryOp::Add,
                                rhs: Box::new(FunctionAst::Unary {
                                    op: UnaryFunction::Ln,
                                    expr: Box::new(FunctionAst::Variable),
                                }),
                            }),
                            op: BinaryOp::Add,
                            rhs: Box::new(FunctionAst::Unary {
                                op: UnaryFunction::Log10,
                                expr: Box::new(FunctionAst::Variable),
                            }),
                        }),
                        op: BinaryOp::Add,
                        rhs: Box::new(FunctionAst::Unary {
                            op: UnaryFunction::Sign,
                            expr: Box::new(FunctionAst::Variable),
                        }),
                    }),
                    op: BinaryOp::Add,
                    rhs: Box::new(FunctionAst::Unary {
                        op: UnaryFunction::Round,
                        expr: Box::new(FunctionAst::Variable),
                    }),
                }),
                op: BinaryOp::Add,
                rhs: Box::new(FunctionAst::Unary {
                    op: UnaryFunction::Trunc,
                    expr: Box::new(FunctionAst::Variable),
                }),
            }))
        );
        let expr = try_decode_function_expr(&file, &groups, function_group).ok();
        assert_eq!(
            expr,
            try_decode_inner_function_expr(payload, &parameters).ok()
        );
    }

    #[test]
    fn decodes_nested_function_expr_in_circle_formation_fixture() {
        let data = include_bytes!("../../tests/fixtures/圆的形成.gsp");
        let file = GspFile::parse(data).expect("fixture parses");
        let groups = file.object_groups();
        let function_group = groups
            .iter()
            .find(|group| {
                group.header.kind() == crate::format::GroupKind::FunctionExpr
                    && group
                        .records
                        .iter()
                        .find(|record| record.record_type == 0x0907)
                        .is_some_and(|record| {
                            let payload = record.payload(&file.data);
                            payload.len() >= 16
                                && read_u16(payload, 12) == 322
                                && read_u16(payload, 14) == 420
                        })
            })
            .expect("nested function group");
        assert_eq!(
            try_decode_function_expr(&file, &groups, function_group).ok(),
            Some(FunctionExpr::Parsed(FunctionAst::Binary {
                lhs: Box::new(FunctionAst::Parameter("t₁".to_string(), 5.0)),
                op: BinaryOp::Add,
                rhs: Box::new(FunctionAst::Constant(1.0)),
            }))
        );
        assert_eq!(
            function_expr_label(
                try_decode_function_expr(&file, &groups, function_group).expect("expression")
            ),
            "t₁ + 1"
        );
    }

    #[test]
    fn decodes_angle_function_expr_in_circle_formation_fixture() {
        let data = include_bytes!("../../tests/fixtures/圆的形成.gsp");
        let file = GspFile::parse(data).expect("fixture parses");
        let groups = file.object_groups();
        let function_group = groups
            .iter()
            .find(|group| {
                group.header.kind() == crate::format::GroupKind::FunctionExpr
                    && group
                        .records
                        .iter()
                        .find(|record| record.record_type == 0x0907)
                        .is_some_and(|record| {
                            let payload = record.payload(&file.data);
                            payload.len() >= 16
                                && read_u16(payload, 12) == 322
                                && read_u16(payload, 14) == 362
                        })
            })
            .expect("angle function group");
        let expr = try_decode_function_expr(&file, &groups, function_group).expect("expression");
        assert_eq!(
            expr,
            FunctionExpr::Parsed(FunctionAst::Binary {
                lhs: Box::new(FunctionAst::Binary {
                    lhs: Box::new(FunctionAst::Constant(2.0)),
                    op: BinaryOp::Mul,
                    rhs: Box::new(FunctionAst::PiAngle),
                }),
                op: BinaryOp::Div,
                rhs: Box::new(FunctionAst::Parameter("t₂".to_string(), 5.0)),
            })
        );
        assert_eq!(function_expr_label(expr), "2*180 / t₂");
    }

    #[test]
    fn decodes_marker_based_function_expr_with_structured_parser() {
        let payload = payload_from_words(&[0x0094, 0x0001, 0x2006, 0x000f, 0x000c, 0x1000, 0x0002]);

        assert_eq!(
            try_decode_inner_function_expr(&payload, &BTreeMap::new()).ok(),
            Some(FunctionExpr::Parsed(FunctionAst::Binary {
                lhs: Box::new(FunctionAst::Unary {
                    op: UnaryFunction::Abs,
                    expr: Box::new(FunctionAst::Variable),
                }),
                op: BinaryOp::Add,
                rhs: Box::new(FunctionAst::Constant(2.0)),
            }))
        );
    }

    #[test]
    fn decodes_fallback_function_expr_with_ignorable_suffix() {
        let payload = payload_from_words(&[0xffff, 0x000f, 0x1000, 0x0001, 0x0101]);

        assert_eq!(
            try_decode_inner_function_expr(&payload, &BTreeMap::new()).ok(),
            Some(FunctionExpr::Parsed(FunctionAst::Binary {
                lhs: Box::new(FunctionAst::Variable),
                op: BinaryOp::Add,
                rhs: Box::new(FunctionAst::Constant(1.0)),
            }))
        );
    }

    #[test]
    fn decodes_hidden_function_exprs_in_changing_polyline_lyg1_fixture() {
        let data = include_bytes!("../../tests/Samples/个人专栏/李有贵作品/变化的折线（lyg)1.gsp");
        let file = GspFile::parse(data).expect("fixture parses");
        let groups = file.object_groups();

        let expr_a = try_decode_function_expr(&file, &groups, &groups[5]).expect("expression #6");
        assert_eq!(
            expr_a,
            FunctionExpr::Parsed(FunctionAst::Binary {
                lhs: Box::new(FunctionAst::Parameter("m₁".to_string(), 4.613963872539072)),
                op: BinaryOp::Sub,
                rhs: Box::new(FunctionAst::Unary {
                    op: UnaryFunction::Trunc,
                    expr: Box::new(FunctionAst::Parameter("m₁".to_string(), 4.613963872539072)),
                }),
            })
        );

        let expr_b = try_decode_function_expr(&file, &groups, &groups[6]).expect("expression #7");
        assert_eq!(
            expr_b,
            FunctionExpr::Parsed(FunctionAst::Binary {
                lhs: Box::new(FunctionAst::Constant(0.0)),
                op: BinaryOp::Div,
                rhs: Box::new(FunctionAst::Unary {
                    op: UnaryFunction::Sign,
                    expr: Box::new(FunctionAst::Binary {
                        lhs: Box::new(FunctionAst::Unary {
                            op: UnaryFunction::Sign,
                            expr: Box::new(FunctionAst::Binary {
                                lhs: Box::new(FunctionAst::Constant(1.0)),
                                op: BinaryOp::Sub,
                                rhs: Box::new(FunctionAst::Parameter(
                                    "m₁".to_string(),
                                    4.613963872539072,
                                )),
                            }),
                        }),
                        op: BinaryOp::Add,
                        rhs: Box::new(FunctionAst::Constant(1.0)),
                    }),
                }),
            })
        );
    }

    #[test]
    fn decodes_chessboard_yougui_x_expr_from_fixture() {
        let data = include_bytes!("../../tests/Samples/个人专栏/李有贵作品/棋盘（有贵）.gsp");
        let file = GspFile::parse(data).expect("fixture parses");
        let groups = file.object_groups();

        let expr = try_decode_function_expr(&file, &groups, &groups[11]).expect("expression #12");
        assert_eq!(
            evaluate_expr_with_parameters(
                &expr,
                0.0,
                &BTreeMap::from([
                    ("t₁".to_string(), 0.0),
                    ("trunc((m₁ + 2))".to_string(), 9.0),
                    ("n".to_string(), 9.0),
                ]),
            ),
            Some(0.0)
        );
    }
}
