use std::collections::HashMap;
use winit::event::{ElementState, WindowEvent};
use winit::keyboard::{KeyCode, PhysicalKey};

/// Input actions produced by the runtime.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum InputAction {
    None,
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
