use super::*;

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
        if self.nodes.is_empty() {
            self.reset_view();
            return;
        }

        let mut min_x = f32::MAX;
        let mut min_y = f32::MAX;
        let mut max_x = f32::MIN;
        let mut max_y = f32::MIN;

        for (_, node, pos) in &self.nodes {
            min_x = min_x.min(pos.x);
            min_y = min_y.min(pos.y);
            max_x = max_x.max(pos.x + NODE_WIDTH);
            max_y = max_y.max(pos.y + node_visual_height(node));
        }

        let padding = 50.0;
        min_x -= padding;
        min_y -= padding;
        max_x += padding;
        max_y += padding;

        let viewport_width = 800.0;
        let viewport_height = 600.0;
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
        let Some((_, node, pos)) = self.nodes.iter().find(|(id, _, _)| *id == node_id).cloned()
        else {
            return;
        };

        let new_pos = egui::pos2(pos.x + 50.0, pos.y + 50.0);
        let new_id = self.add_node(node, new_pos);
        self.selected = Some(new_id);
    }
}
