use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::Path;

use crate::helpers::{
    infer_asset_kind, is_allowed_by_extension, normalize_asset_key, sha256_file_and_size,
};
use crate::model::{
    AssetError, AssetFingerprintEntry, AssetKind, BudgetReport, PlatformBudget, PlatformTarget,
    ScenePreloadPlan, TranscodeRecommendation,
};

#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct AssetFingerprintCatalog {
    pub entries: BTreeMap<String, AssetFingerprintEntry>,
    pub dedup_groups: BTreeMap<String, Vec<String>>,
}

impl AssetFingerprintCatalog {
    pub fn build(root: &Path, allowed_extensions: &[&str]) -> Result<Self, AssetError> {
        let mut entries = BTreeMap::new();
        let mut dedup_groups: BTreeMap<String, Vec<String>> = BTreeMap::new();
        let allowed: HashSet<String> = allowed_extensions
            .iter()
            .map(|value| value.to_ascii_lowercase())
            .collect();
        let mut stack = vec![root.to_path_buf()];

        while let Some(dir) = stack.pop() {
            for entry in fs::read_dir(&dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                    continue;
                }

                if !is_allowed_by_extension(&path, &allowed) {
                    continue;
                }

                let rel = path
                    .strip_prefix(root)
                    .map_err(|_| AssetError::Traversal)?
                    .to_path_buf();
                let rel = normalize_asset_key(&rel);
                let (sha256, size) = sha256_file_and_size(&path)?;
                entries.insert(
                    rel.clone(),
                    AssetFingerprintEntry {
                        rel_path: rel.clone(),
                        sha256: sha256.clone(),
                        size,
                    },
                );
                dedup_groups.entry(sha256).or_default().push(rel);
            }
        }

        Ok(Self {
            entries,
            dedup_groups,
        })
    }

    pub fn unique_blob_count(&self) -> usize {
        self.dedup_groups.len()
    }

    pub fn duplicate_blob_count(&self) -> usize {
        self.dedup_groups
            .values()
            .map(Vec::len)
            .filter(|count| *count > 1)
            .map(|count| count - 1)
            .sum()
    }

    pub fn budget_report(&self, budget: PlatformBudget) -> BudgetReport {
        let total_bytes = self.entries.values().map(|entry| entry.size).sum();
        let asset_count = self.entries.len();
        let within_budget =
            total_bytes <= budget.max_total_bytes && asset_count <= budget.max_assets;
        BudgetReport {
            total_bytes,
            asset_count,
            duplicate_blob_count: self.duplicate_blob_count(),
            unique_blob_count: self.unique_blob_count(),
            within_budget,
        }
    }

    pub fn transcode_recommendations(
        &self,
        target: PlatformTarget,
    ) -> Vec<TranscodeRecommendation> {
        let preset = target.default_transcode_preset();
        let mut output = Vec::new();

        for entry in self.entries.values() {
            let source_extension = Path::new(&entry.rel_path)
                .extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| ext.to_ascii_lowercase())
                .unwrap_or_default();
            let kind = infer_asset_kind(&entry.rel_path);
            let target_extension = match kind {
                AssetKind::Image => Some(preset.image_extension),
                AssetKind::Audio => Some(preset.audio_extension),
                AssetKind::Other => None,
            };

            let Some(target_extension) = target_extension else {
                continue;
            };
            if source_extension == target_extension {
                continue;
            }

            output.push(TranscodeRecommendation {
                rel_path: entry.rel_path.clone(),
                kind,
                source_extension,
                target_extension: target_extension.to_string(),
                reason: format!(
                    "target={:?} prefers .{} for {:?} assets",
                    target, target_extension, kind
                ),
            });
        }

        output
    }

    pub fn scene_preload_plan(scene_assets: &BTreeMap<String, Vec<String>>) -> ScenePreloadPlan {
        let mut by_scene = BTreeMap::new();
        let mut unique = std::collections::BTreeSet::new();
        let mut total_references = 0usize;

        for (scene_id, raw_assets) in scene_assets {
            let mut local_seen = HashSet::new();
            let mut local_assets = Vec::new();
            for asset in raw_assets {
                let trimmed = asset.trim();
                if trimmed.is_empty() {
                    continue;
                }
                total_references = total_references.saturating_add(1);
                if local_seen.insert(trimmed.to_string()) {
                    local_assets.push(trimmed.to_string());
                }
                unique.insert(trimmed.to_string());
            }
            by_scene.insert(scene_id.clone(), local_assets);
        }

        let unique_assets: Vec<String> = unique.into_iter().collect();
        let deduped_references = unique_assets.len();
        let cache_hit_rate = if total_references == 0 {
            1.0
        } else {
            ((total_references.saturating_sub(deduped_references)) as f32)
                / (total_references as f32)
        };

        ScenePreloadPlan {
            by_scene,
            unique_assets,
            total_references,
            deduped_references,
            cache_hit_rate,
        }
    }
}
