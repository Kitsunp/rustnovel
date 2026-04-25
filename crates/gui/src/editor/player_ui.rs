//! Player UI for testing stories in the editor.

mod render;
mod state;

pub use render::render_player_ui;
pub(crate) use render::PlayerVisualContext;
pub use state::PlayerSessionState;
#[allow(unused_imports)]
pub use state::SkipMode;

#[cfg(test)]
#[path = "tests/player_ui_tests.rs"]
mod tests;
