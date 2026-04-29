use serde::{Deserialize, Serialize};

use super::NodeGraph;

pub const AUTHORING_DOCUMENT_SCHEMA_VERSION: &str = "1.0";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AuthoringDocument {
    pub authoring_schema_version: String,
    pub graph: NodeGraph,
}

impl AuthoringDocument {
    pub fn new(graph: NodeGraph) -> Self {
        Self {
            authoring_schema_version: AUTHORING_DOCUMENT_SCHEMA_VERSION.to_string(),
            graph,
        }
    }

    pub fn from_json(source: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(source)
    }

    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }
}
