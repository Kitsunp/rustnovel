//! Security policy validation for scripts.

use crate::error::{VnError, VnResult};
use crate::event::{EventCompiled, EventRaw};
use crate::resource::ResourceLimiter;
use crate::script::{ScriptCompiled, ScriptRaw};

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
        }

        for event in &script.events {
            match event {
                EventRaw::Dialogue(dialogue) => {
                    if !self.allow_empty_speaker && dialogue.speaker.trim().is_empty() {
                        return Err(VnError::SecurityPolicy(
                            "speaker cannot be empty".to_string(),
                        ));
                    }
                    if dialogue.text.len() > limits.max_text_length {
                        return Err(VnError::ResourceLimit("dialogue text".to_string()));
                    }
                }
                EventRaw::Choice(choice) => {
                    if choice.prompt.len() > limits.max_text_length {
                        return Err(VnError::ResourceLimit("choice prompt".to_string()));
                    }
                    if choice.options.is_empty() {
                        return Err(VnError::InvalidScript(
                            "choice must have options".to_string(),
                        ));
                    }
                    for option in &choice.options {
                        if option.text.len() > limits.max_text_length {
                            return Err(VnError::ResourceLimit("choice option".to_string()));
                        }
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
                EventRaw::Scene(scene) => {
                    if scene.characters.len() > limits.max_characters {
                        return Err(VnError::ResourceLimit("character count".to_string()));
                    }
                    if let Some(background) = &scene.background {
                        validate_path(background, "background asset", limits)?;
                    }
                    if let Some(music) = &scene.music {
                        validate_path(music, "music asset", limits)?;
                    }
                    for character in &scene.characters {
                        if character.name.len() > limits.max_asset_length {
                            return Err(VnError::ResourceLimit("character name".to_string()));
                        }
                        if let Some(expression) = &character.expression {
                            if expression.len() > limits.max_asset_length {
                                return Err(VnError::ResourceLimit(
                                    "character expression".to_string(),
                                ));
                            }
                        }
                        if let Some(position) = &character.position {
                            if position.len() > limits.max_asset_length {
                                return Err(VnError::ResourceLimit(
                                    "character position".to_string(),
                                ));
                            }
                        }
                    }
                }
                EventRaw::Patch(patch) => {
                    if let Some(bg) = &patch.background {
                        validate_path(bg, "background image", limits)?;
                    }
                    if let Some(music) = &patch.music {
                        validate_path(music, "music file", limits)?;
                    }
                    for character in &patch.add {
                        validate_path(&character.name, "character name", limits)?;
                        if let Some(expr) = &character.expression {
                            validate_path(expr, "character expression", limits)?;
                        }
                        if let Some(pos) = &character.position {
                            if pos.len() > limits.max_label_length {
                                return Err(VnError::ResourceLimit(
                                    "character position".to_string(),
                                ));
                            }
                        }
                    }
                    for character in &patch.update {
                        validate_path(&character.name, "character name", limits)?;
                        if let Some(expr) = &character.expression {
                            validate_path(expr, "character expression", limits)?;
                        }
                        if let Some(pos) = &character.position {
                            if pos.len() > limits.max_label_length {
                                return Err(VnError::ResourceLimit(
                                    "character position".to_string(),
                                ));
                            }
                        }
                    }
                    for name in &patch.remove {
                        validate_path(name, "character name", limits)?;
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
                EventRaw::ExtCall { command, args } => {
                    if command.len() > limits.max_label_length {
                        return Err(VnError::ResourceLimit("ext command".to_string()));
                    }
                    for arg in args {
                        if arg.len() > limits.max_text_length {
                            return Err(VnError::ResourceLimit("ext arg".to_string()));
                        }
                    }
                }
                EventRaw::AudioAction(action) => {
                    if let Some(asset) = &action.asset {
                        validate_path(asset, "audio asset", limits)?;
                    }
                }
                EventRaw::Transition(_) => {}
                EventRaw::SetCharacterPosition(pos) => {
                    validate_path(&pos.name, "character name", limits)?;
                    if let Some(scale) = pos.scale {
                        if !scale.is_finite() || scale <= 0.0 {
                            return Err(VnError::InvalidScript(
                                "set_character_position scale must be > 0".to_string(),
                            ));
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// Validates compiled targets and flag ids for runtime safety.
    pub fn validate_compiled(
        &self,
        script: &ScriptCompiled,
        _limits: ResourceLimiter,
    ) -> VnResult<()> {
        if script.start_ip as usize >= script.events.len() {
            return Err(VnError::InvalidScript(
                "compiled start_ip outside events".to_string(),
            ));
        }

        for event in &script.events {
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
                EventCompiled::JumpIf { target_ip, .. }
                    if *target_ip as usize > script.events.len() =>
                {
                    return Err(VnError::InvalidScript(format!(
                        "jump_if target_ip {} outside events",
                        target_ip
                    )));
                }
                EventCompiled::SetFlag { flag_id, .. } if *flag_id >= script.flag_count => {
                    return Err(VnError::InvalidScript(format!(
                        "flag id {} outside compiled range",
                        flag_id
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
