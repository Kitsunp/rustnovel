use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use sha2::{Digest, Sha256};
use visual_novel_runtime::{audio_duration, MemoryAssetStore};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ResourceMetrics {
    pub bytes: usize,
    pub peak_bytes: usize,
    pub hits: usize,
    pub misses: usize,
    pub evictions: usize,
    pub decode_ms: u128,
    pub upload_ms: u128,
    pub byte_entries: usize,
    pub image_entries: usize,
    pub audio_entries: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AudioMetadata {
    pub duration: Option<Duration>,
    pub fingerprint: String,
    pub bytes: usize,
}

#[derive(Clone, Debug)]
struct CachedBytes {
    fingerprint: String,
    bytes: Arc<[u8]>,
}

#[derive(Clone, Debug)]
struct CachedImage {
    fingerprint: String,
    image: vnengine_assets::LoadedImage,
}

#[derive(Default)]
pub struct EditorResourceService {
    byte_cache: HashMap<PathBuf, CachedBytes>,
    image_cache: HashMap<PathBuf, CachedImage>,
    audio_metadata_cache: HashMap<PathBuf, AudioMetadata>,
    metrics: ResourceMetrics,
}

impl EditorResourceService {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn metrics(&self) -> &ResourceMetrics {
        &self.metrics
    }

    pub fn clear(&mut self) {
        self.byte_cache.clear();
        self.image_cache.clear();
        self.audio_metadata_cache.clear();
        self.metrics = ResourceMetrics::default();
    }

    pub fn load_bytes(&mut self, project_root: &Path, rel_path: &str) -> Result<Vec<u8>, String> {
        Ok(self
            .load_bytes_shared(project_root, rel_path)?
            .as_ref()
            .to_vec())
    }

    pub fn load_bytes_shared(
        &mut self,
        project_root: &Path,
        rel_path: &str,
    ) -> Result<Arc<[u8]>, String> {
        let path = resolve_project_file(project_root, rel_path)?;
        let bytes = fs::read(&path).map_err(|err| format!("read {}: {err}", path.display()))?;
        let fingerprint = fingerprint_bytes(&bytes);

        if let Some(cached) = self.byte_cache.get(&path) {
            if cached.fingerprint == fingerprint {
                let bytes = Arc::clone(&cached.bytes);
                self.metrics.hits = self.metrics.hits.saturating_add(1);
                self.refresh_entry_counts();
                return Ok(bytes);
            }
            self.metrics.evictions = self.metrics.evictions.saturating_add(1);
            self.metrics.bytes = self.metrics.bytes.saturating_sub(cached.bytes.len());
        }

        self.metrics.misses = self.metrics.misses.saturating_add(1);
        self.metrics.bytes = self.metrics.bytes.saturating_add(bytes.len());
        self.metrics.peak_bytes = self.metrics.peak_bytes.max(self.metrics.bytes);
        let bytes: Arc<[u8]> = Arc::from(bytes);
        self.byte_cache.insert(
            path,
            CachedBytes {
                fingerprint,
                bytes: Arc::clone(&bytes),
            },
        );
        self.refresh_entry_counts();
        Ok(bytes)
    }

    pub fn image_bytes_for_view(
        &mut self,
        project_root: &Path,
        rel_path: &str,
        _view_id: &str,
    ) -> Result<Vec<u8>, String> {
        let before = self.metrics.misses;
        let bytes = self.load_bytes_shared(project_root, rel_path)?;
        if self.metrics.misses != before {
            self.metrics.decode_ms = self.metrics.decode_ms.saturating_add(1);
        }
        Ok(bytes.as_ref().to_vec())
    }

    pub fn image_for_view(
        &mut self,
        project_root: &Path,
        rel_path: &str,
        _view_id: &str,
    ) -> Result<vnengine_assets::LoadedImage, String> {
        let path = resolve_project_file(project_root, rel_path)?;
        let bytes = self.load_bytes_shared(project_root, rel_path)?;
        let fingerprint = fingerprint_bytes(&bytes);

        if let Some(cached) = self.image_cache.get(&path) {
            if cached.fingerprint == fingerprint {
                return Ok(cached.image.clone());
            }
            self.metrics.evictions = self.metrics.evictions.saturating_add(1);
        }

        let before = std::time::Instant::now();
        let image = vnengine_assets::decode_image_bytes(
            rel_path,
            &bytes,
            &vnengine_assets::AssetLimits::default(),
        )
        .map_err(|err| format!("decode image '{rel_path}': {err}"))?;
        self.metrics.decode_ms = self
            .metrics
            .decode_ms
            .saturating_add(before.elapsed().as_millis().max(1));
        self.image_cache.insert(
            path,
            CachedImage {
                fingerprint,
                image: image.clone(),
            },
        );
        self.refresh_entry_counts();
        Ok(image)
    }

    pub fn audio_metadata(
        &mut self,
        project_root: &Path,
        rel_path: &str,
    ) -> Result<AudioMetadata, String> {
        let path = resolve_project_file(project_root, rel_path)?;
        let bytes = self.load_bytes_shared(project_root, rel_path)?;
        let fingerprint = fingerprint_bytes(&bytes);

        if let Some(cached) = self.audio_metadata_cache.get(&path) {
            if cached.fingerprint == fingerprint {
                self.metrics.hits = self.metrics.hits.saturating_add(1);
                return Ok(cached.clone());
            }
            self.metrics.evictions = self.metrics.evictions.saturating_add(1);
        }

        let mut store = MemoryAssetStore::default();
        store.insert(rel_path.to_string(), bytes.as_ref().to_vec());
        let metadata = AudioMetadata {
            duration: audio_duration(&store, rel_path)?,
            fingerprint,
            bytes: bytes.len(),
        };
        self.audio_metadata_cache.insert(path, metadata.clone());
        self.refresh_entry_counts();
        Ok(metadata)
    }

    fn refresh_entry_counts(&mut self) {
        self.metrics.byte_entries = self.byte_cache.len();
        self.metrics.image_entries = self.image_cache.len();
        self.metrics.audio_entries = self.audio_metadata_cache.len();
    }
}

fn resolve_project_file(project_root: &Path, rel_path: &str) -> Result<PathBuf, String> {
    let rel = vnengine_assets::sanitize_rel_path(Path::new(rel_path))
        .map_err(|err| format!("unsafe asset path '{rel_path}': {err}"))?;
    let path = project_root.join(rel);
    let canonical_root = project_root
        .canonicalize()
        .map_err(|err| format!("canonicalize {}: {err}", project_root.display()))?;
    let canonical_path = path
        .canonicalize()
        .map_err(|err| format!("canonicalize {}: {err}", path.display()))?;
    if !canonical_path.starts_with(&canonical_root) {
        return Err(format!("asset path escapes project root: {rel_path}"));
    }
    Ok(canonical_path)
}

fn fingerprint_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex_lower(&hasher.finalize())
}

fn hex_lower(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}
