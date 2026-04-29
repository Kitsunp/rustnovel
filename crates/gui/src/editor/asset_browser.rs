use std::collections::HashMap;
use std::path::{Path, PathBuf};

use eframe::egui;
use visual_novel_engine::manifest::ProjectManifest;

use crate::editor::{AssetImportKind, PreviewQuality};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AssetBrowserAction {
    Import(AssetImportKind),
    PreviewAudio { path: String },
    StopAudio,
}

pub struct AssetBrowserPanel<'a> {
    pub manifest: &'a ProjectManifest,
    project_root: Option<&'a Path>,
    image_cache: &'a mut HashMap<String, egui::TextureHandle>,
    image_failures: &'a mut HashMap<String, String>,
    asset_store: Option<vnengine_assets::AssetStore>,
}

impl<'a> AssetBrowserPanel<'a> {
    pub fn new(
        manifest: &'a ProjectManifest,
        project_root: Option<&'a Path>,
        image_cache: &'a mut HashMap<String, egui::TextureHandle>,
        image_failures: &'a mut HashMap<String, String>,
    ) -> Self {
        Self {
            manifest,
            project_root,
            image_cache,
            image_failures,
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
                    self.render_grid(ui, "bg");
                }
            });

            ui.collapsing("Characters", |ui| {
                if self.manifest.assets.characters.is_empty() {
                    ui.label("No characters in manifest");
                } else {
                    self.render_grid(ui, "char");
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
                            if ui.small_button("Preview").clicked() {
                                actions.push(AssetBrowserAction::PreviewAudio {
                                    path: asset_path.clone(),
                                });
                            }
                            if ui.small_button("Stop").clicked() {
                                actions.push(AssetBrowserAction::StopAudio);
                            }
                        });
                    }
                }
            });
        });

        actions
    }

    fn render_grid(&mut self, ui: &mut egui::Ui, type_id: &str) {
        egui::ScrollArea::vertical()
            .id_source(type_id)
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
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
        let cache_key = thumbnail_cache_key(asset_path);
        if let Some(texture) = self.image_cache.get(&cache_key) {
            return Some(texture.id());
        }
        if self.image_failures.contains_key(&cache_key) {
            return None;
        }
        let Some(project_root) = self.project_root else {
            return None;
        };
        let image = match self.asset_store(project_root)?.load_image(asset_path) {
            Ok(image) => image,
            Err(err) => {
                self.image_failures.insert(cache_key, err.to_string());
                return None;
            }
        };
        let (size, pixels) = PreviewQuality::Draft.scaled_image(image.size, &image.pixels);
        let texture = ctx.load_texture(
            format!("asset_browser::{asset_path}"),
            egui::ColorImage::from_rgba_unmultiplied(size, pixels.as_ref()),
            egui::TextureOptions::LINEAR,
        );
        let id = texture.id();
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
}

fn asset_drag_payload(type_id: &str, value: &str, asset_path: &str) -> String {
    if type_id == "char" {
        format!("asset://char/{value}\n{asset_path}")
    } else {
        format!("asset://{type_id}/{value}")
    }
}

fn thumbnail_cache_key(asset_path: &str) -> String {
    format!("asset_browser::thumb::{asset_path}")
}

fn normalize_asset_path(path: &str) -> String {
    path.replace('\\', "/")
}

fn truncate_label(label: &str, max_chars: usize) -> String {
    let mut value = label.chars().take(max_chars).collect::<String>();
    if label.chars().count() > max_chars {
        value.push_str("...");
    }
    value
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
        assert_eq!(
            super::thumbnail_cache_key("assets/bg/room.png"),
            "asset_browser::thumb::assets/bg/room.png"
        );
    }
}
