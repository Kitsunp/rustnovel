use crate::editor::errors::EditorError;
use crate::editor::{node_graph::NodeGraph, script_sync};
use std::path::{Component, Path, PathBuf};
use visual_novel_engine::{
    manifest::{ManifestMigrationReport, ProjectManifest},
    ScriptRaw,
};

pub struct LoadedProject {
    pub manifest: ProjectManifest,
    pub manifest_migration_report: Option<ManifestMigrationReport>,
    pub entry_point_script: Option<(PathBuf, LoadedScript)>,
}

pub struct LoadedScript {
    pub graph: NodeGraph,
    pub was_imported: bool,
}

pub(crate) fn resolve_existing_project_path(
    root: &Path,
    requested: &Path,
) -> Result<Option<PathBuf>, EditorError> {
    let canonical_root = root.canonicalize().map_err(EditorError::IoError)?;
    let candidate = if requested.is_absolute() {
        requested.to_path_buf()
    } else {
        if requested.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        }) {
            return Err(EditorError::CompileError(format!(
                "Path escapes project root: {}",
                requested.display()
            )));
        }
        canonical_root.join(requested)
    };

    if requested.is_absolute() && !candidate.starts_with(&canonical_root) {
        return Err(EditorError::CompileError(format!(
            "Path escapes project root: {}",
            requested.display()
        )));
    }

    if !candidate.exists() {
        return Ok(None);
    }
    if !candidate.is_file() {
        return Ok(None);
    }

    let canonical_candidate = candidate.canonicalize().map_err(EditorError::IoError)?;
    if !canonical_candidate.starts_with(&canonical_root) {
        return Err(EditorError::CompileError(format!(
            "Path escapes project root after canonicalization: {}",
            requested.display()
        )));
    }

    Ok(Some(canonical_candidate))
}

pub fn load_project(path: PathBuf) -> Result<LoadedProject, EditorError> {
    // 1. Load Manifest (TOML)
    let manifest_content = std::fs::read_to_string(&path).map_err(EditorError::IoError)?;

    let (manifest, migration_report) = ProjectManifest::from_toml_with_migration(&manifest_content)
        .map_err(|e| {
            EditorError::IoError(std::io::Error::new(std::io::ErrorKind::InvalidData, e))
        })?;
    let manifest_migration_report = migration_report.changed().then_some(migration_report);

    // 2. Load Entry Point Script if exists
    let entry_point_script = {
        let project_root = path.parent().unwrap_or(&path);
        match resolve_existing_project_path(
            project_root,
            Path::new(&manifest.settings.entry_point),
        )? {
            Some(script_path) => Some((script_path.clone(), load_script(script_path)?)),
            None => None,
        }
    };

    Ok(LoadedProject {
        manifest,
        manifest_migration_report,
        entry_point_script,
    })
}

pub fn load_script(path: PathBuf) -> Result<LoadedScript, EditorError> {
    let content = std::fs::read_to_string(&path).map_err(EditorError::IoError)?;

    // Try parsing as ScriptRaw (JSON)
    let script = ScriptRaw::from_json(&content)
        .map_err(|e| EditorError::CompileError(format!("Parse error: {}", e)))?;

    let graph = script_sync::from_script(&script);
    Ok(LoadedScript {
        graph,
        was_imported: false,
    })
}

pub fn save_script(path: &std::path::Path, graph: &NodeGraph) -> Result<(), EditorError> {
    let script = script_sync::to_script(graph);
    let json = script
        .to_json()
        .map_err(|e| EditorError::CompileError(format!("Serialization error: {}", e)))?;

    std::fs::write(path, json).map_err(EditorError::IoError)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;
    use visual_novel_engine::manifest::MANIFEST_SCHEMA_VERSION;

    #[test]
    fn load_project_applies_manifest_migration_for_legacy_schema() {
        let dir = tempdir().expect("tempdir");
        let manifest_path = dir.path().join("project.vnm");
        let script_path = dir.path().join("main.json");

        let legacy_manifest = r#"
schema_version = "0.9"

[metadata]
name = "Legacy Project"
author = "QA"
version = "0.1.0"

[settings]
resolution = [1280, 720]
default_language = "es"
supported_languages = ["es", "en"]
entry_point = "main.json"

[assets]
"#;
        fs::write(&manifest_path, legacy_manifest).expect("write manifest");
        fs::write(
            &script_path,
            r#"{
  "script_schema_version": "1.0",
  "events": [
    { "type": "dialogue", "speaker": "Narrador", "text": "Hola" }
  ],
  "labels": { "start": 0 }
}"#,
        )
        .expect("write script");

        let loaded = load_project(manifest_path).expect("legacy manifest should load");
        assert_eq!(
            loaded.manifest.manifest_schema_version,
            MANIFEST_SCHEMA_VERSION
        );
        assert!(loaded.manifest_migration_report.is_some());
        assert!(loaded.entry_point_script.is_some());
    }

    #[test]
    fn load_project_rejects_entry_point_escape_outside_root() {
        let dir = tempdir().expect("tempdir");
        let project_root = dir.path().join("project");
        fs::create_dir_all(&project_root).expect("mkdir project");
        let manifest_path = project_root.join("project.vnm");
        let outside_script = dir.path().join("outside.json");

        fs::write(
            &outside_script,
            r#"{
  "script_schema_version": "1.0",
  "events": [],
  "labels": {}
}"#,
        )
        .expect("write outside script");

        let manifest = r#"
schema_version = "1.0"

[metadata]
name = "Escape Test"
author = "QA"
version = "0.1.0"

[settings]
resolution = [1280, 720]
default_language = "en"
supported_languages = ["en"]
entry_point = "../outside.json"

[assets]
"#;
        fs::write(&manifest_path, manifest).expect("write manifest");

        match load_project(manifest_path) {
            Ok(_) => panic!("escape must be rejected"),
            Err(EditorError::CompileError(message)) => {
                assert!(message.contains("escapes project root"))
            }
            Err(other) => panic!("unexpected error: {other}"),
        }
    }
}
