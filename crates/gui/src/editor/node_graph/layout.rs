use std::collections::{BTreeMap, VecDeque};

use super::*;

const AUTO_LAYOUT_LAYER_VERTICAL_GAP: f32 = 150.0;
const AUTO_LAYOUT_BRANCH_HORIZONTAL_SPACING: f32 = 190.0;
const AUTO_LAYOUT_LAYER_HORIZONTAL_GAP: f32 = 180.0;
const AUTO_LAYOUT_BRANCH_VERTICAL_SPACING: f32 = 118.0;
const AUTO_LAYOUT_BASE_X: f32 = 80.0;
const AUTO_LAYOUT_BASE_Y: f32 = 80.0;
const AUTO_LAYOUT_CENTER_X: f32 = 260.0;
const AUTO_LAYOUT_CENTER_Y: f32 = 220.0;
const AUTO_LAYOUT_LINEAR_WRAP_ROWS_MIN: usize = 8;
const AUTO_LAYOUT_LINEAR_ROW_SPACING: f32 = 120.0;
const AUTO_LAYOUT_LINEAR_COLUMN_GAP: f32 = 80.0;
const AUTO_LAYOUT_LINEAR_WRAP_COLUMNS_MIN: usize = 8;
const AUTO_LAYOUT_LINEAR_COLUMN_SPACING: f32 = 250.0;
const AUTO_LAYOUT_LINEAR_ROW_GAP: f32 = 54.0;
const AUTO_LAYOUT_OVERLAP_PAD_X: f32 = 34.0;
const AUTO_LAYOUT_OVERLAP_PAD_Y: f32 = 24.0;
const AUTO_LAYOUT_OVERLAP_MAX_PASSES: usize = 48;

impl NodeGraph {
    /// Applies a deterministic hierarchical layout using the graph's selected orientation.
    /// Contracts:
    /// - Vertical flow places depth top-to-bottom and stacks branches left-to-right.
    /// - Horizontal flow places depth left-to-right and stacks branches top-to-bottom.
    /// - Very linear graphs wrap after a readable run instead of becoming a single long line.
    /// - Output is deterministic for the same graph topology and orientation.
    pub fn auto_layout_hierarchical(&mut self) -> bool {
        self.auto_layout_hierarchical_with_orientation(self.layout_orientation)
    }

    pub fn auto_layout_hierarchical_with_orientation(
        &mut self,
        orientation: GraphLayoutOrientation,
    ) -> bool {
        if self.is_empty() {
            return false;
        }

        let grouped = self.layout_layers();
        let max_nodes_per_layer = grouped.values().map(Vec::len).max().unwrap_or(0);
        let mostly_linear = max_nodes_per_layer <= 1 && self.len() >= 6;
        let mut changed = match (mostly_linear, orientation) {
            (true, GraphLayoutOrientation::Vertical) => {
                self.apply_wrapped_vertical_linear_layout(&grouped)
            }
            (true, GraphLayoutOrientation::Horizontal) => {
                self.apply_wrapped_horizontal_linear_layout(&grouped)
            }
            (false, GraphLayoutOrientation::Vertical) => {
                self.apply_vertical_hierarchy_layout(&grouped)
            }
            (false, GraphLayoutOrientation::Horizontal) => {
                self.apply_horizontal_hierarchy_layout(&grouped)
            }
        };

        if self.resolve_layout_overlaps() {
            changed = true;
        }

        changed
    }

    fn layout_layers(&self) -> BTreeMap<usize, Vec<u32>> {
        let mut roots: Vec<u32> = self
            .nodes()
            .filter(|(_, node, _)| matches!(node, StoryNode::Start))
            .map(|(id, _, _)| id)
            .collect();
        roots.sort_unstable();
        if roots.is_empty() {
            let mut fallback: Vec<u32> = self.nodes().map(|(id, _, _)| id).collect();
            fallback.sort_unstable();
            if let Some(first) = fallback.first().copied() {
                roots.push(first);
            }
        }

        let mut layers: BTreeMap<u32, usize> = BTreeMap::new();
        let mut queue = VecDeque::new();
        for root in roots {
            layers.insert(root, 0);
            queue.push_back(root);
        }

        while let Some(node_id) = queue.pop_front() {
            let layer = layers.get(&node_id).copied().unwrap_or(0);
            let mut outgoing: Vec<GraphConnection> = self
                .connections()
                .filter(|conn| conn.from == node_id)
                .collect();
            outgoing.sort_by_key(|conn| (conn.from_port, conn.to));
            for conn in &outgoing {
                let candidate = layer.saturating_add(1);
                let update = match layers.get(&conn.to) {
                    Some(existing) => candidate < *existing,
                    None => true,
                };
                if update {
                    layers.insert(conn.to, candidate);
                    queue.push_back(conn.to);
                }
            }
        }

        let mut max_layer = layers.values().copied().max().unwrap_or(0);
        let mut missing: Vec<u32> = self
            .nodes()
            .map(|(id, _, _)| id)
            .filter(|id| !layers.contains_key(id))
            .collect();
        missing.sort_unstable();
        for node_id in missing {
            max_layer = max_layer.saturating_add(1);
            layers.insert(node_id, max_layer);
        }

        let mut grouped: BTreeMap<usize, Vec<u32>> = BTreeMap::new();
        for (node_id, layer) in layers {
            grouped.entry(layer).or_default().push(node_id);
        }
        for ids in grouped.values_mut() {
            ids.sort_unstable();
        }
        grouped
    }

    fn apply_wrapped_vertical_linear_layout(
        &mut self,
        grouped: &BTreeMap<usize, Vec<u32>>,
    ) -> bool {
        let ordered = ordered_layer_nodes(grouped);
        if ordered.is_empty() {
            return false;
        }

        let wrap_rows = ordered.len().clamp(1, AUTO_LAYOUT_LINEAR_WRAP_ROWS_MIN);
        let column_count = ordered.len().div_ceil(wrap_rows);
        let mut column_widths = vec![NODE_WIDTH; column_count];
        for (index, node_id) in ordered.iter().copied().enumerate() {
            let col = index / wrap_rows;
            if let Some(node) = self.get_node(node_id) {
                column_widths[col] = column_widths[col].max(node_visual_width(node));
            }
        }

        let mut column_x = Vec::with_capacity(column_count);
        let mut cursor_x = AUTO_LAYOUT_CENTER_X;
        for width in &column_widths {
            column_x.push(cursor_x);
            cursor_x += *width + AUTO_LAYOUT_LINEAR_COLUMN_GAP;
        }

        let mut changed = false;
        for (index, node_id) in ordered.into_iter().enumerate() {
            let col = index / wrap_rows;
            let row = index % wrap_rows;
            let x = column_x[col];
            let y = AUTO_LAYOUT_BASE_Y + (row as f32) * AUTO_LAYOUT_LINEAR_ROW_SPACING;
            if self.set_node_pos(node_id, egui::pos2(x, y)) {
                changed = true;
            }
        }
        changed
    }

    fn apply_wrapped_horizontal_linear_layout(
        &mut self,
        grouped: &BTreeMap<usize, Vec<u32>>,
    ) -> bool {
        let ordered = ordered_layer_nodes(grouped);
        if ordered.is_empty() {
            return false;
        }

        let wrap_columns = ordered.len().clamp(1, AUTO_LAYOUT_LINEAR_WRAP_COLUMNS_MIN);
        let row_count = ordered.len().div_ceil(wrap_columns);
        let mut row_heights = vec![NODE_HEIGHT; row_count];
        for (index, node_id) in ordered.iter().copied().enumerate() {
            let row = index / wrap_columns;
            if let Some(node) = self.get_node(node_id) {
                row_heights[row] = row_heights[row].max(node_visual_height(node));
            }
        }

        let mut row_y = Vec::with_capacity(row_count);
        let mut cursor_y = AUTO_LAYOUT_CENTER_Y;
        for height in &row_heights {
            row_y.push(cursor_y);
            cursor_y += *height + AUTO_LAYOUT_LINEAR_ROW_GAP;
        }

        let mut changed = false;
        for (index, node_id) in ordered.into_iter().enumerate() {
            let row = index / wrap_columns;
            let col = index % wrap_columns;
            let x = AUTO_LAYOUT_BASE_X + (col as f32) * AUTO_LAYOUT_LINEAR_COLUMN_SPACING;
            let y = row_y[row];
            if self.set_node_pos(node_id, egui::pos2(x, y)) {
                changed = true;
            }
        }
        changed
    }

    fn apply_vertical_hierarchy_layout(&mut self, grouped: &BTreeMap<usize, Vec<u32>>) -> bool {
        let mut changed = false;
        let mut assigned_x: BTreeMap<u32, f32> = BTreeMap::new();
        for (layer, ids) in grouped {
            let mut ordered = ids.clone();
            ordered.sort_by(|a, b| {
                let ax = self.estimated_parent_center_x(*a, &assigned_x);
                let bx = self.estimated_parent_center_x(*b, &assigned_x);
                ax.partial_cmp(&bx)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| a.cmp(b))
            });

            let widths = ordered
                .iter()
                .map(|node_id| {
                    self.get_node(*node_id)
                        .map_or(NODE_WIDTH, node_visual_width)
                })
                .collect::<Vec<_>>();
            let total_width = widths.iter().sum::<f32>()
                + (ordered.len().saturating_sub(1) as f32) * AUTO_LAYOUT_BRANCH_HORIZONTAL_SPACING;
            let mut cursor_x = AUTO_LAYOUT_CENTER_X - total_width * 0.5;
            let y = AUTO_LAYOUT_BASE_Y + (*layer as f32) * AUTO_LAYOUT_LAYER_VERTICAL_GAP;

            for (index, node_id) in ordered.into_iter().enumerate() {
                let width = widths[index];
                let x = cursor_x;
                assigned_x.insert(node_id, x + width * 0.5);
                if self.set_node_pos(node_id, egui::pos2(x, y)) {
                    changed = true;
                }
                cursor_x += width + AUTO_LAYOUT_BRANCH_HORIZONTAL_SPACING;
            }
        }
        changed
    }

    fn apply_horizontal_hierarchy_layout(&mut self, grouped: &BTreeMap<usize, Vec<u32>>) -> bool {
        let mut changed = false;
        let mut assigned_y: BTreeMap<u32, f32> = BTreeMap::new();
        let mut layer_x: BTreeMap<usize, f32> = BTreeMap::new();
        let mut cursor_x = AUTO_LAYOUT_BASE_X;
        for (layer, ids) in grouped {
            layer_x.insert(*layer, cursor_x);
            let layer_width = ids
                .iter()
                .filter_map(|node_id| self.get_node(*node_id))
                .map(node_visual_width)
                .fold(NODE_WIDTH, f32::max);
            cursor_x += layer_width + AUTO_LAYOUT_LAYER_HORIZONTAL_GAP;
        }
        for (layer, ids) in grouped {
            let mut ordered = ids.clone();
            ordered.sort_by(|a, b| {
                let ay = self.estimated_horizontal_child_center_y(*a, &assigned_y);
                let by = self.estimated_horizontal_child_center_y(*b, &assigned_y);
                ay.partial_cmp(&by)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| a.cmp(b))
            });

            let heights = ordered
                .iter()
                .map(|node_id| {
                    self.get_node(*node_id)
                        .map_or(NODE_HEIGHT, node_visual_height)
                })
                .collect::<Vec<_>>();
            let total_height = heights.iter().sum::<f32>()
                + (ordered.len().saturating_sub(1) as f32) * AUTO_LAYOUT_BRANCH_VERTICAL_SPACING;
            let mut cursor_y = AUTO_LAYOUT_CENTER_Y - total_height * 0.5;
            let x = layer_x.get(layer).copied().unwrap_or(AUTO_LAYOUT_BASE_X);

            for (index, node_id) in ordered.into_iter().enumerate() {
                let height = heights[index];
                let y = cursor_y;
                assigned_y.insert(node_id, y + height * 0.5);
                if self.set_node_pos(node_id, egui::pos2(x, y)) {
                    changed = true;
                }
                cursor_y += height + AUTO_LAYOUT_BRANCH_VERTICAL_SPACING;
            }
        }
        changed
    }

    fn estimated_parent_center_x(&self, node_id: u32, assigned_x: &BTreeMap<u32, f32>) -> f32 {
        let mut sum = 0.0f32;
        let mut count = 0usize;
        for conn in self.connections().filter(|conn| conn.to == node_id) {
            if let Some(x) = assigned_x.get(&conn.from) {
                sum += *x;
                count += 1;
            }
        }
        if count == 0 {
            return node_id as f32;
        }
        sum / (count as f32)
    }

    fn estimated_horizontal_child_center_y(
        &self,
        node_id: u32,
        assigned_y: &BTreeMap<u32, f32>,
    ) -> f32 {
        let mut sum = 0.0f32;
        let mut count = 0usize;
        for conn in self.connections().filter(|conn| conn.to == node_id) {
            if let Some(y) = assigned_y.get(&conn.from) {
                sum += *y + self.horizontal_route_offset(&conn);
                count += 1;
            }
        }
        if count == 0 {
            return node_id as f32;
        }
        sum / (count as f32)
    }

    fn horizontal_route_offset(&self, conn: &GraphConnection) -> f32 {
        match self.get_node(conn.from) {
            Some(StoryNode::Choice { options, .. }) => {
                let route_count = (options.len() + 1).max(1) as f32;
                let route_index = conn.from_port.min(options.len()) as f32;
                (route_index - (route_count - 1.0) * 0.5) * AUTO_LAYOUT_BRANCH_VERTICAL_SPACING
            }
            Some(StoryNode::JumpIf { .. }) => {
                let route_index = conn.from_port.min(1) as f32;
                (route_index - 0.5) * AUTO_LAYOUT_BRANCH_VERTICAL_SPACING
            }
            _ => 0.0,
        }
    }

    fn resolve_layout_overlaps(&mut self) -> bool {
        if self.len() < 2 {
            return false;
        }

        let mut changed = false;
        for _ in 0..AUTO_LAYOUT_OVERLAP_MAX_PASSES {
            let mut pass_changed = false;
            let nodes = self.nodes().collect::<Vec<_>>();
            let len = nodes.len();
            for i in 0..len {
                for j in (i + 1)..len {
                    let (id_a, node_a, pos_a) = &nodes[i];
                    let (id_b, node_b, pos_b) = &nodes[j];
                    let half_w_a = (node_visual_width(node_a) + AUTO_LAYOUT_OVERLAP_PAD_X) * 0.5;
                    let half_w_b = (node_visual_width(node_b) + AUTO_LAYOUT_OVERLAP_PAD_X) * 0.5;
                    let half_h_a = (node_visual_height(node_a) + AUTO_LAYOUT_OVERLAP_PAD_Y) * 0.5;
                    let half_h_b = (node_visual_height(node_b) + AUTO_LAYOUT_OVERLAP_PAD_Y) * 0.5;
                    let dx = pos_b.x - pos_a.x;
                    let dy = pos_b.y - pos_a.y;
                    let overlap_x = (half_w_a + half_w_b) - dx.abs();
                    let overlap_y = (half_h_a + half_h_b) - dy.abs();
                    if overlap_x <= 0.0 || overlap_y <= 0.0 {
                        continue;
                    }

                    if overlap_y <= overlap_x {
                        let direction = if dy >= 0.0 { 1.0 } else { -1.0 };
                        let shift = (overlap_y * 0.5) + 1.0;
                        self.set_node_pos(*id_a, egui::pos2(pos_a.x, pos_a.y - shift * direction));
                        self.set_node_pos(*id_b, egui::pos2(pos_b.x, pos_b.y + shift * direction));
                    } else {
                        let direction = if dx >= 0.0 { 1.0 } else { -1.0 };
                        let shift = (overlap_x * 0.5) + 1.0;
                        self.set_node_pos(*id_a, egui::pos2(pos_a.x - shift * direction, pos_a.y));
                        self.set_node_pos(*id_b, egui::pos2(pos_b.x + shift * direction, pos_b.y));
                    }
                    pass_changed = true;
                    changed = true;
                }
            }
            if !pass_changed {
                break;
            }
        }
        changed
    }
}

fn ordered_layer_nodes(grouped: &BTreeMap<usize, Vec<u32>>) -> Vec<u32> {
    let mut ordered = Vec::new();
    for ids in grouped.values() {
        ordered.extend(ids.iter().copied());
    }
    ordered
}
