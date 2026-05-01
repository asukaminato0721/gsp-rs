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
        match self.write_html(&data) {
            Ok(()) => artifacts::write_standard_sidecars(
                self.gsp_path,
                &self.paths,
                &data,
                self.width,
                self.height,
            ),
            Err(error) => Err(artifacts::attach_payload_log(
                self.gsp_path,
                &self.paths,
                &data,
                error,
            )),
        }
    }

    fn compile_html_only(&self) -> Result<()> {
        let data = self.read_source()?;
        self.write_html(&data)
    }

    fn scene_json(&self) -> Result<String> {
        let data = self.read_source()?;
        document::compile_bytes_to_scene_json(&data, self.width, self.height)
    }

    fn read_source(&self) -> Result<Vec<u8>> {
        artifacts::read_source(self.gsp_path)
    }

    fn write_html(&self, data: &[u8]) -> Result<()> {
        compile_bytes_to_html_file(data, &self.paths.html_path, self.width, self.height)
    }
}
