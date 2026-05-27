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
fn presentation_snapshot_parity() {
    let mut graph = NodeGraph::new();
    let start = graph.add_node(StoryNode::Start, pos(0.0, 0.0));
    let scene = graph.add_node(
        StoryNode::Scene {
            profile: None,
            background: Some("bg/room.png".to_string()),
            music: None,
            characters: vec![character("Ava", "char/ava_happy.png")],
        },
        pos(0.0, 90.0),
    );
    let line = graph.add_node(
        StoryNode::Dialogue {
            speaker: "Ava".to_string(),
            text: "Hello".to_string(),
        },
        pos(0.0, 180.0),
    );
    let end = graph.add_node(StoryNode::End, pos(0.0, 270.0));
    graph.connect(start, scene);
    graph.connect(scene, line);
    graph.connect(line, end);

    let composer =
        composer::compose_scene_snapshot(&graph, Some(scene), Some((800, 450)), None, None, None);
    let presentation = composer::build_presentation_snapshot(
        &graph,
        Some(scene),
        Some((800, 450)),
        None,
        None,
        None,
    );
    assert_eq!(presentation.objects, composer.objects);
    assert_eq!(presentation.overlays, composer.overlays);
    assert_eq!(presentation.safe_area.width, 720.0);

    let preview =
        composer::ComposerPreviewSession::start_from_node(&graph, scene).expect("preview session");
    let runtime_presentation = preview.presentation_snapshot(&graph, Some((800, 450)), None, None);
    assert_eq!(
        runtime_presentation.visual_background.as_deref(),
        Some("bg/room.png")
    );
    assert!(runtime_presentation.objects.iter().any(|object| {
        object.kind == composer::StageLayerKind::Background
            && object.source_node_id == Some(scene)
            && object.source_field_path == format!("graph.nodes[{scene}].visual.background")
    }));
    assert_eq!(runtime_presentation.stage_width, presentation.stage_width);
}

#[test]
fn presentation_snapshot_runtime_objects_use_visual_state_provenance() {
    let mut graph = NodeGraph::new();
    let start = graph.add_node(StoryNode::Start, pos(0.0, 0.0));
    let scene = graph.add_node(
        StoryNode::Scene {
            profile: None,
            background: Some("bg/room.png".to_string()),
            music: Some("audio/theme.ogg".to_string()),
            characters: vec![character("Ava", "char/ava_happy.png")],
        },
        pos(0.0, 90.0),
    );
    graph.connect(start, scene);

    let preview =
        composer::ComposerPreviewSession::start_from_node(&graph, scene).expect("preview session");
    let snapshot = preview.presentation_snapshot(&graph, Some((1280, 720)), None, None);

    assert!(snapshot.objects.iter().any(|object| {
        object.kind == composer::StageLayerKind::Background
            && object.asset_path.as_deref() == Some("bg/room.png")
            && object.source_node_id == Some(scene)
    }));
    assert!(snapshot.objects.iter().any(|object| {
        object.kind == composer::StageLayerKind::CharacterMain
            && object.character_name.as_deref() == Some("Ava")
            && object.asset_path.as_deref() == Some("char/ava_happy.png")
            && object.source_node_id == Some(scene)
    }));
    assert!(snapshot.objects.iter().any(|object| {
        object.kind == composer::StageLayerKind::DebugTrace
            && object.asset_path.as_deref() == Some("audio/theme.ogg")
            && object.source_node_id == Some(scene)
    }));
}

#[test]
fn presentation_snapshot_selected_authoring_overlay_wins_layer_provenance() {
    let mut graph = NodeGraph::new();
    let selected_choice = graph.add_node(
        StoryNode::Choice {
            prompt: "Selected?".to_string(),
            options: vec!["A".to_string()],
        },
        pos(0.0, 0.0),
    );
    let runtime_script = crate::ScriptRaw::new(
        vec![crate::EventRaw::Choice(crate::ChoiceRaw {
            prompt: "Runtime?".to_string(),
            options: vec![crate::ChoiceOptionRaw {
                text: "Continue".to_string(),
                target: "__end".to_string(),
            }],
        })],
        BTreeMap::from([("start".to_string(), 0), ("__end".to_string(), 1)]),
    );
    let engine = crate::Engine::new(
        runtime_script,
        crate::SecurityPolicy::default(),
        crate::ResourceLimiter::default(),
    )
    .expect("engine");

    let snapshot = composer::build_presentation_snapshot(
        &graph,
        Some(selected_choice),
        None,
        Some(&engine),
        None,
        None,
    );
    let overlay = snapshot
        .objects
        .iter()
        .find(|object| object.object_id == "overlay:choice")
        .expect("choice overlay object");

    assert_eq!(overlay.source_node_id, Some(selected_choice));
    assert_eq!(
        overlay.source_field_path,
        format!("graph.nodes[{selected_choice}].choice")
    );
}

#[test]
fn duplicated_character_pose_parity() {
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

    let snapshot =
        composer::build_presentation_snapshot(&graph, Some(scene), None, None, None, None);
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
    assert_ne!(snapshot.provenance[1], snapshot.provenance[2]);
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
fn long_choices_transition_snapshot_contract() {
    let mut graph = NodeGraph::new();
    let choice = graph.add_node(
        StoryNode::Choice {
            prompt: "Choose carefully".to_string(),
            options: vec![
                "Take the long branch with enough text to require wrapping in narrow layouts"
                    .to_string(),
                "Stay".to_string(),
            ],
        },
        pos(0.0, 0.0),
    );
    let transition = graph.add_node(
        StoryNode::Transition {
            kind: "fade".to_string(),
            duration_ms: 450,
            color: None,
        },
        pos(0.0, 90.0),
    );

    let choice_snapshot = composer::build_presentation_snapshot(
        &graph,
        Some(choice),
        Some((320, 180)),
        None,
        None,
        None,
    );
    assert!(choice_snapshot.layout.choices_rect.is_some());
    assert!(choice_snapshot.layout.dialogue_rect.is_none());

    let transition_snapshot = composer::build_presentation_snapshot(
        &graph,
        Some(transition),
        Some((320, 180)),
        None,
        None,
        None,
    );
    assert_eq!(
        transition_snapshot.transition,
        Some(composer::PresentationTransition {
            kind: "fade".to_string(),
            duration_ms: 450,
        })
    );
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
