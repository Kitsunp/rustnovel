use std::collections::BTreeMap;

use schemars::JsonSchema;

use crate::error::{VnError, VnResult};
use crate::event::EventRaw;
use crate::event_behavior::{event_behavior_for_raw, CompileCtx, EventBehavior};
use crate::resource::ResourceLimiter;
use crate::schema_policy::{validate_script_schema_value, SchemaPolicy};
use crate::version::SCRIPT_SCHEMA_VERSION;

use super::compiled::ScriptCompiled;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, JsonSchema)]
struct ScriptEnvelope {
    script_schema_version: String,
    events: Vec<EventRaw>,
    labels: BTreeMap<String, usize>,
}

/// JSON-facing script format with label names and raw string data.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, JsonSchema)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub struct ScriptRaw {
    pub events: Vec<EventRaw>,
    pub labels: BTreeMap<String, usize>,
}

impl ScriptRaw {
    /// Creates a raw script from events and labels.
    pub fn new(events: Vec<EventRaw>, labels: BTreeMap<String, usize>) -> Self {
        Self { events, labels }
    }

    /// Parses a JSON script into a raw script structure.
    pub fn from_json(input: &str) -> VnResult<Self> {
        Self::from_json_with_limits(input, ResourceLimiter::default())
    }

    /// Parses a JSON script using an explicit schema compatibility policy.
    pub fn from_json_with_policy(input: &str, policy: SchemaPolicy) -> VnResult<Self> {
        Self::from_json_with_policy_and_limits(input, policy, ResourceLimiter::default())
    }

    /// Serializes the script to a JSON string with the current schema version.
    pub fn to_json(&self) -> VnResult<String> {
        let envelope = ScriptEnvelope {
            script_schema_version: SCRIPT_SCHEMA_VERSION.to_string(),
            events: self.events.clone(),
            labels: self.labels.clone(),
        };
        serde_json::to_string_pretty(&envelope).map_err(|e| VnError::Serialization {
            message: e.to_string(),
            src: "".to_string(),
            span: (0, 0).into(),
        })
    }

    /// Parses a JSON script into a raw script structure with resource limits.
    pub fn from_json_with_limits(input: &str, limits: ResourceLimiter) -> VnResult<Self> {
        Self::from_json_with_policy_and_limits(input, SchemaPolicy::StrictCurrent, limits)
    }

    /// Parses a JSON script into a raw script structure with explicit schema policy and limits.
    pub fn from_json_with_policy_and_limits(
        input: &str,
        policy: SchemaPolicy,
        limits: ResourceLimiter,
    ) -> VnResult<Self> {
        if input.len() > limits.max_script_bytes {
            return Err(VnError::ResourceLimit(
                "script json input budget".to_string(),
            ));
        }
        let mut payload: serde_json::Value =
            serde_json::from_str(input).map_err(|err| json_deserialize_error(input, &err))?;
        let root = payload.as_object_mut().ok_or_else(|| {
            VnError::InvalidScript("script payload must be a JSON object".to_string())
        })?;
        let schema_report =
            validate_script_schema_value(root.get("script_schema_version"), policy)?;
        if !root.contains_key("script_schema_version") {
            root.insert(
                "script_schema_version".to_string(),
                serde_json::Value::String(schema_report.normalized_version),
            );
        }
        let migrated_input =
            serde_json::to_string_pretty(&payload).map_err(|err| VnError::Serialization {
                message: err.to_string(),
                src: input.to_string(),
                span: (0, 0).into(),
            })?;
        let envelope: ScriptEnvelope = serde_json::from_value(payload)
            .map_err(|err| json_deserialize_error(&migrated_input, &err))?;
        let script = Self {
            events: envelope.events,
            labels: envelope.labels,
        };
        script.ensure_string_budget(limits.max_script_bytes)?;
        Ok(script)
    }

    pub fn ensure_string_budget(&self, max_bytes: usize) -> VnResult<()> {
        let mut total = 0usize;
        for label in self.labels.keys() {
            total = total.saturating_add(label.len());
        }
        if total > max_bytes {
            return Err(VnError::ResourceLimit(
                "script string budget (labels)".to_string(),
            ));
        }

        use crate::resource::StringBudget;
        for event in &self.events {
            total = total.saturating_add(event.string_bytes());
            if total > max_bytes {
                return Err(VnError::ResourceLimit("script string budget".to_string()));
            }
        }
        Ok(())
    }

    /// Returns the index of the `start` label.
    pub fn start_index(&self) -> VnResult<usize> {
        self.labels
            .get("start")
            .copied()
            .ok_or_else(|| VnError::InvalidScript("missing 'start' label".to_string()))
    }

    /// Compiles a raw script into its runtime representation.
    ///
    /// Resolves label targets, assigns flag ids, and interns repeated strings.
    pub fn compile(&self) -> VnResult<ScriptCompiled> {
        let _event_len = u32::try_from(self.events.len())
            .map_err(|_| VnError::InvalidScript("event count exceeds u32::MAX".to_string()))?;
        let mut compiled_events = Vec::with_capacity(self.events.len());
        let mut compiled_labels = BTreeMap::new();

        for (label, index) in &self.labels {
            if *index > self.events.len() {
                return Err(VnError::InvalidScript(format!(
                    "label '{label}' points outside events"
                )));
            }
            if label == "start" && *index >= self.events.len() {
                return Err(VnError::InvalidScript(
                    "start label must point to an executable event".to_string(),
                ));
            }
            let ip = u32::try_from(*index)
                .map_err(|_| VnError::InvalidScript(format!("label '{label}' out of range")))?;
            compiled_labels.insert(label.clone(), ip);
        }

        let start_ip = compiled_labels
            .get("start")
            .copied()
            .ok_or_else(|| VnError::InvalidScript("missing 'start' label".to_string()))?;

        let flag_count = {
            let mut compile_ctx = CompileCtx::new(&compiled_labels);
            for event in &self.events {
                let compiled = event_behavior_for_raw(event).compile(&mut compile_ctx, event)?;
                compiled_events.push(compiled);
            }
            compile_ctx.flag_count()
        };

        Ok(ScriptCompiled {
            events: compiled_events,
            labels: compiled_labels,
            start_ip,
            flag_count,
        })
    }
}

#[cold]
#[inline(never)]
fn json_deserialize_error(input: &str, err: &serde_json::Error) -> VnError {
    let (offset, length) = json_error_span(input, err);
    let (window, local_offset) = json_error_window(input, offset, length);
    let max_len = window.len().saturating_sub(local_offset);
    let span_len = if max_len == 0 { 0 } else { length.min(max_len) };
    VnError::Serialization {
        message: err.to_string(),
        src: window,
        span: (local_offset, span_len).into(),
    }
}

#[cold]
#[inline(never)]
fn json_error_span(input: &str, error: &serde_json::Error) -> (usize, usize) {
    let line = error.line();
    let column = error.column();
    if line == 0 || column == 0 {
        return (0, 1);
    }
    let mut offset = 0usize;
    for (current_line, chunk) in (1usize..).zip(input.split_inclusive('\n')) {
        if current_line == line {
            let column_index = column.saturating_sub(1);
            let byte_index = chunk
                .char_indices()
                .nth(column_index)
                .map(|(idx, _)| idx)
                .unwrap_or(chunk.len().saturating_sub(1));
            offset += byte_index;
            return (offset, 1);
        }
        offset += chunk.len();
    }
    (input.len().saturating_sub(1), 1)
}

#[cold]
#[inline(never)]
fn json_error_window(input: &str, offset: usize, length: usize) -> (String, usize) {
    const CONTEXT: usize = 160;
    let mut start = offset.saturating_sub(CONTEXT);
    let mut end = (offset + length + CONTEXT).min(input.len());
    while start > 0 && !input.is_char_boundary(start) {
        start = start.saturating_sub(1);
    }
    while end < input.len() && !input.is_char_boundary(end) {
        end = end.saturating_add(1).min(input.len());
    }
    let window = input[start..end].to_string();
    (window, offset.saturating_sub(start))
}
