use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::cache::ByteCache;
use crate::helpers::{
    candidate_image_paths, canonicalize_within_root, normalize_asset_key, normalize_asset_request,
    sanitize_rel_path, sha256_hex,
};
use crate::model::{
    AssetError, AssetLimits, AssetManifest, LoadedImage, SecurityMode, SUPPORTED_IMAGE_EXTENSIONS,
};

#[derive(Debug)]
pub struct AssetStore {
    root: PathBuf,
    mode: SecurityMode,
    allowed_image_extensions: HashSet<String>,
    limits: AssetLimits,
    manifest: Option<AssetManifest>,
    require_manifest: bool,
    byte_cache: Mutex<ByteCache>,
}

impl AssetStore {
    pub fn new(
        root: PathBuf,
        mode: SecurityMode,
        manifest_path: Option<PathBuf>,
        require_manifest: bool,
    ) -> Result<Self, AssetError> {
        let manifest = match manifest_path {
            Some(path) => {
                let raw = fs::read_to_string(path)?;
                let mut manifest: AssetManifest = serde_json::from_str(&raw)
                    .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err))?;
                if manifest.manifest_version != 1 {
                    return Err(AssetError::ManifestVersion(manifest.manifest_version));
                }
                manifest.assets = manifest
                    .assets
                    .into_iter()
                    .map(|(raw_key, entry)| {
                        let rel = sanitize_rel_path(Path::new(&raw_key))?;
                        Ok((normalize_asset_key(&rel), entry))
                    })
                    .collect::<Result<BTreeMap<_, _>, AssetError>>()?;
                Some(manifest)
            }
            None => None,
        };
        let allowed_image_extensions = SUPPORTED_IMAGE_EXTENSIONS
            .into_iter()
            .map(|ext| ext.to_string())
            .collect();
        Ok(Self {
            root,
            mode,
            allowed_image_extensions,
            limits: AssetLimits::default(),
            manifest,
            require_manifest,
            byte_cache: Mutex::new(ByteCache::new(64 * 1024 * 1024)),
        })
    }

    pub fn with_limits(mut self, limits: AssetLimits) -> Self {
        self.limits = limits;
        self
    }

    pub fn with_cache_budget(mut self, budget_bytes: usize) -> Self {
        self.byte_cache = Mutex::new(ByteCache::new(budget_bytes));
        self
    }

    pub fn load_bytes(&self, asset_path: &str) -> Result<Vec<u8>, AssetError> {
        let normalized = normalize_asset_request(asset_path);
        let rel = sanitize_rel_path(Path::new(&normalized))?;
        let cache_key = normalize_asset_key(&rel);

        if let Some(bytes) = self
            .byte_cache
            .lock()
            .map_err(|_| std::io::Error::other("asset cache lock poisoned"))?
            .get(&cache_key)
        {
            return Ok(bytes);
        }

        let full_path = canonicalize_within_root(&self.root, &rel)?;

        let bytes = fs::read(&full_path)?;
        let size = bytes.len() as u64;
        if size > self.limits.max_bytes {
            return Err(AssetError::TooLarge {
                size,
                max: self.limits.max_bytes,
            });
        }
        self.verify_manifest(&cache_key, size, &bytes)?;
        self.byte_cache
            .lock()
            .map_err(|_| std::io::Error::other("asset cache lock poisoned"))?
            .insert(cache_key, bytes.clone());
        Ok(bytes)
    }

    pub fn load_image(&self, asset_path: &str) -> Result<LoadedImage, AssetError> {
        let resolved_path = self.resolve_image_path(asset_path)?;
        let bytes = self.load_bytes(&resolved_path)?;

        let image = image::load_from_memory(&bytes).map_err(|err| AssetError::Decode {
            path: resolved_path.clone(),
            reason: err.to_string(),
        })?;
        let rgba = image.to_rgba8();
        let (width, height) = (rgba.width(), rgba.height());
        if width > self.limits.max_width || height > self.limits.max_height {
            return Err(AssetError::InvalidDimensions {
                width,
                height,
                max_width: self.limits.max_width,
                max_height: self.limits.max_height,
            });
        }
        Ok(LoadedImage {
            name: resolved_path,
            size: [width as usize, height as usize],
            pixels: rgba.into_raw(),
        })
    }

    pub fn resolve_image_path(&self, asset_path: &str) -> Result<String, AssetError> {
        let normalized = normalize_asset_request(asset_path);
        let rel = sanitize_rel_path(Path::new(&normalized))?;
        if let Some(extension) = rel.extension().and_then(|ext| ext.to_str()) {
            let extension = extension.to_ascii_lowercase();
            if !self.allowed_image_extensions.contains(&extension) {
                return Err(AssetError::UnsupportedExtension(asset_path.to_string()));
            }
        }

        let canonical_root = self.root.canonicalize()?;
        let mut attempted = Vec::new();

        for candidate in candidate_image_paths(&normalized) {
            attempted.push(candidate.clone());
            let rel = sanitize_rel_path(Path::new(&candidate))?;
            let full_path = self.root.join(&rel);
            match full_path.canonicalize() {
                Ok(canonical_path) => {
                    if canonical_path.starts_with(&canonical_root) {
                        return Ok(candidate);
                    }
                    return Err(AssetError::Traversal);
                }
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => continue,
                Err(err) => return Err(AssetError::Io(err)),
            }
        }

        Err(AssetError::ImageNotFound {
            requested: asset_path.to_string(),
            attempts: attempted,
        })
    }

    fn verify_manifest(&self, asset_key: &str, size: u64, bytes: &[u8]) -> Result<(), AssetError> {
        if self.mode == SecurityMode::Untrusted && self.require_manifest && self.manifest.is_none()
        {
            return Err(AssetError::ManifestMissing);
        }
        let Some(manifest) = &self.manifest else {
            return Ok(());
        };
        let entry = manifest
            .assets
            .get(asset_key)
            .ok_or_else(|| AssetError::ManifestEntryMissing(asset_key.to_string()))?;
        if entry.size != size {
            return Err(AssetError::ManifestSizeMismatch(asset_key.to_string()));
        }
        let expected = entry.sha256.to_lowercase();
        let actual = sha256_hex(bytes);
        if expected != actual {
            return Err(AssetError::ManifestHashMismatch(asset_key.to_string()));
        }
        Ok(())
    }
}
