use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::{composer::LayerOverride, NodeGraph, OperationLogEntry, VerificationRun};

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
        return false;
    };
    value.as_object().is_some_and(|object| {
        object.contains_key("authoring_schema_version") || object.contains_key("graph")
    })
}
