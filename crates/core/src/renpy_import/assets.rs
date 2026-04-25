use std::collections::BTreeSet;
use std::fs;
use std::path::{Component, Path, PathBuf};

use crate::error::{VnError, VnResult};
use crate::event::EventRaw;

use super::types::{compose_issue_docs, next_trace_id, ImportArea, ImportIssue, ImportPhase};

pub(super) fn rewrite_and_copy_assets(
    project_root: &Path,
    scan_root: &Path,
    output_root: &Path,
    events: &mut [EventRaw],
    trace_seq: &mut usize,
) -> VnResult<Vec<ImportIssue>> {
    let project_root = project_root.canonicalize().map_err(|e| {
        VnError::InvalidScript(format!(
            "renpy import: canonicalize {}: {e}",
            project_root.display()
        ))
    })?;
    let scan_root = scan_root.canonicalize().map_err(|e| {
        VnError::InvalidScript(format!(
            "renpy import: canonicalize {}: {e}",
            scan_root.display()
        ))
    })?;
    let assets_out = output_root.join("assets");
    fs::create_dir_all(&assets_out).map_err(|e| {
        VnError::InvalidScript(format!("renpy import: mkdir {}: {e}", assets_out.display()))
    })?;
    let mut copied = BTreeSet::new();
    let mut issues = Vec::new();

    for event in events {
        match event {
            EventRaw::Scene(scene) => {
                if let Some(bg) = scene.background.as_mut() {
                    rewrite_path_and_copy(
                        &project_root,
                        &scan_root,
                        &assets_out,
                        bg,
                        &mut copied,
                        &mut issues,
                        trace_seq,
                    );
                }
                if let Some(music) = scene.music.as_mut() {
                    rewrite_path_and_copy(
                        &project_root,
                        &scan_root,
                        &assets_out,
                        music,
                        &mut copied,
                        &mut issues,
                        trace_seq,
                    );
                }
                for character in &mut scene.characters {
                    if looks_like_path(&character.name) {
                        rewrite_path_and_copy(
                            &project_root,
                            &scan_root,
                            &assets_out,
                            &mut character.name,
                            &mut copied,
                            &mut issues,
                            trace_seq,
                        );
                    }
                    if let Some(expr) = character.expression.as_mut() {
                        if looks_like_path(expr) {
                            rewrite_path_and_copy(
                                &project_root,
                                &scan_root,
                                &assets_out,
                                expr,
                                &mut copied,
                                &mut issues,
                                trace_seq,
                            );
                        }
                    }
                }
            }
            EventRaw::Patch(patch) => {
                if let Some(bg) = patch.background.as_mut() {
                    rewrite_path_and_copy(
                        &project_root,
                        &scan_root,
                        &assets_out,
                        bg,
                        &mut copied,
                        &mut issues,
                        trace_seq,
                    );
                }
                if let Some(music) = patch.music.as_mut() {
                    rewrite_path_and_copy(
                        &project_root,
                        &scan_root,
                        &assets_out,
                        music,
                        &mut copied,
                        &mut issues,
                        trace_seq,
                    );
                }
                for character in &mut patch.add {
                    if looks_like_path(&character.name) {
                        rewrite_path_and_copy(
                            &project_root,
                            &scan_root,
                            &assets_out,
                            &mut character.name,
                            &mut copied,
                            &mut issues,
                            trace_seq,
                        );
                    }
                    if let Some(expr) = character.expression.as_mut() {
                        if looks_like_path(expr) {
                            rewrite_path_and_copy(
                                &project_root,
                                &scan_root,
                                &assets_out,
                                expr,
                                &mut copied,
                                &mut issues,
                                trace_seq,
                            );
                        }
                    }
                }
            }
            EventRaw::AudioAction(action) => {
                if let Some(asset) = action.asset.as_mut() {
                    rewrite_path_and_copy(
                        &project_root,
                        &scan_root,
                        &assets_out,
                        asset,
                        &mut copied,
                        &mut issues,
                        trace_seq,
                    );
                }
            }
            _ => {}
        }
    }

    Ok(issues)
}

fn looks_like_path(value: &str) -> bool {
    value.contains('/') || value.contains('\\') || value.contains('.')
}

fn rewrite_path_and_copy(
    project_root: &Path,
    scan_root: &Path,
    assets_out: &Path,
    path_value: &mut String,
    copied: &mut BTreeSet<String>,
    issues: &mut Vec<ImportIssue>,
    trace_seq: &mut usize,
) {
    let original = path_value.trim();
    if original.is_empty() {
        return;
    }

    if original.starts_with("http://") || original.starts_with("https://") {
        push_asset_issue(
            issues,
            "asset_url_unsupported",
            format!("Skipping URL asset path '{original}'"),
            trace_seq,
        );
        return;
    }

    let normalized = match normalize_asset_rel_path(original) {
        Ok(path) => path,
        Err(code) => {
            push_asset_issue(
                issues,
                code,
                format!("Skipping unsafe asset path '{original}'"),
                trace_seq,
            );
            return;
        }
    };

    if normalized.is_empty() {
        return;
    }

    let resolved = match resolve_existing_asset_path(project_root, scan_root, &normalized) {
        Ok(Some(resolved)) => resolved,
        Ok(None) => {
            if is_symbolic_asset_token(&normalized) {
                return;
            }
            push_asset_issue(
                issues,
                "asset_not_found",
                format!("Asset not found during import: '{normalized}'"),
                trace_seq,
            );
            return;
        }
        Err(code) => {
            push_asset_issue(
                issues,
                code,
                format!("Asset resolves outside import root: '{normalized}'"),
                trace_seq,
            );
            return;
        }
    };
    let (source, resolved_rel) = resolved;

    let destination = assets_out.join(&resolved_rel);
    if !copied.contains(&resolved_rel) {
        if let Some(parent) = destination.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if fs::copy(&source, &destination).is_ok() {
            copied.insert(resolved_rel.clone());
        } else {
            push_asset_issue(
                issues,
                "asset_copy_failed",
                format!(
                    "Failed to copy asset '{}' -> '{}'",
                    source.display(),
                    destination.display()
                ),
                trace_seq,
            );
            return;
        }
    }
    *path_value = format!("assets/{}", resolved_rel);
}

fn normalize_asset_rel_path(raw: &str) -> Result<String, &'static str> {
    let mut parts = Vec::new();
    let path = Path::new(raw.trim());
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::Normal(segment) => parts.push(segment.to_string_lossy().to_string()),
            Component::ParentDir => return Err("asset_path_traversal"),
            Component::RootDir => return Err("asset_absolute_path"),
            _ => return Err("asset_absolute_path"),
        }
    }

    if parts.is_empty() {
        return Err("asset_invalid_path");
    }
    Ok(parts.join("/"))
}

fn push_asset_issue(
    issues: &mut Vec<ImportIssue>,
    code: &str,
    message: String,
    trace_seq: &mut usize,
) {
    let docs = compose_issue_docs(code, ImportArea::Assets, ImportPhase::AssetRewrite, None);
    issues.push(ImportIssue {
        severity: "warning".to_string(),
        code: code.to_string(),
        message,
        file: None,
        line: None,
        column: None,
        area: ImportArea::Assets.as_str().to_string(),
        phase: ImportPhase::AssetRewrite.as_str().to_string(),
        snippet: None,
        path_display: "assets".to_string(),
        fallback_applied: None,
        trace_id: next_trace_id(trace_seq),
        root_cause: docs.root_cause,
        how_to_fix: docs.how_to_fix,
        docs_ref: docs.docs_ref,
    });
}

fn resolve_existing_asset_path(
    project_root: &Path,
    scan_root: &Path,
    normalized: &str,
) -> Result<Option<(PathBuf, String)>, &'static str> {
    for candidate in asset_resolution_candidates(normalized) {
        let scan_candidate = scan_root.join(&candidate);
        if scan_candidate.exists() {
            let canonical_candidate = scan_candidate
                .canonicalize()
                .map_err(|_| "asset_path_traversal")?;
            if !canonical_candidate.starts_with(project_root) {
                return Err("asset_path_traversal");
            }
            return Ok(Some((canonical_candidate, candidate)));
        }

        let project_candidate = project_root.join(&candidate);
        if project_candidate.exists() {
            let canonical_candidate = project_candidate
                .canonicalize()
                .map_err(|_| "asset_path_traversal")?;
            if !canonical_candidate.starts_with(project_root) {
                return Err("asset_path_traversal");
            }
            return Ok(Some((canonical_candidate, candidate)));
        }
    }

    Ok(None)
}

fn asset_resolution_candidates(normalized: &str) -> Vec<String> {
    const ASSET_EXTENSIONS: [&str; 10] = [
        "png", "jpg", "jpeg", "webp", "bmp", "ogg", "opus", "mp3", "wav", "flac",
    ];
    let mut candidates = vec![normalized.to_string()];
    if Path::new(normalized).extension().is_some() {
        return candidates;
    }
    for ext in ASSET_EXTENSIONS {
        candidates.push(format!("{normalized}.{ext}"));
    }
    candidates
}

fn is_symbolic_asset_token(value: &str) -> bool {
    if value.contains('/') || value.contains('\\') || value.contains('.') {
        return false;
    }
    matches!(
        value.to_ascii_lowercase().as_str(),
        "black" | "white" | "gray" | "grey" | "red" | "green" | "blue"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{EventRaw, SceneUpdateRaw};
    use tempfile::tempdir;

    fn create_escape_symlink(link: &Path, target: &Path) -> bool {
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(target, link).is_ok()
        }

        #[cfg(windows)]
        {
            std::os::windows::fs::symlink_file(target, link).is_ok()
        }

        #[cfg(not(any(unix, windows)))]
        {
            let _ = link;
            let _ = target;
            false
        }
    }

    #[test]
    fn rewrite_and_copy_assets_rejects_symlink_escape_outside_root() {
        let dir = tempdir().expect("tempdir");
        let project_root = dir.path().join("project");
        let game_root = project_root.join("game");
        let asset_root = game_root.join("bg");
        fs::create_dir_all(&asset_root).expect("mkdir bg");

        let outside_asset = dir.path().join("secret.png");
        fs::write(&outside_asset, b"secret").expect("write outside asset");
        let symlink_path = asset_root.join("escape.png");
        if !create_escape_symlink(&symlink_path, &outside_asset) {
            eprintln!("symlink creation not supported on this platform");
            return;
        }

        let mut events = vec![EventRaw::Scene(SceneUpdateRaw {
            background: Some("bg/escape".to_string()),
            music: None,
            characters: Vec::new(),
        })];
        let output_root = dir.path().join("out");
        let mut trace_seq = 0usize;

        let issues = rewrite_and_copy_assets(
            &project_root,
            &game_root,
            &output_root,
            &mut events,
            &mut trace_seq,
        )
        .expect("asset rewrite should complete");

        assert!(
            issues
                .iter()
                .any(|issue| issue.code == "asset_path_traversal"),
            "symlink escape must be reported"
        );
        assert!(
            !output_root
                .join("assets")
                .join("bg")
                .join("escape.png")
                .exists(),
            "escaped asset must not be copied"
        );
        assert_eq!(
            events
                .iter()
                .find_map(|event| match event {
                    EventRaw::Scene(scene) => scene.background.clone(),
                    _ => None,
                })
                .as_deref(),
            Some("bg/escape"),
            "failed rewrite must not rewrite the original asset path"
        );
    }
}
