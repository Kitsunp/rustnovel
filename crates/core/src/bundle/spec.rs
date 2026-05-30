use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::ExportCapabilityReport;

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

    pub fn expected_executable_name(self) -> &'static str {
        match self {
            Self::Windows => "game.exe",
            Self::Linux | Self::Macos => "game",
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
pub struct BundleFileEntry {
    pub path: String,
    pub sha256: String,
    pub size: u64,
    pub role: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportDiagnostic {
    pub code: String,
    pub severity: String,
    pub phase: String,
    pub target: String,
    pub trace_id: String,
    pub message: String,
    pub probable_cause: String,
    pub suggested_action: String,
    pub consequence: String,
    pub blocking_release: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub asset: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub node: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportBundleReport {
    pub schema: String,
    pub target_platform: String,
    pub output_layout_version: u16,
    pub project_root: String,
    pub output_root: String,
    #[serde(default)]
    pub generator_os: String,
    pub script_source: String,
    pub script_binary: String,
    pub assets_manifest: String,
    pub assets_copied: usize,
    pub runtime_artifact: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime_artifact_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub executable: Option<String>,
    #[serde(default)]
    pub expected_executable: String,
    pub launcher: String,
    #[serde(default)]
    pub graphics_backend: String,
    #[serde(default)]
    pub wgpu_fallback: bool,
    #[serde(default)]
    pub total_size: u64,
    #[serde(default)]
    pub hashes: Vec<BundleFileEntry>,
    pub integrity: String,
    pub bundle_hmac_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bundle_file_manifest: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bundle_file_manifest_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compat_report: Option<String>,
    #[serde(default)]
    pub integrity_scope: String,
    #[serde(default)]
    pub capabilities: ExportCapabilityReport,
    #[serde(default)]
    pub diagnostics: Vec<ExportDiagnostic>,
    #[serde(default)]
    pub smoke_result: ExportRuntimeSmokeResult,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ExportRuntimeSmokeCheck {
    pub code: String,
    pub status: String,
    #[serde(default)]
    pub severity: String,
    pub phase: String,
    pub target: String,
    pub trace_id: String,
    pub message: String,
    #[serde(default)]
    pub probable_cause: String,
    #[serde(default)]
    pub suggested_action: String,
    #[serde(default)]
    pub consequence: String,
    #[serde(default)]
    pub blocking_release: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub asset: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub node: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ExportRuntimeSmokeResult {
    pub status: String,
    pub backend: String,
    pub details: String,
    #[serde(default)]
    pub phase: String,
    #[serde(default)]
    pub target: String,
    #[serde(default)]
    pub trace_id: String,
    #[serde(default)]
    pub checks: Vec<ExportRuntimeSmokeCheck>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportCompatReport {
    pub schema: String,
    pub target_platform: String,
    pub generator_os: String,
    pub runtime_artifact: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime_artifact_sha256: Option<String>,
    pub expected_executable: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub executable: Option<String>,
    pub graphics_backend: String,
    pub wgpu_fallback: bool,
    pub assets_copied: usize,
    pub total_size: u64,
    pub diagnostics: Vec<ExportDiagnostic>,
    pub hashes: Vec<BundleFileEntry>,
    pub bundle_file_manifest_sha256: String,
    pub bundle_hmac_sha256: Option<String>,
    pub smoke_result: ExportRuntimeSmokeResult,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportPlan {
    pub schema: String,
    pub target_platform: String,
    pub output_layout_version: u16,
    pub project_root: String,
    pub output_root: String,
    pub entry_script: String,
    pub script_sha256: String,
    pub layout: Vec<String>,
    pub runtime_artifact: Option<String>,
    pub executable: Option<String>,
    #[serde(default, skip_serializing, skip_deserializing)]
    pub warnings: Vec<String>,
    #[serde(default, skip_serializing, skip_deserializing)]
    pub errors: Vec<String>,
    #[serde(default)]
    pub diagnostics: Vec<ExportDiagnostic>,
    pub integrity: String,
    pub capabilities: ExportCapabilityReport,
}
