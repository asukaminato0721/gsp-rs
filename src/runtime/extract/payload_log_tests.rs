use super::htm_reference::{construction_lines_from_htm, construction_lines_from_log};
use super::test_support::{assert_supported_sample_log, fixture_bytes, fixture_log};
use insta::assert_snapshot;
use std::fs;

#[test]
fn renders_unsupported_payload_log_in_natural_chinese() {
    let Some(data) = fixture_bytes("tests/fixtures/14.gsp") else {
        return;
    };
    let log = fixture_log(&data, "tests/fixtures/14.gsp");

    assert!(log.contains("载荷说明"));
    assert!(log.contains("问题列表"));
    assert!(log.contains("对象 #19 暂时无法导出，因为对象类型 86 还没有实现。"));
    assert!(log.contains("相关对象："));
    assert!(log.contains("#19 = 未知对象，类型是 86，按载荷顺序引用 #18、#16、#6。"));
    assert!(log.contains("#18 = 将 点 #10 按向量 #1 -> #10 平移得到的对象"));
    assert!(log.contains("#15 = 过 #10 且垂直于 线段 #14 的直线。"));
    assert!(log.contains("#1 = 自由点，名称“O”。"));
    assert!(log.contains("原始载荷："));
    assert!(log.contains("Construction VALUE"));
    assert!(log.contains("{1} Point("));
}

#[test]
fn renders_payload_log_for_supported_fixture_too() {
    let log = fixture_log(
        include_bytes!("../../../tests/fixtures/gsp/static/point.gsp"),
        "tests/fixtures/gsp/static/point.gsp",
    );

    assert!(log.contains("问题数量: 0"));
    assert!(log.contains("未发现不支持的载荷。"));
    assert!(log.contains("Construction VALUE"));
    assert!(log.contains("{1} Point(323,217)[mediumPoint];"));
}

#[test]
fn insection_payload_logs_match_reference_htm_construction() {
    let fixture_names = [
        "segment_insection",
        "ray_insection",
        "line_insection",
        "circle_insection",
        "circle_circle_insection",
        "segment_circle",
        "cood",
        "cood_intersection",
        "cood_intersection_xy",
        "cood_intersection_y",
    ];

    for name in fixture_names {
        let gsp_path = format!("tests/fixtures/gsp/insection/{name}.gsp");
        let htm_path = format!("tests/fixtures/gsp/insection/{name}.htm");
        let gsp = fs::read(&gsp_path).expect("fixture gsp should be readable");
        let htm = fs::read_to_string(&htm_path).expect("reference htm should be readable");
        let log = fixture_log(&gsp, &gsp_path);
        assert_eq!(
            construction_lines_from_log(&log),
            construction_lines_from_htm(&htm),
            "expected payload log Construction VALUE to match {htm_path}"
        );
    }
}

#[test]
fn top_level_gsp_payload_logs_match_new_reference_htm_construction() {
    let fixture_names = [
        "calculation",
        "circle_center_radius",
        "circle_y_intersection",
        "middle_point",
        "music",
        "music1",
        "parallel",
        "perp",
        "pert_vert",
        "point_cood_expr",
        "point_on_arc1",
        "point_on_arc2",
        "trace",
        "vert",
        "xy_cood",
        "两个三角形标记全等",
        "垂线段",
        "多行文本",
        "热文本",
    ];

    for name in fixture_names {
        let gsp_path = format!("tests/fixtures/gsp/{name}.gsp");
        let htm_path = format!("tests/fixtures/gsp/{name}.htm");
        let gsp = fs::read(&gsp_path).expect("fixture gsp should be readable");
        let htm = fs::read_to_string(&htm_path).expect("reference htm should be readable");
        let log = fixture_log(&gsp, &gsp_path);
        assert_eq!(
            construction_lines_from_log(&log),
            construction_lines_from_htm(&htm),
            "expected payload log Construction VALUE to match {htm_path}"
        );
    }
}

#[test]
fn he_jixu_sample_payload_logs_use_reference_htm_construction() {
    let fixture_names = ["t以内的减法(hjx4882)", "点的值（hjx4882）"];

    for name in fixture_names {
        let gsp_path = format!("tests/Samples/个人专栏/贺基旭作品/{name}.gsp");
        let htm_path = format!("tests/Samples/个人专栏/贺基旭作品/{name}.htm");
        let Some(gsp) = fixture_bytes(&gsp_path) else {
            continue;
        };
        let Ok(htm) = fs::read_to_string(&htm_path) else {
            continue;
        };
        let log = fixture_log(&gsp, &gsp_path);
        assert_eq!(
            construction_lines_from_log(&log),
            construction_lines_from_htm(&htm),
            "expected payload log Construction VALUE to match {htm_path}"
        );
    }
}

#[test]
fn unimplemented_system_payload_logs_match_reference_htm_construction() {
    let fixture_names = [
        "parameter",
        "三角形的四心",
        "函数",
        "圆的形成",
        "弓形周界动点",
        "扇形周界动点",
        "插入图片",
        "未命名1",
        "极坐标",
        "绘图函数",
        "给定的数值在路径上绘制点",
        "自定义变换",
        "角度标记的标签",
    ];

    for name in fixture_names {
        let gsp_path = format!("tests/fixtures/未实现的系统功能/{name}.gsp");
        let htm_path = format!("tests/fixtures/未实现的系统功能/{name}.htm");
        let gsp = fs::read(&gsp_path).expect("fixture gsp should be readable");
        let htm = fs::read_to_string(&htm_path).expect("reference htm should be readable");
        let log = fixture_log(&gsp, &gsp_path);
        assert_eq!(
            construction_lines_from_log(&log),
            construction_lines_from_htm(&htm),
            "expected payload log Construction VALUE to match {htm_path}"
        );
    }
}

#[test]
fn unimplemented_payload_logs_match_reference_htm_construction() {
    let fixture_names = ["(inRm)两圆之交", "圆系(inRm)"];

    for name in fixture_names {
        let gsp_path = format!("tests/fixtures/未实现/{name}.gsp");
        let htm_path = format!("tests/fixtures/未实现/{name}.htm");
        let gsp = fs::read(&gsp_path).expect("fixture gsp should be readable");
        let htm = fs::read_to_string(&htm_path).expect("reference htm should be readable");
        let log = fixture_log(&gsp, &gsp_path);
        assert_eq!(
            construction_lines_from_log(&log),
            construction_lines_from_htm(&htm),
            "expected payload log Construction VALUE to match {htm_path}"
        );
    }
}

#[test]
fn payload_log_accepts_helper_payload_families_in_sample_fixtures() {
    for path in [
        "tests/Samples/工具例说/14 统计工具-统计工具示例.gsp",
        "tests/Samples/工具例说/19 显隐阴影-积分法-2作圆与正方形重叠面积函数图象.gsp",
        "tests/Samples/工具例说/19 显隐阴影-积分法-3作多圆重叠面积函数图象.gsp",
        "tests/Samples/未分类档/圆柱表面展开.gsp",
    ] {
        assert_supported_sample_log(path);
    }
}

#[test]
fn payload_log_ignores_non_link_button_payloads_when_rendering_labels() {
    let Some(data) = fixture_bytes("tests/Samples/未分类档/100以内口算减法训练(秦国祥).gsp")
    else {
        return;
    };
    let log = fixture_log(
        &data,
        "tests/Samples/未分类档/100以内口算减法训练(秦国祥).gsp",
    );

    assert!(
        !log.contains("链接解析失败（unsupported action button kind"),
        "expected non-link action buttons to stop surfacing as malformed links"
    );
}

#[test]
fn payload_log_names_value_table_row_helper() {
    let Some(data) = fixture_bytes("tests/Samples/未分类档/利用制表功能快速获取点的坐标.gsp")
    else {
        return;
    };
    let log = fixture_log(
        &data,
        "tests/Samples/未分类档/利用制表功能快速获取点的坐标.gsp",
    );

    assert!(
        log.contains("#10 = 数值表行"),
        "expected kind 91 payloads to render with a meaningful name"
    );
}

#[test]
fn payload_log_names_boundary_intersection_point_helper() {
    let Some(data) = fixture_bytes("tests/Samples/个人专栏/周维波作品/不变面积（雪山飞狐）.gsp")
    else {
        return;
    };
    let log = fixture_log(
        &data,
        "tests/Samples/个人专栏/周维波作品/不变面积（雪山飞狐）.gsp",
    );

    assert!(
        log.contains("边界交点"),
        "expected kind 93 payloads to render with a meaningful point name"
    );
}

#[test]
fn payload_log_names_polar_and_vertex_angle_value_helpers() {
    let Some(data) = fixture_bytes("tests/Samples/个人专栏/向忠作品/n叶草系列迭代.gsp")
    else {
        return;
    };
    let log = fixture_log(&data, "tests/Samples/个人专栏/向忠作品/n叶草系列迭代.gsp");

    assert!(
        log.contains("极角值") || log.contains("顶点角值"),
        "expected angle-helper payloads to render with meaningful names"
    );
}

#[test]
fn payload_log_names_point_alias_projection_helpers() {
    let Some(alias_data) =
        fixture_bytes("tests/Samples/个人专栏/周维波作品/不变面积（雪山飞狐）.gsp")
    else {
        return;
    };
    let alias_log = fixture_log(
        &alias_data,
        "tests/Samples/个人专栏/周维波作品/不变面积（雪山飞狐）.gsp",
    );
    assert!(alias_log.contains("点别名"));

    let Some(derived_data) = fixture_bytes("tests/Samples/个人专栏/孙禄京作品/单摆.gsp")
    else {
        return;
    };
    let derived_log = fixture_log(&derived_data, "tests/Samples/个人专栏/孙禄京作品/单摆.gsp");
    assert!(derived_log.contains("三点派生点"));

    let Some(projected_data) =
        fixture_bytes("tests/Samples/个人专栏/况永胜作品/正方体的展开（3D效果）.gsp")
    else {
        return;
    };
    let projected_log = fixture_log(
        &projected_data,
        "tests/Samples/个人专栏/况永胜作品/正方体的展开（3D效果）.gsp",
    );
    assert!(projected_log.contains("投影坐标点"));
}

#[test]
fn payload_log_names_point_reference_alias_helper() {
    let Some(data) =
        fixture_bytes("tests/Samples/个人专栏/周维波作品/2010年四川眉山中考数学试题最后一题.gsp")
    else {
        return;
    };
    let log = fixture_log(
        &data,
        "tests/Samples/个人专栏/周维波作品/2010年四川眉山中考数学试题最后一题.gsp",
    );

    assert!(
        log.contains("点引用别名"),
        "expected kind 108 payloads to render with a meaningful alias name"
    );
}

#[test]
fn payload_log_names_function_definition_helper() {
    let Some(data) = fixture_bytes("tests/Samples/个人专栏/向忠作品/卡西尼卵形线.gsp")
    else {
        return;
    };
    let log = fixture_log(&data, "tests/Samples/个人专栏/向忠作品/卡西尼卵形线.gsp");

    assert!(
        log.contains("函数定义对象"),
        "expected kind 71 payloads to render with a meaningful function-definition name"
    );
}

#[test]
fn payload_log_names_boundary_length_and_rect_image_helpers() {
    let Some(length_data) =
        fixture_bytes("tests/Samples/个人专栏/方小庆作品/圆在四边形上滚动(inRm).gsp")
    else {
        return;
    };
    let length_log = fixture_log(
        &length_data,
        "tests/Samples/个人专栏/方小庆作品/圆在四边形上滚动(inRm).gsp",
    );
    assert!(length_log.contains("边界长度值"));

    let Some(image_data) = fixture_bytes("tests/Samples/个人专栏/向忠作品/计时器3.gsp")
    else {
        return;
    };
    let image_log = fixture_log(&image_data, "tests/Samples/个人专栏/向忠作品/计时器3.gsp");
    assert!(image_log.contains("矩形图片"));
}

#[test]
fn snapshots_payload_log_for_point_fixture() {
    let log = fixture_log(
        include_bytes!("../../../tests/fixtures/gsp/static/point.gsp"),
        "tests/fixtures/gsp/static/point.gsp",
    );

    assert_snapshot!("point_fixture_payload_log", log);
}

#[test]
fn payload_log_accepts_show_hide_button_visibility_actions_in_jinhua_fixture() {
    let Some(data) =
        fixture_bytes("tests/Samples/个人专栏/李忠平作品/金华2010-24题(百年孤独)10.8.9.gsp")
    else {
        return;
    };
    let log = fixture_log(
        &data,
        "tests/Samples/个人专栏/李忠平作品/金华2010-24题(百年孤独)10.8.9.gsp",
    );

    assert!(
        !log.contains("按钮动作类型 (1, 2) 目前还不支持"),
        "expected show-button visibility payloads to be accepted"
    );
    assert!(
        !log.contains("按钮动作类型 (0, 2) 目前还不支持"),
        "expected hide-button visibility payloads to be accepted"
    );
    assert!(
        !log.contains("按钮动作类型 (7, 1) 目前还不支持")
            && !log.contains("按钮动作类型 (1, 6) 目前还不支持"),
        "expected the updated button mappings to remove the remaining Jinhua button errors"
    );
}

#[test]
fn payload_log_accepts_music_button_kinds_in_wave_fixture() {
    let Some(data) = fixture_bytes("tests/Samples/个人专栏/向忠作品/正弦波与音乐.gsp")
    else {
        return;
    };
    let log = fixture_log(&data, "tests/Samples/个人专栏/向忠作品/正弦波与音乐.gsp");

    assert!(
        !log.contains("按钮动作类型 (8, 0) 目前还不支持"),
        "expected play-function button kind to be accepted"
    );
    assert!(
        !log.contains("按钮动作类型 (1, 6) 目前还不支持"),
        "expected show-object button kind to be accepted in the wave fixture"
    );
}

#[test]
fn payload_log_accepts_legacy_sequence_button_kinds_in_ad_clip_fixture() {
    let Some(data) = fixture_bytes("tests/Samples/个人专栏/郑飞宇作品/广告片断.gsp")
    else {
        return;
    };
    let log = fixture_log(&data, "tests/Samples/个人专栏/郑飞宇作品/广告片断.gsp");

    assert!(
        !log.contains("按钮动作类型 (7, 14) 目前还不支持")
            && !log.contains("按钮动作类型 (7, 15) 目前还不支持"),
        "expected legacy sequence-button variants to be accepted"
    );
}

#[test]
fn payload_log_accepts_hidden_unlabeled_buttons_in_classic_dynamic_fixture() {
    assert_supported_sample_log("tests/Samples/个人专栏/陈发铨作品/经典动态题(一线天).gsp");
}

#[test]
fn payload_log_skips_legacy_label_and_image_helper_errors_in_throw_beans_fixture() {
    let Some(data) = fixture_bytes("tests/Samples/热研系列/概率问题/抛豆实验.gsp")
    else {
        return;
    };
    let log = fixture_log(&data, "tests/Samples/热研系列/概率问题/抛豆实验.gsp");

    assert!(
        !log.contains("对象类型 47 还没有实现")
            && !log.contains("对象类型 85 还没有实现")
            && !log.contains("对象类型 88 还没有实现"),
        "expected legacy helper labels and bbox image payloads to stop surfacing as unsupported"
    );
}
