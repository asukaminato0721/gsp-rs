use std::path::PathBuf;

fn main() -> Result<(), String> {
    let output_dir = PathBuf::from("src/html/generated");
    gsp_rs::export::html::export_viewer_types(&output_dir)
        .map_err(|error| format!("failed to export viewer types to {}: {error}", output_dir.display()))
}
