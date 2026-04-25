use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use schemars::JsonSchema;

use crate::error::{VnError, VnResult};
use crate::event::{
    CharacterPatchCompiled, CharacterPlacementCompiled, ChoiceCompiled, ChoiceOptionCompiled,
    CondCompiled, CondRaw, DialogueCompiled, EventCompiled, EventRaw, ScenePatchCompiled,
    SceneUpdateCompiled, SharedStr,
};
use crate::migration::migrate_script_json_value;
use crate::resource::ResourceLimiter;
use crate::version::SCRIPT_SCHEMA_VERSION;

use super::compiled::ScriptCompiled;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, JsonSchema)]
struct ScriptEnvelope {
    #[serde(default)]
    script_schema_version: Option<String>,
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

    /// Serializes the script to a JSON string with the current schema version.
    pub fn to_json(&self) -> VnResult<String> {
        let envelope = ScriptEnvelope {
            script_schema_version: Some(SCRIPT_SCHEMA_VERSION.to_string()),
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
        if input.len() > limits.max_script_bytes {
            return Err(VnError::ResourceLimit(
                "script json input budget".to_string(),
            ));
        }
        let mut payload: serde_json::Value =
            serde_json::from_str(input).map_err(|err| json_deserialize_error(input, &err))?;
        migrate_script_json_value(&mut payload)
            .map_err(|err| VnError::InvalidScript(format!("script migration failed: {err}")))?;

        let migrated_input =
            serde_json::to_string_pretty(&payload).map_err(|err| VnError::Serialization {
                message: err.to_string(),
                src: input.to_string(),
                span: (0, 0).into(),
            })?;
        let envelope: ScriptEnvelope = serde_json::from_value(payload)
            .map_err(|err| json_deserialize_error(&migrated_input, &err))?;
        match envelope.script_schema_version.as_deref() {
            Some(version) if is_compatible_schema(version) => {
                let script = Self {
                    events: envelope.events,
                    labels: envelope.labels,
                };
                script.ensure_string_budget(limits.max_script_bytes)?;
                Ok(script)
            }
            Some(version) => Err(VnError::InvalidScript(format!(
                "schema incompatible: found {version}, expected {SCRIPT_SCHEMA_VERSION}"
            ))),
            None => {
                // Allow legacy scripts without version
                let script = Self {
                    events: envelope.events,
                    labels: envelope.labels,
                };
                script.ensure_string_budget(limits.max_script_bytes)?;
                Ok(script)
            }
        }
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
        let mut pool = StringPool::default();
        let mut compiled_events = Vec::with_capacity(self.events.len());
        let mut compiled_labels = BTreeMap::new();
        let mut flag_map: HashMap<String, u32> = HashMap::new();
        let mut var_map: HashMap<String, u32> = HashMap::new();

        for (label, index) in &self.labels {
            if *index > self.events.len() {
                return Err(VnError::InvalidScript(format!(
                    "label '{label}' points outside events"
                )));
            }
            let ip = u32::try_from(*index)
                .map_err(|_| VnError::InvalidScript(format!("label '{label}' out of range")))?;
            compiled_labels.insert(label.clone(), ip);
        }

        let start_ip = compiled_labels
            .get("start")
            .copied()
            .ok_or_else(|| VnError::InvalidScript("missing 'start' label".to_string()))?;

        for event in &self.events {
            let compiled = match event {
                EventRaw::Dialogue(dialogue) => EventCompiled::Dialogue(DialogueCompiled {
                    speaker: pool.intern(&dialogue.speaker),
                    text: pool.intern(&dialogue.text),
                }),
                EventRaw::Choice(choice) => EventCompiled::Choice(ChoiceCompiled {
                    prompt: pool.intern(&choice.prompt),
                    options: choice
                        .options
                        .iter()
                        .map(|option| {
                            let target_ip = compiled_labels
                                .get(&option.target)
                                .copied()
                                .ok_or_else(|| {
                                    VnError::InvalidScript(format!(
                                        "choice target '{}' not found",
                                        option.target
                                    ))
                                })?;
                            Ok(ChoiceOptionCompiled {
                                text: pool.intern(&option.text),
                                target_ip,
                            })
                        })
                        .collect::<VnResult<Vec<_>>>()?,
                }),
                EventRaw::Scene(scene) => EventCompiled::Scene(SceneUpdateCompiled {
                    background: scene.background.as_deref().map(|value| pool.intern(value)),
                    music: scene.music.as_deref().map(|value| pool.intern(value)),
                    characters: scene
                        .characters
                        .iter()
                        .map(|character| CharacterPlacementCompiled {
                            name: pool.intern(&character.name),
                            expression: character
                                .expression
                                .as_deref()
                                .map(|value| pool.intern(value)),
                            position: character
                                .position
                                .as_deref()
                                .map(|value| pool.intern(value)),
                            x: character.x,
                            y: character.y,
                            scale: character.scale,
                        })
                        .collect(),
                }),
                EventRaw::Jump { target } => {
                    let target_ip = compiled_labels.get(target).copied().ok_or_else(|| {
                        VnError::InvalidScript(format!("jump target '{target}' not found"))
                    })?;
                    EventCompiled::Jump { target_ip }
                }
                EventRaw::SetFlag { key, value } => {
                    let flag_id = get_or_insert_id(&mut flag_map, key)?;
                    EventCompiled::SetFlag {
                        flag_id,
                        value: *value,
                    }
                }
                EventRaw::SetVar { key, value } => {
                    let var_id = get_or_insert_id(&mut var_map, key)?;
                    EventCompiled::SetVar {
                        var_id,
                        value: *value,
                    }
                }
                EventRaw::JumpIf { cond, target } => {
                    let target_ip = compiled_labels.get(target).copied().ok_or_else(|| {
                        VnError::InvalidScript(format!("jump_if target '{target}' not found"))
                    })?;
                    let cond = compile_cond(cond, &mut flag_map, &mut var_map)?;
                    EventCompiled::JumpIf { cond, target_ip }
                }
                EventRaw::Patch(patch) => EventCompiled::Patch(ScenePatchCompiled {
                    background: patch.background.as_deref().map(|value| pool.intern(value)),
                    music: patch.music.as_deref().map(|value| pool.intern(value)),
                    add: patch
                        .add
                        .iter()
                        .map(|character| CharacterPlacementCompiled {
                            name: pool.intern(&character.name),
                            expression: character
                                .expression
                                .as_deref()
                                .map(|value| pool.intern(value)),
                            position: character
                                .position
                                .as_deref()
                                .map(|value| pool.intern(value)),
                            x: character.x,
                            y: character.y,
                            scale: character.scale,
                        })
                        .collect(),
                    update: patch
                        .update
                        .iter()
                        .map(|character| CharacterPatchCompiled {
                            name: pool.intern(&character.name),
                            expression: character
                                .expression
                                .as_deref()
                                .map(|value| pool.intern(value)),
                            position: character
                                .position
                                .as_deref()
                                .map(|value| pool.intern(value)),
                        })
                        .collect(),
                    remove: patch.remove.iter().map(|name| pool.intern(name)).collect(),
                }),
                EventRaw::ExtCall { command, args } => EventCompiled::ExtCall {
                    command: command.clone(),
                    args: args.clone(),
                },
                EventRaw::AudioAction(action) => {
                    EventCompiled::AudioAction(crate::event::AudioActionCompiled {
                        channel: compile_audio_channel(&action.channel)?,
                        action: compile_audio_action(&action.action)?,
                        asset: action.asset.as_deref().map(|s| pool.intern(s)),
                        volume: action.volume,
                        fade_duration_ms: action.fade_duration_ms,
                        loop_playback: action.loop_playback,
                    })
                }
                EventRaw::Transition(transition) => {
                    EventCompiled::Transition(crate::event::SceneTransitionCompiled {
                        kind: compile_transition_kind(&transition.kind)?,
                        duration_ms: transition.duration_ms,
                        color: transition.color.as_deref().map(|s| pool.intern(s)),
                    })
                }
                EventRaw::SetCharacterPosition(pos) => EventCompiled::SetCharacterPosition(
                    crate::event::SetCharacterPositionCompiled {
                        name: pool.intern(&pos.name),
                        x: pos.x,
                        y: pos.y,
                        scale: pos.scale,
                    },
                ),
            };
            compiled_events.push(compiled);
        }

        Ok(ScriptCompiled {
            events: compiled_events,
            labels: compiled_labels,
            start_ip,
            flag_count: flag_map.len() as u32,
        })
    }
}

fn is_compatible_schema(version: &str) -> bool {
    if version == SCRIPT_SCHEMA_VERSION {
        return true;
    }

    let Some((major, _)) = version.split_once('.') else {
        return false;
    };
    let Some((current_major, _)) = SCRIPT_SCHEMA_VERSION.split_once('.') else {
        return false;
    };

    match (major.parse::<u32>(), current_major.parse::<u32>()) {
        // Accept legacy major versions when no structural migration is required.
        (Ok(found_major), Ok(active_major)) => found_major <= active_major,
        _ => false,
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
    let mut current_line = 1usize;
    let mut offset = 0usize;
    for chunk in input.split_inclusive('\n') {
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
        current_line += 1;
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

#[derive(Default)]
struct StringPool {
    cache: HashMap<String, SharedStr>,
}

impl StringPool {
    fn intern(&mut self, value: &str) -> SharedStr {
        if let Some(existing) = self.cache.get(value) {
            return existing.clone();
        }
        let shared: SharedStr = Arc::from(value);
        self.cache.insert(value.to_string(), shared.clone());
        shared
    }
}

fn get_or_insert_id(map: &mut HashMap<String, u32>, key: &str) -> VnResult<u32> {
    if let Some(id) = map.get(key) {
        return Ok(*id);
    }
    let next_id =
        u32::try_from(map.len()).map_err(|_| VnError::InvalidScript("too many ids".to_string()))?;
    map.insert(key.to_string(), next_id);
    Ok(next_id)
}

fn compile_cond(
    cond: &CondRaw,
    flag_map: &mut HashMap<String, u32>,
    var_map: &mut HashMap<String, u32>,
) -> VnResult<CondCompiled> {
    match cond {
        CondRaw::Flag { key, is_set } => {
            let flag_id = get_or_insert_id(flag_map, key)?;
            Ok(CondCompiled::Flag {
                flag_id,
                is_set: *is_set,
            })
        }
        CondRaw::VarCmp { key, op, value } => {
            let var_id = get_or_insert_id(var_map, key)?;
            Ok(CondCompiled::VarCmp {
                var_id,
                op: *op,
                value: *value,
            })
        }
    }
}

fn compile_audio_channel(channel: &str) -> VnResult<u8> {
    let normalized = channel.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "bgm" => Ok(0),
        "sfx" => Ok(1),
        "voice" => Ok(2),
        _ => Err(VnError::InvalidScript(format!(
            "invalid audio channel '{channel}' (expected bgm|sfx|voice)"
        ))),
    }
}

fn compile_audio_action(action: &str) -> VnResult<u8> {
    let normalized = action.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "play" => Ok(0),
        "stop" => Ok(1),
        "fade_out" => Ok(2),
        _ => Err(VnError::InvalidScript(format!(
            "invalid audio action '{action}' (expected play|stop|fade_out)"
        ))),
    }
}

fn compile_transition_kind(kind: &str) -> VnResult<u8> {
    let normalized = kind.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "fade" | "fade_black" => Ok(0),
        "dissolve" => Ok(1),
        "cut" => Ok(2),
        _ => Err(VnError::InvalidScript(format!(
            "invalid transition kind '{kind}' (expected fade|fade_black|dissolve|cut)"
        ))),
    }
}

#[cfg(test)]
#[path = "tests/raw_tests.rs"]
mod tests;
