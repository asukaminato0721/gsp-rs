use super::*;
#[test]
fn exports_three_parameter_color_fixture_with_live_fill_bindings() {
    let Some(data) = fixture_bytes("tests/Samples/个人专栏/侯仰顺作品/三个参数控制颜色(蚂蚁).gsp")
    else {
        return;
    };
    let scene = fixture_scene(&data, "three-parameter color fixture should compile");

    let circles = scene["circles"]
        .as_array()
        .expect("scene circles should be an array");
    assert_eq!(circles.len(), 2, "expected both payload circles");
    assert_eq!(
        circles[0]["fillColorBinding"]["kind"].as_str(),
        Some("rgb"),
        "expected the first circle interior to keep its RGB payload binding"
    );
    assert_eq!(
        circles[0]["fillColorBinding"]["redPointIndex"].as_u64(),
        Some(4)
    );
    assert_eq!(
        circles[0]["fillColorBinding"]["greenPointIndex"].as_u64(),
        Some(5)
    );
    assert_eq!(
        circles[0]["fillColorBinding"]["bluePointIndex"].as_u64(),
        Some(6)
    );
    assert_eq!(
        circles[1]["fillColorBinding"]["kind"].as_str(),
        Some("hsb"),
        "expected the second circle interior to keep its HSB payload binding"
    );
    assert_eq!(
        circles[1]["fillColorBinding"]["huePointIndex"].as_u64(),
        Some(11)
    );
    assert_eq!(
        circles[1]["fillColorBinding"]["saturationPointIndex"].as_u64(),
        Some(12)
    );
    assert_eq!(
        circles[1]["fillColorBinding"]["brightnessPointIndex"].as_u64(),
        Some(13)
    );

    let labels = scene["labels"]
        .as_array()
        .expect("scene labels should be an array");
    let label_color = |text: &str| {
        labels
            .iter()
            .find(|label| label["text"].as_str() == Some(text))
            .and_then(|label| label["color"].as_array())
            .cloned()
            .expect("expected fixture label color to be exported")
    };
    assert_eq!(
        label_color("红"),
        vec![
            Value::from(255),
            Value::from(0),
            Value::from(0),
            Value::from(255)
        ],
        "expected the red payload label to keep its text color"
    );
    assert_eq!(
        label_color("绿"),
        vec![
            Value::from(0),
            Value::from(128),
            Value::from(0),
            Value::from(255)
        ],
        "expected the green payload label to keep its text color"
    );
    assert_eq!(
        label_color("蓝"),
        vec![
            Value::from(0),
            Value::from(0),
            Value::from(255),
            Value::from(255)
        ],
        "expected the blue payload label to keep its text color"
    );
    assert_eq!(
        label_color("色调"),
        vec![
            Value::from(0),
            Value::from(0),
            Value::from(255),
            Value::from(255)
        ],
        "expected the hue payload label to keep its blue text color"
    );
    assert_eq!(
        label_color("饱和度"),
        vec![
            Value::from(0),
            Value::from(0),
            Value::from(255),
            Value::from(255)
        ],
        "expected the saturation payload label to keep its blue text color"
    );
    assert_eq!(
        label_color("亮度"),
        vec![
            Value::from(0),
            Value::from(0),
            Value::from(255),
            Value::from(255)
        ],
        "expected the brightness payload label to keep its blue text color"
    );

    let visible_label = |text: &str| {
        labels
            .iter()
            .find(|label| label["text"].as_str() == Some(text))
            .and_then(|label| label["visible"].as_bool())
            .expect("expected fixture label visibility to be exported")
    };
    assert!(
        visible_label("红 = 0.28"),
        "expected the red segment parameter label to use the concise named form"
    );
    assert!(
        visible_label("绿 = 0.48"),
        "expected the green segment parameter label to use the concise named form"
    );
    assert!(
        visible_label("蓝 = 0.79"),
        "expected the blue segment parameter label to use the concise named form"
    );
    assert!(
        visible_label("色调 = 0.19"),
        "expected the hue segment parameter label to use the concise named form"
    );
    assert!(
        visible_label("饱和度 = 0.54"),
        "expected the saturation segment parameter label to use the concise named form"
    );
    assert!(
        visible_label("亮度 = 0.77"),
        "expected the brightness segment parameter label to use the concise named form"
    );
    assert!(
        labels
            .iter()
            .all(|label| label["text"].as_str() != Some("红在AB上的t值 = 0.28")),
        "expected the verbose red segment helper label to be omitted when the anchor is named"
    );
    assert!(
        labels
            .iter()
            .all(|label| label["text"].as_str() != Some("色调在FG上的t值 = 0.19")),
        "expected the verbose hue segment helper label to be omitted when the anchor is named"
    );
}
