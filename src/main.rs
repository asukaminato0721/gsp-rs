mod config;
#[allow(dead_code)]
mod format;
mod html;
#[allow(dead_code)]
mod png;
#[allow(dead_code)]
mod render;

use crate::config::Config;
use crate::format::GspFile;
use crate::html::render_points_to_html;
use std::env;
use std::fs;
use std::process;

fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error}");
        process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let config = Config::parse(env::args_os().skip(1))?;
    let data = fs::read(&config.gsp_path)
        .map_err(|error| format!("failed to read {}: {error}", config.gsp_path.display()))?;
    let file = GspFile::parse(&data)?;

    render_points_to_html(
        &file,
        &config.html_path,
        config.render_width,
        config.render_height,
    )?;
    println!("generated {}", config.html_path.display());
    Ok(())
}
