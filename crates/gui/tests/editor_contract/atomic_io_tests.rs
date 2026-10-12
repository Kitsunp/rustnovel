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
    let output = dir.path().join("output.json");
    atomic_replace(&output, b"first").expect("create output");
    atomic_replace(&output, b"second").expect("replace output");
    assert_eq!(std::fs::read(&output).unwrap(), b"second");
    let backup = dir.path().join("backup.json");
    visual_novel_engine::atomic_io::atomic_copy(&output, &backup).expect("create backup");
    atomic_replace(&output, b"third").unwrap();
    visual_novel_engine::atomic_io::atomic_copy(&output, &backup).expect("replace backup");
    assert_eq!(std::fs::read(&backup).unwrap(), b"third");

    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        let locked = std::fs::OpenOptions::new()
            .read(true)
            .share_mode(1) // FILE_SHARE_READ: deny write and delete/rename.
            .open(&output)
            .unwrap();
        atomic_replace(&output, b"blocked").expect_err("locked destination");
        let mut permissions = std::fs::metadata(&backup).unwrap().permissions();
        permissions.set_readonly(true);
        std::fs::set_permissions(&backup, permissions).unwrap();
        visual_novel_engine::atomic_io::atomic_copy(&backup, &output)
            .expect_err("locked destination rejects read-only backup publication");
        let copied = dir.path().join("readonly-copy.json");
        visual_novel_engine::atomic_io::atomic_copy(&backup, &copied).unwrap();
        assert!(std::fs::metadata(&copied).unwrap().permissions().readonly());
        for path in [&backup, &copied] {
            let mut permissions = std::fs::metadata(path).unwrap().permissions();
            // Windows-only cleanup of the DOS read-only attribute.
            #[allow(clippy::permissions_set_readonly_false)]
            permissions.set_readonly(false);
            std::fs::set_permissions(path, permissions).unwrap();
        }
        std::fs::remove_file(copied).unwrap();
        assert_eq!(std::fs::read(&output).unwrap(), b"third");
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 2);
        drop(locked);
        atomic_replace(&output, b"unlocked").expect("replace after closing handle");
    }
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
