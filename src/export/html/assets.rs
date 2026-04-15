pub(super) const VIEWER_CSS: &str = include_str!("../../html/viewer.css");
pub(super) const VAN_JS: &str = include_str!("../../html/vendor/van-1.6.0.js");
pub(super) const VIEWER_GEOMETRY_JS: &str = include_str!("../../html/viewer_geometry.js");
pub(super) const VIEWER_SCENE_BASIC_JS: &str = include_str!("../../html/viewer_scene_basic.js");
pub(super) const VIEWER_RENDER_BASIC_JS: &str = include_str!("../../html/viewer_render_basic.js");
pub(super) const VIEWER_RENDER_IMAGES_JS: &str = include_str!("../../html/viewer_render_images.js");
pub(super) const VIEWER_RENDER_POLYGONS_JS: &str =
    include_str!("../../html/viewer_render_polygons.js");
pub(super) const VIEWER_RENDER_CIRCULAR_JS: &str =
    include_str!("../../html/viewer_render_circular.js");
pub(super) const VIEWER_RENDER_LABELS_JS: &str = include_str!("../../html/viewer_render_labels.js");
pub(super) const VIEWER_RENDER_TABLES_JS: &str = include_str!("../../html/viewer_render_tables.js");
pub(super) const VIEWER_RENDER_HOTSPOTS_JS: &str =
    include_str!("../../html/viewer_render_hotspots.js");
pub(super) const VIEWER_OVERLAY_JS: &str = include_str!("../../html/viewer_overlay.js");
pub(super) const VIEWER_OVERLAY_STUB_JS: &str = include_str!("../../html/viewer_overlay_stub.js");
pub(super) const VIEWER_DRAG_JS: &str = include_str!("../../html/viewer_drag.js");
pub(super) const VIEWER_DRAG_PAN_JS: &str = include_str!("../../html/viewer_drag_pan.js");
pub(super) const VIEWER_DYNAMICS_JS: &str = include_str!("../../html/viewer_dynamics.js");
pub(super) const VIEWER_DYNAMICS_STUB_JS: &str = include_str!("../../html/viewer_dynamics_stub.js");
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
