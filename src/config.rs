use std::path::PathBuf;

#[derive(Debug)]
pub struct Config {
    pub gsp_path: PathBuf,
    pub reference_exe: Option<PathBuf>,
    pub render_path: Option<PathBuf>,
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

        let mut gsp_path = None;
        let mut reference_exe = None;
        let mut render_path = None;
        let mut render_width = 800_u32;
        let mut render_height = 600_u32;
        let mut index = 0usize;

        while index < raw_args.len() {
            let current = PathBuf::from(&raw_args[index]);
            let current_text = current.to_string_lossy();

            match current_text.as_ref() {
                "-h" | "--help" => return Err(Self::usage()),
                "--reference-exe" => {
                    index += 1;
                    let Some(path) = raw_args.get(index) else {
                        return Err("--reference-exe requires a path".to_string());
                    };
                    reference_exe = Some(PathBuf::from(path));
                }
                "--render" => {
                    index += 1;
                    let Some(path) = raw_args.get(index) else {
                        return Err("--render requires a path".to_string());
                    };
                    render_path = Some(PathBuf::from(path));
                }
                "--width" => {
                    index += 1;
                    let Some(value) = raw_args.get(index) else {
                        return Err("--width requires an integer".to_string());
                    };
                    render_width = parse_u32_arg(value, "--width")?;
                }
                "--height" => {
                    index += 1;
                    let Some(value) = raw_args.get(index) else {
                        return Err("--height requires an integer".to_string());
                    };
                    render_height = parse_u32_arg(value, "--height")?;
                }
                _ if current_text.starts_with("--") => {
                    return Err(format!("unknown option: {current_text}\n\n{}", Self::usage()));
                }
                _ if gsp_path.is_none() => gsp_path = Some(current),
                _ => {
                    return Err(format!(
                        "unexpected positional argument: {current_text}\n\n{}",
                        Self::usage()
                    ));
                }
            }

            index += 1;
        }

        let Some(gsp_path) = gsp_path else {
            return Err(Self::usage());
        };

        Ok(Self {
            gsp_path,
            reference_exe,
            render_path,
            render_width,
            render_height,
        })
    }

    pub fn usage() -> String {
        "usage: gsp-rs <path/to/file.gsp> [--reference-exe path/to/GSP5Chs.exe] [--render out.png] [--width 800] [--height 600]".to_string()
    }
}

fn parse_u32_arg(value: &std::ffi::OsString, flag: &str) -> Result<u32, String> {
    let text = value.to_string_lossy();
    text.parse::<u32>()
        .map_err(|error| format!("{flag} expects an unsigned integer, got {text:?}: {error}"))
}
