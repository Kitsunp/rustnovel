use std::path::{Path, PathBuf};
use tempfile::tempdir;
use vnengine_assets::{
    read_asset_bytes, sanitize_rel_path, AssetError, AssetLimits, AssetStore, SecurityMode,
};

#[test]
fn rep_asset_paths_reject_windows_aliases_on_all_platforms() {
    for path in [
        "CON",
        "aux.png",
        "images/COM1.png",
        "picture.png:stream",
        "folder/file.png.",
        "portrait.png ",
        ".",
        "",
        "assets/%2e%2e/file.png",
    ] {
        assert!(
            sanitize_rel_path(Path::new(path)).is_err(),
            "unsafe path: {path}"
        );
    }
    assert!(sanitize_rel_path(Path::new("assets/bg/room.png")).is_ok());
    let native = sanitize_rel_path(Path::new("folder\\other"));
    if cfg!(windows) {
        assert_eq!(
            native.expect("native separators"),
            PathBuf::from("folder").join("other")
        );
    } else {
        assert!(native.is_err());
    }
    assert!(sanitize_rel_path(Path::new("folder\\..\\other")).is_err());
}

#[test]
fn rep_untrusted_mode_always_needs_manifest() {
    let directory = tempdir().expect("tempdir");
    assert!(matches!(
        AssetStore::new(
            directory.path().into(),
            SecurityMode::Untrusted,
            None,
            false
        ),
        Err(AssetError::ManifestMissing)
    ));
}

#[test]
fn rep_oversized_file_rejected_before_reading_entire_payload() {
    let directory = tempdir().expect("tempdir");
    std::fs::write(directory.path().join("large.bin"), vec![42u8; 4096]).expect("write fixture");
    let path = directory.path().join("large.bin");
    assert_eq!(
        read_asset_bytes(&path, 4096).expect("exact limit").len(),
        4096
    );
    assert!(matches!(
        read_asset_bytes(&path, 32),
        Err(AssetError::TooLarge {
            size: 4096,
            max: 32
        })
    ));
    let store = AssetStore::new(
        PathBuf::from(directory.path()),
        SecurityMode::Trusted,
        None,
        false,
    )
    .expect("store")
    .with_limits(AssetLimits {
        max_bytes: 32,
        max_width: 64,
        max_height: 64,
    });
    assert!(matches!(
        store.load_bytes("large.bin"),
        Err(AssetError::TooLarge {
            size: 4096,
            max: 32
        })
    ));
}
