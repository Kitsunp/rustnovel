use std::fs;
use std::io::{BufReader, Read};
use std::path::Path;

use crate::error::{VnError, VnResult};
use crate::runtime::Engine;
use crate::script::{RuntimeScriptInput, ScriptCompiled, ScriptRaw};
use crate::version::SCRIPT_BINARY_MAGIC;
use crate::{ResourceLimiter, SecurityPolicy};

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
    parse_runtime_projection(source).map(|(script, _)| script)
}

/// Captures a bundle entry once, retaining compilation already required by strict authoring export.
pub(crate) fn load_runtime_projection_for_bundle(
    path: &Path,
) -> VnResult<(ScriptRaw, Option<ScriptCompiled>)> {
    let source = fs::read_to_string(path).map_err(|error| {
        VnError::invalid_script(format!("read entry '{}': {error}", path.display()))
    })?;
    parse_runtime_projection(&source).map_err(|error| {
        VnError::invalid_script(format!("load entry '{}': {error}", path.display()))
    })
}

fn parse_runtime_projection(source: &str) -> VnResult<(ScriptRaw, Option<ScriptCompiled>)> {
    if let Some(document) = parse_entry_document(source)? {
        let (script, compiled) = super::script_sync::strict_projection(&document.graph)?;
        return Ok((script, Some(compiled)));
    }
    ScriptRaw::from_json(source).map(|script| (script, None))
}

/// Loads runtime JSON, a compiled binary, or a complete authoring document.
pub fn load_runtime_input_from_entry(path: impl AsRef<Path>) -> VnResult<RuntimeScriptInput> {
    load_prepared_runtime_entry(path).map(PreparedRuntimeEntry::into_legacy_input)
}

/// Loads a runtime entry, retaining compilation already performed on authoring input.
pub fn load_prepared_runtime_entry(path: impl AsRef<Path>) -> VnResult<PreparedRuntimeEntry> {
    let path = path.as_ref();
    let read_error =
        |error| VnError::invalid_script(format!("read entry '{}': {error}", path.display()));
    let reader = BufReader::new(fs::File::open(path).map_err(read_error)?);
    read_prepared_runtime_entry(reader).map_err(|error| match error {
        VnError::InvalidScript(message) => {
            VnError::invalid_script(format!("load entry '{}': {message}", path.display()))
        }
        error => error,
    })
}

/// Reads an entry without reopening or seeking, including fragmented streams.
pub fn read_runtime_input_from_entry(reader: impl Read) -> VnResult<RuntimeScriptInput> {
    read_prepared_runtime_entry(reader).map(PreparedRuntimeEntry::into_legacy_input)
}

/// Reads an entry without reopening or seeking. Authoring audit history has
/// independent size semantics, while compiled input uses the runtime byte limit.
pub fn read_prepared_runtime_entry(mut reader: impl Read) -> VnResult<PreparedRuntimeEntry> {
    let read_error = |error| VnError::invalid_script(format!("read entry input: {error}"));
    let mut prefix = Vec::with_capacity(SCRIPT_BINARY_MAGIC.len());
    reader
        .by_ref()
        .take(SCRIPT_BINARY_MAGIC.len() as u64)
        .read_to_end(&mut prefix)
        .map_err(read_error)?;
    let compiled = prefix.as_slice() == SCRIPT_BINARY_MAGIC;
    let mut reader = prefix.as_slice().chain(reader);
    if compiled {
        return RuntimeScriptInput::from_reader(reader, ResourceLimiter::default())
            .map(PreparedEntryInput::Runtime)
            .map(PreparedRuntimeEntry);
    }
    let mut source = String::new();
    reader.read_to_string(&mut source).map_err(read_error)?;
    let (script, compiled) = parse_runtime_projection(&source)?;
    Ok(match compiled {
        Some(compiled) => PreparedRuntimeEntry::from_projection(script, compiled),
        None => PreparedRuntimeEntry(PreparedEntryInput::Runtime(RuntimeScriptInput::Json(
            script,
        ))),
    })
}

/// An entry ready for the runtime policy and limits selected by its consumer.
#[derive(Debug)]
pub struct PreparedRuntimeEntry(PreparedEntryInput);

#[derive(Debug)]
enum PreparedEntryInput {
    Runtime(RuntimeScriptInput),
    Authoring {
        raw: ScriptRaw,
        compiled: ScriptCompiled,
    },
}

impl PreparedRuntimeEntry {
    pub(super) fn from_projection(raw: ScriptRaw, compiled: ScriptCompiled) -> Self {
        Self(PreparedEntryInput::Authoring { raw, compiled })
    }

    pub fn into_compiled(
        self,
        policy: &SecurityPolicy,
        limits: ResourceLimiter,
    ) -> VnResult<ScriptCompiled> {
        self.validate_projection(policy, limits)?
            .into_compiled(policy, limits)
    }

    pub fn into_engine(self, policy: SecurityPolicy, limits: ResourceLimiter) -> VnResult<Engine> {
        self.validate_projection(&policy, limits)?
            .into_engine(policy, limits)
    }

    fn validate_projection(
        self,
        policy: &SecurityPolicy,
        limits: ResourceLimiter,
    ) -> VnResult<RuntimeScriptInput> {
        match self.0 {
            PreparedEntryInput::Runtime(input) => Ok(input),
            PreparedEntryInput::Authoring { raw, compiled } => {
                // Some raw fields, such as flag names, become IDs after compilation.
                policy.validate_raw(&raw, limits)?;
                Ok(RuntimeScriptInput::Compiled(compiled))
            }
        }
    }

    fn into_legacy_input(self) -> RuntimeScriptInput {
        match self.0 {
            PreparedEntryInput::Runtime(input) => input,
            PreparedEntryInput::Authoring { raw, .. } => RuntimeScriptInput::Json(raw),
        }
    }
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

fn parse_entry_document(source: &str) -> VnResult<Option<AuthoringDocument>> {
    // Legacy positional envelopes are editable documents; runtime JSON requires an object.
    if source.trim_start().starts_with('[') || source_looks_like_authoring_document(source) {
        return AuthoringDocument::from_json(source)
            .map(Some)
            .map_err(authoring_document_error);
    }
    Ok(None)
}

fn authoring_document_error(err: AuthoringDocumentError) -> VnError {
    VnError::invalid_script(format!("invalid authoring document: {err}"))
}

/// Parses an editable entry while preserving layers and audit evidence.
/// Authoring markers prevent malformed documents from falling back to runtime input.
pub fn parse_authoring_entry(source: &str) -> VnResult<AuthoringDocument> {
    parse_authoring_entry_with_format(source).map(|(document, _)| document)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthoringEntryFormat {
    Document,
    RuntimeScript,
}

/// Parses an editable entry and reports whether it already carried authoring layout.
pub fn parse_authoring_entry_with_format(
    source: &str,
) -> VnResult<(AuthoringDocument, AuthoringEntryFormat)> {
    let document = parse_entry_document(source).map_err(|error| {
        VnError::invalid_script(format!(
            "parse authoring document (legacy fallback disabled for authoring input): {error}"
        ))
    })?;
    if let Some(document) = document {
        return Ok((document, AuthoringEntryFormat::Document));
    }
    let script = ScriptRaw::from_json(source)?;
    Ok((
        AuthoringDocument::new(NodeGraph::from_script(&script)),
        AuthoringEntryFormat::RuntimeScript,
    ))
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
    write_authoring_document(path.as_ref(), document).map(drop)
}

/// Publishes a complete document and returns the SHA-256 of the bytes written.
pub fn save_authoring_document_with_version(
    path: impl AsRef<Path>,
    document: &AuthoringDocument,
) -> VnResult<[u8; 32]> {
    use sha2::{Digest, Sha256};

    let json = write_authoring_document(path.as_ref(), document)?;
    Ok(Sha256::digest(json.as_bytes()).into())
}

fn write_authoring_document(path: &Path, document: &AuthoringDocument) -> VnResult<String> {
    let json = document.to_json().map_err(|error| {
        VnError::invalid_script(format!("serialize authoring document: {error}"))
    })?;
    crate::atomic_io::atomic_replace(path, json.as_bytes()).map_err(|error| {
        VnError::invalid_script(format!("save document '{}': {error}", path.display()))
    })?;
    Ok(json)
}
