use visual_novel_engine::{CondCompiled, CondRaw, EventCompiled, EventRaw};

pub(super) fn event_kind_compiled(event: &EventCompiled) -> &'static str {
    match event {
        EventCompiled::Dialogue(_) => "dialogue",
        EventCompiled::Choice(_) => "choice",
        EventCompiled::Scene(_) => "scene",
        EventCompiled::Jump { .. } => "jump",
        EventCompiled::SetFlag { .. } => "set_flag",
        EventCompiled::SetVar { .. } => "set_var",
        EventCompiled::JumpIf { .. } => "jump_if",
        EventCompiled::Patch(_) => "patch",
        EventCompiled::ExtCall { .. } => "ext_call",
        EventCompiled::AudioAction(_) => "audio_action",
        EventCompiled::Transition(_) => "transition",
        EventCompiled::SetCharacterPosition(_) => "set_character_position",
    }
}

pub(super) fn event_kind_raw(event: &EventRaw) -> &'static str {
    match event {
        EventRaw::Dialogue(_) => "dialogue",
        EventRaw::Choice(_) => "choice",
        EventRaw::Scene(_) => "scene",
        EventRaw::Jump { .. } => "jump",
        EventRaw::SetFlag { .. } => "set_flag",
        EventRaw::SetVar { .. } => "set_var",
        EventRaw::JumpIf { .. } => "jump_if",
        EventRaw::Patch(_) => "patch",
        EventRaw::ExtCall { .. } => "ext_call",
        EventRaw::AudioAction(_) => "audio_action",
        EventRaw::Transition(_) => "transition",
        EventRaw::SetCharacterPosition(_) => "set_character_position",
    }
}

pub(super) fn compiled_event_signature(event: &EventCompiled) -> String {
    match event {
        EventCompiled::Dialogue(d) => {
            format!("dialogue|{}|{}", d.speaker.as_ref(), d.text.as_ref())
        }
        EventCompiled::Choice(c) => {
            format!("choice|{}|{}", c.prompt.as_ref(), c.options.len())
        }
        EventCompiled::Scene(s) => format!(
            "scene|bg={:?}|music={:?}|chars={}",
            s.background.as_deref(),
            s.music.as_deref(),
            s.characters.len()
        ),
        EventCompiled::Jump { .. } => "jump".to_string(),
        EventCompiled::SetFlag { value, .. } => format!("set_flag|{}", value),
        EventCompiled::SetVar { value, .. } => format!("set_var|{}", value),
        EventCompiled::JumpIf { cond, .. } => format!("jump_if|{}", compiled_cond_signature(cond)),
        EventCompiled::Patch(p) => format!(
            "patch|bg={:?}|music={:?}|add={}|upd={}|rm={}",
            p.background.as_deref(),
            p.music.as_deref(),
            p.add.len(),
            p.update.len(),
            p.remove.len()
        ),
        EventCompiled::ExtCall { command, args } => {
            format!("ext_call|{}|{}", command, args.len())
        }
        EventCompiled::AudioAction(a) => format!(
            "audio|{}|{}|asset={:?}|vol={}|fade={:?}|loop={:?}",
            compiled_audio_channel(a.channel),
            compiled_audio_action(a.action),
            a.asset.as_deref(),
            fmt_opt_f32(a.volume),
            a.fade_duration_ms,
            a.loop_playback
        ),
        EventCompiled::Transition(t) => format!(
            "transition|{}|{}|{:?}",
            compiled_transition_kind(t.kind),
            t.duration_ms,
            t.color.as_deref()
        ),
        EventCompiled::SetCharacterPosition(p) => format!(
            "set_character_position|{}|{}|{}|{}",
            p.name.as_ref(),
            p.x,
            p.y,
            fmt_opt_f32(p.scale)
        ),
    }
}

pub(super) fn raw_event_signature(event: &EventRaw) -> String {
    match event {
        EventRaw::Dialogue(d) => format!("dialogue|{}|{}", d.speaker, d.text),
        EventRaw::Choice(c) => format!("choice|{}|{}", c.prompt, c.options.len()),
        EventRaw::Scene(s) => format!(
            "scene|bg={:?}|music={:?}|chars={}",
            s.background,
            s.music,
            s.characters.len()
        ),
        EventRaw::Jump { .. } => "jump".to_string(),
        EventRaw::SetFlag { value, .. } => format!("set_flag|{}", value),
        EventRaw::SetVar { value, .. } => format!("set_var|{}", value),
        EventRaw::JumpIf { cond, .. } => format!("jump_if|{}", raw_cond_signature(cond)),
        EventRaw::Patch(p) => format!(
            "patch|bg={:?}|music={:?}|add={}|upd={}|rm={}",
            p.background,
            p.music,
            p.add.len(),
            p.update.len(),
            p.remove.len()
        ),
        EventRaw::ExtCall { command, args } => format!("ext_call|{}|{}", command, args.len()),
        EventRaw::AudioAction(a) => format!(
            "audio|{}|{}|asset={:?}|vol={}|fade={:?}|loop={:?}",
            normalize_audio_channel(&a.channel),
            normalize_audio_action(&a.action),
            a.asset,
            fmt_opt_f32(a.volume),
            a.fade_duration_ms,
            a.loop_playback
        ),
        EventRaw::Transition(t) => {
            format!(
                "transition|{}|{}|{:?}",
                normalize_transition_kind(&t.kind),
                t.duration_ms,
                t.color
            )
        }
        EventRaw::SetCharacterPosition(p) => format!(
            "set_character_position|{}|{}|{}|{}",
            p.name,
            p.x,
            p.y,
            fmt_opt_f32(p.scale)
        ),
    }
}

fn compiled_cond_signature(cond: &CondCompiled) -> String {
    match cond {
        CondCompiled::Flag { is_set, .. } => format!("flag|{}", is_set),
        CondCompiled::VarCmp { op, value, .. } => format!("var|{:?}|{}", op, value),
    }
}

fn raw_cond_signature(cond: &CondRaw) -> String {
    match cond {
        CondRaw::Flag { is_set, .. } => format!("flag|{}", is_set),
        CondRaw::VarCmp { op, value, .. } => format!("var|{:?}|{}", op, value),
    }
}

fn compiled_audio_channel(channel: u8) -> &'static str {
    match channel {
        0 => "bgm",
        1 => "sfx",
        2 => "voice",
        _ => "unknown",
    }
}

fn compiled_audio_action(action: u8) -> &'static str {
    match action {
        0 => "play",
        1 => "stop",
        2 => "fade_out",
        _ => "unknown",
    }
}

fn compiled_transition_kind(kind: u8) -> &'static str {
    match kind {
        0 => "fade",
        1 => "dissolve",
        2 => "cut",
        _ => "unknown",
    }
}

fn normalize_audio_channel(channel: &str) -> &'static str {
    match channel.trim().to_ascii_lowercase().as_str() {
        "bgm" => "bgm",
        "sfx" => "sfx",
        "voice" => "voice",
        _ => "unknown",
    }
}

fn normalize_audio_action(action: &str) -> &'static str {
    match action.trim().to_ascii_lowercase().as_str() {
        "play" => "play",
        "stop" => "stop",
        "fade_out" => "fade_out",
        _ => "unknown",
    }
}

fn normalize_transition_kind(kind: &str) -> &'static str {
    match kind.trim().to_ascii_lowercase().as_str() {
        "fade" | "fade_black" => "fade",
        "dissolve" => "dissolve",
        "cut" => "cut",
        _ => "unknown",
    }
}

fn fmt_opt_f32(value: Option<f32>) -> String {
    match value {
        Some(v) => format!("{:.3}", v),
        None => "none".to_string(),
    }
}
