use serde_json::Value;
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use super::{
    artifacts::ArtifactPaths, compile_bytes_to_html_document, compile_bytes_to_scene_json,
    compile_file_to_html, compile_file_to_html_only,
};

pub(super) const FIXTURE_WIDTH: u32 = 800;
pub(super) const FIXTURE_HEIGHT: u32 = 600;

pub(super) fn fixture_html(data: &[u8], message: &str) -> String {
    compile_bytes_to_html_document(data, FIXTURE_WIDTH, FIXTURE_HEIGHT).expect(message)
}

pub(super) fn fixture_scene_json(data: &[u8], message: &str) -> String {
    compile_bytes_to_scene_json(data, FIXTURE_WIDTH, FIXTURE_HEIGHT).expect(message)
}

pub(super) fn fixture_scene(data: &[u8], message: &str) -> Value {
    serde_json::from_str(&fixture_scene_json(data, message))
        .expect("scene json should be valid json")
}

pub(super) fn fixture_scene_error(data: &[u8]) -> String {
    compile_bytes_to_scene_json(data, FIXTURE_WIDTH, FIXTURE_HEIGHT)
        .expect_err("fixture should be rejected before export")
        .to_string()
}

pub(super) fn fixture_bytes(path: &str) -> Option<Vec<u8>> {
    fs::read(path).ok()
}

pub(super) fn standard_fixture_output(prefix: &str, path: &str) -> Option<StandardFixtureOutput> {
    Some(FixtureArtifacts::from_fixture_path(prefix, path)?.compile_standard_with_outputs())
}

pub(super) fn collect_kind_literals(text: &str) -> BTreeSet<String> {
    let mut kinds = BTreeSet::new();
    let needle = "\"kind\": \"";
    let mut rest = text;
    while let Some(start) = rest.find(needle) {
        let suffix = &rest[start + needle.len()..];
        let Some(end) = suffix.find('"') else {
            break;
        };
        kinds.insert(suffix[..end].to_string());
        rest = &suffix[end + 1..];
    }
    kinds
}

pub(super) struct FixtureArtifacts {
    _root: PathBuf,
    pub(super) paths: ArtifactPaths,
    gsp_path: PathBuf,
}

pub(super) struct StandardFixtureOutput {
    pub(super) html: String,
    pub(super) payload_log: String,
    pub(super) debug_json: String,
    pub(super) scene: Value,
}

impl FixtureArtifacts {
    pub(super) fn new(prefix: &str, file_name: &str, data: &[u8]) -> Self {
        let root = unique_test_dir(prefix);
        fs::create_dir_all(&root).expect("temporary directory should be creatable");

        let gsp_path = root.join(file_name);
        fs::write(&gsp_path, data).expect("fixture gsp should be writable");
        let paths = ArtifactPaths::from_gsp(&gsp_path);

        Self {
            _root: root,
            paths,
            gsp_path,
        }
    }

    pub(super) fn from_fixture_path(prefix: &str, path: &str) -> Option<Self> {
        let data = fixture_bytes(path)?;
        let file_name = Path::new(path).file_name()?.to_string_lossy();
        Some(Self::new(prefix, &file_name, &data))
    }

    pub(super) fn compile_standard(&self) {
        compile_file_to_html(
            &self.gsp_path,
            &self.paths.html_path,
            FIXTURE_WIDTH,
            FIXTURE_HEIGHT,
        )
        .expect("fixture should compile to html");
    }

    pub(super) fn compile_standard_with_outputs(&self) -> StandardFixtureOutput {
        self.compile_standard();
        self.assert_standard_artifacts_exist();
        let html = read_to_string(&self.paths.html_path, "html output");
        let payload_log = self.read_log();
        let debug_json = self.read_debug_json();
        let scene =
            serde_json::from_str(&debug_json).expect("debug json should be valid scene json");
        StandardFixtureOutput {
            html,
            payload_log,
            debug_json,
            scene,
        }
    }

    pub(super) fn compile_html_only(&self) {
        compile_file_to_html_only(
            &self.gsp_path,
            &self.paths.html_path,
            FIXTURE_WIDTH,
            FIXTURE_HEIGHT,
        )
        .expect("fixture should compile to html without sidecars");
    }

    pub(super) fn assert_standard_artifacts_exist(&self) {
        assert!(
            self.paths.html_path.exists(),
            "expected html output to be written"
        );
        assert!(
            self.paths.payload_log_path.exists(),
            "expected payload log to be written"
        );
        assert!(
            self.paths.debug_json_path.exists(),
            "expected debug json output to be written"
        );
    }

    pub(super) fn read_log(&self) -> String {
        read_to_string(&self.paths.payload_log_path, "payload log")
    }

    pub(super) fn read_debug_json(&self) -> String {
        read_to_string(&self.paths.debug_json_path, "debug json")
    }
}

impl Drop for FixtureArtifacts {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self._root);
    }
}

fn read_to_string(path: &Path, artifact_name: &str) -> String {
    fs::read_to_string(path).unwrap_or_else(|error| {
        panic!(
            "{artifact_name} should be readable at {}: {error}",
            path.display()
        )
    })
}

fn unique_test_dir(prefix: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time should move forward")
        .as_nanos();
    std::env::temp_dir().join(format!("gsp-rs-{prefix}-{unique}"))
}
