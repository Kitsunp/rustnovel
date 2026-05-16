use crate::editor::authoring_adapter::{from_authoring_graph, to_authoring_graph};
use crate::editor::errors::EditorError;
use crate::editor::node_graph::NodeGraph;
use std::path::{Component, Path, PathBuf};
use visual_novel_engine::{
    authoring::{
        composer::LayerOverride, export_runtime_script_from_authoring,
        parse_authoring_document_or_script, AuthoringDocument, OperationLogEntry, VerificationRun,
    },
    manifest::{ManifestMigrationReport, ProjectManifest},
};

pub struct LoadedProject {
    pub manifest: ProjectManifest,
    pub manifest_migration_report: Option<ManifestMigrationReport>,
    pub entry_point_script: Option<(PathBuf, LoadedScript)>,
}

pub struct LoadedScript {
    pub graph: NodeGraph,
    pub was_imported: bool,
    pub composer_layer_overrides: std::collections::HashMap<String, LayerOverride>,
    pub operation_log: Vec<OperationLogEntry>,
    pub verification_runs: Vec<VerificationRun>,
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
    let source = std::fs::read_to_string(&path).map_err(EditorError::IoError)?;
    if let Ok(document) = AuthoringDocument::from_json(&source) {
        return Ok(LoadedScript {
            graph: from_authoring_graph(&document.graph),
            was_imported: false,
            composer_layer_overrides: document.composer_layer_overrides.into_iter().collect(),
            operation_log: document.operation_log,
            verification_runs: document.verification_runs,
        });
    }
    let graph = parse_authoring_document_or_script(&source)
        .map_err(|e| EditorError::CompileError(format!("Parse error: {}", e)))?;
    Ok(LoadedScript {
        graph: from_authoring_graph(&graph),
        was_imported: false,
        composer_layer_overrides: std::collections::HashMap::new(),
        operation_log: Vec::new(),
        verification_runs: Vec::new(),
    })
}

pub fn save_script(path: &std::path::Path, graph: &NodeGraph) -> Result<(), EditorError> {
    save_authoring_document(path, graph)
}

pub fn save_authoring_document(
    path: &std::path::Path,
    graph: &NodeGraph,
) -> Result<(), EditorError> {
    save_authoring_document_with_metadata(path, graph, &std::collections::HashMap::new(), &[], &[])
}

pub fn save_authoring_document_with_metadata(
    path: &std::path::Path,
    graph: &NodeGraph,
    composer_layer_overrides: &std::collections::HashMap<String, LayerOverride>,
    operation_log: &[OperationLogEntry],
    verification_runs: &[VerificationRun],
) -> Result<(), EditorError> {
    let mut document = AuthoringDocument::new(to_authoring_graph(graph));
    document.composer_layer_overrides = composer_layer_overrides
        .iter()
        .map(|(key, value)| (key.clone(), *value))
        .collect();
    document.operation_log = operation_log.to_vec();
    document.verification_runs = verification_runs.to_vec();
    let json = document
        .to_json()
        .map_err(|e| EditorError::CompileError(format!("Serialization error: {}", e)))?;

    std::fs::write(path, json).map_err(EditorError::IoError)?;

    Ok(())
}

pub fn export_runtime_script(path: &std::path::Path, graph: &NodeGraph) -> Result<(), EditorError> {
    let script = export_runtime_script_from_authoring(&to_authoring_graph(graph))
        .map_err(|e| EditorError::CompileError(format!("Strict export error: {}", e)))?;
    let json = script
        .to_json()
        .map_err(|e| EditorError::CompileError(format!("Serialization error: {}", e)))?;
    std::fs::write(path, json).map_err(EditorError::IoError)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor::StoryNode;
    use eframe::egui;
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

    #[test]
    fn authoring_save_load_preserves_disconnected_draft_nodes() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("draft.vnauthoring");
        let mut graph = NodeGraph::new();
        let start = graph.add_node(StoryNode::Start, egui::pos2(0.0, 0.0));
        let live = graph.add_node(
            StoryNode::Dialogue {
                speaker: "Narrator".to_string(),
                text: "Connected".to_string(),
            },
            egui::pos2(0.0, 100.0),
        );
        let draft = graph.add_node(
            StoryNode::Dialogue {
                speaker: "Draft".to_string(),
                text: "Disconnected but important".to_string(),
            },
            egui::pos2(240.0, 100.0),
        );
        graph.connect(start, live);

        save_script(&path, &graph).expect("save authoring document");
        let saved = fs::read_to_string(&path).expect("read saved document");
        assert!(saved.contains("authoring_schema_version"));

        let loaded = load_script(path).expect("load authoring document");
        assert_eq!(loaded.graph.len(), graph.len());
        assert!(matches!(
            loaded.graph.get_node(draft),
            Some(StoryNode::Dialogue { speaker, text })
                if speaker == "Draft" && text == "Disconnected but important"
        ));
    }

    #[test]
    fn authoring_save_load_preserves_operation_log_and_verification_runs() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("tracked.vnauthoring");
        let mut graph = NodeGraph::new();
        graph.add_node(StoryNode::Start, egui::pos2(0.0, 0.0));
        let operation = visual_novel_engine::authoring::OperationLogEntry::new_typed(
            visual_novel_engine::authoring::OperationKind::NodeCreated,
            "applied",
            "created start node",
        );
        let verification = visual_novel_engine::authoring::VerificationRun::from_diagnostics(
            &operation.operation_id,
            "gui-save",
            &visual_novel_engine::authoring::build_authoring_report_fingerprint(
                &to_authoring_graph(&graph),
                &to_authoring_graph(&graph).to_script_lossy_for_diagnostics(),
            ),
            &[],
            &[],
        );

        save_authoring_document_with_metadata(
            &path,
            &graph,
            &std::collections::HashMap::new(),
            std::slice::from_ref(&operation),
            std::slice::from_ref(&verification),
        )
        .expect("save with metadata");

        let loaded = load_script(path).expect("load with metadata");
        assert_eq!(loaded.operation_log.len(), 1);
        assert_eq!(loaded.verification_runs.len(), 1);
        assert_eq!(
            loaded.operation_log[0].operation_kind,
            operation.operation_kind
        );
    }

    #[test]
    fn authoring_save_load_preserves_composer_layer_overrides() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("layers.vnauthoring");
        let mut graph = NodeGraph::new();
        graph.add_node(StoryNode::Start, egui::pos2(0.0, 0.0));
        let mut overrides = std::collections::HashMap::new();
        overrides.insert(
            "node:1:character:graph.nodes[1].visual.characters[0]:0".to_string(),
            visual_novel_engine::authoring::composer::LayerOverride {
                visible: false,
                locked: true,
            },
        );

        save_authoring_document_with_metadata(&path, &graph, &overrides, &[], &[])
            .expect("save layer overrides");
        let loaded = load_script(path).expect("load layer overrides");

        assert_eq!(loaded.composer_layer_overrides, overrides);
    }
}
