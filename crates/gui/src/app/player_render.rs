use super::*;

impl VnApp {
    pub(super) fn render_toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            ui.heading(&self.config.title);
            ui.separator();
            if self.config.player_menu.enabled
                && self.config.player_menu.layout.quick_action_placement
                    == PlayerMenuQuickActionPlacement::Toolbar
            {
                self.render_quick_action_buttons(ui);
            }
            if let Some(last_warning) = self.audio.last_warning() {
                ui.separator();
                ui.colored_label(
                    egui::Color32::YELLOW,
                    format!("Audio warning: {last_warning}"),
                );
            }
        });
    }

    pub(super) fn render_quick_action_buttons(&mut self, ui: &mut egui::Ui) {
        let style = self.config.player_menu.style.clone();
        let quick_actions = self.config.player_menu.quick_actions.clone();
        let has_quicksave = if quick_actions
            .iter()
            .any(|action| action.visible && action.action == PlayerMenuAction::QuickLoad)
        {
            match self.save_store.has_quicksave() {
                Ok(has_quicksave) => has_quicksave,
                Err(err) => {
                    self.last_error = Some(format!("Quick load availability check failed: {err}"));
                    false
                }
            }
        } else {
            false
        };
        for action in quick_actions {
            if !action.visible {
                continue;
            }
            let size = player_menu_action_button_size(
                ui.available_width(),
                action.action,
                &action.label,
                &style,
            );
            let response = ui
                .add_enabled_ui(
                    player_menu_action_enabled(action.action, has_quicksave),
                    |ui| {
                        ui.add_sized(
                            size,
                            egui::Button::new(player_menu_action_text(
                                action.action,
                                &action.label,
                                &style,
                            ))
                            .rounding(style.button_corner_radius),
                        )
                    },
                )
                .inner;
            if response.clicked() {
                self.execute_menu_action(action.action, None);
            }
        }
    }

    pub(super) fn render_player_stage(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &egui::Context,
        reserved_status_height: f32,
    ) {
        let event = self.engine.current_event();
        let visual = self.preview_visual_state(event.as_ref().ok());
        let available = player_stage_available_size(ui.available_size(), reserved_status_height);
        let viewport = player_stage_viewport_size(available, DEFAULT_STAGE_SIZE);
        let (viewport_rect, _) = ui.allocate_exact_size(viewport, egui::Sense::hover());
        let stage_rect = fit_rect_to_stage(viewport_rect, DEFAULT_STAGE_SIZE);

        let painter = ui.painter().with_clip_rect(stage_rect);
        painter.rect_filled(stage_rect, 0.0, egui::Color32::from_rgb(16, 18, 24));
        self.paint_background(ui, stage_rect, visual.background.as_deref());
        self.paint_character_labels(ui, stage_rect, &visual);

        match event {
            Ok(event) => self.render_event_overlay(ui, ctx, stage_rect, event),
            Err(VnError::EndOfScript) => self.render_end_overlay(ui, stage_rect),
            Err(err) => self.last_error = Some(err.to_string()),
        }
    }

    fn paint_background(
        &mut self,
        ui: &mut egui::Ui,
        stage_rect: egui::Rect,
        background: Option<&str>,
    ) {
        let Some(background) = background else {
            return;
        };
        match self.assets.texture_for_asset(ui.ctx(), background) {
            Ok(Some(texture)) => {
                let image_rect = cover_rect(stage_rect, texture.size_vec2());
                let painter = ui.painter().with_clip_rect(stage_rect);
                painter.image(
                    texture.id(),
                    image_rect,
                    egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
                    egui::Color32::WHITE,
                );
            }
            Ok(None) => {}
            Err(err) => self.last_error = Some(format!("Asset error: {err}")),
        }
    }

    fn paint_character_labels(&self, ui: &egui::Ui, stage_rect: egui::Rect, visual: &VisualState) {
        if visual.characters.is_empty() {
            return;
        }
        let painter = ui.painter().with_clip_rect(stage_rect);
        let mut x = stage_rect.left() + 18.0;
        let y = stage_rect.top() + 18.0;
        for character in &visual.characters {
            let text = character
                .expression
                .as_ref()
                .map(|expression| format!("{} ({})", character.name, expression))
                .unwrap_or_else(|| character.name.as_ref().to_string());
            let galley = ui.painter().layout_no_wrap(
                text,
                egui::FontId::proportional(15.0),
                egui::Color32::WHITE,
            );
            let rect =
                egui::Rect::from_min_size(egui::pos2(x, y), galley.size() + egui::vec2(20.0, 10.0));
            painter.rect_filled(
                rect,
                5.0,
                egui::Color32::from_rgba_premultiplied(8, 10, 16, 190),
            );
            painter.galley(
                rect.min + egui::vec2(10.0, 5.0),
                galley,
                egui::Color32::WHITE,
            );
            x = rect.right() + 8.0;
        }
    }

    fn render_event_overlay(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &egui::Context,
        stage_rect: egui::Rect,
        event: EventCompiled,
    ) {
        let view = UiState::from_event(&event, self.engine.visual_state()).view;
        match view {
            UiView::Dialogue { speaker, text } => {
                self.render_dialogue_overlay(ui, ctx, stage_rect, &speaker, &text);
            }
            UiView::Choice { prompt, options } => {
                self.render_choice_overlay(ui, stage_rect, &prompt, &options);
            }
            UiView::Scene { .. } => {
                self.render_scene_overlay(ui, stage_rect, "Scene updated");
            }
            UiView::System { message } => {
                self.render_scene_overlay(ui, stage_rect, &message);
            }
        }
    }

    fn render_dialogue_overlay(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &egui::Context,
        stage_rect: egui::Rect,
        speaker: &str,
        text: &str,
    ) {
        let layout = crate::player_overlay::dialogue_overlay_layout(stage_rect);
        let rect = layout.panel;
        ui.painter().rect_filled(
            rect,
            layout.corner_radius,
            egui::Color32::from_rgba_premultiplied(8, 8, 14, 225),
        );
        ui.painter().rect_stroke(
            rect,
            layout.corner_radius,
            egui::Stroke::new(1.0, egui::Color32::from_gray(130)),
        );

        let mut should_advance = false;
        ui.allocate_ui_at_rect(rect.shrink2(layout.content_padding), |ui| {
            ui.set_clip_rect(rect.shrink(layout.clip_padding));
            ui.label(
                egui::RichText::new(speaker)
                    .color(egui::Color32::from_rgb(185, 214, 255))
                    .size(layout.speaker_font_size)
                    .strong(),
            );
            ui.add_space(layout.speaker_text_gap);
            let text_height = (rect.height()
                - layout.content_padding.y * 2.0
                - layout.speaker_height
                - layout.speaker_text_gap
                - layout.control_height)
                .max(12.0 * layout.scale);
            ui.add_sized(
                [ui.available_width(), text_height],
                egui::Label::new(
                    egui::RichText::new(text)
                        .color(egui::Color32::WHITE)
                        .size(layout.text_font_size),
                )
                .wrap(true),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add_sized(
                        [96.0 * layout.scale, layout.control_height],
                        egui::Button::new(
                            egui::RichText::new("Continue").size(layout.text_font_size),
                        ),
                    )
                    .clicked()
                {
                    should_advance = true;
                }
            });
        });
        if player_text_panel_advance_enabled(&self.config.player_menu, &self.prefs)
            && ui
                .interact(
                    rect,
                    egui::Id::new(("standalone_dialogue_overlay", self.engine.state().position)),
                    egui::Sense::click(),
                )
                .clicked()
        {
            should_advance = true;
        }
        if should_advance {
            self.advance();
            ctx.request_repaint();
        }
    }

    fn render_choice_overlay(
        &mut self,
        ui: &mut egui::Ui,
        stage_rect: egui::Rect,
        prompt: &str,
        options: &[String],
    ) {
        let layout = crate::player_overlay::choice_overlay_layout(stage_rect, prompt, options);
        let corner_radius = 6.0 * layout.scale;
        ui.painter().rect_filled(
            layout.panel,
            corner_radius,
            egui::Color32::from_rgba_premultiplied(10, 12, 18, 232),
        );
        ui.painter().rect_stroke(
            layout.panel,
            corner_radius,
            egui::Stroke::new(1.0, egui::Color32::from_gray(120)),
        );

        let mut selected = None;
        ui.allocate_ui_at_rect(layout.panel.shrink2(layout.content_padding), |ui| {
            ui.set_clip_rect(layout.panel.shrink(layout.clip_padding));
            ui.add_sized(
                [ui.available_width(), layout.prompt_height],
                egui::Label::new(
                    egui::RichText::new(crate::player_overlay::soft_wrap_long_tokens(prompt, 28))
                        .color(egui::Color32::WHITE)
                        .strong()
                        .size(layout.prompt_font_size),
                )
                .wrap(true),
            );
            ui.add_space(layout.prompt_option_gap);
            egui::ScrollArea::vertical()
                .id_source("standalone_player_choice_overlay_scroll")
                .max_height(layout.options_viewport_height)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    for (idx, option) in options.iter().enumerate() {
                        let row_height = layout.option_heights.get(idx).copied().unwrap_or(38.0);
                        if ui
                            .add_sized(
                                [ui.available_width(), row_height],
                                egui::Button::new(
                                    egui::RichText::new(
                                        crate::player_overlay::soft_wrap_long_tokens(option, 32),
                                    )
                                    .size(layout.option_font_size),
                                ),
                            )
                            .clicked()
                        {
                            selected = Some(idx);
                        }
                        ui.add_space(layout.option_gap);
                    }
                });
        });
        if let Some(index) = selected {
            self.choose(index);
        }
    }

    fn render_scene_overlay(
        &mut self,
        ui: &mut egui::Ui,
        stage_rect: egui::Rect,
        description: &str,
    ) {
        let rect = crate::player_overlay::scene_overlay_rect(stage_rect);
        ui.painter().rect_filled(
            rect,
            6.0,
            egui::Color32::from_rgba_premultiplied(8, 8, 14, 215),
        );
        let mut should_advance = false;
        ui.allocate_ui_at_rect(rect.shrink2(egui::vec2(14.0, 10.0)), |ui| {
            ui.set_clip_rect(rect.shrink(8.0));
            ui.add_sized(
                [ui.available_width(), 34.0],
                egui::Label::new(
                    egui::RichText::new(crate::player_overlay::soft_wrap_long_tokens(
                        description,
                        42,
                    ))
                    .color(egui::Color32::WHITE),
                )
                .wrap(true),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("Continue").clicked() {
                    should_advance = true;
                }
            });
        });
        if player_text_panel_advance_enabled(&self.config.player_menu, &self.prefs)
            && ui
                .interact(
                    rect,
                    egui::Id::new("standalone_player_scene_overlay"),
                    egui::Sense::click(),
                )
                .clicked()
        {
            should_advance = true;
        }
        if should_advance {
            self.advance();
        }
    }

    fn render_end_overlay(&mut self, ui: &mut egui::Ui, stage_rect: egui::Rect) {
        let rect = egui::Rect::from_center_size(stage_rect.center(), egui::vec2(320.0, 150.0));
        ui.painter().rect_filled(
            rect,
            6.0,
            egui::Color32::from_rgba_premultiplied(18, 14, 24, 230),
        );
        ui.allocate_ui_at_rect(rect.shrink(18.0), |ui| {
            ui.vertical_centered(|ui| {
                ui.heading("The End");
                ui.add_space(16.0);
                if ui.button("Play Again").clicked() {
                    if let Err(err) = self.engine.jump_to_label("start") {
                        self.last_error = Some(err.to_string());
                    } else {
                        let audio = self.engine.take_audio_commands();
                        self.apply_audio_commands(audio);
                        self.last_status = Some("Restarted".to_string());
                    }
                }
            });
        });
    }

    fn preview_visual_state(&self, event: Option<&EventCompiled>) -> VisualState {
        let mut visual = self.engine.visual_state().clone();
        match event {
            Some(EventCompiled::Scene(scene)) => visual.apply_scene(scene),
            Some(EventCompiled::Patch(patch)) => visual.apply_patch(patch),
            _ => {}
        }
        visual
    }

    pub(super) fn advance_passthrough_events(&mut self, ctx: &egui::Context) {
        for _ in 0..PASSTHROUGH_EVENT_LIMIT {
            let event = match self.engine.current_event() {
                Ok(event) => event,
                Err(VnError::EndOfScript) => return,
                Err(err) => {
                    self.last_error = Some(err.to_string());
                    return;
                }
            };

            match event {
                EventCompiled::Jump { .. }
                | EventCompiled::SetFlag { .. }
                | EventCompiled::SetVar { .. }
                | EventCompiled::JumpIf { .. }
                | EventCompiled::Scene(_)
                | EventCompiled::Patch(_)
                | EventCompiled::AudioAction(_)
                | EventCompiled::SetCharacterPosition(_) => self.advance(),
                EventCompiled::ExtCall { .. } => match self.engine.resume() {
                    Ok(()) => {
                        let audio = self.engine.take_audio_commands();
                        self.apply_audio_commands(audio);
                    }
                    Err(err) => {
                        self.last_error = Some(err.to_string());
                        return;
                    }
                },
                EventCompiled::Dialogue(_)
                | EventCompiled::Choice(_)
                | EventCompiled::Transition(_) => return,
            }
            ctx.request_repaint_after(std::time::Duration::from_millis(16));
        }
        self.last_error = Some("Stopped auto-advancing after too many system events".to_string());
    }

    pub(super) fn render_history(&self, ctx: &egui::Context) {
        if !self.show_history {
            return;
        }
        egui::Window::new("History").show(ctx, |ui| {
            for entry in &self.engine.state().history {
                ui.label(format!("{}: {}", entry.speaker, entry.text));
                ui.separator();
            }
        });
    }

    pub(super) fn render_inspector(&mut self, ctx: &egui::Context) {
        if !self.show_inspector {
            return;
        }
        let event_summary = match self.engine.current_event() {
            Ok(event) => event_kind(&event),
            Err(err) => format!("Error: {err}"),
        };
        let history_bytes = history_bytes(&self.engine.state().history);
        let dt = ctx.input(|i| i.unstable_dt);
        let fps = if dt > 0.0 { 1.0 / dt } else { 0.0 };
        let cache_stats = self.assets.stats();
        egui::Window::new("Inspector").show(ctx, |ui| {
            ui.label(format!("IP: {}", self.engine.state().position));
            ui.label(format!("Event: {event_summary}"));
            ui.label(format!("FPS: {:.1}", fps));
            ui.label(format!("History bytes (approx): {}", history_bytes));
            ui.label(format!(
                "Texture cache: {} entries, {} MB (budget {} MB)",
                cache_stats.entries,
                cache_stats.bytes / (1024 * 1024),
                cache_stats.budget_bytes / (1024 * 1024)
            ));
            ui.label(format!(
                "Cache hits: {}, misses: {}, evictions: {}",
                cache_stats.hits, cache_stats.misses, cache_stats.evictions
            ));
            ui.separator();
            ui.label("Flags:");
            let flag_count = self.engine.flag_count();
            for flag_id in 0..flag_count {
                let mut value = self.engine.state().get_flag(flag_id);
                if ui.checkbox(&mut value, format!("flag {flag_id}")).changed() {
                    self.engine.set_flag(flag_id, value);
                }
            }
            ui.separator();
            ui.label("Jump to label:");
            ui.text_edit_singleline(&mut self.label_jump_input);
            if ui.button("Jump").clicked() {
                if let Err(err) = self.engine.jump_to_label(&self.label_jump_input) {
                    self.last_error = Some(err.to_string());
                }
            }
            ui.separator();
            ui.label("Available labels:");
            for label in self.engine.labels().keys() {
                ui.label(label);
            }
        });
    }

    fn advance(&mut self) {
        match self.engine.step() {
            Ok((audio, _change)) => self.apply_audio_commands(audio),
            Err(VnError::EndOfScript) => {}
            Err(err) => self.last_error = Some(err.to_string()),
        }
    }

    fn choose(&mut self, index: usize) {
        match self.engine.choose(index) {
            Ok(_) => {
                let audio = self.engine.take_audio_commands();
                self.apply_audio_commands(audio);
            }
            Err(VnError::EndOfScript) => {}
            Err(err) => self.last_error = Some(err.to_string()),
        }
    }
}
