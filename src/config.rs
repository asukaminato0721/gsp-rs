use miette::{Result, miette};
use std::path::PathBuf;

const DEFAULT_UPLOAD_URL: &str = "https://gsp.dmath.net/upload.php";

#[derive(Debug)]
pub struct Config {
    pub jobs: Vec<RenderJob>,
    pub render_width: u32,
    pub render_height: u32,
    pub upload_url: Option<String>,
    pub mode: CompileMode,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderJob {
    pub gsp_path: PathBuf,
    pub html_path: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompileMode {
    Standard,
    HtmlOnly,
}

impl Config {
    pub fn parse(args: impl Iterator<Item = impl Into<std::ffi::OsString>>) -> Result<Self> {
        let raw_args: Vec<_> = args.map(Into::into).collect();
        if raw_args.is_empty() {
            return Err(miette!(Self::usage()));
        }

        if raw_args
            .iter()
            .any(|arg| matches!(arg.to_string_lossy().as_ref(), "-h" | "--help"))
        {
            return Err(miette!(Self::usage()));
        }

        let mut mode = CompileMode::Standard;
        let mut jobs = Vec::new();
        let mut upload_url = None;
        let mut index = 0usize;
        while index < raw_args.len() {
            let arg = raw_args[index].to_string_lossy();
            match arg.as_ref() {
                "--upload" => {
                    upload_url = Some(DEFAULT_UPLOAD_URL.to_string());
                }
                "--html" => {
                    mode = CompileMode::HtmlOnly;
                }
                value if value.starts_with('-') => {
                    return Err(miette!("unknown flag: {value}\n{}", Self::usage()));
                }
                _ => {
                    let gsp_path = PathBuf::from(&raw_args[index]);
                    jobs.push(RenderJob {
                        html_path: gsp_path.with_extension("html"),
                        gsp_path,
                    });
                }
            }
            index += 1;
        }

        if jobs.is_empty() {
            return Err(miette!(Self::usage()));
        }

        if mode == CompileMode::HtmlOnly {
            upload_url = None;
        }

        Ok(Self {
            jobs,
            render_width: 800,
            render_height: 600,
            upload_url,
            mode,
        })
    }

    pub fn usage() -> String {
        [
            "usage: gsp-rs [--upload] [--html] <path/to/file1.gsp> [path/to/file2.gsp ...]",
            "",
            "--upload uploads each successfully compiled .gsp file.",
            "--html only writes the bundled .html output.",
            "GSP_RS_WORKER_TIMEOUT_MS optionally sets a per-file compiler timeout; unset means no timeout.",
        ]
        .join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::{CompileMode, Config, RenderJob};
    use std::path::PathBuf;

    #[test]
    fn parses_multiple_input_paths() {
        let config = Config::parse(["a.gsp", "nested/b.gsp"].into_iter()).expect("config parses");
        assert_eq!(
            config.jobs,
            vec![
                RenderJob {
                    gsp_path: PathBuf::from("a.gsp"),
                    html_path: PathBuf::from("a.html"),
                },
                RenderJob {
                    gsp_path: PathBuf::from("nested/b.gsp"),
                    html_path: PathBuf::from("nested/b.html"),
                },
            ]
        );
        assert_eq!(config.mode, CompileMode::Standard);
        assert_eq!(config.upload_url, None);
    }

    #[test]
    fn rejects_unknown_flags() {
        let error = Config::parse(["--wat", "a.gsp"].into_iter()).expect_err("unknown flag");
        assert!(error.to_string().contains("unknown flag: --wat"));
    }

    #[test]
    fn upload_flag_enables_upload() {
        let config = Config::parse(["--upload", "a.gsp"].into_iter()).expect("config parses");
        assert_eq!(
            config.upload_url.as_deref(),
            Some(super::DEFAULT_UPLOAD_URL)
        );
    }

    #[test]
    fn parses_html_only_flag() {
        let config = Config::parse(["--html", "a.gsp", "nested/b.gsp"].into_iter())
            .expect("html-only config parses");
        assert_eq!(config.mode, CompileMode::HtmlOnly);
        assert_eq!(config.upload_url, None);
        assert_eq!(
            config.jobs,
            vec![
                RenderJob {
                    gsp_path: PathBuf::from("a.gsp"),
                    html_path: PathBuf::from("a.html"),
                },
                RenderJob {
                    gsp_path: PathBuf::from("nested/b.gsp"),
                    html_path: PathBuf::from("nested/b.html"),
                },
            ]
        );
    }

    #[test]
    fn upload_is_disabled_without_explicit_flag() {
        let config = Config::parse(["a.gsp"].into_iter()).expect("config parses");
        assert_eq!(config.upload_url, None);
    }

    #[test]
    fn help_flag_returns_usage() {
        assert!(Config::parse(["--help"].into_iter()).is_err());
    }
}
