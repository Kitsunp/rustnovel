use std::collections::HashSet;
use std::fs;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::model::{AssetError, AssetKind, SUPPORTED_IMAGE_EXTENSIONS};

pub fn sanitize_rel_path(rel: &Path) -> Result<PathBuf, AssetError> {
    use std::path::Component::*;
    // Backslashes are native separators on Windows, but ambiguous literal
    // filename characters on other platforms.
    if !cfg!(windows) && rel.as_os_str().to_string_lossy().contains('\\') {
        return Err(AssetError::Traversal);
    }
    let mut out = PathBuf::new();
    for component in rel.components() {
        match component {
            CurDir => {}
            Normal(part) => {
                let name = part.to_str().ok_or(AssetError::Traversal)?;
                if name.is_empty()
                    || name.ends_with('.')
                    || name.ends_with(' ')
                    || name.contains(':')
                    || ["%2e", "%2f", "%5c", "%00"]
                        .iter()
                        .any(|s| name.to_ascii_lowercase().contains(s))
                    || name.bytes().any(|b| b == 0 || b < 32)
                    || is_windows_device_name(name)
                {
                    return Err(AssetError::Traversal);
                }
                out.push(part);
            }
            ParentDir | RootDir | Prefix(_) => return Err(AssetError::Traversal),
        }
    }
    if out.as_os_str().is_empty() {
        return Err(AssetError::Traversal);
    }
    Ok(out)
}

fn is_windows_device_name(name: &str) -> bool {
    let stem = name
        .split('.')
        .next()
        .unwrap_or(name)
        .trim_end_matches(' ')
        .to_ascii_uppercase();
    matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || (stem.len() == 4
            && (stem.starts_with("COM") || stem.starts_with("LPT"))
            && stem.as_bytes()[3].is_ascii_digit()
            && stem.as_bytes()[3] != b'0')
}

pub(crate) fn normalize_asset_key(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

pub(crate) fn canonicalize_within_root(root: &Path, rel: &Path) -> Result<PathBuf, AssetError> {
    let canonical_root = root.canonicalize()?;
    let full_path = root.join(rel).canonicalize()?;
    if !full_path.starts_with(&canonical_root) {
        return Err(AssetError::Traversal);
    }
    Ok(full_path)
}

pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub(crate) fn sha256_file_and_size(path: &Path) -> Result<(String, u64), AssetError> {
    let file = fs::File::open(path)?;
    let mut reader = BufReader::new(file);
    let mut hasher = Sha256::new();
    let mut total = 0u64;
    let mut chunk = [0u8; 16 * 1024];

    loop {
        let read = reader.read(&mut chunk)?;
        if read == 0 {
            break;
        }
        total = total.saturating_add(read as u64);
        hasher.update(&chunk[..read]);
    }

    let digest = hasher.finalize();
    let hex = digest.iter().map(|byte| format!("{byte:02x}")).collect();
    Ok((hex, total))
}

pub(crate) fn is_allowed_by_extension(path: &Path, allowed: &HashSet<String>) -> bool {
    if allowed.is_empty() {
        return true;
    }
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| allowed.contains(&ext.to_ascii_lowercase()))
        .unwrap_or(false)
}

pub(crate) fn infer_asset_kind(path: &str) -> AssetKind {
    let extension = Path::new(path)
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase());
    match extension.as_deref() {
        Some("png" | "jpg" | "jpeg" | "webp" | "bmp") => AssetKind::Image,
        Some("ogg" | "wav" | "flac" | "mp3" | "m4a") => AssetKind::Audio,
        _ => AssetKind::Other,
    }
}

pub(crate) fn candidate_image_paths(asset_path: &str) -> Vec<String> {
    candidate_asset_paths(asset_path, &SUPPORTED_IMAGE_EXTENSIONS)
}

pub(crate) fn candidate_asset_paths(asset_path: &str, extensions: &[&str]) -> Vec<String> {
    let normalized = normalize_asset_request(asset_path);
    if normalized.is_empty() {
        return Vec::new();
    }

    let mut candidates = Vec::new();
    push_unique_candidate(&mut candidates, &normalized);
    if !normalized.starts_with("assets/") {
        push_unique_candidate(&mut candidates, &format!("assets/{normalized}"));
    }

    if Path::new(&normalized).extension().is_none() {
        let base_candidates = candidates.clone();
        for base in base_candidates {
            for extension in extensions {
                push_unique_candidate(&mut candidates, &format!("{base}.{extension}"));
            }
        }
    }

    candidates
}

fn push_unique_candidate(candidates: &mut Vec<String>, value: &str) {
    if candidates.iter().any(|existing| existing == value) {
        return;
    }
    candidates.push(value.to_string());
}

pub(crate) fn normalize_asset_request(asset_path: &str) -> String {
    asset_path.trim().replace('\\', "/")
}
