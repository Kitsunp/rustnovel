use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use crate::authoring::is_unsafe_asset_ref;
use crate::error::VnResult;
use crate::event::{CharacterPatchRaw, CharacterPlacementRaw, EventRaw, ScenePatchRaw};
use crate::ScriptRaw;

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
    let mut manifest = BTreeMap::new();
    for asset_ref in collect_script_asset_refs(script) {
        let (source, destination_rel) = resolve_asset_reference(project_root, &asset_ref)?;
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
            normalize_path_display(&destination_rel),
            BundleAssetEntry {
                sha256: sha256_hex(&bytes),
                size: bytes.len() as u64,
            },
        );
    }

    Ok(manifest)
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

fn collect_script_asset_refs(script: &ScriptRaw) -> Vec<String> {
    let mut refs = BTreeSet::new();
    for event in &script.events {
        collect_event_asset_refs(event, &mut refs);
    }
    refs.into_iter().collect()
}

fn collect_event_asset_refs(event: &EventRaw, refs: &mut BTreeSet<String>) {
    match event {
        EventRaw::Scene(scene) => {
            push_optional_asset_ref(&scene.background, refs);
            push_optional_asset_ref(&scene.music, refs);
            collect_character_assets(&scene.characters, refs);
        }
        EventRaw::Patch(patch) => collect_patch_asset_refs(patch, refs),
        EventRaw::AudioAction(action) => push_optional_asset_ref(&action.asset, refs),
        _ => {}
    }
}

fn collect_patch_asset_refs(patch: &ScenePatchRaw, refs: &mut BTreeSet<String>) {
    push_optional_asset_ref(&patch.background, refs);
    push_optional_asset_ref(&patch.music, refs);
    collect_character_assets(&patch.add, refs);
    collect_character_patch_assets(&patch.update, refs);
}

fn collect_character_assets(characters: &[CharacterPlacementRaw], refs: &mut BTreeSet<String>) {
    for character in characters {
        push_optional_asset_ref(&character.expression, refs);
    }
}

fn collect_character_patch_assets(characters: &[CharacterPatchRaw], refs: &mut BTreeSet<String>) {
    for character in characters {
        push_optional_asset_ref(&character.expression, refs);
    }
}

fn push_optional_asset_ref(value: &Option<String>, refs: &mut BTreeSet<String>) {
    if let Some(value) = value
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        refs.insert(value.to_string());
    }
}
