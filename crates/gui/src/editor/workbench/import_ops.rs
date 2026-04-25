use super::*;

impl EditorWorkbench {
    pub fn import_renpy_project_native(&mut self) {
        let Some(project_root) = rfd::FileDialog::new().pick_folder() else {
            self.toast = Some(ToastState::warning("Ren'Py import cancelled"));
            return;
        };

        let Some(output_root) = rfd::FileDialog::new()
            .set_directory(&project_root)
            .pick_folder()
        else {
            self.toast = Some(ToastState::warning(
                "Ren'Py output folder selection cancelled",
            ));
            return;
        };

        let options = visual_novel_engine::ImportRenpyOptions {
            project_root: project_root.clone(),
            output_root: output_root.clone(),
            entry_label: "start".to_string(),
            report_path: None,
            profile: visual_novel_engine::ImportProfile::StoryFirst,
            include_tl: None,
            include_ui: None,
            include_patterns: Vec::new(),
            exclude_patterns: Vec::new(),
            strict_mode: false,
            fallback_policy: visual_novel_engine::ImportFallbackPolicy::DegradeWithTrace,
        };

        match visual_novel_engine::import_renpy_project(options) {
            Ok(report) => {
                let manifest_path = output_root.join("project.vnm");
                match self.load_project_with_status(manifest_path, false) {
                    Ok(()) => {
                        self.toast = Some(ToastState::success(format!(
                            "Ren'Py imported and loaded: files={}, events={}, degraded={}, issues={}",
                            report.files_parsed,
                            report.events_generated,
                            report.degraded_events,
                            report.issues.len()
                        )));
                    }
                    Err(load_err) => {
                        self.toast = Some(ToastState::error(format!(
                            "Ren'Py imported but failed to load in UI: {load_err}"
                        )));
                    }
                }
            }
            Err(err) => {
                self.toast = Some(ToastState::error(format!("Ren'Py import failed: {err}")));
            }
        }
    }
}
