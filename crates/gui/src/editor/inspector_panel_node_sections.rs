use eframe::egui;
use visual_novel_engine::{
    CharacterPatchRaw, CharacterPlacementRaw, CmpOp, CondRaw, ScenePatchRaw,
};

use super::{asset_fields, NodeEditActions};
use crate::editor::{inspector_panel::InspectorAction, AssetFieldTarget, AssetImportKind};

pub(super) struct SceneNodeRefs<'a> {
    pub profile: &'a mut Option<String>,
    pub background: &'a mut Option<String>,
    pub music: &'a mut Option<String>,
    pub characters: &'a mut Vec<CharacterPlacementRaw>,
}

pub(super) fn render_dialogue_node(
    ui: &mut egui::Ui,
    speaker: &mut String,
    text: &mut String,
    standard_changed: &mut bool,
) {
    ui.label("Speaker:");
    *standard_changed |= ui.text_edit_singleline(speaker).changed();
    ui.label("Text:");
    *standard_changed |= ui.text_edit_multiline(text).changed();
}

pub(super) fn render_choice_node(
    ui: &mut egui::Ui,
    prompt: &mut String,
    options: &mut [String],
    standard_changed: &mut bool,
    actions: &mut NodeEditActions,
) {
    ui.label("Prompt:");
    *standard_changed |= ui.text_edit_multiline(prompt).changed();

    ui.separator();
    ui.label("Options:");

    for (i, option) in options.iter_mut().enumerate() {
        ui.horizontal(|ui| {
            *standard_changed |= ui.text_edit_singleline(option).changed();
            if ui.button("Delete").clicked() {
                actions.delete_option_idx = Some(i);
            }
        });
    }

    if ui.button("Add Option").clicked() {
        actions.add_option_req = true;
    }
}

pub(super) fn render_scene_node(
    ui: &mut egui::Ui,
    node_id: u32,
    scene: SceneNodeRefs<'_>,
    scene_profile_names: &[String],
    standard_changed: &mut bool,
    actions: &mut NodeEditActions,
) {
    let SceneNodeRefs {
        profile,
        background,
        music,
        characters,
    } = scene;
    let mut profile_id = profile.clone().unwrap_or_default();
    ui.horizontal(|ui| {
        ui.label("Scene Profile:");
        if ui.text_edit_singleline(&mut profile_id).changed() {
            *profile = if profile_id.trim().is_empty() {
                None
            } else {
                Some(profile_id.clone())
            };
            *standard_changed = true;
        }
    });

    if !scene_profile_names.is_empty() {
        let selected_text = profile
            .clone()
            .unwrap_or_else(|| "<select profile>".to_string());
        egui::ComboBox::from_label("Available Profiles")
            .selected_text(selected_text)
            .show_ui(ui, |ui| {
                for name in scene_profile_names {
                    if ui.selectable_label(false, name).clicked() {
                        *profile = Some(name.clone());
                        *standard_changed = true;
                    }
                }
            });
    }

    ui.horizontal(|ui| {
        if ui.button("Save Profile").clicked() {
            actions.save_scene_profile_req = profile.clone();
        }
        if ui.button("Apply Profile").clicked() {
            actions.apply_scene_profile_req = profile.clone();
        }
    });

    ui.separator();
    asset_fields::edit_optional_asset_text(
        ui,
        background,
        asset_fields::AssetFieldEdit {
            label: "Background Image:",
            kind: AssetImportKind::Background,
            target: AssetFieldTarget::SceneBackground,
            node_id,
        },
        standard_changed,
        actions,
    );
    asset_fields::edit_optional_asset_text(
        ui,
        music,
        asset_fields::AssetFieldEdit {
            label: "Background Music:",
            kind: AssetImportKind::Audio,
            target: AssetFieldTarget::SceneMusic,
            node_id,
        },
        standard_changed,
        actions,
    );
    ui.horizontal(|ui| {
        let can_preview = music.as_ref().is_some_and(|path| !path.trim().is_empty());
        if ui
            .add_enabled(can_preview, egui::Button::new("Preview Music"))
            .clicked()
        {
            if let Some(path) = music.clone() {
                actions.inspector_action = Some(InspectorAction::PreviewAudio {
                    channel: "bgm".to_string(),
                    path,
                    volume: None,
                    loop_playback: true,
                });
            }
        }
        if ui.button("Stop BGM").clicked() {
            actions.inspector_action = Some(InspectorAction::StopAudio {
                channel: "bgm".to_string(),
            });
        }
    });

    ui.separator();
    ui.label(format!("Characters in Scene: {}", characters.len()));
    let mut delete_character_idx = None;
    for (idx, character) in characters.iter_mut().enumerate() {
        ui.group(|ui| {
            ui.horizontal(|ui| {
                ui.label(format!("Layer {}", idx + 1));
                if ui.button("Remove").clicked() {
                    delete_character_idx = Some(idx);
                }
            });
            asset_fields::render_character_fields(
                ui,
                node_id,
                idx,
                character,
                standard_changed,
                actions,
            );
        });
    }
    if let Some(idx) = delete_character_idx {
        characters.remove(idx);
        *standard_changed = true;
    }
    if ui.button("Add Character Layer").clicked() {
        characters.push(CharacterPlacementRaw {
            name: "Character".to_string(),
            ..Default::default()
        });
        *standard_changed = true;
    }
}

pub(super) fn render_jump_if_node(
    ui: &mut egui::Ui,
    target: &mut String,
    cond: &mut CondRaw,
    standard_changed: &mut bool,
) {
    ui.label("Target Label:");
    *standard_changed |= ui.text_edit_singleline(target).changed();

    ui.separator();
    ui.label("Condition:");

    let is_flag = matches!(cond, CondRaw::Flag { .. });
    let mut type_changed = false;

    egui::ComboBox::from_label("Type")
        .selected_text(if is_flag {
            "Flag"
        } else {
            "Variable Comparison"
        })
        .show_ui(ui, |ui| {
            if ui.selectable_label(is_flag, "Flag").clicked() && !is_flag {
                *cond = CondRaw::Flag {
                    key: "flag_name".to_string(),
                    is_set: true,
                };
                type_changed = true;
            }
            if ui
                .selectable_label(!is_flag, "Variable Comparison")
                .clicked()
                && is_flag
            {
                *cond = CondRaw::VarCmp {
                    key: "var_name".to_string(),
                    op: CmpOp::Eq,
                    value: 0,
                };
                type_changed = true;
            }
        });

    *standard_changed |= type_changed;

    match cond {
        CondRaw::Flag { key, is_set } => {
            ui.label("Flag Key:");
            *standard_changed |= ui.text_edit_singleline(key).changed();
            ui.horizontal(|ui| {
                ui.label("Is Set:");
                *standard_changed |= ui.checkbox(is_set, "").changed();
            });
        }
        CondRaw::VarCmp { key, op, value } => {
            ui.label("Var Key:");
            *standard_changed |= ui.text_edit_singleline(key).changed();

            ui.horizontal(|ui| {
                ui.label("Op:");
                egui::ComboBox::from_id_source("cmp_op")
                    .selected_text(format!("{:?}", op))
                    .show_ui(ui, |ui| {
                        for candidate in [
                            CmpOp::Eq,
                            CmpOp::Ne,
                            CmpOp::Lt,
                            CmpOp::Le,
                            CmpOp::Gt,
                            CmpOp::Ge,
                        ] {
                            if ui
                                .selectable_label(*op == candidate, format!("{:?}", candidate))
                                .clicked()
                            {
                                *op = candidate;
                                *standard_changed = true;
                            }
                        }
                    });

                ui.label("Val:");
                *standard_changed |= ui.add(egui::DragValue::new(value)).changed();
            });
        }
    }
}

pub(super) fn render_scene_patch_node(
    ui: &mut egui::Ui,
    node_id: u32,
    patch: &mut ScenePatchRaw,
    standard_changed: &mut bool,
    actions: &mut NodeEditActions,
) {
    ui.label("Scene Patch");
    ui.separator();

    asset_fields::edit_optional_asset_inline(
        ui,
        &mut patch.music,
        asset_fields::AssetFieldEdit {
            label: "Music:",
            kind: AssetImportKind::Audio,
            target: AssetFieldTarget::ScenePatchMusic,
            node_id,
        },
        standard_changed,
        actions,
    );
    asset_fields::edit_optional_asset_inline(
        ui,
        &mut patch.background,
        asset_fields::AssetFieldEdit {
            label: "Background (Override):",
            kind: AssetImportKind::Background,
            target: AssetFieldTarget::ScenePatchBackground,
            node_id,
        },
        standard_changed,
        actions,
    );

    ui.separator();
    ui.collapsing(format!("Add Characters ({})", patch.add.len()), |ui| {
        let mut delete_add_idx = None;
        for (i, character) in patch.add.iter_mut().enumerate() {
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    ui.label("Name:");
                    *standard_changed |= ui.text_edit_singleline(&mut character.name).changed();
                    if ui.button("Delete").clicked() {
                        delete_add_idx = Some(i);
                    }
                });
                asset_fields::render_optional_character_fields(
                    ui,
                    node_id,
                    Some(AssetFieldTarget::ScenePatchAddCharacterExpression(i)),
                    &mut character.expression,
                    &mut character.position,
                    standard_changed,
                    actions,
                );
                asset_fields::render_optional_transform_fields(
                    ui,
                    &mut character.x,
                    &mut character.y,
                    &mut character.scale,
                    standard_changed,
                );
            });
        }
        if let Some(idx) = delete_add_idx {
            patch.add.remove(idx);
            *standard_changed = true;
        }
        if ui.button("Add Character").clicked() {
            patch.add.push(CharacterPlacementRaw::default());
            *standard_changed = true;
        }
    });

    ui.separator();
    ui.collapsing(
        format!("Remove Characters ({})", patch.remove.len()),
        |ui| {
            let mut delete_remove_idx = None;
            for (i, name) in patch.remove.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    ui.label("Name:");
                    *standard_changed |= ui.text_edit_singleline(name).changed();
                    if ui.button("Delete").clicked() {
                        delete_remove_idx = Some(i);
                    }
                });
            }
            if let Some(idx) = delete_remove_idx {
                patch.remove.remove(idx);
                *standard_changed = true;
            }
            if ui.button("Add Remove Entry").clicked() {
                patch.remove.push("StartTypingName".to_string());
                *standard_changed = true;
            }
        },
    );
}

pub(super) fn render_transition_node(
    ui: &mut egui::Ui,
    kind: &mut String,
    duration_ms: &mut u32,
    color: &mut Option<String>,
    standard_changed: &mut bool,
) {
    ui.label("Transition");
    ui.separator();

    ui.label("Kind (fade/dissolve/cut):");
    *standard_changed |= ui.text_edit_singleline(kind).changed();

    ui.label("Duration (ms):");
    *standard_changed |= ui.add(egui::DragValue::new(duration_ms)).changed();

    edit_optional_text(ui, "Color (Hex/Name):", color, standard_changed);
}

pub(super) fn render_character_placement_node(
    ui: &mut egui::Ui,
    name: &mut String,
    x: &mut i32,
    y: &mut i32,
    scale: &mut Option<f32>,
    standard_changed: &mut bool,
) {
    ui.label("Character Placement");
    ui.separator();

    ui.label("Name:");
    *standard_changed |= ui.text_edit_singleline(name).changed();

    ui.horizontal(|ui| {
        ui.label("Position:");
        ui.label("X");
        *standard_changed |= ui.add(egui::DragValue::new(x)).changed();
        ui.label("Y");
        *standard_changed |= ui.add(egui::DragValue::new(y)).changed();
    });

    ui.horizontal(|ui| {
        ui.label("Scale:");
        let mut current_scale = scale.unwrap_or(1.0);
        if ui
            .add(egui::DragValue::new(&mut current_scale).speed(0.1))
            .changed()
        {
            *scale = Some(current_scale);
            *standard_changed = true;
        }
    });
}

fn edit_optional_text(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut Option<String>,
    standard_changed: &mut bool,
) {
    ui.label(label);
    let mut text = value.clone().unwrap_or_default();
    if ui.text_edit_singleline(&mut text).changed() {
        *value = (!text.trim().is_empty()).then_some(text);
        *standard_changed = true;
    }
}

#[allow(dead_code)]
fn _type_anchor(_: &CharacterPatchRaw) {}
