use std::path::Path;

use super::*;
use crate::editor::{AssetFieldTarget, AssetImportKind, StoryNode};
use visual_novel_engine::authoring::{AuthoringCommand, AuthoringDocumentCommand};

#[path = "asset_import_helpers.rs"]
mod asset_import_helpers;
use asset_import_helpers::*;

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

    pub fn import_asset_for_node_dialog(
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
                        if let Err(err) = self.sync_graph_to_script() {
                            self.toast = Some(ToastState::error(format!(
                                "{} imported but graph sync failed: {err}",
                                kind.label()
                            )));
                        }
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

    pub fn import_asset_file(
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
        self.resource_service.clear();
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

    pub fn remove_asset_from_manifest(
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
        self.resource_service.clear();
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

    pub fn apply_imported_asset_to_node(
        &mut self,
        node_id: u32,
        target: AssetFieldTarget,
        imported: String,
    ) -> Result<(), String> {
        let mut replacement = self
            .node_graph
            .get_node(node_id)
            .cloned()
            .ok_or_else(|| format!("node {node_id} no longer exists"))?;

        let field_path = asset_node_field_path(node_id, target);
        let before_value;

        match (target, &mut replacement) {
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
                StoryNode::ScenePatch(visual_novel_engine::runtime::ScenePatchRaw {
                    background,
                    ..
                }),
            ) => {
                before_value = stringify_optional_asset(background);
                *background = Some(imported.clone());
            }
            (
                AssetFieldTarget::ScenePatchMusic,
                StoryNode::ScenePatch(visual_novel_engine::runtime::ScenePatchRaw {
                    music, ..
                }),
            ) => {
                before_value = stringify_optional_asset(music);
                *music = Some(imported.clone());
            }
            (
                AssetFieldTarget::ScenePatchAddCharacterExpression(idx),
                StoryNode::ScenePatch(visual_novel_engine::runtime::ScenePatchRaw { add, .. }),
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
        self.apply_node_replacement_with_command_bus(node_id, replacement)?;

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

    pub fn assign_manifest_asset_to_selected_node(
        &mut self,
        kind: AssetImportKind,
        name: &str,
        path: &str,
    ) -> Result<u32, String> {
        let node_id = self
            .selected_node
            .or(self.node_graph.selected)
            .ok_or_else(|| "select a scene, patch, or audio node first".to_string())?;
        let node = self
            .node_graph
            .get_node(node_id)
            .ok_or_else(|| format!("node {node_id} no longer exists"))?;
        let asset_path = path.to_string();
        match (kind, node) {
            (AssetImportKind::Background, StoryNode::Scene { .. }) => {
                self.apply_imported_asset_to_node(
                    node_id,
                    AssetFieldTarget::SceneBackground,
                    asset_path,
                )?;
            }
            (AssetImportKind::Background, StoryNode::ScenePatch(_)) => {
                self.apply_imported_asset_to_node(
                    node_id,
                    AssetFieldTarget::ScenePatchBackground,
                    asset_path,
                )?;
            }
            (AssetImportKind::Audio, StoryNode::Scene { .. }) => {
                self.apply_imported_asset_to_node(
                    node_id,
                    AssetFieldTarget::SceneMusic,
                    asset_path,
                )?;
            }
            (AssetImportKind::Audio, StoryNode::ScenePatch(_)) => {
                self.apply_imported_asset_to_node(
                    node_id,
                    AssetFieldTarget::ScenePatchMusic,
                    asset_path,
                )?;
            }
            (AssetImportKind::Audio, StoryNode::AudioAction { .. }) => {
                self.apply_imported_asset_to_node(
                    node_id,
                    AssetFieldTarget::AudioActionAsset,
                    asset_path,
                )?;
            }
            (AssetImportKind::Character, StoryNode::Scene { .. } | StoryNode::ScenePatch(_)) => {
                self.add_character_asset_to_node(node_id, name.to_string(), asset_path, 640, 360)?;
            }
            _ => {
                return Err(format!(
                    "{} asset cannot be assigned to selected {} node",
                    kind.label(),
                    node.type_name()
                ));
            }
        }
        self.node_graph.set_single_selection(Some(node_id));
        self.selected_node = Some(node_id);
        self.sync_graph_to_script()
            .map_err(|err| format!("asset assigned but graph sync failed: {err}"))?;
        Ok(node_id)
    }

    pub fn add_character_asset_to_node(
        &mut self,
        node_id: u32,
        name: String,
        imported: String,
        x: i32,
        y: i32,
    ) -> Result<(), String> {
        let mut replacement = self
            .node_graph
            .get_node(node_id)
            .cloned()
            .ok_or_else(|| format!("node {node_id} no longer exists"))?;

        let character_name = normalized_character_name(&name, &imported);
        let (field_path, before_value, after_value) = match &mut replacement {
            StoryNode::Scene { characters, .. } => {
                let (idx, before, after) =
                    upsert_character_asset(characters, &character_name, &imported, x, y);
                (
                    format!("graph.nodes[{node_id}].characters[{idx}]"),
                    before,
                    after,
                )
            }
            StoryNode::ScenePatch(visual_novel_engine::runtime::ScenePatchRaw { add, .. }) => {
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
        self.apply_node_replacement_with_command_bus(node_id, replacement)?;

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

    fn apply_node_replacement_with_command_bus(
        &mut self,
        node_id: u32,
        replacement: StoryNode,
    ) -> Result<(), String> {
        self.apply_authoring_document_command(AuthoringDocumentCommand::Graph(
            AuthoringCommand::EditNode {
                node_id,
                replacement,
            },
        ))?;
        Ok(())
    }
}
