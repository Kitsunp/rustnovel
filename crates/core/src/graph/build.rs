use crate::event::EventCompiled;

use super::{EdgeType, GraphEdge, NodeId, NodeType, StoryGraph};

impl StoryGraph {
    /// Processes an event and returns its node type and outgoing edges.
    pub(super) fn process_event(
        ip: NodeId,
        event: &EventCompiled,
        event_count: usize,
    ) -> (NodeType, Vec<GraphEdge>) {
        let next_ip = ip + 1;
        let has_next = (next_ip as usize) < event_count;

        match event {
            EventCompiled::Dialogue(dialogue) => {
                let text_preview = preview_text(&dialogue.text, 50);
                let node_type = NodeType::Dialogue {
                    speaker: dialogue.speaker.to_string(),
                    text_preview,
                };
                (node_type, sequential_edge(ip, next_ip, has_next))
            }

            EventCompiled::Choice(choice) => {
                let node_type = NodeType::Choice {
                    prompt: choice.prompt.to_string(),
                    option_count: choice.options.len(),
                };
                let edges = choice
                    .options
                    .iter()
                    .enumerate()
                    .map(|(idx, opt)| GraphEdge {
                        from: ip,
                        to: opt.target_ip,
                        edge_type: EdgeType::Choice { option_index: idx },
                        label: Some(opt.text.to_string()),
                    })
                    .collect();
                (node_type, edges)
            }

            EventCompiled::Scene(scene) => {
                let node_type = NodeType::Scene {
                    background: scene.background.as_ref().map(|s| s.to_string()),
                };
                (node_type, sequential_edge(ip, next_ip, has_next))
            }

            EventCompiled::Jump { target_ip } => {
                let edges = vec![GraphEdge {
                    from: ip,
                    to: *target_ip,
                    edge_type: EdgeType::Jump,
                    label: None,
                }];
                (NodeType::Jump, edges)
            }

            EventCompiled::JumpIf { cond, target_ip } => {
                let node_type = NodeType::ConditionalJump {
                    condition: Self::format_condition(cond),
                };
                let mut edges = vec![GraphEdge {
                    from: ip,
                    to: *target_ip,
                    edge_type: EdgeType::ConditionalTrue,
                    label: Some("true".to_string()),
                }];
                if has_next {
                    edges.push(GraphEdge {
                        from: ip,
                        to: next_ip,
                        edge_type: EdgeType::ConditionalFalse,
                        label: Some("false".to_string()),
                    });
                }
                (node_type, edges)
            }

            EventCompiled::SetFlag { flag_id, value } => {
                let node_type = NodeType::StateChange {
                    description: format!("flag[{}] = {}", flag_id, value),
                };
                (node_type, sequential_edge(ip, next_ip, has_next))
            }

            EventCompiled::SetVar { var_id, value } => {
                let node_type = NodeType::StateChange {
                    description: format!("var[{}] = {}", var_id, value),
                };
                (node_type, sequential_edge(ip, next_ip, has_next))
            }

            EventCompiled::Patch(_) => (NodeType::Patch, sequential_edge(ip, next_ip, has_next)),

            EventCompiled::ExtCall { command, args: _ } => {
                let node_type = NodeType::ExtCall {
                    command: command.clone(),
                };
                (node_type, sequential_edge(ip, next_ip, has_next))
            }

            EventCompiled::AudioAction(action) => {
                let node_type = NodeType::AudioAction {
                    channel: action.channel,
                    action: action.action,
                    asset: action.asset.as_ref().map(|s| s.to_string()),
                };
                (node_type, sequential_edge(ip, next_ip, has_next))
            }

            EventCompiled::Transition(transition) => {
                let node_type = NodeType::Transition {
                    kind: match transition.kind {
                        0 => "fade".to_string(),
                        1 => "dissolve".to_string(),
                        2 => "cut".to_string(),
                        _ => "unknown".to_string(),
                    },
                    duration: transition.duration_ms.into(),
                };
                (node_type, sequential_edge(ip, next_ip, has_next))
            }
            EventCompiled::SetCharacterPosition(pos) => {
                let node_type = NodeType::CharacterPlacement {
                    name: pos.name.to_string(),
                    x: pos.x,
                    y: pos.y,
                    scale: pos.scale,
                };
                (node_type, sequential_edge(ip, next_ip, has_next))
            }
        }
    }
}

fn sequential_edge(ip: NodeId, next_ip: NodeId, has_next: bool) -> Vec<GraphEdge> {
    if has_next {
        vec![GraphEdge {
            from: ip,
            to: next_ip,
            edge_type: EdgeType::Sequential,
            label: None,
        }]
    } else {
        Vec::new()
    }
}

fn preview_text(text: &str, max_chars: usize) -> String {
    let text_len = text.chars().count();
    if text_len <= max_chars {
        return text.to_string();
    }

    if max_chars <= 3 {
        return ".".repeat(max_chars);
    }

    let head: String = text.chars().take(max_chars - 3).collect();
    format!("{head}...")
}
