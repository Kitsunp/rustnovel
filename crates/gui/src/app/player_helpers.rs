use super::*;

pub fn player_stage_viewport_size(available: egui::Vec2, stage_size: (f32, f32)) -> egui::Vec2 {
    let max_width = available.x.max(1.0);
    let max_height = available.y.max(1.0);
    let (stage_w, stage_h) = stage_size;
    if !stage_w.is_finite() || !stage_h.is_finite() || stage_w <= 0.0 || stage_h <= 0.0 {
        return egui::vec2(max_width, max_height);
    }

    let aspect = stage_w / stage_h;
    let mut width = max_width;
    let mut height = width / aspect;
    if height > max_height {
        height = max_height;
        width = height * aspect;
    }
    egui::vec2(
        width.max(1.0).min(max_width),
        height.max(1.0).min(max_height),
    )
}

pub fn player_status_bar_height(has_status: bool, has_error: bool) -> f32 {
    let lines = usize::from(has_status) + usize::from(has_error);
    if lines == 0 {
        0.0
    } else {
        PLAYER_STATUS_BAR_PADDING + PLAYER_STATUS_LINE_HEIGHT * lines as f32
    }
}

pub fn player_stage_available_size(
    available: egui::Vec2,
    reserved_status_height: f32,
) -> egui::Vec2 {
    egui::vec2(
        available.x.max(1.0),
        (available.y - reserved_status_height.max(0.0)).max(1.0),
    )
}

pub fn player_event_requires_input(event: &EventCompiled) -> bool {
    matches!(
        event,
        EventCompiled::Dialogue(_) | EventCompiled::Choice(_) | EventCompiled::Transition(_)
    )
}

pub fn player_menu_window_width(viewport_width: f32, style: &PlayerMenuStyleConfig) -> f32 {
    let viewport_width = viewport_width.max(1.0);
    let viewport_cap = (viewport_width - 24.0).max(240.0);
    let fraction_cap = (viewport_width * style.panel_width_fraction).min(viewport_cap);
    style
        .panel_width
        .min(fraction_cap)
        .min(viewport_cap)
        .max(viewport_cap.min(240.0))
}

pub fn player_menu_reference_viewport(ctx: &egui::Context) -> egui::Vec2 {
    let current = ctx.available_rect().size();
    let monitor = ctx.input(|input| input.viewport().monitor_size);
    match monitor {
        Some(monitor) if monitor.x.is_finite() && monitor.y.is_finite() => {
            egui::vec2(current.x.min(monitor.x), current.y.min(monitor.y))
        }
        _ => current,
    }
}

pub fn player_menu_window_height(viewport_height: f32, style: &PlayerMenuStyleConfig) -> f32 {
    let viewport_height = viewport_height.max(1.0);
    let viewport_cap = (viewport_height - 32.0).max(180.0);
    let fraction_cap = (viewport_height * style.panel_height_fraction).min(viewport_cap);
    fraction_cap.max(viewport_cap.min(180.0))
}

pub fn player_menu_content_height(viewport_height: f32, style: &PlayerMenuStyleConfig) -> f32 {
    (player_menu_window_height(viewport_height, style) - 132.0).max(96.0)
}

pub fn player_menu_action_button_size(
    available_width: f32,
    action: PlayerMenuAction,
    label: &str,
    style: &PlayerMenuStyleConfig,
) -> egui::Vec2 {
    let desired = menu_action_button_width(action, label).max(style.button_min_width);
    egui::vec2(
        desired.min(available_width.max(1.0)),
        style.button_height.max(1.0),
    )
}

pub fn player_menu_text_button_size(
    available_width: f32,
    label: &str,
    style: &PlayerMenuStyleConfig,
) -> egui::Vec2 {
    let desired = (label.chars().count() as f32 * 8.0 + 28.0).max(style.button_min_width);
    egui::vec2(
        desired.min(available_width.max(1.0)),
        style.button_height.max(1.0),
    )
}

pub fn player_menu_color(color: visual_novel_engine::PlayerMenuColor, alpha: u8) -> egui::Color32 {
    egui::Color32::from_rgba_premultiplied(color.r, color.g, color.b, alpha.min(color.a))
}

pub fn player_menu_action_text(
    action: PlayerMenuAction,
    label: &str,
    style: &PlayerMenuStyleConfig,
) -> egui::RichText {
    let color = match action {
        PlayerMenuAction::ResumeGame
        | PlayerMenuAction::OpenMenu
        | PlayerMenuAction::OpenSaves
        | PlayerMenuAction::OpenHistory
        | PlayerMenuAction::OpenRoutes
        | PlayerMenuAction::OpenSettings
        | PlayerMenuAction::OpenSystem => Some(player_menu_color(style.accent, 255)),
        PlayerMenuAction::RestartStory | PlayerMenuAction::ToggleFullscreen => {
            Some(player_menu_color(style.warning, 255))
        }
        PlayerMenuAction::QuitGame => Some(player_menu_color(style.danger, 255)),
        PlayerMenuAction::QuickSave
        | PlayerMenuAction::QuickLoad
        | PlayerMenuAction::ToggleHistoryWindow => None,
    };
    let text = egui::RichText::new(label.to_string());
    match color {
        Some(color) => text.color(color),
        None => text,
    }
}

pub fn player_menu_action_enabled(action: PlayerMenuAction, has_quicksave: bool) -> bool {
    !matches!(action, PlayerMenuAction::QuickLoad) || has_quicksave
}

pub fn player_text_panel_advance_enabled(menu: &PlayerMenuConfig, prefs: &UserPreferences) -> bool {
    prefs
        .advance_on_text_panel_click
        .unwrap_or(menu.advance_on_text_panel_click)
}

pub fn player_route_history_label(index: usize, entry: &ChoiceHistoryEntry) -> String {
    let prompt = entry.prompt.trim();
    let option = entry.option_text.trim();
    let prompt = if prompt.is_empty() { "Choice" } else { prompt };
    let option = if option.is_empty() {
        format!("Option {}", entry.option_index + 1)
    } else {
        option.to_string()
    };
    format!("{}. {prompt} -> {option}", index + 1)
}

pub(super) fn fit_rect_to_stage(bounds: egui::Rect, stage_size: (f32, f32)) -> egui::Rect {
    let size = player_stage_viewport_size(bounds.size(), stage_size);
    egui::Rect::from_center_size(bounds.center(), size)
}

pub(super) fn cover_rect(bounds: egui::Rect, source_size: egui::Vec2) -> egui::Rect {
    let source_w = source_size.x.max(1.0);
    let source_h = source_size.y.max(1.0);
    let source_aspect = source_w / source_h;
    let bounds_aspect = bounds.width().max(1.0) / bounds.height().max(1.0);
    let size = if source_aspect > bounds_aspect {
        egui::vec2(bounds.height() * source_aspect, bounds.height())
    } else {
        egui::vec2(bounds.width(), bounds.width() / source_aspect)
    };
    egui::Rect::from_center_size(bounds.center(), size)
}

fn menu_action_button_width(action: PlayerMenuAction, label: &str) -> f32 {
    match action {
        PlayerMenuAction::ResumeGame => 132.0,
        PlayerMenuAction::QuitGame => 84.0,
        PlayerMenuAction::QuickSave
        | PlayerMenuAction::QuickLoad
        | PlayerMenuAction::OpenSaves
        | PlayerMenuAction::OpenRoutes
        | PlayerMenuAction::OpenSettings
        | PlayerMenuAction::OpenSystem => 104.0,
        _ => (label.chars().count() as f32 * 8.0 + 28.0).clamp(84.0, 150.0),
    }
}

pub(super) fn player_save_root_for_script(prefs_path: &Path, script_id: &ScriptId) -> PathBuf {
    let base = prefs_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("saves").join(script_id_hex(script_id))
}

fn script_id_hex(script_id: &ScriptId) -> String {
    let mut out = String::with_capacity(script_id.len() * 2);
    for byte in script_id {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

pub(super) fn save_slot_summary(entry: &SaveSlotEntry) -> String {
    let kind = if entry.metadata.quick {
        "Quick"
    } else {
        "Manual"
    };
    let line = entry
        .metadata
        .summary_line
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("No dialogue yet");
    format!("{kind} | Progress {} | {line}", entry.metadata.position)
}
