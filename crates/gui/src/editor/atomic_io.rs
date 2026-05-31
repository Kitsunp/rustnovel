use std::io::Write;
use std::path::Path;

pub fn atomic_replace(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let nonce = uuid::Uuid::new_v4();
    let tmp = parent.join(format!(".{}.tmp-{nonce}", file_name(path)));
    let backup = parent.join(format!(".{}.rollback-{nonce}", file_name(path)));
    let had_existing = existing_regular_file(path, "target")?;
    if had_existing {
        reject_existing_path(&backup, "rollback")?;
    }
    let mut tmp_file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&tmp)?;
    tmp_file.write_all(bytes)?;
    tmp_file.sync_all()?;
    drop(tmp_file);
    publish_file(&tmp, path, &backup, had_existing)
}

fn publish_file(tmp: &Path, path: &Path, backup: &Path, had_existing: bool) -> std::io::Result<()> {
    if had_existing {
        std::fs::rename(path, backup)?;
    }
    match std::fs::rename(tmp, path) {
        Ok(()) => {
            if had_existing {
                std::fs::remove_file(backup)?;
            }
            Ok(())
        }
        Err(err) => restore_backup(path, backup, had_existing, err),
    }
}

fn restore_backup(
    path: &Path,
    backup: &Path,
    had_existing: bool,
    err: std::io::Error,
) -> std::io::Result<()> {
    if had_existing && existing_regular_file(backup, "rollback")? {
        if let Err(restore_err) = std::fs::rename(backup, path) {
            return Err(std::io::Error::new(
                err.kind(),
                format!("{err}; rollback restore failed: {restore_err}"),
            ));
        }
    }
    Err(err)
}

fn existing_regular_file(path: &Path, role: &str) -> std::io::Result<bool> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_file() => Ok(true),
        Ok(_) => Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("{role} path is not a regular file: {}", path.display()),
        )),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(err) => Err(err),
    }
}

fn reject_existing_path(path: &Path, role: &str) -> std::io::Result<()> {
    match std::fs::symlink_metadata(path) {
        Ok(_) => Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            format!("{role} path already exists: {}", path.display()),
        )),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err),
    }
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(str::to_string)
        .unwrap_or_else(|| fallback_name(path))
}

fn fallback_name(path: &Path) -> String {
    path.file_stem()
        .and_then(|name| name.to_str())
        .map(str::to_string)
        .unwrap_or_else(|| "file".to_string())
}

#[cfg(test)]
mod tests {
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

        let err = atomic_replace(&link, b"new")
            .expect_err("atomic replace must not replace or follow output symlinks");

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
}
