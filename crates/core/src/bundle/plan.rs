use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::error::VnResult;
use crate::load_runtime_script_from_entry;
use crate::manifest::ProjectManifest;

use super::capabilities::build_capability_report;
use super::helpers::{
    canonicalize_within_root, invalid_bundle, normalize_path_display, sanitize_relative_path,
    to_hex,
};
use super::{BundleIntegrity, ExportBundleSpec, ExportPlan, ExportTargetPlatform};

pub fn build_export_plan(spec: &ExportBundleSpec) -> VnResult<ExportPlan> {
    let project_root = spec
        .project_root
        .canonicalize()
        .map_err(|e| invalid_bundle(format!("canonicalize project_root: {e}")))?;
    let output_root = if spec.output_root.exists() {
        spec.output_root
            .canonicalize()
            .map_err(|e| invalid_bundle(format!("canonicalize output_root: {e}")))?
    } else {
        spec.output_root.clone()
    };

    let manifest_path = project_root.join("project.vnm");
    if !manifest_path.is_file() {
        return Err(invalid_bundle(format!(
            "missing manifest '{}'",
            manifest_path.display()
        )));
    }
    let manifest = ProjectManifest::load(&manifest_path)
        .map_err(|e| invalid_bundle(format!("load manifest '{}': {e}", manifest_path.display())))?;

    let entry_script = spec
        .entry_script
        .clone()
        .unwrap_or_else(|| PathBuf::from(manifest.settings.entry_point.clone()));
    let entry_script = sanitize_relative_path(&entry_script, "entry_script")?;
    let script_source_path =
        canonicalize_within_root(&project_root, &entry_script, "entry_script")?;
    let script = load_runtime_script_from_entry(&script_source_path).map_err(|e| {
        invalid_bundle(format!(
            "load entry script '{}': {e}",
            entry_script.display()
        ))
    })?;
    let script_json = script
        .to_json()
        .map_err(|e| invalid_bundle(format!("serialize runtime script: {e}")))?;
    let mut hasher = Sha256::new();
    hasher.update(script_json.as_bytes());
    let script_sha256 = to_hex(hasher.finalize().as_slice());
    let capabilities = build_capability_report(&script, spec.runtime_artifact.is_some());

    let mut warnings = Vec::new();
    let mut errors = Vec::new();
    let runtime_artifact = match spec.runtime_artifact.as_deref() {
        Some(path) => match resolve_runtime_artifact_for_plan(path, &project_root) {
            Ok(path) => Some(normalize_path_display(&path)),
            Err(err) => {
                errors.push(err);
                None
            }
        },
        None => {
            warnings.push("missing_runtime_artifact".to_string());
            None
        }
    };
    let executable = runtime_artifact.as_ref().and_then(|path| {
        (spec.target_platform == ExportTargetPlatform::Windows
            && Path::new(path)
                .extension()
                .and_then(|value| value.to_str())
                .is_some_and(|extension| extension.eq_ignore_ascii_case("exe")))
        .then(|| "game.exe".to_string())
    });
    if spec.integrity == BundleIntegrity::HmacSha256
        && spec
            .hmac_key
            .as_deref()
            .is_none_or(|value| value.trim().is_empty())
    {
        errors.push("integrity=hmac_sha256 requires hmac_key".to_string());
    }

    Ok(ExportPlan {
        schema: "vnengine.export_plan.v1".to_string(),
        target_platform: spec.target_platform.as_str().to_string(),
        output_layout_version: spec.output_layout_version,
        project_root: normalize_path_display(&project_root),
        output_root: normalize_path_display(&output_root),
        entry_script: normalize_path_display(&entry_script),
        script_sha256,
        layout: vec![
            "scripts/compiled.vnscript.json".to_string(),
            "scripts/compiled.vnc".to_string(),
            "assets/".to_string(),
            "runtime/".to_string(),
            "meta/project.vnm".to_string(),
            "meta/assets_manifest.json".to_string(),
            "meta/package_report.json".to_string(),
        ],
        runtime_artifact,
        executable,
        warnings,
        errors,
        integrity: spec.integrity.as_str().to_string(),
        capabilities,
    })
}

fn resolve_runtime_artifact_for_plan(
    raw_path: &Path,
    project_root: &Path,
) -> Result<PathBuf, String> {
    let source = if raw_path.is_absolute() {
        raw_path
            .canonicalize()
            .map_err(|e| format!("canonicalize runtime artifact: {e}"))?
    } else {
        let safe_rel =
            sanitize_relative_path(raw_path, "runtime artifact").map_err(|err| err.to_string())?;
        canonicalize_within_root(project_root, &safe_rel, "runtime artifact")
            .map_err(|err| err.to_string())?
    };
    if !source.is_file() {
        return Err(format!(
            "runtime artifact is not a file '{}'",
            source.display()
        ));
    }
    Ok(source)
}
