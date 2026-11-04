use std::fs;
use std::path::{Path, PathBuf};

use hmac::{Hmac, Mac};
use sha2::Sha256;
use uuid::Uuid;

use crate::error::VnResult;

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

/// A captured export plan and its script, consumed exactly once when executed.
pub struct PreparedExport {
    spec: ExportBundleSpec,
    snapshot: plan::PlanSnapshot,
    runtime_sha256: Option<String>,
}

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
        self.prepare_export(spec)?.execute()
    }

    pub fn prepare_export(&self, spec: ExportBundleSpec) -> VnResult<PreparedExport> {
        let snapshot = plan::prepare_plan(&spec)?;
        let runtime_sha256 = if snapshot
            .plan
            .diagnostics
            .iter()
            .any(|issue| issue.severity == "error")
        {
            None
        } else {
            snapshot
                .runtime_artifact_path
                .as_ref()
                .map(|path| {
                    crate::atomic_io::sha256_file(path)
                        .map(|(hash, _)| hash)
                        .map_err(|error| {
                            invalid_bundle(format!(
                                "hash runtime artifact '{}': {error}",
                                path.display()
                            ))
                        })
                })
                .transpose()?
        };
        Ok(PreparedExport {
            spec,
            snapshot,
            runtime_sha256,
        })
    }

    /// Shared release requirements for GUI, CLI and API executable exports.
    pub fn validate_executable_export_plan(
        &self,
        plan: &ExportPlan,
        target: ExportTargetPlatform,
    ) -> VnResult<()> {
        self.validate_export_plan(plan)?;
        if plan.executable.as_deref() != Some(target.expected_executable_name())
            || plan
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.blocking_release)
        {
            return Err(invalid_bundle(format!(
                "Executable export blocked by diagnostics: {}",
                reporting::executable_requirement_message(
                    target,
                    target.expected_executable_name(),
                    Some(plan)
                )
            )));
        }
        Ok(())
    }
}

impl PreparedExport {
    pub fn plan(&self) -> &ExportPlan {
        &self.snapshot.plan
    }

    pub fn execute(mut self) -> VnResult<ExportBundleReport> {
        ExportService::new().validate_export_plan(&self.snapshot.plan)?;
        self.compile_if_needed()?;
        export_bundle_atomic(self, false)
    }

    pub fn execute_executable(mut self) -> VnResult<ExportBundleReport> {
        ExportService::new()
            .validate_executable_export_plan(&self.snapshot.plan, self.spec.target_platform)?;
        self.compile_if_needed()?;
        export_bundle_atomic(self, true)
    }

    fn compile_if_needed(&mut self) -> VnResult<()> {
        if self.snapshot.compiled.is_none() {
            self.snapshot.compiled = Some(self.snapshot.script.compile().map_err(|error| {
                invalid_bundle(format!(
                    "compile entry script '{}': {error}",
                    self.snapshot.plan.entry_script
                ))
            })?);
        }
        Ok(())
    }
}

pub fn export_bundle(spec: ExportBundleSpec) -> VnResult<ExportBundleReport> {
    ExportService::new().execute_export(spec)
}

fn export_bundle_atomic(
    mut prepared: PreparedExport,
    require_executable: bool,
) -> VnResult<ExportBundleReport> {
    let requested_output_root = prepared.spec.output_root.clone();
    let final_output_root = prepared.snapshot.output_root.clone();
    validate_prepared_output(&prepared.snapshot, &requested_output_root)?;
    if planned_output_root(&requested_output_root)? != final_output_root {
        return Err(invalid_bundle(
            "output_root changed after export preparation",
        ));
    }
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

    validate_prepared_output(&prepared.snapshot, &staging_root)?;
    fs::create_dir(&staging_root).map_err(|error| {
        invalid_bundle(format!(
            "create staging directory '{}': {error}",
            staging_root.display()
        ))
    })?;
    prepared.spec.output_root = staging_root.clone();
    let (result, cleanup_staging) =
        match export_bundle_materialized(&prepared, &requested_output_root, require_executable) {
            Ok(report) => {
                let checked = validate_prepared_output(&prepared.snapshot, &requested_output_root)
                    .and_then(|()| validate_prepared_output(&prepared.snapshot, &staging_root))
                    .and_then(|()| {
                        if planned_output_root(&requested_output_root)? != final_output_root {
                            return Err(invalid_bundle(
                                "output_root changed after export preparation",
                            ));
                        }
                        Ok(())
                    });
                match checked {
                    Err(error) => (Err(error), true),
                    Ok(()) => {
                        match publish_staged_bundle(&staging_root, &final_output_root, &backup_root)
                        {
                            Ok(()) => (Ok(report), false),
                            Err(failure) => (Err(failure.error), failure.cleanup_staging),
                        }
                    }
                }
            }
            Err(error) => (Err(error), true),
        };
    if let (Err(error), true) = (&result, cleanup_staging) {
        if let Err(cleanup_error) = fs::remove_dir_all(&staging_root) {
            if cleanup_error.kind() != std::io::ErrorKind::NotFound {
                return Err(invalid_bundle(format!(
                    "{error}; additionally failed to remove staging '{}': {cleanup_error}",
                    staging_root.display()
                )));
            }
        }
    }
    result
}

fn validate_prepared_output(snapshot: &plan::PlanSnapshot, output: &Path) -> VnResult<()> {
    let output = planned_output_root(output)?;
    if snapshot.project_root.starts_with(&output) {
        return Err(invalid_bundle(
            "output_root would replace the source project; choose a separate export folder",
        ));
    }
    if snapshot.script_source_path.starts_with(&output) {
        return Err(invalid_bundle(
            "output_root contains the entry script; choose a separate export folder",
        ));
    }
    if snapshot
        .runtime_artifact_path
        .as_ref()
        .is_some_and(|path| path.starts_with(&output))
    {
        return Err(invalid_bundle(
            "output_root contains the source runtime; choose a separate export folder",
        ));
    }
    if let Some(collision) = assets::detect_asset_path_collisions(
        &snapshot.project_root,
        &snapshot.script,
        Some(&output),
    )?
    .pop()
    {
        return Err(invalid_bundle(collision.message()));
    }
    Ok(())
}

#[derive(Debug)]
struct PublicationFailure {
    error: crate::error::VnError,
    cleanup_staging: bool,
}

fn publish_staged_bundle(
    staging: &Path,
    final_output: &Path,
    backup: &Path,
) -> Result<(), PublicationFailure> {
    publish_staged_bundle_with(
        staging,
        final_output,
        backup,
        |from, to| fs::rename(from, to),
        |path| fs::remove_dir_all(path),
    )
}

fn publish_staged_bundle_with(
    staging: &Path,
    final_output: &Path,
    backup: &Path,
    rename: impl Fn(&Path, &Path) -> std::io::Result<()>,
    remove_backup: impl Fn(&Path) -> std::io::Result<()>,
) -> Result<(), PublicationFailure> {
    let before_publish = |message: String| PublicationFailure {
        error: invalid_bundle(message),
        cleanup_staging: true,
    };
    match fs::symlink_metadata(backup) {
        Ok(_) => {
            return Err(before_publish(format!(
                "rollback path already exists '{}'",
                backup.display()
            )))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(before_publish(format!(
                "inspect rollback '{}' for destination '{}': {error}",
                backup.display(),
                final_output.display()
            )))
        }
    }
    let had_existing =
        output_root_exists_as_directory(final_output).map_err(|error| PublicationFailure {
            error,
            cleanup_staging: true,
        })?;
    if had_existing {
        rename(final_output, backup).map_err(|e| {
            before_publish(format!(
                "prepare rollback '{}' -> '{}': {e}; destination and rollback state are uncertain",
                final_output.display(),
                backup.display()
            ))
        })?;
    }

    match rename(staging, final_output) {
        Ok(()) => {
            if had_existing {
                remove_backup(backup).map_err(|e| PublicationFailure {
                    error: invalid_bundle(format!("bundle published at '{}'; failed to remove rollback '{}': {e}; rollback may be incomplete", final_output.display(), backup.display())),
                    cleanup_staging: false,
                })?;
            }
            Ok(())
        }
        Err(err) => {
            let restore_error = if had_existing {
                rename(backup, final_output).err()
            } else {
                None
            };
            if let Some(restore_error) = restore_error {
                return Err(PublicationFailure { error: invalid_bundle(format!(
                    "publish staged bundle '{}' -> '{}': {err}; restore rollback '{}' -> '{}' failed: {restore_error}; destination state is uncertain; staging retained if present",
                    staging.display(),
                    final_output.display(),
                    backup.display(),
                    final_output.display()
                )), cleanup_staging: false });
            }
            let restored = if had_existing {
                "; previous destination restored"
            } else {
                "; destination state is uncertain"
            };
            Err(PublicationFailure { error: invalid_bundle(format!(
                "publish staged bundle '{}' -> '{}': {err}{restored}; rollback path '{}'; staging retained if present",
                staging.display(),
                final_output.display(),
                backup.display()
            )), cleanup_staging: false })
        }
    }
}

fn export_bundle_materialized(
    prepared: &PreparedExport,
    reported_output_root: &Path,
    require_executable: bool,
) -> VnResult<ExportBundleReport> {
    let spec = &prepared.spec;
    let snapshot = &prepared.snapshot;
    let runtime_sha256 = &prepared.runtime_sha256;
    let plan = &snapshot.plan;
    let project_root = &snapshot.project_root;

    let output_root = spec
        .output_root
        .canonicalize()
        .map_err(|e| invalid_bundle(format!("canonicalize output_root: {e}")))?;

    let manifest = &snapshot.manifest;
    let script_source_path = &snapshot.script_source_path;
    let script = &snapshot.script;
    let compiled = snapshot
        .compiled
        .as_ref()
        .expect("prepared export contains compiled script");
    let compiled_bytes = compiled
        .to_binary()
        .map_err(|e| invalid_bundle(format!("serialize compiled script: {e}")))?;
    let capability_report = build_capability_report(script, spec.runtime_artifact.is_some());

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
    let runtime_script_json = &snapshot.script_json;
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

    let assets_manifest_entries = copy_referenced_assets(project_root, &assets_dir, script)?;
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
        snapshot.runtime_artifact_path.as_deref(),
        project_root,
        &runtime_dir,
    )?;
    if runtime_artifact.as_ref().map(|runtime| &runtime.sha256) != runtime_sha256.as_ref() {
        return Err(invalid_bundle(
            "runtime artifact changed after export preparation",
        ));
    }
    let runtime_artifact_sha256 = runtime_artifact
        .as_ref()
        .map(|runtime| runtime.sha256.clone());
    let executable_rel = materialize_executable(
        spec.target_platform,
        &output_root,
        runtime_artifact.as_ref(),
    )?;
    if executable_rel != plan.executable
        || (require_executable
            && executable_rel.as_deref() != Some(spec.target_platform.expected_executable_name()))
    {
        return Err(invalid_bundle(reporting::executable_requirement_message(
            spec.target_platform,
            spec.target_platform.expected_executable_name(),
            Some(plan),
        )));
    }
    let runtime_artifact_rel = match (&executable_rel, &runtime_artifact) {
        (Some(executable), Some(runtime)) => {
            // The top-level executable is the runtime; do not ship its second copy.
            fs::remove_file(&runtime.output_path).map_err(|err| {
                invalid_bundle(format!("remove duplicate runtime artifact: {err}"))
            })?;
            Some(executable.clone())
        }
        _ => runtime_artifact
            .as_ref()
            .map(|runtime| runtime.rel_path.clone()),
    };
    let launch_target = executable_rel.as_deref();
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
        project_root: normalize_path_display(project_root),
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

#[cfg(test)]
mod publication_tests {
    use super::*;
    use std::cell::Cell;

    fn prepared() -> (tempfile::TempDir, PathBuf, PathBuf, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let staging = dir.path().join("staging");
        let destination = dir.path().join("bundle");
        let backup = dir.path().join("rollback");
        fs::create_dir(&staging).unwrap();
        fs::create_dir(&destination).unwrap();
        fs::write(staging.join("new"), b"new").unwrap();
        fs::write(destination.join("old"), b"old").unwrap();
        (dir, staging, destination, backup)
    }

    #[test]
    fn published_bundle_reports_failed_backup_cleanup() {
        let (_dir, staging, destination, backup) = prepared();
        let failure = publish_staged_bundle_with(
            &staging,
            &destination,
            &backup,
            |from, to| fs::rename(from, to),
            |_| Err(std::io::Error::other("cleanup denied")),
        )
        .unwrap_err();
        let message = failure.error.to_string();
        assert!(message.contains("bundle published at"), "{message}");
        assert!(
            message.contains(&destination.display().to_string()),
            "{message}"
        );
        assert!(message.contains(&backup.display().to_string()), "{message}");
        assert!(
            message.contains("cleanup denied") && message.contains("may be incomplete"),
            "{message}"
        );
        assert!(!failure.cleanup_staging);
        assert_eq!(fs::read(destination.join("new")).unwrap(), b"new");
        assert!(backup.exists());
        assert!(!staging.exists());
    }

    #[test]
    fn failed_publication_reports_restore_status_and_retains_staging() {
        for fail_restore in [false, true] {
            let (_dir, staging, destination, backup) = prepared();
            let calls = Cell::new(0);
            let failure = publish_staged_bundle_with(
                &staging,
                &destination,
                &backup,
                |from, to| {
                    let call = calls.get() + 1;
                    calls.set(call);
                    match call {
                        2 => Err(std::io::Error::other("publish denied")),
                        3 if fail_restore => Err(std::io::Error::other("restore denied")),
                        _ => fs::rename(from, to),
                    }
                },
                |path| fs::remove_dir_all(path),
            )
            .unwrap_err();
            let message = failure.error.to_string();
            assert!(message.contains("publish denied"), "{message}");
            assert!(
                message.contains(&destination.display().to_string()),
                "{message}"
            );
            assert!(message.contains(&backup.display().to_string()), "{message}");
            assert!(!failure.cleanup_staging);
            assert_eq!(fs::read(staging.join("new")).unwrap(), b"new");
            if fail_restore {
                assert!(
                    message.contains("restore denied")
                        && message.contains("destination state is uncertain"),
                    "{message}"
                );
                assert_eq!(fs::read(backup.join("old")).unwrap(), b"old");
                assert!(!destination.exists());
            } else {
                assert!(
                    message.contains("previous destination restored"),
                    "{message}"
                );
                assert_eq!(fs::read(destination.join("old")).unwrap(), b"old");
                assert!(!backup.exists());
            }
        }
    }
}
