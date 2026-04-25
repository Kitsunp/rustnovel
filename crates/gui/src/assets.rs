use std::collections::HashMap;

use eframe::egui;
pub use vnengine_assets::{sanitize_rel_path, AssetError, AssetManifest, AssetStore, SecurityMode};

#[derive(Clone, Debug, Default)]
pub struct CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
    pub entries: usize,
    pub bytes: usize,
    pub budget_bytes: usize,
}

struct CachedTexture {
    texture: egui::TextureHandle,
    bytes: usize,
    last_used: u64,
}

pub struct AssetManager {
    store: AssetStore,
    cache: HashMap<String, CachedTexture>,
    budget_bytes: usize,
    current_bytes: usize,
    usage_counter: u64,
    stats: CacheStats,
}

impl AssetManager {
    pub fn new(store: AssetStore, budget_bytes: usize) -> Self {
        let stats = CacheStats {
            budget_bytes,
            ..CacheStats::default()
        };
        Self {
            store,
            cache: HashMap::new(),
            budget_bytes,
            current_bytes: 0,
            usage_counter: 0,
            stats,
        }
    }

    pub fn stats(&self) -> CacheStats {
        CacheStats {
            entries: self.cache.len(),
            bytes: self.current_bytes,
            ..self.stats.clone()
        }
    }

    pub fn texture_for_asset(
        &mut self,
        ctx: &egui::Context,
        asset_path: &str,
    ) -> Result<Option<egui::TextureHandle>, AssetError> {
        self.usage_counter = self.usage_counter.wrapping_add(1);
        let cache_key = self.store.resolve_image_path(asset_path)?;
        if let Some(entry) = self.cache.get_mut(&cache_key) {
            entry.last_used = self.usage_counter;
            self.stats.hits += 1;
            return Ok(Some(entry.texture.clone()));
        }

        let loaded = self.store.load_image(&cache_key)?;
        let bytes = loaded.pixels.len();

        if bytes > self.budget_bytes {
            return Err(AssetError::BudgetExceeded {
                bytes,
                budget: self.budget_bytes,
            });
        }
        self.stats.misses += 1;
        while self.current_bytes + bytes > self.budget_bytes {
            if !self.evict_lru() {
                break;
            }
        }
        let texture = ctx.load_texture(
            loaded.name.clone(),
            egui::ColorImage::from_rgba_unmultiplied(loaded.size, &loaded.pixels),
            egui::TextureOptions::default(),
        );
        self.current_bytes += bytes;
        self.cache.insert(
            cache_key.clone(),
            CachedTexture {
                texture,
                bytes,
                last_used: self.usage_counter,
                // Note: LoadedImage from assets doesn't store validation info, but we verified it via load_image
            },
        );
        Ok(self
            .cache
            .get(&cache_key)
            .map(|entry| entry.texture.clone()))
    }

    fn evict_lru(&mut self) -> bool {
        let Some((key, _)) = self
            .cache
            .iter()
            .min_by_key(|(_, entry)| entry.last_used)
            .map(|(key, entry)| (key.clone(), entry.bytes))
        else {
            return false;
        };
        if let Some(entry) = self.cache.remove(&key) {
            self.current_bytes = self.current_bytes.saturating_sub(entry.bytes);
            self.stats.evictions += 1;
            return true;
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_png(path: &std::path::Path) {
        let image = image::RgbaImage::from_pixel(1, 1, image::Rgba([12, 34, 56, 255]));
        image.save(path).expect("write png");
    }

    #[test]
    fn texture_for_asset_reuses_normalized_cache_key() {
        let tmp = tempfile::tempdir().expect("temp dir");
        let root = tmp.path();
        std::fs::create_dir_all(root.join("assets/bg")).expect("asset dir");
        write_png(&root.join("assets/bg/portrait.png"));

        let store = AssetStore::new(root.to_path_buf(), SecurityMode::Trusted, None, false)
            .expect("asset store");
        let mut manager = AssetManager::new(store, 8 * 1024 * 1024);
        let ctx = egui::Context::default();

        let first = manager
            .texture_for_asset(&ctx, "bg/portrait")
            .expect("first lookup");
        let first = first.expect("texture should load");
        let stats_after_first = manager.stats();
        assert_eq!(stats_after_first.misses, 1);
        assert_eq!(stats_after_first.hits, 0);

        let second = manager
            .texture_for_asset(&ctx, "assets/bg/portrait.png")
            .expect("second lookup");
        let second = second.expect("texture should load");
        let stats_after_second = manager.stats();
        assert_eq!(stats_after_second.misses, 1);
        assert_eq!(stats_after_second.hits, 1);
        assert_eq!(first.id(), second.id());
    }
}
