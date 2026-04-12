use std::env;

use gsp_rs::{Config, pipeline::compile_file_to_html};
use miette::{Result, miette};

fn main() -> Result<()> {
    run()
}

fn run() -> Result<()> {
    let config = Config::parse(env::args_os().skip(1))?;
    run_jobs(&config)
}

fn run_jobs(config: &Config) -> Result<()> {
    let mut failures = Vec::new();
    for job in &config.jobs {
        match compile_file_to_html(
            &job.gsp_path,
            &job.html_path,
            config.render_width,
            config.render_height,
        ) {
            Ok(()) => {
                println!(
                    "generated {} from {}",
                    job.html_path.display(),
                    job.gsp_path.display()
                );
            }
            Err(error) => failures.push(format!("{}: {error}", job.gsp_path.display())),
        }
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(miette!(
            "{} file(s) failed:\n{}",
            failures.len(),
            failures.join("\n")
        ))
    }
}
