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
    let sync_error = workbench
        .sync_graph_to_script()
        .expect_err("strict export should keep detached draft scenes out of runtime preview");
    assert!(
        sync_error.contains("unreachable/draft"),
        "unexpected sync error: {sync_error}"
    );

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

#[test]
fn isolated_preview_mode_clears_runtime_inherited_background() {
    let config = VnConfig::default();
    let mut workbench = EditorWorkbench::new(config);

    let start = workbench
        .node_graph
        .add_node(StoryNode::Start, egui::pos2(0.0, 0.0));
    let scene_with_bg = workbench.node_graph.add_node(
        StoryNode::Scene {
            profile: None,
            background: Some("bg/room.png".to_string()),
            music: None,
            characters: Vec::new(),
        },
        egui::pos2(0.0, 120.0),
    );
    let scene_without_bg = workbench.node_graph.add_node(
        StoryNode::Scene {
            profile: None,
            background: None,
            music: None,
            characters: Vec::new(),
        },
        egui::pos2(0.0, 240.0),
    );
    workbench.node_graph.connect(start, scene_with_bg);
    workbench
        .node_graph
        .connect(scene_with_bg, scene_without_bg);
    workbench
        .sync_graph_to_script()
        .expect("runtime preview should compile");
    workbench.selected_node = Some(scene_without_bg);

    workbench.composer_preview_mode = crate::editor::ComposerPreviewMode::RuntimeInherited;
    workbench.refresh_scene_from_engine_preview();
    assert!(
        workbench.scene.iter().any(|entity| matches!(
            &entity.kind,
            visual_novel_engine::EntityKind::Image(image) if image.path.as_ref() == "bg/room.png"
        )),
        "runtime-inherited preview should show accumulated background"
    );

    workbench.composer_preview_mode = crate::editor::ComposerPreviewMode::IsolatedNode;
    workbench.refresh_scene_from_engine_preview();
    assert!(
        !workbench
            .scene
            .iter()
            .any(|entity| matches!(&entity.kind, visual_novel_engine::EntityKind::Image(_))),
        "isolated preview must clear inherited backgrounds for scenes without explicit background"
    );
}

#[test]
fn preview_mode_action_rebuilds_scene_immediately() {
    let config = VnConfig::default();
    let mut workbench = EditorWorkbench::new(config);

    let start = workbench
        .node_graph
        .add_node(StoryNode::Start, egui::pos2(0.0, 0.0));
    let scene_with_bg = workbench.node_graph.add_node(
        StoryNode::Scene {
            profile: None,
            background: Some("bg/room.png".to_string()),
            music: None,
            characters: Vec::new(),
        },
        egui::pos2(0.0, 120.0),
    );
    let scene_without_bg = workbench.node_graph.add_node(
        StoryNode::Scene {
            profile: None,
            background: None,
            music: None,
            characters: Vec::new(),
        },
        egui::pos2(0.0, 240.0),
    );
    workbench.node_graph.connect(start, scene_with_bg);
    workbench
        .node_graph
        .connect(scene_with_bg, scene_without_bg);
    workbench
        .sync_graph_to_script()
        .expect("runtime preview should compile");
    workbench.selected_node = Some(scene_without_bg);
    workbench.composer_preview_mode = crate::editor::ComposerPreviewMode::RuntimeInherited;
    workbench.refresh_scene_from_engine_preview();
    assert!(workbench.scene.iter().any(|entity| matches!(
        &entity.kind,
        visual_novel_engine::EntityKind::Image(image) if image.path.as_ref() == "bg/room.png"
    )));

    workbench.handle_composer_actions(
        vec![
            crate::editor::visual_composer::VisualComposerAction::PreviewModeChanged(
                crate::editor::ComposerPreviewMode::IsolatedNode,
            ),
        ],
        Some(scene_without_bg),
    );
    assert!(
        !workbench
            .scene
            .iter()
            .any(|entity| matches!(&entity.kind, visual_novel_engine::EntityKind::Image(_))),
        "changing preview mode from the toolbar must refresh the stage immediately"
    );

    workbench.handle_composer_actions(
        vec![
            crate::editor::visual_composer::VisualComposerAction::PreviewModeChanged(
                crate::editor::ComposerPreviewMode::RuntimeInherited,
            ),
        ],
        Some(scene_without_bg),
    );
    assert!(workbench.scene.iter().any(|entity| matches!(
        &entity.kind,
        visual_novel_engine::EntityKind::Image(image) if image.path.as_ref() == "bg/room.png"
    )));
}
