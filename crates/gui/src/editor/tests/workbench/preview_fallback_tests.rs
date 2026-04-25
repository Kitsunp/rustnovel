use super::super::*;
use crate::editor::StoryNode;

#[test]
fn scene_preview_falls_back_to_selected_unreachable_scene() {
    let config = VnConfig::default();
    let mut workbench = EditorWorkbench::new(config);

    let start = workbench
        .node_graph
        .add_node(StoryNode::Start, egui::pos2(0.0, 0.0));
    let intro = workbench.node_graph.add_node(
        StoryNode::Dialogue {
            speaker: "Narrator".to_string(),
            text: "Intro".to_string(),
        },
        egui::pos2(0.0, 100.0),
    );
    let detached_scene = workbench.node_graph.add_node(
        StoryNode::Scene {
            profile: None,
            background: Some("bg/detached.png".to_string()),
            music: Some("audio/detached.ogg".to_string()),
            characters: Vec::new(),
        },
        egui::pos2(320.0, 100.0),
    );
    workbench.node_graph.connect(start, intro);
    workbench
        .sync_graph_to_script()
        .expect("reachable intro should compile");

    workbench.selected_node = Some(detached_scene);
    workbench.refresh_scene_from_engine_preview();

    assert!(
        workbench.scene.iter().any(|entity| matches!(
            &entity.kind,
            visual_novel_engine::EntityKind::Image(image) if image.path.as_ref() == "bg/detached.png"
        )),
        "composer should preview the selected disconnected scene directly"
    );
    assert!(
        workbench.scene.iter().any(|entity| matches!(
            &entity.kind,
            visual_novel_engine::EntityKind::Audio(audio) if audio.path.as_ref() == "audio/detached.ogg"
        )),
        "composer should include selected scene audio in direct fallback"
    );
}
