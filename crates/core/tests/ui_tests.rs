use std::sync::Arc;

use visual_novel_engine::{
    CharacterPlacementCompiled, ChoiceCompiled, ChoiceOptionCompiled, DialogueCompiled,
    EventCompiled, SceneUpdateCompiled, SharedStr, UiState, UiView, VisualState,
};

fn shared(value: &str) -> SharedStr {
    Arc::from(value)
}

#[test]
fn ui_state_maps_dialogue() {
    let event = EventCompiled::Dialogue(DialogueCompiled {
        speaker: shared("Ava"),
        text: shared("Hola"),
    });
    let ui = UiState::from_event(&event, &VisualState::default());
    assert_eq!(
        ui.view,
        UiView::Dialogue {
            speaker: "Ava".to_string(),
            text: "Hola".to_string()
        }
    );
}

#[test]
fn ui_state_maps_choice() {
    let event = EventCompiled::Choice(ChoiceCompiled {
        prompt: shared("Go?"),
        options: vec![
            ChoiceOptionCompiled {
                text: shared("Yes"),
                target_ip: 1,
            },
            ChoiceOptionCompiled {
                text: shared("No"),
                target_ip: 2,
            },
        ],
    });
    let ui = UiState::from_event(&event, &VisualState::default());
    assert_eq!(
        ui.view,
        UiView::Choice {
            prompt: "Go?".to_string(),
            options: vec!["Yes".to_string(), "No".to_string()]
        }
    );
}

#[test]
fn ui_state_maps_scene_description() {
    let event = EventCompiled::Scene(SceneUpdateCompiled {
        background: Some(shared("bg/room.png")),
        music: Some(shared("music/theme.ogg")),
        characters: vec![CharacterPlacementCompiled {
            name: shared("Ava"),
            expression: Some(shared("smile")),
            position: Some(shared("center")),
            x: None,
            y: None,
            scale: None,
        }],
    });
    let ui = UiState::from_event(&event, &VisualState::default());
    match ui.view {
        UiView::Scene { description } => {
            assert!(description.contains("Background: bg/room.png"));
            assert!(description.contains("Music: music/theme.ogg"));
            assert!(description.contains("Characters: Ava (smile) @ center"));
        }
        other => panic!("Expected scene view, got {other:?}"),
    }
}

#[test]
fn ui_state_uses_existing_visual_state_for_scene() {
    let event = EventCompiled::Scene(SceneUpdateCompiled {
        background: None,
        music: None,
        characters: Vec::new(),
    });
    let visual = VisualState {
        background: Some(shared("bg/forest.png")),
        music: Some(shared("music/ambient.ogg")),
        ..VisualState::default()
    };
    let ui = UiState::from_event(&event, &visual);
    match ui.view {
        UiView::Scene { description } => {
            assert!(description.contains("Background: bg/forest.png"));
            assert!(description.contains("Music: music/ambient.ogg"));
        }
        other => panic!("Expected scene view, got {other:?}"),
    }
}

#[test]
fn ui_state_maps_system_events() {
    let jump = EventCompiled::Jump { target_ip: 7 };
    let ui = UiState::from_event(&jump, &VisualState::default());
    assert_eq!(
        ui.view,
        UiView::System {
            message: "Jump to 7".to_string()
        }
    );

    let set_flag = EventCompiled::SetFlag {
        flag_id: 3,
        value: true,
    };
    let ui = UiState::from_event(&set_flag, &VisualState::default());
    assert_eq!(
        ui.view,
        UiView::System {
            message: "Flag 3 = true".to_string()
        }
    );
}
