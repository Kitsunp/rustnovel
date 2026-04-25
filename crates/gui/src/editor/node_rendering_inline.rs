use eframe::egui;

pub(super) fn edit_optional_text(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut Option<String>,
) -> bool {
    let mut changed = false;
    let mut text = value.clone().unwrap_or_default();
    ui.horizontal(|ui| {
        ui.label(label);
        if ui.text_edit_singleline(&mut text).changed() {
            *value = if text.trim().is_empty() {
                None
            } else {
                Some(text)
            };
            changed = true;
        }
    });
    changed
}

pub(super) fn edit_scene_patch_inline(
    ui: &mut egui::Ui,
    patch: &mut visual_novel_engine::ScenePatchRaw,
) -> bool {
    let mut changed = false;
    ui.label("Scene Patch");
    changed |= edit_optional_text(ui, "Background:", &mut patch.background);
    changed |= edit_optional_text(ui, "Music:", &mut patch.music);

    ui.label("Characters to add/update:");
    for character in &mut patch.add {
        ui.horizontal(|ui| {
            ui.label("Name:");
            changed |= ui.text_edit_singleline(&mut character.name).changed();
            ui.label("Expression:");
            changed |= edit_inline_optional(ui, &mut character.expression);
        });
    }
    if ui.button("Add Character").clicked() {
        patch
            .add
            .push(visual_novel_engine::CharacterPlacementRaw::default());
        changed = true;
    }
    ui.label("Remove character names:");
    for name in &mut patch.remove {
        changed |= ui.text_edit_singleline(name).changed();
    }
    if ui.button("Add Remove Entry").clicked() {
        patch.remove.push(String::new());
        changed = true;
    }
    changed
}

pub(super) fn edit_audio_action_inline(
    ui: &mut egui::Ui,
    channel: &mut String,
    action: &mut String,
    asset: &mut Option<String>,
    volume: &mut Option<f32>,
    fade_duration_ms: &mut Option<u64>,
    loop_playback: &mut Option<bool>,
) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.label("Channel:");
        changed |= ui.text_edit_singleline(channel).changed();
        ui.label("Action:");
        changed |= ui.text_edit_singleline(action).changed();
    });
    changed |= edit_optional_text(ui, "Asset:", asset);
    let mut vol = volume.unwrap_or(1.0);
    if ui
        .add(egui::Slider::new(&mut vol, 0.0..=1.0).text("Volume"))
        .changed()
    {
        *volume = Some(vol);
        changed = true;
    }
    let mut fade = fade_duration_ms.unwrap_or(0);
    if ui
        .add(egui::DragValue::new(&mut fade).prefix("Fade ms "))
        .changed()
    {
        *fade_duration_ms = if fade == 0 { None } else { Some(fade) };
        changed = true;
    }
    let mut looping = loop_playback.unwrap_or(false);
    if ui.checkbox(&mut looping, "Loop").changed() {
        *loop_playback = Some(looping);
        changed = true;
    }
    changed
}

pub(super) fn edit_transition_inline(
    ui: &mut egui::Ui,
    kind: &mut String,
    duration_ms: &mut u32,
    color: &mut Option<String>,
) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.label("Kind:");
        changed |= ui.text_edit_singleline(kind).changed();
        ui.label("Duration ms:");
        changed |= ui.add(egui::DragValue::new(duration_ms)).changed();
    });
    changed |= edit_optional_text(ui, "Color:", color);
    changed
}

pub(super) fn edit_generic_event_inline(
    ui: &mut egui::Ui,
    event: &mut visual_novel_engine::EventRaw,
) -> bool {
    let mut json = event.to_json_string();
    let mut changed = false;
    ui.label("Generic Event JSON:");
    if ui
        .add(
            egui::TextEdit::multiline(&mut json)
                .code_editor()
                .desired_rows(5),
        )
        .changed()
    {
        match serde_json::from_str::<visual_novel_engine::EventRaw>(&json) {
            Ok(updated) => {
                *event = updated;
                changed = true;
            }
            Err(err) => {
                ui.colored_label(egui::Color32::YELLOW, format!("Invalid event JSON: {err}"));
            }
        }
    }
    changed
}

fn edit_inline_optional(ui: &mut egui::Ui, value: &mut Option<String>) -> bool {
    let mut text = value.clone().unwrap_or_default();
    if ui.text_edit_singleline(&mut text).changed() {
        *value = if text.trim().is_empty() {
            None
        } else {
            Some(text)
        };
        true
    } else {
        false
    }
}
