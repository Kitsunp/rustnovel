#![allow(unused_assignments)]

mod cache;
mod catalog;
mod helpers;
mod model;
mod platform;
mod store;

pub use catalog::AssetFingerprintCatalog;
pub use helpers::sanitize_rel_path;
pub use model::{
    AssetEntry, AssetError, AssetFingerprintEntry, AssetKind, AssetLimits, AssetManifest,
    BudgetReport, LoadedImage, PlatformBudget, PlatformTarget, ScenePreloadPlan, SecurityMode,
    TranscodePreset, TranscodeRecommendation,
};
pub use store::AssetStore;

#[cfg(test)]
use helpers::sha256_hex;
#[cfg(test)]
use std::collections::BTreeMap;

#[cfg(test)]
#[path = "tests/lib_tests.rs"]
mod tests;
