use std::fs;
use std::path::{Path, PathBuf};

use hmac::{Hmac, Mac};
use sha2::Sha256;
use uuid::Uuid;

use crate::error::VnResult;
use crate::load_runtime_script_from_entry;
use crate::manifest::ProjectManifest;

type HmacSha256 = Hmac<Sha256>;

#[path = "bundle/assets.rs"]
mod assets;
#[path = "bundle/capabilities.rs"]
mod capabilities;
#[path = "bundle/helpers.rs"]
mod helpers;
#[path = "bundle/materialize.rs"]
mod materialize;
#[path = "bundle/plan.rs"]
mod plan;
#[path = "bundle/reporting.rs"]
mod reporting;
#[path = "bundle/spec.rs"]
mod spec;

use assets::copy_referenced_assets;
use capabilities::build_capability_report;
use helpers::*;
use materialize::{copy_runtime_artifact, materialize_executable, write_launcher};
use reporting::{
    build_compat_report, collect_bundle_file_manifest, export_graphics_backend_from_env,
    not_run_smoke_result,
};

pub use capabilities::ExportCapabilityReport;
pub use plan::build_export_plan;
pub use reporting::{export_executable_bundle, export_windows_executable_bundle};
pub use spec::{
    BundleAssetEntry, BundleFileEntry, BundleIntegrity, ExportBundleReport, ExportBundleSpec,
    ExportCompatReport, ExportDiagnostic, ExportPlan, ExportRuntimeSmokeCheck,
    ExportRuntimeSmokeResult, ExportTargetPlatform,
};

#[derive(Debug, Default, Clone, Copy)]
pub struct ExportService;

impl ExportService {
    pub fn new() -> Self {
        Self
    }

    pub fn plan_export(&self, spec: &ExportBundleSpec) -> VnResult<ExportPlan> {
        build_export_plan(spec)
    }

    pub fn validate_export_plan(&self, plan: &ExportPlan) -> VnResult<()> {
        let blocking_errors = plan
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.severity == "error")
            .collect::<Vec<_>>();
        if blocking_errors.is_empty() {
            Ok(())
        } else {
            let diagnostic_summary = blocking_errors
                .iter()
                .map(|diagnostic| {
                    let mut scope = Vec::new();
                    if let Some(file) = &diagnostic.file {
                        scope.push(format!("file={file}"));
                    }
                    if let Some(asset) = &diagnostic.asset {
                        scope.push(format!("asset={asset}"));
                    }
                    if let Some(node) = &diagnostic.node {
                        scope.push(format!("node={node}"));
                    }
                    if let Some(field) = &diagnostic.field {
                        scope.push(format!("field={field}"));
                    }
                    let scope = if scope.is_empty() {
                        "scope=project".to_string()
                    } else {
                        scope.join(" ")
                    };
                    format!(
                        "{}: {} trace_id={} phase={} target={} {} cause={} action={} consequence={}",
                        diagnostic.code,
                        diagnostic.message,
                        diagnostic.trace_id,
                        diagnostic.phase,
                        diagnostic.target,
                        scope,
                        diagnostic.probable_cause,
                        diagnostic.suggested_action,
                        diagnostic.consequence
                    )
                })
                .collect::<Vec<_>>()
                .join("; ");
            Err(invalid_bundle(format!(
                "export plan has blocking diagnostics: {diagnostic_summary}"
            )))
        }
    }

    pub fn execute_export(&self, spec: ExportBundleSpec) -> VnResult<ExportBundleReport> {
        let plan = self.plan_export(&spec)?;
        self.validate_export_plan(&plan)?;
        export_bundle_atomic(spec)
    }
}

pub fn export_bundle(spec: ExportBundleSpec) -> VnResult<ExportBundleReport> {
    ExportService::new().execute_export(spec)
}

fn export_bundle_atomic(spec: ExportBundleSpec) -> VnResult<ExportBundleReport> {
    let final_output_root = spec.output_root.clone();
    output_root_exists_as_directory(&final_output_root)?;
    let parent = final_output_root
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    fs::create_dir_all(&parent)
        .map_err(|e| invalid_bundle(format!("create output parent '{}': {e}", parent.display())))?;
    let output_name = final_output_root
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("bundle");
    let nonce = Uuid::new_v4();
    let staging_root = parent.join(format!(".{output_name}.staging-{nonce}"));
    let backup_root = parent.join(format!(".{output_name}.rollback-{nonce}"));

    let mut staging_spec = spec;
    staging_spec.output_root = staging_root.clone();
    match export_bundle_materialized(staging_spec, &final_output_root) {
        Ok(report) => {
            publish_staged_bundle(&staging_root, &final_output_root, &backup_root)?;
            Ok(report)
        }
        Err(err) => {
            if let Err(cleanup_err) = fs::remove_dir_all(&staging_root) {
                return Err(invalid_bundle(format!(
                    "{err}; additionally failed to remove staging '{}': {cleanup_err}",
                    staging_root.display()
                )));
            }
            Err(err)
        }
    }
}

fn publish_staged_bundle(staging: &Path, final_output: &Path, backup: &Path) -> VnResult<()> {
    if backup.exists() {
        return Err(invalid_bundle(format!(
            "rollback path already exists '{}'",
            backup.display()
        )));
    }
    let had_existing = final_output.exists();
    if had_existing {
        fs::rename(final_output, backup).map_err(|e| {
            invalid_bundle(format!(
                "prepare rollback '{}' -> '{}': {e}",
                final_output.display(),
                backup.display()
            ))
        })?;
    }

    match fs::rename(staging, final_output) {
        Ok(()) => {
            if had_existing {
                fs::remove_dir_all(backup).map_err(|e| {
                    invalid_bundle(format!("remove rollback '{}': {e}", backup.display()))
                })?;
            }
            Ok(())
        }
        Err(err) => {
            let restore_error = if had_existing {
                fs::rename(backup, final_output).err()
            } else {
                None
            };
            if let Some(restore_error) = restore_error {
                return Err(invalid_bundle(format!(
                    "publish staged bundle '{}' -> '{}': {err}; rollback restore failed: {restore_error}",
                    staging.display(),
                    final_output.display()
                )));
            }
            Err(invalid_bundle(format!(
                "publish staged bundle '{}' -> '{}': {err}",
                staging.display(),
                final_output.display()
            )))
        }
    }
}

fn export_bundle_materialized(
    spec: ExportBundleSpec,
    reported_output_root: &Path,
) -> VnResult<ExportBundleReport> {
    let plan = build_export_plan(&spec)?;
    ExportService::new().validate_export_plan(&plan)?;
    let project_root = spec
        .project_root
        .canonicalize()
        .map_err(|e| invalid_bundle(format!("canonicalize project_root: {e}")))?;

    fs::create_dir_all(&spec.output_root).map_err(|e| {
        invalid_bundle(format!(
            "create output_root '{}': {e}",
            spec.output_root.display()
        ))
    })?;
    let output_root = spec
        .output_root
        .canonicalize()
        .map_err(|e| invalid_bundle(format!("canonicalize output_root: {e}")))?;

    let manifest_path = project_root.join("project.vnm");
    ensure_regular_file(&manifest_path, "manifest")?;
    let manifest = ProjectManifest::load(&manifest_path)
        .map_err(|e| invalid_bundle(format!("load manifest '{}': {e}", manifest_path.display())))?;

    let entry_script = spec
        .entry_script
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
    let compiled = script.compile().map_err(|e| {
        invalid_bundle(format!(
            "compile entry script '{}': {e}",
            entry_script.display()
        ))
    })?;
    let compiled_bytes = compiled
        .to_binary()
        .map_err(|e| invalid_bundle(format!("serialize compiled script: {e}")))?;
    let capability_report = build_capability_report(&script, spec.runtime_artifact.is_some());

    let scripts_dir = output_root.join("scripts");
    let assets_dir = output_root.join("assets");
    let runtime_dir = output_root.join("runtime");
    let meta_dir = output_root.join("meta");
    fs::create_dir_all(&scripts_dir).map_err(|e| {
        invalid_bundle(format!(
            "create scripts dir '{}': {e}",
            scripts_dir.display()
        ))
    })?;
    fs::create_dir_all(&assets_dir).map_err(|e| {
        invalid_bundle(format!("create assets dir '{}': {e}", assets_dir.display()))
    })?;
    fs::create_dir_all(&runtime_dir).map_err(|e| {
        invalid_bundle(format!(
            "create runtime dir '{}': {e}",
            runtime_dir.display()
        ))
    })?;
    fs::create_dir_all(&meta_dir)
        .map_err(|e| invalid_bundle(format!("create meta dir '{}': {e}", meta_dir.display())))?;

    let runtime_script_rel = PathBuf::from("compiled.vnscript.json");
    let script_source_out = scripts_dir.join(&runtime_script_rel);
    if let Some(parent) = script_source_out.parent() {
        fs::create_dir_all(parent).map_err(|e| {
            invalid_bundle(format!(
                "create script source parent '{}': {e}",
                parent.display()
            ))
        })?;
    }
    let runtime_script_json = script
        .to_json()
        .map_err(|e| invalid_bundle(format!("serialize runtime script: {e}")))?;
    fs::write(&script_source_out, runtime_script_json.as_bytes()).map_err(|e| {
        invalid_bundle(format!(
            "write runtime script '{}' -> '{}': {e}",
            script_source_path.display(),
            script_source_out.display()
        ))
    })?;

    let script_binary_rel = PathBuf::from("compiled.vnc");
    let script_binary_out = scripts_dir.join(&script_binary_rel);
    if let Some(parent) = script_binary_out.parent() {
        fs::create_dir_all(parent).map_err(|e| {
            invalid_bundle(format!(
                "create script binary parent '{}': {e}",
                parent.display()
            ))
        })?;
    }
    fs::write(&script_binary_out, &compiled_bytes).map_err(|e| {
        invalid_bundle(format!(
            "write compiled script '{}': {e}",
            script_binary_out.display()
        ))
    })?;

    let manifest_out = meta_dir.join("project.vnm");
    manifest.save(&manifest_out).map_err(|e| {
        invalid_bundle(format!(
            "write bundle manifest '{}': {e}",
            manifest_out.display()
        ))
    })?;

    let assets_manifest_entries = copy_referenced_assets(&project_root, &assets_dir, &script)?;
    let assets_manifest_payload = serde_json::json!({
        "manifest_version": 1,
        "assets": &assets_manifest_entries,
    });
    let assets_manifest_json = serde_json::to_string_pretty(&assets_manifest_payload)
        .map_err(|e| invalid_bundle(format!("serialize assets manifest: {e}")))?;
    let assets_manifest_out = meta_dir.join("assets_manifest.json");
    fs::write(&assets_manifest_out, assets_manifest_json.as_bytes()).map_err(|e| {
        invalid_bundle(format!(
            "write assets manifest '{}': {e}",
            assets_manifest_out.display()
        ))
    })?;

    let runtime_artifact = copy_runtime_artifact(
        spec.runtime_artifact.as_deref(),
        &project_root,
        &runtime_dir,
    )?;
    let runtime_artifact_rel = runtime_artifact
        .as_ref()
        .map(|runtime| runtime.rel_path.clone());
    let runtime_artifact_sha256 = runtime_artifact
        .as_ref()
        .map(|runtime| runtime.sha256.clone());
    let executable_rel = materialize_executable(
        spec.target_platform,
        &output_root,
        runtime_artifact.as_ref(),
    )?;
    let launch_target = executable_rel.as_deref().or_else(|| {
        runtime_artifact
            .as_ref()
            .map(|runtime| runtime.rel_path.as_str())
    });
    let launcher_rel = write_launcher(spec.target_platform, &output_root, launch_target)?;

    let integrity_scope = "bundle_file_manifest_v2_signed_manifest_covers_payload_files";

    let (graphics_backend, wgpu_fallback, backend_diagnostic) = export_graphics_backend_from_env();
    let mut diagnostics = plan.diagnostics.clone();
    if let Some(diagnostic) = backend_diagnostic {
        diagnostics.push(diagnostic);
    }

    let mut report = ExportBundleReport {
        schema: "vnengine.export_bundle_report.v1".to_string(),
        target_platform: spec.target_platform.as_str().to_string(),
        output_layout_version: spec.output_layout_version,
        project_root: normalize_path_display(&project_root),
        output_root: normalize_path_display(reported_output_root),
        generator_os: std::env::consts::OS.to_string(),
        script_source: normalize_path_display(
            Path::new("scripts").join(&runtime_script_rel).as_path(),
        ),
        script_binary: normalize_path_display(
            Path::new("scripts").join(&script_binary_rel).as_path(),
        ),
        assets_manifest: normalize_path_display(Path::new("meta/assets_manifest.json")),
        assets_copied: assets_manifest_entries.len(),
        runtime_artifact: runtime_artifact_rel,
        runtime_artifact_sha256,
        executable: executable_rel,
        expected_executable: spec.target_platform.expected_executable_name().to_string(),
        launcher: launcher_rel,
        graphics_backend,
        wgpu_fallback,
        total_size: 0,
        hashes: Vec::new(),
        integrity: spec.integrity.as_str().to_string(),
        bundle_hmac_sha256: None,
        bundle_file_manifest: Some(normalize_path_display(Path::new(
            "meta/bundle_file_manifest.json",
        ))),
        bundle_file_manifest_sha256: None,
        compat_report: Some(normalize_path_display(Path::new("meta/compat_report.json"))),
        integrity_scope: integrity_scope.to_string(),
        capabilities: capability_report,
        diagnostics,
        smoke_result: not_run_smoke_result(spec.target_platform),
    };

    let report_path = meta_dir.join("package_report.json");
    let report_json = serde_json::to_string_pretty(&report)
        .map_err(|e| invalid_bundle(format!("serialize package report: {e}")))?;
    fs::write(&report_path, report_json).map_err(|e| {
        invalid_bundle(format!(
            "write package report '{}': {e}",
            report_path.display()
        ))
    })?;

    let file_manifest_entries = collect_bundle_file_manifest(&output_root)?;
    let file_manifest_payload = serde_json::json!({
        "manifest_version": 2,
        "integrity_scope": integrity_scope,
        "files": &file_manifest_entries,
    });
    let file_manifest_json = serde_json::to_string_pretty(&file_manifest_payload)
        .map_err(|e| invalid_bundle(format!("serialize bundle file manifest: {e}")))?;
    let file_manifest_sha256 = sha256_hex(file_manifest_json.as_bytes());
    report.bundle_file_manifest_sha256 = Some(file_manifest_sha256.clone());
    report.total_size = file_manifest_entries.iter().map(|entry| entry.size).sum();
    report.hashes = file_manifest_entries.clone();
    let file_manifest_out = meta_dir.join("bundle_file_manifest.json");
    fs::write(&file_manifest_out, file_manifest_json.as_bytes()).map_err(|e| {
        invalid_bundle(format!(
            "write bundle file manifest '{}': {e}",
            file_manifest_out.display()
        ))
    })?;

    let bundle_hmac_sha256 = match spec.integrity {
        BundleIntegrity::None => None,
        BundleIntegrity::HmacSha256 => {
            let key = spec
                .hmac_key
                .as_deref()
                .filter(|v| !v.trim().is_empty())
                .ok_or_else(|| invalid_bundle("integrity=hmac_sha256 requires hmac_key"))?;
            let mut mac = HmacSha256::new_from_slice(key.as_bytes())
                .map_err(|e| invalid_bundle(format!("init hmac: {e}")))?;
            mac.update(file_manifest_json.as_bytes());
            Some(to_hex(mac.finalize().into_bytes().as_slice()))
        }
    };

    if let Some(signature) = &bundle_hmac_sha256 {
        let signature_path = meta_dir.join("bundle.hmac_sha256");
        fs::write(&signature_path, signature).map_err(|e| {
            invalid_bundle(format!(
                "write bundle signature '{}': {e}",
                signature_path.display()
            ))
        })?;
    }
    report.bundle_hmac_sha256 = bundle_hmac_sha256;
    let report_json = serde_json::to_string_pretty(&report)
        .map_err(|e| invalid_bundle(format!("serialize package report: {e}")))?;
    fs::write(&report_path, report_json).map_err(|e| {
        invalid_bundle(format!(
            "write package report '{}': {e}",
            report_path.display()
        ))
    })?;

    let compat_report = build_compat_report(
        spec.target_platform,
        &report,
        &file_manifest_entries,
        &file_manifest_sha256,
    );
    let compat_report_path = meta_dir.join("compat_report.json");
    let compat_report_json = serde_json::to_string_pretty(&compat_report)
        .map_err(|e| invalid_bundle(format!("serialize compat report: {e}")))?;
    fs::write(&compat_report_path, compat_report_json).map_err(|e| {
        invalid_bundle(format!(
            "write compat report '{}': {e}",
            compat_report_path.display()
        ))
    })?;

    Ok(report)
}
