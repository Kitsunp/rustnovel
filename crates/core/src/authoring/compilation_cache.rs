use std::path::{Path, PathBuf};

use super::compiler::CompilationResult;
use super::validation::assets::resolve_asset_from_canonical_project_root;
use super::{collect_authoring_asset_refs, should_probe_asset_exists, NodeGraph};
use crate::atomic_io::sha256_file;
use crate::hashing::hex_digest;
use sha2::{Digest, Sha256};

#[derive(Clone)]
struct CompilationCacheEntry {
    graph: NodeGraph,
    project_root: Option<PathBuf>,
    asset_state_sha256: Option<String>,
    result: CompilationResult,
}

#[derive(Clone, Default)]
pub struct CompilationCache {
    entry: Option<CompilationCacheEntry>,
    hits: usize,
    misses: usize,
}

impl CompilationCache {
    pub fn invalidate(&mut self) {
        self.entry = None;
    }

    pub fn get_or_compile(
        &mut self,
        graph: &NodeGraph,
        project_root: Option<&Path>,
    ) -> CompilationResult {
        let asset_state_sha256 = project_root.map(|root| hash_referenced_asset_state(graph, root));
        if let Some(entry) = &self.entry {
            if entry.project_root.as_deref() == project_root
                && entry.asset_state_sha256 == asset_state_sha256
                && entry.graph.has_same_compilation_input(graph)
            {
                self.hits += 1;
                return entry.result.clone();
            }
        }

        self.misses += 1;
        let result = super::compiler::compile_authoring_graph(graph, project_root);
        self.entry = Some(CompilationCacheEntry {
            graph: graph.clone(),
            project_root: project_root.map(Path::to_path_buf),
            asset_state_sha256,
            result: result.clone(),
        });
        result
    }

    pub fn stats(&self) -> (usize, usize) {
        (self.hits, self.misses)
    }
}

fn hash_referenced_asset_state(graph: &NodeGraph, project_root: &Path) -> String {
    let mut hasher = Sha256::new();
    let canonical_root = match project_root.canonicalize() {
        Ok(root) => root,
        Err(error) => {
            hasher.update([0]);
            hasher.update(error.to_string().as_bytes());
            return hex_digest(&hasher.finalize());
        }
    };
    hasher.update([1]);
    for asset in collect_authoring_asset_refs(graph) {
        let asset = asset.trim();
        if !should_probe_asset_exists(asset) {
            continue;
        }
        hasher.update(asset.as_bytes());
        hasher.update([0]);
        match resolve_asset_from_canonical_project_root(&canonical_root, asset) {
            Ok(candidate) => match sha256_file(&candidate) {
                Ok((file_hash, size)) => {
                    hasher.update([1]);
                    hasher.update(size.to_le_bytes());
                    hasher.update(file_hash.as_bytes());
                }
                Err(error) => {
                    hasher.update([2]);
                    hasher.update(error.to_string().as_bytes());
                }
            },
            Err(error) => {
                hasher.update([0]);
                hasher.update(format!("{error:?}").as_bytes());
            }
        }
        hasher.update([0xff]);
    }
    hex_digest(&hasher.finalize())
}

#[cfg(test)]
#[path = "compilation_cache_tests.rs"]
mod tests;
