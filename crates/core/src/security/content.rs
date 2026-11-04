//! Payload policy is shared by JSON and compiled events. Label names and
//! numeric runtime ranges stay in the format-specific validation below.

macro_rules! validate_event_content {
    ($event:expr, $kind:ident, $policy:expr, $limits:expr) => {
        match $event {
            $kind::Dialogue(dialogue) => {
                if !$policy.allow_empty_speaker && dialogue.speaker.trim().is_empty() {
                    return Err(VnError::SecurityPolicy(
                        "speaker cannot be empty".to_string(),
                    ));
                }
                if dialogue.text.len() > $limits.max_text_length {
                    return Err(VnError::ResourceLimit("dialogue text".to_string()));
                }
            }
            $kind::Choice(choice) => {
                if choice.prompt.len() > $limits.max_text_length {
                    return Err(VnError::ResourceLimit("choice prompt".to_string()));
                }
                if choice.options.is_empty() {
                    return Err(VnError::InvalidScript(
                        "choice must have options".to_string(),
                    ));
                }
                for option in &choice.options {
                    if option.text.len() > $limits.max_text_length {
                        return Err(VnError::ResourceLimit("choice option".to_string()));
                    }
                }
            }
            $kind::Scene(scene) => {
                if scene.characters.len() > $limits.max_characters {
                    return Err(VnError::ResourceLimit("character count".to_string()));
                }
                if let Some(background) = &scene.background {
                    if !background.is_empty() {
                        validate_path(background, "background asset", $limits)?;
                    }
                }
                if let Some(music) = &scene.music {
                    if !music.is_empty() {
                        validate_path(music, "music asset", $limits)?;
                    }
                }
                for character in &scene.characters {
                    if character.scale.is_some_and(|scale| !scale.is_finite()) {
                        return Err(VnError::InvalidScript(
                            "character scale must be finite".to_string(),
                        ));
                    }
                    if character.name.len() > $limits.max_asset_length {
                        return Err(VnError::ResourceLimit("character name".to_string()));
                    }
                    if let Some(expression) = &character.expression {
                        if expression.len() > $limits.max_asset_length {
                            return Err(VnError::ResourceLimit("character expression".to_string()));
                        }
                    }
                    if let Some(position) = &character.position {
                        if position.len() > $limits.max_asset_length {
                            return Err(VnError::ResourceLimit("character position".to_string()));
                        }
                    }
                }
            }
            $kind::Patch(patch) => {
                if let Some(bg) = &patch.background {
                    if !bg.is_empty() {
                        validate_path(bg, "background image", $limits)?;
                    }
                }
                if let Some(music) = &patch.music {
                    if !music.is_empty() {
                        validate_path(music, "music file", $limits)?;
                    }
                }
                for character in &patch.add {
                    validate_path(&character.name, "character name", $limits)?;
                    if let Some(expr) = &character.expression {
                        validate_path(expr, "character expression", $limits)?;
                    }
                    if let Some(pos) = &character.position {
                        if pos.len() > $limits.max_label_length {
                            return Err(VnError::ResourceLimit("character position".to_string()));
                        }
                    }
                    if let Some(scale) = character.scale {
                        validate_positive_scale(scale, "character scale")?;
                    }
                }
                for character in &patch.update {
                    validate_path(&character.name, "character name", $limits)?;
                    if let Some(expr) = &character.expression {
                        validate_path(expr, "character expression", $limits)?;
                    }
                    if let Some(pos) = &character.position {
                        if pos.len() > $limits.max_label_length {
                            return Err(VnError::ResourceLimit("character position".to_string()));
                        }
                    }
                    if let Some(scale) = character.scale {
                        validate_positive_scale(scale, "character patch scale")?;
                    }
                }
                for name in &patch.remove {
                    validate_path(name, "character name", $limits)?;
                }
            }
            $kind::ExtCall { command, args } => {
                if command.len() > $limits.max_label_length {
                    return Err(VnError::ResourceLimit("ext command".to_string()));
                }
                for arg in args {
                    if arg.len() > $limits.max_text_length {
                        return Err(VnError::ResourceLimit("ext arg".to_string()));
                    }
                }
            }
            $kind::AudioAction(action) => {
                if let Some(asset) = &action.asset {
                    validate_path(asset, "audio asset", $limits)?;
                }
                if let Some(volume) = action.volume {
                    if !volume.is_finite() || !(0.0..=1.0).contains(&volume) {
                        return Err(VnError::InvalidScript(
                            "audio volume must be finite and in 0.0..=1.0".to_string(),
                        ));
                    }
                }
                if let Some(duration) = action.fade_duration_ms {
                    if duration > 600_000 {
                        return Err(VnError::InvalidScript(
                            "audio fade duration must be <= 600000ms".to_string(),
                        ));
                    }
                }
            }
            $kind::Transition(transition) => {
                if transition.duration_ms > 600_000 {
                    return Err(VnError::InvalidScript(
                        "transition duration must be <= 600000ms".to_string(),
                    ));
                }
                if let Some(color) = &transition.color {
                    validate_hex_color(color, "transition color")?;
                }
            }
            $kind::SetCharacterPosition(pos) => {
                validate_path(&pos.name, "character name", $limits)?;
                if let Some(scale) = pos.scale {
                    validate_positive_scale(scale, "set_character_position scale")?;
                }
            }
            _ => {}
        }
    };
}

pub(super) use validate_event_content;
