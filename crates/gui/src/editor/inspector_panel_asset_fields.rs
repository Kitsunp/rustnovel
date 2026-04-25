use eframe::egui;

use super::NodeEditActions;
use crate::editor::{inspector_panel::InspectorAction, AssetFieldTarget, AssetImportKind};

pub(super) fn edit_optional_asset_text(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut Option<String>,
    kind: AssetImportKind,
    target: AssetFieldTarget,
    node_id: u32,
    standard_changed: &mut bool,
    actions: &mut NodeEditActions,
) {
    ui.label(label);
    ui.horizontal(|ui| {
        render_asset_text_field(ui, value, kind, target, node_id, standard_changed, actions);
    });
}

pub(super) fn edit_optional_asset_inline(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut Option<String>,
    kind: AssetImportKind,
    target: AssetFieldTarget,
    node_id: u32,
    standard_changed: &mut bool,
    actions: &mut NodeEditActions,
) {
    ui.horizontal(|ui| {
        ui.label(label);
        render_asset_text_field(ui, value, kind, target, node_id, standard_changed, actions);
    });
}

pub(super) fn edit_optional_text_inline(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut Option<String>,
    standard_changed: &mut bool,
) {
    let mut text = value.clone().unwrap_or_default();
    ui.horizontal(|ui| {
        ui.label(label);
        if ui.text_edit_singleline(&mut text).changed() {
            *value = (!text.is_empty()).then_some(text.clone());
            *standard_changed = true;
        }
    });
}

pub(super) fn render_character_fields(
    ui: &mut egui::Ui,
    node_id: u32,
    character_index: usize,
    character: &mut visual_novel_engine::CharacterPlacementRaw,
    standard_changed: &mut bool,
    actions: &mut NodeEditActions,
) {
    ui.horizontal(|ui| {
        ui.label("Name:");
        *standard_changed |= ui.text_edit_singleline(&mut character.name).changed();
    });
    render_optional_character_fields(
        ui,
        node_id,
        Some(AssetFieldTarget::SceneCharacterExpression(character_index)),
        &mut character.expression,
        &mut character.position,
        standard_changed,
        actions,
    );
    render_optional_transform_fields(
        ui,
        &mut character.x,
        &mut character.y,
        &mut character.scale,
        standard_changed,
    );
}

pub(super) fn render_optional_character_fields(
    ui: &mut egui::Ui,
    node_id: u32,
    expression_target: Option<AssetFieldTarget>,
    expression: &mut Option<String>,
    position: &mut Option<String>,
    standard_changed: &mut bool,
    actions: &mut NodeEditActions,
) {
    if let Some(target) = expression_target {
        edit_optional_asset_inline(
            ui,
            "Expr:",
            expression,
            AssetImportKind::Character,
            target,
            node_id,
            standard_changed,
            actions,
        );
    } else {
        edit_optional_text_inline(ui, "Expr:", expression, standard_changed);
    }
    edit_optional_text_inline(ui, "Pos:", position, standard_changed);
}

pub(super) fn render_optional_transform_fields(
    ui: &mut egui::Ui,
    x: &mut Option<i32>,
    y: &mut Option<i32>,
    scale: &mut Option<f32>,
    standard_changed: &mut bool,
) {
    ui.horizontal_wrapped(|ui| {
        let mut use_position = x.is_some() || y.is_some();
        if ui.checkbox(&mut use_position, "XY").changed() {
            if use_position {
                *x = Some(x.unwrap_or(0));
                *y = Some(y.unwrap_or(0));
            } else {
                *x = None;
                *y = None;
            }
            *standard_changed = true;
        }
        if use_position {
            let mut x_value = x.unwrap_or(0);
            let mut y_value = y.unwrap_or(0);
            if ui
                .add(egui::DragValue::new(&mut x_value).prefix("x "))
                .changed()
            {
                *x = Some(x_value);
                *standard_changed = true;
            }
            if ui
                .add(egui::DragValue::new(&mut y_value).prefix("y "))
                .changed()
            {
                *y = Some(y_value);
                *standard_changed = true;
            }
        }

        let mut use_scale = scale.is_some();
        if ui.checkbox(&mut use_scale, "Scale").changed() {
            *scale = use_scale.then_some(scale.unwrap_or(1.0));
            *standard_changed = true;
        }
        if use_scale {
            let mut scale_value = scale.unwrap_or(1.0);
            if ui
                .add(egui::DragValue::new(&mut scale_value).speed(0.05))
                .changed()
            {
                *scale = Some(scale_value.clamp(0.1, 4.0));
                *standard_changed = true;
            }
        }
    });
}

fn render_asset_text_field(
    ui: &mut egui::Ui,
    value: &mut Option<String>,
    kind: AssetImportKind,
    target: AssetFieldTarget,
    node_id: u32,
    standard_changed: &mut bool,
    actions: &mut NodeEditActions,
) {
    let mut text = value.clone().unwrap_or_default();
    let button_width = 116.0;
    let text_width = (ui.available_width() - button_width - 8.0).max(80.0);
    if ui
        .add_sized([text_width, 20.0], egui::TextEdit::singleline(&mut text))
        .changed()
    {
        *value = (!text.trim().is_empty()).then_some(text);
        *standard_changed = true;
    }
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
            target,
        });
    }
}
