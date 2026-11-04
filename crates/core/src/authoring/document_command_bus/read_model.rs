use std::collections::BTreeMap;
use std::sync::{Arc, OnceLock};

use super::super::{
    collect_authoring_asset_refs,
    composer::{self, ComposerSnapshot, LayerOverride, LayeredSceneObject},
    validate_authoring_graph_no_io, AuthoringDocument, AuthoringReportFingerprint, LintIssue,
    NodeGraph, StoryNode,
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
        self.preview_index.get(node_id)
    }

    pub fn report_stale_state(
        &self,
        report: &AuthoringReportFingerprint,
        current: &AuthoringReportFingerprint,
    ) -> AuthoringReportStaleState {
        AuthoringReportStaleState::compare(report, current)
    }

    pub(crate) fn replace_diagnostics(&mut self, issues: Vec<LintIssue>) -> Vec<LintIssue> {
        std::mem::replace(
            &mut self.diagnostics_index,
            DiagnosticsIndex::from_issues(issues),
        )
        .issues
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
            | AuthoringDocumentDelta::BackgroundFitCleared { .. }
            | AuthoringDocumentDelta::NodesMoved { .. } => {}
            AuthoringDocumentDelta::Graph(_) | AuthoringDocumentDelta::GraphReplaced { .. } => {
                self.rebuild_document_projection(document);
            }
            AuthoringDocumentDelta::ComposerOverridesReset { .. } => {
                self.composer_layer_index = ComposerLayerIndex::from_document(document);
                self.preview_index = PreviewIndex::from_document(document);
            }
            AuthoringDocumentDelta::Reverted { reverted }
                if matches!(reverted.as_ref(), AuthoringDocumentDelta::NodesMoved { .. }) => {}
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

#[derive(Clone, Debug, Default)]
struct PreviewIndex {
    generation: Arc<PreviewGeneration>,
}

#[derive(Debug, Default)]
struct PreviewGeneration {
    graph: NodeGraph,
    layer_overrides: BTreeMap<String, LayerOverride>,
    snapshots_by_node: BTreeMap<u32, OnceLock<Box<ComposerSnapshot>>>,
}

impl PreviewIndex {
    fn from_document(document: &AuthoringDocument) -> Self {
        let snapshots_by_node = document
            .graph
            .nodes()
            .map(|(node_id, _, _)| (*node_id, OnceLock::new()))
            .collect();
        Self {
            generation: Arc::new(PreviewGeneration {
                graph: document.graph.clone(),
                layer_overrides: document.composer_layer_overrides.clone(),
                snapshots_by_node,
            }),
        }
    }

    fn get(&self, node_id: u32) -> Option<&ComposerSnapshot> {
        let slot = self.generation.snapshots_by_node.get(&node_id)?;
        let snapshot = slot.get_or_init(|| {
            let mut snapshot = composer::compose_scene_snapshot(
                &self.generation.graph,
                Some(node_id),
                None,
                None,
                None,
                None,
            );
            composer::apply_layer_overrides(
                &mut snapshot.objects,
                &self.generation.layer_overrides,
            );
            Box::new(snapshot)
        });
        Some(snapshot.as_ref())
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

#[cfg(test)]
mod lazy_preview_tests {
    use super::*;
    use crate::authoring::{
        AuthoringCommand, AuthoringDocumentCommand, AuthoringDocumentSession, AuthoringPosition,
    };

    fn scene(background: &str) -> StoryNode {
        StoryNode::Scene {
            profile: None,
            background: Some(background.into()),
            music: None,
            characters: Vec::new(),
        }
    }

    #[test]
    fn snapshots_are_lazy_shared_and_equivalent_to_eager_projection() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<AuthoringReadModel>();
        let mut graph = NodeGraph::new();
        for id in 0..32 {
            graph.add_node(scene(&format!("bg/{id}.png")), AuthoringPosition::default());
        }
        let document = AuthoringDocument::new(graph);
        let model = AuthoringReadModel::from_document(&document);
        let clone = model.clone();
        assert!(Arc::ptr_eq(
            &model.preview_index.generation,
            &clone.preview_index.generation
        ));
        assert!(model
            .preview_index
            .generation
            .snapshots_by_node
            .values()
            .all(|entry| entry.get().is_none()));
        assert!(model.preview_data_for_node(u32::MAX).is_none());
        std::thread::scope(|scope| {
            for _ in 0..4 {
                let model = &model;
                scope.spawn(move || {
                    let snapshot = model.preview_data_for_node(2).unwrap();
                    let expected = composer::compose_scene_snapshot(
                        &model.preview_index.generation.graph,
                        Some(2),
                        None,
                        None,
                        None,
                        None,
                    );
                    assert_eq!(snapshot, &expected);
                });
            }
        });
        assert_eq!(
            model
                .preview_index
                .generation
                .snapshots_by_node
                .values()
                .filter(|entry| entry.get().is_some())
                .count(),
            1
        );
        assert!(std::ptr::eq(
            model.preview_data_for_node(2).unwrap(),
            clone.preview_data_for_node(2).unwrap()
        ));
    }

    #[test]
    fn mutations_invalidate_generation_without_changing_cloned_read_model() {
        let mut graph = NodeGraph::new();
        let id = graph.add_node(scene("before.png"), AuthoringPosition::default());
        let mut session = AuthoringDocumentSession::new(AuthoringDocument::new(graph));
        let old = session.read_model().clone();
        let object_id = old.preview_data_for_node(id).unwrap().objects[0]
            .object_id
            .clone();
        session
            .apply(AuthoringDocumentCommand::SetLayerVisible {
                object_id: object_id.clone(),
                visible: false,
            })
            .unwrap();
        assert!(old.preview_data_for_node(id).unwrap().objects[0].visible);
        assert!(
            !session
                .read_model()
                .preview_data_for_node(id)
                .unwrap()
                .objects[0]
                .visible
        );
        session.apply(AuthoringDocumentCommand::UndoLast).unwrap();
        assert!(
            session
                .read_model()
                .preview_data_for_node(id)
                .unwrap()
                .objects[0]
                .visible
        );
        session
            .apply(AuthoringDocumentCommand::Graph(
                AuthoringCommand::EditNode {
                    node_id: id,
                    replacement: scene("after.png"),
                },
            ))
            .unwrap();
        assert_eq!(
            session
                .read_model()
                .preview_data_for_node(id)
                .unwrap()
                .objects[0]
                .asset_path
                .as_deref(),
            Some("after.png")
        );
        assert_eq!(
            old.preview_data_for_node(id).unwrap().objects[0]
                .asset_path
                .as_deref(),
            Some("before.png")
        );
    }
}
