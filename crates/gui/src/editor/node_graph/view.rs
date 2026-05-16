use super::*;
use std::collections::{BTreeMap, BTreeSet};

impl NodeGraph {
    /// Returns the current zoom level.
    #[inline]
    pub fn zoom(&self) -> f32 {
        self.zoom
    }

    /// Sets the zoom level, clamping to valid range.
    pub fn set_zoom(&mut self, zoom: f32) {
        self.zoom = zoom.clamp(ZOOM_MIN, ZOOM_MAX);
    }

    /// Zooms by a delta (positive = zoom in, negative = zoom out).
    pub fn zoom_by(&mut self, delta: f32) {
        self.set_zoom(self.zoom + delta);
    }

    /// Returns the current pan offset.
    #[inline]
    pub fn pan(&self) -> egui::Vec2 {
        self.pan
    }

    /// Adds to the pan offset.
    pub fn pan_by(&mut self, delta: egui::Vec2) {
        self.pan += delta;
    }

    /// Resets pan and zoom to default values.
    pub fn reset_view(&mut self) {
        self.pan = egui::Vec2::ZERO;
        self.zoom = ZOOM_DEFAULT;
    }

    /// Adjusts pan and zoom to show all nodes.
    pub fn zoom_to_fit(&mut self) {
        self.zoom_to_fit_viewport(egui::vec2(800.0, 600.0));
    }

    /// Adjusts pan and zoom to show all nodes in a concrete viewport.
    pub fn zoom_to_fit_viewport(&mut self, viewport: egui::Vec2) {
        if self.is_empty() {
            self.reset_view();
            return;
        }

        let mut min_x = f32::MAX;
        let mut min_y = f32::MAX;
        let mut max_x = f32::MIN;
        let mut max_y = f32::MIN;

        for (_, node, pos) in self.nodes() {
            min_x = min_x.min(pos.x);
            min_y = min_y.min(pos.y);
            max_x = max_x.max(pos.x + NODE_WIDTH);
            max_y = max_y.max(pos.y + node_visual_height(&node));
        }

        let padding = 50.0;
        min_x -= padding;
        min_y -= padding;
        max_x += padding;
        max_y += padding;

        let viewport_width = viewport.x.max(1.0);
        let viewport_height = viewport.y.max(1.0);
        let content_width = max_x - min_x;
        let content_height = max_y - min_y;

        let zoom_x = viewport_width / content_width;
        let zoom_y = viewport_height / content_height;
        let new_zoom = zoom_x.min(zoom_y).clamp(ZOOM_MIN, ZOOM_MAX);

        let center_x = (min_x + max_x) / 2.0;
        let center_y = (min_y + max_y) / 2.0;

        self.zoom = new_zoom;
        self.pan = egui::vec2(
            viewport_width / (2.0 * new_zoom) - center_x,
            viewport_height / (2.0 * new_zoom) - center_y,
        );
    }

    /// Duplicates a node at an offset position.
    pub fn duplicate_node(&mut self, node_id: u32) {
        let Some(node) = self.get_node(node_id).cloned() else {
            return;
        };
        let Some(pos) = self.get_node_pos(node_id) else {
            return;
        };

        let new_pos = egui::pos2(pos.x + 50.0, pos.y + 50.0);
        let new_id = self.add_node(node, new_pos);
        self.set_single_selection(Some(new_id));
    }

    /// Duplicates the current selection as a coherent group.
    ///
    /// Internal connections between selected nodes are recreated in the copy,
    /// while external connections are intentionally left detached so the user
    /// can reconnect the duplicated branch explicitly.
    pub fn duplicate_selected_nodes(&mut self) -> Vec<u32> {
        let selected = self.selected_node_ids();
        if selected.is_empty() {
            return Vec::new();
        }

        let selected_set = selected.iter().copied().collect::<BTreeSet<_>>();
        let mut id_map = BTreeMap::new();
        let mut new_ids = Vec::with_capacity(selected.len());

        for old_id in &selected {
            let Some(node) = self.get_node(*old_id).cloned() else {
                continue;
            };
            let Some(pos) = self.get_node_pos(*old_id) else {
                continue;
            };
            let new_id = self.add_node(node, egui::pos2(pos.x + 50.0, pos.y + 50.0));
            id_map.insert(*old_id, new_id);
            new_ids.push(new_id);
        }

        let internal_connections = self
            .connections()
            .filter(|conn| selected_set.contains(&conn.from) && selected_set.contains(&conn.to))
            .collect::<Vec<_>>();
        for conn in internal_connections {
            let (Some(from), Some(to)) = (id_map.get(&conn.from), id_map.get(&conn.to)) else {
                continue;
            };
            self.connect_port(*from, conn.from_port, *to);
        }

        self.selected_nodes = new_ids.iter().copied().collect();
        self.selected = self
            .selected
            .and_then(|old_primary| id_map.get(&old_primary).copied())
            .or_else(|| new_ids.last().copied());

        self.queue_operation_hint(
            "node_created",
            format!("Duplicated {} selected node(s)", new_ids.len()),
            Some("graph.nodes".to_string()),
            true,
        );
        new_ids
    }
}
