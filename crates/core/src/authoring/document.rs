use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::{
    composer::{BackgroundFit, LayerOverride},
    NodeGraph, OperationLogEntry, VerificationRun,
};

pub const AUTHORING_DOCUMENT_SCHEMA_VERSION: &str = "1.1";
pub const AUTHORING_DOCUMENT_LEGACY_SCHEMA_VERSION: &str = "1.0";

#[derive(Debug, Error)]
pub enum AuthoringDocumentError {
    #[error("invalid authoring document json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("authoring document is missing authoring_schema_version")]
    MissingSchemaVersion,
    #[error("unsupported authoring_schema_version '{found}', expected '{expected}'")]
    UnsupportedSchemaVersion {
        found: String,
        expected: &'static str,
    },
    #[error("authoring document is missing graph")]
    MissingGraph,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AuthoringDocument {
    pub authoring_schema_version: String,
    pub graph: NodeGraph,
    #[serde(default)]
    pub composer_layer_overrides: BTreeMap<String, LayerOverride>,
    #[serde(default)]
    pub composer_background_fit_overrides: BTreeMap<String, BackgroundFit>,
    #[serde(default)]
    pub operation_log: Vec<OperationLogEntry>,
    #[serde(default)]
    pub verification_runs: Vec<VerificationRun>,
}

#[derive(Deserialize)]
struct AuthoringDocumentEnvelope {
    #[serde(default)]
    authoring_schema_version: Option<String>,
    #[serde(default)]
    graph: Option<NodeGraph>,
    #[serde(default)]
    composer_layer_overrides: BTreeMap<String, LayerOverride>,
    #[serde(default)]
    composer_background_fit_overrides: BTreeMap<String, BackgroundFit>,
    #[serde(default)]
    operation_log: Vec<OperationLogEntry>,
    #[serde(default)]
    verification_runs: Vec<VerificationRun>,
}

impl AuthoringDocument {
    pub fn new(graph: NodeGraph) -> Self {
        Self {
            authoring_schema_version: AUTHORING_DOCUMENT_SCHEMA_VERSION.to_string(),
            graph,
            composer_layer_overrides: BTreeMap::new(),
            composer_background_fit_overrides: BTreeMap::new(),
            operation_log: Vec::new(),
            verification_runs: Vec::new(),
        }
    }

    pub fn from_json(source: &str) -> Result<Self, AuthoringDocumentError> {
        let envelope: AuthoringDocumentEnvelope = serde_json::from_str(source)?;
        let version = envelope
            .authoring_schema_version
            .ok_or(AuthoringDocumentError::MissingSchemaVersion)?;
        if version != AUTHORING_DOCUMENT_SCHEMA_VERSION
            && version != AUTHORING_DOCUMENT_LEGACY_SCHEMA_VERSION
        {
            return Err(AuthoringDocumentError::UnsupportedSchemaVersion {
                found: version,
                expected: AUTHORING_DOCUMENT_SCHEMA_VERSION,
            });
        }
        let graph = envelope.graph.ok_or(AuthoringDocumentError::MissingGraph)?;
        Ok(Self {
            authoring_schema_version: AUTHORING_DOCUMENT_SCHEMA_VERSION.to_string(),
            graph,
            composer_layer_overrides: envelope.composer_layer_overrides,
            composer_background_fit_overrides: envelope.composer_background_fit_overrides,
            operation_log: envelope.operation_log,
            verification_runs: envelope.verification_runs,
        })
    }

    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }
}

pub fn source_looks_like_authoring_document(source: &str) -> bool {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(source) else {
        return source_contains_authoring_markers(source);
    };
    value.as_object().is_some_and(|object| {
        object.contains_key("authoring_schema_version")
            || object.contains_key("authoring_document_version")
            || object.contains_key("authoring")
            || object.contains_key("editor")
            || object.contains_key("editor_metadata")
            || object.contains_key("graph")
            || object.contains_key("composer_layer_overrides")
            || object.contains_key("composer_background_fit_overrides")
            || object.contains_key("operation_log")
            || object.contains_key("verification_runs")
            || object.contains_key("nodes")
            || object.contains_key("connections")
            || object.get("graph").is_some_and(|graph| {
                graph.get("nodes").is_some() || graph.get("connections").is_some()
            })
    })
}

fn source_contains_authoring_markers(source: &str) -> bool {
    [
        "authoring_schema_version",
        "authoring_document_version",
        "editor_metadata",
        "\"editor\"",
        "\"graph\"",
        "\"nodes\"",
        "\"connections\"",
        "composer_layer_overrides",
        "composer_background_fit_overrides",
        "operation_log",
        "verification_runs",
    ]
    .iter()
    .any(|marker| source.contains(marker))
}
