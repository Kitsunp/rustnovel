#![allow(unused_assignments)]

mod cache;
mod catalog;
mod helpers;
mod io;
mod model;
mod platform;
mod store;

pub use catalog::AssetFingerprintCatalog;
pub use helpers::sanitize_rel_path;
pub use io::{hash_asset_file, read_asset_bytes};
pub use model::{
    AssetEntry, AssetError, AssetFingerprintEntry, AssetKind, AssetLimits, AssetManifest,
    BudgetReport, LoadedImage, PlatformBudget, PlatformTarget, ScenePreloadPlan, SecurityMode,
    TranscodePreset, TranscodeRecommendation,
};
pub use store::{decode_image_bytes, AssetStore};
