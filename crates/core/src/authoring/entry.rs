use std::fs;
use std::path::Path;

use crate::{ScriptRaw, VnError, VnResult};

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
    let path = path.as_ref();
    let source = fs::read_to_string(path).map_err(|err| {
        VnError::invalid_script(format!("read entry '{}': {err}", path.display()))
    })?;
    parse_authoring_document_or_script(&source)
        .map_err(|err| VnError::invalid_script(format!("load entry '{}': {err}", path.display())))
}

pub fn parse_authoring_document_or_script(source: &str) -> VnResult<NodeGraph> {
    if source_looks_like_authoring_document(source) {
        return Ok(parse_authoring_document(source)?.graph);
    }
    let script = ScriptRaw::from_json(source)?;
    Ok(NodeGraph::from_script(&script))
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
