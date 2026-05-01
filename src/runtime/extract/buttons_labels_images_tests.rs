use super::test_support::{
    fixture_buttons_without_validation, fixture_bytes, fixture_images_without_validation,
    fixture_labels_without_validation, fixture_log, fixture_scene, function_expr_has_unary,
};
use crate::runtime::functions::UnaryFunction;
use crate::runtime::scene::{ButtonAction, ShapeBinding, ShapeTransformBinding, TextLabelBinding};

#[test]
fn collects_button_visibility_targets_for_show_hide_line_segment_controls() {
    let Some(data) =
        fixture_bytes("tests/Samples/个人专栏/李忠平作品/金华2010-24题(百年孤独)10.8.9.gsp")
    else {
        return;
    };
    let buttons = fixture_buttons_without_validation(&data);

    let hide_line = buttons
        .iter()
        .find(|button| button.text == "隐藏线段")
        .expect("expected hide-line button");
    match &hide_line.action {
        ButtonAction::SetVisibility {
            visible,
            button_indices,
            ..
        } => {
            assert!(!visible, "expected hide-line button to hide its targets");
            assert_eq!(
                button_indices.len(),
                3,
                "expected hide-line payload to target the three line-control buttons"
            );
        }
        action => panic!("expected set-visibility action, got {action:?}"),
    }

    let show_line = buttons
        .iter()
        .find(|button| button.text == "显示线段")
        .expect("expected show-line button");
    match &show_line.action {
        ButtonAction::SetVisibility {
            visible,
            button_indices,
            ..
        } => {
            assert!(*visible, "expected show-line button to show its targets");
            assert_eq!(
                button_indices.len(),
                3,
                "expected show-line payload to target the same three line-control buttons"
            );
        }
        action => panic!("expected set-visibility action, got {action:?}"),
    }
}

#[test]
fn preserves_show_image_button_in_wuxi_fixture() {
    let Some(data) =
        fixture_bytes("tests/Samples/个人专栏/李忠平作品/2011中考江苏无锡第26题(百年孤独)简化.gsp")
    else {
        return;
    };
    let scene = fixture_scene(&data);

    assert_eq!(scene.images.len(), 1, "expected one exported image");
    let show_image = scene
        .buttons
        .iter()
        .find(|button| button.text == "显示图片")
        .expect("expected show-image button");
    match &show_image.action {
        ButtonAction::SetVisibility {
            visible,
            image_indices,
            point_indices,
            ..
        } => {
            assert!(*visible, "expected the image button to show its target");
            assert_eq!(image_indices.as_slice(), &[0]);
            assert!(
                point_indices.is_empty(),
                "expected image visibility to target the exported image, not a point fallback"
            );
        }
        action => panic!("expected set-visibility action, got {action:?}"),
    }
}

#[test]
fn collects_label_visibility_button_without_validation() {
    let Some(data) = fixture_bytes("tests/Samples/个人专栏/向忠作品/正弦波与音乐.gsp")
    else {
        return;
    };
    let buttons = fixture_buttons_without_validation(&data);

    let hide_help = buttons
        .iter()
        .find(|button| button.text == "隐藏说明")
        .expect("expected hide-help button");
    match &hide_help.action {
        ButtonAction::SetVisibility {
            visible,
            label_indices,
            ..
        } => {
            assert!(!visible, "expected hide-help button to hide its labels");
            assert_eq!(label_indices.len(), 2);
        }
        action => panic!("expected set-visibility action, got {action:?}"),
    }
}

#[test]
fn collects_hjx4882_subtraction_text_visibility_targets() {
    let path = "tests/Samples/个人专栏/贺基旭作品/10以内的减法（hjx4882）.gsp";
    let Some(data) = fixture_bytes(path) else {
        return;
    };
    let scene = fixture_scene(&data);
    let log = fixture_log(&data, path);

    assert!(
        log.contains("{102} Function(1479,140,'','A B + x - @sgn_ 1 + 2 / ')(100,101)[hidden]"),
        "expected the subtraction membership function to come from the payload"
    );

    assert_eq!(
        scene
            .circles
            .iter()
            .filter(|circle| circle.fill_color.is_some())
            .count(),
        20,
        "expected every visible Circle interior payload to export as a filled disk"
    );
    assert_eq!(
        scene
            .circles
            .iter()
            .filter(|circle| !circle.visible && circle.fill_color.is_some() && circle.fill_visible)
            .count(),
        20,
        "hidden helper circle outlines should not suppress their visible circle interiors"
    );
    assert!(
        scene.circles.len() >= 22,
        "expected the two large circles plus the filled helper circles"
    );
    let translated_circle = scene
        .circles
        .iter()
        .find(|circle| {
            circle
                .debug
                .as_ref()
                .is_some_and(|debug| debug.group_ordinal == 7)
        })
        .expect("expected VectorTranslation(4,2,6) to export the cyan minuend circle");
    assert_eq!(
        translated_circle.color,
        [0, 255, 255, 255],
        "translated circle should use the payload color on group #7"
    );
    assert!(translated_circle.visible);
    assert_eq!(translated_circle.fill_color, None);
    match &translated_circle.binding {
        Some(ShapeBinding::DerivedTransform {
            source_index,
            transform:
                ShapeTransformBinding::TranslateVector {
                    vector_start_index,
                    vector_end_index,
                },
        }) => {
            assert_eq!(
                *source_index, 0,
                "group #7 should translate the first exported source circle"
            );
            let vector_group_ordinals =
                [*vector_start_index, *vector_end_index].map(|point_index| {
                    scene.points[point_index]
                        .debug
                        .as_ref()
                        .expect("translation vector point should keep payload debug source")
                        .group_ordinal
                });
            assert_eq!(
                vector_group_ordinals,
                [2, 6],
                "group #7 should use vector #2 -> #6"
            );
        }
        binding => panic!("expected vector translation binding, got {binding:?}"),
    }

    let function_value = scene
        .labels
        .iter()
        .find(|label| {
            label
                .debug
                .as_ref()
                .is_some_and(|debug| debug.group_ordinal == 103)
        })
        .expect("expected f(1A) expression value label");
    assert_eq!(function_value.text, "f(1A) = 0");
    match &function_value.binding {
        Some(TextLabelBinding::ExpressionValue {
            expr, expr_label, ..
        }) => {
            assert_eq!(expr_label, "f(1A)");
            assert!(
                function_expr_has_unary(expr, UnaryFunction::Sign),
                "expected the function payload to decode the @sgn_ operation"
            );
        }
        binding => panic!("expected expression value for f(1A), got {binding:?}"),
    }
    let minuend_sum = scene
        .labels
        .iter()
        .find(|label| {
            label
                .debug
                .as_ref()
                .is_some_and(|debug| debug.group_ordinal == 113)
        })
        .expect("expected displayed minuend sum calculation");
    assert_eq!(
        minuend_sum.text,
        "f(1A) + f(2A) + f(3A) + f(4A) + f(5A) + f(6A) + f(7A) + f(8A) + f(9A) + f(10A) = 6"
    );
    let subtrahend_sum = scene
        .labels
        .iter()
        .find(|label| {
            label
                .debug
                .as_ref()
                .is_some_and(|debug| debug.group_ordinal == 134)
        })
        .expect("expected displayed subtrahend sum calculation");
    assert_eq!(
        subtrahend_sum.text,
        "f(B1') + f(B2') + f(B3') + f(B4') + f(B5') + f(B6') + f(B7') + f(B8') + f(B9') + f(B10') = 5"
    );
    let difference = scene
        .labels
        .iter()
        .find(|label| {
            label
                .debug
                .as_ref()
                .is_some_and(|debug| debug.group_ordinal == 136)
        })
        .expect("expected displayed subtraction result calculation");
    assert_eq!(difference.text, "被减数 - 减数 = 1");
    let headline_equation = scene
        .labels
        .iter()
        .find(|label| label.visible && label.text.contains("6 － 5 = 1"))
        .expect("expected visible subtraction headline equation");
    match &headline_equation.binding {
        Some(TextLabelBinding::RichTextExpressionValues { refs, .. }) => {
            assert_eq!(
                refs.iter()
                    .map(|reference| (reference.slot, reference.source_group_ordinal))
                    .collect::<Vec<_>>(),
                vec![(1, 113), (2, 134), (3, 136)],
                "headline equation should follow the rich-text payload links to the calculations"
            );
        }
        binding => panic!("expected rich-text expression binding for headline, got {binding:?}"),
    }

    let show_text = scene
        .buttons
        .iter()
        .find(|button| button.text == "显示文本对象")
        .expect("expected text visibility button");
    match &show_text.action {
        ButtonAction::ShowHideVisibility {
            label_indices,
            point_indices,
            ..
        } => {
            assert!(
                point_indices.is_empty(),
                "text visibility should target labels, not point fallbacks"
            );
            let target_group_ordinals = label_indices
                .iter()
                .filter_map(|index| scene.labels.get(*index))
                .filter_map(|label| label.debug.as_ref())
                .map(|debug| debug.group_ordinal)
                .collect::<Vec<_>>();
            for expected in [90, 113, 134] {
                assert!(
                    target_group_ordinals.contains(&expected),
                    "expected text visibility payload to target calculation label group #{expected}"
                );
            }
            for label_index in label_indices {
                assert!(
                    !scene.labels[*label_index].visible,
                    "payload-hidden text group should stay hidden until the button is used"
                );
            }
        }
        action => panic!("expected show-hide visibility action, got {action:?}"),
    }
}

#[test]
fn collects_play_function_button_without_validation() {
    let Some(data) = fixture_bytes("tests/Samples/个人专栏/向忠作品/正弦波与音乐.gsp")
    else {
        return;
    };
    let buttons = fixture_buttons_without_validation(&data);

    let play_button = buttons
        .iter()
        .find(|button| button.text == "演奏&M")
        .expect("expected play-function button");
    match &play_button.action {
        ButtonAction::PlayFunction { function_key } => {
            assert_eq!(*function_key, 99);
        }
        action => panic!("expected play-function action, got {action:?}"),
    }
}

#[test]
fn collects_legacy_bbox_image_without_validation() {
    let Some(data) = fixture_bytes("tests/Samples/热研系列/概率问题/抛豆实验.gsp")
    else {
        return;
    };
    let images = fixture_images_without_validation(&data);

    assert!(
        !images.is_empty(),
        "expected the legacy bbox-backed image payload to export"
    );
}

#[test]
fn collects_legacy_symbolic_labels_without_validation() {
    let Some(data) = fixture_bytes("tests/Samples/热研系列/概率问题/抛豆实验.gsp")
    else {
        return;
    };
    let labels = fixture_labels_without_validation(&data);

    assert!(
        labels.iter().any(|label| label.text == "k"),
        "expected the legacy three-point helper label to export"
    );
    assert!(
        labels.iter().any(|label| label.text == "圈内豆子数"),
        "expected the legacy named helper label to export"
    );
}

#[test]
fn collects_sequence_button_variants_without_validation() {
    let Some(data) =
        fixture_bytes("tests/Samples/个人专栏/李忠平作品/金华2010-24题(百年孤独)10.8.9.gsp")
    else {
        return;
    };
    let buttons = fixture_buttons_without_validation(&data);

    let sequence = buttons
        .iter()
        .find(|button| button.text == "顺序3个动作")
        .expect("expected sequence button");
    match &sequence.action {
        ButtonAction::Sequence {
            button_indices,
            interval_ms,
        } => {
            assert!(
                !button_indices.is_empty(),
                "expected sequence button to retain at least one exported child action"
            );
            assert!(
                *interval_ms <= 10_000,
                "expected sequence payload interval to remain a sane exported value"
            );
        }
        action => panic!("expected sequence action, got {action:?}"),
    }
}

#[test]
fn collects_move_point_button_variants_without_validation() {
    let Some(data) = fixture_bytes("tests/Samples/个人专栏/常新德作品/3d_魔方.gsp")
    else {
        return;
    };
    let buttons = fixture_buttons_without_validation(&data);

    let rotate_z = buttons
        .iter()
        .find(|button| button.text == "转动z")
        .expect("expected rotate-z button");
    match &rotate_z.action {
        ButtonAction::MovePoint {
            point_index,
            target_point_index,
        } => {
            assert!(
                target_point_index.is_some(),
                "expected move-point variant to keep its target point"
            );
            assert!(*point_index != target_point_index.unwrap_or(*point_index));
        }
        action => panic!("expected move-point action, got {action:?}"),
    }
}

#[test]
fn collects_focus_point_button_without_validation() {
    let Some(data) = fixture_bytes("tests/Samples/个人专栏/孙禄京作品/正三角形重叠.gsp")
    else {
        return;
    };
    let buttons = fixture_buttons_without_validation(&data);

    let focus = buttons
        .iter()
        .find(|button| matches!(button.action, ButtonAction::FocusPoint { .. }))
        .expect("expected focus-point button");
    assert_eq!(focus.text, "居中");
}

#[test]
fn preserves_draw_function_fixture_interactivity() {
    let scene = fixture_scene(include_bytes!(
        "../../../tests/fixtures/未实现的系统功能/绘图函数.gsp"
    ));

    assert!(scene.graph_mode, "expected graph scene");
    assert!(
        scene.images.len() == 1,
        "expected one embedded graph image, got {}",
        scene.images.len()
    );
    assert!(
        scene.images[0].screen_space,
        "expected payload-positioned screen image"
    );
    assert!(
        scene.images[0].src.starts_with("data:image/png;base64,"),
        "expected embedded png data url"
    );
    assert!(
        scene.images[0].top_left.x < scene.images[0].bottom_right.x
            && scene.images[0].top_left.y < scene.images[0].bottom_right.y,
        "expected visible screen-space image bounds"
    );
    assert!(
        scene.lines.len() >= 3,
        "expected graph helpers to remain visible with the embedded image"
    );
}

#[test]
fn preserves_insert_image_fixture() {
    let scene = fixture_scene(include_bytes!(
        "../../../tests/fixtures/未实现的系统功能/插入图片.gsp"
    ));

    assert!(!scene.graph_mode, "expected non-graph image fixture");
    assert_eq!(scene.images.len(), 1, "expected one embedded image");
    assert!(
        scene.images[0].screen_space,
        "expected screen-space image placement"
    );
    assert!(
        scene.images[0].src.starts_with("data:image/png;base64,"),
        "expected embedded png data url"
    );
    assert_eq!(scene.images[0].top_left.x, 118.0);
    assert_eq!(scene.images[0].top_left.y, 112.0);
    assert_eq!(scene.images[0].bottom_right.x, 373.0);
    assert_eq!(scene.images[0].bottom_right.y, 270.0);
    assert!(
        scene.lines.is_empty(),
        "expected image-only fixture without line artifacts"
    );
}

#[test]
fn preserves_multiline_text_labels() {
    let scene = fixture_scene(include_bytes!("../../../tests/fixtures/gsp/多行文本.gsp"));

    assert_eq!(scene.labels.len(), 1);
    assert_eq!(
        scene.labels[0].text,
        "线段中垂线\n垂线\n平行线\n直角三角形\n点的轨迹\n圆上的弧\n过三点的弧"
    );
}

#[test]
fn preserves_hot_text_actions_in_rich_text_gsp() {
    let scene = fixture_scene(include_bytes!("../../../tests/fixtures/gsp/热文本.gsp"));

    let rich_label = scene
        .labels
        .iter()
        .find(|label| label.text.contains("BAC"))
        .expect("expected hot text label");
    assert_eq!(rich_label.text, "在△ACB中，CA=AB，∠BAC=∠CBA");
    assert_eq!(
        rich_label
            .hotspots
            .iter()
            .map(|hotspot| hotspot.text.as_str())
            .collect::<Vec<_>>(),
        vec!["△", "ACB", "CA", "AB", "∠", "BAC", "∠", "CBA"]
    );
    assert!(matches!(
        rich_label.hotspots[0].action,
        crate::runtime::scene::TextLabelHotspotAction::Polygon { .. }
    ));
    assert!(matches!(
        rich_label.hotspots[2].action,
        crate::runtime::scene::TextLabelHotspotAction::Segment { .. }
    ));
    assert!(matches!(
        rich_label.hotspots[3].action,
        crate::runtime::scene::TextLabelHotspotAction::Segment { .. }
    ));
    assert!(matches!(
        rich_label.hotspots[4].action,
        crate::runtime::scene::TextLabelHotspotAction::AngleMarker { .. }
    ));
    assert!(matches!(
        rich_label.hotspots[6].action,
        crate::runtime::scene::TextLabelHotspotAction::AngleMarker { .. }
    ));
    assert_eq!(scene.buttons.len(), 1, "expected linked action button");
    assert_eq!(scene.buttons[0].text, "隐藏三角形 ACB");
}
