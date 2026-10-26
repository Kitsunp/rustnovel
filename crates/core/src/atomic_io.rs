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
            published: false,
        };
        let file = pending.file.as_mut().expect("temporary file is open");
        write(file)?;
        file.sync_all()?;
        drop(pending.file.take());
        fs::rename(&pending.path, path)?;
        pending.published = true;
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
    published: bool,
}

impl Drop for PendingFile {
    fn drop(&mut self) {
        // Windows requires closing our handle before removing an unpublished file.
        drop(self.file.take());
        if !self.published {
            // A copied read-only source must not prevent temporary cleanup on Windows.
            #[cfg(windows)]
            if let Ok(metadata) = fs::metadata(&self.path) {
                let mut permissions = metadata.permissions();
                if permissions.readonly() {
                    // This cfg(windows) block only clears the DOS read-only attribute.
                    #[allow(clippy::permissions_set_readonly_false)]
                    permissions.set_readonly(false);
                    let _ = fs::set_permissions(&self.path, permissions);
                }
            }
            let _ = fs::remove_file(&self.path);
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
    }
}
