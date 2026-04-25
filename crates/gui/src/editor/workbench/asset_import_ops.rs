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
        self.player_audio_backend = None;
        self.player_audio_root = None;
        Ok(rel_string)
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

        match (target, node) {
            (AssetFieldTarget::SceneBackground, StoryNode::Scene { background, .. }) => {
                *background = Some(imported);
            }
            (AssetFieldTarget::SceneMusic, StoryNode::Scene { music, .. }) => {
                *music = Some(imported);
            }
            (
                AssetFieldTarget::SceneCharacterExpression(idx),
                StoryNode::Scene { characters, .. },
            ) => {
                let Some(character) = characters.get_mut(idx) else {
                    return Err(format!("scene character index {idx} no longer exists"));
                };
                character.expression = Some(imported);
            }
            (
                AssetFieldTarget::ScenePatchBackground,
                StoryNode::ScenePatch(visual_novel_engine::ScenePatchRaw { background, .. }),
            ) => {
                *background = Some(imported);
            }
            (
                AssetFieldTarget::ScenePatchMusic,
                StoryNode::ScenePatch(visual_novel_engine::ScenePatchRaw { music, .. }),
            ) => {
                *music = Some(imported);
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
                character.expression = Some(imported);
            }
            (AssetFieldTarget::AudioActionAsset, StoryNode::AudioAction { asset, .. }) => {
                *asset = Some(imported);
            }
            _ => return Err("selected node does not support that asset field".to_string()),
        }

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
