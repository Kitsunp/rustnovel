use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{CharacterPlacementRaw, EventRaw, ScenePatchRaw, ScriptRaw};

use super::{
    AuthoringDocument, NodeGraph, SceneProfile, StoryNode, AUTHORING_DOCUMENT_SCHEMA_VERSION,
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthoringReportBuildInfo {
    pub engine_version: String,
    pub build_profile: String,
    pub target_os: String,
    pub target_arch: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthoringSemanticFingerprint {
    pub script_sha256: String,
    pub graph_sha256: String,
    pub story_graph_sha256: String,
    pub asset_refs_sha256: String,
    pub asset_refs_count: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthoringReportFingerprint {
    pub fingerprint_schema_version: String,
    pub authoring_schema_version: String,
    pub story_semantic_sha256: String,
    pub layout_sha256: String,
    pub assets_sha256: String,
    pub full_document_sha256: String,
    pub semantic_sha256: String,
    pub semantic: AuthoringSemanticFingerprint,
    // Kept top-level for v1 report readers and human inspection.
    pub script_sha256: String,
    pub graph_sha256: String,
    pub asset_refs_sha256: String,
    pub asset_refs_count: usize,
    pub build: AuthoringReportBuildInfo,
}

pub fn build_authoring_report_fingerprint(
    graph: &NodeGraph,
    script: &ScriptRaw,
) -> AuthoringReportFingerprint {
    let document = AuthoringDocument::new(graph.clone());
    build_authoring_document_report_fingerprint(&document, script)
}

pub fn build_authoring_document_report_fingerprint(
    document: &AuthoringDocument,
    script: &ScriptRaw,
) -> AuthoringReportFingerprint {
    let graph = &document.graph;
    let mut asset_refs = collect_authoring_asset_refs(graph);
    asset_refs.sort();
    asset_refs.dedup();
    let story_graph_sha256 = authoring_story_graph_sha256(graph);
    let semantic = AuthoringSemanticFingerprint {
        script_sha256: sha256_json(script),
        graph_sha256: story_graph_sha256.clone(),
        story_graph_sha256,
        asset_refs_sha256: sha256_json(&asset_refs),
        asset_refs_count: asset_refs.len(),
    };
    let story_semantic_sha256 = sha256_json(&semantic);
    let layout_sha256 = authoring_document_layout_sha256(document);
    let full_document_sha256 = authoring_document_sha256(document);

    AuthoringReportFingerprint {
        fingerprint_schema_version: "vnengine.authoring.fingerprint.v2".to_string(),
        authoring_schema_version: AUTHORING_DOCUMENT_SCHEMA_VERSION.to_string(),
        script_sha256: semantic.script_sha256.clone(),
        graph_sha256: full_document_sha256.clone(),
        asset_refs_sha256: semantic.asset_refs_sha256.clone(),
        asset_refs_count: semantic.asset_refs_count,
        story_semantic_sha256: story_semantic_sha256.clone(),
        layout_sha256,
        assets_sha256: semantic.asset_refs_sha256.clone(),
        full_document_sha256,
        semantic_sha256: story_semantic_sha256,
        semantic,
        build: AuthoringReportBuildInfo {
            engine_version: env!("CARGO_PKG_VERSION").to_string(),
            build_profile: build_profile().to_string(),
            target_os: std::env::consts::OS.to_string(),
            target_arch: std::env::consts::ARCH.to_string(),
        },
    }
}

pub fn authoring_graph_sha256(graph: &NodeGraph) -> String {
    sha256_json(&AuthoringDocument::new(graph.clone()))
}

pub fn authoring_document_sha256(document: &AuthoringDocument) -> String {
    sha256_json(document)
}

pub fn authoring_story_graph_sha256(graph: &NodeGraph) -> String {
    let mut nodes = graph
        .nodes()
        .map(|(id, node, _)| serde_json::json!({ "id": id, "node": node }))
        .collect::<Vec<_>>();
    nodes.sort_by_key(|value| value.get("id").and_then(serde_json::Value::as_u64));
    let mut connections = graph.connections().cloned().collect::<Vec<_>>();
    connections.sort_by_key(|conn| (conn.from, conn.from_port, conn.to));
    let scene_profiles = graph
        .scene_profiles()
        .map(|(id, profile)| serde_json::json!({ "id": id, "profile": profile }))
        .collect::<Vec<_>>();
    let fragments = graph
        .fragments()
        .map(|(id, fragment)| serde_json::json!({ "id": id, "fragment": fragment }))
        .collect::<Vec<_>>();
    sha256_json(&serde_json::json!({
        "nodes": nodes,
        "connections": connections,
        "scene_profiles": scene_profiles,
        "fragments": fragments,
    }))
}

pub fn authoring_layout_sha256(graph: &NodeGraph) -> String {
    sha256_json(&graph_layout_payload(graph))
}

pub fn authoring_document_layout_sha256(document: &AuthoringDocument) -> String {
    sha256_json(&serde_json::json!({
        "graph": graph_layout_payload(&document.graph),
        "composer_layer_overrides": document.composer_layer_overrides,
    }))
}

fn graph_layout_payload(graph: &NodeGraph) -> serde_json::Value {
    let mut positions = graph
        .nodes()
        .map(|(id, _, position)| {
            serde_json::json!({
                "id": id,
                "x": position.x,
                "y": position.y,
            })
        })
        .collect::<Vec<_>>();
    positions.sort_by_key(|value| value.get("id").and_then(serde_json::Value::as_u64));
    let bookmarks = graph
        .bookmarks()
        .map(|(name, target)| serde_json::json!({ "name": name, "target": target }))
        .collect::<Vec<_>>();
    serde_json::json!({
        "positions": positions,
        "bookmarks": bookmarks,
    })
}

pub fn authoring_fingerprints_semantically_match(
    imported: &serde_json::Value,
    current: &serde_json::Value,
) -> bool {
    let Some(imported_semantic) = semantic_value(imported) else {
        return false;
    };
    let Some(current_semantic) = semantic_value(current) else {
        return false;
    };
    imported_semantic == current_semantic
}

fn semantic_value(value: &serde_json::Value) -> Option<serde_json::Value> {
    if let Some(hash) = value
        .get("story_semantic_sha256")
        .or_else(|| value.get("semantic_sha256"))
        .and_then(serde_json::Value::as_str)
    {
        return Some(serde_json::Value::String(hash.to_string()));
    }
    if let Some(semantic) = value.get("semantic") {
        return Some(semantic.clone());
    }
    let script_sha256 = value.get("script_sha256")?.clone();
    let graph_sha256 = value
        .get("story_graph_sha256")
        .or_else(|| value.get("graph_sha256"))?
        .clone();
    let asset_refs_sha256 = value.get("asset_refs_sha256")?.clone();
    let asset_refs_count = value.get("asset_refs_count")?.clone();
    Some(serde_json::json!({
        "script_sha256": script_sha256,
        "graph_sha256": graph_sha256,
        "asset_refs_sha256": asset_refs_sha256,
        "asset_refs_count": asset_refs_count,
    }))
}

pub fn collect_authoring_asset_refs(graph: &NodeGraph) -> Vec<String> {
    let mut refs = Vec::new();
    for (_, node, _) in graph.nodes() {
        collect_node_asset_refs(node, &mut refs);
    }
    for (_, profile) in graph.scene_profiles() {
        collect_profile_asset_refs(profile, &mut refs);
    }
    refs.sort();
    refs.dedup();
    refs
}

fn collect_node_asset_refs(node: &StoryNode, refs: &mut Vec<String>) {
    match node {
        StoryNode::Scene {
            background,
            music,
            characters,
            ..
        } => {
            push_optional(background, refs);
            push_optional(music, refs);
            collect_character_assets(characters, refs);
        }
        StoryNode::ScenePatch(patch) => collect_patch_asset_refs(patch, refs),
        StoryNode::AudioAction {
            asset: Some(asset), ..
        } => refs.push(asset.clone()),
        StoryNode::Generic(event) => collect_event_asset_refs(event, refs),
        _ => {}
    }
}

fn collect_event_asset_refs(event: &EventRaw, refs: &mut Vec<String>) {
    match event {
        EventRaw::Scene(scene) => {
            push_optional(&scene.background, refs);
            push_optional(&scene.music, refs);
            collect_character_assets(&scene.characters, refs);
        }
        EventRaw::Patch(patch) => collect_patch_asset_refs(patch, refs),
        EventRaw::AudioAction(action) => push_optional(&action.asset, refs),
        _ => {}
    }
}

fn collect_patch_asset_refs(patch: &ScenePatchRaw, refs: &mut Vec<String>) {
    push_optional(&patch.background, refs);
    push_optional(&patch.music, refs);
    collect_character_assets(&patch.add, refs);
    for character in &patch.update {
        push_optional(&character.expression, refs);
    }
}

fn collect_profile_asset_refs(profile: &SceneProfile, refs: &mut Vec<String>) {
    push_optional(&profile.background, refs);
    push_optional(&profile.music, refs);
    collect_character_assets(&profile.characters, refs);
    for layer in &profile.layers {
        push_optional(&layer.background, refs);
        collect_character_assets(&layer.characters, refs);
    }
    for pose in &profile.poses {
        refs.push(pose.image.clone());
    }
}

fn collect_character_assets(characters: &[CharacterPlacementRaw], refs: &mut Vec<String>) {
    for character in characters {
        push_optional(&character.expression, refs);
    }
}

fn push_optional(value: &Option<String>, refs: &mut Vec<String>) {
    if let Some(value) = value
        .as_ref()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
    {
        refs.push(value.to_string());
    }
}

fn sha256_json<T: Serialize>(value: &T) -> String {
    match serde_json::to_vec(value) {
        Ok(bytes) => sha256_bytes(&bytes),
        Err(error) => sha256_bytes(error.to_string().as_bytes()),
    }
}

fn sha256_bytes(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn build_profile() -> &'static str {
    if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    }
}
