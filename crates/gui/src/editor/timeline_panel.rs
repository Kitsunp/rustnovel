//! Timeline panel for the editor workbench.
//!
//! Displays and edits animation keyframes.

use eframe::egui;
use visual_novel_engine::{Easing, EntityId, Keyframe, PropertyType, Timeline};

pub const TIMELINE_ROW_HEIGHT: f32 = 28.0;
pub const TIMELINE_LABEL_WIDTH_MIN: f32 = 112.0;
pub const TIMELINE_LABEL_WIDTH_MAX: f32 = 176.0;
pub const TIMELINE_TRACK_LIST_MIN_HEIGHT: f32 = 56.0;
pub const TIMELINE_TRACK_LIST_MAX_HEIGHT: f32 = 160.0;

pub const ANIMATABLE_PROPERTIES: [PropertyType; 6] = [
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
    selected_entity_id: Option<u32>,
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
            selected_entity_id: None,
        }
    }

    pub fn with_selected_entity(mut self, selected_entity_id: Option<u32>) -> Self {
        self.selected_entity_id = selected_entity_id;
        self
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) {
        self.render_transport(ui);
        ui.separator();
        self.render_keyframe_controls(ui);
        ui.separator();
        self.render_track_list(ui);
    }

    fn render_transport(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            ui.label(egui::RichText::new("Timeline").strong());
            ui.separator();

            let play_text = if *self.is_playing { "Pause" } else { "Play" };
            if ui.small_button(play_text).clicked() {
                *self.is_playing = !*self.is_playing;
            }
            if ui.small_button("Stop").clicked() {
                *self.is_playing = false;
                *self.current_time = 0;
                self.timeline.seek(0);
            }
            if ui.small_button("Rewind").clicked() {
                *self.current_time = 0;
                self.timeline.seek(0);
            }

            ui.separator();
            let seconds = *self.current_time as f32 / self.timeline.ticks_per_second as f32;
            let duration = self.timeline.duration();
            let duration_secs = duration as f32 / self.timeline.ticks_per_second as f32;
            ui.label(format!(
                "{:.2}s / {:.2}s | {} ticks | {} tracks",
                seconds,
                duration_secs,
                *self.current_time,
                self.timeline.track_count()
            ));
        });

        let duration = self.timeline.duration().max(1);
        let mut time_float = *self.current_time as f32;
        ui.horizontal(|ui| {
            let slider_width = ui.available_width().max(96.0);
            if ui
                .add_sized(
                    [slider_width, ui.spacing().interact_size.y],
                    egui::Slider::new(&mut time_float, 0.0..=duration as f32).show_value(false),
                )
                .changed()
            {
                *self.current_time = time_float.round() as u32;
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
            .or(self.selected_entity_id)
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
        let mut add_requested = false;
        ui.horizontal_wrapped(|ui| {
            ui.label(egui::RichText::new("Keyframe").strong());
            if ui.small_button("Add").clicked() {
                add_requested = true;
            }
            if ui
                .add(egui::DragValue::new(&mut entity_id).prefix("Entity "))
                .changed()
            {
                ui.ctx()
                    .data_mut(|data| data.insert_persisted(entity_id_key, entity_id));
            }
            if let Some(selected) = self.selected_entity_id {
                if ui.small_button(format!("Use #{selected}")).clicked() {
                    entity_id = selected;
                    ui.ctx()
                        .data_mut(|data| data.insert_persisted(entity_id_key, entity_id));
                }
            }
            egui::ComboBox::from_id_source("timeline_property")
                .width(116.0)
                .selected_text(property_label(property))
                .show_ui(ui, |ui| {
                    for candidate in ANIMATABLE_PROPERTIES {
                        ui.selectable_value(&mut property, candidate, property_label(candidate));
                    }
                });
            ui.ctx()
                .data_mut(|data| data.insert_persisted(property_key, property));
            if ui
                .add_sized(
                    [108.0, ui.spacing().interact_size.y],
                    egui::DragValue::new(&mut value).prefix("Value "),
                )
                .changed()
            {
                ui.ctx()
                    .data_mut(|data| data.insert_persisted(value_key, value));
            }
        });
        if add_requested {
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
        if let Some(message) = feedback {
            ui.label(message);
        }
    }

    fn render_track_list(&mut self, ui: &mut egui::Ui) {
        let duration = self.timeline.duration().max(1);
        let current_time = (*self.current_time).min(duration);
        let list_height = timeline_track_list_height(ui.available_height());
        egui::ScrollArea::vertical()
            .max_height(list_height)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for (idx, track) in self.timeline.tracks().enumerate() {
                    let row_width = ui.available_width().max(1.0);
                    let (row_rect, _) = ui.allocate_exact_size(
                        egui::vec2(row_width, TIMELINE_ROW_HEIGHT),
                        egui::Sense::hover(),
                    );
                    let lane_rect = timeline_lane_rect(row_rect);
                    let label = format!(
                        "{} | Entity {} | {} keys",
                        property_label(track.property),
                        track.target.raw(),
                        track.len()
                    );
                    let painter = ui.painter();
                    painter.rect_filled(row_rect, 3.0, egui::Color32::from_gray(24));
                    painter.rect_stroke(
                        lane_rect,
                        2.0,
                        egui::Stroke::new(1.0, egui::Color32::from_gray(58)),
                    );
                    let label_clip = egui::Rect::from_min_max(
                        row_rect.min,
                        egui::pos2(
                            (lane_rect.left() - 4.0).max(row_rect.left()),
                            row_rect.bottom(),
                        ),
                    );
                    painter.with_clip_rect(label_clip).text(
                        row_rect.left_center() + egui::vec2(6.0, 0.0),
                        egui::Align2::LEFT_CENTER,
                        format!("{idx}: {label}"),
                        egui::FontId::proportional(12.0),
                        egui::Color32::from_gray(210),
                    );
                    let playhead_x = keyframe_marker_x(lane_rect, current_time, duration);
                    painter.line_segment(
                        [
                            egui::pos2(playhead_x, lane_rect.top()),
                            egui::pos2(playhead_x, lane_rect.bottom()),
                        ],
                        egui::Stroke::new(1.0, egui::Color32::from_rgb(120, 180, 255)),
                    );
                    for keyframe in track.keyframes() {
                        let x = keyframe_marker_x(lane_rect, keyframe.time, duration);
                        painter.circle_filled(
                            egui::pos2(x, lane_rect.center().y),
                            3.5,
                            egui::Color32::from_rgb(180, 230, 140),
                        );
                    }
                }

                if self.timeline.track_count() == 0 {
                    let (rect, _) = ui.allocate_exact_size(
                        egui::vec2(ui.available_width().max(1.0), TIMELINE_ROW_HEIGHT),
                        egui::Sense::hover(),
                    );
                    ui.painter()
                        .rect_filled(rect, 3.0, egui::Color32::from_gray(22));
                    ui.painter().text(
                        rect.center(),
                        egui::Align2::CENTER_CENTER,
                        "No tracks",
                        egui::FontId::proportional(12.0),
                        egui::Color32::from_gray(170),
                    );
                }
            });
    }
}

pub fn timeline_track_list_height(available_height: f32) -> f32 {
    available_height.clamp(
        TIMELINE_TRACK_LIST_MIN_HEIGHT,
        TIMELINE_TRACK_LIST_MAX_HEIGHT,
    )
}

pub fn timeline_lane_rect(row_rect: egui::Rect) -> egui::Rect {
    let label_width =
        (row_rect.width() * 0.36).clamp(TIMELINE_LABEL_WIDTH_MIN, TIMELINE_LABEL_WIDTH_MAX);
    let right = (row_rect.right() - 6.0).max(row_rect.left() + 1.0);
    let lane_min_x = (row_rect.left() + label_width + 6.0).min(right - 1.0);
    egui::Rect::from_min_max(
        egui::pos2(lane_min_x, row_rect.top() + 6.0),
        egui::pos2(right, row_rect.bottom() - 6.0),
    )
}

pub fn keyframe_marker_x(lane_rect: egui::Rect, time: u32, duration: u32) -> f32 {
    let duration = duration.max(1) as f32;
    let ratio = (time as f32 / duration).clamp(0.0, 1.0);
    egui::lerp(lane_rect.left()..=lane_rect.right(), ratio)
}

pub fn add_keyframe(
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

pub fn property_label(property: PropertyType) -> &'static str {
    match property {
        PropertyType::PositionX => "Position X",
        PropertyType::PositionY => "Position Y",
        PropertyType::ZOrder => "Z Order",
        PropertyType::Scale => "Scale",
        PropertyType::Opacity => "Opacity",
        PropertyType::Rotation => "Rotation",
    }
}
