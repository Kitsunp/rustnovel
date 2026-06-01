use super::*;

#[test]
fn sync_graph_to_script_builds_non_empty_scene_preview_from_visual_events() {
    let config = VnConfig::default();
    let mut workbench = EditorWorkbench::new(config);

    let start = workbench
        .node_graph
        .add_node(StoryNode::Start, egui::pos2(0.0, 0.0));
    let prelude = workbench.node_graph.add_node(
        StoryNode::SetVariable {
            key: "intro_ready".to_string(),
            value: 1,
        },
        egui::pos2(0.0, 80.0),
    );
    let scene = workbench.node_graph.add_node(
        StoryNode::Scene {
            profile: None,
            background: Some("bg/lecturehall".to_string()),
            music: Some("assets/illurock.opus".to_string()),
            characters: vec![visual_novel_engine::runtime::CharacterPlacementRaw {
                name: "sylvie".to_string(),
                expression: Some("green".to_string()),
                position: Some("center".to_string()),
                x: Some(640),
                y: Some(480),
                scale: Some(1.0),
            }],
        },
        egui::pos2(0.0, 160.0),
    );
    workbench.node_graph.connect(start, prelude);
    workbench.node_graph.connect(prelude, scene);

    workbench
        .sync_graph_to_script()
        .expect("graph should compile for preview");

    assert!(
        !workbench.scene.is_empty(),
        "scene preview should not be empty"
    );
    assert!(
        workbench.scene.iter().any(|entity| matches!(
            &entity.kind,
            visual_novel_engine::EntityKind::Image(image) if image.path.as_ref() == "bg/lecturehall"
        )),
        "preview scene should include imported background image entity"
    );
    let background = workbench
        .scene
        .iter()
        .find(|entity| matches!(
            &entity.kind,
            visual_novel_engine::EntityKind::Image(image) if image.path.as_ref() == "bg/lecturehall"
        ))
        .expect("background entity must exist");
    assert_eq!(background.transform.x, 0);
    assert_eq!(background.transform.y, 0);
    assert!(background.transform.z_order <= -50);
}

#[test]
fn scene_preview_tracks_selected_node_context() {
    let config = VnConfig::default();
    let mut workbench = EditorWorkbench::new(config);

    let start = workbench
        .node_graph
        .add_node(StoryNode::Start, egui::pos2(0.0, 0.0));
    let scene_a = workbench.node_graph.add_node(
        StoryNode::Scene {
            profile: None,
            background: Some("bg/one.png".to_string()),
            music: None,
            characters: Vec::new(),
        },
        egui::pos2(0.0, 100.0),
    );
    let scene_b = workbench.node_graph.add_node(
        StoryNode::Scene {
            profile: None,
            background: Some("bg/two.png".to_string()),
            music: None,
            characters: Vec::new(),
        },
        egui::pos2(0.0, 200.0),
    );
    workbench.node_graph.connect(start, scene_a);
    workbench.node_graph.connect(scene_a, scene_b);
    workbench
        .sync_graph_to_script()
        .expect("scene chain should compile");

    workbench.selected_node = Some(scene_b);
    workbench.refresh_scene_from_engine_preview();

    assert!(
        workbench.scene.iter().any(|entity| matches!(
            &entity.kind,
            visual_novel_engine::EntityKind::Image(image) if image.path.as_ref() == "bg/two.png"
        )),
        "composer preview should follow selected node context"
    );
}

#[test]
fn composer_preview_reconciles_graph_selection_after_composer_interaction() {
    let config = VnConfig::default();
    let mut workbench = EditorWorkbench::new(config);

    let scene_a = workbench.node_graph.add_node(
        StoryNode::Scene {
            profile: None,
            background: Some("bg/one.png".to_string()),
            music: None,
            characters: Vec::new(),
        },
        egui::pos2(0.0, 100.0),
    );
    let scene_b = workbench.node_graph.add_node(
        StoryNode::Scene {
            profile: None,
            background: Some("bg/two.png".to_string()),
            music: None,
            characters: Vec::new(),
        },
        egui::pos2(0.0, 200.0),
    );
    workbench.selected_node = Some(scene_a);
    workbench.node_graph.set_single_selection(Some(scene_b));

    assert!(workbench.reconcile_editor_selection_for_frame(Some(scene_a)));

    assert_eq!(workbench.selected_node, Some(scene_b));
    assert!(
        workbench.scene.iter().any(|entity| matches!(
            &entity.kind,
            visual_novel_engine::EntityKind::Image(image) if image.path.as_ref() == "bg/two.png"
        )),
        "composer preview should follow the graph click after prior composer interaction"
    );
    assert!(
        !workbench.scene.iter().any(|entity| matches!(
            &entity.kind,
            visual_novel_engine::EntityKind::Image(image) if image.path.as_ref() == "bg/one.png"
        )),
        "stale composer-selected scene must not remain visible after graph selection changes"
    );
}

#[test]
fn scene_preview_reconstructs_selected_context_from_start_after_runtime_advances() {
    let config = VnConfig::default();
    let mut workbench = EditorWorkbench::new(config);

    let start = workbench
        .node_graph
        .add_node(StoryNode::Start, egui::pos2(0.0, 0.0));
    let scene_a = workbench.node_graph.add_node(
        StoryNode::Scene {
            profile: None,
            background: Some("bg/one.png".to_string()),
            music: None,
            characters: Vec::new(),
        },
        egui::pos2(0.0, 100.0),
    );
    let scene_b = workbench.node_graph.add_node(
        StoryNode::Scene {
            profile: None,
            background: Some("bg/two.png".to_string()),
            music: None,
            characters: Vec::new(),
        },
        egui::pos2(0.0, 200.0),
    );
    workbench.node_graph.connect(start, scene_a);
    workbench.node_graph.connect(scene_a, scene_b);
    workbench
        .sync_graph_to_script()
        .expect("scene chain should compile");
    workbench
        .engine
        .as_mut()
        .expect("engine should exist")
        .jump_to_label(&format!("node_{scene_b}"))
        .expect("runtime should advance to the later scene");

    workbench.selected_node = Some(scene_a);
    workbench.refresh_scene_from_engine_preview();

    assert!(
        workbench.scene.iter().any(|entity| matches!(
            &entity.kind,
            visual_novel_engine::EntityKind::Image(image) if image.path.as_ref() == "bg/one.png"
        )),
        "composer preview should rebuild from script start when selecting an earlier node"
    );
    assert!(
        !workbench.scene.iter().any(|entity| matches!(
            &entity.kind,
            visual_novel_engine::EntityKind::Image(image) if image.path.as_ref() == "bg/two.png"
        )),
        "later runtime visual state must not leak into earlier selected-node preview"
    );
}

#[test]
fn scene_preview_uses_selected_branch_scene_when_default_choice_route_misses_target() {
    let config = VnConfig::default();
    let mut workbench = EditorWorkbench::new(config);

    let start = workbench
        .node_graph
        .add_node(StoryNode::Start, egui::pos2(0.0, 0.0));
    let choice = workbench.node_graph.add_node(
        StoryNode::Choice {
            prompt: "Where?".to_string(),
            options: vec!["A".to_string(), "B".to_string()],
        },
        egui::pos2(0.0, 100.0),
    );
    let scene_a = workbench.node_graph.add_node(
        StoryNode::Scene {
            profile: None,
            background: Some("bg/a.png".to_string()),
            music: None,
            characters: Vec::new(),
        },
        egui::pos2(-160.0, 220.0),
    );
    let scene_b = workbench.node_graph.add_node(
        StoryNode::Scene {
            profile: None,
            background: Some("bg/b.png".to_string()),
            music: None,
            characters: Vec::new(),
        },
        egui::pos2(160.0, 220.0),
    );
    let end = workbench
        .node_graph
        .add_node(StoryNode::End, egui::pos2(0.0, 360.0));
    workbench.node_graph.connect(start, choice);
    workbench.node_graph.connect_port(choice, 0, scene_a);
    workbench.node_graph.connect_port(choice, 1, scene_b);
    workbench.node_graph.connect(scene_a, end);
    workbench.node_graph.connect(scene_b, end);
    workbench
        .sync_graph_to_script()
        .expect("branching graph should compile");

    workbench.selected_node = Some(scene_b);
    workbench.refresh_scene_from_engine_preview();

    assert!(
        workbench.scene.iter().any(|entity| matches!(
            &entity.kind,
            visual_novel_engine::EntityKind::Image(image) if image.path.as_ref() == "bg/b.png"
        )),
        "selected branch scene should preview bg/b.png"
    );
    assert!(
        !workbench.scene.iter().any(|entity| matches!(
            &entity.kind,
            visual_novel_engine::EntityKind::Image(image) if image.path.as_ref() == "bg/a.png"
        )),
        "composer must not silently show the default choice branch"
    );
}
