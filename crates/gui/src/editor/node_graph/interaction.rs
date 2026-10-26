use super::*;

impl NodeGraph {
    pub fn toggle_multi_selection(&mut self, node_id: u32) {
        if !self.selected_nodes.insert(node_id) {
            self.selected_nodes.remove(&node_id);
            if self.selected == Some(node_id) {
                self.selected = self.selected_nodes.iter().next_back().copied();
            }
            return;
        }
        self.selected = Some(node_id);
    }

    pub fn set_single_selection(&mut self, node_id: Option<u32>) {
        self.selected = node_id;
        self.selected_nodes.clear();
        if let Some(node_id) = node_id {
            self.selected_nodes.insert(node_id);
        }
    }

    pub fn select_nodes_in_rect(&mut self, rect: egui::Rect, additive: bool) -> usize {
        let selected = self
            .visible_nodes()
            .filter_map(|(id, node, pos)| {
                let node_rect = egui::Rect::from_min_size(
                    pos,
                    egui::vec2(node_visual_width(&node), node_visual_height(&node)),
                );
                rect.intersects(node_rect).then_some(id)
            })
            .collect::<Vec<_>>();
        if !additive {
            self.selected_nodes.clear();
        }
        for node_id in &selected {
            self.selected_nodes.insert(*node_id);
        }
        self.selected = selected
            .last()
            .copied()
            .or_else(|| additive.then_some(self.selected).flatten());
        selected.len()
    }

    pub fn reconcile_view_after_document_change(&mut self) {
        let graph = self.authoring_graph();
        let selected = self.selected.filter(|id| graph.get_node(*id).is_some());
        let retained = self
            .selected_nodes
            .iter()
            .copied()
            .filter(|id| graph.get_node(*id).is_some())
            .collect();
        self.selected_nodes = retained;
        self.selected = selected.or_else(|| self.selected_nodes.iter().next_back().copied());
        self.editing = None;
        self.clear_transient_interaction();
    }

    pub fn clear_transient_interaction(&mut self) {
        self.view.drag_positions.clear();
        self.dragging_node = None;
        self.connecting_from = None;
        self.connecting_sticky = false;
        self.marquee_start = None;
        self.marquee_current = None;
        self.context_menu = None;
    }

    pub fn has_active_interaction(&self) -> bool {
        self.dragging_node.is_some()
            || self.connecting_from.is_some()
            || self.marquee_start.is_some()
            || self.context_menu.is_some()
            || self.editing.is_some()
    }

    pub fn queue_operation_hint(
        &mut self,
        kind: impl Into<String>,
        details: impl Into<String>,
        field_path: Option<String>,
        push_undo_snapshot: bool,
    ) {
        self.queue_operation_hint_with_values(
            kind,
            details,
            field_path,
            None,
            None,
            push_undo_snapshot,
        );
    }

    pub fn queue_operation_hint_with_values(
        &mut self,
        kind: impl Into<String>,
        details: impl Into<String>,
        field_path: Option<String>,
        before_value: Option<String>,
        after_value: Option<String>,
        push_undo_snapshot: bool,
    ) {
        self.operation_hint = Some(GraphOperationHint {
            kind: kind.into(),
            details: details.into(),
            field_path,
            before_value,
            after_value,
            push_undo_snapshot,
        });
    }

    pub fn operation_hint_pushes_undo(&self) -> bool {
        self.operation_hint
            .as_ref()
            .map(|hint| hint.push_undo_snapshot)
            .unwrap_or(true)
    }

    pub fn take_operation_hint(&mut self) -> Option<GraphOperationHint> {
        self.operation_hint.take()
    }

    pub fn clear_operation_hint(&mut self) {
        self.operation_hint = None;
    }

    pub fn selected_node_ids(&self) -> Vec<u32> {
        if self.selected_nodes.is_empty() {
            self.selected.into_iter().collect()
        } else {
            self.selected_nodes.iter().copied().collect()
        }
    }
}
