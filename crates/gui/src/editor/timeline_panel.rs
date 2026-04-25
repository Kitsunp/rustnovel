//! Timeline panel for the editor workbench.
//!
//! Displays and edits animation keyframes.

use eframe::egui;
use visual_novel_engine::{Easing, EntityId, Keyframe, PropertyType, Timeline};

const ANIMATABLE_PROPERTIES: [PropertyType; 6] = [
    PropertyType::PositionX,
    PropertyType::PositionY,
    PropertyType::ZOrder,
    PropertyType::Scale,
    PropertyType::Opacity,
    PropertyType::Rotation,
];

/// Timeline panel widget.
pub struct TimelinePanel<'a> {
    timeline: &'a mut Timeline,
    current_time: &'a mut u32,
    is_playing: &'a mut bool,
}

impl<'a> TimelinePanel<'a> {
    pub fn new(
        timeline: &'a mut Timeline,
        current_time: &'a mut u32,
        is_playing: &'a mut bool,
    ) -> Self {
        Self {
            timeline,
            current_time,
            is_playing,
        }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) {
        ui.heading("Timeline");
        ui.separator();
        self.render_playback_controls(ui);
        ui.separator();
        self.render_scrubber(ui);
        ui.separator();
        self.render_keyframe_controls(ui);
        ui.separator();
        self.render_track_list(ui);
    }

    fn render_playback_controls(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let play_text = if *self.is_playing { "Pause" } else { "Play" };
            if ui.button(play_text).clicked() {
                *self.is_playing = !*self.is_playing;
            }
            if ui.button("Stop").clicked() {
                *self.is_playing = false;
                *self.current_time = 0;
                self.timeline.seek(0);
            }
            if ui.button("Rewind").clicked() {
                *self.current_time = 0;
                self.timeline.seek(0);
            }

            ui.separator();
            let seconds = *self.current_time as f32 / self.timeline.ticks_per_second as f32;
            ui.label(format!(
                "Time: {:.2}s ({} ticks)",
                seconds, *self.current_time
            ));

            let duration = self.timeline.duration();
            let duration_secs = duration as f32 / self.timeline.ticks_per_second as f32;
            ui.label(format!("Duration: {:.2}s", duration_secs));
        });
    }

    fn render_scrubber(&mut self, ui: &mut egui::Ui) {
        let duration = self.timeline.duration().max(1);
        let mut time_float = *self.current_time as f32;
        ui.horizontal(|ui| {
            ui.label("Scrub:");
            if ui
                .add(egui::Slider::new(&mut time_float, 0.0..=duration as f32).show_value(false))
                .changed()
            {
                *self.current_time = time_float as u32;
                self.timeline.seek(*self.current_time);
            }
        });
    }

    fn render_keyframe_controls(&mut self, ui: &mut egui::Ui) {
        let entity_id_key = egui::Id::new("timeline_add_keyframe_entity");
        let property_key = egui::Id::new("timeline_add_keyframe_property");
        let value_key = egui::Id::new("timeline_add_keyframe_value");

        let mut entity_id = ui
            .ctx()
            .data_mut(|data| data.get_persisted::<u32>(entity_id_key))
            .unwrap_or(0);
        let mut property = ui
            .ctx()
            .data_mut(|data| data.get_persisted::<PropertyType>(property_key))
            .unwrap_or(PropertyType::PositionX);
        let mut value = ui
            .ctx()
            .data_mut(|data| data.get_persisted::<i32>(value_key))
            .unwrap_or(0);

        let mut feedback = None;
        ui.horizontal_wrapped(|ui| {
            ui.label("Add keyframe:");
            if ui
                .add(egui::DragValue::new(&mut entity_id).prefix("Entity "))
                .changed()
            {
                ui.ctx()
                    .data_mut(|data| data.insert_persisted(entity_id_key, entity_id));
            }
            egui::ComboBox::from_id_source("timeline_property")
                .selected_text(property_label(property))
                .show_ui(ui, |ui| {
                    for candidate in ANIMATABLE_PROPERTIES {
                        ui.selectable_value(&mut property, candidate, property_label(candidate));
                    }
                });
            ui.ctx()
                .data_mut(|data| data.insert_persisted(property_key, property));
            if ui
                .add(egui::DragValue::new(&mut value).prefix("Value "))
                .changed()
            {
                ui.ctx()
                    .data_mut(|data| data.insert_persisted(value_key, value));
            }
            if ui.button("Add").clicked() {
                let result = add_keyframe(
                    self.timeline,
                    entity_id,
                    property,
                    *self.current_time,
                    value,
                );
                feedback = Some(match result {
                    Ok(()) => "Keyframe added".to_string(),
                    Err(err) => format!("Keyframe rejected: {err}"),
                });
            }
        });
        if let Some(message) = feedback {
            ui.label(message);
        }
    }

    fn render_track_list(&mut self, ui: &mut egui::Ui) {
        ui.label(format!("Tracks: {}", self.timeline.track_count()));
        egui::ScrollArea::vertical()
            .max_height(80.0)
            .show(ui, |ui| {
                for (idx, track) in self.timeline.tracks().enumerate() {
                    ui.horizontal(|ui| {
                        ui.label(format!(
                            "Track {}: Entity {:?} - {:?}",
                            idx,
                            track.target.raw(),
                            track.property
                        ));
                        ui.label(format!("({} keyframes)", track.len()));
                    });
                }

                if self.timeline.track_count() == 0 {
                    ui.label("No tracks yet.");
                }
            });
    }
}

fn add_keyframe(
    timeline: &mut Timeline,
    entity_id: u32,
    property: PropertyType,
    time: u32,
    value: i32,
) -> Result<(), visual_novel_engine::TimelineError> {
    timeline
        .get_or_create_track(EntityId::new(entity_id), property)
        .and_then(|track| track.add_keyframe(Keyframe::new(time, value, Easing::Linear)))
}

fn property_label(property: PropertyType) -> &'static str {
    match property {
        PropertyType::PositionX => "Position X",
        PropertyType::PositionY => "Position Y",
        PropertyType::ZOrder => "Z Order",
        PropertyType::Scale => "Scale",
        PropertyType::Opacity => "Opacity",
        PropertyType::Rotation => "Rotation",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_keyframe_creates_track_when_missing() {
        let mut timeline = Timeline::new(60);
        add_keyframe(&mut timeline, 7, PropertyType::PositionX, 12, 300).expect("add keyframe");

        assert_eq!(timeline.track_count(), 1);
        let track = timeline.get_track(0).expect("track exists");
        assert_eq!(track.target, EntityId::new(7));
        assert_eq!(track.property, PropertyType::PositionX);
        assert_eq!(track.len(), 1);
    }

    #[test]
    fn property_labels_cover_all_timeline_properties() {
        for property in ANIMATABLE_PROPERTIES {
            assert!(!property_label(property).is_empty());
        }
    }
}
