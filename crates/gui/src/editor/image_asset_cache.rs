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
    let Ok(rel) = sanitize_candidate(asset_path) else {
        return "unsafe".to_string();
    };
    let metadata = std::fs::metadata(project_root.join(&rel)).ok().or_else(|| {
        crate::editor::asset_candidates::candidate_asset_paths(asset_path, &["png", "jpg", "jpeg"])
            .into_iter()
            .filter_map(|candidate| sanitize_candidate(&candidate).ok())
            .find_map(|candidate| std::fs::metadata(project_root.join(candidate)).ok())
    });
    let Some(metadata) = metadata else {
        return "missing".to_string();
    };
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
    crate::editor::asset_candidates::candidate_asset_paths(asset_path, &["png", "jpg", "jpeg"])
        .into_iter()
        .filter_map(|candidate| sanitize_candidate(&candidate).ok())
        .any(|rel| project_root.join(rel).is_file())
}

fn sanitize_candidate(candidate: &str) -> Result<PathBuf, vnengine_assets::AssetError> {
    vnengine_assets::sanitize_rel_path(Path::new(candidate))
}
