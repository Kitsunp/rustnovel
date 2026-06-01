use std::path::Path;

use super::*;

fn create_file_symlink(link: &Path, target: &Path) -> bool {
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
fn atomic_replace_rejects_existing_symlink_without_touching_target() {
    let dir = tempfile::tempdir().expect("tempdir");
    let outside = dir.path().join("outside.json");
    std::fs::write(&outside, b"sentinel").expect("outside");
    let link = dir.path().join("prefs.json");
    if !create_file_symlink(&link, &outside) {
        eprintln!("file symlink creation not supported on this platform");
        return;
    }

    let err =
        atomic_replace(&link, b"new").expect_err("atomic replace must not follow output symlinks");

    assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
    assert_eq!(std::fs::read(&outside).expect("outside"), b"sentinel");
    assert!(
        std::fs::symlink_metadata(&link)
            .expect("link")
            .file_type()
            .is_symlink(),
        "rejected symlink should remain in place"
    );
}
