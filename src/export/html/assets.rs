pub(super) const VIEWER_CSS: &str = include_str!("../../html/viewer.css");
pub(super) const VAN_JS: &str = include_str!("../../html/vendor/van-1.6.0.js");
pub(super) const VIEWER_SCENE_JS: &str = include_str!("../../html/viewer_scene.js");
pub(super) const VIEWER_RENDER_JS: &str = include_str!("../../html/viewer_render.js");
pub(super) const VIEWER_DRAG_JS: &str = include_str!("../../html/viewer_drag.js");
pub(super) const VIEWER_JS: &str = include_str!("../../html/viewer.js");

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
