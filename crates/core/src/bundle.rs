use std::fs;
use std::path::{Path, PathBuf};

use hmac::{Hmac, Mac};
use sha2::Sha256;

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
#[path = "bundle/spec.rs"]
mod spec;

use assets::copy_referenced_assets;
use capabilities::build_capability_report;
use helpers::*;
use materialize::{copy_runtime_artifact, materialize_executable, write_launcher};

pub use capabilities::ExportCapabilityReport;
pub use plan::build_export_plan;
pub use spec::{
    BundleAssetEntry, BundleIntegrity, ExportBundleReport, ExportBundleSpec, ExportPlan,
    ExportTargetPlatform,
};

pub fn export_bundle(spec: ExportBundleSpec) -> VnResult<ExportBundleReport> {
    let _plan = build_export_plan(&spec)?;
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
    let assets_manifest_json = serde_json::to_string_pretty(&assets_manifest_entries)
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
            mac.update(&compiled_bytes);
            mac.update(assets_manifest_json.as_bytes());
            let manifest_bytes = fs::read(&manifest_out).map_err(|e| {
                invalid_bundle(format!(
                    "read bundle manifest for hmac '{}': {e}",
                    manifest_out.display()
                ))
            })?;
            mac.update(&manifest_bytes);
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

    let report = ExportBundleReport {
        schema: "vnengine.export_bundle_report.v1".to_string(),
        target_platform: spec.target_platform.as_str().to_string(),
        output_layout_version: spec.output_layout_version,
        project_root: normalize_path_display(&project_root),
        output_root: normalize_path_display(&output_root),
        script_source: normalize_path_display(
            Path::new("scripts").join(&runtime_script_rel).as_path(),
        ),
        script_binary: normalize_path_display(
            Path::new("scripts").join(&script_binary_rel).as_path(),
        ),
        assets_manifest: normalize_path_display(Path::new("meta/assets_manifest.json")),
        assets_copied: assets_manifest_entries.len(),
        runtime_artifact: runtime_artifact.map(|runtime| runtime.rel_path),
        executable: executable_rel,
        launcher: launcher_rel,
        integrity: spec.integrity.as_str().to_string(),
        bundle_hmac_sha256,
        capabilities: capability_report,
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

    Ok(report)
}
