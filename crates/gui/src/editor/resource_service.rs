use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::time::Duration;

use visual_novel_runtime::{audio_duration, MemoryAssetStore};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ResourceMetrics {
    pub bytes: usize,
    pub hits: usize,
    pub misses: usize,
    pub evictions: usize,
    pub decode_ms: u128,
    pub upload_ms: u128,
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
    bytes: Vec<u8>,
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
        let path = resolve_project_file(project_root, rel_path)?;
        let bytes = fs::read(&path).map_err(|err| format!("read {}: {err}", path.display()))?;
        let fingerprint = fingerprint_bytes(&bytes);

        if let Some(cached) = self.byte_cache.get(&path) {
            if cached.fingerprint == fingerprint {
                self.metrics.hits = self.metrics.hits.saturating_add(1);
                return Ok(cached.bytes.clone());
            }
            self.metrics.evictions = self.metrics.evictions.saturating_add(1);
        }

        self.metrics.misses = self.metrics.misses.saturating_add(1);
        self.metrics.bytes = self.metrics.bytes.saturating_add(bytes.len());
        self.byte_cache.insert(
            path,
            CachedBytes {
                fingerprint,
                bytes: bytes.clone(),
            },
        );
        Ok(bytes)
    }

    pub fn image_bytes_for_view(
        &mut self,
        project_root: &Path,
        rel_path: &str,
        _view_id: &str,
    ) -> Result<Vec<u8>, String> {
        let before = self.metrics.misses;
        let bytes = self.load_bytes(project_root, rel_path)?;
        if self.metrics.misses != before {
            self.metrics.decode_ms = self.metrics.decode_ms.saturating_add(1);
        }
        Ok(bytes)
    }

    pub fn image_for_view(
        &mut self,
        project_root: &Path,
        rel_path: &str,
        _view_id: &str,
    ) -> Result<vnengine_assets::LoadedImage, String> {
        let path = resolve_project_file(project_root, rel_path)?;
        let bytes = self.load_bytes(project_root, rel_path)?;
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
        Ok(image)
    }

    pub fn audio_metadata(
        &mut self,
        project_root: &Path,
        rel_path: &str,
    ) -> Result<AudioMetadata, String> {
        let path = resolve_project_file(project_root, rel_path)?;
        let bytes = self.load_bytes(project_root, rel_path)?;
        let fingerprint = fingerprint_bytes(&bytes);

        if let Some(cached) = self.audio_metadata_cache.get(&path) {
            if cached.fingerprint == fingerprint {
                self.metrics.hits = self.metrics.hits.saturating_add(1);
                return Ok(cached.clone());
            }
            self.metrics.evictions = self.metrics.evictions.saturating_add(1);
        }

        let mut store = MemoryAssetStore::default();
        store.insert(rel_path.to_string(), bytes.clone());
        let metadata = AudioMetadata {
            duration: audio_duration(&store, rel_path)?,
            fingerprint,
            bytes: bytes.len(),
        };
        self.audio_metadata_cache.insert(path, metadata.clone());
        Ok(metadata)
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
    let mut hasher = DefaultHasher::new();
    bytes.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn asset_cache_multiview_stress() {
        let tmp = tempfile::tempdir().expect("temp dir");
        fs::create_dir_all(tmp.path().join("assets/backgrounds")).expect("asset dir");
        fs::write(
            tmp.path().join("assets/backgrounds/room.bin"),
            b"image-bytes",
        )
        .expect("asset");
        let mut service = EditorResourceService::new();

        for view in ["browser", "composer", "player"] {
            let bytes = service
                .image_bytes_for_view(tmp.path(), "assets/backgrounds/room.bin", view)
                .expect("image bytes");
            assert_eq!(bytes, b"image-bytes");
        }

        assert_eq!(service.metrics().misses, 1);
        assert_eq!(service.metrics().hits, 2);
        assert_eq!(service.metrics().decode_ms, 1);
    }

    #[test]
    fn decoded_image_cache_multiview_stress() {
        let tmp = tempfile::tempdir().expect("temp dir");
        fs::create_dir_all(tmp.path().join("assets/backgrounds")).expect("asset dir");
        fs::write(
            tmp.path().join("assets/backgrounds/room.png"),
            tiny_png([200, 20, 20, 255]),
        )
        .expect("png asset");
        let mut service = EditorResourceService::new();

        let first = service
            .image_for_view(tmp.path(), "assets/backgrounds/room.png", "browser")
            .expect("first decoded image");
        let decode_ms_after_first = service.metrics().decode_ms;
        assert_eq!(first.size, [1, 1]);

        for view in ["composer", "player"] {
            let image = service
                .image_for_view(tmp.path(), "assets/backgrounds/room.png", view)
                .expect("cached decoded image");
            assert_eq!(image.pixels, first.pixels);
        }

        assert_eq!(service.metrics().misses, 1);
        assert_eq!(service.metrics().hits, 2);
        assert!(decode_ms_after_first >= 1);
        assert_eq!(service.metrics().decode_ms, decode_ms_after_first);
    }

    #[test]
    fn asset_cache_fingerprint_invalidation() {
        let tmp = tempfile::tempdir().expect("temp dir");
        fs::create_dir_all(tmp.path().join("assets")).expect("asset dir");
        let path = tmp.path().join("assets/bg.bin");
        fs::write(&path, b"old").expect("old asset");
        let mut service = EditorResourceService::new();

        assert_eq!(
            service
                .load_bytes(tmp.path(), "assets/bg.bin")
                .expect("old bytes"),
            b"old"
        );
        fs::write(&path, b"new").expect("new asset");
        assert_eq!(
            service
                .load_bytes(tmp.path(), "assets/bg.bin")
                .expect("new bytes"),
            b"new"
        );
        assert_eq!(service.metrics().evictions, 1);
    }

    #[test]
    fn audio_metadata_invalidation() {
        let tmp = tempfile::tempdir().expect("temp dir");
        fs::create_dir_all(tmp.path().join("assets/audio")).expect("audio dir");
        let path = tmp.path().join("assets/audio/tone.wav");
        fs::write(&path, tiny_wav(Duration::from_millis(120), 8_000)).expect("wav");
        let mut service = EditorResourceService::new();

        let first = service
            .audio_metadata(tmp.path(), "assets/audio/tone.wav")
            .expect("first metadata");
        fs::write(&path, tiny_wav(Duration::from_millis(260), 8_000)).expect("wav updated");
        let second = service
            .audio_metadata(tmp.path(), "assets/audio/tone.wav")
            .expect("second metadata");

        assert_ne!(first.fingerprint, second.fingerprint);
        assert!(second.duration > first.duration);
        assert!(service.metrics().evictions >= 1);
    }

    #[test]
    fn decoded_image_cache_fingerprint_invalidation() {
        let tmp = tempfile::tempdir().expect("temp dir");
        fs::create_dir_all(tmp.path().join("assets")).expect("asset dir");
        let path = tmp.path().join("assets/bg.png");
        fs::write(&path, tiny_png([255, 0, 0, 255])).expect("old png");
        let mut service = EditorResourceService::new();

        let first = service
            .image_for_view(tmp.path(), "assets/bg.png", "browser")
            .expect("first image");
        fs::write(&path, tiny_png([0, 0, 255, 255])).expect("new png");
        let second = service
            .image_for_view(tmp.path(), "assets/bg.png", "browser")
            .expect("second image");

        assert_ne!(first.pixels, second.pixels);
        assert!(service.metrics().evictions >= 1);
    }

    fn tiny_wav(duration: Duration, sample_rate: u32) -> Vec<u8> {
        let samples = (duration.as_secs_f32() * sample_rate as f32).round() as u32;
        let data_len = samples * 2;
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&(36 + data_len).to_le_bytes());
        bytes.extend_from_slice(b"WAVEfmt ");
        bytes.extend_from_slice(&16u32.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&sample_rate.to_le_bytes());
        bytes.extend_from_slice(&(sample_rate * 2).to_le_bytes());
        bytes.extend_from_slice(&2u16.to_le_bytes());
        bytes.extend_from_slice(&16u16.to_le_bytes());
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&data_len.to_le_bytes());
        bytes.resize(bytes.len() + data_len as usize, 0);
        bytes
    }

    fn tiny_png(rgba: [u8; 4]) -> Vec<u8> {
        let image = image::RgbaImage::from_pixel(1, 1, image::Rgba(rgba));
        let mut cursor = std::io::Cursor::new(Vec::new());
        image
            .write_to(&mut cursor, image::ImageOutputFormat::Png)
            .expect("encode png");
        cursor.into_inner()
    }
}
