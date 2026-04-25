use visual_novel_engine::EventCompiled;

pub fn history_bytes(
    history: &std::collections::VecDeque<visual_novel_engine::DialogueCompiled>,
) -> usize {
    history
        .iter()
        .map(|entry| entry.speaker.len() + entry.text.len())
        .sum()
}

pub fn event_kind(event: &EventCompiled) -> String {
    match event {
        EventCompiled::Dialogue(_) => "Dialogue".to_string(),
        EventCompiled::Choice(_) => "Choice".to_string(),
        EventCompiled::Scene(_) => "Scene".to_string(),
        EventCompiled::Jump { .. } => "Jump".to_string(),
        EventCompiled::SetFlag { .. } => "SetFlag".to_string(),
        EventCompiled::SetVar { .. } => "SetVar".to_string(),
        EventCompiled::JumpIf { .. } => "JumpIf".to_string(),
        EventCompiled::Patch(_) => "Patch".to_string(),
        EventCompiled::ExtCall { .. } => "ExtCall".to_string(),
        EventCompiled::AudioAction(_) => "Audio".to_string(),
        EventCompiled::Transition(_) => "Transition".to_string(),
        EventCompiled::SetCharacterPosition(_) => "Placement".to_string(),
    }
}
