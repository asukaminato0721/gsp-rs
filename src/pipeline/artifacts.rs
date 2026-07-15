use crate::export::html::write_standalone_html;
use crate::runtime::render_payload_log;
use miette::{IntoDiagnostic, Report, Result, WrapErr, miette};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ArtifactPaths {
    pub(super) html_path: PathBuf,
    pub(super) debug_json_path: PathBuf,
    pub(super) payload_log_path: PathBuf,
}

impl ArtifactPaths {
    pub(super) fn from_output(gsp_path: &Path, html_path: &Path) -> Self {
        Self {
            html_path: html_path.to_path_buf(),
            debug_json_path: html_path.with_extension("debug.json"),
            payload_log_path: gsp_path.with_extension("log"),
        }
    }

    #[cfg(test)]
    pub(super) fn from_gsp(gsp_path: &Path) -> Self {
        let html_path = gsp_path.with_extension("html");
        Self::from_output(gsp_path, &html_path)
    }
}

pub(super) fn read_source(gsp_path: &Path) -> Result<Vec<u8>> {
    fs::read(gsp_path)
        .into_diagnostic()
        .wrap_err_with(|| format!("failed to read {}", gsp_path.display()))
}

pub(super) fn write_html(html_path: &Path, html: &str) -> Result<()> {
    write_standalone_html(html_path, html).map_err(|error| miette!("{error}"))
}

pub(super) fn write_debug_json(paths: &ArtifactPaths, debug_json: &str) -> Result<PathBuf> {
    fs::write(&paths.debug_json_path, debug_json)
        .into_diagnostic()
        .wrap_err_with(|| format!("failed to write {}", paths.debug_json_path.display()))?;
    Ok(paths.debug_json_path.clone())
}

pub(super) fn write_payload_log(
    gsp_path: &Path,
    paths: &ArtifactPaths,
    file: &crate::format::GspFile,
) -> Result<PathBuf> {
    let log_body = render_payload_log(gsp_path, file);
    fs::write(&paths.payload_log_path, log_body)
        .into_diagnostic()
        .wrap_err_with(|| format!("failed to write {}", paths.payload_log_path.display()))?;
    Ok(paths.payload_log_path.clone())
}

pub(super) fn attach_payload_log(
    gsp_path: &Path,
    paths: &ArtifactPaths,
    file: &crate::format::GspFile,
    error: Report,
) -> Report {
    match write_payload_log(gsp_path, paths, file) {
        Ok(log_path) => miette!("{error}\npayload log: {}", log_path.display()),
        Err(_) => error,
    }
}

#[cfg(test)]
mod tests {
    use super::ArtifactPaths;
    use std::path::Path;

    #[test]
    fn derives_output_sidecar_paths_from_gsp_and_html_paths() {
        let paths = ArtifactPaths::from_output(
            Path::new("fixtures/source/point.gsp"),
            Path::new("out/point.html"),
        );

        assert_eq!(paths.html_path, Path::new("out/point.html"));
        assert_eq!(paths.debug_json_path, Path::new("out/point.debug.json"));
        assert_eq!(
            paths.payload_log_path,
            Path::new("fixtures/source/point.log")
        );
    }
}
