use super::player_menu_visibility_after_window;

#[test]
fn menu_action_can_close_window_during_render() {
    assert!(!player_menu_visibility_after_window(true, false));
}

#[test]
fn window_close_button_can_close_menu() {
    assert!(!player_menu_visibility_after_window(false, true));
}
