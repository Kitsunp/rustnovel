use std::collections::HashMap;
use std::sync::Arc;

/// Asset store trait for runtime resource loading.
pub trait AssetStore {
    fn load_bytes(&self, id: &str) -> Result<Vec<u8>, String>;
}

impl<T: AssetStore + ?Sized> AssetStore for Arc<T> {
    fn load_bytes(&self, id: &str) -> Result<Vec<u8>, String> {
        (**self).load_bytes(id)
    }
}

/// In-memory asset store mock for testing.
#[derive(Default)]
pub struct MemoryAssetStore {
    assets: HashMap<String, Vec<u8>>,
}

impl MemoryAssetStore {
    pub fn insert(&mut self, id: impl Into<String>, data: Vec<u8>) {
        self.assets.insert(id.into(), data);
    }
}

impl AssetStore for MemoryAssetStore {
    fn load_bytes(&self, id: &str) -> Result<Vec<u8>, String> {
        self.assets
            .get(id)
            .cloned()
            .ok_or_else(|| format!("Asset not found: {}", id))
    }
}

// Adapt vnengine_assets::AssetStore to Runtime AssetStore trait
impl AssetStore for vnengine_assets::AssetStore {
    fn load_bytes(&self, id: &str) -> Result<Vec<u8>, String> {
        vnengine_assets::AssetStore::load_bytes(self, id).map_err(|e| e.to_string())
    }
}
