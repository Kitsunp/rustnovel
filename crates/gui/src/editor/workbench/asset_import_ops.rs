use std::path::{Path, PathBuf};

use super::*;
use crate::editor::{AssetFieldTarget, AssetImportKind, StoryNode};

impl EditorWorkbench {
    pub fn import_asset_dialog(&mut self, kind: AssetImportKind) {
        let Some(path) = pick_asset_file(kind, self.project_root.as_deref()) else {
            self.toast = Some(ToastState::warning(format!(
                "{} import cancelled",
                kind.label()
            )));
            return;
        };

        match self.import_asset_file(&path, kind) {
            Ok(imported) => {
                self.toast = Some(ToastState::success(format!(
                    "{} imported: {}",
                    kind.label(),
                    imported
                )));
            }
            Err(err) => {
                self.toast = Some(ToastState::error(format!(
                    "{} import failed: {err}",
                    kind.label()
                )));
            }
        }
    }

    pub(crate) fn import_asset_for_node_dialog(
        &mut self,
        node_id: u32,
        kind: AssetImportKind,
        target: AssetFieldTarget,
    ) {
        let Some(path) = pick_asset_file(kind, self.project_root.as_deref()) else {
            self.toast = Some(ToastState::warning(format!(
                "{} import cancelled",
                kind.label()
            )));
            return;
        };

        match self.import_asset_file(&path, kind) {
            Ok(imported) => {
                match self.apply_imported_asset_to_node(node_id, target, imported.clone()) {
                    Ok(()) => {
                        self.toast = Some(ToastState::success(format!(
                            "{} imported and assigned: {}",
                            kind.label(),
                            imported
                        )));
                        let _ = self.sync_graph_to_script();
                    }
                    Err(err) => {
                        self.toast = Some(ToastState::error(format!(
                            "{} imported but assignment failed: {err}",
                            kind.label()
                        )));
                    }
                }
            }
            Err(err) => {
                self.toast = Some(ToastState::error(format!(
                    "{} import failed: {err}",
                    kind.label()
                )));
            }
        }
    }

    pub(crate) fn import_asset_file(
        &mut self,
        source: &Path,
        kind: AssetImportKind,
    ) -> Result<String, String> {
        let before_fingerprint = self.current_authoring_fingerprint();
        let project_root = self
            .project_root
            .clone()
            .ok_or_else(|| "load a project before importing assets".to_string())?;
        let manifest_path = self
            .manifest_path
            .clone()
            .unwrap_or_else(|| project_root.join("project.vnm"));
        let manifest = self
            .manifest
            .as_mut()
            .ok_or_else(|| "load a project manifest before importing assets".to_string())?;

        let canonical_root = project_root
            .canonicalize()
            .map_err(|err| format!("project root unavailable: {err}"))?;
        let canonical_source = source
            .canonicalize()
            .map_err(|err| format!("source file unavailable: {err}"))?;
        if !canonical_source.is_file() {
            return Err(format!("source is not a file: {}", source.display()));
        }

        let extension = normalized_extension(&canonical_source)
            .ok_or_else(|| "asset file needs an extension".to_string())?;
        if !kind.accepts_extension(&extension) {
            return Err(format!(
                "unsupported .{} file for {}; allowed: {}",
                extension,
                kind.label(),
                kind.allowed_extensions().join(", ")
            ));
        }

        let metadata = std::fs::metadata(&canonical_source)
            .map_err(|err| format!("source metadata unavailable: {err}"))?;
        let limits = vnengine_assets::AssetLimits::default();
        if metadata.len() > limits.max_bytes {
            return Err(format!(
                "asset is too large: {} bytes (max {})",
                metadata.len(),
                limits.max_bytes
            ));
        }

        let rel_path = if canonical_source.starts_with(&canonical_root) {
            canonical_source
                .strip_prefix(&canonical_root)
                .map_err(|err| format!("failed to relativize project asset: {err}"))?
                .to_path_buf()
        } else {
            copy_external_asset(&canonical_source, &project_root, kind, &extension)?
        };

        let rel_path = vnengine_assets::sanitize_rel_path(&rel_path)
            .map_err(|err| format!("unsafe asset path rejected: {err}"))?;
        let rel_string = rel_path.to_string_lossy().replace('\\', "/");
        let asset_name = unique_manifest_name(manifest, kind, source);
        let manifest_field_path = manifest_asset_field_path(kind, &asset_name);

        match kind {
            AssetImportKind::Background => {
                manifest.assets.backgrounds.insert(asset_name, rel_path);
            }
            AssetImportKind::Character => {
                manifest.assets.characters.insert(
                    asset_name,
                    visual_novel_engine::manifest::CharacterAsset {
                        path: rel_path,
                        scale: None,
                    },
                );
            }
            AssetImportKind::Audio => {
                manifest.assets.audio.insert(asset_name, rel_path);
            }
        }

        manifest
            .save(&manifest_path)
            .map_err(|err| format!("manifest save failed: {err}"))?;

        self.composer_image_cache.clear();
        self.composer_image_failures.clear();
        self.audio_duration_cache.clear();
        self.compilation_cache.invalidate();
        self.player_audio_backend = None;
        self.player_audio_root = None;
        self.record_editor_operation_now(
            "asset_imported",
            format!(
                "Imported {} asset from {} as {}",
                kind.label(),
                canonical_source.display(),
                rel_string
            ),
            Some(manifest_field_path),
            Some(canonical_source.to_string_lossy().to_string()),
            Some(rel_string.clone()),
            before_fingerprint,
        );
        Ok(rel_string)
    }

    pub(crate) fn remove_asset_from_manifest(
        &mut self,
        kind: AssetImportKind,
        name: &str,
    ) -> Result<(), String> {
        let before_fingerprint = self.current_authoring_fingerprint();
        let manifest_path = self
            .manifest_path
            .clone()
            .ok_or_else(|| "load a project manifest before removing assets".to_string())?;
        let manifest = self
            .manifest
            .as_mut()
            .ok_or_else(|| "load a project manifest before removing assets".to_string())?;
        let manifest_field_path = manifest_asset_field_path(kind, name);

        let removed = match kind {
            AssetImportKind::Background => manifest.assets.backgrounds.remove(name),
            AssetImportKind::Character => manifest
                .assets
                .characters
                .remove(name)
                .map(|asset| asset.path),
            AssetImportKind::Audio => manifest.assets.audio.remove(name),
        }
        .ok_or_else(|| format!("{} asset '{name}' is not in the manifest", kind.label()))?;
        let removed_path = removed.to_string_lossy().replace('\\', "/");

        manifest
            .save(&manifest_path)
            .map_err(|err| format!("manifest save failed: {err}"))?;

        self.composer_image_cache.clear();
        self.composer_image_failures.clear();
        self.audio_duration_cache.clear();
        self.compilation_cache.invalidate();
        self.player_audio_backend = None;
        self.player_audio_root = None;
        self.record_editor_operation_now(
            "asset_removed",
            format!("Removed {} asset '{}' from manifest", kind.label(), name),
            Some(manifest_field_path),
            Some(removed_path),
            Some("<removed>".to_string()),
            before_fingerprint,
        );
        Ok(())
    }

    pub(crate) fn apply_imported_asset_to_node(
        &mut self,
        node_id: u32,
        target: AssetFieldTarget,
        imported: String,
    ) -> Result<(), String> {
        let Some(node) = self.node_graph.get_node_mut(node_id) else {
            return Err(format!("node {node_id} no longer exists"));
        };

        let field_path = asset_node_field_path(node_id, target);
        let before_value;

        match (target, node) {
            (AssetFieldTarget::SceneBackground, StoryNode::Scene { background, .. }) => {
                before_value = stringify_optional_asset(background);
                *background = Some(imported.clone());
            }
            (AssetFieldTarget::SceneMusic, StoryNode::Scene { music, .. }) => {
                before_value = stringify_optional_asset(music);
                *music = Some(imported.clone());
            }
            (
                AssetFieldTarget::SceneCharacterExpression(idx),
                StoryNode::Scene { characters, .. },
            ) => {
                let Some(character) = characters.get_mut(idx) else {
                    return Err(format!("scene character index {idx} no longer exists"));
                };
                before_value = stringify_optional_asset(&character.expression);
                character.expression = Some(imported.clone());
            }
            (
                AssetFieldTarget::ScenePatchBackground,
                StoryNode::ScenePatch(visual_novel_engine::ScenePatchRaw { background, .. }),
            ) => {
                before_value = stringify_optional_asset(background);
                *background = Some(imported.clone());
            }
            (
                AssetFieldTarget::ScenePatchMusic,
                StoryNode::ScenePatch(visual_novel_engine::ScenePatchRaw { music, .. }),
            ) => {
                before_value = stringify_optional_asset(music);
                *music = Some(imported.clone());
            }
            (
                AssetFieldTarget::ScenePatchAddCharacterExpression(idx),
                StoryNode::ScenePatch(visual_novel_engine::ScenePatchRaw { add, .. }),
            ) => {
                let Some(character) = add.get_mut(idx) else {
                    return Err(format!(
                        "scene patch character index {idx} no longer exists"
                    ));
                };
                before_value = stringify_optional_asset(&character.expression);
                character.expression = Some(imported.clone());
            }
            (AssetFieldTarget::AudioActionAsset, StoryNode::AudioAction { asset, .. }) => {
                before_value = stringify_optional_asset(asset);
                *asset = Some(imported.clone());
            }
            _ => return Err("selected node does not support that asset field".to_string()),
        }

        self.queue_editor_operation_with_values(
            "field_edited",
            format!("Assigned imported asset {imported} to node {node_id}"),
            Some(field_path),
            Some(before_value),
            Some(imported),
        );
        self.node_graph.mark_modified();
        Ok(())
    }

    pub(crate) fn add_character_asset_to_node(
        &mut self,
        node_id: u32,
        name: String,
        imported: String,
        x: i32,
        y: i32,
    ) -> Result<(), String> {
        let Some(node) = self.node_graph.get_node_mut(node_id) else {
            return Err(format!("node {node_id} no longer exists"));
        };

        let character_name = normalized_character_name(&name, &imported);
        let (field_path, before_value, after_value) = match node {
            StoryNode::Scene { characters, .. } => {
                let (idx, before, after) =
                    upsert_character_asset(characters, &character_name, &imported, x, y);
                (
                    format!("graph.nodes[{node_id}].characters[{idx}]"),
                    before,
                    after,
                )
            }
            StoryNode::ScenePatch(visual_novel_engine::ScenePatchRaw { add, .. }) => {
                let (idx, before, after) =
                    upsert_character_asset(add, &character_name, &imported, x, y);
                (
                    format!("graph.nodes[{node_id}].patch.add[{idx}]"),
                    before,
                    after,
                )
            }
            _ => return Err("selected node does not accept character placement".to_string()),
        };

        self.queue_editor_operation_with_values(
            "field_edited",
            format!("Assigned character asset {imported} to node {node_id}"),
            Some(field_path),
            Some(before_value),
            Some(after_value),
        );
        self.node_graph.mark_modified();
        Ok(())
    }
}

fn pick_asset_file(kind: AssetImportKind, project_root: Option<&Path>) -> Option<PathBuf> {
    let mut dialog = rfd::FileDialog::new()
        .set_title(kind.dialog_title())
        .add_filter(kind.label(), kind.file_dialog_extensions());
    if let Some(root) = project_root {
        dialog = dialog.set_directory(root);
    }
    dialog.pick_file()
}

fn copy_external_asset(
    source: &Path,
    project_root: &Path,
    kind: AssetImportKind,
    extension: &str,
) -> Result<PathBuf, String> {
    let dest_dir = PathBuf::from(kind.destination_dir());
    let dest_rel = unique_destination_path(project_root, &dest_dir, source, extension);
    let dest_abs = project_root.join(&dest_rel);
    if let Some(parent) = dest_abs.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|err| format!("failed to create asset directory: {err}"))?;
    }
    std::fs::copy(source, &dest_abs)
        .map_err(|err| format!("failed to copy external asset into project: {err}"))?;
    Ok(dest_rel)
}

fn unique_destination_path(
    project_root: &Path,
    dest_dir: &Path,
    source: &Path,
    extension: &str,
) -> PathBuf {
    let base = sanitized_stem(source);
    let mut suffix = 0usize;
    loop {
        let name = if suffix == 0 {
            format!("{base}.{extension}")
        } else {
            format!("{base}-{suffix}.{extension}")
        };
        let rel = dest_dir.join(name);
        if !project_root.join(&rel).exists() {
            return rel;
        }
        suffix += 1;
    }
}

fn unique_manifest_name(
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

fn normalized_extension(path: &Path) -> Option<String> {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase())
}

fn manifest_asset_field_path(kind: AssetImportKind, asset_name: &str) -> String {
    let table = match kind {
        AssetImportKind::Background => "backgrounds",
        AssetImportKind::Character => "characters",
        AssetImportKind::Audio => "audio",
    };
    format!("manifest.assets.{table}[{asset_name}]")
}

fn asset_node_field_path(node_id: u32, target: AssetFieldTarget) -> String {
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

fn stringify_optional_asset(value: &Option<String>) -> String {
    value.clone().unwrap_or_else(|| "<none>".to_string())
}

fn normalized_character_name(name: &str, asset_path: &str) -> String {
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

fn upsert_character_asset(
    characters: &mut Vec<visual_novel_engine::CharacterPlacementRaw>,
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
    characters.push(visual_novel_engine::CharacterPlacementRaw {
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

fn character_snapshot(character: &visual_novel_engine::CharacterPlacementRaw) -> String {
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
