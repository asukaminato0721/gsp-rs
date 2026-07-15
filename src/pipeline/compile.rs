use miette::Result;
use std::path::Path;

use super::{artifacts, document};

pub fn compile_file_to_html(
    gsp_path: &Path,
    html_path: &Path,
    width: u32,
    height: u32,
) -> Result<()> {
    FileCompileJob::new(gsp_path, html_path, width, height).compile_standard()
}

pub fn compile_file_to_html_only(
    gsp_path: &Path,
    html_path: &Path,
    width: u32,
    height: u32,
) -> Result<()> {
    FileCompileJob::new(gsp_path, html_path, width, height).compile_html_only()
}

pub fn compile_file_to_scene_json(gsp_path: &Path, width: u32, height: u32) -> Result<String> {
    let html_path = gsp_path.with_extension("html");
    FileCompileJob::new(gsp_path, &html_path, width, height).scene_json()
}

pub fn compile_bytes_to_html_file(
    data: &[u8],
    html_path: &Path,
    width: u32,
    height: u32,
) -> Result<()> {
    let html = document::compile_bytes_to_html_document(data, width, height)?;
    artifacts::write_html(html_path, &html)
}

struct FileCompileJob<'a> {
    gsp_path: &'a Path,
    paths: artifacts::ArtifactPaths,
    width: u32,
    height: u32,
}

impl<'a> FileCompileJob<'a> {
    fn new(gsp_path: &'a Path, html_path: &Path, width: u32, height: u32) -> Self {
        Self {
            gsp_path,
            paths: artifacts::ArtifactPaths::from_output(gsp_path, html_path),
            width,
            height,
        }
    }

    fn compile_standard(&self) -> Result<()> {
        let data = self.read_source()?;
        let file = crate::gsp::parse(&data).map_err(miette::Report::new)?;
        let reference_htm = self.read_reference_definitions();
        let document = match document::CompiledDocument::compile(
            &file,
            self.width,
            self.height,
            reference_htm.as_deref(),
        ) {
            Ok(document) => document,
            Err(error) => {
                return Err(artifacts::attach_payload_log(
                    self.gsp_path,
                    &self.paths,
                    &file,
                    error,
                ));
            }
        };
        if let Err(error) = artifacts::write_html(&self.paths.html_path, &document.render_html()) {
            return Err(artifacts::attach_payload_log(
                self.gsp_path,
                &self.paths,
                &file,
                error,
            ));
        }
        artifacts::write_debug_json(&self.paths, &document.render_scene_json())?;
        artifacts::write_payload_log(
            self.gsp_path,
            &self.paths,
            &file,
            document.payload_log_graph_transform(),
        )?;
        Ok(())
    }

    fn compile_html_only(&self) -> Result<()> {
        let data = self.read_source()?;
        self.write_html(&data)
    }

    fn scene_json(&self) -> Result<String> {
        let data = self.read_source()?;
        let reference_htm = self.read_reference_definitions();
        document::compile_bytes_to_scene_json_with_reference(
            &data,
            self.width,
            self.height,
            reference_htm.as_deref(),
        )
    }

    fn read_source(&self) -> Result<Vec<u8>> {
        artifacts::read_source(self.gsp_path)
    }

    fn write_html(&self, data: &[u8]) -> Result<()> {
        let reference_htm = self.read_reference_definitions();
        let html = document::compile_bytes_to_html_document_with_reference(
            data,
            self.width,
            self.height,
            reference_htm.as_deref(),
        )?;
        artifacts::write_html(&self.paths.html_path, &html)
    }

    fn read_reference_definitions(&self) -> Option<String> {
        let htm = std::fs::read_to_string(self.gsp_path.with_extension("htm")).ok();
        let log = std::fs::read_to_string(self.gsp_path.with_extension("log")).ok();
        match (htm, log) {
            (Some(htm), Some(log)) => Some(format!("{htm}\n{log}")),
            (Some(htm), None) => Some(htm),
            (None, Some(log)) => Some(log),
            (None, None) => None,
        }
    }
}
