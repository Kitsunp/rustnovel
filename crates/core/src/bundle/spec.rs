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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub executable: Option<String>,
    pub launcher: String,
    pub integrity: String,
    pub bundle_hmac_sha256: Option<String>,
    #[serde(default)]
    pub capabilities: ExportCapabilityReport,
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
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
    pub integrity: String,
    pub capabilities: ExportCapabilityReport,
}
