pub(super) const VIEWER_CSS: &str = include_str!("../../html/viewer.css");
pub(super) const VAN_JS: &str = include_str!("../../html/vendor/van-1.6.0.js");
pub(super) const VIEWER_RUNTIME_JS: &str = include_str!("../../html/generated/viewer-runtime.js");

pub(super) fn van_runtime_to_global() -> String {
    VAN_JS.replacen("export default {", "window.van = {", 1)
}

pub(super) fn indent_asset(asset: &str, spaces: usize) -> String {
    let indent = " ".repeat(spaces);
    asset
        .lines()
        .map(|line| format!("{indent}{line}"))
        .collect::<Vec<_>>()
        .join("\n")
}

pub(super) fn minify_css_asset(asset: &str) -> String {
    let mut minified = asset
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("");
    for (needle, replacement) in [
        (": ", ":"),
        ("; ", ";"),
        (", ", ","),
        (" {", "{"),
        ("} ", "}"),
    ] {
        minified = minified.replace(needle, replacement);
    }
    minified
}
