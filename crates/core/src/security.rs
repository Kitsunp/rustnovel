//! Security policy validation for scripts.

use crate::error::{VnError, VnResult};
use crate::event::{
    audio_action_name, audio_channel_name, transition_kind_name, AudioActionCompiled, CondCompiled,
    EventCompiled, EventRaw,
};
use crate::resource::ResourceLimiter;
use crate::script::{ScriptCompiled, ScriptRaw};

mod content;
use content::validate_event_content;

/// Policy used to validate script content and compiled ranges.
#[derive(Clone, Debug, Default)]
pub struct SecurityPolicy {
    pub allow_empty_speaker: bool,
}

impl SecurityPolicy {
    /// Validates a raw script against policy and resource limits.
    pub fn validate_raw(&self, script: &ScriptRaw, limits: ResourceLimiter) -> VnResult<()> {
        if script.events.len() > limits.max_events {
            return Err(VnError::ResourceLimit("event count".to_string()));
        }

        if !script.labels.contains_key("start") {
            return Err(VnError::InvalidScript("missing 'start' label".to_string()));
        }

        for (label, index) in &script.labels {
            if label.len() > limits.max_label_length {
                return Err(VnError::ResourceLimit(format!("label '{label}' too long")));
            }
            if *index > script.events.len() {
                return Err(VnError::InvalidScript(format!(
                    "label '{label}' points outside events"
                )));
            }
            if label == "start" && *index >= script.events.len() {
                return Err(VnError::InvalidScript(
                    "start label must point to an executable event".to_string(),
                ));
            }
        }

        for event in &script.events {
            validate_event_content!(event, EventRaw, self, limits);
            match event {
                EventRaw::Choice(choice) => {
                    for option in &choice.options {
                        if option.target.len() > limits.max_label_length {
                            return Err(VnError::ResourceLimit("choice target".to_string()));
                        }
                        if !script.labels.contains_key(&option.target) {
                            return Err(VnError::InvalidScript(format!(
                                "choice target '{}' not found",
                                option.target
                            )));
                        }
                    }
                }
                EventRaw::Jump { target } => {
                    if target.len() > limits.max_label_length {
                        return Err(VnError::ResourceLimit("jump target".to_string()));
                    }
                    if !script.labels.contains_key(target) {
                        return Err(VnError::InvalidScript(format!(
                            "jump target '{target}' not found"
                        )));
                    }
                }
                EventRaw::SetFlag { key, .. } => {
                    if key.len() > limits.max_label_length {
                        return Err(VnError::ResourceLimit("flag key".to_string()));
                    }
                }
                EventRaw::SetVar { key, .. } => {
                    if key.len() > limits.max_label_length {
                        return Err(VnError::ResourceLimit("var key".to_string()));
                    }
                }
                EventRaw::JumpIf { target, .. } => {
                    if target.len() > limits.max_label_length {
                        return Err(VnError::ResourceLimit("jump_if target".to_string()));
                    }
                    if !script.labels.contains_key(target) {
                        return Err(VnError::InvalidScript(format!(
                            "jump_if target '{target}' not found"
                        )));
                    }
                }
                EventRaw::Dialogue(_)
                | EventRaw::Scene(_)
                | EventRaw::Patch(_)
                | EventRaw::ExtCall { .. }
                | EventRaw::AudioAction(_)
                | EventRaw::Transition(_)
                | EventRaw::SetCharacterPosition(_) => {}
            }
        }
        Ok(())
    }

    /// Validates compiled targets and flag ids for runtime safety.
    pub fn validate_compiled(
        &self,
        script: &ScriptCompiled,
        limits: ResourceLimiter,
    ) -> VnResult<()> {
        if script.events.len() > limits.max_events {
            return Err(VnError::ResourceLimit("compiled event count".to_string()));
        }
        // Raw compilation introduces at most one flag per event. Bound the
        // declared binary capacity too, before allocating the runtime bitset.
        if u64::from(script.flag_count) > limits.max_events as u64 {
            return Err(VnError::ResourceLimit("compiled flag capacity".to_string()));
        }
        for (label, &index) in &script.labels {
            if label.len() > limits.max_label_length {
                return Err(VnError::ResourceLimit(format!("label '{label}' too long")));
            }
            if index as usize > script.events.len() {
                return Err(VnError::InvalidScript(format!(
                    "compiled label '{label}' outside events"
                )));
            }
        }
        let var_limit = script.events.len();
        if script.start_ip as usize >= script.events.len() {
            return Err(VnError::InvalidScript(
                "compiled start_ip outside events".to_string(),
            ));
        }

        for event in &script.events {
            validate_event_content!(event, EventCompiled, self, limits);
            match event {
                EventCompiled::Choice(choice) => {
                    for option in &choice.options {
                        if option.target_ip as usize > script.events.len() {
                            return Err(VnError::InvalidScript(format!(
                                "choice target_ip {} outside events",
                                option.target_ip
                            )));
                        }
                    }
                }
                EventCompiled::Jump { target_ip } if *target_ip as usize > script.events.len() => {
                    return Err(VnError::InvalidScript(format!(
                        "jump target_ip {} outside events",
                        target_ip
                    )));
                }
                EventCompiled::JumpIf { target_ip, cond } => {
                    if *target_ip as usize > script.events.len() {
                        return Err(VnError::InvalidScript(format!(
                            "jump_if target_ip {} outside events",
                            target_ip
                        )));
                    }
                    match cond {
                        CondCompiled::VarCmp { var_id, .. } if *var_id as usize >= var_limit => {
                            return Err(VnError::InvalidScript(format!(
                                "condition variable id {var_id} outside compiled range"
                            )));
                        }
                        CondCompiled::Flag { flag_id, .. } if *flag_id >= script.flag_count => {
                            return Err(VnError::InvalidScript(format!(
                                "condition flag id {flag_id} outside compiled range"
                            )));
                        }
                        _ => {}
                    }
                }
                EventCompiled::SetVar { var_id, .. } if *var_id as usize >= var_limit => {
                    return Err(VnError::InvalidScript(format!(
                        "variable id {var_id} outside compiled range"
                    )));
                }
                EventCompiled::SetFlag { flag_id, .. } if *flag_id >= script.flag_count => {
                    return Err(VnError::InvalidScript(format!(
                        "flag id {} outside compiled range",
                        flag_id
                    )));
                }
                EventCompiled::AudioAction(action) => {
                    validate_compiled_audio_action(action)?;
                }
                EventCompiled::Transition(transition)
                    if transition_kind_name(transition.kind) == "unknown" =>
                {
                    return Err(VnError::InvalidScript(format!(
                        "invalid compiled transition kind {}",
                        transition.kind
                    )));
                }
                _ => {}
            }
        }
        Ok(())
    }

    /// Backwards-compatible validation entrypoint for raw scripts.
    pub fn validate(&self, script: &ScriptRaw, limits: ResourceLimiter) -> VnResult<()> {
        self.validate_raw(script, limits)
    }
}

fn validate_path(
    path: &str,
    name: &str,
    limits: crate::resource::ResourceLimiter,
) -> crate::error::VnResult<()> {
    if path.len() > limits.max_asset_length {
        return Err(crate::error::VnError::ResourceLimit(name.to_string()));
    }
    if is_unsafe_resource_path(path) {
        return Err(crate::error::VnError::SecurityPolicy(format!(
            "{name} path is unsafe"
        )));
    }
    Ok(())
}

fn validate_positive_scale(scale: f32, name: &str) -> VnResult<()> {
    if !scale.is_finite() || scale <= 0.0 {
        return Err(VnError::InvalidScript(format!("{name} must be > 0")));
    }
    Ok(())
}

fn validate_compiled_audio_action(action: &AudioActionCompiled) -> VnResult<()> {
    if audio_channel_name(action.channel) == "unknown" {
        return Err(VnError::InvalidScript(format!(
            "invalid compiled audio channel {}",
            action.channel
        )));
    }
    if audio_action_name(action.action) == "unknown" {
        return Err(VnError::InvalidScript(format!(
            "invalid compiled audio action {}",
            action.action
        )));
    }
    if action.action == 0
        && action
            .asset
            .as_deref()
            .is_none_or(|asset| asset.trim().is_empty())
    {
        return Err(VnError::InvalidScript(
            "compiled audio play action requires a non-empty asset".to_string(),
        ));
    }
    Ok(())
}

fn validate_hex_color(color: &str, name: &str) -> VnResult<()> {
    let bytes = color.as_bytes();
    let valid_len = bytes.len() == 7 || bytes.len() == 9;
    let valid_hex =
        bytes.first() == Some(&b'#') && bytes.iter().skip(1).all(|byte| byte.is_ascii_hexdigit());
    if !valid_len || !valid_hex {
        return Err(VnError::InvalidScript(format!(
            "{name} must be #RRGGBB or #RRGGBBAA"
        )));
    }
    Ok(())
}

fn is_unsafe_resource_path(path: &str) -> bool {
    let trimmed = path.trim();
    if trimmed.is_empty()
        || trimmed.starts_with('/')
        || trimmed.starts_with('\\')
        || trimmed.contains('\\')
        || trimmed.contains(':')
    {
        return true;
    }
    let lowered = trimmed.to_ascii_lowercase();
    if lowered.contains("%2e") || lowered.contains("%2f") || lowered.contains("%5c") {
        let decoded = percent_decode_path_once(&lowered);
        if decoded != lowered {
            return is_unsafe_resource_path(&decoded);
        }
        return true;
    }
    trimmed
        .split('/')
        .any(|part| part.is_empty() || part == "." || part == "..")
}

fn percent_decode_path_once(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut output = String::with_capacity(value.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            let hi = from_hex(bytes[index + 1]);
            let lo = from_hex(bytes[index + 2]);
            if let (Some(hi), Some(lo)) = (hi, lo) {
                output.push((hi << 4 | lo) as char);
                index += 3;
                continue;
            }
        }
        output.push(bytes[index] as char);
        index += 1;
    }
    output
}

fn from_hex(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}
