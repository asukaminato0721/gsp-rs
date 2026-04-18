use miette::{Result, miette};
use std::path::PathBuf;

const DEFAULT_UPLOAD_URL: &str = "https://gsp.dmath.net/upload.php";

#[derive(Debug)]
pub struct Config {
    pub jobs: Vec<RenderJob>,
    pub render_width: u32,
    pub render_height: u32,
    pub upload_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderJob {
    pub gsp_path: PathBuf,
    pub html_path: PathBuf,
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

        let mut jobs = Vec::new();
        let mut upload_url = None;
        let mut index = 0usize;
        while index < raw_args.len() {
            let arg = raw_args[index].to_string_lossy();
            match arg.as_ref() {
                "--upload" => {
                    upload_url = Some(DEFAULT_UPLOAD_URL.to_string());
                }
                "--upload-url" => {
                    let Some(value) = raw_args.get(index + 1) else {
                        return Err(miette!("missing value for --upload-url\n{}", Self::usage()));
                    };
                    upload_url = Some(value.to_string_lossy().into_owned());
                    index += 1;
                }
                value if value.starts_with("--upload-url=") => {
                    let provided = value.trim_start_matches("--upload-url=");
                    if provided.is_empty() {
                        return Err(miette!("missing value for --upload-url\n{}", Self::usage()));
                    }
                    upload_url = Some(provided.to_string());
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

        Ok(Self {
            jobs,
            render_width: 800,
            render_height: 600,
            upload_url,
        })
    }

    pub fn usage() -> String {
        "usage: gsp-rs [--upload] [--upload-url <url>] <path/to/file1.gsp> [path/to/file2.gsp ...]"
            .to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::{Config, RenderJob};
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
        assert_eq!(config.upload_url, None);
    }

    #[test]
    fn rejects_unknown_flags() {
        let error = Config::parse(["--wat", "a.gsp"].into_iter()).expect_err("unknown flag");
        assert!(error.to_string().contains("unknown flag: --wat"));
    }

    #[test]
    fn upload_flag_enables_default_upload_endpoint() {
        let config = Config::parse(["--upload", "a.gsp"].into_iter()).expect("config parses");
        assert_eq!(
            config.upload_url.as_deref(),
            Some(super::DEFAULT_UPLOAD_URL)
        );
    }

    #[test]
    fn upload_url_flag_overrides_upload_endpoint() {
        let config =
            Config::parse(["--upload-url", "https://example.test/upload", "a.gsp"].into_iter())
                .expect("config parses");
        assert_eq!(
            config.upload_url.as_deref(),
            Some("https://example.test/upload")
        );
    }

    #[test]
    fn inline_upload_url_flag_enables_upload() {
        let config =
            Config::parse(["--upload-url=https://example.test/upload", "a.gsp"].into_iter())
                .expect("config parses");
        assert_eq!(
            config.upload_url.as_deref(),
            Some("https://example.test/upload")
        );
    }

    #[test]
    fn rejects_missing_upload_url_value() {
        let error = Config::parse(["--upload-url"].into_iter()).expect_err("missing url");
        assert!(error.to_string().contains("missing value for --upload-url"));
    }

    #[test]
    fn help_flag_returns_usage() {
        assert!(Config::parse(["--help"].into_iter()).is_err());
    }
}
