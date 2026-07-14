use std::collections::BTreeMap;
use std::fs;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use gsp_rs::pipeline::compile_file_to_scene_json;
use serde::{Deserialize, Serialize};
use serde_json::Value;

const WORKER_PATH_ENV: &str = "GSP_OBJECT_GRAPH_AUDIT_WORKER_PATH";
const WORKER_REPORT_ENV: &str = "GSP_OBJECT_GRAPH_AUDIT_WORKER_REPORT";

#[derive(Debug, Serialize, Deserialize)]
struct PendingExample {
    path: PathBuf,
    operation: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct FileAuditReport {
    compile_failures: Vec<String>,
    pending_counts: BTreeMap<String, usize>,
    pending_examples: BTreeMap<String, Vec<PendingExample>>,
}

fn collect_gsp_files(root: &Path, output: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_gsp_files(&path, output);
        } else if path
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("gsp"))
        {
            output.push(path);
        }
    }
}

fn visit_object_graphs(
    value: &Value,
    path: &Path,
    counts: &mut BTreeMap<String, usize>,
    examples: &mut BTreeMap<String, Vec<PendingExample>>,
) {
    if let Some(graph) = value.get("objectGraph") {
        if graph
            .get("geometryComplete")
            .and_then(Value::as_bool)
            .is_some_and(|complete| !complete)
        {
            for operation in graph
                .get("pendingOperations")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
            {
                let base_category = if operation.starts_with("graph-validation:") {
                    "graph-validation".to_string()
                } else {
                    operation
                        .rsplit_once(':')
                        .map_or(operation, |(_, category)| category)
                        .to_string()
                };
                let category = pending_object_group_kind(value, operation)
                    .map_or(base_category.clone(), |kind| {
                        format!("{base_category}/{kind}")
                    });
                *counts.entry(category.clone()).or_default() += 1;
                let examples = examples.entry(category).or_default();
                if examples.len() < 5 && !examples.iter().any(|example| example.path == path) {
                    examples.push(PendingExample {
                        path: path.to_path_buf(),
                        operation: operation.to_string(),
                    });
                }
            }
        }
    }
    match value {
        Value::Array(values) => {
            for value in values {
                visit_object_graphs(value, path, counts, examples);
            }
        }
        Value::Object(values) => {
            for value in values.values() {
                visit_object_graphs(value, path, counts, examples);
            }
        }
        _ => {}
    }
}

fn pending_object_group_kind<'a>(scene: &'a Value, operation: &str) -> Option<&'a str> {
    let (object, rest) = operation.split_once(':')?;
    let (index, category) = rest.split_once(':')?;
    let collection = match (object, category) {
        ("point", "point-binding") => "points",
        ("line", "line-binding") => "lines",
        ("arc", "arc-binding") => "arcs",
        ("circle", "circle-binding") => "circles",
        ("polygon", "polygon-binding") => "polygons",
        _ => return None,
    };
    scene
        .get(collection)?
        .get(index.parse::<usize>().ok()?)?
        .get("debug")?
        .get("groupKind")?
        .as_str()
}

fn audit_file(path: &Path) -> FileAuditReport {
    let mut report = FileAuditReport::default();
    let compiled = catch_unwind(AssertUnwindSafe(|| {
        compile_file_to_scene_json(path, 960, 640)
    }));
    let Ok(compiled) = compiled else {
        report
            .compile_failures
            .push(format!("{}: compiler panicked", path.display()));
        return report;
    };
    match compiled {
        Ok(json) => match serde_json::from_str::<Value>(&json) {
            Ok(scene) => visit_object_graphs(
                &scene,
                path,
                &mut report.pending_counts,
                &mut report.pending_examples,
            ),
            Err(error) => report
                .compile_failures
                .push(format!("{}: invalid scene JSON: {error}", path.display())),
        },
        Err(error) => report
            .compile_failures
            .push(format!("{}: {error:#}", path.display())),
    }
    report
}

fn merge_report(
    report: FileAuditReport,
    compile_failures: &mut Vec<String>,
    pending_counts: &mut BTreeMap<String, usize>,
    pending_examples: &mut BTreeMap<String, Vec<PendingExample>>,
) {
    compile_failures.extend(report.compile_failures);
    for (category, count) in report.pending_counts {
        *pending_counts.entry(category).or_default() += count;
    }
    for (category, examples) in report.pending_examples {
        let merged = pending_examples.entry(category).or_default();
        for example in examples {
            if merged.len() >= 5 {
                break;
            }
            if !merged.iter().any(|existing| existing.path == example.path) {
                merged.push(example);
            }
        }
    }
}

fn audit_file_in_worker(path: &Path, timeout: Duration, worker_index: usize) -> FileAuditReport {
    let report_path = std::env::temp_dir().join(format!(
        "gsp-object-graph-audit-{}-{worker_index}.json",
        std::process::id()
    ));
    let mut child = match Command::new(std::env::current_exe().expect("resolve audit test binary"))
        .args([
            "--exact",
            "object_graph_audit_single_file_worker",
            "--ignored",
            "--nocapture",
        ])
        .env(WORKER_PATH_ENV, path)
        .env(WORKER_REPORT_ENV, &report_path)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(child) => child,
        Err(error) => {
            return FileAuditReport {
                compile_failures: vec![format!(
                    "{}: failed to start audit worker: {error}",
                    path.display()
                )],
                ..FileAuditReport::default()
            };
        }
    };
    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(10)),
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                break None;
            }
            Err(_) => break None,
        }
    };
    let report = if status.is_some_and(|status| status.success()) {
        fs::read(&report_path)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
    } else {
        None
    };
    let _ = fs::remove_file(&report_path);
    report.unwrap_or_else(|| FileAuditReport {
        compile_failures: vec![if status.is_none() {
            format!(
                "{}: compiler timed out after {} ms",
                path.display(),
                timeout.as_millis()
            )
        } else {
            format!("{}: audit worker failed", path.display())
        }],
        ..FileAuditReport::default()
    })
}

#[test]
#[ignore = "internal subprocess for the full corpus audit"]
fn object_graph_audit_single_file_worker() {
    let Some(path) = std::env::var_os(WORKER_PATH_ENV).map(PathBuf::from) else {
        return;
    };
    let report_path = std::env::var_os(WORKER_REPORT_ENV)
        .map(PathBuf::from)
        .expect("audit worker report path");
    let report = audit_file(&path);
    fs::write(report_path, serde_json::to_vec(&report).unwrap()).unwrap();
}

#[test]
#[ignore = "full corpus migration gate; run explicitly while ObjectGraph coverage is incomplete"]
fn every_test_gsp_has_a_complete_object_graph() {
    let root = std::env::var_os("GSP_OBJECT_GRAPH_AUDIT_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("tests"));
    let mut paths = Vec::new();
    collect_gsp_files(&root, &mut paths);
    paths.sort();
    if let Ok(filter) = std::env::var("GSP_OBJECT_GRAPH_AUDIT_FILTER") {
        paths.retain(|path| path.to_string_lossy().contains(&filter));
    }
    let shard_count = std::env::var("GSP_OBJECT_GRAPH_AUDIT_SHARD_COUNT")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|count| *count > 0)
        .unwrap_or(1);
    let shard_index = std::env::var("GSP_OBJECT_GRAPH_AUDIT_SHARD_INDEX")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(0);
    assert!(
        shard_index < shard_count,
        "audit shard index is out of range"
    );
    if shard_count > 1 {
        paths = paths
            .into_iter()
            .enumerate()
            .filter_map(|(index, path)| (index % shard_count == shard_index).then_some(path))
            .collect();
    }
    if let Some(limit) = std::env::var("GSP_OBJECT_GRAPH_AUDIT_LIMIT")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
    {
        paths.truncate(limit);
    }

    let mut compile_failures = Vec::new();
    let mut pending_counts = BTreeMap::new();
    let mut pending_examples = BTreeMap::new();
    let worker_timeout = std::env::var("GSP_OBJECT_GRAPH_AUDIT_TIMEOUT_MS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|milliseconds| *milliseconds > 0)
        .map(Duration::from_millis);
    let panic_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    for (index, path) in paths.iter().enumerate() {
        if index % 25 == 0 || paths.len() <= 25 {
            eprintln!("audited {index}/{}: {}", paths.len(), path.display());
        }
        let report = worker_timeout.map_or_else(
            || audit_file(path),
            |timeout| audit_file_in_worker(path, timeout, index),
        );
        merge_report(
            report,
            &mut compile_failures,
            &mut pending_counts,
            &mut pending_examples,
        );
    }
    std::panic::set_hook(panic_hook);

    let pending_report = pending_counts
        .iter()
        .map(|(category, count)| {
            let examples = pending_examples[category]
                .iter()
                .map(|example| format!("{}: {}", example.path.display(), example.operation))
                .collect::<Vec<_>>()
                .join("; ");
            format!("{category}: {count} (examples {examples})",)
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        compile_failures.is_empty() && pending_counts.is_empty(),
        "ObjectGraph corpus audit failed for {} files\ncompile failures: {}\n{}",
        paths.len(),
        compile_failures.join("\n"),
        pending_report,
    );
}
