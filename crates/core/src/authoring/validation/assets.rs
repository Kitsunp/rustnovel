use std::path::Path;

use crate::authoring::{
    DiagnosticTarget, FieldPath, LintCode, LintIssue, SemanticValue, SemanticValueKind,
    ValidationPhase,
};

pub(super) fn validate_asset<F>(
    node_id: Option<u32>,
    value: &Option<String>,
    label: &str,
    asset_exists: &F,
    issues: &mut Vec<LintIssue>,
) where
    F: Fn(&str) -> bool,
{
    let field_path = node_id
        .map(|id| format!("graph.nodes[{id}].{label}"))
        .unwrap_or_else(|| format!("graph.scene_profiles[].{label}"));
    validate_asset_at(node_id, value, label, field_path, asset_exists, issues)
}

pub(super) fn validate_asset_at<F>(
    node_id: Option<u32>,
    value: &Option<String>,
    label: &str,
    field_path: impl Into<String>,
    asset_exists: &F,
    issues: &mut Vec<LintIssue>,
) where
    F: Fn(&str) -> bool,
{
    let Some(path) = value else {
        return;
    };
    let field_path = FieldPath::new(field_path);
    let target = DiagnosticTarget::AssetRef {
        node_id,
        field_path: field_path.clone(),
        asset_path: path.clone(),
    };
    let semantic_value = SemanticValue::new(
        SemanticValueKind::AssetRef,
        path.clone(),
        field_path.value.clone(),
    );
    if path.trim().is_empty() {
        let code = if label == "background" {
            LintCode::SceneBackgroundEmpty
        } else {
            LintCode::AudioAssetEmpty
        };
        issues.push(
            LintIssue::warning(node_id, ValidationPhase::Graph, code, "Asset path is empty")
                .with_target(target)
                .with_field_path(field_path.value)
                .with_semantic_value(semantic_value)
                .with_evidence_trace(),
        );
    } else if is_unsafe_asset_ref(path) {
        issues.push(
            LintIssue::error(
                node_id,
                ValidationPhase::Graph,
                LintCode::UnsafeAssetPath,
                "Asset path is unsafe",
            )
            .with_asset_path(Some(path.clone()))
            .with_target(target)
            .with_field_path(field_path.value)
            .with_semantic_value(semantic_value)
            .with_evidence_trace(),
        );
    } else if should_probe_asset_exists(path) && !asset_exists(path) {
        issues.push(
            LintIssue::error(
                node_id,
                ValidationPhase::Graph,
                LintCode::AssetReferenceMissing,
                "Asset reference does not exist",
            )
            .with_asset_path(Some(path.clone()))
            .with_target(target)
            .with_field_path(field_path.value)
            .with_semantic_value(semantic_value)
            .with_evidence_trace(),
        );
    }
}

pub fn default_asset_exists(path: &str) -> bool {
    let candidate = Path::new(path.trim());
    if candidate.is_absolute() {
        return candidate.is_file();
    }

    match std::env::current_dir() {
        Ok(cwd) => cwd.join(candidate).is_file(),
        Err(_) => candidate.is_file(),
    }
}

pub fn asset_exists_from_project_root(project_root: &Path, path: &str) -> bool {
    let candidate = Path::new(path.trim());
    if candidate.is_absolute() {
        return candidate.is_file();
    }
    project_root.join(candidate).is_file()
}

pub fn should_probe_asset_exists(path: &str) -> bool {
    let p = path.trim();
    if p.is_empty() {
        return false;
    }

    p.contains('/')
        || p.contains('\\')
        || Path::new(p).extension().is_some()
        || p.starts_with("assets/")
        || p.starts_with("assets\\")
}

pub fn is_unsafe_asset_ref(path: &str) -> bool {
    let path = path.trim();
    if path.is_empty() {
        return false;
    }
    let lower = path.to_ascii_lowercase();
    path.starts_with('/')
        || path.starts_with('\\')
        || lower.contains("://")
        || path.chars().nth(1).is_some_and(|second| {
            second == ':' && path.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
        })
        || path.split(['/', '\\']).any(|part| part == "..")
}
