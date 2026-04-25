mod assets;
mod decorators;
mod flow;
mod parser;
mod postprocess;
mod syntax;
mod types;

pub use types::{
    ImportArea, ImportFallbackPolicy, ImportIssue, ImportPhase, ImportProfile, ImportRenpyOptions,
    ImportReport,
};

use std::fs;
use std::path::{Path, PathBuf};

use crate::error::{VnError, VnResult};
use crate::manifest::ProjectManifest;
use crate::script::ScriptRaw;
use types::ImportState;
use walkdir::WalkDir;

pub fn import_renpy_project(options: ImportRenpyOptions) -> VnResult<ImportReport> {
    let project_root = options
        .project_root
        .canonicalize()
        .map_err(|e| invalid_import(format!("canonicalize project_root: {e}")))?;
    let scan_root = detect_scan_root(&project_root);
    let output_root = options.output_root.clone();

    let mut files = collect_rpy_files(&project_root, &scan_root, &options)?;
    files.sort();

    let mut state = ImportState::default();
    for file in &files {
        state.parse_file(file)?;
    }

    if state.events.is_empty() {
        state.push_ext_call(
            "renpy_empty_project",
            vec!["No executable statements found".to_string()],
            None,
            "empty_project",
            "Project produced no supported executable events",
        );
    }

    postprocess::patch_missing_targets(&mut state);
    postprocess::enforce_start_label(&mut state, &options.entry_label);

    let asset_issues = assets::rewrite_and_copy_assets(
        &project_root,
        &scan_root,
        &output_root,
        &mut state.events,
        &mut state.trace_seq,
    )?;
    state.issues.extend(asset_issues);

    if (options.strict_mode || matches!(options.fallback_policy, ImportFallbackPolicy::Strict))
        && state.degraded_events > 0
    {
        let top_codes = summarize_top_issue_codes(&state.issues, 5);
        return Err(invalid_import(format!(
            "strict policy rejected degraded import: degraded_events={} top_codes=[{}]",
            state.degraded_events, top_codes
        )));
    }

    let script = ScriptRaw::new(state.events.clone(), state.labels.clone());
    let json = script
        .to_json()
        .map_err(|e| invalid_import(format!("serialize imported script: {e}")))?;

    fs::create_dir_all(&output_root).map_err(|e| {
        invalid_import(format!(
            "create output dir '{}': {e}",
            output_root.display()
        ))
    })?;

    let script_path = output_root.join("main.json");
    fs::write(&script_path, json)
        .map_err(|e| invalid_import(format!("write '{}': {e}", script_path.display())))?;

    let project_name = project_root
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "renpy_import".to_string());
    let manifest = ProjectManifest::new(&project_name, "renpy_import");
    let manifest_path = output_root.join("project.vnm");
    manifest
        .save(&manifest_path)
        .map_err(|e| invalid_import(format!("write '{}': {e}", manifest_path.display())))?;

    let report = ImportReport {
        importer_version: "0.2".to_string(),
        profile: options.profile.as_str().to_string(),
        strict_mode: options.strict_mode,
        fallback_policy: options.fallback_policy.as_str().to_string(),
        project_root: normalize_path(&project_root),
        scan_root: normalize_path(&scan_root),
        output_root: normalize_path(&output_root),
        include_patterns: options.include_patterns.clone(),
        exclude_patterns: options.exclude_patterns.clone(),
        files_scanned: files.len(),
        files_parsed: files.len(),
        events_generated: state.events.len(),
        labels_generated: state.labels.len(),
        degraded_events: state.degraded_events,
        issues_by_code: summarize_issues_by_code(&state.issues),
        issues_by_area: summarize_issues_by_area(&state.issues),
        issues: state.issues,
    };

    let report_path = options
        .report_path
        .unwrap_or_else(|| output_root.join("import_report.json"));
    let report_json = serde_json::to_string_pretty(&report)
        .map_err(|e| invalid_import(format!("serialize import report: {e}")))?;
    fs::write(&report_path, report_json)
        .map_err(|e| invalid_import(format!("write '{}': {e}", report_path.display())))?;

    Ok(report)
}

fn collect_rpy_files(
    project_root: &Path,
    scan_root: &Path,
    options: &ImportRenpyOptions,
) -> VnResult<Vec<PathBuf>> {
    let mut files = Vec::new();
    for entry in WalkDir::new(scan_root).into_iter().filter_map(Result::ok) {
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.into_path();
        if path
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case("rpy"))
            && should_include_file(project_root, scan_root, &path, options)
        {
            files.push(path);
        }
    }
    Ok(files)
}

fn should_include_file(
    project_root: &Path,
    scan_root: &Path,
    path: &Path,
    options: &ImportRenpyOptions,
) -> bool {
    let include_tl = options.include_tl.unwrap_or(match options.profile {
        ImportProfile::StoryFirst => false,
        ImportProfile::Full => true,
        ImportProfile::Custom => true,
    });
    let include_ui = options.include_ui.unwrap_or(match options.profile {
        ImportProfile::StoryFirst => false,
        ImportProfile::Full => true,
        ImportProfile::Custom => true,
    });

    let normalized_rel_scan = path
        .strip_prefix(scan_root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
        .to_ascii_lowercase();
    let normalized_rel_project = path
        .strip_prefix(project_root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
        .to_ascii_lowercase();

    let is_tl =
        is_translation_file(&normalized_rel_scan) || is_translation_file(&normalized_rel_project);
    if is_tl && !include_tl {
        return false;
    }

    let is_ui = is_ui_file(&normalized_rel_scan) || is_ui_file(&normalized_rel_project);
    if is_ui && !include_ui {
        return false;
    }

    if !options.include_patterns.is_empty()
        && !matches_any_pattern_multi(
            &[
                normalized_rel_project.as_str(),
                normalized_rel_scan.as_str(),
            ],
            &options.include_patterns,
        )
    {
        return false;
    }

    if matches_any_pattern_multi(
        &[
            normalized_rel_project.as_str(),
            normalized_rel_scan.as_str(),
        ],
        &options.exclude_patterns,
    ) {
        return false;
    }

    true
}

fn normalize_path(path: &Path) -> String {
    path.to_string_lossy()
        .replace('\\', "/")
        .trim_start_matches("//?/")
        .trim_start_matches("\\\\?/")
        .to_string()
}

fn summarize_issues_by_code(issues: &[ImportIssue]) -> std::collections::BTreeMap<String, usize> {
    let mut out = std::collections::BTreeMap::new();
    for issue in issues {
        *out.entry(issue.code.clone()).or_insert(0) += 1;
    }
    out
}

fn summarize_issues_by_area(issues: &[ImportIssue]) -> std::collections::BTreeMap<String, usize> {
    let mut out = std::collections::BTreeMap::new();
    for issue in issues {
        *out.entry(issue.area.clone()).or_insert(0) += 1;
    }
    for area in [
        ImportArea::Story,
        ImportArea::Ui,
        ImportArea::Translation,
        ImportArea::Assets,
        ImportArea::Flow,
        ImportArea::Other,
    ] {
        out.entry(area.as_str().to_string()).or_insert(0);
    }
    out
}

fn summarize_top_issue_codes(issues: &[ImportIssue], limit: usize) -> String {
    let mut entries: Vec<(String, usize)> = summarize_issues_by_code(issues).into_iter().collect();
    entries.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    entries
        .into_iter()
        .take(limit)
        .map(|(code, count)| format!("{code}:{count}"))
        .collect::<Vec<_>>()
        .join(",")
}

fn matches_any_pattern_multi(paths: &[&str], patterns: &[String]) -> bool {
    patterns
        .iter()
        .map(|value| value.trim().replace('\\', "/").to_ascii_lowercase())
        .filter(|value| !value.is_empty())
        .any(|pattern| paths.iter().any(|path| wildcard_match(&pattern, path)))
}

fn wildcard_match(pattern: &str, value: &str) -> bool {
    let p: Vec<char> = pattern.chars().collect();
    let v: Vec<char> = value.chars().collect();
    let mut dp = vec![vec![false; v.len() + 1]; p.len() + 1];
    dp[0][0] = true;

    for i in 1..=p.len() {
        if p[i - 1] == '*' {
            dp[i][0] = dp[i - 1][0];
        }
    }

    for i in 1..=p.len() {
        for j in 1..=v.len() {
            dp[i][j] = match p[i - 1] {
                '*' => dp[i - 1][j] || dp[i][j - 1],
                '?' => dp[i - 1][j - 1],
                ch => ch == v[j - 1] && dp[i - 1][j - 1],
            };
        }
    }

    dp[p.len()][v.len()]
}

fn invalid_import(message: impl Into<String>) -> VnError {
    VnError::InvalidScript(format!("renpy import: {}", message.into()))
}

fn detect_scan_root(project_root: &Path) -> PathBuf {
    let game_dir = project_root.join("game");
    if game_dir.is_dir() {
        return game_dir;
    }
    project_root.to_path_buf()
}

fn is_translation_file(path: &str) -> bool {
    path.starts_with("tl/")
        || path.contains("/tl/")
        || path.starts_with("game/tl/")
        || path.contains("/game/tl/")
}

fn is_ui_file(path: &str) -> bool {
    path == "gui.rpy"
        || path == "screens.rpy"
        || path == "options.rpy"
        || path.ends_with("/gui.rpy")
        || path.ends_with("/screens.rpy")
        || path.ends_with("/options.rpy")
        || path.starts_with("gui/")
        || path.contains("/gui/")
        || path.starts_with("game/gui/")
        || path.contains("/game/gui/")
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;

#[cfg(test)]
#[path = "tests_profile_and_security.rs"]
mod tests_profile_and_security;

#[cfg(test)]
#[path = "tests_asset_resolution.rs"]
mod tests_asset_resolution;
