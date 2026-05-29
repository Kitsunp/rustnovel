use crate::editor::authoring_adapter::{from_authoring_graph, to_authoring_graph};
use crate::editor::errors::EditorError;
use crate::editor::node_graph::NodeGraph;
use std::path::{Component, Path, PathBuf};
use visual_novel_engine::{
    authoring::{
        composer::{BackgroundFit, LayerOverride},
        export_runtime_script_from_authoring, parse_authoring_document_or_script,
        AuthoringDocument, OperationLogEntry, VerificationRun,
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
    pub composer_background_fit_overrides: std::collections::HashMap<String, BackgroundFit>,
    pub operation_log: Vec<OperationLogEntry>,
    pub verification_runs: Vec<VerificationRun>,
}

pub fn resolve_existing_project_path(
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
            composer_background_fit_overrides: document
                .composer_background_fit_overrides
                .into_iter()
                .collect(),
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
        composer_background_fit_overrides: std::collections::HashMap::new(),
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
    save_authoring_document_with_metadata(
        path,
        graph,
        &std::collections::HashMap::new(),
        &std::collections::HashMap::new(),
        &[],
        &[],
    )
}

pub fn save_authoring_document_with_metadata(
    path: &std::path::Path,
    graph: &NodeGraph,
    composer_layer_overrides: &std::collections::HashMap<String, LayerOverride>,
    composer_background_fit_overrides: &std::collections::HashMap<String, BackgroundFit>,
    operation_log: &[OperationLogEntry],
    verification_runs: &[VerificationRun],
) -> Result<(), EditorError> {
    let mut document = AuthoringDocument::new(to_authoring_graph(graph));
    document.composer_layer_overrides = composer_layer_overrides
        .iter()
        .map(|(key, value)| (key.clone(), *value))
        .collect();
    document.composer_background_fit_overrides = composer_background_fit_overrides
        .iter()
        .map(|(key, value)| (key.clone(), *value))
        .collect();
    document.operation_log = operation_log.to_vec();
    document.verification_runs = verification_runs.to_vec();
    let json = document
        .to_json()
        .map_err(|e| EditorError::CompileError(format!("Serialization error: {}", e)))?;

    crate::editor::atomic_io::atomic_replace(path, json.as_bytes())
        .map_err(EditorError::IoError)?;

    Ok(())
}

pub fn export_runtime_script(path: &std::path::Path, graph: &NodeGraph) -> Result<(), EditorError> {
    let script = export_runtime_script_from_authoring(&to_authoring_graph(graph))
        .map_err(|e| EditorError::CompileError(format!("Strict export error: {}", e)))?;
    let json = script
        .to_json()
        .map_err(|e| EditorError::CompileError(format!("Serialization error: {}", e)))?;
    crate::editor::atomic_io::atomic_replace(path, json.as_bytes())
        .map_err(EditorError::IoError)?;
    Ok(())
}
