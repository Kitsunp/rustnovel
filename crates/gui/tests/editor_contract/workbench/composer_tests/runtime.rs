use super::*;

#[test]
fn composer_runtime_preview_can_start_from_selected_node_and_advance() {
    let config = VnConfig::default();
    let mut workbench = EditorWorkbench::new(config);

    let start = workbench
        .node_graph
        .add_node(StoryNode::Start, egui::pos2(0.0, 0.0));
    let first = workbench.node_graph.add_node(
        StoryNode::Dialogue {
            speaker: "Ava".to_string(),
            text: "First".to_string(),
        },
        egui::pos2(0.0, 100.0),
    );
    let second = workbench.node_graph.add_node(
        StoryNode::Dialogue {
            speaker: "Ava".to_string(),
            text: "Second".to_string(),
        },
        egui::pos2(0.0, 200.0),
    );
    workbench.node_graph.connect(start, first);
    workbench.node_graph.connect(first, second);
    workbench.selected_node = Some(second);

    workbench.start_composer_runtime_preview_from_selection();
    let engine = workbench.engine.as_ref().expect("engine should start");
    let event = engine
        .current_event()
        .expect("selected event should be current");
    assert!(matches!(
        event,
        visual_novel_engine::runtime::EventCompiled::Dialogue(dialogue)
            if dialogue.text.as_ref() == "Second"
    ));

    workbench.advance_composer_runtime_preview(None);
    assert!(workbench
        .engine
        .as_ref()
        .and_then(|engine| engine.current_event().ok())
        .is_none());
}

#[test]
fn composer_runtime_preview_uses_live_graph_selection_when_workbench_selection_is_stale() {
    let config = VnConfig::default();
    let mut workbench = EditorWorkbench::new(config);

    let start = workbench
        .node_graph
        .add_node(StoryNode::Start, egui::pos2(0.0, 0.0));
    let first = workbench.node_graph.add_node(
        StoryNode::Dialogue {
            speaker: "Ava".to_string(),
            text: "First".to_string(),
        },
        egui::pos2(0.0, 100.0),
    );
    let second = workbench.node_graph.add_node(
        StoryNode::Dialogue {
            speaker: "Ava".to_string(),
            text: "Second".to_string(),
        },
        egui::pos2(0.0, 200.0),
    );
    workbench.node_graph.connect(start, first);
    workbench.node_graph.connect(first, second);
    workbench.node_graph.set_single_selection(Some(second));
    workbench.selected_node = Some(first);

    workbench.start_composer_runtime_preview_from_node(workbench.node_graph.selected);

    let event = workbench
        .engine
        .as_ref()
        .and_then(|engine| engine.current_event().ok())
        .expect("live graph selection should be current");
    assert!(matches!(
        event,
        visual_novel_engine::runtime::EventCompiled::Dialogue(dialogue)
            if dialogue.text.as_ref() == "Second"
    ));
    assert_eq!(workbench.selected_node, Some(second));
}

#[test]
fn composer_runtime_preview_can_start_from_selected_choice_and_choose_route() {
    let config = VnConfig::default();
    let mut workbench = EditorWorkbench::new(config);

    let start = workbench
        .node_graph
        .add_node(StoryNode::Start, egui::pos2(0.0, 0.0));
    let choice = workbench.node_graph.add_node(
        StoryNode::Choice {
            prompt: "Where now?".to_string(),
            options: vec!["Library".to_string(), "Garden".to_string()],
        },
        egui::pos2(0.0, 100.0),
    );
    let library = workbench.node_graph.add_node(
        StoryNode::Dialogue {
            speaker: "Ava".to_string(),
            text: "Library route".to_string(),
        },
        egui::pos2(-120.0, 220.0),
    );
    let garden = workbench.node_graph.add_node(
        StoryNode::Dialogue {
            speaker: "Ava".to_string(),
            text: "Garden route".to_string(),
        },
        egui::pos2(120.0, 220.0),
    );
    workbench.node_graph.connect(start, choice);
    workbench.node_graph.connect_port(choice, 0, library);
    workbench.node_graph.connect_port(choice, 1, garden);
    workbench.selected_node = Some(choice);

    workbench.start_composer_runtime_preview_from_selection();
    let event = workbench
        .engine
        .as_ref()
        .and_then(|engine| engine.current_event().ok())
        .expect("selected choice should be current");
    assert!(matches!(
        event,
        visual_novel_engine::runtime::EventCompiled::Choice(choice)
            if choice.prompt.as_ref() == "Where now?" && choice.options.len() == 2
    ));

    workbench.advance_composer_runtime_preview(Some(1));
    let event = workbench
        .engine
        .as_ref()
        .and_then(|engine| engine.current_event().ok())
        .expect("chosen route should become current");
    assert!(matches!(
        event,
        visual_novel_engine::runtime::EventCompiled::Dialogue(dialogue)
            if dialogue.text.as_ref() == "Garden route"
    ));
}
