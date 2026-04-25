use std::collections::{BTreeMap, VecDeque};

use super::*;

const AUTO_LAYOUT_LAYER_VERTICAL_GAP: f32 = 84.0;
const AUTO_LAYOUT_LAYER_HORIZONTAL_SPACING: f32 = 230.0;
const AUTO_LAYOUT_CENTER_X: f32 = 420.0;
const AUTO_LAYOUT_BASE_Y: f32 = 80.0;
const AUTO_LAYOUT_LINEAR_WRAP_ROWS_MIN: usize = 8;
const AUTO_LAYOUT_LINEAR_WRAP_ROWS_MAX: usize = 16;
const AUTO_LAYOUT_LINEAR_COLUMN_SPACING: f32 = 250.0;
const AUTO_LAYOUT_LINEAR_ROW_GAP: f32 = 54.0;
const AUTO_LAYOUT_LINEAR_ZIGZAG_X: f32 = 56.0;
const AUTO_LAYOUT_OVERLAP_PAD_X: f32 = 34.0;
const AUTO_LAYOUT_OVERLAP_PAD_Y: f32 = 24.0;
const AUTO_LAYOUT_OVERLAP_MAX_PASSES: usize = 48;

impl NodeGraph {
    /// Applies a deterministic hierarchical layout favoring vertical flow.
    ///
    /// Contracts:
    /// - Branches are distributed horizontally inside each depth layer.
    /// - Very linear graphs are wrapped into columns (avoid single straight line).
    /// - Output is deterministic for the same graph topology.
    pub fn auto_layout_hierarchical(&mut self) -> bool {
        if self.nodes.is_empty() {
            return false;
        }

        let mut roots: Vec<u32> = self
            .nodes
            .iter()
            .filter(|(_, node, _)| matches!(node, StoryNode::Start))
            .map(|(id, _, _)| *id)
            .collect();
        roots.sort_unstable();
        if roots.is_empty() {
            let mut fallback: Vec<u32> = self.nodes.iter().map(|(id, _, _)| *id).collect();
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
            let mut outgoing: Vec<&GraphConnection> = self
                .connections
                .iter()
                .filter(|connection| connection.from == node_id)
                .collect();
            outgoing.sort_by_key(|connection| (connection.from_port, connection.to));

            for connection in outgoing {
                let candidate = layer.saturating_add(1);
                let update = match layers.get(&connection.to) {
                    Some(existing) => candidate < *existing,
                    None => true,
                };
                if update {
                    layers.insert(connection.to, candidate);
                    queue.push_back(connection.to);
                }
            }
        }

        let mut max_layer = layers.values().copied().max().unwrap_or(0);
        let mut missing: Vec<u32> = self
            .nodes
            .iter()
            .map(|(id, _, _)| *id)
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

        let max_nodes_per_layer = grouped.values().map(Vec::len).max().unwrap_or(0);
        let mostly_linear = max_nodes_per_layer <= 1 && self.nodes.len() >= 6;

        let mut changed = if mostly_linear {
            self.apply_wrapped_linear_layout(&grouped)
        } else {
            self.apply_vertical_hierarchy_layout(&grouped)
        };
        if self.resolve_layout_overlaps() {
            changed = true;
        }
        if changed {
            self.modified = true;
        }
        changed
    }

    fn apply_wrapped_linear_layout(&mut self, grouped: &BTreeMap<usize, Vec<u32>>) -> bool {
        let mut ordered = Vec::new();
        for ids in grouped.values() {
            ordered.extend(ids.iter().copied());
        }
        if ordered.is_empty() {
            return false;
        }

        let wrap_rows = ordered.len().clamp(
            AUTO_LAYOUT_LINEAR_WRAP_ROWS_MIN,
            AUTO_LAYOUT_LINEAR_WRAP_ROWS_MAX,
        );

        let mut row_heights = vec![NODE_HEIGHT; wrap_rows];
        for (index, node_id) in ordered.iter().copied().enumerate() {
            let col = index / wrap_rows;
            let raw_row = index % wrap_rows;
            let row = if col % 2 == 0 {
                raw_row
            } else {
                wrap_rows - 1 - raw_row
            };
            if let Some(node) = self.get_node(node_id) {
                row_heights[row] = row_heights[row].max(node_visual_height(node));
            }
        }
        let mut row_y = Vec::with_capacity(wrap_rows);
        let mut cursor_y = AUTO_LAYOUT_BASE_Y;
        for height in &row_heights {
            row_y.push(cursor_y);
            cursor_y += *height + AUTO_LAYOUT_LINEAR_ROW_GAP;
        }

        let mut changed = false;
        for (index, node_id) in ordered.into_iter().enumerate() {
            let col = index / wrap_rows;
            let raw_row = index % wrap_rows;
            let row = if col % 2 == 0 {
                raw_row
            } else {
                wrap_rows - 1 - raw_row
            };
            let zigzag = match index % 3 {
                0 => -AUTO_LAYOUT_LINEAR_ZIGZAG_X,
                1 => 0.0,
                _ => AUTO_LAYOUT_LINEAR_ZIGZAG_X,
            };
            let x =
                AUTO_LAYOUT_CENTER_X + (col as f32) * AUTO_LAYOUT_LINEAR_COLUMN_SPACING + zigzag;
            let y = row_y[row];
            if let Some(pos) = self.get_node_pos_mut(node_id) {
                if (pos.x - x).abs() > f32::EPSILON || (pos.y - y).abs() > f32::EPSILON {
                    *pos = egui::pos2(x, y);
                    changed = true;
                }
            }
        }
        changed
    }

    fn apply_vertical_hierarchy_layout(&mut self, grouped: &BTreeMap<usize, Vec<u32>>) -> bool {
        let mut changed = false;
        let mut assigned_x: BTreeMap<u32, f32> = BTreeMap::new();
        let mut layer_y: BTreeMap<usize, f32> = BTreeMap::new();
        let mut cursor_y = AUTO_LAYOUT_BASE_Y;
        for (layer, ids) in grouped {
            let max_height = ids
                .iter()
                .filter_map(|node_id| self.get_node(*node_id))
                .map(node_visual_height)
                .fold(NODE_HEIGHT, f32::max);
            layer_y.insert(*layer, cursor_y);
            cursor_y += max_height + AUTO_LAYOUT_LAYER_VERTICAL_GAP;
        }

        for (layer, ids) in grouped {
            let mut ordered = ids.clone();
            ordered.sort_by(|a, b| {
                let ax = self.estimated_parent_center_x(*a, &assigned_x);
                let bx = self.estimated_parent_center_x(*b, &assigned_x);
                ax.partial_cmp(&bx)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| a.cmp(b))
            });

            let total_width =
                (ordered.len().saturating_sub(1) as f32) * AUTO_LAYOUT_LAYER_HORIZONTAL_SPACING;
            let start_x = AUTO_LAYOUT_CENTER_X - (total_width * 0.5);
            let y = layer_y.get(layer).copied().unwrap_or(AUTO_LAYOUT_BASE_Y);
            for (index, node_id) in ordered.into_iter().enumerate() {
                let x = start_x + (index as f32) * AUTO_LAYOUT_LAYER_HORIZONTAL_SPACING;
                assigned_x.insert(node_id, x);
                if let Some(pos) = self.get_node_pos_mut(node_id) {
                    if (pos.x - x).abs() > f32::EPSILON || (pos.y - y).abs() > f32::EPSILON {
                        *pos = egui::pos2(x, y);
                        changed = true;
                    }
                }
            }
        }
        changed
    }

    fn estimated_parent_center_x(&self, node_id: u32, assigned_x: &BTreeMap<u32, f32>) -> f32 {
        let mut sum = 0.0f32;
        let mut count = 0usize;
        for connection in self.connections.iter().filter(|conn| conn.to == node_id) {
            if let Some(x) = assigned_x.get(&connection.from) {
                sum += *x;
                count += 1;
            }
        }
        if count == 0 {
            return node_id as f32;
        }
        sum / (count as f32)
    }

    fn resolve_layout_overlaps(&mut self) -> bool {
        if self.nodes.len() < 2 {
            return false;
        }

        let mut changed = false;
        for _ in 0..AUTO_LAYOUT_OVERLAP_MAX_PASSES {
            let mut pass_changed = false;
            let len = self.nodes.len();
            for i in 0..len {
                for j in (i + 1)..len {
                    let (left, right) = self.nodes.split_at_mut(j);
                    let (_, node_a, pos_a) = &mut left[i];
                    let (_, node_b, pos_b) = &mut right[0];

                    let half_w_a = (NODE_WIDTH + AUTO_LAYOUT_OVERLAP_PAD_X) * 0.5;
                    let half_w_b = (NODE_WIDTH + AUTO_LAYOUT_OVERLAP_PAD_X) * 0.5;
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
                        pos_a.y -= shift * direction;
                        pos_b.y += shift * direction;
                    } else {
                        let direction = if dx >= 0.0 { 1.0 } else { -1.0 };
                        let shift = (overlap_x * 0.5) + 1.0;
                        pos_a.x -= shift * direction;
                        pos_b.x += shift * direction;
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
