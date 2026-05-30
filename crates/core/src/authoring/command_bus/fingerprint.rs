use std::collections::BTreeMap;
use std::io;

use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::asset_refs::{
    collect_character_assets, collect_event_asset_refs, collect_scene_patch_asset_refs, AssetRefSet,
};

use super::super::report_fingerprint::build_fingerprint_from_parts;
use super::super::{
    AuthoringPosition, AuthoringReportFingerprint, AuthoringSemanticFingerprint, GraphConnection,
    GraphFragment, GraphStack, NodeGraph, StoryNode,
};
use super::AuthoringDelta;

#[derive(Clone, Debug, Default)]
pub(super) struct CommandBusFingerprintState {
    story_acc: [u8; 32],
    story_count: usize,
    layout_acc: [u8; 32],
    layout_count: usize,
    asset_acc: [u8; 32],
    node_story_hashes: BTreeMap<u32, [u8; 32]>,
    node_layout_hashes: BTreeMap<u32, [u8; 32]>,
    node_asset_refs: BTreeMap<u32, Vec<String>>,
    connection_hashes: BTreeMap<(u32, usize, u32), [u8; 32]>,
    scene_profile_hashes: BTreeMap<String, [u8; 32]>,
    fragment_hashes: BTreeMap<String, [u8; 32]>,
    bookmark_hashes: BTreeMap<String, [u8; 32]>,
    asset_ref_counts: BTreeMap<String, usize>,
    stack_hash: Option<[u8; 32]>,
}

impl CommandBusFingerprintState {
    pub(super) fn from_graph(graph: &NodeGraph) -> Self {
        let mut state = Self::default();
        for (id, node, position) in graph.nodes() {
            state.insert_node(*id, node, *position);
        }
        for connection in graph.connections() {
            state.insert_connection(connection);
        }
        state.sync_scene_profiles(graph);
        state.sync_fragments(graph);
        state.sync_bookmarks(graph);
        state.replace_stack(graph.graph_stack());
        state
    }

    pub(super) fn fingerprint(&self) -> AuthoringReportFingerprint {
        let story_graph_sha256 = component_root("story", &self.story_acc, self.story_count);
        let layout_sha256 = component_root("layout", &self.layout_acc, self.layout_count);
        let assets_sha256 = component_root("assets", &self.asset_acc, self.asset_ref_counts.len());
        let semantic = AuthoringSemanticFingerprint {
            script_sha256: story_graph_sha256.clone(),
            graph_sha256: story_graph_sha256.clone(),
            story_graph_sha256,
            asset_refs_sha256: assets_sha256.clone(),
            asset_refs_count: self.asset_ref_counts.len(),
        };
        let story_semantic_sha256 = hash_json_hex("semantic", &semantic);
        let full_document_sha256 = hash_json_hex(
            "full_document",
            &(&semantic.story_graph_sha256, &layout_sha256, &assets_sha256),
        );
        build_fingerprint_from_parts(
            semantic,
            story_semantic_sha256,
            layout_sha256,
            full_document_sha256,
        )
    }

    pub(super) fn apply_delta(&mut self, graph: &NodeGraph, delta: &AuthoringDelta) {
        match delta {
            AuthoringDelta::NodeCreated {
                node_id,
                node,
                position,
            } => self.insert_node(*node_id, node, *position),
            AuthoringDelta::NodeRemoved {
                node_id,
                connections,
                ..
            } => {
                self.remove_node(*node_id);
                for connection in connections {
                    self.remove_connection(connection);
                }
                self.sync_fragments(graph);
                self.sync_bookmarks(graph);
            }
            AuthoringDelta::Connected {
                connection,
                replaced,
            } => {
                for connection in replaced {
                    self.remove_connection(connection);
                }
                self.insert_connection(connection);
            }
            AuthoringDelta::Disconnected { removed } => {
                for connection in removed {
                    self.remove_connection(connection);
                }
            }
            AuthoringDelta::ChoiceOptionConnected {
                choice_id,
                connection,
                ..
            } => {
                self.replace_node_from_graph(graph, *choice_id);
                self.insert_connection(connection);
            }
            AuthoringDelta::ChoiceOptionTargetSet { before, after, .. } => {
                for connection in before {
                    self.remove_connection(connection);
                }
                if let Some(connection) = after {
                    self.insert_connection(connection);
                }
            }
            AuthoringDelta::BranchConnected { .. } | AuthoringDelta::QuickFixApplied { .. } => {
                self.reconcile_graph_components(graph);
            }
            AuthoringDelta::NodeEdited { node_id, .. } => {
                self.replace_node_from_graph(graph, *node_id);
            }
            AuthoringDelta::FragmentCreated { fragment } => {
                self.insert_fragment(fragment);
            }
            AuthoringDelta::FragmentRemoved { fragment, .. } => {
                self.remove_fragment(&fragment.fragment_id);
                self.replace_stack(graph.graph_stack());
            }
            AuthoringDelta::FragmentEntered { after_stack, .. }
            | AuthoringDelta::FragmentLeft { after_stack, .. } => {
                self.replace_stack(after_stack);
            }
            AuthoringDelta::FragmentPortsRefreshed { after, .. } => {
                self.replace_fragment(after);
            }
            AuthoringDelta::ChoiceOptionReordered {
                node_id,
                before_connections,
                after_connections,
                ..
            }
            | AuthoringDelta::ChoiceOptionRemoved {
                node_id,
                before_connections,
                after_connections,
                ..
            } => {
                self.replace_node_from_graph(graph, *node_id);
                for connection in before_connections {
                    self.remove_connection(connection);
                }
                for connection in after_connections {
                    self.insert_connection(connection);
                }
            }
            AuthoringDelta::AssetImported { .. } => {}
            AuthoringDelta::LayerMoved { object_id, .. } => {
                if let Some(node_id) = node_id_from_character_object_id(object_id) {
                    self.replace_node_from_graph(graph, node_id);
                }
            }
            AuthoringDelta::Reverted { reverted } => self.apply_reverted_delta(graph, reverted),
        }
    }

    pub(super) fn apply_reverted_delta(&mut self, graph: &NodeGraph, delta: &AuthoringDelta) {
        match delta {
            AuthoringDelta::NodeCreated { node_id, .. } => self.remove_node(*node_id),
            AuthoringDelta::NodeRemoved {
                node_id,
                node,
                position,
                connections,
            } => {
                self.insert_node(*node_id, node, *position);
                for connection in connections {
                    self.insert_connection(connection);
                }
                self.sync_fragments(graph);
                self.sync_bookmarks(graph);
            }
            AuthoringDelta::Connected {
                connection,
                replaced,
            } => {
                self.remove_connection(connection);
                for connection in replaced {
                    self.insert_connection(connection);
                }
            }
            AuthoringDelta::Disconnected { removed } => {
                for connection in removed {
                    self.insert_connection(connection);
                }
            }
            AuthoringDelta::ChoiceOptionConnected {
                choice_id,
                connection,
                ..
            } => {
                self.replace_node_from_graph(graph, *choice_id);
                self.remove_connection(connection);
            }
            AuthoringDelta::ChoiceOptionTargetSet {
                node_id,
                before,
                after,
                ..
            } => {
                if let Some(connection) = after {
                    self.remove_connection(connection);
                }
                for connection in before {
                    self.insert_connection(connection);
                }
                self.replace_node_from_graph(graph, *node_id);
            }
            AuthoringDelta::BranchConnected { .. } | AuthoringDelta::QuickFixApplied { .. } => {
                self.reconcile_graph_components(graph);
            }
            AuthoringDelta::NodeEdited { node_id, .. } => {
                self.replace_node_from_graph(graph, *node_id);
            }
            AuthoringDelta::FragmentCreated { fragment } => {
                self.remove_fragment(&fragment.fragment_id);
                self.replace_stack(graph.graph_stack());
            }
            AuthoringDelta::FragmentRemoved { fragment, .. } => {
                self.insert_fragment(fragment);
                self.replace_stack(graph.graph_stack());
            }
            AuthoringDelta::FragmentEntered { before_stack, .. }
            | AuthoringDelta::FragmentLeft { before_stack, .. } => {
                self.replace_stack(before_stack);
            }
            AuthoringDelta::FragmentPortsRefreshed { before, .. } => {
                self.replace_fragment(before);
            }
            AuthoringDelta::ChoiceOptionReordered { node_id, .. }
            | AuthoringDelta::ChoiceOptionRemoved { node_id, .. } => {
                self.replace_node_from_graph(graph, *node_id);
                self.sync_connections(graph);
            }
            AuthoringDelta::AssetImported { .. } => {}
            AuthoringDelta::LayerMoved { object_id, .. } => {
                if let Some(node_id) = node_id_from_character_object_id(object_id) {
                    self.replace_node_from_graph(graph, node_id);
                }
            }
            AuthoringDelta::Reverted { reverted } => self.apply_delta(graph, reverted),
        }
    }

    fn reconcile_graph_components(&mut self, graph: &NodeGraph) {
        self.sync_nodes(graph);
        self.sync_connections(graph);
        self.sync_scene_profiles(graph);
        self.sync_fragments(graph);
        self.sync_bookmarks(graph);
        self.replace_stack(graph.graph_stack());
    }

    fn sync_nodes(&mut self, graph: &NodeGraph) {
        let current = graph
            .nodes()
            .map(|(id, node, position)| (*id, (node.clone(), *position)))
            .collect::<BTreeMap<_, _>>();
        let previous = self.node_story_hashes.keys().copied().collect::<Vec<_>>();
        for id in previous {
            if !current.contains_key(&id) {
                self.remove_node(id);
            }
        }
        for (id, (node, position)) in current {
            self.replace_node(id, &node, position);
        }
    }

    fn sync_connections(&mut self, graph: &NodeGraph) {
        let current = graph
            .connections()
            .map(|connection| connection_key(connection))
            .collect::<Vec<_>>();
        let previous = self.connection_hashes.keys().copied().collect::<Vec<_>>();
        for key in previous {
            if !current.contains(&key) {
                self.remove_connection_key(key);
            }
        }
        for key in current {
            if !self.connection_hashes.contains_key(&key) {
                self.insert_connection_key(key);
            }
        }
    }

    fn sync_scene_profiles(&mut self, graph: &NodeGraph) {
        let current = graph
            .scene_profiles()
            .map(|(id, profile)| (id.clone(), leaf_hash("scene_profile", &(id, profile))))
            .collect::<BTreeMap<_, _>>();
        sync_story_hash_map(
            &mut self.story_acc,
            &mut self.story_count,
            &mut self.scene_profile_hashes,
            current,
        );
    }

    fn sync_fragments(&mut self, graph: &NodeGraph) {
        let current = graph
            .fragments()
            .map(|(id, fragment)| (id.clone(), leaf_hash("fragment", &(id, fragment))))
            .collect::<BTreeMap<_, _>>();
        sync_story_hash_map(
            &mut self.story_acc,
            &mut self.story_count,
            &mut self.fragment_hashes,
            current,
        );
    }

    fn sync_bookmarks(&mut self, graph: &NodeGraph) {
        let current = graph
            .bookmarks()
            .map(|(name, target)| (name.clone(), leaf_hash("bookmark", &(name, target))))
            .collect::<BTreeMap<_, _>>();
        sync_layout_hash_map(
            &mut self.layout_acc,
            &mut self.layout_count,
            &mut self.bookmark_hashes,
            current,
        );
    }

    fn insert_node(&mut self, node_id: u32, node: &StoryNode, position: AuthoringPosition) {
        self.replace_node(node_id, node, position);
    }

    fn replace_node_from_graph(&mut self, graph: &NodeGraph, node_id: u32) {
        if let (Some(node), Some(position)) = (graph.get_node(node_id), graph.get_node_pos(node_id))
        {
            self.replace_node(node_id, node, position);
        } else {
            self.remove_node(node_id);
        }
    }

    fn replace_node(&mut self, node_id: u32, node: &StoryNode, position: AuthoringPosition) {
        self.remove_node(node_id);
        let story_hash = leaf_hash("node", &(node_id, node));
        add_component(&mut self.story_acc, &mut self.story_count, story_hash);
        self.node_story_hashes.insert(node_id, story_hash);

        let layout_hash = leaf_hash("node_position", &(node_id, position));
        add_component(&mut self.layout_acc, &mut self.layout_count, layout_hash);
        self.node_layout_hashes.insert(node_id, layout_hash);

        let refs = node_asset_refs(node);
        for asset in &refs {
            self.add_asset_ref(asset);
        }
        self.node_asset_refs.insert(node_id, refs);
    }

    fn remove_node(&mut self, node_id: u32) {
        if let Some(hash) = self.node_story_hashes.remove(&node_id) {
            remove_component(&mut self.story_acc, &mut self.story_count, hash);
        }
        if let Some(hash) = self.node_layout_hashes.remove(&node_id) {
            remove_component(&mut self.layout_acc, &mut self.layout_count, hash);
        }
        if let Some(refs) = self.node_asset_refs.remove(&node_id) {
            for asset in refs {
                self.remove_asset_ref(&asset);
            }
        }
    }

    fn insert_connection(&mut self, connection: &GraphConnection) {
        self.insert_connection_key(connection_key(connection));
    }

    fn insert_connection_key(&mut self, key: (u32, usize, u32)) {
        if self.connection_hashes.contains_key(&key) {
            return;
        }
        let hash = leaf_hash("connection", &key);
        add_component(&mut self.story_acc, &mut self.story_count, hash);
        self.connection_hashes.insert(key, hash);
    }

    fn remove_connection(&mut self, connection: &GraphConnection) {
        self.remove_connection_key(connection_key(connection));
    }

    fn remove_connection_key(&mut self, key: (u32, usize, u32)) {
        if let Some(hash) = self.connection_hashes.remove(&key) {
            remove_component(&mut self.story_acc, &mut self.story_count, hash);
        }
    }

    fn insert_fragment(&mut self, fragment: &GraphFragment) {
        self.replace_fragment(fragment);
    }

    fn replace_fragment(&mut self, fragment: &GraphFragment) {
        self.remove_fragment(&fragment.fragment_id);
        let hash = leaf_hash("fragment", &(&fragment.fragment_id, fragment));
        add_component(&mut self.story_acc, &mut self.story_count, hash);
        self.fragment_hashes
            .insert(fragment.fragment_id.clone(), hash);
    }

    fn remove_fragment(&mut self, fragment_id: &str) {
        if let Some(hash) = self.fragment_hashes.remove(fragment_id) {
            remove_component(&mut self.story_acc, &mut self.story_count, hash);
        }
    }

    fn replace_stack(&mut self, stack: &GraphStack) {
        if let Some(hash) = self.stack_hash.take() {
            remove_component(&mut self.layout_acc, &mut self.layout_count, hash);
        }
        let hash = leaf_hash("graph_stack", stack);
        add_component(&mut self.layout_acc, &mut self.layout_count, hash);
        self.stack_hash = Some(hash);
    }

    fn add_asset_ref(&mut self, asset: &str) {
        let count = self.asset_ref_counts.entry(asset.to_string()).or_insert(0);
        if *count == 0 {
            xor_hash(&mut self.asset_acc, leaf_hash("asset_ref", &asset));
        }
        *count += 1;
    }

    fn remove_asset_ref(&mut self, asset: &str) {
        let Some(count) = self.asset_ref_counts.get_mut(asset) else {
            return;
        };
        *count -= 1;
        if *count == 0 {
            self.asset_ref_counts.remove(asset);
            xor_hash(&mut self.asset_acc, leaf_hash("asset_ref", &asset));
        }
    }
}

fn sync_story_hash_map<K: Ord + Clone>(
    acc: &mut [u8; 32],
    count: &mut usize,
    previous: &mut BTreeMap<K, [u8; 32]>,
    current: BTreeMap<K, [u8; 32]>,
) {
    sync_hash_map(acc, count, previous, current);
}

fn sync_layout_hash_map<K: Ord + Clone>(
    acc: &mut [u8; 32],
    count: &mut usize,
    previous: &mut BTreeMap<K, [u8; 32]>,
    current: BTreeMap<K, [u8; 32]>,
) {
    sync_hash_map(acc, count, previous, current);
}

fn sync_hash_map<K: Ord + Clone>(
    acc: &mut [u8; 32],
    count: &mut usize,
    previous: &mut BTreeMap<K, [u8; 32]>,
    current: BTreeMap<K, [u8; 32]>,
) {
    let old_keys = previous.keys().cloned().collect::<Vec<_>>();
    let mut remove_keys = Vec::new();
    for key in old_keys {
        if current.get(&key) != previous.get(&key) {
            remove_keys.push(key);
        }
    }
    for key in remove_keys {
        if let Some(hash) = previous.remove(&key) {
            remove_component(acc, count, hash);
        }
    }
    for (key, hash) in current {
        if previous.get(&key) != Some(&hash) {
            add_component(acc, count, hash);
            previous.insert(key, hash);
        }
    }
}

fn connection_key(connection: &GraphConnection) -> (u32, usize, u32) {
    (connection.from, connection.from_port, connection.to)
}

fn node_asset_refs(node: &StoryNode) -> Vec<String> {
    let mut refs = AssetRefSet::default();
    match node {
        StoryNode::Scene {
            background,
            music,
            characters,
            ..
        } => {
            refs.push_optional(background);
            refs.push_optional(music);
            collect_character_assets(characters, &mut refs);
        }
        StoryNode::ScenePatch(patch) => collect_scene_patch_asset_refs(patch, &mut refs),
        StoryNode::AudioAction {
            asset: Some(asset), ..
        } => refs.push(asset),
        StoryNode::Generic(event) => collect_event_asset_refs(event, &mut refs),
        _ => {}
    }
    refs.into_vec()
}

fn node_id_from_character_object_id(object_id: &str) -> Option<u32> {
    let mut parts = object_id.split(':');
    (parts.next()? == "node").then_some(())?;
    let node_id = parts.next()?.parse().ok()?;
    (parts.next()? == "character").then_some(())?;
    Some(node_id)
}

fn component_root(domain: &str, acc: &[u8; 32], count: usize) -> String {
    hash_json_hex(domain, &(hex_digest(acc), count))
}

fn add_component(acc: &mut [u8; 32], count: &mut usize, hash: [u8; 32]) {
    xor_hash(acc, hash);
    *count += 1;
}

fn remove_component(acc: &mut [u8; 32], count: &mut usize, hash: [u8; 32]) {
    xor_hash(acc, hash);
    *count = count.saturating_sub(1);
}

fn leaf_hash<T: Serialize>(domain: &str, value: &T) -> [u8; 32] {
    let mut writer = Sha256Writer(Sha256::new());
    writer.0.update(domain.as_bytes());
    writer.0.update([0]);
    serde_json::to_writer(&mut writer, value).expect("serializing fingerprint leaf cannot fail");
    writer.0.finalize().into()
}

fn hash_json_hex<T: Serialize>(domain: &str, value: &T) -> String {
    hex_digest(&leaf_hash(domain, value))
}

fn xor_hash(acc: &mut [u8; 32], hash: [u8; 32]) {
    for (left, right) in acc.iter_mut().zip(hash) {
        *left ^= right;
    }
}

fn hex_digest(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

struct Sha256Writer(Sha256);

impl io::Write for Sha256Writer {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.update(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
