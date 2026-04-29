use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use walkdir::WalkDir;

use crate::error::VnResult;
use crate::load_runtime_script_from_entry;
use crate::manifest::ProjectManifest;

type HmacSha256 = Hmac<Sha256>;

#[path = "bundle/helpers.rs"]
mod helpers;
use helpers::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExportTargetPlatform {
    Windows,
    Linux,
    Macos,
}

impl ExportTargetPlatform {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Windows => "windows",
            Self::Linux => "linux",
            Self::Macos => "macos",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BundleIntegrity {
    None,
    HmacSha256,
}

impl BundleIntegrity {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::HmacSha256 => "hmac_sha256",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportBundleSpec {
    pub project_root: PathBuf,
    pub output_root: PathBuf,
    pub target_platform: ExportTargetPlatform,
    pub entry_script: Option<PathBuf>,
    pub runtime_artifact: Option<PathBuf>,
    pub integrity: BundleIntegrity,
    pub output_layout_version: u16,
    pub hmac_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleAssetEntry {
    pub sha256: String,
    pub size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportBundleReport {
    pub schema: String,
    pub target_platform: String,
    pub output_layout_version: u16,
    pub project_root: String,
    pub output_root: String,
    pub script_source: String,
    pub script_binary: String,
    pub assets_manifest: String,
    pub assets_copied: usize,
    pub runtime_artifact: Option<String>,
    pub launcher: String,
    pub integrity: String,
    pub bundle_hmac_sha256: Option<String>,
}

pub fn export_bundle(spec: ExportBundleSpec) -> VnResult<ExportBundleReport> {
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

    let script_source_out = scripts_dir.join(&entry_script);
    if let Some(parent) = script_source_out.parent() {
        fs::create_dir_all(parent).map_err(|e| {
            invalid_bundle(format!(
                "create script source parent '{}': {e}",
                parent.display()
            ))
        })?;
    }
    fs::copy(&script_source_path, &script_source_out).map_err(|e| {
        invalid_bundle(format!(
            "copy script source '{}' -> '{}': {e}",
            script_source_path.display(),
            script_source_out.display()
        ))
    })?;

    let mut script_binary_rel = entry_script.clone();
    script_binary_rel.set_extension("vnc");
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

    let assets_manifest_entries = copy_assets_tree(&project_root.join("assets"), &assets_dir)?;
    let assets_manifest_json = serde_json::to_string_pretty(&assets_manifest_entries)
        .map_err(|e| invalid_bundle(format!("serialize assets manifest: {e}")))?;
    let assets_manifest_out = meta_dir.join("assets_manifest.json");
    fs::write(&assets_manifest_out, assets_manifest_json.as_bytes()).map_err(|e| {
        invalid_bundle(format!(
            "write assets manifest '{}': {e}",
            assets_manifest_out.display()
        ))
    })?;

    let runtime_rel = copy_runtime_artifact(
        spec.runtime_artifact.as_deref(),
        &project_root,
        &runtime_dir,
    )?;
    let launcher_rel = write_launcher(spec.target_platform, &output_root, runtime_rel.as_deref())?;

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
        script_source: normalize_path_display(Path::new("scripts").join(&entry_script).as_path()),
        script_binary: normalize_path_display(
            Path::new("scripts").join(&script_binary_rel).as_path(),
        ),
        assets_manifest: normalize_path_display(Path::new("meta/assets_manifest.json")),
        assets_copied: assets_manifest_entries.len(),
        runtime_artifact: runtime_rel,
        launcher: launcher_rel,
        integrity: spec.integrity.as_str().to_string(),
        bundle_hmac_sha256,
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

fn copy_assets_tree(
    assets_source_root: &Path,
    assets_output_root: &Path,
) -> VnResult<BTreeMap<String, BundleAssetEntry>> {
    let mut manifest = BTreeMap::new();
    if !assets_source_root.exists() {
        return Ok(manifest);
    }

    for entry in WalkDir::new(assets_source_root).follow_links(true) {
        let entry = entry.map_err(|e| invalid_bundle(format!("walk assets tree: {e}")))?;
        if !entry.file_type().is_file() {
            continue;
        }
        let source = entry.path();
        let rel_raw = source.strip_prefix(assets_source_root).map_err(|e| {
            invalid_bundle(format!(
                "strip assets prefix '{}' from '{}': {e}",
                assets_source_root.display(),
                source.display()
            ))
        })?;
        let rel = sanitize_relative_path(rel_raw, "assets path")?;
        let destination = assets_output_root.join(&rel);
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).map_err(|e| {
                invalid_bundle(format!("create asset parent '{}': {e}", parent.display()))
            })?;
        }
        let canonical_source = canonicalize_within_root(assets_source_root, source, "asset")?;
        fs::copy(&canonical_source, &destination).map_err(|e| {
            invalid_bundle(format!(
                "copy asset '{}' -> '{}': {e}",
                canonical_source.display(),
                destination.display()
            ))
        })?;
        let bytes = fs::read(&canonical_source).map_err(|e| {
            invalid_bundle(format!(
                "read copied asset '{}': {e}",
                canonical_source.display()
            ))
        })?;
        manifest.insert(
            normalize_path_display(&rel),
            BundleAssetEntry {
                sha256: sha256_hex(&bytes),
                size: bytes.len() as u64,
            },
        );
    }

    Ok(manifest)
}

fn copy_runtime_artifact(
    runtime_artifact: Option<&Path>,
    project_root: &Path,
    runtime_output_root: &Path,
) -> VnResult<Option<String>> {
    let Some(raw_path) = runtime_artifact else {
        return Ok(None);
    };

    let source = if raw_path.is_absolute() {
        raw_path
            .canonicalize()
            .map_err(|e| invalid_bundle(format!("canonicalize runtime artifact: {e}")))?
    } else {
        let safe_rel = sanitize_relative_path(raw_path, "runtime_artifact")?;
        canonicalize_within_root(project_root, &safe_rel, "runtime_artifact")?
    };
    if !source.is_file() {
        return Err(invalid_bundle(format!(
            "runtime artifact is not a file '{}'",
            source.display()
        )));
    }

    let file_name = source
        .file_name()
        .ok_or_else(|| invalid_bundle("runtime artifact has no filename"))?;
    let destination = runtime_output_root.join(file_name);
    fs::copy(&source, &destination).map_err(|e| {
        invalid_bundle(format!(
            "copy runtime artifact '{}' -> '{}': {e}",
            source.display(),
            destination.display()
        ))
    })?;

    Ok(Some(normalize_path_display(
        Path::new("runtime").join(file_name).as_path(),
    )))
}

fn write_launcher(
    target: ExportTargetPlatform,
    output_root: &Path,
    runtime_rel: Option<&str>,
) -> VnResult<String> {
    match target {
        ExportTargetPlatform::Windows => {
            let launcher_path = output_root.join("launch.bat");
            let content = if let Some(runtime) = runtime_rel {
                format!("@echo off\r\nsetlocal\r\n\"%~dp0{runtime}\" %*\r\n")
            } else {
                "@echo off\r\necho Runtime artifact missing in bundle\r\nexit /b 1\r\n".to_string()
            };
            fs::write(&launcher_path, content).map_err(|e| {
                invalid_bundle(format!("write launcher '{}': {e}", launcher_path.display()))
            })?;
            Ok("launch.bat".to_string())
        }
        ExportTargetPlatform::Linux | ExportTargetPlatform::Macos => {
            let launcher_path = output_root.join("launch.sh");
            let content = if let Some(runtime) = runtime_rel {
                format!(
                    "#!/usr/bin/env sh\nset -eu\nDIR=\"$(CDPATH= cd -- \"$(dirname -- \"$0\")\" && pwd)\"\nexec \"$DIR/{runtime}\" \"$@\"\n"
                )
            } else {
                "#!/usr/bin/env sh\nset -eu\necho \"Runtime artifact missing in bundle\"\nexit 1\n"
                    .to_string()
            };
            fs::write(&launcher_path, content).map_err(|e| {
                invalid_bundle(format!("write launcher '{}': {e}", launcher_path.display()))
            })?;
            Ok("launch.sh".to_string())
        }
    }
}
