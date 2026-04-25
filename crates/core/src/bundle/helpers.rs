use std::fmt::Write;
use std::path::{Component, Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::error::{VnError, VnResult};

pub(super) fn sanitize_relative_path(path: &Path, field_name: &str) -> VnResult<PathBuf> {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::Normal(segment) => out.push(segment),
            Component::ParentDir => {
                return Err(invalid_bundle(format!(
                    "{field_name} contains path traversal: '{}'",
                    path.display()
                )))
            }
            Component::RootDir | Component::Prefix(_) => {
                return Err(invalid_bundle(format!(
                    "{field_name} must be relative: '{}'",
                    path.display()
                )))
            }
        }
    }

    if out.as_os_str().is_empty() {
        return Err(invalid_bundle(format!(
            "{field_name} resolved to empty relative path"
        )));
    }
    Ok(out)
}

pub(super) fn canonicalize_within_root(
    root: &Path,
    path: &Path,
    field_name: &str,
) -> VnResult<PathBuf> {
    let canonical_root = root.canonicalize().map_err(|e| {
        invalid_bundle(format!(
            "canonicalize {field_name} root '{}': {e}",
            root.display()
        ))
    })?;
    let candidate = if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    };
    let canonical_path = candidate.canonicalize().map_err(|e| {
        invalid_bundle(format!(
            "canonicalize {field_name} '{}': {e}",
            candidate.display()
        ))
    })?;

    if !canonical_path.starts_with(&canonical_root) {
        return Err(invalid_bundle(format!(
            "{field_name} escapes project root: '{}'",
            path.display()
        )));
    }

    Ok(canonical_path)
}

pub(super) fn normalize_path_display(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

pub(super) fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    to_hex(digest.as_slice())
}

pub(super) fn invalid_bundle(message: impl Into<String>) -> VnError {
    VnError::InvalidScript(format!("bundle export: {}", message.into()))
}

pub(super) fn to_hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        let _ = write!(&mut out, "{byte:02x}");
    }
    out
}
