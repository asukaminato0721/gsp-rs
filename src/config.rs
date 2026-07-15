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
    pub output_dir: Option<PathBuf>,
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
        let mut inputs = Vec::new();
        let mut upload_url = None;
        let mut output_dir = None;
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
                "--output-dir" => {
                    index += 1;
                    let Some(value) = raw_args.get(index) else {
                        return Err(miette!("--output-dir requires a path\n{}", Self::usage()));
                    };
                    output_dir = Some(PathBuf::from(value));
                }
                value if value.starts_with('-') => {
                    return Err(miette!("unknown flag: {value}\n{}", Self::usage()));
                }
                _ => {
                    inputs.push(PathBuf::from(&raw_args[index]));
                }
            }
            index += 1;
        }

        if inputs.is_empty() {
            return Err(miette!(Self::usage()));
        }

        let jobs = inputs
            .into_iter()
            .map(|gsp_path| RenderJob {
                html_path: output_path(&gsp_path, output_dir.as_deref()),
                gsp_path,
            })
            .collect();

        if mode == CompileMode::HtmlOnly {
            upload_url = None;
        }

        Ok(Self {
            jobs,
            render_width: 800,
            render_height: 600,
            upload_url,
            mode,
            output_dir,
        })
    }

    pub fn usage() -> String {
        [
            "usage: gsp-rs [--upload] [--html] [--output-dir DIR] <file1.gsp> [file2.gsp ...]",
            "",
            "--upload uploads each successfully compiled .gsp file.",
            "--html only writes the bundled .html output.",
            "--output-dir mirrors output artifacts below DIR.",
            "GSP_RS_WORKER_TIMEOUT_MS optionally sets a per-file compiler timeout; unset means no timeout.",
        ]
        .join("\n")
    }
}

fn output_path(gsp_path: &std::path::Path, output_dir: Option<&std::path::Path>) -> PathBuf {
    let Some(output_dir) = output_dir else {
        return gsp_path.with_extension("html");
    };
    let relative = if gsp_path.is_absolute() {
        std::env::current_dir()
            .ok()
            .and_then(|cwd| gsp_path.strip_prefix(cwd).ok().map(PathBuf::from))
            .or_else(|| gsp_path.file_name().map(PathBuf::from))
            .unwrap_or_else(|| PathBuf::from("output.gsp"))
    } else {
        gsp_path.to_path_buf()
    };
    output_dir.join(relative).with_extension("html")
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
    fn mirrors_outputs_below_the_requested_directory() {
        let config =
            Config::parse(["--output-dir", "target/corpus", "tests/nested/a.gsp"].into_iter())
                .expect("config parses");
        assert_eq!(config.output_dir, Some(PathBuf::from("target/corpus")));
        assert_eq!(
            config.jobs[0].html_path,
            PathBuf::from("target/corpus/tests/nested/a.html")
        );
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
