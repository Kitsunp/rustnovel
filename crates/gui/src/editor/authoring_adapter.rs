//! Adapter between the headless authoring graph and the egui editor graph.
//!
//! The GUI keeps interaction state such as selection, pan, zoom and drag state.
//! Semantic nodes, connections, scene profiles and script lowering live in
//! `visual_novel_engine::authoring`.

use std::collections::BTreeMap;

use eframe::egui;
use visual_novel_engine::authoring::{
    self, AuthoringPosition, NodeGraph as AuthoringGraph, StoryNode as AuthoringStoryNode,
};

use super::node_graph::{CharacterPoseBinding, NodeGraph, SceneLayer, SceneProfile};
use super::node_types::StoryNode;

pub(crate) fn to_authoring_graph(graph: &NodeGraph) -> AuthoringGraph {
    let mut authoring = AuthoringGraph::new();
    let mut id_map = BTreeMap::new();

    for (id, node, pos) in graph.nodes() {
        let _ = authoring.add_node_with_id(
            *id,
            to_authoring_node(node),
            AuthoringPosition::new(pos.x, pos.y),
        );
        id_map.insert(*id, *id);
    }

    for connection in graph.connections() {
        let Some(from) = id_map.get(&connection.from).copied() else {
            continue;
        };
        let Some(to) = id_map.get(&connection.to).copied() else {
            continue;
        };
        authoring.connect_port(from, connection.from_port, to);
    }

    for (profile_id, profile) in &graph.scene_profiles {
        authoring.insert_scene_profile(profile_id.clone(), to_authoring_scene_profile(profile));
    }
    for (name, node_id) in &graph.bookmarks {
        if let Some(mapped_id) = id_map.get(node_id).copied() {
            authoring.set_bookmark(name.clone(), mapped_id);
        }
    }

    if !graph.is_modified() {
        authoring.clear_modified();
    }
    authoring
}

pub(crate) fn from_authoring_graph(authoring: &AuthoringGraph) -> NodeGraph {
    let mut graph = NodeGraph::new();
    let mut id_map = BTreeMap::new();

    for (id, node, pos) in authoring.nodes() {
        let _ = graph.add_node_with_id(*id, from_authoring_node(node), egui::pos2(pos.x, pos.y));
        id_map.insert(*id, *id);
    }

    for connection in authoring.connections() {
        let Some(from) = id_map.get(&connection.from).copied() else {
            continue;
        };
        let Some(to) = id_map.get(&connection.to).copied() else {
            continue;
        };
        graph.connect_port(from, connection.from_port, to);
    }

    for (profile_id, profile) in authoring.scene_profiles() {
        graph
            .scene_profiles
            .insert(profile_id.clone(), from_authoring_scene_profile(profile));
    }
    for (name, node_id) in authoring.bookmarks() {
        if let Some(mapped_id) = id_map.get(node_id).copied() {
            graph.bookmarks.insert(name.clone(), mapped_id);
        }
    }

    if !authoring.is_modified() {
        graph.clear_modified();
    }
    graph
}

pub(crate) fn replace_gui_semantics_from_authoring(
    graph: &mut NodeGraph,
    authoring: &AuthoringGraph,
) {
    let selected = graph.selected;
    let pan = graph.pan;
    let zoom = graph.zoom;
    let editing = graph.editing;
    let dragging_node = graph.dragging_node;
    let connecting_from = graph.connecting_from;
    let context_menu = graph.context_menu.clone();
    let mut next = from_authoring_graph(authoring);
    next.selected = selected.filter(|id| next.get_node(*id).is_some());
    next.pan = pan;
    next.zoom = zoom;
    next.editing = editing.filter(|id| next.get_node(*id).is_some());
    next.dragging_node = dragging_node.filter(|id| next.get_node(*id).is_some());
    next.connecting_from = connecting_from.filter(|(id, _)| next.get_node(*id).is_some());
    next.context_menu = context_menu.filter(|menu| next.get_node(menu.node_id).is_some());
    *graph = next;
}

pub(crate) fn to_authoring_node(node: &StoryNode) -> AuthoringStoryNode {
    node.clone()
}

fn from_authoring_node(node: &AuthoringStoryNode) -> StoryNode {
    node.clone()
}

fn to_authoring_scene_profile(profile: &SceneProfile) -> authoring::SceneProfile {
    authoring::SceneProfile {
        background: profile.background.clone(),
        music: profile.music.clone(),
        characters: profile.characters.clone(),
        layers: profile
            .layers
            .iter()
            .map(to_authoring_scene_layer)
            .collect(),
        poses: profile
            .poses
            .iter()
            .map(to_authoring_pose_binding)
            .collect(),
    }
}

fn from_authoring_scene_profile(profile: &authoring::SceneProfile) -> SceneProfile {
    SceneProfile {
        background: profile.background.clone(),
        music: profile.music.clone(),
        characters: profile.characters.clone(),
        layers: profile
            .layers
            .iter()
            .map(from_authoring_scene_layer)
            .collect(),
        poses: profile
            .poses
            .iter()
            .map(from_authoring_pose_binding)
            .collect(),
    }
}

fn to_authoring_scene_layer(layer: &SceneLayer) -> authoring::SceneLayer {
    authoring::SceneLayer {
        name: layer.name.clone(),
        visible: layer.visible,
        background: layer.background.clone(),
        characters: layer.characters.clone(),
    }
}

fn from_authoring_scene_layer(layer: &authoring::SceneLayer) -> SceneLayer {
    SceneLayer {
        name: layer.name.clone(),
        visible: layer.visible,
        background: layer.background.clone(),
        characters: layer.characters.clone(),
    }
}

fn to_authoring_pose_binding(binding: &CharacterPoseBinding) -> authoring::CharacterPoseBinding {
    authoring::CharacterPoseBinding {
        character: binding.character.clone(),
        pose: binding.pose.clone(),
        image: binding.image.clone(),
    }
}

fn from_authoring_pose_binding(binding: &authoring::CharacterPoseBinding) -> CharacterPoseBinding {
    CharacterPoseBinding {
        character: binding.character.clone(),
        pose: binding.pose.clone(),
        image: binding.image.clone(),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use visual_novel_engine::{
        CharacterPlacementRaw, ChoiceOptionRaw, ChoiceRaw, DialogueRaw, EventRaw, SceneUpdateRaw,
        ScriptRaw,
    };

    use super::*;
    use crate::editor::script_sync;

    #[test]
    fn adapter_roundtrip_preserves_semantic_graph_state() {
        let mut graph = NodeGraph::new();
        let start = graph.add_node(StoryNode::Start, egui::pos2(12.0, 24.0));
        let choice = graph.add_node(
            StoryNode::Choice {
                prompt: "Path?".to_string(),
                options: vec!["A".to_string(), "B".to_string()],
            },
            egui::pos2(120.0, 180.0),
        );
        let scene = graph.add_node(
            StoryNode::Scene {
                profile: None,
                background: Some("bg/room.png".to_string()),
                music: Some("bgm/theme.ogg".to_string()),
                characters: vec![CharacterPlacementRaw {
                    name: "Ava".to_string(),
                    expression: Some("ava/smile.png".to_string()),
                    position: Some("left".to_string()),
                    ..Default::default()
                }],
            },
            egui::pos2(330.0, 210.0),
        );
        let end = graph.add_node(StoryNode::End, egui::pos2(500.0, 260.0));
        graph.connect(start, choice);
        graph.connect_port(choice, 1, scene);
        graph.connect(scene, end);
        assert!(graph.save_scene_profile("room", scene));
        assert!(graph.set_bookmark("scene:room", scene));
        graph.clear_modified();

        let authoring = to_authoring_graph(&graph);
        let restored = from_authoring_graph(&authoring);

        assert_eq!(restored.len(), graph.len());
        assert_eq!(restored.connection_count(), graph.connection_count());
        assert_eq!(restored.scene_profile_names(), vec!["room".to_string()]);
        assert_eq!(restored.bookmarked_node("scene:room"), Some(scene));
        assert!(!restored.is_modified());

        let restored_pos = restored
            .nodes()
            .find(|(id, _, _)| *id == choice)
            .map(|(_, _, pos)| *pos)
            .expect("choice pos");
        assert_eq!(restored_pos, egui::pos2(120.0, 180.0));
    }

    #[test]
    fn script_sync_matches_core_authoring_lowering() {
        let script = ScriptRaw::new(
            vec![
                EventRaw::Dialogue(DialogueRaw {
                    speaker: "Narrator".to_string(),
                    text: "Hello".to_string(),
                }),
                EventRaw::Choice(ChoiceRaw {
                    prompt: "Stay?".to_string(),
                    options: vec![ChoiceOptionRaw {
                        text: "End".to_string(),
                        target: "__end".to_string(),
                    }],
                }),
                EventRaw::Scene(SceneUpdateRaw {
                    background: Some("bg/room.png".to_string()),
                    music: None,
                    characters: Vec::new(),
                }),
            ],
            BTreeMap::from([("start".to_string(), 0), ("__end".to_string(), 3)]),
        );

        let gui_script = script_sync::to_script(&script_sync::from_script(&script));
        let core_script = AuthoringGraph::from_script(&script).to_script();
        assert_eq!(
            gui_script.to_json().expect("gui json"),
            core_script.to_json().expect("core json")
        );
    }
}
