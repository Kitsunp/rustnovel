use std::collections::HashMap;

use visual_novel_engine::{Engine, SceneState};

use crate::editor::{ComposerPreviewMode, StoryNode};

pub(super) fn short_event_label(event: &visual_novel_engine::EventCompiled) -> String {
    match event {
        visual_novel_engine::EventCompiled::Scene(scene) => {
            let background = scene.background.as_deref().unwrap_or("<none>");
            format!("Event: Scene bg={background}")
        }
        visual_novel_engine::EventCompiled::Patch(patch) => {
            let background = patch.background.as_deref().unwrap_or("<none>");
            format!("Event: Patch bg={background}")
        }
        visual_novel_engine::EventCompiled::Dialogue(dialogue) => {
            format!("Event: Dialogue {}", dialogue.speaker.as_ref())
        }
        visual_novel_engine::EventCompiled::Choice(choice) => {
            format!("Event: Choice {} option(s)", choice.options.len())
        }
        visual_novel_engine::EventCompiled::AudioAction(action) => {
            format!("Event: Audio {}", audio_channel_label(action.channel))
        }
        _ => format!("Event: {:?}", event),
    }
}

pub(super) fn audio_channel_label(channel: u8) -> &'static str {
    match channel {
        0 => "bgm",
        1 => "sfx",
        2 => "voice",
        _ => "unknown",
    }
}

pub(crate) fn preview_source_label(
    scene: &SceneState,
    engine: &Option<Engine>,
    preview_mode: ComposerPreviewMode,
    selected_node_id: Option<u32>,
    selected_node: Option<&StoryNode>,
    entity_owners: &HashMap<u32, u32>,
) -> &'static str {
    if scene.is_empty() {
        return "Preview: Empty";
    }

    let background_is_inherited = selected_node_id.is_some()
        && selected_node.is_some_and(scene_node_has_no_own_background)
        && scene.iter().any(|entity| {
            crate::editor::scene_stage::is_background_image(&entity.kind, entity.transform.z_order)
                && entity_owners
                    .get(&entity.id.raw())
                    .is_some_and(|owner| Some(*owner) != selected_node_id)
        });

    if preview_mode == ComposerPreviewMode::IsolatedNode {
        "Preview: Isolated node"
    } else if background_is_inherited {
        "Preview: Runtime inherited"
    } else if engine.is_some() {
        "Preview: Runtime"
    } else {
        "Preview: Selection direct"
    }
}

fn scene_node_has_no_own_background(node: &StoryNode) -> bool {
    match node {
        StoryNode::Scene { background, .. } => background.is_none(),
        StoryNode::ScenePatch(patch) => patch.background.is_none(),
        _ => false,
    }
}
