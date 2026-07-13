use crate::export::html::{
    StandaloneHtmlPage, render_document_scene_json, render_scene_json, render_standalone_html_pages,
};
use crate::gsp;
use crate::runtime::build_scene_checked;
use crate::runtime::scene::Scene;
use miette::{Result, WrapErr, miette};

pub fn compile_bytes_to_html_document(data: &[u8], width: u32, height: u32) -> Result<String> {
    let file = gsp::parse(data).map_err(miette::Report::new)?;
    let compiled_pages = compile_pages(&file, width, height)?;
    let html_pages = compiled_pages
        .iter()
        .map(|page| StandaloneHtmlPage {
            title: &page.title,
            scene: &page.scene,
            width: page.width,
            height: page.height,
            document_layout: page.document_layout,
        })
        .collect::<Vec<_>>();
    Ok(render_standalone_html_pages(&html_pages))
}

pub fn compile_bytes_to_scene_json(data: &[u8], width: u32, height: u32) -> Result<String> {
    let file = gsp::parse(data).map_err(miette::Report::new)?;
    let compiled_pages = compile_pages(&file, width, height)?;
    if let [page] = compiled_pages.as_slice() {
        return Ok(render_scene_json(
            &page.scene,
            page.width,
            page.height,
            true,
        ));
    }
    let html_pages = as_html_pages(&compiled_pages);
    Ok(render_document_scene_json(&html_pages))
}

fn compile_pages(
    file: &crate::format::GspFile,
    width: u32,
    height: u32,
) -> Result<Vec<CompiledHtmlPage>> {
    let page_files = file.page_files();
    let mut compiled_pages = Vec::with_capacity(page_files.len());
    for (index, page_file) in page_files.iter().enumerate() {
        let scene = build_scene_checked(page_file)
            .map_err(|error| miette!("{error:#}"))
            .wrap_err_with(|| format!("failed to build scene from page {}", index + 1))?;
        let document_layout = is_document_layout(page_file, &scene);
        let (width, height) = export_dimensions(page_file, &scene, width, height);
        compiled_pages.push(CompiledHtmlPage {
            title: format!("Page {}", index + 1),
            scene,
            width,
            height,
            document_layout,
        });
    }
    Ok(compiled_pages)
}

fn as_html_pages(pages: &[CompiledHtmlPage]) -> Vec<StandaloneHtmlPage<'_>> {
    pages
        .iter()
        .map(|page| StandaloneHtmlPage {
            title: &page.title,
            scene: &page.scene,
            width: page.width,
            height: page.height,
            document_layout: page.document_layout,
        })
        .collect()
}

struct CompiledHtmlPage {
    title: String,
    scene: Scene,
    width: u32,
    height: u32,
    document_layout: bool,
}

fn export_dimensions(
    file: &crate::format::GspFile,
    scene: &crate::runtime::scene::Scene,
    fallback_width: u32,
    fallback_height: u32,
) -> (u32, u32) {
    if is_document_layout(file, scene)
        && let Some((width, height)) = file.document_canvas_size()
    {
        return (width, height);
    }
    (fallback_width, fallback_height)
}

fn is_document_layout(file: &crate::format::GspFile, scene: &crate::runtime::scene::Scene) -> bool {
    !scene.graph_mode && file.document_canvas_size().is_some()
}
