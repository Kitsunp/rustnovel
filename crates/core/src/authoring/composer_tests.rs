use crate::CharacterPlacementRaw;
use std::collections::BTreeMap;

use super::{composer, AuthoringPosition, NodeGraph, StoryNode};

fn pos(x: f32, y: f32) -> AuthoringPosition {
    AuthoringPosition::new(x, y)
}

fn character(name: &str, image: &str) -> CharacterPlacementRaw {
    CharacterPlacementRaw {
        name: name.to_string(),
        expression: Some(image.to_string()),
        ..Default::default()
    }
}

#[test]
fn composer_snapshot_uses_provenance_for_duplicate_characters() {
    let mut graph = NodeGraph::new();
    let scene = graph.add_node(
        StoryNode::Scene {
            profile: None,
            background: Some("bg/room.png".to_string()),
            music: None,
            characters: vec![
                character("Ava", "char/ava_happy.png"),
                character("Ava", "char/ava_angry.png"),
            ],
        },
        pos(0.0, 0.0),
    );

    let snapshot = composer::compose_scene_snapshot(&graph, Some(scene), None, None, None, None);
    let character_objects = snapshot
        .objects
        .iter()
        .filter(|object| object.character_name.as_deref() == Some("Ava"))
        .collect::<Vec<_>>();

    assert_eq!(character_objects.len(), 2);
    assert_ne!(
        character_objects[0].object_id,
        character_objects[1].object_id
    );
    assert_ne!(
        character_objects[0].source_field_path,
        character_objects[1].source_field_path
    );

    let moved = composer::move_scene_object(
        &mut graph,
        &character_objects[1].object_id,
        640,
        480,
        Some(1.25),
    );
    assert!(moved);
    let Some(StoryNode::Scene { characters, .. }) = graph.get_node(scene) else {
        panic!("scene node should remain present");
    };
    assert_eq!(characters[0].x, None);
    assert_eq!(characters[1].x, Some(640));
    assert_eq!(characters[1].scale, Some(1.25));
}

#[test]
fn layer_lock_override_does_not_hide_object_by_default() {
    let mut graph = NodeGraph::new();
    let scene = graph.add_node(
        StoryNode::Scene {
            profile: None,
            background: Some("bg/room.png".to_string()),
            music: None,
            characters: vec![character("Ava", "char/ava_happy.png")],
        },
        pos(0.0, 0.0),
    );
    let mut snapshot =
        composer::compose_scene_snapshot(&graph, Some(scene), None, None, None, None);
    let object_id = snapshot
        .objects
        .iter()
        .find(|object| object.character_name.as_deref() == Some("Ava"))
        .expect("character object")
        .object_id
        .clone();
    let mut overrides = BTreeMap::new();

    composer::set_layer_locked(&mut overrides, &object_id, true);
    composer::apply_layer_overrides(&mut snapshot.objects, &overrides);
    let object = snapshot
        .objects
        .iter()
        .find(|object| object.object_id == object_id)
        .expect("object still present");

    assert!(object.locked);
    assert!(object.visible, "locking must not implicitly hide the layer");
}

#[test]
fn composer_snapshot_exposes_authoring_choice_overlay_without_runtime_engine() {
    let mut graph = NodeGraph::new();
    let choice = graph.add_node(
        StoryNode::Choice {
            prompt: "loc:route.prompt".to_string(),
            options: vec![
                "loc:route.left".to_string(),
                "Take the long branch with enough text to require wrapping".to_string(),
            ],
        },
        pos(0.0, 0.0),
    );
    let mut catalog = crate::LocalizationCatalog::default();
    catalog.insert_locale_table(
        "es",
        std::collections::BTreeMap::from([
            ("route.prompt".to_string(), "Elige una ruta".to_string()),
            ("route.left".to_string(), "Biblioteca".to_string()),
        ]),
    );

    let snapshot = composer::compose_scene_snapshot(
        &graph,
        Some(choice),
        Some((320, 180)),
        None,
        Some("es"),
        Some(&catalog),
    );

    assert_eq!(snapshot.overlays.len(), 1);
    match &snapshot.overlays[0] {
        composer::ComposerOverlay::Choice { prompt, options } => {
            assert_eq!(prompt, "Elige una ruta");
            assert_eq!(options[0], "Biblioteca");
            assert_eq!(
                options[1],
                "Take the long branch with enough text to require wrapping"
            );
        }
        other => panic!("expected choice overlay, got {other:?}"),
    }
}
