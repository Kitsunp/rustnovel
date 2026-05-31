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
    if let Some(collision) = detect_asset_path_collisions(project_root, script)?.pop() {
        return Err(invalid_bundle(collision.message()));
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum AssetPathCollision {
    Case {
        asset: String,
        existing: String,
    },
    Destination {
        destination: String,
        first_source: String,
        second_source: String,
    },
}

impl AssetPathCollision {
    pub(super) fn message(&self) -> String {
        match self {
            Self::Case { asset, existing } => format!(
                "case-sensitive asset collision: '{}' conflicts with '{}'",
                asset, existing
            ),
            Self::Destination {
                destination,
                first_source,
                second_source,
            } => format!(
                "asset destination collision: '{}' would be copied from both '{}' and '{}'",
                destination, first_source, second_source
            ),
        }
    }
}

pub(super) fn detect_asset_path_collisions(
    project_root: &Path,
    script: &ScriptRaw,
) -> VnResult<Vec<AssetPathCollision>> {
    let mut seen_destinations: BTreeMap<String, (String, String)> = BTreeMap::new();
    let mut collisions = Vec::new();
    for asset_ref in collect_script_asset_refs(script) {
        let (source, destination_rel) = resolve_asset_reference(project_root, &asset_ref)?;
        let manifest_path = normalize_path_display(&destination_rel);
        let source_path = normalize_path_display(&source);
        let case_folded = manifest_path.to_lowercase();
        if let Some((existing_manifest_path, existing_source_path)) =
            seen_destinations.get(&case_folded)
        {
            if existing_manifest_path != &manifest_path {
                collisions.push(AssetPathCollision::Case {
                    asset: manifest_path,
                    existing: existing_manifest_path.clone(),
                });
            } else if existing_source_path != &source_path {
                collisions.push(AssetPathCollision::Destination {
                    destination: manifest_path,
                    first_source: existing_source_path.clone(),
                    second_source: source_path,
                });
            }
        } else {
            seen_destinations.insert(case_folded, (manifest_path, source_path));
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
    let (source_rel, destination_rel) = if rel.starts_with("assets") {
        (rel.clone(), rel.clone())
    } else if project_root.join(&rel).is_file() {
        (rel.clone(), PathBuf::from("assets").join(&rel))
    } else {
        let asset_rel = PathBuf::from("assets").join(&rel);
        (asset_rel.clone(), asset_rel)
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
    Ok((source, destination_rel))
}
