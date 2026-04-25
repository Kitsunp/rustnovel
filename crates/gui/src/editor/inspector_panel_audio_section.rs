use eframe::egui;

use super::NodeEditActions;
use crate::editor::{inspector_panel::InspectorAction, AssetFieldTarget, AssetImportKind};

pub(super) struct AudioActionRefs<'a> {
    pub channel: &'a mut String,
    pub action: &'a mut String,
    pub asset: &'a mut Option<String>,
    pub volume: &'a mut Option<f32>,
    pub fade_duration_ms: &'a mut Option<u64>,
    pub loop_playback: &'a mut Option<bool>,
}

pub(super) fn render_audio_action_node(
    ui: &mut egui::Ui,
    node_id: u32,
    audio: AudioActionRefs<'_>,
    standard_changed: &mut bool,
    actions: &mut NodeEditActions,
) {
    let AudioActionRefs {
        channel,
        action,
        asset,
        volume,
        fade_duration_ms,
        loop_playback,
    } = audio;
    ui.label("Audio Action");
    ui.separator();

    egui::ComboBox::from_label("Channel")
        .selected_text(channel.as_str())
        .show_ui(ui, |ui| {
            for candidate in ["bgm", "sfx", "voice"] {
                if ui
                    .selectable_label(channel.as_str() == candidate, candidate)
                    .clicked()
                {
                    *channel = candidate.to_string();
                    *standard_changed = true;
                }
            }
        });

    egui::ComboBox::from_label("Action")
        .selected_text(action.as_str())
        .show_ui(ui, |ui| {
            for candidate in ["play", "stop", "fade_out"] {
                if ui
                    .selectable_label(action == candidate, candidate)
                    .clicked()
                {
                    *action = candidate.to_string();
                    *standard_changed = true;
                }
            }
        });

    let mut asset_text = asset.clone().unwrap_or_default();
    ui.label("Asset Path:");
    let mut asset_response = None;
    ui.horizontal(|ui| {
        let kind = AssetImportKind::Audio;
        let button_width = 116.0;
        let text_width = (ui.available_width() - button_width - 8.0).max(80.0);
        asset_response = Some(ui.add_sized(
            [text_width, 20.0],
            egui::TextEdit::singleline(&mut asset_text),
        ));
        if ui
            .add_sized(
                [button_width, 20.0],
                egui::Button::new(kind.field_button_label()),
            )
            .clicked()
        {
            actions.inspector_action = Some(InspectorAction::ImportAssetForNode {
                node_id,
                kind,
                target: AssetFieldTarget::AudioActionAsset,
            });
        }
    });
    let Some(asset_response) = asset_response else {
        return;
    };
    if asset_response.changed() {
        *asset = (!asset_text.is_empty()).then_some(asset_text);
        *standard_changed = true;
    }

    if asset_response.hovered() && ui.ctx().dragged_id().is_some() {
        let payload = ui.memory(|mem| mem.data.get_temp::<String>(egui::Id::new("dragged_asset")));
        if let Some(payload) = payload {
            if payload.starts_with("asset://audio/") {
                ui.painter()
                    .rect_stroke(asset_response.rect, 0.0, (2.0, egui::Color32::GREEN));

                if ui.input(|i| i.pointer.any_released()) {
                    if let Some(filename) = payload.strip_prefix("asset://audio/") {
                        *asset = Some(filename.to_string());
                        *standard_changed = true;
                    }
                }
            }
        }
    }

    ui.separator();
    ui.label("Options:");

    let mut current_volume = volume.unwrap_or(1.0);
    ui.horizontal(|ui| {
        ui.label("Volume:");
        if ui
            .add(egui::Slider::new(&mut current_volume, 0.0..=1.0))
            .changed()
        {
            *volume = Some(current_volume);
            *standard_changed = true;
        }
    });

    let mut fade_ms = fade_duration_ms.unwrap_or(0);
    ui.horizontal(|ui| {
        ui.label("Fade (ms):");
        if ui.add(egui::DragValue::new(&mut fade_ms)).changed() {
            *fade_duration_ms = (fade_ms > 0).then_some(fade_ms);
            *standard_changed = true;
        }
    });

    let mut looping = loop_playback.unwrap_or(false);
    ui.horizontal(|ui| {
        ui.label("Loop:");
        if ui.checkbox(&mut looping, "").changed() {
            *loop_playback = Some(looping);
            *standard_changed = true;
        }
    });

    ui.separator();
    ui.horizontal(|ui| {
        let can_preview = action.trim().eq_ignore_ascii_case("play")
            && asset.as_ref().is_some_and(|path| !path.trim().is_empty());
        if ui
            .add_enabled(can_preview, egui::Button::new("Preview"))
            .clicked()
        {
            if let Some(path) = asset.clone() {
                actions.inspector_action = Some(InspectorAction::PreviewAudio {
                    channel: channel.clone(),
                    path,
                    volume: *volume,
                    loop_playback: loop_playback.unwrap_or(channel.as_str() == "bgm"),
                });
            }
        }
        if ui.button("Stop Channel").clicked() {
            actions.inspector_action = Some(InspectorAction::StopAudio {
                channel: channel.clone(),
            });
        }
    });
}
