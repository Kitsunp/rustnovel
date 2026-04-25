use super::*;

impl eframe::App for VnApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.show_settings = !self.show_settings;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::F12)) {
            self.show_inspector = !self.show_inspector;
        }

        self.apply_preferences(ctx);

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading(&self.config.title);
            ui.separator();
            self.render_scene(ui);
            ui.separator();
            self.render_ui(ui);
            if let Some(message) = &self.last_error {
                ui.separator();
                ui.colored_label(egui::Color32::RED, message);
            }
        });

        if self.show_settings {
            self.render_settings_window(ctx);
        }

        self.render_history(ctx);
        self.render_inspector(ctx);
    }
}

impl VnApp {
    fn render_settings_window(&mut self, ctx: &egui::Context) {
        let mut dirty = false;
        egui::Window::new("Settings").show(ctx, |ui| {
            dirty |= ui
                .checkbox(&mut self.prefs.fullscreen, "Fullscreen")
                .changed();
            dirty |= ui
                .checkbox(&mut self.prefs.vsync, "VSync (restart required)")
                .changed();
            dirty |= ui
                .add(egui::Slider::new(&mut self.prefs.ui_scale, 0.75..=2.0).text("UI Scale"))
                .changed();
            if ui.button("Save State").clicked() {
                if let Some(path) = FileDialog::new().set_title("Save State").save_file() {
                    self.save_state(&path);
                }
            }
            if ui.button("Load State").clicked() {
                if let Some(path) = FileDialog::new().set_title("Load State").pick_file() {
                    self.load_state(&path);
                }
            }
        });

        if dirty {
            self.persist_preferences();
        }
    }
}
