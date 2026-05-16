use std::path::{Path, PathBuf};

use crate::editor::PreviewQuality;

const MISSING_IMAGE_PREFIX: &str = "missing image:";

pub(crate) fn normalize_asset_path(path: &str) -> String {
    path.replace('\\', "/")
}

pub(crate) fn scene_stage_cache_key(
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

pub(crate) fn thumbnail_cache_key(project_root: &Path, asset_path: &str) -> String {
    let version = image_asset_version(project_root, asset_path);
    format!(
        "asset_browser::thumb::{}::{}::{}",
        normalize_asset_path(&project_root.display().to_string()),
        normalize_asset_path(asset_path),
        version
    )
}

pub(crate) fn image_asset_version(project_root: &Path, asset_path: &str) -> String {
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

pub(crate) fn image_failure_message(asset_path: &str, err: &vnengine_assets::AssetError) -> String {
    match err {
        vnengine_assets::AssetError::ImageNotFound { .. } => {
            format!("{MISSING_IMAGE_PREFIX} {err}")
        }
        _ => format!("image '{asset_path}' load failed: {err}"),
    }
}

pub(crate) fn should_retry_missing_image_failure(
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_image_failure_retries_only_after_candidate_exists() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path();
        let failure = format!("{MISSING_IMAGE_PREFIX} image asset not found");

        assert!(!should_retry_missing_image_failure(
            &failure,
            root,
            "assets/backgrounds/room.png"
        ));

        std::fs::create_dir_all(root.join("assets/backgrounds")).expect("mkdir assets");
        std::fs::write(root.join("assets/backgrounds/room.png"), b"not-a-real-png")
            .expect("write file");

        assert!(should_retry_missing_image_failure(
            &failure,
            root,
            "assets/backgrounds/room.png"
        ));
    }

    #[test]
    fn decode_failures_do_not_retry_every_frame() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path();
        std::fs::create_dir_all(root.join("assets/backgrounds")).expect("mkdir assets");
        std::fs::write(root.join("assets/backgrounds/room.png"), b"not-a-real-png")
            .expect("write file");

        assert!(!should_retry_missing_image_failure(
            "image 'assets/backgrounds/room.png' load failed: decode",
            root,
            "assets/backgrounds/room.png"
        ));
    }

    #[test]
    fn scene_stage_cache_key_dedupes_after_asset_store_resolution() {
        let temp = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir_all(temp.path().join("assets/backgrounds")).expect("mkdir assets");
        std::fs::write(
            temp.path().join("assets/backgrounds/room.png"),
            b"placeholder",
        )
        .expect("write asset");
        let store = vnengine_assets::AssetStore::new(
            temp.path().to_path_buf(),
            vnengine_assets::SecurityMode::Trusted,
            None,
            false,
        )
        .expect("asset store");

        let resolved_short = store
            .resolve_image_path("backgrounds/room")
            .expect("short path should resolve");
        let resolved_full = store
            .resolve_image_path("assets/backgrounds/room.png")
            .expect("full path should resolve");

        assert_eq!(resolved_short, resolved_full);
        assert_eq!(
            scene_stage_cache_key(temp.path(), PreviewQuality::Draft, &resolved_short),
            scene_stage_cache_key(temp.path(), PreviewQuality::Draft, &resolved_full)
        );
    }

    #[test]
    fn cache_key_changes_when_resolved_asset_contents_change() {
        let temp = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir_all(temp.path().join("assets/backgrounds")).expect("mkdir assets");
        let asset = temp.path().join("assets/backgrounds/room.png");
        std::fs::write(&asset, b"first").expect("write first asset");
        let before = scene_stage_cache_key(
            temp.path(),
            PreviewQuality::Draft,
            "assets/backgrounds/room.png",
        );

        std::fs::write(&asset, b"second-version").expect("write second asset");
        let after = scene_stage_cache_key(
            temp.path(),
            PreviewQuality::Draft,
            "assets/backgrounds/room.png",
        );

        assert_ne!(
            before, after,
            "replacing an imported image at the same path must invalidate cached textures"
        );
    }

    #[test]
    fn extensionless_cache_key_tracks_resolved_candidate_contents() {
        let temp = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir_all(temp.path().join("assets/backgrounds")).expect("mkdir assets");
        let asset = temp.path().join("assets/backgrounds/room.png");
        std::fs::write(&asset, b"first").expect("write first asset");
        let before = scene_stage_cache_key(temp.path(), PreviewQuality::Draft, "backgrounds/room");

        std::fs::write(&asset, b"second-version").expect("write second asset");
        let after = scene_stage_cache_key(temp.path(), PreviewQuality::Draft, "backgrounds/room");

        assert_ne!(
            before, after,
            "extensionless aliases must be versioned by the real candidate file"
        );
        assert!(
            !before.ends_with("::missing"),
            "existing extensionless assets should not be cached as permanently missing"
        );
    }
}
