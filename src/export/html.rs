mod assets;
mod document;
mod function_expr_json;
mod function_scene_json;
mod iteration_scene_json;
mod label_button_scene_json;
mod line_shape_scene_json;
mod point_scene_json;
mod scene_json;

use crate::runtime::scene::Scene;
use std::fs;
use std::path::Path;
use ts_rs::{Config, ExportError};

pub(crate) fn write_standalone_html(output_path: &Path, html: &str) -> Result<(), String> {
    let extension = match output_path.extension() {
        Some(ext) => ext.to_str(),
        None => None,
    };
    if !matches!(
        extension,
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

pub(crate) fn render_standalone_html_document(
    scene: &Scene,
    width: u32,
    height: u32,
    document_layout: bool,
) -> String {
    document::render_standalone_html_document(scene, width, height, document_layout)
}

pub(crate) fn render_scene_json(scene: &Scene, width: u32, height: u32, pretty: bool) -> String {
    scene_json::scene_to_json(scene, width, height, pretty)
}

pub fn export_viewer_types(output_dir: &Path) -> Result<(), ExportError> {
    let cfg = Config::new().with_out_dir(output_dir);
    scene_json::export_bindings(&cfg)
}
