use super::*;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

fn write_png(path: &Path) {
    let image = image::RgbaImage::from_pixel(1, 1, image::Rgba([12, 34, 56, 255]));
    image.save(path).expect("write png");
}

#[test]
fn load_image_rejects_unsupported_extension_before_io() {
    let store = AssetStore::new(PathBuf::from("."), SecurityMode::Trusted, None, false)
        .expect("asset store should initialize");

    let err = match store.load_image("assets/theme.ogg") {
        Ok(_) => panic!("non-image extension must be rejected"),
        Err(err) => err,
    };

    assert!(matches!(err, AssetError::UnsupportedExtension(_)));
}

#[test]
fn load_image_resolves_assets_prefix_and_extensionless_path() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock must be after unix epoch")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("vn_assets_image_resolve_{unique}"));
    std::fs::create_dir_all(root.join("assets/bg")).expect("asset dir");
    write_png(&root.join("assets/bg/portrait.png"));

    let store = AssetStore::new(root.clone(), SecurityMode::Trusted, None, false)
        .expect("asset store should initialize");

    let image = store
        .load_image("bg/portrait")
        .expect("image should resolve through assets prefix");
    assert_eq!(image.name, "assets/bg/portrait.png");
    assert_eq!(image.size, [1, 1]);
    assert_eq!(image.pixels.len(), 4);

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn load_image_reports_attempted_candidates_when_missing() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock must be after unix epoch")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("vn_assets_image_missing_{unique}"));
    std::fs::create_dir_all(&root).expect("root dir");

    let store = AssetStore::new(root.clone(), SecurityMode::Trusted, None, false)
        .expect("asset store should initialize");

    let err = match store.load_image("bg/portrait") {
        Ok(_) => panic!("missing image should report a structured error"),
        Err(err) => err,
    };
    match err {
        AssetError::ImageNotFound {
            requested,
            attempts,
        } => {
            assert_eq!(requested, "bg/portrait");
            assert!(attempts.iter().any(|item| item == "bg/portrait"));
            assert!(attempts.iter().any(|item| item == "assets/bg/portrait.png"));
        }
        other => panic!("unexpected error: {other:?}"),
    }

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn load_bytes_uses_cache_for_repeated_reads() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock must be after unix epoch")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("vn_assets_cache_{unique}"));
    std::fs::create_dir_all(&root).expect("temp root should be created");
    let asset_rel = PathBuf::from("audio").join("theme.ogg");
    let asset_path = root.join(&asset_rel);
    std::fs::create_dir_all(asset_path.parent().expect("parent path should exist"))
        .expect("asset parent directory should be created");
    std::fs::write(&asset_path, [1u8, 2, 3, 4]).expect("asset file should be written");

    let store = AssetStore::new(root.clone(), SecurityMode::Trusted, None, false)
        .expect("asset store should initialize")
        .with_cache_budget(1024);

    let first = store
        .load_bytes("audio/theme.ogg")
        .expect("first read should succeed");
    assert_eq!(first, vec![1, 2, 3, 4]);

    std::fs::remove_file(&asset_path).expect("asset file should be removed");

    let second = store
        .load_bytes("audio/theme.ogg")
        .expect("second read should be served from cache");
    assert_eq!(second, vec![1, 2, 3, 4]);

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn load_bytes_manifest_lookup_normalizes_separators() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock must be after unix epoch")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("vn_assets_manifest_norm_{unique}"));
    std::fs::create_dir_all(root.join("audio")).expect("audio dir");
    let payload = [4u8, 5, 6, 7];
    std::fs::write(root.join("audio").join("theme.ogg"), payload).expect("write asset");

    let mut manifest_assets = BTreeMap::new();
    manifest_assets.insert(
        "audio\\theme.ogg".to_string(),
        AssetEntry {
            sha256: sha256_hex(&payload),
            size: payload.len() as u64,
        },
    );
    let manifest = AssetManifest {
        manifest_version: 1,
        assets: manifest_assets,
    };
    let manifest_path = root.join("assets_manifest.json");
    std::fs::write(
        &manifest_path,
        serde_json::to_string_pretty(&manifest).expect("serialize manifest"),
    )
    .expect("write manifest");

    let store = AssetStore::new(
        root.clone(),
        SecurityMode::Untrusted,
        Some(manifest_path),
        true,
    )
    .expect("asset store");

    let bytes = store
        .load_bytes("audio/theme.ogg")
        .expect("normalized manifest key should resolve");
    assert_eq!(bytes, payload);

    let _ = std::fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn load_bytes_blocks_symlink_escape() {
    use std::os::unix::fs::symlink;

    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock must be after unix epoch")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("vn_assets_symlink_root_{unique}"));
    let outside = std::env::temp_dir().join(format!("vn_assets_symlink_out_{unique}.ogg"));
    std::fs::create_dir_all(root.join("audio")).expect("audio dir");
    std::fs::write(&outside, [9u8, 9, 9]).expect("outside file");
    symlink(&outside, root.join("audio").join("escape.ogg")).expect("create symlink");

    let store =
        AssetStore::new(root.clone(), SecurityMode::Trusted, None, false).expect("asset store");
    let err = store
        .load_bytes("audio/escape.ogg")
        .expect_err("symlink escape must be blocked");
    assert!(matches!(err, AssetError::Traversal));

    let _ = std::fs::remove_dir_all(root);
    let _ = std::fs::remove_file(outside);
}

#[test]
fn fingerprint_catalog_detects_duplicate_blobs_and_budget() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock must be after unix epoch")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("vn_assets_fingerprint_{unique}"));
    std::fs::create_dir_all(root.join("audio")).expect("audio dir");
    std::fs::create_dir_all(root.join("bg")).expect("bg dir");

    std::fs::write(root.join("audio/a.ogg"), [1u8, 2, 3]).expect("write a");
    std::fs::write(root.join("audio/b.ogg"), [1u8, 2, 3]).expect("write b duplicate");
    std::fs::write(root.join("bg/c.png"), [9u8, 8, 7, 6]).expect("write c");

    let catalog = AssetFingerprintCatalog::build(&root, &["ogg", "png"]).expect("catalog");
    assert_eq!(catalog.entries.len(), 3);
    assert_eq!(catalog.unique_blob_count(), 2);
    assert_eq!(catalog.duplicate_blob_count(), 1);

    let ok_budget = PlatformBudget {
        max_total_bytes: 32,
        max_assets: 8,
    };
    let report = catalog.budget_report(ok_budget);
    assert!(report.within_budget);
    assert_eq!(report.asset_count, 3);

    let tight_budget = PlatformBudget {
        max_total_bytes: 4,
        max_assets: 2,
    };
    let report = catalog.budget_report(tight_budget);
    assert!(!report.within_budget);

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn asset_fingerprint_stability() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock must be after unix epoch")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("vn_assets_stability_{unique}"));
    std::fs::create_dir_all(root.join("audio")).expect("audio dir");
    std::fs::create_dir_all(root.join("bg")).expect("bg dir");
    std::fs::write(root.join("audio/theme.ogg"), [1u8, 3, 5, 7]).expect("write audio");
    std::fs::write(root.join("bg/room.png"), [9u8, 8, 7, 6]).expect("write image");

    let first = AssetFingerprintCatalog::build(&root, &["ogg", "png"]).expect("catalog 1");
    let second = AssetFingerprintCatalog::build(&root, &["ogg", "png"]).expect("catalog 2");

    assert_eq!(first.entries, second.entries);
    assert_eq!(first.dedup_groups, second.dedup_groups);

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn dedup_reduces_duplicate_load() {
    let scenes = std::collections::BTreeMap::from([
        (
            "intro".to_string(),
            vec![
                "bg/room.png".to_string(),
                "music/theme.ogg".to_string(),
                "bg/room.png".to_string(),
            ],
        ),
        (
            "choice_a".to_string(),
            vec![
                "bg/room.png".to_string(),
                "music/theme.ogg".to_string(),
                "sfx/click.ogg".to_string(),
            ],
        ),
    ]);

    let plan = AssetFingerprintCatalog::scene_preload_plan(&scenes);
    assert_eq!(plan.total_references, 6);
    assert_eq!(plan.deduped_references, 3);
    assert!(plan.cache_hit_rate > 0.4);
}

#[test]
fn platform_budget_enforcement() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock must be after unix epoch")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("vn_assets_budget_platform_{unique}"));
    std::fs::create_dir_all(root.join("audio")).expect("audio dir");
    std::fs::write(root.join("audio/theme.ogg"), [1u8, 2, 3, 4, 5]).expect("write audio");
    let catalog = AssetFingerprintCatalog::build(&root, &["ogg"]).expect("catalog");

    let mobile_budget = PlatformTarget::Mobile.default_budget();
    assert!(catalog.budget_report(mobile_budget).within_budget);

    let tight = PlatformBudget {
        max_total_bytes: 2,
        max_assets: 1,
    };
    assert!(!catalog.budget_report(tight).within_budget);

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn scene_preload_hit_rate() {
    let scenes = std::collections::BTreeMap::from([
        (
            "s1".to_string(),
            vec!["bg/a.png".to_string(), "music/a.ogg".to_string()],
        ),
        (
            "s2".to_string(),
            vec!["bg/a.png".to_string(), "music/b.ogg".to_string()],
        ),
        (
            "s3".to_string(),
            vec!["bg/a.png".to_string(), "music/a.ogg".to_string()],
        ),
    ]);

    let plan = AssetFingerprintCatalog::scene_preload_plan(&scenes);
    assert_eq!(plan.total_references, 6);
    assert_eq!(plan.deduped_references, 3);
    assert!((plan.cache_hit_rate - 0.5).abs() < f32::EPSILON);
}

#[test]
fn transcode_recommendations_follow_platform_presets() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock must be after unix epoch")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("vn_assets_transcode_{unique}"));
    std::fs::create_dir_all(root.join("audio")).expect("audio dir");
    std::fs::create_dir_all(root.join("bg")).expect("bg dir");
    std::fs::write(root.join("audio/theme.wav"), [1u8, 2, 3]).expect("write audio");
    std::fs::write(root.join("bg/room.png"), [7u8, 8, 9]).expect("write image");
    std::fs::write(root.join("bg/skip.webp"), [0u8, 1, 2]).expect("write webp");

    let catalog = AssetFingerprintCatalog::build(&root, &["wav", "png", "webp"]).expect("catalog");
    let mobile = catalog.transcode_recommendations(PlatformTarget::Mobile);
    assert!(mobile
        .iter()
        .any(|item| item.rel_path == "audio/theme.wav" && item.target_extension == "ogg"));
    assert!(mobile
        .iter()
        .any(|item| item.rel_path == "bg/room.png" && item.target_extension == "webp"));
    assert!(!mobile.iter().any(|item| item.rel_path == "bg/skip.webp"));

    let _ = std::fs::remove_dir_all(root);
}
