use std::collections::BTreeMap;

use super::super::{
    collect_authoring_asset_refs,
    composer::{self, ComposerSnapshot, LayerOverride, LayeredSceneObject},
    validate_authoring_graph_no_io, AuthoringDocument, AuthoringReportFingerprint, LintIssue,
    StoryNode,
};
use super::AuthoringDocumentDelta;

#[derive(Clone, Debug, Default)]
pub struct AuthoringReadModel {
    node_index: NodeIndex,
    composer_layer_index: ComposerLayerIndex,
    asset_ref_index: AssetRefIndex,
    diagnostics_index: DiagnosticsIndex,
    route_index: RouteIndex,
    preview_index: PreviewIndex,
}

impl AuthoringReadModel {
    pub fn from_document(document: &AuthoringDocument) -> Self {
        let diagnostics = validate_authoring_graph_no_io(&document.graph);
        Self {
            node_index: NodeIndex::from_document(document),
            composer_layer_index: ComposerLayerIndex::from_document(document),
            asset_ref_index: AssetRefIndex::from_document(document),
            diagnostics_index: DiagnosticsIndex::from_issues(diagnostics),
            route_index: RouteIndex::from_document(document),
            preview_index: PreviewIndex::from_document(document),
        }
    }

    pub fn node_ids(&self) -> &[u32] {
        &self.node_index.ids
    }

    pub fn nodes_by_text(&self, query: &str) -> Vec<u32> {
        self.node_index.search(query)
    }

    pub fn composer_layers(&self) -> &[LayeredSceneObject] {
        &self.composer_layer_index.layers
    }

    pub fn composer_layer(&self, object_id: &str) -> Option<&LayeredSceneObject> {
        self.composer_layer_index
            .layers
            .iter()
            .find(|layer| layer.object_id == object_id)
    }

    pub fn visible_composer_objects(&self) -> impl Iterator<Item = &LayeredSceneObject> {
        self.composer_layer_index
            .layers
            .iter()
            .filter(|layer| layer.visible)
    }

    pub fn asset_refs(&self) -> &[String] {
        &self.asset_ref_index.refs
    }

    pub fn diagnostics(&self) -> &[LintIssue] {
        &self.diagnostics_index.issues
    }

    pub fn diagnostics_for_node(&self, node_id: u32) -> Vec<&LintIssue> {
        self.diagnostics_index.by_node(node_id)
    }

    pub fn diagnostics_by_code(&self, code: &str) -> Vec<&LintIssue> {
        self.diagnostics_index.by_code(code)
    }

    pub fn diagnostics_for_target(&self, target_key: &str) -> Vec<&LintIssue> {
        self.diagnostics_index.by_target(target_key)
    }

    pub fn route_order_node_ids(&self) -> &[u32] {
        &self.route_index.route_order_node_ids
    }

    pub fn reachable_node_ids(&self) -> &[u32] {
        &self.route_index.reachable_node_ids
    }

    pub fn unreachable_node_ids(&self) -> &[u32] {
        &self.route_index.unreachable_node_ids
    }

    pub fn reachable_cycle_node_ids(&self) -> &[u32] {
        &self.route_index.reachable_cycle_node_ids
    }

    pub fn preview_data_for_node(&self, node_id: u32) -> Option<&ComposerSnapshot> {
        self.preview_index.snapshots_by_node.get(&node_id)
    }

    pub fn report_stale_state(
        &self,
        report: &AuthoringReportFingerprint,
        current: &AuthoringReportFingerprint,
    ) -> AuthoringReportStaleState {
        AuthoringReportStaleState::compare(report, current)
    }

    pub(crate) fn replace_diagnostics(&mut self, issues: &[LintIssue]) {
        self.diagnostics_index = DiagnosticsIndex::from_issues(issues.to_vec());
    }

    pub(crate) fn update_from_delta(
        &mut self,
        document: &AuthoringDocument,
        delta: &AuthoringDocumentDelta,
    ) {
        match delta {
            AuthoringDocumentDelta::LayerVisibleChanged {
                object_id, after, ..
            }
            | AuthoringDocumentDelta::LayerLockedChanged {
                object_id, after, ..
            } => self
                .composer_layer_index
                .update_layer_override(object_id, *after),
            AuthoringDocumentDelta::BackgroundFitChanged { .. }
            | AuthoringDocumentDelta::BackgroundFitCleared { .. } => {}
            AuthoringDocumentDelta::Graph(_) => {
                self.rebuild_document_projection(document);
            }
            AuthoringDocumentDelta::Reverted { .. } => self.rebuild_document_projection(document),
        }
        if matches!(
            delta,
            AuthoringDocumentDelta::LayerVisibleChanged { .. }
                | AuthoringDocumentDelta::LayerLockedChanged { .. }
        ) {
            self.preview_index = PreviewIndex::from_document(document);
        }
    }

    fn rebuild_document_projection(&mut self, document: &AuthoringDocument) {
        self.node_index = NodeIndex::from_document(document);
        self.composer_layer_index = ComposerLayerIndex::from_document(document);
        self.asset_ref_index = AssetRefIndex::from_document(document);
        self.route_index = RouteIndex::from_document(document);
        self.preview_index = PreviewIndex::from_document(document);
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ComposerLayerIndex {
    layers: Vec<LayeredSceneObject>,
}

impl ComposerLayerIndex {
    fn from_document(document: &AuthoringDocument) -> Self {
        let mut layers = composer::list_layered_objects(&document.graph, None);
        composer::apply_layer_overrides(&mut layers, &document.composer_layer_overrides);
        Self { layers }
    }

    fn update_layer_override(&mut self, object_id: &str, override_state: Option<LayerOverride>) {
        let Some(layer) = self
            .layers
            .iter_mut()
            .find(|layer| layer.object_id == object_id)
        else {
            return;
        };
        let override_state = override_state.unwrap_or_default();
        layer.visible = override_state.visible;
        layer.locked = override_state.locked;
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AssetRefIndex {
    refs: Vec<String>,
}

impl AssetRefIndex {
    fn from_document(document: &AuthoringDocument) -> Self {
        let mut refs = collect_authoring_asset_refs(&document.graph);
        refs.sort();
        refs.dedup();
        Self { refs }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NodeIndex {
    ids: Vec<u32>,
    search_text_by_node: BTreeMap<u32, String>,
}

impl NodeIndex {
    fn from_document(document: &AuthoringDocument) -> Self {
        let mut ids = Vec::new();
        let mut search_text_by_node = BTreeMap::new();
        for (node_id, node, _) in document.graph.nodes() {
            ids.push(*node_id);
            search_text_by_node.insert(*node_id, node_search_text(node));
        }
        ids.sort_unstable();
        Self {
            ids,
            search_text_by_node,
        }
    }

    fn search(&self, query: &str) -> Vec<u32> {
        let needle = query.trim().to_ascii_lowercase();
        if needle.is_empty() {
            return Vec::new();
        }
        self.ids
            .iter()
            .copied()
            .filter(|node_id| {
                self.search_text_by_node
                    .get(node_id)
                    .is_some_and(|text| text.contains(&needle))
            })
            .collect()
    }
}

#[derive(Clone, Debug, Default)]
pub struct DiagnosticsIndex {
    issues: Vec<LintIssue>,
    by_code: BTreeMap<String, Vec<usize>>,
    by_node: BTreeMap<u32, Vec<usize>>,
    by_target: BTreeMap<String, Vec<usize>>,
}

impl DiagnosticsIndex {
    fn from_issues(issues: Vec<LintIssue>) -> Self {
        let mut by_code = BTreeMap::<String, Vec<usize>>::new();
        let mut by_node = BTreeMap::<u32, Vec<usize>>::new();
        let mut by_target = BTreeMap::<String, Vec<usize>>::new();
        for (index, issue) in issues.iter().enumerate() {
            by_code
                .entry(issue.code.label().to_string())
                .or_default()
                .push(index);
            if let Some(node_id) = issue.node_id {
                by_node.entry(node_id).or_default().push(index);
            }
            if let Some(target) = &issue.target {
                by_target
                    .entry(target.stable_key())
                    .or_default()
                    .push(index);
            }
        }
        Self {
            issues,
            by_code,
            by_node,
            by_target,
        }
    }

    fn by_code(&self, code: &str) -> Vec<&LintIssue> {
        self.lookup_indices(&self.by_code, code)
    }

    fn by_node(&self, node_id: u32) -> Vec<&LintIssue> {
        self.by_node
            .get(&node_id)
            .into_iter()
            .flatten()
            .filter_map(|index| self.issues.get(*index))
            .collect()
    }

    fn by_target(&self, target_key: &str) -> Vec<&LintIssue> {
        self.lookup_indices(&self.by_target, target_key)
    }

    fn lookup_indices(&self, index: &BTreeMap<String, Vec<usize>>, key: &str) -> Vec<&LintIssue> {
        index
            .get(key)
            .into_iter()
            .flatten()
            .filter_map(|issue_index| self.issues.get(*issue_index))
            .collect()
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RouteIndex {
    route_order_node_ids: Vec<u32>,
    reachable_node_ids: Vec<u32>,
    unreachable_node_ids: Vec<u32>,
    reachable_cycle_node_ids: Vec<u32>,
}

impl RouteIndex {
    fn from_document(document: &AuthoringDocument) -> Self {
        let start_nodes = document
            .graph
            .nodes()
            .filter_map(|(id, node, _)| matches!(node, StoryNode::Start).then_some(*id))
            .collect::<Vec<_>>();
        let flow = document.graph.flow_analysis(&start_nodes);
        let reachable_node_ids = flow.reachable.iter().copied().collect::<Vec<_>>();
        Self {
            route_order_node_ids: document.graph.script_order_node_ids(),
            reachable_node_ids,
            unreachable_node_ids: flow.unreachable,
            reachable_cycle_node_ids: flow.reachable_cycle_nodes,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
struct PreviewIndex {
    snapshots_by_node: BTreeMap<u32, ComposerSnapshot>,
}

impl PreviewIndex {
    fn from_document(document: &AuthoringDocument) -> Self {
        let mut snapshots_by_node = BTreeMap::new();
        for (node_id, _, _) in document.graph.nodes() {
            let mut snapshot = composer::compose_scene_snapshot(
                &document.graph,
                Some(*node_id),
                None,
                None,
                None,
                None,
            );
            composer::apply_layer_overrides(
                &mut snapshot.objects,
                &document.composer_layer_overrides,
            );
            snapshots_by_node.insert(*node_id, snapshot);
        }
        Self { snapshots_by_node }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AuthoringReportStaleState {
    pub semantic_stale: bool,
    pub layout_stale: bool,
    pub assets_stale: bool,
    pub full_document_stale: bool,
}

impl AuthoringReportStaleState {
    fn compare(report: &AuthoringReportFingerprint, current: &AuthoringReportFingerprint) -> Self {
        Self {
            semantic_stale: report.story_semantic_sha256 != current.story_semantic_sha256,
            layout_stale: report.layout_sha256 != current.layout_sha256,
            assets_stale: report.assets_sha256 != current.assets_sha256,
            full_document_stale: report.full_document_sha256 != current.full_document_sha256,
        }
    }

    pub fn is_stale(self) -> bool {
        self.semantic_stale || self.layout_stale || self.assets_stale || self.full_document_stale
    }
}

fn node_search_text(node: &StoryNode) -> String {
    format!(
        "{} {}",
        node.type_name(),
        serde_json::to_string(node).unwrap_or_default()
    )
    .to_ascii_lowercase()
}
