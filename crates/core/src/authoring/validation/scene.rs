use crate::{CharacterPlacementRaw, ScenePatchRaw};

use super::assets::validate_asset_at;
use super::{LintCode, LintIssue, NodeGraph, ValidationPhase};

pub(super) fn validate_scene_profiles<F>(
    graph: &NodeGraph,
    asset_exists: &F,
    issues: &mut Vec<LintIssue>,
) where
    F: Fn(&str) -> bool,
{
    for (profile_id, profile) in graph.scene_profiles() {
        validate_asset_at(
            None,
            &profile.background,
            "background",
            format!("graph.scene_profiles[{profile_id}].background"),
            asset_exists,
            issues,
        );
        validate_asset_at(
            None,
            &profile.music,
            "music",
            format!("graph.scene_profiles[{profile_id}].music"),
            asset_exists,
            issues,
        );
        validate_scene_profile_characters(
            profile_id,
            "characters",
            &profile.characters,
            asset_exists,
            issues,
        );
        for (layer_idx, layer) in profile.layers.iter().enumerate() {
            validate_asset_at(
                None,
                &layer.background,
                "background",
                format!("graph.scene_profiles[{profile_id}].layers[{layer_idx}].background"),
                asset_exists,
                issues,
            );
            validate_scene_profile_characters(
                profile_id,
                &format!("layers[{layer_idx}].characters"),
                &layer.characters,
                asset_exists,
                issues,
            );
        }
        for (pose_idx, pose) in profile.poses.iter().enumerate() {
            validate_asset_at(
                None,
                &Some(pose.image.clone()),
                "character_expression",
                format!("graph.scene_profiles[{profile_id}].poses[{pose_idx}].image"),
                asset_exists,
                issues,
            );
            if pose.character.trim().is_empty() || pose.pose.trim().is_empty() {
                issues.push(
                    LintIssue::warning(
                        None,
                        ValidationPhase::Graph,
                        LintCode::EmptyCharacterName,
                        format!("Scene profile '{profile_id}' has an incomplete pose binding"),
                    )
                    .with_asset_path(Some(pose.image.clone()))
                    .with_field_path(format!(
                        "graph.scene_profiles[{profile_id}].poses[{pose_idx}]"
                    )),
                );
            }
        }
    }
}

fn validate_scene_profile_characters<F>(
    profile_id: &str,
    owner_path: &str,
    characters: &[CharacterPlacementRaw],
    asset_exists: &F,
    issues: &mut Vec<LintIssue>,
) where
    F: Fn(&str) -> bool,
{
    for (character_idx, character) in characters.iter().enumerate() {
        if character.name.trim().is_empty() {
            issues.push(
                LintIssue::error(
                    None,
                    ValidationPhase::Graph,
                    LintCode::EmptyCharacterName,
                    format!("Scene profile '{profile_id}' has an empty character name"),
                )
                .with_field_path(format!(
                    "graph.scene_profiles[{profile_id}].{owner_path}[{character_idx}].name"
                )),
            );
        }
        validate_asset_at(
            None,
            &character.expression,
            "character_expression",
            format!("graph.scene_profiles[{profile_id}].{owner_path}[{character_idx}].expression"),
            asset_exists,
            issues,
        );
        validate_character_scale(None, &character.scale, issues);
    }
}

pub(super) fn validate_scene<F>(
    id: u32,
    background: &Option<String>,
    music: &Option<String>,
    characters: &[CharacterPlacementRaw],
    asset_exists: &F,
    issues: &mut Vec<LintIssue>,
) where
    F: Fn(&str) -> bool,
{
    validate_asset_at(
        Some(id),
        background,
        "background",
        format!("graph.nodes[{id}].background"),
        asset_exists,
        issues,
    );
    validate_asset_at(
        Some(id),
        music,
        "music",
        format!("graph.nodes[{id}].music"),
        asset_exists,
        issues,
    );
    if characters
        .iter()
        .any(|character| character.name.trim().is_empty())
    {
        issues.push(LintIssue::error(
            Some(id),
            ValidationPhase::Graph,
            LintCode::EmptyCharacterName,
            "Scene has an empty character name",
        ));
    }
    for (character_idx, character) in characters.iter().enumerate() {
        validate_asset_at(
            Some(id),
            &character.expression,
            "character_expression",
            format!("graph.nodes[{id}].characters[{character_idx}].expression"),
            asset_exists,
            issues,
        );
        validate_character_scale(Some(id), &character.scale, issues);
    }
}

pub(super) fn validate_scene_patch<F>(
    id: u32,
    patch: &ScenePatchRaw,
    asset_exists: &F,
    issues: &mut Vec<LintIssue>,
) where
    F: Fn(&str) -> bool,
{
    validate_asset_at(
        Some(id),
        &patch.background,
        "background",
        format!("graph.nodes[{id}].patch.background"),
        asset_exists,
        issues,
    );
    validate_asset_at(
        Some(id),
        &patch.music,
        "music",
        format!("graph.nodes[{id}].patch.music"),
        asset_exists,
        issues,
    );
    if patch
        .add
        .iter()
        .any(|character| character.name.trim().is_empty())
        || patch
            .update
            .iter()
            .any(|character| character.name.trim().is_empty())
    {
        issues.push(LintIssue::error(
            Some(id),
            ValidationPhase::Graph,
            LintCode::EmptyCharacterName,
            "Scene patch has an empty character name",
        ));
    }
    if patch.remove.iter().any(|name| name.trim().is_empty()) {
        issues.push(LintIssue::warning(
            Some(id),
            ValidationPhase::Graph,
            LintCode::EmptyCharacterName,
            "Scene patch has an empty character name in remove-list",
        ));
    }
    for (character_idx, character) in patch.add.iter().enumerate() {
        validate_asset_at(
            Some(id),
            &character.expression,
            "character_expression",
            format!("graph.nodes[{id}].patch.add[{character_idx}].expression"),
            asset_exists,
            issues,
        );
        validate_character_scale(Some(id), &character.scale, issues);
    }
    for (character_idx, character) in patch.update.iter().enumerate() {
        validate_asset_at(
            Some(id),
            &character.expression,
            "character_expression",
            format!("graph.nodes[{id}].patch.update[{character_idx}].expression"),
            asset_exists,
            issues,
        );
    }
}

pub(super) fn validate_character_scale(
    node_id: Option<u32>,
    scale: &Option<f32>,
    issues: &mut Vec<LintIssue>,
) {
    if scale.is_some_and(|value| !value.is_finite() || value <= 0.0) {
        issues.push(LintIssue::error(
            node_id,
            ValidationPhase::Graph,
            LintCode::InvalidCharacterScale,
            "Character scale is invalid",
        ));
    }
}
