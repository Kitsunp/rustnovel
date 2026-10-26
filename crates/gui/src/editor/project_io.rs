use crate::editor::errors::EditorError;
use crate::editor::node_graph::NodeGraph;
use std::path::{Component, Path, PathBuf};
use visual_novel_engine::{
    authoring::{
        export_runtime_script_from_authoring, source_looks_like_authoring_document,
        AuthoringDocument,
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

    match std::fs::symlink_metadata(&candidate) {
        Ok(_) => {}
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(EditorError::IoError(err)),
    }

    let canonical_candidate = candidate.canonicalize().map_err(EditorError::IoError)?;
    if !canonical_candidate.starts_with(&canonical_root) {
        return Err(EditorError::CompileError(format!(
            "Path escapes project root after canonicalization: {}",
            requested.display()
        )));
    }
    if !canonical_candidate.is_file() {
        return Err(EditorError::CompileError(format!(
            "Path is not a regular file: {}",
            requested.display()
        )));
    }

    Ok(Some(canonical_candidate))
}

pub fn load_project(path: PathBuf) -> Result<LoadedProject, EditorError> {
    let path = std::path::absolute(path).map_err(EditorError::IoError)?;
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
    let source_has_authoring_layout = source_looks_like_authoring_document(&source);
    let document = visual_novel_engine::authoring::parse_authoring_entry(&source)
        .map_err(|error| EditorError::CompileError(error.to_string()))?;
    let mut graph = NodeGraph::from_document(document);
    if !source_has_authoring_layout {
        graph.auto_layout_hierarchical();
        graph.zoom_to_fit();
        graph.authoring_session = visual_novel_engine::authoring::AuthoringDocumentSession::new(
            AuthoringDocument::new(graph.authoring_graph().clone()),
        );
        graph.clear_modified();
    }
    Ok(LoadedScript {
        graph,
        was_imported: false,
    })
}

pub fn save_script(path: &std::path::Path, graph: &NodeGraph) -> Result<(), EditorError> {
    save_authoring_document(path, graph)
}

pub fn save_authoring_document(
    path: &std::path::Path,
    graph: &NodeGraph,
) -> Result<(), EditorError> {
    visual_novel_engine::authoring::save_authoring_document(
        path,
        graph.authoring_session.document(),
    )
    .map_err(|error| EditorError::CompileError(error.to_string()))
}

pub fn export_runtime_script(path: &std::path::Path, graph: &NodeGraph) -> Result<(), EditorError> {
    let script = export_runtime_script_from_authoring(graph.authoring_graph())
        .map_err(|e| EditorError::CompileError(format!("Strict export error: {}", e)))?;
    let json = script
        .to_json()
        .map_err(|e| EditorError::CompileError(format!("Serialization error: {}", e)))?;
    crate::editor::atomic_io::atomic_replace(path, json.as_bytes())
        .map_err(EditorError::IoError)?;
    Ok(())
}
