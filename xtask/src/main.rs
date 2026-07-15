use std::{
    env, fs, io,
    path::{Path, PathBuf},
    process::{Command, ExitCode, Stdio},
    time::Instant,
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
        Some("corpus") => run_corpus(args.collect()),
        Some(command) => Err(format!("unknown xtask command `{command}`").into()),
        None => {
            eprintln!(
                "usage: cargo xtask <runtime|check-runtime|build|corpus>\n\
                 corpus options: [--root DIR] [--output-dir DIR] [--jobs N] \
                 [--timeout-ms N] [--html] [--no-build]"
            );
            Ok(())
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
struct CorpusOptions {
    root: PathBuf,
    output_dir: PathBuf,
    jobs: usize,
    timeout_ms: u64,
    html_only: bool,
    build: bool,
}

impl Default for CorpusOptions {
    fn default() -> Self {
        Self {
            root: PathBuf::from("tests"),
            output_dir: PathBuf::from("target/xtask-gsp-corpus"),
            jobs: std::thread::available_parallelism()
                .map_or(1, usize::from)
                .clamp(1, 4),
            timeout_ms: 5_000,
            html_only: false,
            build: true,
        }
    }
}

fn parse_corpus_options(args: Vec<String>) -> Result<CorpusOptions, Box<dyn std::error::Error>> {
    let mut options = CorpusOptions::default();
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--root" | "--output-dir" | "--jobs" | "--timeout-ms" => {
                let flag = args[index].clone();
                index += 1;
                let value = args
                    .get(index)
                    .ok_or_else(|| format!("{flag} requires a value"))?;
                match flag.as_str() {
                    "--root" => options.root = PathBuf::from(value),
                    "--output-dir" => options.output_dir = PathBuf::from(value),
                    "--jobs" => options.jobs = value.parse::<usize>()?.max(1),
                    "--timeout-ms" => options.timeout_ms = value.parse::<u64>()?.max(1),
                    _ => unreachable!(),
                }
            }
            "--html" => options.html_only = true,
            "--no-build" => options.build = false,
            unknown => return Err(format!("unknown corpus option `{unknown}`").into()),
        }
        index += 1;
    }
    Ok(options)
}

fn run_corpus(args: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
    let options = parse_corpus_options(args)?;
    let root = repo_root();
    if options.build {
        run_command(
            Command::new("cargo").args(["build", "--release", "--bin", "gsp-rs"]),
            &root,
        )?;
    }

    let corpus_root = root.join(&options.root);
    let mut files = Vec::new();
    collect_gsp_files(&corpus_root, &mut files)?;
    if files.is_empty() {
        return Err(format!("no .gsp files found below {}", corpus_root.display()).into());
    }
    let mut weighted_files = files
        .into_iter()
        .map(|path| {
            let weight = fs::metadata(&path)
                .map(|metadata| metadata.len())
                .unwrap_or(0);
            let path = path.strip_prefix(&root).unwrap_or(&path).to_path_buf();
            (path, weight)
        })
        .collect::<Vec<_>>();
    weighted_files.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
    let shards = balance_shards(weighted_files, options.jobs);
    let output_dir = root.join(&options.output_dir);
    fs::create_dir_all(&output_dir)?;
    let compiler = root.join("target/release/gsp-rs");
    let started = Instant::now();
    eprintln!(
        "corpus: {} files, {} workers, {} ms per file",
        shards.iter().map(Vec::len).sum::<usize>(),
        shards.len(),
        options.timeout_ms
    );

    let mut children = Vec::with_capacity(shards.len());
    for shard in shards {
        let mut command = Command::new(&compiler);
        command
            .current_dir(&root)
            .env("GSP_RS_WORKER_TIMEOUT_MS", options.timeout_ms.to_string())
            .arg("--output-dir")
            .arg(&output_dir)
            .stdout(Stdio::null())
            .stderr(Stdio::inherit());
        if options.html_only {
            command.arg("--html");
        }
        command.args(shard);
        children.push(command.spawn()?);
    }

    let mut failed_workers = 0usize;
    for mut child in children {
        if !child.wait()?.success() {
            failed_workers += 1;
        }
    }
    eprintln!(
        "corpus: finished in {:.2}s",
        started.elapsed().as_secs_f64()
    );
    if failed_workers == 0 {
        Ok(())
    } else {
        Err(format!("{failed_workers} corpus worker(s) reported failed files").into())
    }
}

fn collect_gsp_files(root: &Path, files: &mut Vec<PathBuf>) -> io::Result<()> {
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            collect_gsp_files(&path, files)?;
        } else if path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("gsp"))
        {
            files.push(path);
        }
    }
    Ok(())
}

fn balance_shards(files: Vec<(PathBuf, u64)>, jobs: usize) -> Vec<Vec<PathBuf>> {
    let shard_count = jobs.max(1).min(files.len().max(1));
    let mut shards = vec![Vec::new(); shard_count];
    let mut weights = vec![0u64; shard_count];
    for (path, weight) in files {
        let index = weights
            .iter()
            .enumerate()
            .min_by_key(|(_, weight)| **weight)
            .map(|(index, _)| index)
            .unwrap_or(0);
        shards[index].push(path);
        weights[index] = weights[index].saturating_add(weight);
    }
    shards
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

#[cfg(test)]
mod tests {
    use super::{CorpusOptions, balance_shards, parse_corpus_options};
    use std::path::PathBuf;

    #[test]
    fn parses_corpus_overrides() {
        let options = parse_corpus_options(vec![
            "--root".into(),
            "fixtures".into(),
            "--jobs".into(),
            "3".into(),
            "--timeout-ms".into(),
            "2500".into(),
            "--html".into(),
            "--no-build".into(),
        ])
        .expect("options parse");
        assert_eq!(
            options,
            CorpusOptions {
                root: PathBuf::from("fixtures"),
                output_dir: PathBuf::from("target/xtask-gsp-corpus"),
                jobs: 3,
                timeout_ms: 2500,
                html_only: true,
                build: false,
            }
        );
    }

    #[test]
    fn balances_largest_files_across_workers() {
        let shards = balance_shards(
            vec![
                (PathBuf::from("a.gsp"), 10),
                (PathBuf::from("b.gsp"), 8),
                (PathBuf::from("c.gsp"), 5),
                (PathBuf::from("d.gsp"), 3),
            ],
            2,
        );
        assert_eq!(shards[0], [PathBuf::from("a.gsp"), PathBuf::from("d.gsp")]);
        assert_eq!(shards[1], [PathBuf::from("b.gsp"), PathBuf::from("c.gsp")]);
    }
}
