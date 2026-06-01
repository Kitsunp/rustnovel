use super::*;

impl VnApp {
    pub(super) fn apply_audio_commands(
        &mut self,
        commands: Vec<visual_novel_engine::runtime::AudioCommand>,
    ) {
        if let Err(err) = self.audio.apply_commands(commands) {
            self.last_error = Some(err);
        }
    }

    fn audio_mix_from_preferences(&self) -> PlayerAudioMix {
        PlayerAudioMix {
            master: self.prefs.master_volume,
            bgm: self.prefs.bgm_volume,
            sfx: self.prefs.sfx_volume,
            voice: self.prefs.voice_volume,
            muted: self.prefs.audio_muted,
        }
    }

    pub(super) fn apply_audio_preferences(&mut self) {
        self.audio.set_mix(self.audio_mix_from_preferences());
    }

    pub(super) fn execute_menu_action(
        &mut self,
        action: PlayerMenuAction,
        ctx: Option<&egui::Context>,
    ) {
        match action {
            PlayerMenuAction::ResumeGame => self.show_menu = false,
            PlayerMenuAction::OpenMenu => self.show_menu = true,
            PlayerMenuAction::QuickSave => self.quicksave(),
            PlayerMenuAction::QuickLoad => self.quickload(),
            PlayerMenuAction::OpenSaves => self.open_menu_tab(PlayerMenuTabKind::Saves),
            PlayerMenuAction::OpenHistory => self.open_menu_tab(PlayerMenuTabKind::History),
            PlayerMenuAction::OpenRoutes => self.open_menu_tab(PlayerMenuTabKind::Routes),
            PlayerMenuAction::OpenSettings => self.open_menu_tab(PlayerMenuTabKind::Settings),
            PlayerMenuAction::OpenSystem => self.open_menu_tab(PlayerMenuTabKind::System),
            PlayerMenuAction::ToggleHistoryWindow => self.show_history = !self.show_history,
            PlayerMenuAction::ToggleFullscreen => {
                self.prefs.fullscreen = !self.prefs.fullscreen;
                self.persist_preferences();
            }
            PlayerMenuAction::RestartStory => self.restart_story(),
            PlayerMenuAction::QuitGame => {
                if let Some(ctx) = ctx {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
        }
    }

    fn open_menu_tab(&mut self, tab: PlayerMenuTabKind) {
        if self.config.player_menu.tab_label(tab).is_some() {
            self.menu_tab = tab;
        }
        self.show_menu = true;
    }

    pub(super) fn apply_preferences(&mut self, ctx: &egui::Context) {
        let scale = (self.config.scale_factor * self.prefs.ui_scale).max(0.5);
        if (scale - self.applied_scale).abs() > f32::EPSILON {
            ctx.set_pixels_per_point(scale);
            self.applied_scale = scale;
        }
        ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(self.prefs.fullscreen));
        self.apply_audio_preferences();
    }

    fn current_save_data(&self) -> visual_novel_engine::SaveData {
        visual_novel_engine::SaveData::new(self.script_id, self.engine.state().clone())
    }

    pub(super) fn quicksave(&mut self) {
        let data = self.current_save_data();
        match self.save_store.quicksave(&data) {
            Ok(entry) => {
                self.last_error = None;
                self.last_status = Some(format!("Quick saved: {}", save_slot_summary(&entry)));
            }
            Err(err) => self.last_error = Some(format!("Quick save failed: {err}")),
        }
    }

    pub(super) fn quickload(&mut self) {
        match self.save_store.quickload() {
            Ok(data) => self.apply_loaded_save(data, "Quick loaded"),
            Err(err) => self.last_error = Some(format!("Quick load failed: {err}")),
        }
    }

    pub(super) fn save_slot(&mut self, slot_id: u16) {
        let data = self.current_save_data();
        match self.save_store.save_slot(slot_id, &data) {
            Ok(entry) => {
                self.last_error = None;
                self.last_status = Some(format!("Saved: {}", save_slot_summary(&entry)));
            }
            Err(err) => self.last_error = Some(format!("Save slot {slot_id} failed: {err}")),
        }
    }

    pub(super) fn load_slot(&mut self, slot_id: u16) {
        match self.save_store.load_slot(slot_id) {
            Ok(data) => self.apply_loaded_save(data, &format!("Loaded slot {slot_id}")),
            Err(err) => self.last_error = Some(format!("Load slot {slot_id} failed: {err}")),
        }
    }

    fn apply_loaded_save(&mut self, data: visual_novel_engine::SaveData, status: &str) {
        if let Err(err) = data.validate_script_id(&self.script_id) {
            self.last_error = Some(format!("Save data mismatch: {err}"));
            return;
        }
        match self.engine.set_state(data.state) {
            Ok(()) => {
                self.last_error = None;
                self.last_status = Some(status.to_string());
                let audio = self.engine.take_audio_commands();
                self.apply_audio_commands(audio);
            }
            Err(err) => self.last_error = Some(format!("Failed to load state: {err}")),
        }
    }

    pub(super) fn list_save_slots_for_menu(&mut self) -> Vec<SaveSlotEntry> {
        match self.save_store.list_slots() {
            Ok(entries) => entries,
            Err(err) => {
                self.last_error = Some(format!("List saves failed: {err}"));
                Vec::new()
            }
        }
    }

    pub(super) fn restart_story(&mut self) {
        match self.engine.jump_to_label("start") {
            Ok(()) => {
                self.engine.clear_session_history();
                let audio = self.engine.take_audio_commands();
                self.apply_audio_commands(audio);
                self.last_error = None;
                self.last_status = Some("Story restarted".to_string());
            }
            Err(err) => self.last_error = Some(format!("Restart failed: {err}")),
        }
    }

    #[allow(dead_code)]
    pub(super) fn save_state(&mut self, path: &Path) {
        let data = visual_novel_engine::SaveData::new(self.script_id, self.engine.state().clone());
        match crate::persist::save_state_to(path, &data) {
            Ok(()) => {
                self.last_error = None;
                self.last_status = Some(format!("Saved: {}", path.display()));
            }
            Err(err) => self.last_error = Some(format!("Failed to save state: {err}")),
        }
    }

    #[allow(dead_code)]
    pub(super) fn load_state(&mut self, path: &Path) {
        match crate::persist::load_state_from(path) {
            Ok(data) => {
                if let Err(err) = data.validate_script_id(&self.script_id) {
                    self.last_error = Some(format!("Save data mismatch: {err}"));
                    return;
                }
                if let Err(err) = self.engine.set_state(data.state) {
                    self.last_error = Some(format!("Failed to load state: {err}"));
                } else {
                    self.last_error = None;
                    self.last_status = Some(format!("Loaded: {}", path.display()));
                }
            }
            Err(err) => self.last_error = Some(format!("Failed to load state: {err}")),
        }
    }

    pub(super) fn persist_preferences(&self) {
        if let Err(err) = self.prefs.save_to(&self.prefs_path) {
            eprintln!("Failed to save GUI preferences: {err}");
        }
    }
}
