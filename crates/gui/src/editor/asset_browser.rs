use std::collections::HashMap;
use std::path::{Path, PathBuf};

use eframe::egui;
use visual_novel_engine::manifest::ProjectManifest;

use crate::editor::image_asset_cache::{
    image_failure_message, normalize_asset_path, should_retry_missing_image_failure,
    thumbnail_cache_key,
};
use crate::editor::{AssetImportKind, PreviewQuality};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AssetBrowserAction {
    Import(AssetImportKind),
    Remove { kind: AssetImportKind, name: String },
    PreviewAudio { path: String, offset_ms: u64 },
    StopAudio,
}

pub struct AssetBrowserPanel<'a> {
    pub manifest: &'a ProjectManifest,
    project_root: Option<&'a Path>,
    image_cache: &'a mut HashMap<String, egui::TextureHandle>,
    image_failures: &'a mut HashMap<String, String>,
    audio_duration_cache: &'a mut HashMap<String, Option<f32>>,
    asset_store: Option<vnengine_assets::AssetStore>,
}

impl<'a> AssetBrowserPanel<'a> {
    pub fn new(
        manifest: &'a ProjectManifest,
        project_root: Option<&'a Path>,
        image_cache: &'a mut HashMap<String, egui::TextureHandle>,
        image_failures: &'a mut HashMap<String, String>,
        audio_duration_cache: &'a mut HashMap<String, Option<f32>>,
    ) -> Self {
        Self {
            manifest,
            project_root,
            image_cache,
            image_failures,
            audio_duration_cache,
            asset_store: None,
        }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) -> Vec<AssetBrowserAction> {
        let mut actions = Vec::new();
        ui.heading("Asset Browser");
        ui.horizontal_wrapped(|ui| {
            if ui.button("Import BG").clicked() {
                actions.push(AssetBrowserAction::Import(AssetImportKind::Background));
            }
            if ui.button("Import Character").clicked() {
                actions.push(AssetBrowserAction::Import(AssetImportKind::Character));
            }
            if ui.button("Import Audio").clicked() {
                actions.push(AssetBrowserAction::Import(AssetImportKind::Audio));
            }
        });
        ui.separator();

        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.collapsing("Backgrounds", |ui| {
                if self.manifest.assets.backgrounds.is_empty() {
                    ui.label("No backgrounds in manifest");
                } else {
                    self.render_grid(ui, "bg", &mut actions);
                }
            });

            ui.collapsing("Characters", |ui| {
                if self.manifest.assets.characters.is_empty() {
                    ui.label("No characters in manifest");
                } else {
                    self.render_grid(ui, "char", &mut actions);
                }
            });

            ui.collapsing("Audio", |ui| {
                if self.manifest.assets.audio.is_empty() {
                    ui.label("No audio in manifest");
                } else {
                    for (name, path) in &self.manifest.assets.audio {
                        ui.horizontal(|ui| {
                            let asset_path = normalize_asset_path(&path.to_string_lossy());
                            let button = ui.add(egui::Button::new(format!("Audio {name}")));
                            if button.drag_started() {
                                let payload = format!("asset://audio/{asset_path}");
                                ui.memory_mut(|mem| {
                                    mem.data
                                        .insert_temp(egui::Id::new("dragged_asset"), payload)
                                });
                            }
                            button.on_hover_text(format!("Drag to scene\nPath: {:?}", path));
                            let offset_ms = self.render_audio_position(ui, &asset_path);
                            if ui.small_button("Preview").clicked() {
                                actions.push(AssetBrowserAction::PreviewAudio {
                                    path: asset_path.clone(),
                                    offset_ms,
                                });
                            }
                            if ui.small_button("Stop").clicked() {
                                actions.push(AssetBrowserAction::StopAudio);
                            }
                            if ui.small_button("Remove").clicked() {
                                actions.push(AssetBrowserAction::Remove {
                                    kind: AssetImportKind::Audio,
                                    name: name.clone(),
                                });
                            }
                        });
                    }
                }
            });
        });

        actions
    }

    fn render_grid(
        &mut self,
        ui: &mut egui::Ui,
        type_id: &str,
        actions: &mut Vec<AssetBrowserAction>,
    ) {
        egui::ScrollArea::vertical()
            .id_source(type_id)
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    let kind = match type_id {
                        "bg" => AssetImportKind::Background,
                        "char" => AssetImportKind::Character,
                        _ => return,
                    };
                    let assets: Vec<(String, PathBuf)> = match type_id {
                        "bg" => self
                            .manifest
                            .assets
                            .backgrounds
                            .iter()
                            .map(|(name, path)| (name.clone(), path.clone()))
                            .collect(),
                        "char" => self
                            .manifest
                            .assets
                            .characters
                            .iter()
                            .map(|(name, asset)| (name.clone(), asset.path.clone()))
                            .collect(),
                        _ => Vec::new(),
                    };

                    for (name, path) in assets {
                        ui.vertical(|ui| {
                            let response = self.render_image_asset_card(ui, type_id, &name, &path);

                            let value = match type_id {
                                "bg" => normalize_asset_path(&path.to_string_lossy()),
                                "char" => name.clone(),
                                _ => name.clone(),
                            };
                            if response.drag_started() {
                                let asset_path = normalize_asset_path(&path.to_string_lossy());
                                let payload = asset_drag_payload(type_id, &value, &asset_path);
                                ui.memory_mut(|mem| {
                                    mem.data
                                        .insert_temp(egui::Id::new("dragged_asset"), payload)
                                });
                            }

                            response.on_hover_text(format!("Drag to scene\nPath: {:?}", path));
                            if ui.small_button("Remove").clicked() {
                                actions.push(AssetBrowserAction::Remove {
                                    kind,
                                    name: name.clone(),
                                });
                            }
                        });
                    }
                });
            });
    }

    fn render_image_asset_card(
        &mut self,
        ui: &mut egui::Ui,
        type_id: &str,
        name: &str,
        path: &Path,
    ) -> egui::Response {
        let card_size = egui::vec2(96.0, 116.0);
        let (rect, response) = ui.allocate_exact_size(card_size, egui::Sense::click_and_drag());
        ui.painter()
            .rect_filled(rect, 4.0, egui::Color32::from_rgb(46, 46, 46));
        ui.painter().rect_stroke(
            rect,
            4.0,
            egui::Stroke::new(1.0, egui::Color32::from_gray(72)),
        );

        let asset_path = normalize_asset_path(&path.to_string_lossy());
        let image_rect =
            egui::Rect::from_min_size(rect.min + egui::vec2(6.0, 6.0), egui::vec2(84.0, 72.0));
        if let Some(texture_id) = self.thumbnail_texture(ui.ctx(), &asset_path) {
            ui.painter().image(
                texture_id,
                image_rect,
                egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
                egui::Color32::WHITE,
            );
        } else {
            ui.painter()
                .rect_filled(image_rect, 2.0, egui::Color32::from_rgb(66, 74, 88));
            ui.painter().text(
                image_rect.center(),
                egui::Align2::CENTER_CENTER,
                if type_id == "bg" { "BG" } else { "CHAR" },
                egui::FontId::proportional(13.0),
                egui::Color32::from_gray(210),
            );
        }

        ui.painter().text(
            egui::pos2(rect.left() + 6.0, rect.bottom() - 30.0),
            egui::Align2::LEFT_TOP,
            truncate_label(name, 16),
            egui::FontId::proportional(12.0),
            egui::Color32::from_gray(220),
        );
        response
    }

    fn thumbnail_texture(
        &mut self,
        ctx: &egui::Context,
        asset_path: &str,
    ) -> Option<egui::TextureId> {
        let project_root = self.project_root?;
        let request_cache_key = thumbnail_cache_key(project_root, asset_path);
        if let Some(texture) = self.image_cache.get(&request_cache_key) {
            return Some(texture.id());
        }
        if let Some(should_retry) = self
            .image_failures
            .get(&request_cache_key)
            .map(|failure| should_retry_missing_image_failure(failure, project_root, asset_path))
        {
            if should_retry {
                self.image_failures.remove(&request_cache_key);
            } else {
                return None;
            }
        }
        self.asset_store(project_root)?;
        let resolved_asset_path = match self.asset_store.as_ref()?.resolve_image_path(asset_path) {
            Ok(path) => normalize_asset_path(&path),
            Err(err) => {
                self.image_failures
                    .insert(request_cache_key, image_failure_message(asset_path, &err));
                return None;
            }
        };
        let cache_key = thumbnail_cache_key(project_root, &resolved_asset_path);
        if let Some(texture) = self.image_cache.get(&cache_key) {
            let texture = texture.clone();
            if request_cache_key != cache_key {
                self.image_cache.insert(request_cache_key, texture.clone());
            }
            return Some(texture.id());
        }
        if self.image_failures.contains_key(&cache_key) {
            return None;
        }

        let image = match self.asset_store.as_ref()?.load_image(&resolved_asset_path) {
            Ok(image) => image,
            Err(err) => {
                self.image_failures
                    .insert(cache_key, image_failure_message(&resolved_asset_path, &err));
                return None;
            }
        };
        let (size, pixels) = PreviewQuality::Draft.scaled_image(image.size, &image.pixels);
        let texture = ctx.load_texture(
            format!("asset_browser::{cache_key}"),
            egui::ColorImage::from_rgba_unmultiplied(size, pixels.as_ref()),
            egui::TextureOptions::LINEAR,
        );
        let id = texture.id();
        if request_cache_key != cache_key {
            self.image_cache.insert(request_cache_key, texture.clone());
        }
        self.image_cache.insert(cache_key, texture);
        Some(id)
    }

    fn asset_store(&mut self, project_root: &Path) -> Option<&vnengine_assets::AssetStore> {
        if self.asset_store.is_none() {
            self.asset_store = vnengine_assets::AssetStore::new(
                project_root.to_path_buf(),
                vnengine_assets::SecurityMode::Trusted,
                None,
                false,
            )
            .ok();
        }
        self.asset_store.as_ref()
    }

    fn render_audio_position(&mut self, ui: &mut egui::Ui, asset_path: &str) -> u64 {
        let duration = self.audio_duration_secs(asset_path);
        let slider_id = egui::Id::new(("asset_audio_offset", asset_path.to_string()));
        let mut offset_secs = ui
            .data_mut(|data| data.get_persisted::<f32>(slider_id))
            .unwrap_or(0.0);

        if let Some(duration_secs) = duration.filter(|value| *value > 0.0) {
            offset_secs = offset_secs.clamp(0.0, duration_secs);
            let slider_label = format_audio_position(offset_secs, duration_secs);
            ui.add(
                egui::Slider::new(&mut offset_secs, 0.0..=duration_secs)
                    .show_value(false)
                    .text(slider_label),
            );
            ui.data_mut(|data| data.insert_persisted(slider_id, offset_secs));
            secs_to_ms(offset_secs)
        } else {
            ui.label("Duration --:--");
            0
        }
    }

    fn audio_duration_secs(&mut self, asset_path: &str) -> Option<f32> {
        if let Some(cached) = self.audio_duration_cache.get(asset_path) {
            return *cached;
        }
        let duration = if let Some(root) = self.project_root {
            self.asset_store(root)
                .and_then(|store| visual_novel_runtime::audio_duration(store, asset_path).ok())
                .flatten()
                .map(|duration| duration.as_secs_f32())
        } else {
            None
        };
        self.audio_duration_cache
            .insert(asset_path.to_string(), duration);
        duration
    }
}

fn asset_drag_payload(type_id: &str, value: &str, asset_path: &str) -> String {
    if type_id == "char" {
        format!("asset://char/{value}\n{asset_path}")
    } else {
        format!("asset://{type_id}/{value}")
    }
}

fn truncate_label(label: &str, max_chars: usize) -> String {
    let mut value = label.chars().take(max_chars).collect::<String>();
    if label.chars().count() > max_chars {
        value.push_str("...");
    }
    value
}

fn format_audio_position(offset_secs: f32, duration_secs: f32) -> String {
    format!(
        "{} / {}",
        format_duration_secs(offset_secs),
        format_duration_secs(duration_secs)
    )
}

fn format_duration_secs(seconds: f32) -> String {
    let total = seconds.max(0.0).round() as u64;
    let minutes = total / 60;
    let seconds = total % 60;
    format!("{minutes}:{seconds:02}")
}

fn secs_to_ms(seconds: f32) -> u64 {
    if seconds.is_finite() {
        (seconds.max(0.0) * 1000.0).round() as u64
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn character_drag_payload_keeps_name_and_path() {
        let payload = super::asset_drag_payload("char", "furina", "assets/characters/furina.png");
        assert_eq!(payload, "asset://char/furina\nassets/characters/furina.png");
    }

    #[test]
    fn thumbnail_cache_keys_do_not_overlap_scene_cache() {
        let root = std::path::Path::new("C:/project-one");
        let asset_path = "assets/bg/room.png";
        let thumbnail = super::thumbnail_cache_key(root, asset_path);
        let stage = crate::editor::image_asset_cache::scene_stage_cache_key(
            root,
            crate::editor::PreviewQuality::Draft,
            asset_path,
        );
        assert!(thumbnail.starts_with("asset_browser::thumb::C:/project-one::assets/bg/room.png::"));
        assert_ne!(thumbnail, stage);
    }

    #[test]
    fn thumbnail_cache_keys_include_project_root() {
        let asset_path = "assets/bg/room.png";
        assert_ne!(
            super::thumbnail_cache_key(std::path::Path::new("C:/project-one"), asset_path),
            super::thumbnail_cache_key(std::path::Path::new("C:/project-two"), asset_path)
        );
    }

    #[test]
    fn thumbnail_cache_can_dedupe_equivalent_resolved_asset_paths() {
        let temp = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir_all(temp.path().join("assets/bg")).expect("mkdir assets");
        std::fs::write(temp.path().join("assets/bg/room.png"), b"placeholder")
            .expect("write asset");
        let store = vnengine_assets::AssetStore::new(
            temp.path().to_path_buf(),
            vnengine_assets::SecurityMode::Trusted,
            None,
            false,
        )
        .expect("asset store");

        let resolved_short = store
            .resolve_image_path("bg/room")
            .expect("short path should resolve");
        let resolved_full = store
            .resolve_image_path("assets/bg/room.png")
            .expect("full path should resolve");

        assert_eq!(resolved_short, resolved_full);
        assert_eq!(
            super::thumbnail_cache_key(temp.path(), &resolved_short),
            super::thumbnail_cache_key(temp.path(), &resolved_full)
        );
    }

    #[test]
    fn audio_position_format_clamps_to_minutes_and_seconds() {
        assert_eq!(super::format_audio_position(-4.0, 61.4), "0:00 / 1:01");
        assert_eq!(super::format_audio_position(125.0, 3661.0), "2:05 / 61:01");
    }

    #[test]
    fn audio_preview_offset_contract_is_integer_ms_and_finite() {
        assert_eq!(super::secs_to_ms(1.234), 1234);
        assert_eq!(super::secs_to_ms(-2.0), 0);
        assert_eq!(super::secs_to_ms(f32::NAN), 0);
        assert_eq!(super::secs_to_ms(f32::INFINITY), 0);

        let action = super::AssetBrowserAction::PreviewAudio {
            path: "audio/theme.ogg".to_string(),
            offset_ms: 1234,
        };
        assert_eq!(
            action,
            super::AssetBrowserAction::PreviewAudio {
                path: "audio/theme.ogg".to_string(),
                offset_ms: 1234
            }
        );
    }
}
