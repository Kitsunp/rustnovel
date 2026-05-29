//! Shared schema policy for script JSON entry points.
//!
//! The core remains strict by default, while callers that are explicitly
//! migrating or opening legacy scripts can opt into the softer policies.

use crate::error::{VnError, VnResult};
use crate::version::SCRIPT_SCHEMA_VERSION;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SchemaPolicy {
    StrictCurrent,
    LegacyReadOnly,
    Migrating,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SchemaValidationReport {
    pub policy: SchemaPolicy,
    pub expected_version: String,
    pub found_version: Option<String>,
    pub accepted: bool,
    pub normalized_version: String,
    pub warnings: Vec<String>,
}

impl SchemaPolicy {
    pub fn as_str(self) -> &'static str {
        match self {
            SchemaPolicy::StrictCurrent => "strict_current",
            SchemaPolicy::LegacyReadOnly => "legacy_read_only",
            SchemaPolicy::Migrating => "migrating",
        }
    }
}

pub fn validate_script_schema(
    found_version: Option<&str>,
    policy: SchemaPolicy,
) -> VnResult<SchemaValidationReport> {
    let expected = SCRIPT_SCHEMA_VERSION.to_string();
    let mut warnings = Vec::new();
    let normalized_version = match found_version {
        Some(found) if found == SCRIPT_SCHEMA_VERSION => found.to_string(),
        Some(found) if accepts_legacy_version(found, policy) => {
            warnings.push(format!(
                "legacy script_schema_version '{found}' accepted under {}",
                policy.as_str()
            ));
            found.to_string()
        }
        Some(found) => {
            return Err(VnError::InvalidScript(format!(
                "schema incompatible: found {found}, expected {SCRIPT_SCHEMA_VERSION} under {}",
                policy.as_str()
            )));
        }
        None if matches!(
            policy,
            SchemaPolicy::LegacyReadOnly | SchemaPolicy::Migrating
        ) =>
        {
            warnings.push(format!(
                "missing script_schema_version accepted under {}",
                policy.as_str()
            ));
            SCRIPT_SCHEMA_VERSION.to_string()
        }
        None => {
            return Err(VnError::InvalidScript(format!(
                "missing script_schema_version under {}",
                policy.as_str()
            )));
        }
    };

    Ok(SchemaValidationReport {
        policy,
        expected_version: expected,
        found_version: found_version.map(ToString::to_string),
        accepted: true,
        normalized_version,
        warnings,
    })
}

pub fn validate_script_schema_value(
    raw_version: Option<&serde_json::Value>,
    policy: SchemaPolicy,
) -> VnResult<SchemaValidationReport> {
    let found = match raw_version {
        Some(value) => Some(value.as_str().ok_or_else(|| {
            VnError::InvalidScript("script_schema_version must be a string".to_string())
        })?),
        None => None,
    };
    validate_script_schema(found, policy)
}

fn accepts_legacy_version(found: &str, policy: SchemaPolicy) -> bool {
    if matches!(policy, SchemaPolicy::StrictCurrent) {
        return false;
    }
    let Some(found_major) = major_version(found) else {
        return false;
    };
    let Some(expected_major) = major_version(SCRIPT_SCHEMA_VERSION) else {
        return false;
    };
    found_major <= expected_major
}

fn major_version(version: &str) -> Option<u64> {
    version.split_once('.')?.0.parse().ok()
}
