use std::env;
use std::ffi::{OsStr, OsString};
use std::io::{self, BufWriter, Read, Write};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::thread;
use std::time::Duration;

use gsp_rs::upload::upload_gsp_file;
use gsp_rs::{
    CompileMode, Config, RenderJob,
    pipeline::{compile_file_to_html, compile_file_to_html_only, compile_file_to_inspector},
};
use miette::{Result, miette};

const BATCH_WORKER_ENV: &str = "GSP_RS_BATCH_WORKER";
const WORKER_TIMEOUT_ENV: &str = "GSP_RS_WORKER_TIMEOUT_MS";
const WORKER_BATCH_SIZE: usize = 64;
const MAX_WORKER_MESSAGE_SIZE: usize = 16 * 1024 * 1024;

fn main() -> Result<()> {
    run()
}

fn run() -> Result<()> {
    let config = Config::parse(env::args_os().skip(1))?;
    if env::var_os(BATCH_WORKER_ENV).is_some() {
        run_batch_worker(&config)
    } else {
        run_jobs_out_of_process(&config)
    }
}

fn compile_job(config: &Config, job: &RenderJob) -> Result<()> {
    match config.mode {
        CompileMode::Standard => compile_file_to_html(
            &job.gsp_path,
            &job.html_path,
            config.render_width,
            config.render_height,
        ),
        CompileMode::HtmlOnly => compile_file_to_html_only(
            &job.gsp_path,
            &job.html_path,
            config.render_width,
            config.render_height,
        ),
        CompileMode::Inspect => compile_file_to_inspector(&job.gsp_path, &job.html_path),
    }
}

fn run_batch_worker(config: &Config) -> Result<()> {
    let stdout = io::stdout();
    let mut stdout = BufWriter::new(stdout.lock());
    for job in &config.jobs {
        let response = match compile_job(config, job) {
            Ok(()) => WorkerResponse::Success,
            Err(error) => WorkerResponse::Failure(format!("{error:#}")),
        };
        write_worker_response(&mut stdout, &response).map_err(|error| {
            miette!(
                "failed to report compiler result for {}: {error}",
                job.gsp_path.display()
            )
        })?;
    }
    Ok(())
}

fn run_jobs_out_of_process(config: &Config) -> Result<()> {
    let exe_path = env::current_exe().map_err(|error| {
        miette!("failed to resolve current executable for isolated job execution: {error}")
    })?;
    let worker_timeout = worker_timeout_from_env()?;
    let mut failures = Vec::new();
    let mut next_job = 0;

    while next_job < config.jobs.len() {
        let batch_end = (next_job + WORKER_BATCH_SIZE).min(config.jobs.len());
        let mut worker = BatchWorker::spawn(&exe_path, config, &config.jobs[next_job..batch_end])?;
        let mut worker_failed = false;

        while next_job < batch_end {
            let job = &config.jobs[next_job];
            match worker.receive(worker_timeout) {
                WorkerReceive::Response(WorkerResponse::Success) => {
                    println!(
                        "generated {} from {}",
                        job.html_path.display(),
                        job.gsp_path.display()
                    );
                    upload_if_requested(config, job, &mut failures);
                    next_job += 1;
                }
                WorkerReceive::Response(WorkerResponse::Failure(error)) => {
                    failures.push(format!("{}: {error}", job.gsp_path.display()));
                    next_job += 1;
                }
                WorkerReceive::TimedOut => {
                    worker.terminate();
                    worker_failed = true;
                    failures.push(format!(
                        "{}: compiler process timed out after {} ms",
                        job.gsp_path.display(),
                        worker_timeout
                            .expect("a timeout is required for timeout status")
                            .as_millis()
                    ));
                    next_job += 1;
                    break;
                }
                WorkerReceive::Disconnected(error) => {
                    let status = worker.wait();
                    worker_failed = true;
                    failures.push(format!(
                        "{}: compiler process terminated unexpectedly ({}; protocol: {error})",
                        job.gsp_path.display(),
                        status.map_or_else(
                            |wait_error| format!("wait failed: {wait_error}"),
                            format_exit_status
                        )
                    ));
                    next_job += 1;
                    break;
                }
            }
        }

        if !worker_failed {
            let status = worker.wait().map_err(|error| {
                miette!("failed while waiting for compiler batch process: {error}")
            })?;
            if !status.success() {
                return Err(miette!(
                    "compiler batch process terminated after reporting every result ({})",
                    format_exit_status(status)
                ));
            }
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

fn upload_if_requested(config: &Config, job: &RenderJob, failures: &mut Vec<String>) {
    let Some(upload_url) = &config.upload_url else {
        return;
    };
    match upload_gsp_file(&job.gsp_path, upload_url) {
        Ok(response) if response.is_empty() => {
            println!("uploaded {} to {}", job.gsp_path.display(), upload_url);
        }
        Ok(response) => {
            println!(
                "uploaded {} to {}: {}",
                job.gsp_path.display(),
                upload_url,
                response
            );
        }
        Err(error) => failures.push(format_job_error(&job.gsp_path, &error)),
    }
}

struct BatchWorker {
    child: Child,
    responses: Receiver<io::Result<WorkerResponse>>,
}

impl BatchWorker {
    fn spawn(exe_path: &std::path::Path, config: &Config, jobs: &[RenderJob]) -> Result<Self> {
        let mut child = Command::new(exe_path)
            .env(BATCH_WORKER_ENV, "1")
            .args(worker_args_for_jobs(config, jobs))
            .stdout(Stdio::piped())
            .spawn()
            .map_err(|error| miette!("failed to launch compiler batch process: {error}"))?;
        let mut stdout = child
            .stdout
            .take()
            .expect("piped compiler stdout must be available");
        let (sender, responses) = mpsc::channel();
        thread::spawn(move || {
            loop {
                let response = read_worker_response(&mut stdout);
                let should_stop = response.is_err();
                if sender.send(response).is_err() || should_stop {
                    break;
                }
            }
        });
        Ok(Self { child, responses })
    }

    fn receive(&self, timeout: Option<Duration>) -> WorkerReceive {
        let response = match timeout {
            Some(timeout) => match self.responses.recv_timeout(timeout) {
                Ok(response) => response,
                Err(RecvTimeoutError::Timeout) => return WorkerReceive::TimedOut,
                Err(RecvTimeoutError::Disconnected) => {
                    return WorkerReceive::Disconnected("response channel closed".to_string());
                }
            },
            None => match self.responses.recv() {
                Ok(response) => response,
                Err(_) => {
                    return WorkerReceive::Disconnected("response channel closed".to_string());
                }
            },
        };
        match response {
            Ok(response) => WorkerReceive::Response(response),
            Err(error) => WorkerReceive::Disconnected(error.to_string()),
        }
    }

    fn terminate(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }

    fn wait(&mut self) -> io::Result<ExitStatus> {
        self.child.wait()
    }
}

enum WorkerReceive {
    Response(WorkerResponse),
    TimedOut,
    Disconnected(String),
}

#[derive(Debug, PartialEq, Eq)]
enum WorkerResponse {
    Success,
    Failure(String),
}

fn write_worker_response(writer: &mut impl Write, response: &WorkerResponse) -> io::Result<()> {
    let (status, message) = match response {
        WorkerResponse::Success => (0, ""),
        WorkerResponse::Failure(message) => (1, message.as_str()),
    };
    writer.write_all(&[status])?;
    writer.write_all(&(message.len() as u64).to_le_bytes())?;
    writer.write_all(message.as_bytes())?;
    writer.flush()
}

fn read_worker_response(reader: &mut impl Read) -> io::Result<WorkerResponse> {
    let mut status = [0];
    reader.read_exact(&mut status)?;
    let mut length = [0; 8];
    reader.read_exact(&mut length)?;
    let length = usize::try_from(u64::from_le_bytes(length))
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "worker message is too large"))?;
    if length > MAX_WORKER_MESSAGE_SIZE {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "worker message is too large",
        ));
    }
    let mut message = vec![0; length];
    reader.read_exact(&mut message)?;
    let message = String::from_utf8(message)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    match status[0] {
        0 if message.is_empty() => Ok(WorkerResponse::Success),
        1 => Ok(WorkerResponse::Failure(message)),
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "invalid compiler worker response",
        )),
    }
}

fn worker_args_for_jobs(config: &Config, jobs: &[RenderJob]) -> Vec<OsString> {
    let mut args = Vec::with_capacity(jobs.len() + 3);
    if config.mode == CompileMode::HtmlOnly {
        args.push(OsString::from("--html"));
    } else if config.mode == CompileMode::Inspect {
        args.push(OsString::from("--inspect"));
    }
    if let Some(output_dir) = &config.output_dir {
        args.push(OsString::from("--output-dir"));
        args.push(output_dir.as_os_str().to_owned());
    }
    args.extend(jobs.iter().map(|job| job.gsp_path.as_os_str().to_owned()));
    args
}

fn format_job_error(job_path: &std::path::Path, error: &miette::Report) -> String {
    format!("{}: {error:#}", job_path.display())
}

fn worker_timeout_from_env() -> Result<Option<Duration>> {
    parse_worker_timeout(env::var_os(WORKER_TIMEOUT_ENV).as_deref())
}

fn parse_worker_timeout(raw: Option<&OsStr>) -> Result<Option<Duration>> {
    let Some(raw) = raw else {
        return Ok(None);
    };
    let raw = raw.to_string_lossy();
    let millis = raw.parse::<u64>().map_err(|_| {
        miette!("{WORKER_TIMEOUT_ENV} must be a positive integer number of milliseconds")
    })?;
    if millis == 0 {
        return Err(miette!(
            "{WORKER_TIMEOUT_ENV} must be a positive integer number of milliseconds"
        ));
    }
    Ok(Some(Duration::from_millis(millis)))
}

fn format_exit_status(status: ExitStatus) -> String {
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;

        if let Some(signal) = status.signal() {
            return format!("signal {signal}");
        }
    }

    status.code().map_or_else(
        || "unknown exit status".to_string(),
        |code| format!("exit code {code}"),
    )
}

#[cfg(test)]
mod tests {
    use super::{
        WorkerResponse, format_exit_status, format_job_error, read_worker_response,
        worker_args_for_jobs, write_worker_response,
    };
    use gsp_rs::{CompileMode, Config, RenderJob};
    use std::ffi::{OsStr, OsString};
    use std::io::Cursor;
    use std::path::PathBuf;
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

    #[test]
    fn worker_timeout_is_disabled_by_default_and_configurable_in_milliseconds() {
        assert_eq!(super::parse_worker_timeout(None).unwrap(), None);
        assert_eq!(
            super::parse_worker_timeout(Some(OsStr::new("2500"))).unwrap(),
            Some(Duration::from_millis(2500))
        );
        assert!(super::parse_worker_timeout(Some(OsStr::new("0"))).is_err());
        assert!(super::parse_worker_timeout(Some(OsStr::new("later"))).is_err());
    }

    #[test]
    fn formats_job_errors_with_full_cause_chain() {
        let error = miette::miette!("outer context").wrap_err("inner context");
        let rendered = format_job_error(std::path::Path::new("sample.gsp"), &error);
        assert!(rendered.contains("sample.gsp:"));
        assert!(rendered.contains("inner context"));
        assert!(rendered.contains("outer context"));
    }

    #[test]
    fn worker_protocol_round_trips_success_and_multiline_failure() {
        let responses = [
            WorkerResponse::Success,
            WorkerResponse::Failure("first line\n第二行".to_string()),
        ];
        let mut bytes = Vec::new();
        for response in &responses {
            write_worker_response(&mut bytes, response).unwrap();
        }
        let mut reader = Cursor::new(bytes);
        for expected in responses {
            assert_eq!(read_worker_response(&mut reader).unwrap(), expected);
        }
    }

    #[test]
    fn rejects_invalid_worker_protocol_status() {
        let mut bytes = vec![9];
        bytes.extend_from_slice(&0_u64.to_le_bytes());
        assert!(read_worker_response(&mut Cursor::new(bytes)).is_err());
    }

    #[test]
    fn worker_uses_one_process_argument_list_for_the_whole_batch() {
        let config = Config {
            jobs: vec![],
            render_width: 800,
            render_height: 600,
            upload_url: None,
            mode: CompileMode::HtmlOnly,
            output_dir: None,
        };
        let jobs = [
            RenderJob {
                gsp_path: PathBuf::from("first.gsp"),
                html_path: PathBuf::from("first.html"),
            },
            RenderJob {
                gsp_path: PathBuf::from("第二个.gsp"),
                html_path: PathBuf::from("第二个.html"),
            },
        ];
        assert_eq!(
            worker_args_for_jobs(&config, &jobs),
            vec![
                OsString::from("--html"),
                OsString::from("first.gsp"),
                OsString::from("第二个.gsp")
            ]
        );
    }

    #[test]
    fn standard_worker_batch_does_not_enable_upload() {
        let config = Config {
            jobs: vec![],
            render_width: 800,
            render_height: 600,
            upload_url: Some("https://example.test/upload".to_string()),
            mode: CompileMode::Standard,
            output_dir: None,
        };
        let jobs = [RenderJob {
            gsp_path: PathBuf::from("sample.gsp"),
            html_path: PathBuf::from("sample.html"),
        }];

        assert_eq!(
            worker_args_for_jobs(&config, &jobs),
            vec![OsString::from("sample.gsp")]
        );
    }

    #[test]
    fn inspector_worker_batch_preserves_inspect_mode() {
        let config = Config {
            jobs: vec![],
            render_width: 800,
            render_height: 600,
            upload_url: None,
            mode: CompileMode::Inspect,
            output_dir: Some(PathBuf::from("target/inspect")),
        };
        let jobs = [RenderJob {
            gsp_path: PathBuf::from("sample.gsp"),
            html_path: PathBuf::from("target/inspect/sample.inspect.html"),
        }];

        assert_eq!(
            worker_args_for_jobs(&config, &jobs),
            vec![
                OsString::from("--inspect"),
                OsString::from("--output-dir"),
                OsString::from("target/inspect"),
                OsString::from("sample.gsp")
            ]
        );
    }
}
