use std::path::Path;

use crate::editor::authoring_adapter::to_authoring_graph;
use crate::editor::NodeGraph;
use visual_novel_engine::FlowGraphAnalysis;

pub(super) fn has_outgoing(graph: &NodeGraph, node_id: u32) -> bool {
    graph.connections.iter().any(|c| c.from == node_id)
}

pub(super) fn default_asset_exists(path: &str) -> bool {
    let candidate = Path::new(path.trim());
    if candidate.is_absolute() {
        return candidate.is_file();
    }

    match std::env::current_dir() {
        Ok(cwd) => cwd.join(candidate).is_file(),
        Err(_) => candidate.is_file(),
    }
}

#[cfg(test)]
pub(super) fn asset_exists_from_project_root(project_root: &Path, path: &str) -> bool {
    let candidate = Path::new(path.trim());
    if candidate.is_absolute() {
        return candidate.is_file();
    }
    project_root.join(candidate).is_file()
}

pub(super) fn should_probe_asset_exists(path: &str) -> bool {
    let p = path.trim();
    if p.is_empty() {
        return false;
    }

    p.contains('/')
        || p.contains('\\')
        || Path::new(p).extension().is_some()
        || p.starts_with("assets/")
        || p.starts_with("assets\\")
}

pub(super) fn is_valid_audio_channel(channel: &str) -> bool {
    matches!(channel, "bgm" | "sfx" | "voice")
}

pub(super) fn is_valid_audio_action(action: &str) -> bool {
    matches!(action, "play" | "stop" | "fade_out")
}

pub(super) fn is_valid_transition_kind(kind: &str) -> bool {
    matches!(
        kind.trim().to_ascii_lowercase().as_str(),
        "fade" | "fade_black" | "dissolve" | "cut"
    )
}

pub(super) fn is_unsafe_asset_ref(path: &str) -> bool {
    let p = path.trim();
    if p.is_empty() {
        return false;
    }

    p.contains("..")
        || p.starts_with('/')
        || p.starts_with('\\')
        || p.starts_with("http://")
        || p.starts_with("https://")
        || p.chars().nth(1).is_some_and(|second| {
            second == ':' && p.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
        })
}

pub(super) fn analyze_editor_flow(graph: &NodeGraph, start_nodes: &[u32]) -> FlowGraphAnalysis {
    to_authoring_graph(graph).flow_analysis(start_nodes)
}
