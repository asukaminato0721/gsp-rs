use std::env;
use std::process::Child;
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

use gsp_rs::{Config, pipeline::compile_file_to_html};
use miette::{Result, miette};

const WORKER_ENV: &str = "GSP_RS_WORKER";
const WORKER_TIMEOUT: Duration = Duration::from_secs(5);

fn main() -> Result<()> {
    run()
}

fn run() -> Result<()> {
    let config = Config::parse(env::args_os().skip(1))?;
    run_jobs(&config)
}

fn run_jobs(config: &Config) -> Result<()> {
    if env::var_os(WORKER_ENV).is_some() {
        return run_jobs_in_process(config);
    }
    run_jobs_out_of_process(config)
}

fn run_jobs_in_process(config: &Config) -> Result<()> {
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

fn run_jobs_out_of_process(config: &Config) -> Result<()> {
    let exe_path = env::current_exe().map_err(|error| {
        miette!("failed to resolve current executable for isolated job execution: {error}")
    })?;
    let mut failures = Vec::new();

    for job in &config.jobs {
        let mut child = Command::new(&exe_path)
            .env(WORKER_ENV, "1")
            .arg(&job.gsp_path)
            .spawn()
            .map_err(|error| {
                miette!(
                    "failed to launch isolated compiler process for {}: {error}",
                    job.gsp_path.display()
                )
            })?;

        let Some(status) = wait_for_child_exit(&mut child, WORKER_TIMEOUT).map_err(|error| {
            miette!(
                "failed while waiting for isolated compiler process for {}: {error}",
                job.gsp_path.display()
            )
        })?
        else {
            let _ = child.kill();
            let _ = child.wait();
            failures.push(format!(
                "{}: compiler process timed out after {}s",
                job.gsp_path.display(),
                WORKER_TIMEOUT.as_secs()
            ));
            continue;
        };

        if !status.success() {
            failures.push(format!(
                "{}: compiler process terminated unexpectedly ({})",
                job.gsp_path.display(),
                format_exit_status(status)
            ));
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

fn wait_for_child_exit(
    child: &mut Child,
    timeout: Duration,
) -> std::io::Result<Option<std::process::ExitStatus>> {
    let start = Instant::now();
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok(Some(status));
        }
        if start.elapsed() >= timeout {
            return Ok(None);
        }
        thread::sleep(Duration::from_millis(10));
    }
}

fn format_exit_status(status: std::process::ExitStatus) -> String {
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;

        if let Some(signal) = status.signal() {
            return format!("signal {signal}");
        }
    }

    if let Some(code) = status.code() {
        format!("exit code {code}")
    } else {
        "unknown exit status".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::format_exit_status;
    use std::process::Command;
    use std::time::Duration;

    #[test]
    fn formats_nonzero_exit_codes() {
        let status = Command::new("sh")
            .arg("-c")
            .arg("exit 7")
            .status()
            .expect("shell status");
        assert_eq!(format_exit_status(status), "exit code 7");
    }

    #[cfg(unix)]
    #[test]
    fn formats_signal_terminations() {
        use std::os::unix::process::ExitStatusExt;

        let status = std::process::ExitStatus::from_raw(6);
        assert_eq!(format_exit_status(status), "signal 6");
    }

    #[cfg(unix)]
    #[test]
    fn reports_timeout_when_child_runs_too_long() {
        let mut child = Command::new("sh")
            .arg("-c")
            .arg("sleep 1")
            .spawn()
            .expect("spawn sleepy child");
        let status = super::wait_for_child_exit(&mut child, Duration::from_millis(10))
            .expect("wait should succeed");
        assert!(status.is_none(), "expected timeout for sleepy child");
        let _ = child.kill();
        let _ = child.wait();
    }
}
