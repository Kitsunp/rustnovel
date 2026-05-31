use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::error::VnResult;
use crate::load_runtime_script_from_entry;
use crate::manifest::ProjectManifest;

use super::assets::{detect_asset_path_collisions, AssetPathCollision};
use super::capabilities::{build_capability_report, ExportCapabilityReport};
use super::helpers::{
    canonicalize_within_root, ensure_regular_file, invalid_bundle, normalize_path_display,
    planned_output_root, sanitize_relative_path, to_hex,
};
use super::materialize::{has_extension, runtime_artifact_matches_target};
use super::{
    BundleIntegrity, ExportBundleSpec, ExportDiagnostic, ExportPlan, ExportTargetPlatform,
};

pub fn build_export_plan(spec: &ExportBundleSpec) -> VnResult<ExportPlan> {
    let project_root = spec
        .project_root
        .canonicalize()
        .map_err(|e| invalid_bundle(format!("canonicalize project_root: {e}")))?;
    let output_root = planned_output_root(&spec.output_root)?;

    let manifest_path = project_root.join("project.vnm");
    ensure_regular_file(&manifest_path, "manifest")?;
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

    let mut diagnostics = capability_diagnostics(&capabilities, spec.target_platform);
    for collision in detect_asset_path_collisions(&project_root, &script)? {
        let message = collision.message();
        let (code, probable_cause, suggested_action, consequence, asset) = match &collision {
            AssetPathCollision::Case { asset, existing } => (
                "export.asset.case_collision",
                format!(
                    "The script references '{asset}' and '{existing}', which differ only by letter case."
                ),
                "Rename one asset or update the script so every packaged asset path is unique ignoring case.",
                "The export is blocked because Windows and Linux would disagree about which asset is packaged.",
                asset.as_str(),
            ),
            AssetPathCollision::Destination {
                destination,
                first_source,
                second_source,
            } => (
                "export.asset.destination_collision",
                format!(
                    "Both '{first_source}' and '{second_source}' resolve to packaged asset '{destination}'."
                ),
                "Rename one source asset or update references so each packaged asset destination has exactly one source.",
                "The export is blocked because one referenced asset would overwrite another in the bundle.",
                destination.as_str(),
            ),
        };
        diagnostics.push(export_diagnostic(ExportDiagnosticInput {
            code,
            severity: "error",
            phase: "plan",
            target: spec.target_platform,
            message: &message,
            probable_cause: &probable_cause,
            suggested_action,
            consequence,
            blocking_release: true,
            file: Some(asset),
            asset: Some(asset),
            node: None,
            field: Some("asset_ref"),
        }));
    }
    let runtime_artifact_path = match spec.runtime_artifact.as_deref() {
        Some(path) => match resolve_runtime_artifact_for_plan(path, &project_root) {
            Ok(path) => Some(path),
            Err(err) => {
                let file = normalize_path_display(path);
                diagnostics.push(export_diagnostic(ExportDiagnosticInput {
                    code: "export.runtime_artifact.unreadable",
                    severity: "error",
                    phase: "plan",
                    target: spec.target_platform,
                    message: "Runtime artifact could not be resolved for export.",
                    probable_cause: &err,
                    suggested_action: "Choose an existing runtime artifact inside the project or provide an absolute path.",
                    consequence: "Executable package export is blocked before writing the final bundle.",
                    blocking_release: true,
                    file: Some(&file),
                    asset: None,
                    node: None,
                    field: Some("runtime_artifact"),
                }));
                None
            }
        },
        None => {
            let message = format!(
                "{} package is missing runtime_artifact, so '{}' cannot be materialized.",
                spec.target_platform.as_str(),
                spec.target_platform.expected_executable_name()
            );
            let consequence = format!(
                "Release packaging for target '{}' is blocked until a matching runtime artifact is selected.",
                spec.target_platform.as_str()
            );
            diagnostics.push(export_diagnostic(ExportDiagnosticInput {
                code: "export.runtime_artifact.missing",
                severity: "warning",
                phase: "plan",
                target: spec.target_platform,
                message: &message,
                probable_cause: "The package command or wizard did not receive a runtime_artifact path.",
                suggested_action: "Build or select the platform runtime and pass it with --runtime-artifact before release packaging.",
                consequence: &consequence,
                blocking_release: true,
                file: None,
                asset: None,
                node: None,
                field: Some("runtime_artifact"),
            }));
            None
        }
    };
    let mut executable = None;
    if let Some(path) = runtime_artifact_path.as_ref() {
        match runtime_artifact_matches_target(path, spec.target_platform) {
            Ok(true) => {
                if spec.target_platform != ExportTargetPlatform::Windows
                    || has_extension(path, "exe")
                {
                    executable = Some(spec.target_platform.expected_executable_name().to_string());
                }
            }
            Ok(false) => {
                let file = normalize_path_display(path);
                let message = format!(
                    "Runtime artifact '{}' is not a {} binary, so '{}' cannot be published.",
                    file,
                    spec.target_platform.as_str(),
                    spec.target_platform.expected_executable_name()
                );
                let probable_cause = format!(
                    "The selected runtime artifact appears to be built for a different platform than target '{}'.",
                    spec.target_platform.as_str()
                );
                diagnostics.push(export_diagnostic(ExportDiagnosticInput {
                    code: "export.runtime_artifact.target_mismatch",
                    severity: "warning",
                    phase: "plan",
                    target: spec.target_platform,
                    message: &message,
                    probable_cause: &probable_cause,
                    suggested_action: "Build the runtime for the requested target and rerun package planning.",
                    consequence: "Executable package export is blocked for release because the launcher would not start on the target.",
                    blocking_release: true,
                    file: Some(&file),
                    asset: None,
                    node: None,
                    field: Some("runtime_artifact"),
                }));
            }
            Err(err) => {
                let message = err.to_string();
                let file = normalize_path_display(path);
                let diagnostic_message = format!(
                    "Runtime artifact '{}' could not be inspected as a {} executable.",
                    file,
                    spec.target_platform.as_str()
                );
                diagnostics.push(export_diagnostic(ExportDiagnosticInput {
                    code: "export.runtime_artifact.invalid",
                    severity: "error",
                    phase: "plan",
                    target: spec.target_platform,
                    message: &diagnostic_message,
                    probable_cause: &message,
                    suggested_action: "Replace the runtime artifact with a valid PE, ELF, or Mach-O executable for the requested target.",
                    consequence: "Executable package export is blocked before writing the final bundle.",
                    blocking_release: true,
                    file: Some(&file),
                    asset: None,
                    node: None,
                    field: Some("runtime_artifact"),
                }));
            }
        }
    }
    let runtime_artifact = runtime_artifact_path
        .as_ref()
        .map(|path| normalize_path_display(path));
    if spec.integrity == BundleIntegrity::HmacSha256
        && spec
            .hmac_key
            .as_deref()
            .is_none_or(|value| value.trim().is_empty())
    {
        diagnostics.push(export_diagnostic(ExportDiagnosticInput {
            code: "export.integrity.hmac_key_missing",
            severity: "error",
            phase: "plan",
            target: spec.target_platform,
            message: "HMAC integrity requires hmac_key, but no key was provided.",
            probable_cause: "The CLI, API, or export wizard set integrity=hmac_sha256 but did not provide hmac_key.",
            suggested_action: "Provide an HMAC key for release signing or switch integrity to none for unsigned development exports.",
            consequence: "The export is blocked because the manifest signature could not be generated.",
            blocking_release: true,
            file: None,
            asset: None,
            node: None,
            field: Some("hmac_key"),
        }));
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
            "meta/compat_report.json".to_string(),
        ],
        runtime_artifact,
        executable,
        warnings: Vec::new(),
        errors: Vec::new(),
        diagnostics,
        integrity: spec.integrity.as_str().to_string(),
        capabilities,
    })
}

struct ExportDiagnosticInput<'a> {
    code: &'a str,
    severity: &'a str,
    phase: &'a str,
    target: ExportTargetPlatform,
    message: &'a str,
    probable_cause: &'a str,
    suggested_action: &'a str,
    consequence: &'a str,
    blocking_release: bool,
    file: Option<&'a str>,
    asset: Option<&'a str>,
    node: Option<&'a str>,
    field: Option<&'a str>,
}

fn capability_diagnostics(
    capabilities: &ExportCapabilityReport,
    target: ExportTargetPlatform,
) -> Vec<ExportDiagnostic> {
    let mut diagnostics = Vec::new();
    if !capabilities.ext_call_commands.is_empty() {
        diagnostics.push(export_diagnostic(ExportDiagnosticInput {
                code: "export.capability.ext_call_runtime_required",
                severity: "warning",
                phase: "plan",
                target,
                message: "Script contains external calls that require runtime handlers.",
                probable_cause: "At least one event uses an external command not handled by the static bundle itself.",
                suggested_action: "Confirm the target runtime registers the required external call handlers before release.",
                consequence: "The game may stop at runtime if handlers are missing.",
                blocking_release: false,
                file: None,
                asset: None,
                node: None,
                field: Some("events.ext_call"),
            }));
    }
    if !capabilities.audio_actions.is_empty() {
        diagnostics.push(export_diagnostic(ExportDiagnosticInput {
                code: "export.capability.audio_backend_required",
                severity: "warning",
                phase: "plan",
                target,
                message: "Script uses audio actions that require runtime audio backend support.",
                probable_cause: "At least one event starts, stops, or changes audio playback.",
                suggested_action: "Package with a runtime that includes audio support and smoke it on the target platform.",
                consequence: "Audio cues may be silent or fail at runtime.",
                blocking_release: false,
                file: None,
                asset: None,
                node: None,
                field: Some("events.audio"),
            }));
    }
    if !capabilities.transitions.is_empty() {
        diagnostics.push(export_diagnostic(ExportDiagnosticInput {
            code: "export.capability.transition_support_required",
            severity: "warning",
            phase: "plan",
            target,
            message: "Script uses visual transitions that require runtime renderer support.",
            probable_cause: "At least one scene event includes a transition.",
            suggested_action:
                "Smoke the package with the selected renderer/backend on the target platform.",
            consequence: "Transitions may be skipped or render incorrectly.",
            blocking_release: false,
            file: None,
            asset: None,
            node: None,
            field: Some("events.transition"),
        }));
    }
    diagnostics
}

fn export_diagnostic(input: ExportDiagnosticInput<'_>) -> ExportDiagnostic {
    let file = input.file.map(ToOwned::to_owned);
    let asset = input.asset.map(ToOwned::to_owned);
    let node = input.node.map(ToOwned::to_owned);
    let field = input.field.map(ToOwned::to_owned);
    let trace_id = export_trace_id(
        input.code,
        input.phase,
        input.target,
        file.as_deref()
            .or(field.as_deref())
            .unwrap_or(input.message),
    );
    let mut details = vec![
        format!("target={}", input.target.as_str()),
        format!("phase={}", input.phase),
    ];
    if let Some(file) = &file {
        details.push(format!("file={file}"));
    }
    if let Some(asset) = &asset {
        details.push(format!("asset={asset}"));
    }
    if let Some(node) = &node {
        details.push(format!("node={node}"));
    }
    if let Some(field) = &field {
        details.push(format!("field={field}"));
    }
    ExportDiagnostic {
        code: input.code.to_string(),
        severity: input.severity.to_string(),
        phase: input.phase.to_string(),
        target: input.target.as_str().to_string(),
        trace_id,
        message: format!("{} ({})", input.message, details.join(", ")),
        probable_cause: input.probable_cause.to_string(),
        suggested_action: input.suggested_action.to_string(),
        consequence: input.consequence.to_string(),
        blocking_release: input.blocking_release,
        file,
        asset,
        node,
        field,
    }
}

fn export_trace_id(code: &str, phase: &str, target: ExportTargetPlatform, subject: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(code.as_bytes());
    hasher.update(b"\0");
    hasher.update(phase.as_bytes());
    hasher.update(b"\0");
    hasher.update(target.as_str().as_bytes());
    hasher.update(b"\0");
    hasher.update(subject.as_bytes());
    let digest = to_hex(hasher.finalize().as_slice());
    format!("export-{}", &digest[..16])
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
