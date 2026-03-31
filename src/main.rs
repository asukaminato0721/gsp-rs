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
    for job in &config.jobs {
        let data = fs::read(&job.gsp_path)
            .map_err(|error| format!("failed to read {}: {error}", job.gsp_path.display()))?;
        let file = GspFile::parse(&data)?;

        render_points_to_html(
            &file,
            &job.html_path,
            config.render_width,
            config.render_height,
        )?;
        println!(
            "generated {} from {}",
            job.html_path.display(),
            job.gsp_path.display()
        );
    }
    Ok(())
}
