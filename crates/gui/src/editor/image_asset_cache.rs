use std::collections::{HashMap, HashSet};
use std::fs::{self, Metadata};
use std::path::{Path, PathBuf};

use crate::editor::PreviewQuality;
use eframe::egui;

const MISSING_IMAGE_PREFIX: &str = "missing image:";
pub(super) const MAX_ASSET_CACHE_ENTRIES: usize = 512;

mod texture_loader;
pub(super) use texture_loader::{ImageTextureCache, ImageView};

pub(super) fn record_asset_failure(
    failures: &mut HashMap<String, String>,
    key: String,
    error: String,
) {
    failures.insert(key.clone(), error);
    while failures.len() > MAX_ASSET_CACHE_ENTRIES {
        let Some(expired) = failures.keys().find(|cached| **cached != key).cloned() else {
            break;
        };
        failures.remove(&expired);
    }
}

/// Shared retention policy for thumbnails, the composer and player preview.
/// A frame's working set stays alive until painting has finished, even when
/// it exceeds the retention budget. Aliases do not duplicate texture bytes.
pub(super) struct EditorTextureRetention {
    last_used: HashMap<egui::TextureId, u64>,
    budget_bytes: usize,
}

impl Default for EditorTextureRetention {
    fn default() -> Self {
        Self {
            last_used: HashMap::new(),
            budget_bytes: 128 * 1024 * 1024,
        }
    }
}

impl EditorTextureRetention {
    pub(super) fn clear(&mut self) {
        self.last_used.clear();
    }

    pub(super) fn record_use(&mut self, id: egui::TextureId, frame: u64) {
        self.last_used.insert(id, frame);
    }

    pub(super) fn trim(&mut self, cache: &mut HashMap<String, egui::TextureHandle>, frame: u64) {
        let mut textures = HashMap::<_, (usize, usize)>::new();
        for texture in cache.values() {
            let size = texture.size();
            let bytes = size[0].saturating_mul(size[1]).saturating_mul(4);
            let entry = textures.entry(texture.id()).or_insert((bytes, 0));
            entry.1 += 1;
        }
        self.last_used.retain(|id, _| textures.contains_key(id));
        let mut bytes = textures
            .values()
            .map(|entry| entry.0)
            .fold(0usize, usize::saturating_add);
        let mut entries = cache.len();
        if bytes <= self.budget_bytes && entries <= MAX_ASSET_CACHE_ENTRIES {
            return;
        }
        let mut candidates: Vec<_> = textures
            .keys()
            .filter_map(|id| {
                let used = self.last_used.get(id).copied();
                (used != Some(frame)).then_some((*id, used))
            })
            .collect();
        candidates.sort_by_key(|(_, used)| *used);
        let mut evicted = HashSet::new();
        for (id, _) in candidates {
            if bytes <= self.budget_bytes && entries <= MAX_ASSET_CACHE_ENTRIES {
                break;
            }
            let (texture_bytes, aliases) = textures[&id];
            bytes = bytes.saturating_sub(texture_bytes);
            entries -= aliases;
            evicted.insert(id);
        }
        cache.retain(|_, texture| !evicted.contains(&texture.id()));
        self.last_used.retain(|id, _| !evicted.contains(id));
    }
}

pub fn normalize_asset_path(path: &str) -> String {
    path.replace('\\', "/")
}

/// Release obsolete versions of a texture (including extensionless aliases)
/// before loading the current version in either editor view.
pub fn prune_image_cache_versions<T>(
    cache: &mut std::collections::HashMap<String, T>,
    current_key: &str,
) {
    let Some((identity, _)) = current_key.rsplit_once("::") else {
        return;
    };
    cache.retain(|key, _| {
        key == current_key
            || key
                .rsplit_once("::")
                .is_none_or(|(other, _)| other != identity)
    });
}

pub fn scene_stage_cache_key(
    project_root: &Path,
    preview_quality: PreviewQuality,
    asset_path: &str,
) -> String {
    let version = image_asset_version(project_root, asset_path);
    format!(
        "scene_stage::{}::{}::{}::{}",
        normalize_asset_path(&project_root.display().to_string()),
        preview_quality.label(),
        normalize_asset_path(asset_path),
        version
    )
}

pub fn thumbnail_cache_key(project_root: &Path, asset_path: &str) -> String {
    let version = image_asset_version(project_root, asset_path);
    format!(
        "asset_browser::thumb::{}::{}::{}",
        normalize_asset_path(&project_root.display().to_string()),
        normalize_asset_path(asset_path),
        version
    )
}

pub fn image_asset_version(project_root: &Path, asset_path: &str) -> String {
    let metadata = match image_candidate_metadata(project_root, asset_path) {
        Ok(Some(metadata)) => metadata,
        Ok(None) => return "missing".to_string(),
        Err(vnengine_assets::AssetError::Traversal) => return "unsafe".to_string(),
        Err(vnengine_assets::AssetError::Io(err)) => {
            return format!("io-error:{:?}", err.kind());
        }
        Err(err) => return format!("error:{err}"),
    };
    if !metadata.is_file() {
        return "missing".to_string();
    }
    let modified_nanos = metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    format!("{}:{modified_nanos}", metadata.len())
}

pub fn image_failure_message(asset_path: &str, err: &vnengine_assets::AssetError) -> String {
    match err {
        vnengine_assets::AssetError::ImageNotFound { .. } => {
            format!("{MISSING_IMAGE_PREFIX} {err}")
        }
        _ => format!("image '{asset_path}' load failed: {err}"),
    }
}

pub fn should_retry_missing_image_failure(
    failure: &str,
    project_root: &Path,
    asset_path: &str,
) -> bool {
    failure.starts_with(MISSING_IMAGE_PREFIX) && image_candidate_exists(project_root, asset_path)
}

fn image_candidate_exists(project_root: &Path, asset_path: &str) -> bool {
    matches!(
        image_candidate_metadata(project_root, asset_path),
        Ok(Some(_))
    )
}

fn image_candidate_metadata(
    project_root: &Path,
    asset_path: &str,
) -> Result<Option<Metadata>, vnengine_assets::AssetError> {
    let canonical_root = match project_root.canonicalize() {
        Ok(root) => root,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(vnengine_assets::AssetError::Io(err)),
    };
    let mut candidates = Vec::new();
    candidates.push(sanitize_candidate(asset_path)?);
    candidates.extend(
        crate::editor::asset_candidates::candidate_asset_paths(asset_path, &["png", "jpg", "jpeg"])
            .into_iter()
            .filter_map(|candidate| sanitize_candidate(&candidate).ok()),
    );
    for rel in candidates {
        match safe_candidate_metadata(project_root, &canonical_root, &rel)? {
            Some(metadata) if metadata.is_file() => return Ok(Some(metadata)),
            Some(_) | None => {}
        }
    }
    Ok(None)
}

fn safe_candidate_metadata(
    project_root: &Path,
    canonical_root: &Path,
    rel: &Path,
) -> Result<Option<Metadata>, vnengine_assets::AssetError> {
    let candidate_path = project_root.join(rel);
    match fs::symlink_metadata(&candidate_path) {
        Ok(_) => {}
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(vnengine_assets::AssetError::Io(err)),
    }
    let canonical_path = match candidate_path.canonicalize() {
        Ok(path) => path,
        Err(err) => return Err(vnengine_assets::AssetError::Io(err)),
    };
    if !canonical_path.starts_with(canonical_root) {
        return Err(vnengine_assets::AssetError::Traversal);
    }
    let metadata = fs::metadata(&canonical_path)?;
    Ok(Some(metadata))
}

fn sanitize_candidate(candidate: &str) -> Result<PathBuf, vnengine_assets::AssetError> {
    vnengine_assets::sanitize_rel_path(Path::new(candidate))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn texture_retention_counts_aliases_once_and_releases_inactive_working_sets() {
        let ctx = egui::Context::default();
        let mut cache = HashMap::new();
        let mut retention = EditorTextureRetention {
            budget_bytes: 8,
            ..Default::default()
        };
        let texture = |name| {
            ctx.load_texture(
                name,
                egui::ColorImage::new([1, 1], egui::Color32::RED),
                egui::TextureOptions::LINEAR,
            )
        };
        let first = texture("first");
        let first_id = first.id();
        cache.insert("first".into(), first.clone());
        cache.insert("alias".into(), first);
        let second = texture("second");
        let second_id = second.id();
        cache.insert("second".into(), second);
        retention.record_use(first_id, 0);
        retention.record_use(second_id, 0);
        retention.trim(&mut cache, 0);
        retention.record_use(second_id, 1);
        retention.trim(&mut cache, 1);
        assert_eq!(
            cache.len(),
            3,
            "two RGBA textures fit exactly, despite the alias"
        );

        retention.budget_bytes = 4;
        let output = ctx.run(Default::default(), |_| retention.trim(&mut cache, 1));
        assert_eq!(cache.len(), 1);
        assert_eq!(cache["second"].id(), second_id);
        assert!(output.textures_delta.free.contains(&first_id));
        assert!(!output.textures_delta.free.contains(&second_id));

        for frame in 2..100 {
            let current = texture("next-scene");
            let id = current.id();
            cache.insert(format!("scene-{frame}"), current);
            retention.record_use(id, frame);
            retention.trim(&mut cache, frame);
            assert_eq!(cache.len(), 1);
            assert_eq!(retention.last_used.len(), 1);
            assert_eq!(cache.values().next().unwrap().id(), id);
        }
        retention.budget_bytes = 0;
        retention.trim(&mut cache, 99);
        assert_eq!(
            cache.len(),
            1,
            "current drawing remains valid with a zero budget"
        );
        retention.trim(&mut cache, 100);
        assert!(cache.is_empty());
        assert!(retention.last_used.is_empty());

        retention.budget_bytes = usize::MAX;
        let shared = texture("many aliases");
        let shared_id = shared.id();
        for alias in 0..513 {
            cache.insert(format!("alias-{alias}"), shared.clone());
        }
        drop(shared);
        retention.record_use(shared_id, 100);
        retention.trim(&mut cache, 100);
        assert_eq!(cache.len(), 513, "active aliases are protected together");
        retention.trim(&mut cache, 101);
        assert!(
            cache.is_empty(),
            "the entry limit bounds even tiny textures and aliases"
        );
        assert!(retention.last_used.is_empty());
    }
}
