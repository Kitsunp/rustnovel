use crate::error::{VnError, VnResult};
use crate::schema_policy::{validate_script_schema_value, SchemaPolicy};
use crate::script::ScriptRaw;
use crate::version::SCRIPT_SCHEMA_VERSION;
use serde_json::Value;

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MigrationTraceEntry {
    pub step_id: String,
    pub from_version: String,
    pub to_version: String,
    pub changed: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MigrationReport {
    pub from_version: String,
    pub to_version: String,
    pub entries: Vec<MigrationTraceEntry>,
}

impl MigrationReport {
    pub fn changed(&self) -> bool {
        self.entries.iter().any(|entry| entry.changed)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MigrationError {
    InvalidEnvelope(String),
    UnsupportedVersion(String),
    StepFailed {
        step_id: String,
        from_version: String,
        to_version: String,
        message: String,
    },
}

impl std::fmt::Display for MigrationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MigrationError::InvalidEnvelope(message) => {
                write!(f, "invalid migration envelope: {message}")
            }
            MigrationError::UnsupportedVersion(version) => {
                write!(f, "unsupported script schema version '{version}'")
            }
            MigrationError::StepFailed {
                step_id,
                from_version,
                to_version,
                message,
            } => write!(
                f,
                "migration step '{step_id}' failed ({from_version} -> {to_version}): {message}"
            ),
        }
    }
}

impl std::error::Error for MigrationError {}

pub fn migrate_script_json_value(input: &mut Value) -> Result<MigrationReport, MigrationError> {
    let original = input.clone();
    match migrate_script_json_value_inner(input) {
        Ok(report) => Ok(report),
        Err(err) => {
            *input = original;
            Err(err)
        }
    }
}

pub fn migrate_script_json_to_current(input: &str) -> VnResult<(String, MigrationReport)> {
    let mut value: Value = serde_json::from_str(input).map_err(|err| VnError::Serialization {
        message: err.to_string(),
        src: input.to_string(),
        span: (0, 0).into(),
    })?;
    let report = migrate_script_json_value(&mut value)
        .map_err(|err| VnError::InvalidScript(err.to_string()))?;
    let output = serde_json::to_string_pretty(&value).map_err(|err| VnError::Serialization {
        message: err.to_string(),
        src: input.to_string(),
        span: (0, 0).into(),
    })?;
    Ok((output, report))
}

fn migrate_script_json_value_inner(input: &mut Value) -> Result<MigrationReport, MigrationError> {
    let root = input.as_object_mut().ok_or_else(|| {
        MigrationError::InvalidEnvelope("script payload must be a JSON object".to_string())
    })?;
    let schema =
        validate_script_schema_value(root.get("script_schema_version"), SchemaPolicy::Migrating)
            .map_err(|err| MigrationError::InvalidEnvelope(err.to_string()))?;
    let from_version = schema
        .found_version
        .clone()
        .unwrap_or_else(|| "missing".to_string());
    if schema.found_version.as_deref() == Some(SCRIPT_SCHEMA_VERSION) {
        return Ok(MigrationReport {
            from_version: from_version.clone(),
            to_version: from_version,
            entries: Vec::new(),
        });
    }
    root.insert(
        "script_schema_version".to_string(),
        Value::String(SCRIPT_SCHEMA_VERSION.to_string()),
    );
    let migrated = serde_json::to_string(input).map_err(|err| MigrationError::StepFailed {
        step_id: "schema_policy_migrate_to_current".to_string(),
        from_version: from_version.clone(),
        to_version: SCRIPT_SCHEMA_VERSION.to_string(),
        message: err.to_string(),
    })?;
    ScriptRaw::from_json(&migrated).map_err(|err| MigrationError::StepFailed {
        step_id: "schema_policy_migrate_to_current".to_string(),
        from_version: from_version.clone(),
        to_version: SCRIPT_SCHEMA_VERSION.to_string(),
        message: err.to_string(),
    })?;
    Ok(MigrationReport {
        from_version: from_version.clone(),
        to_version: SCRIPT_SCHEMA_VERSION.to_string(),
        entries: vec![MigrationTraceEntry {
            step_id: "schema_policy_migrate_to_current".to_string(),
            from_version,
            to_version: SCRIPT_SCHEMA_VERSION.to_string(),
            changed: true,
        }],
    })
}
