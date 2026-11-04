//! Stable media names and compiled codes shared by authoring and presentation.
// Array order is the persisted u8 representation; append new kinds without reordering.
const AUDIO_CHANNELS: &[&str] = &["bgm", "sfx", "voice"];
const AUDIO_ACTIONS: &[&str] = &["play", "stop", "fade_out"];
const TRANSITION_KINDS: &[&str] = &["fade", "dissolve", "cut"];

fn code_for(name: &str, names: &[&str]) -> Option<u8> {
    names
        .iter()
        .position(|candidate| candidate.eq_ignore_ascii_case(name.trim()))
        .map(|index| index as u8)
}

pub fn audio_channel_code(name: &str) -> Option<u8> {
    code_for(name, AUDIO_CHANNELS)
}

pub fn audio_channel_name(code: u8) -> &'static str {
    AUDIO_CHANNELS
        .get(usize::from(code))
        .copied()
        .unwrap_or("unknown")
}

pub fn audio_action_code(name: &str) -> Option<u8> {
    code_for(name, AUDIO_ACTIONS)
}

pub fn audio_action_name(code: u8) -> &'static str {
    AUDIO_ACTIONS
        .get(usize::from(code))
        .copied()
        .unwrap_or("unknown")
}

pub fn transition_kind_code(name: &str) -> Option<u8> {
    if name.trim().eq_ignore_ascii_case("fade_black") {
        Some(0)
    } else {
        code_for(name, TRANSITION_KINDS)
    }
}

pub fn transition_kind_name(code: u8) -> &'static str {
    TRANSITION_KINDS
        .get(usize::from(code))
        .copied()
        .unwrap_or("unknown")
}
