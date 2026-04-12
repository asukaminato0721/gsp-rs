use miette::{IntoDiagnostic, Result, WrapErr};
use std::path::PathBuf;

fn main() -> Result<()> {
    let output_dir = PathBuf::from("src/html/generated");
    gsp_rs::export::html::export_viewer_types(&output_dir)
        .into_diagnostic()
        .wrap_err_with(|| format!("failed to export viewer types to {}", output_dir.display()))
}
