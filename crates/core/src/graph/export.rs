use super::*;
use crate::event::CondCompiled;

impl StoryGraph {
    pub(super) fn format_condition(cond: &CondCompiled) -> String {
        match cond {
            CondCompiled::Flag { flag_id, is_set } => {
                if *is_set {
                    format!("flag[{}]", flag_id)
                } else {
                    format!("!flag[{}]", flag_id)
                }
            }
            CondCompiled::VarCmp { var_id, op, value } => {
                format!("var[{}] {:?} {}", var_id, op, value)
            }
        }
    }

    pub fn to_dot(&self) -> String {
        let mut dot = String::from("digraph StoryGraph {\n");
        dot.push_str("    rankdir=TB;\n");
        dot.push_str("    node [shape=box];\n\n");

        // Nodes
        for node in &self.nodes {
            let color = if !node.reachable {
                "red"
            } else if node.id == self.start_id {
                "green"
            } else {
                "black"
            };

            let label = match &node.node_type {
                NodeType::Dialogue {
                    speaker,
                    text_preview,
                } => {
                    format!(
                        "[{}] {}: {}",
                        node.id,
                        speaker,
                        text_preview.replace('"', "'")
                    )
                }
                NodeType::Choice {
                    prompt,
                    option_count,
                } => {
                    format!(
                        "[{}] Choice: {} ({} options)",
                        node.id,
                        prompt.replace('"', "'"),
                        option_count
                    )
                }
                NodeType::Scene { background } => {
                    format!("[{}] Scene: {:?}", node.id, background)
                }
                NodeType::Jump => format!("[{}] Jump", node.id),
                NodeType::ConditionalJump { condition } => {
                    format!("[{}] If: {}", node.id, condition)
                }
                NodeType::StateChange { description } => {
                    format!("[{}] {}", node.id, description)
                }
                NodeType::Patch => format!("[{}] Patch", node.id),
                NodeType::ExtCall { command } => format!("[{}] Call: {}", node.id, command),
                NodeType::AudioAction {
                    channel, action, ..
                } => {
                    format!("[{}] Audio: {}/{}", node.id, channel, action)
                }
                NodeType::Transition { kind, .. } => {
                    format!("[{}] Transition: {}", node.id, kind)
                }
                NodeType::CharacterPlacement { name, x, y, scale } => {
                    format!(
                        "[{}] Placement: {} ({}, {}) s={:?}",
                        node.id, name, x, y, scale
                    )
                }
            };

            let shape = match &node.node_type {
                NodeType::Choice { .. } => "diamond",
                NodeType::ConditionalJump { .. } => "diamond",
                NodeType::Jump => "ellipse",
                _ => "box",
            };

            dot.push_str(&format!(
                "    n{} [label=\"{}\" shape={} color={}];\n",
                node.id, label, shape, color
            ));
        }

        dot.push('\n');

        // Edges
        for edge in &self.edges {
            let style = match edge.edge_type {
                EdgeType::Sequential => "solid",
                EdgeType::Jump => "dashed",
                EdgeType::ConditionalTrue => "bold",
                EdgeType::ConditionalFalse => "dotted",
                EdgeType::Choice { .. } => "solid",
            };

            let label = edge
                .label
                .as_ref()
                .map(|l| format!(" [label=\"{}\"]", l.replace('"', "'")))
                .unwrap_or_default();

            dot.push_str(&format!(
                "    n{} -> n{} [style={}{}];\n",
                edge.from, edge.to, style, label
            ));
        }

        dot.push_str("}\n");
        dot
    }
}
