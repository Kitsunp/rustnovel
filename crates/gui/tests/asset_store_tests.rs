use std::path::Path;

use visual_novel_gui::{sanitize_rel_path, AssetError};

#[test]
fn sanitize_rel_path_blocks_traversal() {
    let err = sanitize_rel_path(Path::new("../secrets.txt")).expect_err("should fail");
    assert!(matches!(err, AssetError::Traversal));

    let err = sanitize_rel_path(Path::new("/etc/passwd")).expect_err("should fail");
    assert!(matches!(err, AssetError::Traversal));
}

#[test]
fn sanitize_rel_path_allows_normal_paths() {
    let path = sanitize_rel_path(Path::new("characters/ava.png")).expect("valid");
    assert_eq!(path, Path::new("characters/ava.png"));
}
