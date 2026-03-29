use std::path::PathBuf;

#[derive(Debug)]
pub struct Config {
    pub gsp_path: PathBuf,
    pub html_path: PathBuf,
    pub render_width: u32,
    pub render_height: u32,
}

impl Config {
    pub fn parse(
        args: impl Iterator<Item = impl Into<std::ffi::OsString>>,
    ) -> Result<Self, String> {
        let raw_args: Vec<_> = args.map(Into::into).collect();
        if raw_args.is_empty() {
            return Err(Self::usage());
        }

        if raw_args.len() == 1 {
            let gsp_path = PathBuf::from(&raw_args[0]);
            let gsp_text = gsp_path.to_string_lossy();
            if matches!(gsp_text.as_ref(), "-h" | "--help") {
                return Err(Self::usage());
            }

            let html_path = gsp_path.with_extension("html");
            return Ok(Self {
                gsp_path,
                html_path,
                render_width: 800,
                render_height: 600,
            });
        }

        Err(Self::usage())
    }

    pub fn usage() -> String {
        "usage: gsp-rs <path/to/file.gsp>".to_string()
    }
}
