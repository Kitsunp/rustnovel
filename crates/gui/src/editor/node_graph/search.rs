use super::*;

impl NodeGraph {
    /// Case-insensitive global search over node labels and content.
    pub fn search_nodes(&self, query: &str) -> Vec<u32> {
        let needle = query.trim().to_ascii_lowercase();
        if needle.is_empty() {
            return Vec::new();
        }

        self.nodes
            .iter()
            .filter_map(|(id, node, _)| {
                let haystack = searchable_text(node);
                if haystack.contains(&needle) {
                    Some(*id)
                } else {
                    None
                }
            })
            .collect()
    }
}

fn searchable_text(node: &StoryNode) -> String {
    let mut fields = Vec::new();
    fields.push(node.type_name().to_ascii_lowercase());
    match node {
        StoryNode::Dialogue { speaker, text } => {
            fields.push(speaker.to_ascii_lowercase());
            fields.push(text.to_ascii_lowercase());
        }
        StoryNode::Choice { prompt, options } => {
            fields.push(prompt.to_ascii_lowercase());
            for option in options {
                fields.push(option.to_ascii_lowercase());
            }
        }
        StoryNode::Scene {
            profile,
            background,
            music,
            characters,
        } => {
            if let Some(profile) = profile {
                fields.push(profile.to_ascii_lowercase());
            }
            if let Some(background) = background {
                fields.push(background.to_ascii_lowercase());
            }
            if let Some(music) = music {
                fields.push(music.to_ascii_lowercase());
            }
            for character in characters {
                fields.push(character.name.to_ascii_lowercase());
                if let Some(expression) = &character.expression {
                    fields.push(expression.to_ascii_lowercase());
                }
                if let Some(position) = &character.position {
                    fields.push(position.to_ascii_lowercase());
                }
            }
        }
        StoryNode::Jump { target } => fields.push(target.to_ascii_lowercase()),
        StoryNode::SetVariable { key, .. } => fields.push(key.to_ascii_lowercase()),
        StoryNode::JumpIf { target, .. } => fields.push(target.to_ascii_lowercase()),
        StoryNode::ScenePatch(_) => {}
        StoryNode::AudioAction {
            channel,
            action,
            asset,
            ..
        } => {
            fields.push(channel.to_ascii_lowercase());
            fields.push(action.to_ascii_lowercase());
            if let Some(asset) = asset {
                fields.push(asset.to_ascii_lowercase());
            }
        }
        StoryNode::Transition { kind, color, .. } => {
            fields.push(kind.to_ascii_lowercase());
            if let Some(color) = color {
                fields.push(color.to_ascii_lowercase());
            }
        }
        StoryNode::CharacterPlacement { name, .. } => fields.push(name.to_ascii_lowercase()),
        StoryNode::Generic(event) => fields.push(event.to_json_string().to_ascii_lowercase()),
        StoryNode::Start | StoryNode::End => {}
    }
    fields.join(" ")
}
