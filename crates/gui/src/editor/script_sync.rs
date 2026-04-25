//! Script synchronization for the visual editor graph.
//!
//! Semantic import/export is delegated to `visual_novel_engine::authoring`.
//! The GUI layer only adapts egui positions and interaction state.

use visual_novel_engine::{authoring::NodeGraph as AuthoringGraph, ScriptRaw};

use super::authoring_adapter::{from_authoring_graph, to_authoring_graph};
use super::node_graph::NodeGraph;

/// Creates a GUI `NodeGraph` from a raw script.
///
/// The headless authoring model owns the script semantics. The GUI applies its
/// layout pass afterward so imported scripts remain immediately navigable.
pub fn from_script(script: &ScriptRaw) -> NodeGraph {
    let authoring = AuthoringGraph::from_script(script);
    let mut graph = from_authoring_graph(&authoring);
    graph.auto_layout_hierarchical();
    graph.zoom_to_fit();
    graph.clear_modified();
    graph
}

/// Converts a GUI `NodeGraph` to a raw script.
pub fn to_script(graph: &NodeGraph) -> ScriptRaw {
    to_authoring_graph(graph).to_script()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use eframe::egui;
    use visual_novel_engine::{
        CharacterPlacementRaw, ChoiceOptionRaw, ChoiceRaw, DialogueRaw, EventRaw, SceneUpdateRaw,
        ScriptRaw,
    };

    use super::*;
    use crate::editor::StoryNode;

    #[test]
    fn test_roundtrip_empty_script() {
        let script = ScriptRaw::new(vec![], BTreeMap::new());
        let graph = from_script(&script);
        let roundtrip = to_script(&graph);

        assert!(roundtrip.events.is_empty());
    }

    #[test]
    fn test_roundtrip_single_dialogue() {
        let original = ScriptRaw::new(
            vec![EventRaw::Dialogue(DialogueRaw {
                speaker: "Alice".to_string(),
                text: "Hello, world!".to_string(),
            })],
            BTreeMap::from([("start".to_string(), 0)]),
        );
        let graph = from_script(&original);
        let roundtrip = to_script(&graph);

        assert!(!roundtrip.events.is_empty());
        assert!(roundtrip.labels.contains_key("start"));
    }

    #[test]
    fn test_roundtrip_scene_preserves_music_and_characters() {
        let original = ScriptRaw::new(
            vec![EventRaw::Scene(SceneUpdateRaw {
                background: Some("bg/room.png".to_string()),
                music: Some("bgm/theme.ogg".to_string()),
                characters: vec![CharacterPlacementRaw {
                    name: "Ava".to_string(),
                    expression: Some("smile".to_string()),
                    position: Some("left".to_string()),
                    x: Some(10),
                    y: Some(20),
                    scale: Some(1.2),
                }],
            })],
            BTreeMap::from([("start".to_string(), 0)]),
        );
        let graph = from_script(&original);
        let roundtrip = to_script(&graph);

        let Some(EventRaw::Scene(scene)) = roundtrip.events.first() else {
            panic!("Expected first event to be scene");
        };
        assert_eq!(scene.background.as_deref(), Some("bg/room.png"));
        assert_eq!(scene.music.as_deref(), Some("bgm/theme.ogg"));
        assert_eq!(scene.characters.len(), 1);
        assert_eq!(scene.characters[0].name, "Ava");
    }

    #[test]
    fn test_unlinked_choice_option_is_explicitly_marked() {
        let mut graph = NodeGraph::new();
        let start_id = graph.add_node(StoryNode::Start, egui::pos2(50.0, 30.0));
        let choice_id = graph.add_node(
            StoryNode::Choice {
                prompt: "Elige".to_string(),
                options: vec!["A".to_string(), "B".to_string()],
            },
            egui::pos2(100.0, 120.0),
        );
        graph.connect(start_id, choice_id);

        let script = to_script(&graph);
        let Some(EventRaw::Choice(choice)) = script.events.first() else {
            panic!("Expected first event to be choice");
        };
        assert_eq!(choice.options.len(), 2);
        assert!(choice.options[0].target.starts_with("__unlinked_node_"));
        assert!(choice.options[1].target.starts_with("__unlinked_node_"));
    }

    #[test]
    fn test_choice_targeting_end_label_roundtrips_without_unlinked_target() {
        let script = ScriptRaw::new(
            vec![EventRaw::Choice(ChoiceRaw {
                prompt: "Salir?".to_string(),
                options: vec![ChoiceOptionRaw {
                    text: "Fin".to_string(),
                    target: "__end".to_string(),
                }],
            })],
            BTreeMap::from([("start".to_string(), 0), ("__end".to_string(), 1)]),
        );

        let graph = from_script(&script);
        let roundtrip = to_script(&graph);

        let Some(EventRaw::Choice(choice)) = roundtrip.events.first() else {
            panic!("Expected first event to be choice");
        };
        assert_eq!(choice.options[0].target, "__end");
        assert!(
            roundtrip.compile().is_ok(),
            "roundtrip script should remain compilable when targeting __end"
        );
    }
}
