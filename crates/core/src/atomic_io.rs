//! Shared publication of complete files through a sibling temporary file.

use sha2::{Digest, Sha256};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

/// Hashes a file incrementally and returns its hash and bytes read.
pub fn sha256_file(path: &Path) -> io::Result<(String, u64)> {
    use std::io::Read;
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut total = 0u64;
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
        total = total.saturating_add(read as u64);
    }
    Ok((crate::hashing::hex_digest(&hasher.finalize()), total))
}

/// Writes and syncs a sibling temporary before replacing a regular destination.
/// Failed writes leave the destination in place and attempt to remove the temporary.
/// This does not sync the parent directory or provide a multi-file transaction.
pub fn atomic_replace(path: &Path, bytes: &[u8]) -> io::Result<()> {
    replace_with(path, |file| file.write_all(bytes))
}

/// Copies a regular source through the same publication path without buffering it whole.
pub fn atomic_copy(source: &Path, target: &Path) -> io::Result<()> {
    if !fs::symlink_metadata(source)?.file_type().is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("source path is not a regular file: {}", source.display()),
        ));
    }
    let mut source = File::open(source)?;
    let permissions = source.metadata()?.permissions();
    replace_with(target, |file| {
        io::copy(&mut source, file)?;
        file.set_permissions(permissions)
    })
}

fn replace_with(path: &Path, write: impl FnOnce(&mut File) -> io::Result<()>) -> io::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_file() => {}
        Ok(_) => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("output path is not a regular file: {}", path.display()),
            ));
        }
        Err(err) if err.kind() == io::ErrorKind::NotFound => {}
        Err(err) => return Err(err),
    }
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)?;
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    for _ in 0..16 {
        let temp_path = parent.join(format!(".{name}.tmp-{}", uuid::Uuid::new_v4()));
        let file = match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp_path)
        {
            Ok(file) => file,
            Err(err) if err.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(err) => return Err(err),
        };
        let mut pending = PendingFile {
            path: temp_path,
            file: Some(file),
            cleanup_on_drop: true,
        };
        let file = pending.file.as_mut().expect("temporary file is open");
        let written = write(file).and_then(|()| file.sync_all());
        drop(pending.file.take());
        if let Err(error) = written.and_then(|()| fs::rename(&pending.path, path)) {
            return Err(pending.abort(error));
        }
        pending.cleanup_on_drop = false;
        return Ok(());
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        format!("could not allocate unique temporary for {}", path.display()),
    ))
}

struct PendingFile {
    path: PathBuf,
    file: Option<File>,
    cleanup_on_drop: bool,
}

#[derive(Debug)]
struct CleanupFailure {
    primary: io::Error,
    cleanup: io::Error,
    path: PathBuf,
}

impl std::fmt::Display for CleanupFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}; failed to remove temporary '{}': {}",
            self.primary,
            self.path.display(),
            self.cleanup
        )
    }
}

impl std::error::Error for CleanupFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.primary)
    }
}

impl PendingFile {
    fn cleanup(&mut self) -> io::Result<()> {
        // Windows requires the handle to be closed before removal.
        drop(self.file.take());
        #[cfg(windows)]
        let permission_error = match fs::metadata(&self.path) {
            Ok(metadata) if metadata.permissions().readonly() => {
                let mut permissions = metadata.permissions();
                #[allow(clippy::permissions_set_readonly_false)]
                permissions.set_readonly(false);
                fs::set_permissions(&self.path, permissions).err()
            }
            _ => None,
        };
        match fs::remove_file(&self.path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            #[cfg(windows)]
            Err(error) if permission_error.is_some() => Err(io::Error::new(
                error.kind(),
                format!(
                    "{error}; additionally failed to clear readonly attribute: {}",
                    permission_error.unwrap()
                ),
            )),
            Err(error) => Err(error),
        }
    }

    fn abort(&mut self, primary: io::Error) -> io::Error {
        let cleanup = self.cleanup();
        // Do not retry in Drop: it would make an error reporting an abandoned
        // temporary inaccurate if a second, unobserved removal succeeded.
        self.cleanup_on_drop = false;
        match cleanup {
            Ok(()) => primary,
            Err(cleanup) => io::Error::new(
                primary.kind(),
                CleanupFailure {
                    primary,
                    cleanup,
                    path: self.path.clone(),
                },
            ),
        }
    }
}

impl Drop for PendingFile {
    fn drop(&mut self) {
        if self.cleanup_on_drop {
            let _ = self.cleanup();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failed_writer_preserves_destination_and_removes_partial_temporary() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("output");
        fs::write(&target, b"original").unwrap();
        let err = replace_with(&target, |file| {
            file.write_all(b"partial")?;
            Err(io::Error::other("writer failed"))
        })
        .unwrap_err();
        assert_eq!(err.to_string(), "writer failed");
        assert_eq!(fs::read(&target).unwrap(), b"original");
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);

        let original_error = io::Error::from_raw_os_error(5);
        let original_kind = original_error.kind();
        let mut missing = PendingFile {
            path: dir.path().join("already-absent"),
            file: None,
            cleanup_on_drop: true,
        };
        let err = missing.abort(original_error);
        assert_eq!(err.kind(), original_kind);
        assert_eq!(err.raw_os_error(), Some(5));

        // A directory cannot be removed by remove_file, making cleanup fail
        // deterministically without depending on platform-specific permissions.
        let blocked = dir.path().join("blocked-temp");
        fs::create_dir(&blocked).unwrap();
        let mut pending = PendingFile {
            path: blocked.clone(),
            file: None,
            cleanup_on_drop: true,
        };
        let err = pending.abort(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "primary write failure",
        ));
        assert_eq!(err.kind(), io::ErrorKind::PermissionDenied);
        assert!(err.to_string().contains("primary write failure"));
        assert!(err.to_string().contains(&blocked.display().to_string()));
        let combined = err
            .get_ref()
            .unwrap()
            .downcast_ref::<CleanupFailure>()
            .unwrap();
        assert_eq!(combined.primary.to_string(), "primary write failure");
        assert!(!combined.cleanup.to_string().is_empty());
        assert!(blocked.exists());

        let rename_target = dir.path().join("rename-target");
        let err = replace_with(&rename_target, |file| {
            file.write_all(b"new content")?;
            fs::create_dir(&rename_target)
        })
        .unwrap_err();
        assert!(!err.to_string().is_empty());
        assert!(rename_target.is_dir());
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 3);

        #[cfg(windows)]
        {
            let readonly_target = dir.path().join("readonly-target");
            let err = replace_with(&readonly_target, |file| {
                let mut permissions = file.metadata()?.permissions();
                permissions.set_readonly(true);
                file.set_permissions(permissions)?;
                Err(io::Error::other("readonly writer failed"))
            })
            .unwrap_err();
            assert_eq!(err.to_string(), "readonly writer failed");
            assert!(!readonly_target.exists());
            assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 3);
        }
    }
}
