use visual_novel_engine::runtime::{InteractionSpec, LayoutRect, RenderCommand, SceneFrame};
use vnengine_runtime::{pointer_action_for_scene_frame, InputAction};

#[test]
fn pointer_action_maps_choice_buttons_from_design_to_window_coordinates() {
    let frame = SceneFrame {
        commands: vec![RenderCommand::Button {
            id: "choice:1".to_string(),
            label: "Second".to_string(),
            style: "button.choice".to_string(),
            rect: LayoutRect {
                x: 320.0,
                y: 180.0,
                width: 320.0,
                height: 80.0,
            },
        }],
        interactions: vec![InteractionSpec {
            id: "choice:1".to_string(),
            label: "Second".to_string(),
            action: "choose:1".to_string(),
        }],
        ..SceneFrame::default()
    };

    assert_eq!(
        pointer_action_for_scene_frame(&frame, (640, 360), 200.0, 110.0),
        InputAction::Choose(1)
    );
    assert_eq!(
        pointer_action_for_scene_frame(&frame, (640, 360), 40.0, 40.0),
        InputAction::None
    );
}

#[test]
fn pointer_action_maps_malformed_scene_actions_to_invalid_not_none() {
    let frame = SceneFrame {
        commands: vec![RenderCommand::Button {
            id: "choice:broken".to_string(),
            label: "Broken".to_string(),
            style: "button.choice".to_string(),
            rect: LayoutRect {
                x: 0.0,
                y: 0.0,
                width: 320.0,
                height: 80.0,
            },
        }],
        interactions: vec![InteractionSpec {
            id: "choice:broken".to_string(),
            label: "Broken".to_string(),
            action: "choose:not-a-number".to_string(),
        }],
        ..SceneFrame::default()
    };

    assert_eq!(
        pointer_action_for_scene_frame(&frame, (640, 360), 10.0, 10.0),
        InputAction::InvalidSceneAction
    );
}
