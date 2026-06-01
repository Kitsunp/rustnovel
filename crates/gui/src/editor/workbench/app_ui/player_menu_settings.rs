use super::*;

impl EditorWorkbench {
    pub(super) fn render_player_menu_settings_window(&mut self, ctx: &egui::Context) {
        if !self.show_player_menu_settings {
            return;
        }

        let mut open = self.show_player_menu_settings;
        let mut changed = false;
        let mut reset_defaults = false;
        egui::Window::new("Player Menu Settings")
            .open(&mut open)
            .default_width(520.0)
            .resizable(true)
            .show(ctx, |ui| {
                let Some(manifest) = self.manifest.as_mut() else {
                    ui.label("Open a project manifest before editing the player menu.");
                    return;
                };
                let menu = &mut manifest.settings.player_menu;

                changed |= ui.checkbox(&mut menu.enabled, "Enabled").changed();
                changed |= ui
                    .checkbox(&mut menu.open_on_start, "Open on start")
                    .changed();
                changed |= ui
                    .checkbox(&mut menu.advance_on_text_panel_click, "Text panel advances")
                    .changed();
                ui.horizontal_wrapped(|ui| {
                    ui.label("Title");
                    changed |= ui.text_edit_singleline(&mut menu.title).changed();
                });
                changed |= ui
                    .add(egui::Slider::new(&mut menu.save_slots, 1..=24).text("Save slots"))
                    .changed();

                ui.separator();
                ui.collapsing("Layout", |ui| {
                    changed |= combo_value(
                        ui,
                        "Panel anchor",
                        &mut menu.layout.panel_anchor,
                        &[
                            visual_novel_engine::PlayerMenuPanelAnchor::Center,
                            visual_novel_engine::PlayerMenuPanelAnchor::TopLeft,
                            visual_novel_engine::PlayerMenuPanelAnchor::TopRight,
                            visual_novel_engine::PlayerMenuPanelAnchor::BottomLeft,
                            visual_novel_engine::PlayerMenuPanelAnchor::BottomRight,
                        ],
                    );
                    changed |= combo_value(
                        ui,
                        "Quick actions",
                        &mut menu.layout.quick_action_placement,
                        &[
                            visual_novel_engine::PlayerMenuQuickActionPlacement::MenuHeader,
                            visual_novel_engine::PlayerMenuQuickActionPlacement::Toolbar,
                            visual_novel_engine::PlayerMenuQuickActionPlacement::Hidden,
                        ],
                    );
                    changed |= combo_value(
                        ui,
                        "Tabs",
                        &mut menu.layout.tabs_position,
                        &[
                            visual_novel_engine::PlayerMenuTabsPosition::Left,
                            visual_novel_engine::PlayerMenuTabsPosition::Top,
                        ],
                    );
                });

                ui.collapsing("Responsive Style", |ui| {
                    changed |= ui
                        .add(
                            egui::Slider::new(&mut menu.style.panel_width, 320.0..=1280.0)
                                .text("Panel width"),
                        )
                        .changed();
                    changed |= ui
                        .add(
                            egui::Slider::new(&mut menu.style.panel_width_fraction, 0.25..=1.0)
                                .text("Viewport width"),
                        )
                        .changed();
                    changed |= ui
                        .add(
                            egui::Slider::new(&mut menu.style.panel_height_fraction, 0.25..=0.95)
                                .text("Viewport height"),
                        )
                        .changed();
                    changed |= ui
                        .add(
                            egui::Slider::new(&mut menu.style.button_min_width, 48.0..=240.0)
                                .text("Button min width"),
                        )
                        .changed();
                    changed |= ui
                        .add(
                            egui::Slider::new(&mut menu.style.button_height, 24.0..=64.0)
                                .text("Button height"),
                        )
                        .changed();
                    changed |= ui
                        .add(
                            egui::Slider::new(&mut menu.style.button_corner_radius, 0.0..=32.0)
                                .text("Button corner radius"),
                        )
                        .changed();
                    changed |= ui
                        .add(
                            egui::Slider::new(&mut menu.style.panel_alpha, 120..=255)
                                .text("Panel alpha"),
                        )
                        .changed();
                });

                ui.collapsing("Color Palette", |ui| {
                    changed |= super::theme_controls::player_menu_color_control(
                        ui,
                        "Background",
                        &mut menu.style.background,
                    );
                    changed |= super::theme_controls::player_menu_color_control(
                        ui,
                        "Accent",
                        &mut menu.style.accent,
                    );
                    changed |= super::theme_controls::player_menu_color_control(
                        ui,
                        "Warning",
                        &mut menu.style.warning,
                    );
                    changed |= super::theme_controls::player_menu_color_control(
                        ui,
                        "Danger",
                        &mut menu.style.danger,
                    );
                });

                ui.collapsing("Quick Action Buttons", |ui| {
                    egui::ScrollArea::vertical()
                        .id_source("player_menu_quick_actions_scroll")
                        .max_height(180.0)
                        .show(ui, |ui| {
                            for action in &mut menu.quick_actions {
                                ui.horizontal_wrapped(|ui| {
                                    ui.label(format!("{:?}", action.action));
                                    changed |=
                                        ui.checkbox(&mut action.visible, "Visible").changed();
                                    changed |= ui.text_edit_singleline(&mut action.label).changed();
                                });
                            }
                        });
                });

                ui.collapsing("Tabs", |ui| {
                    for tab in &mut menu.tabs {
                        ui.horizontal_wrapped(|ui| {
                            ui.label(format!("{:?}", tab.kind));
                            changed |= ui.checkbox(&mut tab.visible, "Visible").changed();
                            changed |= ui.text_edit_singleline(&mut tab.label).changed();
                        });
                    }
                });

                ui.separator();
                if ui.button("Reset Player Menu Defaults").clicked() {
                    reset_defaults = true;
                    changed = true;
                }
            });

        self.show_player_menu_settings = open;
        if reset_defaults {
            if let Some(manifest) = self.manifest.as_mut() {
                manifest.settings.player_menu = visual_novel_engine::PlayerMenuConfig::default();
            }
        }
        if changed {
            self.persist_player_menu_settings();
        }
    }

    fn persist_player_menu_settings(&mut self) {
        let Some(manifest) = self.manifest.as_mut() else {
            return;
        };
        manifest.settings.player_menu = manifest.settings.player_menu.normalized();
        let Some(path) = self.manifest_path.clone() else {
            self.toast = Some(ToastState::warning(
                "Player menu changed in memory; no manifest path is loaded",
            ));
            return;
        };
        match manifest.save(&path) {
            Ok(()) => {
                self.toast = Some(ToastState::success("Player menu settings saved"));
            }
            Err(err) => {
                self.toast = Some(ToastState::error(format!(
                    "Player menu settings save failed: {err}"
                )));
            }
        }
    }
}

fn combo_value<T>(ui: &mut egui::Ui, label: &str, current: &mut T, values: &[T]) -> bool
where
    T: Copy + std::fmt::Debug + PartialEq,
{
    let before = *current;
    egui::ComboBox::from_label(label)
        .selected_text(format!("{:?}", current))
        .show_ui(ui, |ui| {
            for value in values {
                ui.selectable_value(current, *value, format!("{:?}", value));
            }
        });
    before != *current
}
