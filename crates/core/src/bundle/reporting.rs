use std::fs;
use std::io::Read;
use std::path::Path;

use sha2::{Digest, Sha256};
use walkdir::WalkDir;

use crate::error::VnResult;

use super::helpers::{invalid_bundle, normalize_path_display, sha256_hex, to_hex};
use super::{
    build_export_plan, export_bundle, BundleFileEntry, ExportBundleReport, ExportDiagnostic,
    ExportPlan, ExportRuntimeSmokeCheck, ExportRuntimeSmokeResult, ExportService,
    ExportTargetPlatform,
};

pub(super) fn build_compat_report(
    target_platform: ExportTargetPlatform,
    report: &ExportBundleReport,
    _file_manifest_entries: &[BundleFileEntry],
    file_manifest_sha256: &str,
) -> super::ExportCompatReport {
    super::ExportCompatReport {
        schema: "vnengine.export_compat_report.v1".to_string(),
        target_platform: target_platform.as_str().to_string(),
        generator_os: std::env::consts::OS.to_string(),
        runtime_artifact: report.runtime_artifact.clone(),
        runtime_artifact_sha256: report.runtime_artifact_sha256.clone(),
        expected_executable: target_platform.expected_executable_name().to_string(),
        executable: report.executable.clone(),
        graphics_backend: report.graphics_backend.clone(),
        wgpu_fallback: report.wgpu_fallback,
        assets_copied: report.assets_copied,
        total_size: report.total_size,
        diagnostics: report.diagnostics.clone(),
        hashes: report.hashes.clone(),
        bundle_file_manifest_sha256: report
            .bundle_file_manifest_sha256
            .clone()
            .unwrap_or_else(|| file_manifest_sha256.to_string()),
        bundle_hmac_sha256: report.bundle_hmac_sha256.clone(),
        smoke_result: report.smoke_result.clone(),
    }
}

pub(super) fn export_graphics_backend_from_env() -> (String, bool, Option<ExportDiagnostic>) {
    let value = std::env::var("VNENGINE_RENDER_BACKEND").ok();
    export_graphics_backend_from_value(value.as_deref())
}

fn export_graphics_backend_from_value(
    value: Option<&str>,
) -> (String, bool, Option<ExportDiagnostic>) {
    match value
        .map(|value| value.trim().to_ascii_lowercase())
        .as_deref()
    {
        Some("wgpu") | Some("hardware") => ("wgpu".to_string(), false, None),
        Some("auto") | Some("") => ("auto".to_string(), true, None),
        Some("software") | Some("pixels") | None => ("software".to_string(), true, None),
        Some(other) => {
            let diagnostic = unsupported_render_backend_diagnostic(other);
            ("software".to_string(), true, Some(diagnostic))
        }
    }
}

fn unsupported_render_backend_diagnostic(value: &str) -> ExportDiagnostic {
    let code = "export.render_backend.unsupported";
    ExportDiagnostic {
        code: code.to_string(),
        severity: "warning".to_string(),
        phase: "package".to_string(),
        target: "export".to_string(),
        trace_id: format!(
            "export-render-backend-{}",
            &sha256_hex(format!("{code}:{value}").as_bytes())[..16]
        ),
        message: format!(
            "unsupported VNENGINE_RENDER_BACKEND value '{value}' during export; using software metadata"
        ),
        probable_cause: "The render backend environment variable contains an unsupported value."
            .to_string(),
        suggested_action: "Use auto, software, pixels, wgpu, or hardware.".to_string(),
        consequence: "Export compatibility metadata may not match the intended renderer.".to_string(),
        blocking_release: false,
        file: None,
        asset: None,
        node: None,
        field: Some("VNENGINE_RENDER_BACKEND".to_string()),
    }
}

pub(super) fn not_run_smoke_result(
    target_platform: ExportTargetPlatform,
) -> ExportRuntimeSmokeResult {
    let target = target_platform.as_str().to_string();
    let code = "export.runtime_smoke.not_run";
    let trace_id = format!(
        "export-smoke-{}",
        &sha256_hex(format!("{code}:{target}:not_run").as_bytes())[..16]
    );
    ExportRuntimeSmokeResult {
        status: "not_run".to_string(),
        backend: "software".to_string(),
        details: "runtime smoke is executed by CI/package smoke jobs".to_string(),
        phase: "smoke".to_string(),
        target: target.clone(),
        trace_id: trace_id.clone(),
        checks: vec![ExportRuntimeSmokeCheck {
            code: code.to_string(),
            status: "not_run".to_string(),
            severity: "warning".to_string(),
            phase: "smoke".to_string(),
            target,
            trace_id,
            message: "Package has not been loaded by the runtime smoke yet.".to_string(),
            probable_cause:
                "The export completed before a target runtime smoke job executed the bundle."
                    .to_string(),
            suggested_action: "Run the package smoke job on the target platform before release."
                .to_string(),
            consequence:
                "The package remains unverified as playable until smoke_result is updated to passed."
                    .to_string(),
            blocking_release: true,
            file: Some("meta/runtime_smoke_report.json".to_string()),
            asset: None,
            node: None,
            field: Some("smoke_result".to_string()),
        }],
    }
}

pub(super) fn collect_bundle_file_manifest(output_root: &Path) -> VnResult<Vec<BundleFileEntry>> {
    let mut entries = Vec::new();
    for entry in WalkDir::new(output_root).sort_by_file_name() {
        let entry = entry.map_err(|e| invalid_bundle(format!("walk bundle output: {e}")))?;
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        let rel_path = path.strip_prefix(output_root).map_err(|e| {
            invalid_bundle(format!(
                "strip bundle root '{}' from '{}': {e}",
                output_root.display(),
                path.display()
            ))
        })?;
        let rel = normalize_path_display(rel_path);
        if is_integrity_metadata_path(&rel) {
            continue;
        }
        let (sha256, size) = sha256_file(path)?;
        entries.push(BundleFileEntry {
            role: classify_bundle_file_role(&rel).to_string(),
            path: rel,
            sha256,
            size,
        });
    }
    Ok(entries)
}

fn is_integrity_metadata_path(path: &str) -> bool {
    matches!(
        path,
        "meta/package_report.json"
            | "meta/compat_report.json"
            | "meta/bundle_file_manifest.json"
            | "meta/bundle.hmac_sha256"
    )
}

fn sha256_file(path: &Path) -> VnResult<(String, u64)> {
    let mut file = fs::File::open(path)
        .map_err(|e| invalid_bundle(format!("open file for sha256 '{}': {e}", path.display())))?;
    let mut hasher = Sha256::new();
    let mut size = 0u64;
    let mut buf = [0u8; 64 * 1024];
    loop {
        let read = file.read(&mut buf).map_err(|e| {
            invalid_bundle(format!("read file for sha256 '{}': {e}", path.display()))
        })?;
        if read == 0 {
            break;
        }
        size += read as u64;
        hasher.update(&buf[..read]);
    }
    Ok((to_hex(hasher.finalize().as_slice()), size))
}

fn classify_bundle_file_role(path: &str) -> &'static str {
    if path.starts_with("scripts/") {
        "script"
    } else if path.starts_with("assets/") {
        "asset"
    } else if path.starts_with("runtime/") {
        "runtime"
    } else if path == "game.exe" || path == "game" {
        "executable"
    } else if path.starts_with("launch") {
        "launcher"
    } else if path.starts_with("meta/") {
        "metadata"
    } else {
        "other"
    }
}

pub fn export_executable_bundle(spec: super::ExportBundleSpec) -> VnResult<ExportBundleReport> {
    let target = spec.target_platform;
    let plan = build_export_plan(&spec)?;
    ExportService::new().validate_export_plan(&plan)?;
    let expected = target.expected_executable_name();
    if plan.executable.as_deref() != Some(expected) {
        return Err(invalid_bundle(executable_requirement_message(
            target,
            expected,
            Some(&plan),
        )));
    }
    let report = export_bundle(spec)?;
    if report.executable.as_deref() != Some(expected) {
        return Err(invalid_bundle(executable_requirement_message(
            target, expected, None,
        )));
    }
    Ok(report)
}

fn executable_requirement_message(
    target: ExportTargetPlatform,
    expected: &str,
    plan: Option<&ExportPlan>,
) -> String {
    let hint = match target {
        ExportTargetPlatform::Windows => ".exe runtime_artifact",
        ExportTargetPlatform::Linux => "linux runtime_artifact",
        ExportTargetPlatform::Macos => "macos runtime_artifact",
    };
    let mut message = format!(
        "{} executable export cannot produce '{}': required input is a matching {hint}",
        target.as_str(),
        expected
    );
    if let Some(plan) = plan {
        let diagnostics = plan
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.blocking_release)
            .map(|diagnostic| {
                format!(
                    "{} trace_id={} phase={} target={} message={} action={}",
                    diagnostic.code,
                    diagnostic.trace_id,
                    diagnostic.phase,
                    diagnostic.target,
                    diagnostic.message,
                    diagnostic.suggested_action
                )
            })
            .collect::<Vec<_>>();
        if !diagnostics.is_empty() {
            message.push_str("; blocking diagnostics: ");
            message.push_str(&diagnostics.join("; "));
        }
    }
    message
}

pub fn export_windows_executable_bundle(
    spec: super::ExportBundleSpec,
) -> VnResult<ExportBundleReport> {
    if spec.target_platform != ExportTargetPlatform::Windows {
        return Err(invalid_bundle(format!(
            "windows executable export requires target_platform=windows, got {}",
            spec.target_platform.as_str()
        )));
    }
    export_executable_bundle(spec)
}
