use std::path::Path;

pub fn atomic_replace(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let nonce = uuid::Uuid::new_v4();
    let tmp = parent.join(format!(".{}.tmp-{nonce}", file_name(path)));
    let backup = parent.join(format!(".{}.rollback-{nonce}", file_name(path)));
    std::fs::write(&tmp, bytes)?;
    publish_file(&tmp, path, &backup)
}

fn publish_file(tmp: &Path, path: &Path, backup: &Path) -> std::io::Result<()> {
    let had_existing = path.exists();
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
    if had_existing && backup.exists() {
        if let Err(restore_err) = std::fs::rename(backup, path) {
            return Err(std::io::Error::new(
                err.kind(),
                format!("{err}; rollback restore failed: {restore_err}"),
            ));
        }
    }
    Err(err)
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
