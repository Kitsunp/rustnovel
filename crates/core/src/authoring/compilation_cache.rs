use std::fs;
use std::path::{Path, PathBuf};

use super::compiler::CompilationResult;
use super::{collect_authoring_asset_refs, should_probe_asset_exists, NodeGraph};
use crate::hashing::{hex_digest, sha256_json};
use sha2::{Digest, Sha256};

#[derive(Clone, Debug, PartialEq, Eq)]
struct CompilationCacheKey {
    graph_sha256: String,
    project_root: Option<PathBuf>,
    asset_state_sha256: String,
}

#[derive(Clone)]
struct CompilationCacheEntry {
    key: CompilationCacheKey,
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
        let key = CompilationCacheKey::from_graph(graph, project_root);
        if let Some(entry) = &self.entry {
            if entry.key == key {
                self.hits += 1;
                return entry.result.clone();
            }
        }

        self.misses += 1;
        let result = super::compiler::compile_authoring_graph(graph, project_root);
        self.entry = Some(CompilationCacheEntry {
            key,
            result: result.clone(),
        });
        result
    }

    pub fn stats(&self) -> (usize, usize) {
        (self.hits, self.misses)
    }
}

impl CompilationCacheKey {
    fn from_graph(graph: &NodeGraph, project_root: Option<&Path>) -> Self {
        let graph_sha256 =
            sha256_json(graph).unwrap_or_else(|err| format!("serialization_error:{err}"));
        let project_root = project_root.map(Path::to_path_buf);
        let asset_state_sha256 = project_root
            .as_deref()
            .map(|root| hash_referenced_asset_state(graph, root))
            .unwrap_or_default();
        Self {
            graph_sha256,
            project_root,
            asset_state_sha256,
        }
    }
}

fn hash_referenced_asset_state(graph: &NodeGraph, project_root: &Path) -> String {
    let mut hasher = Sha256::new();
    let canonical_root = project_root
        .canonicalize()
        .map_err(|err| format!("root_error:{:?}", err.kind()));
    for asset in collect_authoring_asset_refs(graph) {
        let asset = asset.trim();
        if !should_probe_asset_exists(asset) {
            continue;
        }
        hasher.update(asset.as_bytes());
        hasher.update([0]);
        let path = Path::new(asset);
        let candidate = if path.is_absolute() {
            path.to_path_buf()
        } else {
            project_root.join(path)
        };
        let candidate = if path.is_absolute() {
            Ok(candidate)
        } else {
            match &canonical_root {
                Ok(root) => match candidate.canonicalize() {
                    Ok(canonical_candidate) if canonical_candidate.starts_with(root) => {
                        Ok(canonical_candidate)
                    }
                    Ok(_) => Err("traversal".to_string()),
                    Err(err) => Err(format!("{:?}", err.kind())),
                },
                Err(err) => Err(err.clone()),
            }
        };
        match candidate.and_then(|candidate| {
            fs::metadata(&candidate)
                .map(|metadata| (candidate, metadata))
                .map_err(|err| format!("{:?}", err.kind()))
        }) {
            Ok((candidate, metadata)) => {
                hasher.update([1, metadata.is_file() as u8]);
                hasher.update(metadata.len().to_le_bytes());
                if metadata.is_file() {
                    match sha256_file(&candidate) {
                        Ok(file_hash) => hasher.update(file_hash.as_bytes()),
                        Err(err) => hasher.update(format!("read_error:{err}").as_bytes()),
                    }
                }
            }
            Err(error) => {
                hasher.update([0]);
                hasher.update(error.as_bytes());
            }
        }
        hasher.update([0xff]);
    }
    hex_digest(&hasher.finalize())
}

fn sha256_file(path: &Path) -> Result<String, std::io::Error> {
    crate::atomic_io::sha256_file(path).map(|(hash, _)| hash)
}

#[cfg(test)]
#[path = "compilation_cache_tests.rs"]
mod tests;
