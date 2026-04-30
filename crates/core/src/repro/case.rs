use std::collections::BTreeMap;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::error::{VnError, VnResult};
use crate::script::ScriptRaw;

use super::report::ReproOracle;
use super::REPRO_CASE_SCHEMA;

const DEFAULT_REPRO_MAX_STEPS: usize = 2048;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReproCase {
    // Stable schema tag keeps repro artifacts auditable across versions.
    pub schema: String,
    pub title: String,
    pub created_unix_ms: u64,
    pub script: ScriptRaw,
    #[serde(default = "default_max_steps")]
    pub max_steps: usize,
    #[serde(default)]
    pub choice_route: Vec<usize>,
    #[serde(default)]
    pub diagnostic_id: Option<String>,
    #[serde(default)]
    pub semantic_fingerprint_sha256: Option<String>,
    #[serde(default)]
    pub operation_id: Option<String>,
    #[serde(default)]
    pub capabilities: Vec<String>,
    #[serde(default)]
    pub plugins: Vec<String>,
    #[serde(default)]
    pub asset_manifest_sha256: Option<String>,
    #[serde(default)]
    pub seed: Option<u64>,
    #[serde(default)]
    pub validation_profile: Option<String>,
    #[serde(default)]
    pub environment: BTreeMap<String, String>,
    #[serde(default)]
    pub oracle: ReproOracle,
    #[serde(default)]
    pub notes: Option<String>,
}

impl ReproCase {
    pub fn new(title: impl Into<String>, script: ScriptRaw) -> Self {
        Self {
            schema: REPRO_CASE_SCHEMA.to_string(),
            title: title.into(),
            created_unix_ms: now_unix_ms(),
            script,
            max_steps: DEFAULT_REPRO_MAX_STEPS,
            choice_route: Vec::new(),
            diagnostic_id: None,
            semantic_fingerprint_sha256: None,
            operation_id: None,
            capabilities: vec![
                "headless_repro_v1".to_string(),
                "extcall_simulated".to_string(),
            ],
            plugins: Vec::new(),
            asset_manifest_sha256: None,
            seed: Some(0),
            validation_profile: Some("default".to_string()),
            environment: default_environment_snapshot(),
            oracle: ReproOracle::default(),
            notes: None,
        }
    }

    pub fn with_diagnostic_context(
        mut self,
        diagnostic_id: impl Into<String>,
        semantic_fingerprint_sha256: impl Into<String>,
        operation_id: impl Into<String>,
    ) -> Self {
        self.diagnostic_id = Some(diagnostic_id.into());
        self.semantic_fingerprint_sha256 = Some(semantic_fingerprint_sha256.into());
        self.operation_id = Some(operation_id.into());
        self
    }

    pub fn from_json(payload: &str) -> VnResult<Self> {
        let case: Self = serde_json::from_str(payload).map_err(|err| VnError::Serialization {
            message: format!("invalid repro JSON: {err}"),
            src: payload.to_string(),
            span: (0, 0).into(),
        })?;
        if case.schema != REPRO_CASE_SCHEMA {
            return Err(VnError::InvalidScript(format!(
                "unsupported repro schema '{}'",
                case.schema
            )));
        }
        Ok(case)
    }

    pub fn to_json(&self) -> VnResult<String> {
        serde_json::to_string_pretty(self).map_err(|err| VnError::Serialization {
            message: err.to_string(),
            src: "".to_string(),
            span: (0, 0).into(),
        })
    }
}

fn default_max_steps() -> usize {
    DEFAULT_REPRO_MAX_STEPS
}

fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

fn default_environment_snapshot() -> BTreeMap<String, String> {
    // Capture host identity for traceability without depending on absolute paths.
    let mut env = BTreeMap::new();
    env.insert("os".to_string(), std::env::consts::OS.to_string());
    env.insert("arch".to_string(), std::env::consts::ARCH.to_string());
    env.insert("family".to_string(), std::env::consts::FAMILY.to_string());
    env
}
