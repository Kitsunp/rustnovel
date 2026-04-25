use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SecurityMode {
    Trusted,
    Untrusted,
}

#[derive(Clone, Debug)]
pub struct AssetLimits {
    pub max_bytes: u64,
    pub max_width: u32,
    pub max_height: u32,
}

impl Default for AssetLimits {
    fn default() -> Self {
        Self {
            max_bytes: 15 * 1024 * 1024,
            max_width: 4096,
            max_height: 4096,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AssetManifest {
    pub manifest_version: u16,
    pub assets: BTreeMap<String, AssetEntry>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AssetEntry {
    pub sha256: String,
    pub size: u64,
}

#[derive(Debug, Error)]
pub enum AssetError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("asset path traversal blocked")]
    Traversal,
    #[error("unsupported image extension for '{0}' (supported: png, jpg, jpeg)")]
    UnsupportedExtension(String),
    #[error("asset too large: {size} bytes (max {max})")]
    TooLarge { size: u64, max: u64 },
    #[error("asset dimensions {width}x{height} exceed limit {max_width}x{max_height}")]
    InvalidDimensions {
        width: u32,
        height: u32,
        max_width: u32,
        max_height: u32,
    },
    #[error("manifest required for untrusted assets")]
    ManifestMissing,
    #[error("unsupported manifest version {0}")]
    ManifestVersion(u16),
    #[error("manifest entry missing for asset '{0}'")]
    ManifestEntryMissing(String),
    #[error("manifest hash mismatch for asset '{0}'")]
    ManifestHashMismatch(String),
    #[error("manifest size mismatch for asset '{0}'")]
    ManifestSizeMismatch(String),
    #[error("image asset not found for '{requested}'; attempted {attempts:?}")]
    ImageNotFound {
        requested: String,
        attempts: Vec<String>,
    },
    #[error("image decode error for '{path}': {reason}")]
    Decode { path: String, reason: String },
    #[error("asset exceeds cache budget: {bytes} bytes (budget {budget})")]
    BudgetExceeded { bytes: usize, budget: usize },
}

pub(crate) const SUPPORTED_IMAGE_EXTENSIONS: [&str; 3] = ["png", "jpg", "jpeg"];

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetFingerprintEntry {
    pub rel_path: String,
    pub sha256: String,
    pub size: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlatformBudget {
    pub max_total_bytes: u64,
    pub max_assets: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BudgetReport {
    pub total_bytes: u64,
    pub asset_count: usize,
    pub duplicate_blob_count: usize,
    pub unique_blob_count: usize,
    pub within_budget: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PlatformTarget {
    Desktop,
    Mobile,
    Web,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AssetKind {
    Image,
    Audio,
    Other,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TranscodePreset {
    pub target: PlatformTarget,
    pub image_extension: &'static str,
    pub audio_extension: &'static str,
    pub image_quality: u8,
    pub audio_bitrate_kbps: u16,
    pub max_texture_side: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TranscodeRecommendation {
    pub rel_path: String,
    pub kind: AssetKind,
    pub source_extension: String,
    pub target_extension: String,
    pub reason: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ScenePreloadPlan {
    pub by_scene: BTreeMap<String, Vec<String>>,
    pub unique_assets: Vec<String>,
    pub total_references: usize,
    pub deduped_references: usize,
    pub cache_hit_rate: f32,
}

pub struct LoadedImage {
    pub name: String,
    pub size: [usize; 2],
    pub pixels: Vec<u8>,
}
