use std::path::PathBuf;

#[derive(Debug)]
pub struct Config {
    pub jobs: Vec<RenderJob>,
    pub render_width: u32,
    pub render_height: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderJob {
    pub gsp_path: PathBuf,
    pub html_path: PathBuf,
}

impl Config {
    pub fn parse(
        args: impl Iterator<Item = impl Into<std::ffi::OsString>>,
    ) -> Result<Self, String> {
        let raw_args: Vec<_> = args.map(Into::into).collect();
        if raw_args.is_empty() {
            return Err(Self::usage());
        }

        if raw_args
            .iter()
            .any(|arg| matches!(arg.to_string_lossy().as_ref(), "-h" | "--help"))
        {
            return Err(Self::usage());
        }

        let jobs = raw_args
            .into_iter()
            .map(PathBuf::from)
            .map(|gsp_path| RenderJob {
                html_path: gsp_path.with_extension("html"),
                gsp_path,
            })
            .collect();

        Ok(Self {
            jobs,
            render_width: 800,
            render_height: 600,
        })
    }

    pub fn usage() -> String {
        "usage: gsp-rs <path/to/file1.gsp> [path/to/file2.gsp ...]".to_string()
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
    }

    #[test]
    fn help_flag_returns_usage() {
        assert!(Config::parse(["--help"].into_iter()).is_err());
    }
}
