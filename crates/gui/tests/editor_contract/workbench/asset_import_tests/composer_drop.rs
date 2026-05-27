use super::*;

#[test]
fn dropped_character_adds_to_selected_scene_without_creating_extra_node_and_upserts_exact_asset() {
    let temp = tempdir().expect("tempdir");
    let project_root = temp.path().join("project");
    std::fs::create_dir_all(&project_root).expect("mkdir project");

    let mut workbench = workbench_with_project(&project_root);
    let node_id = workbench
        .node_graph
        .add_node(empty_scene(), egui::pos2(0.0, 0.0));
    let before_nodes = workbench.node_graph.len();
    let before_assign = workbench.node_graph.clone();

    add_character(
        &mut workbench,
        node_id,
        "assets/characters/ava_happy.png",
        320,
        240,
    );
    add_character(
        &mut workbench,
        node_id,
        "assets/characters/ava_happy.png",
        420,
        260,
    );
    workbench.commit_modified_graph(before_assign);

    assert_eq!(workbench.node_graph.len(), before_nodes);
    assert_scene_character(
        &workbench,
        node_id,
        0,
        "Ava",
        "assets/characters/ava_happy.png",
        420,
    );
    let entry = workbench
        .operation_log
        .last()
        .expect("character assignment should be logged");
    assert_eq!(entry.operation_kind, "field_edited");
    let expected_field_path = format!("graph.nodes[{node_id}].characters[0]");
    assert_eq!(
        entry.field_paths.first().map(|path| path.value.as_str()),
        Some(expected_field_path.as_str())
    );
    assert!(entry
        .before_value
        .as_deref()
        .is_some_and(|value| value.contains("x=320")));
    assert!(entry
        .after_value
        .as_deref()
        .is_some_and(|value| value.contains("x=420")));
}

#[test]
fn dropped_same_name_different_character_assets_remain_distinct_scene_instances() {
    let temp = tempdir().expect("tempdir");
    let project_root = temp.path().join("project");
    std::fs::create_dir_all(&project_root).expect("mkdir project");

    let mut workbench = workbench_with_project(&project_root);
    let node_id = workbench
        .node_graph
        .add_node(empty_scene(), egui::pos2(0.0, 0.0));

    add_character(
        &mut workbench,
        node_id,
        "assets/characters/ava_happy.png",
        240,
        240,
    );
    add_character(
        &mut workbench,
        node_id,
        "assets/characters/ava_angry.png",
        520,
        240,
    );

    let Some(StoryNode::Scene { characters, .. }) = workbench.node_graph.get_node(node_id) else {
        panic!("expected scene node");
    };
    assert_eq!(characters.len(), 2);
    assert_eq!(characters[0].name, characters[1].name);
    assert_ne!(characters[0].expression, characters[1].expression);
    assert_eq!(characters[0].x, Some(240));
    assert_eq!(characters[1].x, Some(520));
}

fn add_character(workbench: &mut EditorWorkbench, node_id: u32, asset: &str, x: i32, y: i32) {
    workbench
        .add_character_asset_to_node(node_id, "Ava".to_string(), asset.to_string(), x, y)
        .expect("add character asset");
}

fn empty_scene() -> StoryNode {
    StoryNode::Scene {
        profile: None,
        background: None,
        music: None,
        characters: Vec::new(),
    }
}

fn assert_scene_character(
    workbench: &EditorWorkbench,
    node_id: u32,
    index: usize,
    name: &str,
    expression: &str,
    x: i32,
) {
    let Some(StoryNode::Scene { characters, .. }) = workbench.node_graph.get_node(node_id) else {
        panic!("expected scene node");
    };
    assert_eq!(characters.len(), 1);
    let character = &characters[index];
    assert_eq!(character.name, name);
    assert_eq!(character.expression.as_deref(), Some(expression));
    assert_eq!(character.x, Some(x));
    assert_eq!(character.y, Some(260));
}
