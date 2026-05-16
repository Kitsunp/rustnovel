use super::super::StoryNode;

pub(super) fn searchable_text(node: &StoryNode) -> String {
    let mut fields = vec![node.type_name().to_ascii_lowercase()];
    match node {
        StoryNode::Dialogue { speaker, text } => {
            fields.push(speaker.to_ascii_lowercase());
            fields.push(text.to_ascii_lowercase());
        }
        StoryNode::Choice { prompt, options } => {
            fields.push(prompt.to_ascii_lowercase());
            fields.extend(options.iter().map(|value| value.to_ascii_lowercase()));
        }
        StoryNode::Scene {
            profile,
            background,
            music,
            characters,
        } => {
            fields.extend(profile.iter().map(|value| value.to_ascii_lowercase()));
            fields.extend(background.iter().map(|value| value.to_ascii_lowercase()));
            fields.extend(music.iter().map(|value| value.to_ascii_lowercase()));
            for character in characters {
                fields.push(character.name.to_ascii_lowercase());
                fields.extend(
                    character
                        .expression
                        .iter()
                        .map(|value| value.to_ascii_lowercase()),
                );
                fields.extend(
                    character
                        .position
                        .iter()
                        .map(|value| value.to_ascii_lowercase()),
                );
            }
        }
        StoryNode::Jump { target } | StoryNode::JumpIf { target, .. } => {
            fields.push(target.to_ascii_lowercase());
        }
        StoryNode::SetVariable { key, .. } | StoryNode::SetFlag { key, .. } => {
            fields.push(key.to_ascii_lowercase())
        }
        StoryNode::AudioAction {
            channel,
            action,
            asset,
            ..
        } => {
            fields.push(channel.to_ascii_lowercase());
            fields.push(action.to_ascii_lowercase());
            fields.extend(asset.iter().map(|value| value.to_ascii_lowercase()));
        }
        StoryNode::Transition { kind, color, .. } => {
            fields.push(kind.to_ascii_lowercase());
            fields.extend(color.iter().map(|value| value.to_ascii_lowercase()));
        }
        StoryNode::CharacterPlacement { name, .. } => fields.push(name.to_ascii_lowercase()),
        StoryNode::SubgraphCall {
            fragment_id,
            entry_port,
            exit_port,
        } => {
            fields.push(fragment_id.to_ascii_lowercase());
            fields.extend(entry_port.iter().map(|value| value.to_ascii_lowercase()));
            fields.extend(exit_port.iter().map(|value| value.to_ascii_lowercase()));
        }
        StoryNode::Generic(event) => fields.push(event.to_json_string().to_ascii_lowercase()),
        StoryNode::ScenePatch(_) | StoryNode::Start | StoryNode::End => {}
    }
    fields.join(" ")
}
