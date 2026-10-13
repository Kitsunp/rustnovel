use std::fs;
use std::path::Path;

use crate::error::{VnError, VnResult};
use crate::script::ScriptRaw;

use super::{
    source_looks_like_authoring_document, AuthoringDocument, AuthoringDocumentError, NodeGraph,
};

pub fn load_runtime_script_from_entry(path: impl AsRef<Path>) -> VnResult<ScriptRaw> {
    let path = path.as_ref();
    let source = fs::read_to_string(path).map_err(|err| {
        VnError::invalid_script(format!("read entry '{}': {err}", path.display()))
    })?;
    parse_runtime_script_from_entry(&source)
        .map_err(|err| VnError::invalid_script(format!("load entry '{}': {err}", path.display())))
}

pub fn parse_runtime_script_from_entry(source: &str) -> VnResult<ScriptRaw> {
    if source_looks_like_authoring_document(source) {
        let document = parse_authoring_document(source)?;
        return export_runtime_script_from_authoring(&document.graph);
    }
    ScriptRaw::from_json(source)
}

pub fn load_authoring_document_or_script(path: impl AsRef<Path>) -> VnResult<NodeGraph> {
    Ok(load_authoring_entry(path)?.graph)
}

pub fn parse_authoring_document_or_script(source: &str) -> VnResult<NodeGraph> {
    Ok(parse_authoring_entry(source)?.graph)
}

pub fn export_runtime_script_from_authoring(graph: &NodeGraph) -> VnResult<ScriptRaw> {
    graph.to_script_strict()
}

fn parse_authoring_document(source: &str) -> VnResult<AuthoringDocument> {
    AuthoringDocument::from_json(source).map_err(authoring_document_error)
}

fn authoring_document_error(err: AuthoringDocumentError) -> VnError {
    VnError::invalid_script(format!("invalid authoring document: {err}"))
}

/// Parses an editable entry while preserving layers and audit evidence.
/// Authoring markers prevent malformed documents from falling back to runtime input.
pub fn parse_authoring_entry(source: &str) -> VnResult<AuthoringDocument> {
    if source_looks_like_authoring_document(source) {
        return parse_authoring_document(source).map_err(|error| VnError::invalid_script(format!("parse authoring document (legacy fallback disabled because authoring markers were found): {error}")));
    }
    if let Ok(document) = AuthoringDocument::from_json(source) {
        return Ok(document);
    }
    let script = ScriptRaw::from_json(source)?;
    Ok(AuthoringDocument::new(NodeGraph::from_script(&script)))
}

pub fn load_authoring_entry(path: impl AsRef<Path>) -> VnResult<AuthoringDocument> {
    let path = path.as_ref();
    let source = fs::read_to_string(path).map_err(|error| {
        VnError::invalid_script(format!("read entry '{}': {error}", path.display()))
    })?;
    parse_authoring_entry(&source).map_err(|error| {
        VnError::invalid_script(format!("load entry '{}': {error}", path.display()))
    })
}

/// Publishes a complete document without exposing a partially written destination.
pub fn save_authoring_document(
    path: impl AsRef<Path>,
    document: &AuthoringDocument,
) -> VnResult<()> {
    let json = document.to_json().map_err(|error| {
        VnError::invalid_script(format!("serialize authoring document: {error}"))
    })?;
    let path = path.as_ref();
    crate::atomic_io::atomic_replace(path, json.as_bytes()).map_err(|error| {
        VnError::invalid_script(format!("save document '{}': {error}", path.display()))
    })
}
