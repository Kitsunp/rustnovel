mod app;
mod assets;
pub mod editor;
mod persist;
pub mod player_overlay;
mod widgets;

pub use app::{
    player_audio_command_label, player_event_requires_input, player_menu_action_button_size,
    player_menu_action_enabled, player_menu_action_text, player_menu_color,
    player_menu_content_height, player_menu_reference_viewport, player_menu_text_button_size,
    player_menu_window_height, player_menu_window_width, player_route_history_label,
    player_stage_available_size, player_stage_viewport_size, player_status_bar_height,
    player_text_panel_advance_enabled, run_app, sanitize_volume, DisplayInfo, GuiError,
    PlayerAudioChannel, PlayerAudioMix, ResolvedConfig, VnConfig,
};
pub use assets::{
    sanitize_rel_path, AssetError, AssetManager, AssetManifest, AssetStore, CacheStats,
    SecurityMode,
};
pub use editor::{run_editor, run_editor_with_project, EditorMode, EditorWorkbench};
pub use persist::{load_state_from, save_state_to, PersistError, UserPreferences};

/// Keep startup failures visible when a Windows application has no console.
pub fn report_startup_error(message: &str, interactive: bool) {
    eprintln!("{message}");
    #[cfg(windows)]
    if interactive {
        rfd::MessageDialog::new()
            .set_title("RustNovel could not start")
            .set_description(message)
            .set_level(rfd::MessageLevel::Error)
            .show();
    }
    #[cfg(not(windows))]
    let _ = interactive;
}
