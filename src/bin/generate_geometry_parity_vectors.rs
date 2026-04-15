use miette::{IntoDiagnostic, Result, WrapErr};
use std::path::PathBuf;

fn main() -> Result<()> {
    let output_dir = PathBuf::from("src/html/generated");
    gsp_rs::geometry_parity::export_geometry_parity_vectors(&output_dir)
        .into_diagnostic()
        .wrap_err_with(|| {
            format!(
                "failed to export geometry parity vectors to {}",
                output_dir.display()
            )
        })?;
    Ok(())
}
