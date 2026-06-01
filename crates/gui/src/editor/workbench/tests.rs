use super::*;

fn create_file_symlink(link: &std::path::Path, target: &std::path::Path) -> bool {
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(target, link).is_ok()
    }

    #[cfg(windows)]
    {
        std::os::windows::fs::symlink_file(target, link).is_ok()
    }

    #[cfg(not(any(unix, windows)))]
    {
        let _ = link;
        let _ = target;
        false
    }
}

#[test]
fn corrupt_layout_prefs_are_not_silently_ignored() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("layout.json");
    std::fs::write(&path, "{not-json").expect("write corrupt prefs");

    let err = EditorWorkbench::load_layout_prefs(&path)
        .expect_err("corrupt layout preferences must report an error");

    assert!(err.contains("parse layout preferences"));
    assert!(err.contains("layout.json"));
}

#[test]
fn dangling_layout_prefs_symlink_is_not_silently_ignored() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("layout.json");
    let missing_target = dir.path().join("missing-layout.json");
    if !create_file_symlink(&path, &missing_target) {
        eprintln!("file symlink creation not supported on this platform");
        return;
    }

    let err = EditorWorkbench::load_layout_prefs(&path)
        .expect_err("dangling layout preferences symlink must report an error");

    assert!(err.contains("read '"));
    assert!(err.contains("layout.json"));
}
