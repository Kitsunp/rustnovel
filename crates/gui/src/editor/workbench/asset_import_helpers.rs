use std::path::{Path, PathBuf};

use crate::editor::{AssetFieldTarget, AssetImportKind};

pub fn pick_asset_file(kind: AssetImportKind, project_root: Option<&Path>) -> Option<PathBuf> {
    let mut dialog = rfd::FileDialog::new()
        .set_title(kind.dialog_title())
        .add_filter(kind.label(), kind.file_dialog_extensions());
    if let Some(root) = project_root {
        dialog = dialog.set_directory(root);
    }
    dialog.pick_file()
}

pub fn copy_external_asset(
    source: &Path,
    project_root: &Path,
    kind: AssetImportKind,
    extension: &str,
) -> Result<PathBuf, String> {
    let canonical_root = project_root
        .canonicalize()
        .map_err(|err| format!("project root unavailable: {err}"))?;
    let dest_dir = PathBuf::from(kind.destination_dir());
    let dest_rel = unique_destination_path(project_root, &dest_dir, source, extension)?;
    let dest_abs = project_root.join(&dest_rel);
    if let Some(parent) = dest_abs.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|err| format!("failed to create asset directory: {err}"))?;
        let canonical_parent = parent
            .canonicalize()
            .map_err(|err| format!("asset destination unavailable: {err}"))?;
        if !canonical_parent.starts_with(&canonical_root) {
            return Err(format!(
                "asset destination escapes project root: {}",
                parent.display()
            ));
        }
    }
    std::fs::copy(source, &dest_abs)
        .map_err(|err| format!("failed to copy external asset into project: {err}"))?;
    let canonical_dest = dest_abs
        .canonicalize()
        .map_err(|err| format!("copied asset unavailable: {err}"))?;
    if !canonical_dest.starts_with(&canonical_root) {
        return Err(format!(
            "copied asset escaped project root: {}",
            dest_abs.display()
        ));
    }
    Ok(dest_rel)
}

fn unique_destination_path(
    project_root: &Path,
    dest_dir: &Path,
    source: &Path,
    extension: &str,
) -> Result<PathBuf, String> {
    let base = sanitized_stem(source);
    let mut suffix = 0usize;
    loop {
        let name = if suffix == 0 {
            format!("{base}.{extension}")
        } else {
            format!("{base}-{suffix}.{extension}")
        };
        let rel = dest_dir.join(name);
        match std::fs::symlink_metadata(project_root.join(&rel)) {
            Ok(_) => {}
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(rel),
            Err(err) => {
                return Err(format!(
                    "failed to inspect asset destination {}: {err}",
                    project_root.join(&rel).display()
                ));
            }
        }
        suffix += 1;
    }
}

pub fn unique_manifest_name(
    manifest: &visual_novel_engine::manifest::ProjectManifest,
    kind: AssetImportKind,
    source: &Path,
) -> String {
    let base = sanitized_stem(source);
    let exists = |candidate: &str| match kind {
        AssetImportKind::Background => manifest.assets.backgrounds.contains_key(candidate),
        AssetImportKind::Character => manifest.assets.characters.contains_key(candidate),
        AssetImportKind::Audio => manifest.assets.audio.contains_key(candidate),
    };

    if !exists(&base) {
        return base;
    }

    let mut suffix = 1usize;
    loop {
        let candidate = format!("{base}_{suffix}");
        if !exists(&candidate) {
            return candidate;
        }
        suffix += 1;
    }
}

fn sanitized_stem(path: &Path) -> String {
    let raw = path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("asset");
    let mut out = String::with_capacity(raw.len().max(5));
    for ch in raw.chars() {
        if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_') {
            out.push(ch);
        } else if !out.ends_with('_') {
            out.push('_');
        }
    }
    let trimmed = out.trim_matches('_');
    if trimmed.is_empty() {
        "asset".to_string()
    } else {
        trimmed.to_string()
    }
}

pub fn normalized_extension(path: &Path) -> Option<String> {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase())
}

pub fn manifest_asset_field_path(kind: AssetImportKind, asset_name: &str) -> String {
    let table = match kind {
        AssetImportKind::Background => "backgrounds",
        AssetImportKind::Character => "characters",
        AssetImportKind::Audio => "audio",
    };
    format!("manifest.assets.{table}[{asset_name}]")
}

pub fn asset_node_field_path(node_id: u32, target: AssetFieldTarget) -> String {
    match target {
        AssetFieldTarget::SceneBackground => format!("graph.nodes[{node_id}].background"),
        AssetFieldTarget::SceneMusic => format!("graph.nodes[{node_id}].music"),
        AssetFieldTarget::SceneCharacterExpression(idx) => {
            format!("graph.nodes[{node_id}].characters[{idx}].expression")
        }
        AssetFieldTarget::ScenePatchBackground => {
            format!("graph.nodes[{node_id}].patch.background")
        }
        AssetFieldTarget::ScenePatchMusic => format!("graph.nodes[{node_id}].patch.music"),
        AssetFieldTarget::ScenePatchAddCharacterExpression(idx) => {
            format!("graph.nodes[{node_id}].patch.add[{idx}].expression")
        }
        AssetFieldTarget::AudioActionAsset => format!("graph.nodes[{node_id}].audio.asset"),
    }
}

pub fn stringify_optional_asset(value: &Option<String>) -> String {
    value.clone().unwrap_or_else(|| "<none>".to_string())
}

pub fn normalized_character_name(name: &str, asset_path: &str) -> String {
    let trimmed = name.trim();
    if !trimmed.is_empty() {
        return trimmed.to_string();
    }
    std::path::Path::new(asset_path)
        .file_stem()
        .and_then(|value| value.to_str())
        .map(sanitized_identifier)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "Character".to_string())
}

fn sanitized_identifier(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    for ch in raw.chars() {
        if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_') {
            out.push(ch);
        } else if !out.ends_with('_') {
            out.push('_');
        }
    }
    out.trim_matches('_').to_string()
}

pub fn upsert_character_asset(
    characters: &mut Vec<visual_novel_engine::runtime::CharacterPlacementRaw>,
    name: &str,
    imported: &str,
    x: i32,
    y: i32,
) -> (usize, String, String) {
    if let Some((idx, character)) = characters.iter_mut().enumerate().find(|(_, character)| {
        character.name == name && character.expression.as_deref() == Some(imported)
    }) {
        let before = character_snapshot(character);
        character.x = Some(x);
        character.y = Some(y);
        character.scale = character.scale.or(Some(1.0));
        let after = character_snapshot(character);
        return (idx, before, after);
    }

    let idx = characters.len();
    characters.push(visual_novel_engine::runtime::CharacterPlacementRaw {
        name: name.to_string(),
        expression: Some(imported.to_string()),
        position: None,
        x: Some(x),
        y: Some(y),
        scale: Some(1.0),
    });
    let after = character_snapshot(&characters[idx]);
    (idx, "<none>".to_string(), after)
}

fn character_snapshot(character: &visual_novel_engine::runtime::CharacterPlacementRaw) -> String {
    format!(
        "{}|{}|x={}|y={}|scale={}",
        character.name,
        character.expression.as_deref().unwrap_or("<none>"),
        character
            .x
            .map(|value| value.to_string())
            .unwrap_or_else(|| "<none>".to_string()),
        character
            .y
            .map(|value| value.to_string())
            .unwrap_or_else(|| "<none>".to_string()),
        character
            .scale
            .map(|value| value.to_string())
            .unwrap_or_else(|| "<none>".to_string())
    )
}
