use super::*;

#[test]
fn exports_ant_sliding_triangle_parameter_application_as_live_geometry() {
    let Some(data) = fixture_bytes(
        "tests/Samples/个人专栏/侯仰顺作品/参数的应用-正三角形在正方形内滑动【蚂蚁制作】.gsp",
    ) else {
        return;
    };
    let scene = fixture_scene(&data, "ant sliding triangle fixture should compile");
    let points = scene["points"]
        .as_array()
        .expect("scene points should be an array");

    let dragged = points.get(4).expect("expected E point");
    let driven = points.get(5).expect("expected F point");
    assert_eq!(
        driven["binding"]["kind"].as_str(),
        Some("constraint-parameter-from-point-expr"),
        "expected F to stay bound to the payload parameter expression"
    );
    assert_eq!(
        driven["binding"]["expr"]["kind"].as_str(),
        Some("parsed"),
        "expected the hidden payload expression to decode instead of falling back to constant 0"
    );
    let dx = driven["x"].as_f64().expect("F x") - dragged["x"].as_f64().expect("E x");
    let dy = driven["y"].as_f64().expect("F y") - dragged["y"].as_f64().expect("E y");
    assert!(
        (dx * dx + dy * dy).sqrt() > 100.0,
        "expected F to move away from E; the previous constant-0 decode collapsed them together"
    );
    assert!(
        driven["y"].as_f64().expect("F y") < dragged["y"].as_f64().expect("E y") - 120.0,
        "expected F to use the expression result as its polygon parameter, matching the upper-left position in the source"
    );
    let trace = scene["lines"]
        .as_array()
        .expect("scene lines should be an array")
        .iter()
        .find(|line| line["binding"]["kind"].as_str() == Some("point-trace"))
        .expect("expected the trace of G to export");
    assert_eq!(trace["binding"]["pointIndex"].as_u64(), Some(6));
    assert_eq!(trace["binding"]["driverIndex"].as_u64(), Some(4));
    let trace_points = trace["points"]
        .as_array()
        .expect("trace should carry sampled points");
    let (min_x, max_x, min_y, max_y) = trace_points.iter().fold(
        (
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::INFINITY,
            f64::NEG_INFINITY,
        ),
        |(min_x, max_x, min_y, max_y), point| {
            let x = point["x"].as_f64().expect("trace x");
            let y = point["y"].as_f64().expect("trace y");
            (min_x.min(x), max_x.max(x), min_y.min(y), max_y.max(y))
        },
    );
    assert!(
        min_x > 206.0 && max_x < 416.0 && min_y > 178.0 && max_y < 388.0,
        "expected G trace to stay inside the square, got x={min_x}..{max_x}, y={min_y}..{max_y}"
    );
    assert!(
        max_x - min_x > 200.0 && max_y - min_y > 200.0,
        "expected G trace to span the rounded-square-like locus, got x={min_x}..{max_x}, y={min_y}..{max_y}"
    );

    let labels = scene["labels"]
        .as_array()
        .expect("scene labels should be an array");
    assert!(
        labels
            .iter()
            .all(|label| label["text"].as_str() != Some("0 = 0")),
        "expected the hidden expression label to stop advertising the failed constant parse"
    );
    assert!(
        labels.iter().any(|label| {
            label["text"].as_str() == Some("F") && label["visible"].as_bool() == Some(true)
        }),
        "expected the parameter-controlled point label F to export visibly"
    );
    assert!(
        labels
            .iter()
            .any(|label| { label["text"].as_str() == Some("E(拖动)在BCDA上的值 = 0.15") }),
        "expected the polygon parameter label to match the source display"
    );
    let expression_label = labels
        .iter()
        .find(|label| {
            label["text"]
                .as_str()
                .is_some_and(|text| text.contains("E(拖动)在BCDA上的值") && text.contains('√'))
        })
        .expect("expected the expression label to use the source parameter name");
    let rich_markup = expression_label["richMarkup"]
        .as_str()
        .expect("expression label should have rich markup");
    assert!(
        rich_markup.contains("</") && rich_markup.contains("<R"),
        "expected the expression label to render as a fraction with a radical, got {rich_markup}"
    );
}

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
