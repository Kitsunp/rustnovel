use std::collections::BTreeMap;
use std::fmt;

use serde::de::{DeserializeSeed, Deserializer, MapAccess, SeqAccess, Visitor};
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
    let mut deserializer = serde_json::Deserializer::from_str(source);
    MarkerProbe::Root
        .deserialize(&mut deserializer)
        .and_then(|found| deserializer.end().map(|()| found))
        .unwrap_or_else(|_| source_contains_authoring_markers(source))
}

// Walk values with serde_json's usual syntax, depth and number checks, while
// retaining only whether a top-level authoring key was seen.
#[derive(Clone, Copy, PartialEq, Eq)]
enum MarkerProbe {
    Root,
    Nested,
    Key,
}

impl<'de> DeserializeSeed<'de> for MarkerProbe {
    type Value = bool;

    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<bool, D::Error> {
        deserializer.deserialize_any(self)
    }
}

impl<'de> Visitor<'de> for MarkerProbe {
    type Value = bool;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a JSON value")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<bool, A::Error> {
        let mut found = false;
        while let Some(marker) = map.next_key_seed(Self::Key)? {
            found |= self == Self::Root && marker;
            map.next_value_seed(Self::Nested)?;
        }
        Ok(found)
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<bool, A::Error> {
        while seq.next_element_seed(Self::Nested)?.is_some() {}
        Ok(false)
    }

    fn visit_str<E>(self, value: &str) -> Result<bool, E> {
        Ok(self == Self::Key
            && matches!(
                value,
                "authoring_schema_version"
                    | "authoring_document_version"
                    | "authoring"
                    | "editor"
                    | "editor_metadata"
                    | "graph"
                    | "composer_layer_overrides"
                    | "composer_background_fit_overrides"
                    | "operation_log"
                    | "verification_runs"
                    | "nodes"
                    | "connections"
            ))
    }

    fn visit_bool<E>(self, _: bool) -> Result<bool, E> {
        Ok(false)
    }

    fn visit_i64<E>(self, _: i64) -> Result<bool, E> {
        Ok(false)
    }

    fn visit_u64<E>(self, _: u64) -> Result<bool, E> {
        Ok(false)
    }

    fn visit_f64<E>(self, _: f64) -> Result<bool, E> {
        Ok(false)
    }

    fn visit_unit<E>(self) -> Result<bool, E> {
        Ok(false)
    }
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
