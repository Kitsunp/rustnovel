use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::asset_refs::collect_script_asset_refs;
use crate::authoring::is_unsafe_asset_ref;
use crate::error::VnResult;
use crate::script::ScriptRaw;

use super::helpers::{
    canonicalize_within_root, invalid_bundle, normalize_path_display, sanitize_relative_path,
    sha256_hex,
};
use super::BundleAssetEntry;

pub(super) fn copy_referenced_assets(
    project_root: &Path,
    assets_output_root: &Path,
    script: &ScriptRaw,
) -> VnResult<BTreeMap<String, BundleAssetEntry>> {
    if let Some((asset, existing)) = detect_asset_case_collisions(project_root, script)?.pop() {
        return Err(invalid_bundle(format!(
            "case-sensitive asset collision: '{}' conflicts with '{}'",
            asset, existing
        )));
    }

    let mut manifest = BTreeMap::new();
    for asset_ref in collect_script_asset_refs(script) {
        let (source, destination_rel) = resolve_asset_reference(project_root, &asset_ref)?;
        let manifest_path = normalize_path_display(&destination_rel);
        let destination = assets_output_root.join(
            destination_rel
                .strip_prefix("assets")
                .unwrap_or(destination_rel.as_path()),
        );
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).map_err(|e| {
                invalid_bundle(format!("create asset parent '{}': {e}", parent.display()))
            })?;
        }
        fs::copy(&source, &destination).map_err(|e| {
            invalid_bundle(format!(
                "copy asset '{}' -> '{}': {e}",
                source.display(),
                destination.display()
            ))
        })?;
        let bytes = fs::read(&source).map_err(|e| {
            invalid_bundle(format!("read copied asset '{}': {e}", source.display()))
        })?;
        manifest.insert(
            manifest_path,
            BundleAssetEntry {
                sha256: sha256_hex(&bytes),
                size: bytes.len() as u64,
            },
        );
    }

    Ok(manifest)
}

pub(super) fn detect_asset_case_collisions(
    project_root: &Path,
    script: &ScriptRaw,
) -> VnResult<Vec<(String, String)>> {
    let mut seen_destinations: BTreeMap<String, String> = BTreeMap::new();
    let mut collisions = Vec::new();
    for asset_ref in collect_script_asset_refs(script) {
        let (_, destination_rel) = resolve_asset_reference(project_root, &asset_ref)?;
        let manifest_path = normalize_path_display(&destination_rel);
        let case_folded = manifest_path.to_lowercase();
        if let Some(existing) = seen_destinations.get(&case_folded) {
            if existing != &manifest_path {
                collisions.push((manifest_path, existing.clone()));
            }
        } else {
            seen_destinations.insert(case_folded, manifest_path);
        }
    }
    Ok(collisions)
}

fn resolve_asset_reference(project_root: &Path, asset_ref: &str) -> VnResult<(PathBuf, PathBuf)> {
    if is_unsafe_asset_ref(asset_ref) {
        return Err(invalid_bundle(format!(
            "asset reference is unsafe '{}'",
            asset_ref
        )));
    }
    let rel = sanitize_relative_path(Path::new(asset_ref.trim()), "asset reference")?;
    let source_rel = if rel.starts_with("assets") || project_root.join(&rel).is_file() {
        rel.clone()
    } else {
        PathBuf::from("assets").join(&rel)
    };
    let source_path = project_root.join(&source_rel);
    let metadata = fs::symlink_metadata(&source_path).map_err(|e| {
        invalid_bundle(format!(
            "read asset metadata '{}': {e}",
            source_path.display()
        ))
    })?;
    if metadata.file_type().is_symlink() {
        return Err(invalid_bundle(format!(
            "asset escapes project root: '{}'",
            asset_ref
        )));
    }
    let source = canonicalize_within_root(project_root, &source_rel, "asset")?;
    if !source.is_file() {
        return Err(invalid_bundle(format!(
            "asset reference is not a file '{}'",
            asset_ref
        )));
    }
    Ok((source, source_rel))
}
