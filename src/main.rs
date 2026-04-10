use std::env;
use std::process;

use gsp_rs::{Config, pipeline::compile_file_to_html};

fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error}");
        process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let config = Config::parse(env::args_os().skip(1))?;
    run_jobs(&config)
}

fn run_jobs(config: &Config) -> Result<(), String> {
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
        Err(format!(
            "{} file(s) failed:\n{}",
            failures.len(),
            failures.join("\n")
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::run_jobs;
    use gsp_rs::{Config, RenderJob};
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn keeps_processing_other_files_and_writes_detailed_payload_logs() {
        let temp_dir = make_temp_dir("batch-unsupported-log");
        let unsupported_path = copy_fixture(
            Path::new("tests/fixtures/未实现的系统功能/未命名1.gsp"),
            &temp_dir.join("unsupported.gsp"),
        );
        let supported_path = copy_fixture(
            Path::new("tests/fixtures/gsp/static/point.gsp"),
            &temp_dir.join("supported.gsp"),
        );
        let config = Config {
            jobs: vec![
                RenderJob {
                    gsp_path: unsupported_path.clone(),
                    html_path: unsupported_path.with_extension("html"),
                },
                RenderJob {
                    gsp_path: supported_path.clone(),
                    html_path: supported_path.with_extension("html"),
                },
            ],
            render_width: 800,
            render_height: 600,
        };

        let error = run_jobs(&config).expect_err("unsupported payload should fail the batch");
        let log_path = unsupported_path.with_extension("log");
        let html_path = supported_path.with_extension("html");

        assert!(html_path.exists(), "expected later valid file to still compile");
        assert!(log_path.exists(), "expected unsupported file to emit a .log");
        assert!(
            error.contains("unsupported payload log:"),
            "expected batch error to mention the emitted log path: {error}"
        );

        let log = fs::read_to_string(&log_path).expect("log should be readable");
        assert!(
            log.contains("Unsupported payload log"),
            "expected log header, got: {log}"
        );
        assert!(
            log.contains("type:"),
            "expected log to include geometry object type details, got: {log}"
        );
        assert!(
            log.contains("name:") || log.contains("label_text:") || log.contains("strings:"),
            "expected log to include decoded object identity details, got: {log}"
        );
    }

    fn copy_fixture(from: &Path, to: &Path) -> PathBuf {
        fs::copy(from, to).expect("fixture copy should succeed");
        to.to_path_buf()
    }

    fn make_temp_dir(prefix: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after epoch")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "gsp-rs-{prefix}-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir_all(&dir).expect("temp dir should be creatable");
        dir
    }
}
