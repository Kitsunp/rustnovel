use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use crate::editor::compiler::CompilationResult;
use sha2::{Digest, Sha256};
use visual_novel_engine::authoring::{collect_authoring_asset_refs, should_probe_asset_exists};

use super::EditorWorkbench;

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

    pub fn stats(&self) -> (usize, usize) {
        (self.hits, self.misses)
    }
}

impl CompilationCacheKey {
    fn from_graph(
        graph: &crate::editor::node_graph::NodeGraph,
        project_root: Option<&Path>,
    ) -> Self {
        let graph_sha256 = serde_json::to_vec(graph)
            .map(|bytes| sha256_bytes(&bytes))
            .unwrap_or_else(|err| format!("serialization_error:{err}"));
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

fn hash_referenced_asset_state(
    graph: &crate::editor::node_graph::NodeGraph,
    project_root: &Path,
) -> String {
    let mut hasher = Sha256::new();
    let canonical_root = project_root
        .canonicalize()
        .map_err(|err| format!("root_error:{:?}", err.kind()));
    for asset in collect_authoring_asset_refs(graph.authoring_graph()) {
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
    hex_lower(&hasher.finalize())
}

fn sha256_file(path: &Path) -> Result<String, std::io::Error> {
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hex_lower(&hasher.finalize()))
}

fn sha256_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex_lower(&hasher.finalize())
}

fn hex_lower(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

impl EditorWorkbench {
    pub fn compile_current_graph(&mut self) -> CompilationResult {
        let project_root = self.project_root.clone();
        self.compilation_cache
            .get_or_compile(&self.node_graph, project_root.as_deref())
    }

    pub fn compilation_cache_stats(&self) -> (usize, usize) {
        self.compilation_cache.stats()
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::editor::{NodeGraph, StoryNode};

    use super::*;

    fn create_file_symlink(link: &Path, target: &Path) -> bool {
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(target, link).is_ok()
        }

        #[cfg(windows)]
        {
            std::os::windows::fs::symlink_file(target, link).is_ok()
        }

        #[cfg(not(any(unix, windows)))]
        {
            let _ = link;
            let _ = target;
            false
        }
    }

    #[test]
    fn referenced_asset_hash_does_not_follow_relative_symlink_escape() {
        let temp = tempfile::tempdir().expect("tempdir");
        let project_root = temp.path().join("project");
        let outside = temp.path().join("outside.png");
        std::fs::create_dir_all(project_root.join("assets/backgrounds")).expect("asset dir");
        std::fs::write(&outside, b"outside-one").expect("outside one");
        let link = project_root.join("assets/backgrounds/escape.png");
        if !create_file_symlink(&link, &outside) {
            eprintln!("file symlink creation not supported on this platform");
            return;
        }

        let mut graph = NodeGraph::new();
        graph.add_node(
            StoryNode::Scene {
                profile: None,
                background: Some("assets/backgrounds/escape.png".to_string()),
                music: None,
                characters: Vec::new(),
            },
            eframe::egui::pos2(0.0, 0.0),
        );

        let before = hash_referenced_asset_state(&graph, &project_root);
        std::fs::write(&outside, b"outside-two").expect("outside two");
        let after = hash_referenced_asset_state(&graph, &project_root);

        assert_eq!(
            before, after,
            "relative symlink escapes should hash as blocked traversal, not outside file contents"
        );
    }
}
