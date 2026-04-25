use super::*;

impl<'a> InspectorPanel<'a> {
    pub(super) fn render_entity_info(&self, ui: &mut egui::Ui) {
        if let Some(entity_id) = self.selected_entity {
            if let Some(entity) = self
                .scene
                .get(visual_novel_engine::EntityId::new(entity_id))
            {
                ui.label(format!("ID: {}", entity.id));

                ui.separator();
                ui.label("Transform:");
                ui.label(format!(
                    "  Position: ({}, {})",
                    entity.transform.x, entity.transform.y
                ));
                ui.label(format!("  Z-Order: {}", entity.transform.z_order));
                ui.label(format!(
                    "  Scale: {}",
                    entity.transform.scale as f32 / 1000.0
                ));
                ui.label(format!(
                    "  Opacity: {}",
                    entity.transform.opacity as f32 / 1000.0
                ));

                ui.separator();
                ui.label(format!("Kind: {:?}", entity.kind));
            }
        } else {
            ui.label("No entity selected");
        }
    }
}
