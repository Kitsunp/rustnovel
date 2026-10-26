use std::collections::HashMap;
use std::path::Path;

use eframe::egui;

use super::{
    image_failure_message, normalize_asset_path, prune_image_cache_versions, scene_stage_cache_key,
    should_retry_missing_image_failure, thumbnail_cache_key, MAX_IMAGE_CACHE_ENTRIES,
};
use crate::editor::{resource_service::EditorResourceService, PreviewQuality};

#[derive(Clone, Copy)]
pub(in crate::editor) enum ImageView {
    Thumbnail,
    Scene(PreviewQuality),
}

impl ImageView {
    fn key(self, root: &Path, path: &str) -> String {
        match self {
            Self::Thumbnail => thumbnail_cache_key(root, path),
            Self::Scene(quality) => scene_stage_cache_key(root, quality, path),
        }
    }

    fn quality(self) -> PreviewQuality {
        match self {
            Self::Thumbnail => PreviewQuality::Draft,
            Self::Scene(quality) => quality,
        }
    }
}

/// Both editor views use the same version, alias, failure and retention rules.
pub(in crate::editor) struct ImageTextureCache<'a> {
    cache: &'a mut HashMap<String, egui::TextureHandle>,
    failures: &'a mut HashMap<String, String>,
    resources: &'a mut EditorResourceService,
}

impl<'a> ImageTextureCache<'a> {
    pub fn new(
        cache: &'a mut HashMap<String, egui::TextureHandle>,
        failures: &'a mut HashMap<String, String>,
        resources: &'a mut EditorResourceService,
    ) -> Self {
        Self {
            cache,
            failures,
            resources,
        }
    }

    pub fn texture(
        &mut self,
        ctx: &egui::Context,
        root: &Path,
        path: &str,
        view: ImageView,
    ) -> Option<egui::TextureId> {
        let path = normalize_asset_path(path);
        let request_key = view.key(root, &path);
        prune_image_cache_versions(self.cache, &request_key);
        prune_image_cache_versions(self.failures, &request_key);
        if let Some(texture) = self.cache.get(&request_key) {
            self.resources
                .record_texture_use(texture.id(), ctx.frame_nr());
            return Some(texture.id());
        }
        if let Some(failure) = self.failures.get(&request_key) {
            if !should_retry_missing_image_failure(failure, root, &path) {
                return None;
            }
            self.failures.remove(&request_key);
        }

        let resolved = vnengine_assets::AssetStore::new(
            root.to_path_buf(),
            vnengine_assets::SecurityMode::Trusted,
            None,
            false,
        )
        .and_then(|store| store.resolve_image_path(&path));
        let resolved = match resolved {
            Ok(resolved) => normalize_asset_path(&resolved),
            Err(error) => {
                self.record_failure(request_key, image_failure_message(&path, &error));
                return None;
            }
        };
        let key = view.key(root, &resolved);
        prune_image_cache_versions(self.cache, &key);
        prune_image_cache_versions(self.failures, &key);
        let texture = if let Some(texture) = self.cache.get(&key) {
            texture.clone()
        } else {
            if self.failures.contains_key(&key) {
                return None;
            }
            let image = match self
                .resources
                .image_for_view_shared(root, &resolved, "editor")
            {
                Ok(image) => image,
                Err(error) => {
                    self.record_failure(key, format!("image '{resolved}' load failed: {error}"));
                    return None;
                }
            };
            let (size, pixels) = view.quality().scaled_image(image.size, &image.pixels);
            let options = match view {
                ImageView::Thumbnail => egui::TextureOptions::LINEAR,
                ImageView::Scene(quality) => quality.texture_options(),
            };
            ctx.load_texture(
                key.clone(),
                egui::ColorImage::from_rgba_unmultiplied(size, pixels.as_ref()),
                options,
            )
        };
        let id = texture.id();
        self.resources.record_texture_use(id, ctx.frame_nr());
        if request_key != key {
            self.cache.insert(request_key, texture.clone());
        }
        self.cache.insert(key, texture);
        Some(id)
    }

    fn record_failure(&mut self, key: String, error: String) {
        if self.failures.len() >= MAX_IMAGE_CACHE_ENTRIES && !self.failures.contains_key(&key) {
            if let Some(cached_key) = self.failures.keys().next().cloned() {
                self.failures.remove(&cached_key);
            }
        }
        self.failures.insert(key, error);
    }
}
