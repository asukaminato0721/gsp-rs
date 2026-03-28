mod config;
mod format;
mod png;
mod render;
mod report;
mod util;

use crate::config::Config;
use crate::format::GspFile;
use crate::render::render_points_to_png;
use crate::report::render_report;
use crate::util::analyze_reference_exe;
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
    let exe_terms = config
        .reference_exe
        .as_ref()
        .map(|path| analyze_reference_exe(path))
        .transpose()?;

    if let Some(render_path) = &config.render_path {
        render_points_to_png(&file, render_path, config.render_width, config.render_height)?;
    }

    println!("{}", render_report(&config, &file, exe_terms.as_ref()));
    Ok(())
}
