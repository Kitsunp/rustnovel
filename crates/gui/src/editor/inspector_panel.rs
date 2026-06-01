//! Inspector panel for the editor workbench.
//!
//! Displays properties of selected nodes and entities.

use crate::editor::NodeGraph;
use eframe::egui;
use visual_novel_engine::SceneState;

use super::{AssetFieldTarget, AssetImportKind};

#[derive(Clone, Debug, PartialEq)]
pub enum InspectorAction {
    PreviewAudio {
        channel: String,
        path: String,
        volume: Option<f32>,
        loop_playback: bool,
    },
    StopAudio {
        channel: String,
    },
    ImportAssetForNode {
        node_id: u32,
        kind: AssetImportKind,
        target: AssetFieldTarget,
    },
}

/// Inspector panel widget.
pub struct InspectorPanel<'a> {
    scene: &'a SceneState,
    graph: &'a mut NodeGraph,
    selected_node: Option<u32>,
    selected_entity: Option<u32>,
}

impl<'a> InspectorPanel<'a> {
    pub fn new(
        scene: &'a SceneState,
        graph: &'a mut NodeGraph,
        selected_node: Option<u32>,
        selected_entity: Option<u32>,
    ) -> Self {
        Self {
            scene,
            graph,
            selected_node,
            selected_entity,
        }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) -> Option<InspectorAction> {
        ui.heading("Inspector");
        ui.separator();

        let mut action = None;
        egui::ScrollArea::vertical()
            .id_source("inspector_panel_main_scroll")
            .show(ui, |ui| {
                ui.collapsing("Selected Node", |ui| {
                    action = self.render_node_editor(ui);
                });

                ui.separator();

                ui.collapsing("Selected Entity", |ui| {
                    self.render_entity_info(ui);
                });

                ui.separator();
                ui.collapsing("Graph Summary", |ui| {
                    for line in graph_summary_lines(self.graph) {
                        ui.label(line);
                    }
                });
            });
        action
    }
}

pub fn graph_summary_lines(graph: &NodeGraph) -> Vec<String> {
    let selected = graph.selected_node_ids();
    let selected_label = match selected.as_slice() {
        [] => "none".to_string(),
        [id] => id.to_string(),
        ids => format!("{} nodes ({})", ids.len(), join_node_ids(ids)),
    };
    let fragment_label = graph.active_fragment().unwrap_or("<root>");
    vec![
        format!("Nodes: {}", graph.len()),
        format!("Connections: {}", graph.connection_count()),
        format!("Selected: {selected_label}"),
        format!("Active fragment: {fragment_label}"),
        format!("Fragments: {}", graph.fragments().len()),
    ]
}

fn join_node_ids(ids: &[u32]) -> String {
    ids.iter()
        .map(u32::to_string)
        .collect::<Vec<_>>()
        .join(", ")
}

mod entity_info;
#[path = "inspector_panel_node_editor.rs"]
pub mod node_editor;
