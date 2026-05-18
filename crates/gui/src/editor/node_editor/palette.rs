use crate::editor::StoryNode;
use visual_novel_engine::{CondRaw, EventRaw, ScenePatchRaw};

pub(crate) fn extended_node_palette_items() -> Vec<(&'static str, StoryNode)> {
    vec![
        (
            "Scene Patch",
            StoryNode::ScenePatch(ScenePatchRaw::default()),
        ),
        (
            "Branch If",
            StoryNode::JumpIf {
                target: "label".to_string(),
                cond: CondRaw::Flag {
                    key: "flag".to_string(),
                    is_set: true,
                },
            },
        ),
        (
            "Set Variable",
            StoryNode::SetVariable {
                key: "variable".to_string(),
                value: 0,
            },
        ),
        (
            "Set Flag",
            StoryNode::SetFlag {
                key: "flag".to_string(),
                value: true,
            },
        ),
        (
            "Audio",
            StoryNode::AudioAction {
                channel: "bgm".to_string(),
                action: "play".to_string(),
                asset: None,
                volume: Some(1.0),
                fade_duration_ms: Some(0),
                loop_playback: Some(true),
            },
        ),
        (
            "Transition",
            StoryNode::Transition {
                kind: "fade_black".to_string(),
                duration_ms: 500,
                color: Some("#000000".to_string()),
            },
        ),
        (
            "Character Placement",
            StoryNode::CharacterPlacement {
                name: "Character".to_string(),
                x: 0,
                y: 0,
                scale: Some(1.0),
            },
        ),
        (
            "ExtCall",
            StoryNode::Generic(EventRaw::ExtCall {
                command: "command".to_string(),
                args: Vec::new(),
            }),
        ),
        (
            "Subgraph Call",
            StoryNode::SubgraphCall {
                fragment_id: String::new(),
                entry_port: None,
                exit_port: None,
            },
        ),
    ]
}
