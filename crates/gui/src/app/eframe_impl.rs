use super::*;

impl eframe::App for VnApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.config.player_menu.enabled
            && ctx.input(|i| i.key_pressed(egui::Key::Escape) || i.key_pressed(egui::Key::F1))
        {
            self.show_menu = !self.show_menu;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::F12)) {
            self.show_inspector = !self.show_inspector;
        }

        self.apply_preferences(ctx);
        self.advance_passthrough_events(ctx);

        egui::CentralPanel::default().show(ctx, |ui| {
            self.render_toolbar(ui);
            ui.separator();
            let status_height =
                player_status_bar_height(self.last_status.is_some(), self.last_error.is_some());
            self.render_player_stage(ui, ctx, status_height);
            self.render_status_bar(ui, status_height);
        });

        if self.config.player_menu.enabled && self.show_menu {
            self.render_player_menu(ctx);
        }

        self.render_history(ctx);
        self.render_inspector(ctx);
    }
}

impl VnApp {
    fn render_status_bar(&self, ui: &mut egui::Ui, height: f32) {
        if height <= 0.0 {
            return;
        }
        let (rect, _) = ui.allocate_exact_size(
            egui::vec2(ui.available_width(), height),
            egui::Sense::hover(),
        );
        ui.painter()
            .rect_filled(rect, 0.0, egui::Color32::from_rgb(14, 16, 22));
        ui.painter().line_segment(
            [rect.left_top(), rect.right_top()],
            egui::Stroke::new(1.0, egui::Color32::from_rgb(58, 64, 78)),
        );
        let content_rect = rect.shrink2(egui::vec2(10.0, PLAYER_STATUS_BAR_PADDING * 0.5));
        ui.allocate_ui_at_rect(content_rect, |ui| {
            ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                if let Some(message) = &self.last_status {
                    ui.add(
                        egui::Label::new(
                            egui::RichText::new(message)
                                .color(egui::Color32::from_rgb(130, 220, 160)),
                        )
                        .wrap(false),
                    );
                }
                if let Some(message) = &self.last_error {
                    ui.add(
                        egui::Label::new(egui::RichText::new(message).color(egui::Color32::RED))
                            .wrap(false),
                    );
                }
            });
        });
    }

    fn render_player_menu(&mut self, ctx: &egui::Context) {
        if self.config.player_menu.tab_label(self.menu_tab).is_none() {
            self.menu_tab = self.config.player_menu.initial_tab();
        }
        let mut open = self.show_menu;
        let viewport = player_menu_reference_viewport(ctx);
        let menu_width = player_menu_window_width(viewport.x, &self.config.player_menu.style);
        let menu_height = player_menu_window_height(viewport.y, &self.config.player_menu.style);
        let max_menu_height = (viewport.y - 24.0).max(180.0);
        let mut frame = egui::Frame::window(&ctx.style());
        frame.fill = player_menu_color(
            self.config.player_menu.style.background,
            self.config.player_menu.style.panel_alpha,
        );
        egui::Window::new(self.config.player_menu.title.clone())
            .open(&mut open)
            .default_width(menu_width)
            .default_height(menu_height)
            .max_width((viewport.x - 24.0).max(240.0))
            .min_height(180.0)
            .max_height(max_menu_height)
            .anchor(
                menu_anchor(self.config.player_menu.layout.panel_anchor),
                egui::Vec2::ZERO,
            )
            .frame(frame)
            .resizable(true)
            .collapsible(false)
            .show(ctx, |ui| {
                ui.set_max_width(menu_width);
                if self.config.player_menu.layout.quick_action_placement
                    == PlayerMenuQuickActionPlacement::MenuHeader
                {
                    ui.horizontal_wrapped(|ui| self.render_quick_action_buttons(ui));
                    ui.separator();
                }

                match self.config.player_menu.layout.tabs_position {
                    PlayerMenuTabsPosition::Top => {
                        self.render_menu_tabs(ui);
                        ui.separator();
                        self.render_scrollable_active_menu_tab(ui, ctx);
                    }
                    PlayerMenuTabsPosition::Left => {
                        ui.horizontal(|ui| {
                            ui.vertical(|ui| self.render_menu_tabs(ui));
                            ui.separator();
                            ui.vertical(|ui| self.render_scrollable_active_menu_tab(ui, ctx));
                        });
                    }
                }
            });
        self.show_menu = player_menu_visibility_after_window(open, self.show_menu);
    }

    fn render_menu_tabs(&mut self, ui: &mut egui::Ui) {
        let tabs = self.config.player_menu.tabs.clone();
        match self.config.player_menu.layout.tabs_position {
            PlayerMenuTabsPosition::Top => {
                ui.horizontal_wrapped(|ui| {
                    for tab in tabs {
                        if tab.visible {
                            self.menu_tab_button(ui, tab.kind, &tab.label);
                        }
                    }
                });
            }
            PlayerMenuTabsPosition::Left => {
                for tab in tabs {
                    if tab.visible {
                        self.menu_tab_button(ui, tab.kind, &tab.label);
                    }
                }
            }
        }
    }

    fn menu_tab_button(&mut self, ui: &mut egui::Ui, tab: PlayerMenuTabKind, label: &str) {
        let selected = self.menu_tab == tab;
        let text = if selected {
            egui::RichText::new(label)
                .color(player_menu_color(self.config.player_menu.style.accent, 255))
        } else {
            egui::RichText::new(label)
        };
        if ui.selectable_label(selected, text).clicked() {
            self.menu_tab = tab;
        }
    }

    fn render_active_menu_tab(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        match self.menu_tab {
            PlayerMenuTabKind::Saves => self.render_saves_menu(ui),
            PlayerMenuTabKind::History => self.render_history_menu(ui),
            PlayerMenuTabKind::Routes => self.render_routes_menu(ui),
            PlayerMenuTabKind::Settings => self.render_settings_menu(ui),
            PlayerMenuTabKind::System => self.render_system_menu(ui, ctx),
        }
    }

    fn render_scrollable_active_menu_tab(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let viewport = player_menu_reference_viewport(ctx);
        let max_height = player_menu_content_height(viewport.y, &self.config.player_menu.style);
        egui::ScrollArea::vertical()
            .id_source("player_menu_active_tab_scroll")
            .max_height(max_height)
            .auto_shrink([false, false])
            .show(ui, |ui| self.render_active_menu_tab(ui, ctx));
    }

    fn render_saves_menu(&mut self, ui: &mut egui::Ui) {
        let style = self.config.player_menu.style.clone();
        let slots = self.list_save_slots_for_menu();
        let has_quicksave = slots.iter().any(|entry| entry.metadata.quick);
        ui.horizontal_wrapped(|ui| {
            if ui
                .add_sized(
                    player_menu_text_button_size(ui.available_width(), "Quick Save", &style),
                    egui::Button::new("Quick Save").rounding(style.button_corner_radius),
                )
                .clicked()
            {
                self.quicksave();
            }
            let quick_load = ui
                .add_enabled_ui(has_quicksave, |ui| {
                    ui.add_sized(
                        player_menu_text_button_size(ui.available_width(), "Quick Load", &style),
                        egui::Button::new("Quick Load").rounding(style.button_corner_radius),
                    )
                })
                .inner;
            if quick_load.clicked() {
                self.quickload();
            }
        });
        ui.add_space(8.0);
        for slot_id in 1..=self.config.player_menu.save_slots {
            let summary = slots
                .iter()
                .find(|entry| !entry.metadata.quick && entry.metadata.slot_id == slot_id);
            let summary = summary.map(save_slot_summary);
            let has_entry = summary.is_some();
            ui.group(|ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label(format!("Slot {slot_id}"));
                    if let Some(summary) = &summary {
                        ui.label(summary);
                    } else {
                        ui.label("Empty");
                    }
                    if ui
                        .add_sized(
                            player_menu_text_button_size(ui.available_width(), "Save", &style),
                            egui::Button::new("Save").rounding(style.button_corner_radius),
                        )
                        .clicked()
                    {
                        self.save_slot(slot_id);
                    }
                    let load = ui
                        .add_enabled_ui(has_entry, |ui| {
                            ui.add_sized(
                                player_menu_text_button_size(ui.available_width(), "Load", &style),
                                egui::Button::new("Load").rounding(style.button_corner_radius),
                            )
                        })
                        .inner;
                    if load.clicked() {
                        self.load_slot(slot_id);
                    }
                });
            });
        }
    }

    fn render_history_menu(&self, ui: &mut egui::Ui) {
        if self.engine.state().history.is_empty() {
            ui.label("No dialogue history yet.");
            return;
        }
        egui::ScrollArea::vertical()
            .max_height(360.0)
            .show(ui, |ui| {
                for entry in &self.engine.state().history {
                    ui.label(format!("{}: {}", entry.speaker, entry.text));
                    ui.separator();
                }
            });
    }

    fn render_routes_menu(&self, ui: &mut egui::Ui) {
        if self.engine.choice_history().is_empty() {
            ui.label("No choices selected in this run yet.");
            return;
        }
        egui::ScrollArea::vertical()
            .max_height(360.0)
            .show(ui, |ui| {
                for (idx, entry) in self.engine.choice_history().iter().enumerate() {
                    ui.colored_label(
                        egui::Color32::from_rgb(235, 238, 245),
                        player_route_history_label(idx, entry),
                    );
                }
            });
    }

    fn render_settings_menu(&mut self, ui: &mut egui::Ui) {
        let mut dirty = false;
        let mut audio_dirty = false;
        dirty |= ui
            .checkbox(&mut self.prefs.fullscreen, "Fullscreen")
            .changed();
        dirty |= ui
            .checkbox(&mut self.prefs.vsync, "VSync (restart required)")
            .changed();
        dirty |= ui
            .add(egui::Slider::new(&mut self.prefs.ui_scale, 0.75..=2.0).text("UI Scale"))
            .changed();
        let mut advance_on_text_panel =
            player_text_panel_advance_enabled(&self.config.player_menu, &self.prefs);
        if ui
            .checkbox(&mut advance_on_text_panel, "Text panel advances")
            .changed()
        {
            self.prefs.advance_on_text_panel_click = Some(advance_on_text_panel);
            dirty = true;
        }
        if self.prefs.advance_on_text_panel_click.is_some()
            && ui.button("Use author default").clicked()
        {
            self.prefs.advance_on_text_panel_click = None;
            dirty = true;
        }
        ui.separator();
        ui.heading("Audio");
        audio_dirty |= ui.checkbox(&mut self.prefs.audio_muted, "Mute").changed();
        audio_dirty |= ui
            .add(egui::Slider::new(&mut self.prefs.master_volume, 0.0..=1.0).text("Master"))
            .changed();
        audio_dirty |= ui
            .add(egui::Slider::new(&mut self.prefs.bgm_volume, 0.0..=1.0).text("BGM"))
            .changed();
        audio_dirty |= ui
            .add(egui::Slider::new(&mut self.prefs.sfx_volume, 0.0..=1.0).text("SFX"))
            .changed();
        audio_dirty |= ui
            .add(egui::Slider::new(&mut self.prefs.voice_volume, 0.0..=1.0).text("Voice"))
            .changed();
        if audio_dirty {
            self.apply_audio_preferences();
            dirty = true;
        }
        ui.add_space(8.0);
        self.render_audio_status(ui);
        ui.separator();
        ui.add(
            egui::Label::new(format!("Save folder: {}", self.save_store.root().display()))
                .wrap(true),
        );
        if dirty {
            self.persist_preferences();
        }
    }

    fn render_audio_status(&self, ui: &mut egui::Ui) {
        let snapshot = self.audio.snapshot();
        ui.label(if snapshot.backend_silent {
            "Backend: silent"
        } else {
            "Backend: audio output"
        });
        ui.label(format!(
            "Mix: master {:.0}%{}",
            snapshot.mix.master * 100.0,
            if snapshot.mix.muted { " (muted)" } else { "" }
        ));
        render_audio_channel_row(ui, "BGM", &snapshot.bgm);
        render_audio_channel_row(ui, "SFX", &snapshot.sfx);
        render_audio_channel_row(ui, "Voice", &snapshot.voice);
        if let Some(last_event) = self.audio.last_event() {
            ui.label(format!("Audio: {last_event}"));
        }
        if let Some(last_warning) = self.audio.last_warning() {
            ui.colored_label(
                egui::Color32::YELLOW,
                format!("Audio warning: {last_warning}"),
            );
        }
    }

    fn render_system_menu(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let style = self.config.player_menu.style.clone();
        if ui
            .add_sized(
                player_menu_text_button_size(ui.available_width(), "Restart Story", &style),
                egui::Button::new(
                    egui::RichText::new("Restart Story")
                        .color(player_menu_color(style.warning, 255)),
                )
                .rounding(style.button_corner_radius),
            )
            .clicked()
        {
            self.execute_menu_action(PlayerMenuAction::RestartStory, Some(ctx));
        }
        if ui
            .add_sized(
                player_menu_text_button_size(ui.available_width(), "Open History Window", &style),
                egui::Button::new("Open History Window").rounding(style.button_corner_radius),
            )
            .clicked()
        {
            self.execute_menu_action(PlayerMenuAction::ToggleHistoryWindow, Some(ctx));
        }
        if ui
            .add_sized(
                player_menu_text_button_size(ui.available_width(), "Inspector", &style),
                egui::Button::new("Inspector").rounding(style.button_corner_radius),
            )
            .clicked()
        {
            self.show_inspector = !self.show_inspector;
        }
        ui.separator();
        if ui
            .add_sized(
                player_menu_text_button_size(ui.available_width(), "Close Game", &style),
                egui::Button::new(
                    egui::RichText::new("Close Game").color(player_menu_color(style.danger, 255)),
                )
                .rounding(style.button_corner_radius),
            )
            .clicked()
        {
            self.execute_menu_action(PlayerMenuAction::QuitGame, Some(ctx));
        }
    }
}

fn menu_anchor(anchor: PlayerMenuPanelAnchor) -> egui::Align2 {
    match anchor {
        PlayerMenuPanelAnchor::Center => egui::Align2::CENTER_CENTER,
        PlayerMenuPanelAnchor::TopLeft => egui::Align2::LEFT_TOP,
        PlayerMenuPanelAnchor::TopRight => egui::Align2::RIGHT_TOP,
        PlayerMenuPanelAnchor::BottomLeft => egui::Align2::LEFT_BOTTOM,
        PlayerMenuPanelAnchor::BottomRight => egui::Align2::RIGHT_BOTTOM,
    }
}

fn render_audio_channel_row(ui: &mut egui::Ui, label: &str, state: &PlayerAudioChannelState) {
    ui.horizontal_wrapped(|ui| {
        ui.label(label);
        let path = state.path.as_deref().unwrap_or("idle");
        ui.label(path);
        ui.label(if state.active { "active" } else { "idle" });
        ui.add(
            egui::ProgressBar::new(state.effective_volume)
                .desired_width(100.0)
                .text(format!("{:.0}%", state.effective_volume * 100.0)),
        );
        if state.active {
            ui.label(format!("source {:.0}%", state.command_volume * 100.0));
        }
    });
}

fn player_menu_visibility_after_window(window_open: bool, requested_open: bool) -> bool {
    window_open && requested_open
}

#[cfg(test)]
mod tests {
    use super::player_menu_visibility_after_window;

    #[test]
    fn menu_action_can_close_window_during_render() {
        assert!(!player_menu_visibility_after_window(true, false));
    }

    #[test]
    fn window_close_button_can_close_menu() {
        assert!(!player_menu_visibility_after_window(false, true));
    }
}
