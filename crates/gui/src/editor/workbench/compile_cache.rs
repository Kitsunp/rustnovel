use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

use crate::editor::compiler::CompilationResult;

use super::EditorWorkbench;

#[derive(Clone, Debug, PartialEq, Eq)]
struct CompilationCacheKey {
    graph_hash: u64,
    project_root: Option<PathBuf>,
}

#[derive(Clone)]
struct CompilationCacheEntry {
    key: CompilationCacheKey,
    result: CompilationResult,
}

#[derive(Clone, Default)]
pub(super) struct CompilationCache {
    entry: Option<CompilationCacheEntry>,
    hits: usize,
    misses: usize,
}

impl CompilationCache {
    fn get_or_compile(
        &mut self,
        graph: &crate::editor::node_graph::NodeGraph,
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
        let result =
            crate::editor::compiler::compile_project_with_project_root(graph, project_root);
        self.entry = Some(CompilationCacheEntry {
            key,
            result: result.clone(),
        });
        result
    }

    #[cfg(test)]
    fn stats(&self) -> (usize, usize) {
        (self.hits, self.misses)
    }
}

impl CompilationCacheKey {
    fn from_graph(
        graph: &crate::editor::node_graph::NodeGraph,
        project_root: Option<&Path>,
    ) -> Self {
        let mut hasher = DefaultHasher::new();
        serde_json::to_vec(graph)
            .unwrap_or_default()
            .hash(&mut hasher);
        let project_root = project_root.map(Path::to_path_buf);
        project_root.hash(&mut hasher);
        Self {
            graph_hash: hasher.finish(),
            project_root,
        }
    }
}

impl EditorWorkbench {
    pub(super) fn compile_current_graph(&mut self) -> CompilationResult {
        let project_root = self.project_root.clone();
        self.compilation_cache
            .get_or_compile(&self.node_graph, project_root.as_deref())
    }

    #[cfg(test)]
    pub(crate) fn compilation_cache_stats(&self) -> (usize, usize) {
        self.compilation_cache.stats()
    }
}
