use std::collections::HashMap;

use crate::event::{
    AudioActionRaw, CharacterPlacementRaw, CmpOp, CondRaw, DialogueRaw, ScenePatchRaw,
    SceneTransitionRaw, SceneUpdateRaw,
};

#[path = "syntax/helpers.rs"]
mod helpers;
use helpers::*;

#[derive(Debug)]
pub(super) struct SceneParse {
    pub scene: SceneUpdateRaw,
    pub transition: Option<SceneTransitionRaw>,
}

#[derive(Debug)]
pub(super) struct ShowParse {
    pub patch: ScenePatchRaw,
    pub transition: Option<SceneTransitionRaw>,
}

#[derive(Debug)]
pub(super) struct HideParse {
    pub patch: ScenePatchRaw,
    pub transition: Option<SceneTransitionRaw>,
}

pub(super) enum AssignmentValue {
    Bool(bool),
    Int(i32),
    Unsupported(String),
}

pub(super) fn parse_define_character(text: &str) -> Option<(String, String)> {
    if !text.starts_with("define ") || !text.contains("= Character(") {
        return None;
    }
    let after_define = text.trim_start_matches("define ").trim();
    let (alias, rhs) = after_define.split_once('=')?;
    let alias = alias.trim().to_string();
    let display = parse_first_quoted(rhs).unwrap_or_else(|| alias.clone());
    Some((alias, display))
}

pub(super) fn parse_image_alias(text: &str) -> Option<(String, String)> {
    if !text.starts_with("image ") {
        return None;
    }
    let body = text.trim_start_matches("image ").trim();
    let (alias, rhs) = body.split_once('=')?;
    let alias = alias.trim().to_string();
    let path = parse_first_quoted(rhs)?;
    Some((alias, path))
}

pub(super) fn parse_jump_decl(text: &str) -> Option<String> {
    if !text.starts_with("jump ") {
        return None;
    }
    Some(text.trim_start_matches("jump ").trim().to_string())
}

pub(super) fn parse_call_decl(text: &str) -> Option<String> {
    if !text.starts_with("call ") {
        return None;
    }
    Some(text.trim_start_matches("call ").trim().to_string())
}

pub(super) fn parse_if_cond_decl(text: &str) -> Option<String> {
    if !text.starts_with("if ") || !text.ends_with(':') {
        return None;
    }
    Some(
        text["if ".len()..text.len().saturating_sub(1)]
            .trim()
            .to_string(),
    )
}

pub(super) fn parse_elif_decl(text: &str) -> Option<String> {
    if !text.starts_with("elif ") || !text.ends_with(':') {
        return None;
    }
    Some(
        text["elif ".len()..text.len().saturating_sub(1)]
            .trim()
            .to_string(),
    )
}

pub(super) fn parse_menu_option_decl(text: &str) -> Option<(String, Option<CondRaw>)> {
    let trimmed = text.trim();
    let (quoted, rest) = parse_leading_quoted(trimmed)?;
    let rest = rest.trim();
    if rest == ":" {
        return Some((quoted, None));
    }
    if rest.starts_with("if ") && rest.ends_with(':') {
        let cond_expr = rest["if ".len()..rest.len().saturating_sub(1)].trim();
        return Some((quoted, parse_cond_expr(cond_expr)));
    }
    None
}

pub(super) fn parse_menu_caption_line(text: &str) -> Option<String> {
    let trimmed = text.trim();
    let (quoted, rest) = parse_leading_quoted(trimmed)?;
    if rest.trim().is_empty() {
        Some(quoted)
    } else {
        None
    }
}

pub(super) fn parse_assignment_decl(text: &str) -> Option<(String, AssignmentValue)> {
    let (raw, is_default) = if text.starts_with("default ") {
        (text.trim_start_matches("default ").trim(), true)
    } else if text.starts_with('$') {
        (text.trim_start_matches('$').trim(), false)
    } else {
        return None;
    };
    let (key, rhs) = raw.split_once('=')?;
    let key = key.trim().to_string();
    if !is_simple_identifier(&key) {
        let prefix = if is_default { "default" } else { "$" };
        return Some((
            key,
            AssignmentValue::Unsupported(format!("{prefix} assignment: {raw}")),
        ));
    }
    let rhs = rhs.trim();
    if rhs.eq_ignore_ascii_case("true") {
        return Some((key, AssignmentValue::Bool(true)));
    }
    if rhs.eq_ignore_ascii_case("false") {
        return Some((key, AssignmentValue::Bool(false)));
    }
    if let Ok(value) = rhs.parse::<i32>() {
        return Some((key, AssignmentValue::Int(value)));
    }
    let prefix = if is_default { "default" } else { "$" };
    Some((
        key,
        AssignmentValue::Unsupported(format!("{prefix} assignment: {raw}")),
    ))
}

pub(super) fn parse_cond_expr(expr: &str) -> Option<CondRaw> {
    let trimmed = expr.trim();
    if let Some(rest) = trimmed.strip_prefix("not ") {
        let key = sanitize_identifier(rest.trim())?;
        return Some(CondRaw::Flag { key, is_set: false });
    }

    let candidates = [
        ("==", CmpOp::Eq),
        ("!=", CmpOp::Ne),
        (">=", CmpOp::Ge),
        ("<=", CmpOp::Le),
        (">", CmpOp::Gt),
        ("<", CmpOp::Lt),
    ];
    for (token, op) in candidates {
        if let Some((lhs, rhs)) = trimmed.split_once(token) {
            let key = sanitize_identifier(lhs.trim())?;
            let rhs = rhs.trim();
            if rhs.eq_ignore_ascii_case("true") && matches!(op, CmpOp::Eq) {
                return Some(CondRaw::Flag { key, is_set: true });
            }
            if rhs.eq_ignore_ascii_case("false") && matches!(op, CmpOp::Eq) {
                return Some(CondRaw::Flag { key, is_set: false });
            }
            if let Ok(value) = rhs.parse::<i32>() {
                return Some(CondRaw::VarCmp { key, op, value });
            }
            return None;
        }
    }

    let key = sanitize_identifier(trimmed)?;
    Some(CondRaw::Flag { key, is_set: true })
}

pub(super) fn parse_dialogue_line(
    text: &str,
    char_aliases: &HashMap<String, String>,
) -> Option<DialogueRaw> {
    let trimmed = text.trim();
    if trimmed.starts_with('"') || trimmed.starts_with('\'') {
        let narrator_text = parse_first_quoted(trimmed)?;
        return Some(DialogueRaw {
            speaker: "Narrator".to_string(),
            text: narrator_text,
        });
    }
    let (quote_pos, _) = find_first_quote(trimmed)?;
    let (speaker_raw, quoted_and_rest) = trimmed.split_at(quote_pos);
    let dialogue_text = parse_first_quoted(quoted_and_rest)?;
    let speaker_token = speaker_raw.trim().to_string();
    if speaker_token.is_empty() {
        return None;
    }
    if speaker_token.contains(char::is_whitespace) {
        return None;
    }
    let speaker = char_aliases
        .get(&speaker_token)
        .cloned()
        .unwrap_or(speaker_token);
    Some(DialogueRaw {
        speaker,
        text: dialogue_text,
    })
}

pub(super) fn parse_scene_decl(
    text: &str,
    image_aliases: &HashMap<String, String>,
) -> Option<SceneParse> {
    if !text.starts_with("scene ") {
        return None;
    }
    let body = text.trim_start_matches("scene ").trim();
    let (content, with_clause) = split_with_clause(body);
    let alias = content.trim();
    let background = if alias.is_empty() {
        None
    } else if let Some(path) = parse_first_quoted(alias) {
        Some(path)
    } else if let Some(path) = image_aliases.get(alias) {
        Some(path.clone())
    } else {
        Some(alias.replace(' ', "/"))
    };
    Some(SceneParse {
        scene: SceneUpdateRaw {
            background,
            music: None,
            characters: Vec::new(),
        },
        transition: with_clause.and_then(parse_with_kind),
    })
}

pub(super) fn parse_show_decl(
    text: &str,
    image_aliases: &HashMap<String, String>,
) -> Option<ShowParse> {
    if !text.starts_with("show ") {
        return None;
    }
    let body = text.trim_start_matches("show ").trim();
    let (content, with_clause) = split_with_clause(body);
    let mut tokens: Vec<&str> = content.split_whitespace().collect();
    if tokens.is_empty() {
        return None;
    }

    let mut position: Option<String> = None;
    if let Some(at_idx) = tokens.iter().position(|t| *t == "at") {
        if tokens.len() > at_idx + 1 {
            position = Some(tokens[at_idx + 1].to_string());
        }
        tokens.truncate(at_idx);
    }

    let alias_candidate = tokens.join(" ");
    if let Some(path) = image_aliases.get(&alias_candidate) {
        if looks_like_background_alias(&alias_candidate, path) {
            return Some(ShowParse {
                patch: ScenePatchRaw {
                    background: Some(path.clone()),
                    music: None,
                    add: Vec::new(),
                    update: Vec::new(),
                    remove: Vec::new(),
                },
                transition: with_clause.and_then(parse_with_kind),
            });
        }
        return Some(ShowParse {
            patch: ScenePatchRaw {
                background: None,
                music: None,
                add: vec![CharacterPlacementRaw {
                    name: path.clone(),
                    expression: None,
                    position,
                    x: None,
                    y: None,
                    scale: None,
                }],
                update: Vec::new(),
                remove: Vec::new(),
            },
            transition: with_clause.and_then(parse_with_kind),
        });
    }

    if looks_like_background_alias(&alias_candidate, &alias_candidate) {
        let inferred_background = alias_candidate.replace(' ', "/");
        return Some(ShowParse {
            patch: ScenePatchRaw {
                background: Some(inferred_background),
                music: None,
                add: Vec::new(),
                update: Vec::new(),
                remove: Vec::new(),
            },
            transition: with_clause.and_then(parse_with_kind),
        });
    }

    let name = tokens.first().map(|v| (*v).to_string())?;
    let expression = tokens.get(1).map(|v| (*v).to_string());
    Some(ShowParse {
        patch: ScenePatchRaw {
            background: None,
            music: None,
            add: vec![CharacterPlacementRaw {
                name,
                expression,
                position,
                x: None,
                y: None,
                scale: None,
            }],
            update: Vec::new(),
            remove: Vec::new(),
        },
        transition: with_clause.and_then(parse_with_kind),
    })
}

pub(super) fn parse_hide_decl(text: &str) -> Option<HideParse> {
    if !text.starts_with("hide ") {
        return None;
    }
    let body = text.trim_start_matches("hide ").trim();
    let (content, with_clause) = split_with_clause(body);
    let target = content.split_whitespace().next()?.to_string();
    Some(HideParse {
        patch: ScenePatchRaw {
            background: None,
            music: None,
            add: Vec::new(),
            update: Vec::new(),
            remove: vec![target],
        },
        transition: with_clause.and_then(parse_with_kind),
    })
}

pub(super) fn parse_play_decl(text: &str) -> Option<AudioActionRaw> {
    if !text.starts_with("play ") {
        return None;
    }
    let body = text.trim_start_matches("play ").trim();
    let mut parts = body.split_whitespace();
    let channel_raw = parts.next()?.trim();
    let channel = normalize_audio_channel(channel_raw);
    let asset = parse_first_quoted(body);
    Some(AudioActionRaw {
        channel,
        action: "play".to_string(),
        asset,
        volume: None,
        fade_duration_ms: None,
        loop_playback: None,
    })
}

pub(super) fn parse_stop_decl(text: &str) -> Option<AudioActionRaw> {
    if !text.starts_with("stop ") {
        return None;
    }
    let body = text.trim_start_matches("stop ").trim();
    let channel_raw = body.split_whitespace().next()?;
    Some(AudioActionRaw {
        channel: normalize_audio_channel(channel_raw),
        action: "stop".to_string(),
        asset: None,
        volume: None,
        fade_duration_ms: None,
        loop_playback: None,
    })
}

pub(super) fn parse_queue_decl(text: &str) -> Option<String> {
    if text.starts_with("queue ") {
        return Some(text.to_string());
    }
    None
}

pub(super) fn parse_with_decl(text: &str) -> Option<SceneTransitionRaw> {
    if !text.starts_with("with ") {
        return None;
    }
    parse_with_kind(text.trim_start_matches("with ").trim())
}

fn sanitize_identifier(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed.chars().any(char::is_whitespace) {
        return None;
    }
    Some(trimmed.to_string())
}

fn normalize_audio_channel(raw: &str) -> String {
    match raw.to_ascii_lowercase().as_str() {
        "music" => "bgm".to_string(),
        "sound" => "sfx".to_string(),
        "audio" => "sfx".to_string(),
        "voice" => "voice".to_string(),
        other => other.to_string(),
    }
}

fn looks_like_background_alias(alias: &str, path: &str) -> bool {
    let alias_lc = alias.trim().to_ascii_lowercase();
    let path_lc = path.trim().replace('\\', "/").to_ascii_lowercase();
    alias_lc.starts_with("bg ")
        || alias_lc.starts_with("background ")
        || path_lc.starts_with("bg/")
        || path_lc.contains("/bg/")
        || path_lc.contains("background")
}

fn split_with_clause(input: &str) -> (&str, Option<&str>) {
    if let Some((left, right)) = input.rsplit_once(" with ") {
        return (left.trim(), Some(right.trim()));
    }
    (input.trim(), None)
}

fn parse_with_kind(kind_raw: &str) -> Option<SceneTransitionRaw> {
    let normalized = kind_raw.trim().to_ascii_lowercase();
    let kind = match normalized.as_str() {
        "fade" | "fade_black" => "fade_black",
        "dissolve" => "dissolve",
        "cut" => "cut",
        _ => "dissolve",
    };
    Some(SceneTransitionRaw {
        kind: kind.to_string(),
        duration_ms: 400,
        color: None,
    })
}
