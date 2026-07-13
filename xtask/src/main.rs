use std::{
    env, io,
    path::{Path, PathBuf},
    process::{Command, ExitCode},
};

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("xtask: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args().skip(1);
    match args.next().as_deref() {
        Some("runtime") => build_runtime(),
        Some("check-runtime") => {
            build_runtime()?;
            run_command(
                Command::new("npm").args(["run", "typecheck:js"]),
                repo_root(),
            )?;
            Ok(())
        }
        Some("build") => {
            build_runtime()?;
            let mut cargo_args = vec!["build".to_string()];
            cargo_args.extend(args);
            let mut command = Command::new("cargo");
            command.args(cargo_args);
            run_command(&mut command, repo_root())?;
            Ok(())
        }
        Some(command) => Err(format!("unknown xtask command `{command}`").into()),
        None => {
            eprintln!("usage: cargo xtask <runtime|check-runtime|build>");
            Ok(())
        }
    }
}

fn build_runtime() -> Result<(), Box<dyn std::error::Error>> {
    let root = repo_root();
    gsp_runtime_assets::build_runtime_assets(
        &root,
        &root.join("src/html/generated"),
        &root.join("target/runtime-assets"),
    )?;
    Ok(())
}

fn run_command(command: &mut Command, cwd: impl AsRef<Path>) -> io::Result<()> {
    let status = command.current_dir(cwd).status()?;
    if status.success() {
        return Ok(());
    }
    Err(io::Error::other(format!(
        "command failed with status {status}: {command:?}",
    )))
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask crate should live under repository root")
        .to_path_buf()
}
