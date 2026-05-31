use std::fs::{self, Metadata};
use std::path::{Path, PathBuf};

use crate::editor::PreviewQuality;

const MISSING_IMAGE_PREFIX: &str = "missing image:";

pub fn normalize_asset_path(path: &str) -> String {
    path.replace('\\', "/")
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
