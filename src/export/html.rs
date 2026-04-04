mod assets;
mod document;
mod scene_json;

use crate::runtime::scene::Scene;
use std::fs;
use std::path::Path;

pub(crate) fn write_standalone_html(output_path: &Path, html: &str) -> Result<(), String> {
    if !matches!(
        output_path.extension().and_then(|ext| ext.to_str()),
        Some("html") | Some("HTML") | Some("htm") | Some("HTM")
    ) {
        return Err(format!(
            "html output path must end with .html or .htm: {}",
            output_path.display()
        ));
    }

    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "failed to create html output directory {}: {error}",
                parent.display()
            )
        })?;
    }

    fs::write(output_path, html)
        .map_err(|error| format!("failed to write {}: {error}", output_path.display()))?;
    Ok(())
}

pub(crate) fn render_standalone_html_document(scene: &Scene, width: u32, height: u32) -> String {
    document::render_standalone_html_document(scene, width, height)
}

pub(crate) fn render_scene_json(
    scene: &Scene,
    width: u32,
    height: u32,
    pretty: bool,
) -> String {
    scene_json::scene_to_json(scene, width, height, pretty)
}
