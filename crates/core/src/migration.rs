use crate::error::{VnError, VnResult};
use crate::version::SCRIPT_SCHEMA_VERSION;
use serde_json::{Map, Value};

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

#[derive(Clone, Copy)]
struct ScriptMigrationStep {
    from_version: &'static str,
    to_version: &'static str,
    step_id: &'static str,
    apply: fn(&mut Value) -> Result<bool, String>,
}

const LEGACY_MAJOR: &str = "0.x";
const MIGRATION_GUARD_LIMIT: usize = 8;
const SCRIPT_MIGRATION_STEPS: &[ScriptMigrationStep] = &[ScriptMigrationStep {
    from_version: LEGACY_MAJOR,
    to_version: SCRIPT_SCHEMA_VERSION,
    step_id: "script_legacy_to_1_0",
    apply: migrate_legacy_to_1_0,
}];

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
    let from_version = detect_script_version(input)?;
    if from_version == SCRIPT_SCHEMA_VERSION {
        return Ok(MigrationReport {
            from_version: from_version.clone(),
            to_version: from_version,
            entries: Vec::new(),
        });
    }
    if !from_version.starts_with("0.") {
        return Ok(MigrationReport {
            from_version: from_version.clone(),
            to_version: from_version,
            entries: Vec::new(),
        });
    }

    let mut current_version = from_version.clone();
    let mut entries = Vec::new();
    let mut guard = 0usize;

    while current_version != SCRIPT_SCHEMA_VERSION {
        guard += 1;
        if guard > MIGRATION_GUARD_LIMIT {
            return Err(MigrationError::UnsupportedVersion(current_version));
        }

        let step = select_step_for(&current_version)
            .ok_or_else(|| MigrationError::UnsupportedVersion(current_version.clone()))?;
        let changed = (step.apply)(input).map_err(|message| MigrationError::StepFailed {
            step_id: step.step_id.to_string(),
            from_version: current_version.clone(),
            to_version: step.to_version.to_string(),
            message,
        })?;

        let root = expect_object_mut(input)?;
        root.insert(
            "script_schema_version".to_string(),
            Value::String(step.to_version.to_string()),
        );

        entries.push(MigrationTraceEntry {
            step_id: step.step_id.to_string(),
            from_version: current_version.clone(),
            to_version: step.to_version.to_string(),
            changed,
        });
        current_version = step.to_version.to_string();
    }

    Ok(MigrationReport {
        from_version,
        to_version: current_version,
        entries,
    })
}

fn select_step_for(version: &str) -> Option<&'static ScriptMigrationStep> {
    SCRIPT_MIGRATION_STEPS
        .iter()
        .find(|step| step.from_version == version)
        .or_else(|| {
            if version.starts_with("0.") {
                SCRIPT_MIGRATION_STEPS
                    .iter()
                    .find(|step| step.from_version == LEGACY_MAJOR)
            } else {
                None
            }
        })
}

fn detect_script_version(input: &Value) -> Result<String, MigrationError> {
    let root = input.as_object().ok_or_else(|| {
        MigrationError::InvalidEnvelope("script payload must be a JSON object".to_string())
    })?;
    let Some(raw_version) = root.get("script_schema_version") else {
        return Ok("0.9".to_string());
    };
    let version = raw_version.as_str().ok_or_else(|| {
        MigrationError::InvalidEnvelope("script_schema_version must be a string".to_string())
    })?;
    Ok(version.to_string())
}

fn expect_object_mut(input: &mut Value) -> Result<&mut Map<String, Value>, MigrationError> {
    input.as_object_mut().ok_or_else(|| {
        MigrationError::InvalidEnvelope("script payload must be a JSON object".to_string())
    })
}

fn migrate_legacy_to_1_0(input: &mut Value) -> Result<bool, String> {
    let root = input
        .as_object_mut()
        .ok_or_else(|| "script payload must be a JSON object".to_string())?;
    let mut changed = false;

    if !root.contains_key("events") {
        root.insert("events".to_string(), Value::Array(Vec::new()));
        changed = true;
    }
    if !root.contains_key("labels") {
        let mut labels = Map::new();
        labels.insert("start".to_string(), Value::Number(0usize.into()));
        root.insert("labels".to_string(), Value::Object(labels));
        changed = true;
    }

    let events = root
        .get_mut("events")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| "events must be an array".to_string())?;
    for event in events {
        let Some(event_map) = event.as_object_mut() else {
            return Err("every event must be an object".to_string());
        };
        let Some(event_type) = event_map
            .get("type")
            .and_then(Value::as_str)
            .map(str::to_string)
        else {
            return Err("event object is missing string field 'type'".to_string());
        };
        let normalized = normalize_legacy_event_type(&event_type);
        if normalized != event_type {
            event_map.insert("type".to_string(), Value::String(normalized.to_string()));
            changed = true;
        }
        if normalized == "ext_call" && !event_map.contains_key("args") {
            event_map.insert("args".to_string(), Value::Array(Vec::new()));
            changed = true;
        }
    }
    Ok(changed)
}

fn normalize_legacy_event_type(input: &str) -> &str {
    match input {
        "extcall" => "ext_call",
        "audio" => "audio_action",
        "set_character_pos" => "set_character_position",
        other => other,
    }
}
