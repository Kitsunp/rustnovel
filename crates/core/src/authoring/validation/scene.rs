use crate::{CharacterPlacementRaw, ScenePatchRaw};

use super::assets::validate_asset;
use super::{LintCode, LintIssue, NodeGraph, ValidationPhase};

pub(super) fn validate_scene_profiles<F>(
    graph: &NodeGraph,
    asset_exists: &F,
    issues: &mut Vec<LintIssue>,
) where
    F: Fn(&str) -> bool,
{
    for (profile_id, profile) in graph.scene_profiles() {
        validate_asset(
            None,
            &profile.background,
            "background",
            asset_exists,
            issues,
        );
        validate_asset(None, &profile.music, "music", asset_exists, issues);
        validate_scene_profile_characters(profile_id, &profile.characters, asset_exists, issues);
        for layer in &profile.layers {
            validate_asset(None, &layer.background, "background", asset_exists, issues);
            validate_scene_profile_characters(profile_id, &layer.characters, asset_exists, issues);
        }
        for pose in &profile.poses {
            validate_asset(
                None,
                &Some(pose.image.clone()),
                "character_expression",
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
                    .with_asset_path(Some(pose.image.clone())),
                );
            }
        }
    }
}

fn validate_scene_profile_characters<F>(
    profile_id: &str,
    characters: &[CharacterPlacementRaw],
    asset_exists: &F,
    issues: &mut Vec<LintIssue>,
) where
    F: Fn(&str) -> bool,
{
    for character in characters {
        if character.name.trim().is_empty() {
            issues.push(LintIssue::error(
                None,
                ValidationPhase::Graph,
                LintCode::EmptyCharacterName,
                format!("Scene profile '{profile_id}' has an empty character name"),
            ));
        }
        validate_asset(
            None,
            &character.expression,
            "character_expression",
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
    validate_asset(Some(id), background, "background", asset_exists, issues);
    validate_asset(Some(id), music, "music", asset_exists, issues);
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
    for character in characters {
        validate_asset(
            Some(id),
            &character.expression,
            "character_expression",
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
    validate_asset(
        Some(id),
        &patch.background,
        "background",
        asset_exists,
        issues,
    );
    validate_asset(Some(id), &patch.music, "music", asset_exists, issues);
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
    for character in &patch.add {
        validate_asset(
            Some(id),
            &character.expression,
            "character_expression",
            asset_exists,
            issues,
        );
        validate_character_scale(Some(id), &character.scale, issues);
    }
    for character in &patch.update {
        validate_asset(
            Some(id),
            &character.expression,
            "character_expression",
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
