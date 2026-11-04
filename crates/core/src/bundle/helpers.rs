use std::fs;
use std::path::{Component, Path, PathBuf};

pub(super) use crate::hashing::{hex_digest as to_hex, sha256_hex};

use crate::error::{VnError, VnResult};

/// Standalone entries need no source manifest. A present manifest must still be
/// a regular, valid file; unreadable or malformed projects never use defaults.
pub(super) fn load_export_manifest(
    root: &Path,
    entry: Option<&Path>,
) -> VnResult<crate::ProjectManifest> {
    let path = root.join("project.vnm");
    match fs::symlink_metadata(&path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound && entry.is_some() => {
            let mut manifest = crate::ProjectManifest::new("Visual Novel Game", "Author");
            manifest.settings.entry_point = normalize_path_display(&sanitize_relative_path(
                entry.expect("explicit entry checked"),
                "entry_script",
            )?);
            Ok(manifest)
        }
        _ => {
            ensure_regular_file(&path, "manifest")?;
            crate::ProjectManifest::load(&path).map_err(|error| {
                invalid_bundle(format!("load manifest '{}': {error}", path.display()))
            })
        }
    }
}

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

pub(super) fn ensure_regular_file(path: &Path, field_name: &str) -> VnResult<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_file() => Ok(()),
        Ok(_) => Err(invalid_bundle(format!(
            "{field_name} is not a regular file: '{}'",
            path.display()
        ))),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Err(invalid_bundle(format!(
            "missing {field_name} '{}'",
            path.display()
        ))),
        Err(err) => Err(invalid_bundle(format!(
            "inspect {field_name} '{}': {err}",
            path.display()
        ))),
    }
}

pub(super) fn output_root_exists_as_directory(path: &Path) -> VnResult<bool> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_dir() => Ok(true),
        Ok(_) => Err(invalid_bundle(format!(
            "output_root is not a directory: '{}'",
            path.display()
        ))),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(err) => Err(invalid_bundle(format!(
            "inspect output_root '{}': {err}",
            path.display()
        ))),
    }
}

pub(super) fn planned_output_root(path: &Path) -> VnResult<PathBuf> {
    let absolute = std::path::absolute(path).map_err(|error| {
        invalid_bundle(format!("resolve output_root '{}': {error}", path.display()))
    })?;
    let mut ancestor = absolute.as_path();
    let mut missing = Vec::new();
    loop {
        match fs::symlink_metadata(ancestor) {
            Ok(metadata) => {
                if missing.is_empty() && !metadata.file_type().is_dir() {
                    return Err(invalid_bundle(format!(
                        "output_root is not a directory: '{}'",
                        path.display()
                    )));
                }
                let mut resolved = ancestor.canonicalize().map_err(|error| {
                    invalid_bundle(format!(
                        "canonicalize output_root ancestor '{}': {error}",
                        ancestor.display()
                    ))
                })?;
                if !output_root_exists_as_directory(&resolved)? {
                    return Err(invalid_bundle(format!(
                        "output_root ancestor disappeared: '{}'",
                        resolved.display()
                    )));
                }
                resolved.extend(missing.into_iter().rev());
                return Ok(resolved);
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let leaf = ancestor.file_name().ok_or_else(|| {
                    invalid_bundle(format!(
                        "cannot resolve missing output_root component '{}': {error}",
                        ancestor.display()
                    ))
                })?;
                missing.push(leaf);
                ancestor = ancestor.parent().ok_or_else(|| {
                    invalid_bundle(format!("cannot resolve output_root '{}'", path.display()))
                })?;
            }
            Err(error) => {
                return Err(invalid_bundle(format!(
                    "inspect output_root ancestor '{}': {error}",
                    ancestor.display()
                )));
            }
        }
    }
}

pub(super) fn normalize_path_display(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

pub(super) fn invalid_bundle(message: impl Into<String>) -> VnError {
    VnError::InvalidScript(format!("bundle export: {}", message.into()))
}
