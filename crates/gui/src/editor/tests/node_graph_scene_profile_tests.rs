use super::*;
use visual_novel_engine::CharacterPlacementRaw;

fn pos(x: f32, y: f32) -> egui::Pos2 {
    egui::pos2(x, y)
}

fn character(name: &str, expression: &str, x: i32) -> CharacterPlacementRaw {
    CharacterPlacementRaw {
        name: name.to_string(),
        expression: Some(expression.to_string()),
        position: Some("center".to_string()),
        x: Some(x),
        y: Some(50),
        scale: Some(1.0),
    }
}

#[test]
fn scene_profile_save_and_apply_preserves_scene_content() {
    let mut graph = NodeGraph::new();
    let scene = graph.add_node(
        StoryNode::Scene {
            profile: None,
            background: Some("bg/room.png".to_string()),
            music: Some("bgm/theme.ogg".to_string()),
            characters: vec![character("Ava", "sprites/ava_smile.png", 30)],
        },
        pos(0.0, 0.0),
    );
    let other_scene = graph.add_node(
        StoryNode::Scene {
            profile: None,
            background: None,
            music: None,
            characters: Vec::new(),
        },
        pos(0.0, 120.0),
    );

    assert!(graph.save_scene_profile("intro", scene));
    assert!(graph.apply_scene_profile("intro", other_scene));

    let Some(StoryNode::Scene {
        profile,
        background,
        music,
        characters,
    }) = graph.get_node(other_scene)
    else {
        panic!("expected scene node");
    };

    assert_eq!(profile.as_deref(), Some("intro"));
    assert_eq!(background.as_deref(), Some("bg/room.png"));
    assert_eq!(music.as_deref(), Some("bgm/theme.ogg"));
    assert_eq!(
        characters,
        &vec![character("Ava", "sprites/ava_smile.png", 30)]
    );
}

#[test]
fn scene_profile_saves_background_character_layers_and_pose_bindings() {
    let mut graph = NodeGraph::new();
    let scene = graph.add_node(
        StoryNode::Scene {
            profile: None,
            background: Some("bg/classroom.png".to_string()),
            music: None,
            characters: vec![
                character("Ava", "sprites/ava_smile.png", 25),
                character("Mika", "sprites/mika_angry.png", 75),
            ],
        },
        pos(0.0, 0.0),
    );

    assert!(graph.save_scene_profile("classroom_intro", scene));
    let profile = graph
        .scene_profile("classroom_intro")
        .expect("saved scene profile");

    assert_eq!(profile.layers.len(), 3);
    assert_eq!(profile.layers[0].name, "background");
    assert_eq!(
        profile.layers[0].background.as_deref(),
        Some("bg/classroom.png")
    );
    assert_eq!(profile.layers[1].name, "character:Ava");
    assert_eq!(profile.layers[1].characters[0].name, "Ava");
    assert_eq!(profile.layers[2].name, "character:Mika");
    assert_eq!(profile.layers[2].characters[0].name, "Mika");

    assert_eq!(profile.poses.len(), 2);
    assert!(profile.poses.iter().any(|pose| {
        pose.character == "Ava" && pose.pose == "ava_smile" && pose.image == "sprites/ava_smile.png"
    }));
    assert!(profile.poses.iter().any(|pose| {
        pose.character == "Mika"
            && pose.pose == "mika_angry"
            && pose.image == "sprites/mika_angry.png"
    }));
}

#[test]
fn scene_profile_can_detach_and_swap_character_pose() {
    let mut graph = NodeGraph::new();
    let scene = graph.add_node(
        StoryNode::Scene {
            profile: None,
            background: Some("bg/room.png".to_string()),
            music: None,
            characters: vec![
                character("Ava", "sprites/ava_smile.png", 30),
                character("Ava", "sprites/ava_angry.png", 40),
            ],
        },
        pos(0.0, 0.0),
    );
    assert!(graph.save_scene_profile("intro", scene));

    assert!(graph.set_scene_character_pose(scene, "Ava", "ava_angry"));
    let Some(StoryNode::Scene { characters, .. }) = graph.get_node(scene) else {
        panic!("expected scene node");
    };
    assert_eq!(
        characters
            .iter()
            .find(|character| character.name == "Ava")
            .and_then(|character| character.expression.as_deref()),
        Some("sprites/ava_angry.png")
    );

    assert!(graph.detach_scene_profile(scene));
    let Some(StoryNode::Scene {
        profile,
        background,
        characters,
        ..
    }) = graph.get_node(scene)
    else {
        panic!("expected scene node");
    };
    assert!(profile.is_none());
    assert_eq!(background.as_deref(), Some("bg/room.png"));
    assert!(!characters.is_empty());
}
