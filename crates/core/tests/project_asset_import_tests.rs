use std::fs;
use tempfile::tempdir;
use visual_novel_engine::manifest::*;

#[test]
fn import_reuses_same_file_and_renamed_content_without_rewriting_manifest() {
    let temp = tempdir().unwrap();
    let root = temp.path().join("project");
    fs::create_dir(&root).unwrap();
    let source = temp.path().join("room.png");
    fs::write(&source, b"image bytes").unwrap();
    let path = root.join("project.vnm");
    let mut manifest = ProjectManifest::new("Import", "Tests");
    manifest.save(&path).unwrap();
    let first = import_project_asset(
        &root,
        &path,
        &mut manifest,
        &source,
        AssetImportKind::Background,
        100,
    )
    .unwrap();
    assert!(first.imported && first.copied);
    assert_eq!(first.path, "assets/backgrounds/room.png");
    let saved = fs::read(&path).unwrap();
    let renamed = temp.path().join("renamed.png");
    fs::copy(&source, &renamed).unwrap();
    for source in [&source, &renamed, &root.join(&first.path)] {
        let reused = import_project_asset(
            &root,
            &path,
            &mut manifest,
            source,
            AssetImportKind::Background,
            100,
        )
        .unwrap();
        assert!(!reused.imported && !reused.copied);
        assert_eq!(reused.path, first.path);
        assert_eq!(reused.name, first.name);
        assert_eq!(fs::read(&path).unwrap(), saved);
    }
    fs::write(&source, b"changed bytes").unwrap();
    let different = import_project_asset(
        &root,
        &path,
        &mut manifest,
        &source,
        AssetImportKind::Background,
        100,
    )
    .unwrap();
    assert_eq!(different.path, "assets/backgrounds/room-1.png");
    assert_eq!(manifest.assets.backgrounds.len(), 2);
    assert_eq!(ProjectManifest::load(&path).unwrap(), manifest);
}

#[test]
fn failed_manifest_save_rolls_back_memory_and_removes_new_asset_copy() {
    let temp = tempdir().unwrap();
    let source = temp.path().join("theme.ogg");
    fs::write(&source, b"audio").unwrap();
    let root = temp.path().join("project");
    fs::create_dir(&root).unwrap();
    let destination = root.join("project.vnm");
    fs::create_dir(&destination).unwrap();
    let mut manifest = ProjectManifest::new("Rollback", "Tests");
    let before = manifest.clone();
    assert!(import_project_asset(
        &root,
        &destination,
        &mut manifest,
        &source,
        AssetImportKind::Audio,
        100
    )
    .is_err());
    assert_eq!(manifest, before);
    assert!(destination.is_dir());
    assert_eq!(fs::read_dir(root.join("assets/audio")).unwrap().count(), 0);
}

#[test]
fn invalid_kind_size_and_manifest_destination_do_not_copy_or_mutate() {
    let temp = tempdir().unwrap();
    let root = temp.path().join("project");
    fs::create_dir(&root).unwrap();
    let source = temp.path().join("theme.ogg");
    fs::write(&source, b"audio").unwrap();
    let mut manifest = ProjectManifest::new("Validation", "Tests");
    let before = manifest.clone();
    let path = root.join("project.vnm");
    manifest.save(&path).unwrap();
    let saved = fs::read(&path).unwrap();
    for (path, kind, limit) in [
        (path.clone(), AssetImportKind::Background, 100),
        (path.clone(), AssetImportKind::Audio, 1),
        (temp.path().join("outside.vnm"), AssetImportKind::Audio, 100),
    ] {
        assert!(import_project_asset(&root, &path, &mut manifest, &source, kind, limit).is_err());
        assert_eq!(manifest, before);
    }
    assert_eq!(fs::read(&path).unwrap(), saved);
    assert!(!root.join("assets").exists());
    assert!(!temp.path().join("outside.vnm").exists());
}

#[test]
fn hashing_is_shared_and_counts_all_bytes_across_buffer_boundaries() {
    use sha2::{Digest, Sha256};
    let temp = tempdir().unwrap();
    let path = temp.path().join("large.bin");
    let bytes = vec![17u8; 2 * 64 * 1024 + 17];
    fs::write(&path, &bytes).unwrap();
    let (hash, size) = visual_novel_engine::atomic_io::sha256_file(&path).unwrap();
    assert_eq!(size, bytes.len() as u64);
    assert_eq!(hash, format!("{:x}", Sha256::digest(&bytes)));
}

#[test]
fn deduplication_does_not_reuse_unsafe_manifest_path_aliases() {
    let temp = tempdir().unwrap();
    let root = temp.path().join("project");
    fs::create_dir_all(root.join("assets/backgrounds")).unwrap();
    let unsafe_path = "assets/backgrounds/room.png ";
    let physical = if cfg!(windows) {
        unsafe_path.trim_end()
    } else {
        unsafe_path
    };
    fs::write(root.join(physical), b"image").unwrap();
    let source = temp.path().join("external.png");
    fs::write(&source, b"image").unwrap();
    let path = root.join("project.vnm");
    let mut manifest = ProjectManifest::new("Paths", "Tests");
    manifest
        .assets
        .backgrounds
        .insert("unsafe".into(), unsafe_path.into());
    manifest.save(&path).unwrap();
    let result = import_project_asset(
        &root,
        &path,
        &mut manifest,
        &source,
        AssetImportKind::Background,
        100,
    )
    .unwrap();
    assert!(result.imported && result.copied);
    assert_eq!(result.path, "assets/backgrounds/external.png");
}
