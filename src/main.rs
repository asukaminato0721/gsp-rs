use gsp_rs::{Config, pipeline::compile_file_to_html};
use std::env;
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
        compile_file_to_html(
            &job.gsp_path,
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
