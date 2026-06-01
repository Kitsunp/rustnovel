use std::collections::HashMap;
use visual_novel_engine::runtime::{RenderCommand, SceneFrame};
use winit::event::{ElementState, WindowEvent};
use winit::keyboard::{KeyCode, PhysicalKey};

const DESIGN_WIDTH: f64 = 1280.0;
const DESIGN_HEIGHT: f64 = 720.0;

/// Input actions produced by the runtime.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum InputAction {
    None,
    InvalidSceneAction,
    Advance,
    Choose(usize),
    Quit,
    Back,
    Menu,
}

/// Input trait that maps window events into engine actions.
pub trait Input {
    fn handle_window_event(&mut self, event: &WindowEvent) -> InputAction;
}

/// A flexible input handler that maps keys to actions.
#[derive(Clone, Debug)]
pub struct ConfigurableInput {
    key_map: HashMap<KeyCode, InputAction>,
}

impl ConfigurableInput {
    /// Creates a new input handler with the given key mappings.
    pub fn new(key_map: HashMap<KeyCode, InputAction>) -> Self {
        Self { key_map }
    }
}

impl Default for ConfigurableInput {
    fn default() -> Self {
        let mut map = HashMap::new();
        map.insert(KeyCode::Space, InputAction::Advance);
        map.insert(KeyCode::Enter, InputAction::Advance);
        map.insert(KeyCode::Escape, InputAction::Quit);

        map.insert(KeyCode::Digit1, InputAction::Choose(0));
        map.insert(KeyCode::Digit2, InputAction::Choose(1));
        map.insert(KeyCode::Digit3, InputAction::Choose(2));
        map.insert(KeyCode::Digit4, InputAction::Choose(3));
        map.insert(KeyCode::Digit5, InputAction::Choose(4));
        map.insert(KeyCode::Digit6, InputAction::Choose(5));
        map.insert(KeyCode::Digit7, InputAction::Choose(6));
        map.insert(KeyCode::Digit8, InputAction::Choose(7));
        map.insert(KeyCode::Digit9, InputAction::Choose(8));

        Self { key_map: map }
    }
}

impl Input for ConfigurableInput {
    fn handle_window_event(&mut self, event: &WindowEvent) -> InputAction {
        if let WindowEvent::KeyboardInput {
            event: key_event, ..
        } = event
        {
            if key_event.state == ElementState::Pressed {
                if let PhysicalKey::Code(key) = key_event.physical_key {
                    if let Some(action) = self.key_map.get(&key) {
                        return *action;
                    }
                }
            }
        }
        InputAction::None
    }
}

/// Maps a pointer press in window coordinates to the topmost scene-frame button action.
pub fn pointer_action_for_scene_frame(
    frame: &SceneFrame,
    window_size: (u32, u32),
    x: f64,
    y: f64,
) -> InputAction {
    for command in frame.commands.iter().rev() {
        let RenderCommand::Button { id, rect, .. } = command else {
            continue;
        };
        if !point_in_scaled_rect(window_size, x, y, *rect) {
            continue;
        }
        if let Some(interaction) = frame
            .interactions
            .iter()
            .find(|interaction| interaction.id == *id)
        {
            return input_action_from_scene_action(&interaction.action);
        }
    }
    InputAction::None
}

fn point_in_scaled_rect(
    window_size: (u32, u32),
    x: f64,
    y: f64,
    rect: visual_novel_engine::runtime::LayoutRect,
) -> bool {
    let sx = window_size.0 as f64 / DESIGN_WIDTH;
    let sy = window_size.1 as f64 / DESIGN_HEIGHT;
    let min_x = rect.x as f64 * sx;
    let min_y = rect.y as f64 * sy;
    let max_x = (rect.x + rect.width) as f64 * sx;
    let max_y = (rect.y + rect.height) as f64 * sy;
    x >= min_x && x <= max_x && y >= min_y && y <= max_y
}

fn input_action_from_scene_action(action: &str) -> InputAction {
    match action {
        "advance" | "continue" | "resume" => InputAction::Advance,
        "quit" => InputAction::Quit,
        "back" => InputAction::Back,
        "menu" => InputAction::Menu,
        _ => action
            .strip_prefix("choose:")
            .map(|index| {
                index
                    .parse::<usize>()
                    .map(InputAction::Choose)
                    .unwrap_or(InputAction::InvalidSceneAction)
            })
            .unwrap_or(InputAction::InvalidSceneAction),
    }
}
