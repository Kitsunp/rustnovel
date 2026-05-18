use super::*;
use crate::editor::StoryNode;

struct MutationRecord {
    kind: &'static str,
    details: String,
    field_path: String,
}

struct CharacterPositionEdit {
    name: String,
    expression: Option<String>,
    source_instance_index: usize,
    x: i32,
    y: i32,
    scale: Option<f32>,
}

impl EditorWorkbench {
    pub(crate) fn apply_composer_node_mutation(
        &mut self,
        node_id: u32,
        mutation: crate::editor::visual_composer::ComposerNodeMutation,
    ) -> bool {
        let before_value = self
            .node_graph
            .get_node(node_id)
            .and_then(|node| serde_json::to_string(node).ok());
        let Some(record) = self.apply_composer_mutation_inner(node_id, mutation) else {
            return false;
        };
        let after_value = self
            .node_graph
            .get_node(node_id)
            .and_then(|node| serde_json::to_string(node).ok());
        self.node_graph.queue_operation_hint_with_values(
            record.kind,
            record.details,
            Some(record.field_path),
            before_value,
            after_value,
            true,
        );
        true
    }

    fn apply_composer_mutation_inner(
        &mut self,
        node_id: u32,
        mutation: crate::editor::visual_composer::ComposerNodeMutation,
    ) -> Option<MutationRecord> {
        match mutation {
            crate::editor::visual_composer::ComposerNodeMutation::DialogueText {
                speaker,
                text,
            } => self.apply_dialogue_text_mutation(node_id, speaker, text),
            crate::editor::visual_composer::ComposerNodeMutation::ChoicePrompt { prompt } => {
                self.apply_choice_prompt_mutation(node_id, prompt)
            }
            crate::editor::visual_composer::ComposerNodeMutation::ChoiceOptionText {
                option_index,
                text,
            } => self.apply_choice_option_text_mutation(node_id, option_index, text),
            crate::editor::visual_composer::ComposerNodeMutation::ChoiceOptionOrder {
                from_index,
                to_index,
            } => self.apply_choice_option_order_mutation(node_id, from_index, to_index),
            crate::editor::visual_composer::ComposerNodeMutation::ChoiceOptionTarget {
                option_index,
                target_node_id,
            } => self.apply_choice_option_target_mutation(node_id, option_index, target_node_id),
            crate::editor::visual_composer::ComposerNodeMutation::CharacterPosition {
                name,
                expression,
                source_instance_index,
                x,
                y,
                scale,
            } => self.apply_character_position_mutation(
                node_id,
                CharacterPositionEdit {
                    name,
                    expression,
                    source_instance_index,
                    x,
                    y,
                    scale,
                },
            ),
        }
    }

    fn apply_dialogue_text_mutation(
        &mut self,
        node_id: u32,
        speaker: String,
        text: String,
    ) -> Option<MutationRecord> {
        let Some(StoryNode::Dialogue {
            speaker: current_speaker,
            text: current_text,
        }) = self.node_graph.get_node_mut(node_id)
        else {
            return None;
        };
        if *current_speaker == speaker && *current_text == text {
            return None;
        }
        *current_speaker = speaker;
        *current_text = text;
        Some(field_record(
            node_id,
            "dialogue",
            "Edited Visual Composer dialogue overlay",
        ))
    }

    fn apply_choice_prompt_mutation(
        &mut self,
        node_id: u32,
        prompt: String,
    ) -> Option<MutationRecord> {
        let Some(StoryNode::Choice {
            prompt: current, ..
        }) = self.node_graph.get_node_mut(node_id)
        else {
            return None;
        };
        if *current == prompt {
            return None;
        }
        *current = prompt;
        Some(field_record(
            node_id,
            "choice.prompt",
            "Edited Visual Composer choice prompt",
        ))
    }

    fn apply_choice_option_text_mutation(
        &mut self,
        node_id: u32,
        option_index: usize,
        text: String,
    ) -> Option<MutationRecord> {
        let Some(StoryNode::Choice { options, .. }) = self.node_graph.get_node_mut(node_id) else {
            return None;
        };
        let option = options.get_mut(option_index)?;
        if *option == text {
            return None;
        }
        *option = text;
        Some(field_record(
            node_id,
            &format!("choice.options[{option_index}].text"),
            "Edited Visual Composer choice option",
        ))
    }

    fn apply_choice_option_order_mutation(
        &mut self,
        node_id: u32,
        from_index: usize,
        to_index: usize,
    ) -> Option<MutationRecord> {
        let option_count = match self.node_graph.get_node_mut(node_id)? {
            StoryNode::Choice { options, .. } => {
                if from_index >= options.len()
                    || to_index >= options.len()
                    || from_index == to_index
                {
                    return None;
                }
                let option = options.remove(from_index);
                options.insert(to_index, option);
                options.len()
            }
            _ => return None,
        };
        self.remap_choice_connections_after_move(node_id, option_count, from_index, to_index);
        Some(field_record(
            node_id,
            "choice.options",
            "Reordered Visual Composer choice options",
        ))
    }

    fn apply_choice_option_target_mutation(
        &mut self,
        node_id: u32,
        option_index: usize,
        target_node_id: Option<u32>,
    ) -> Option<MutationRecord> {
        let Some(StoryNode::Choice { options, .. }) = self.node_graph.get_node(node_id) else {
            return None;
        };
        if option_index >= options.len() {
            return None;
        }
        match target_node_id {
            Some(target) if self.node_graph.get_node(target).is_some() => {
                let already_connected = self.node_graph.connections().any(|conn| {
                    conn.from == node_id && conn.from_port == option_index && conn.to == target
                });
                if already_connected {
                    return None;
                }
                self.node_graph.connect_port(node_id, option_index, target);
            }
            Some(_) => return None,
            None => {
                let had_connection = self
                    .node_graph
                    .connections()
                    .any(|conn| conn.from == node_id && conn.from_port == option_index);
                if !had_connection {
                    return None;
                }
                self.node_graph.disconnect_port(node_id, option_index);
            }
        }
        Some(field_record(
            node_id,
            &format!("choice.options[{option_index}].target"),
            "Edited Visual Composer choice target",
        ))
    }

    fn apply_character_position_mutation(
        &mut self,
        node_id: u32,
        edit: CharacterPositionEdit,
    ) -> Option<MutationRecord> {
        let node = self.node_graph.get_node_mut(node_id)?;
        let changed = match node {
            StoryNode::CharacterPlacement {
                name: node_name,
                x: node_x,
                y: node_y,
                scale: node_scale,
            } => update_character_placement_node(node_name, node_x, node_y, node_scale, edit),
            StoryNode::Scene { characters, .. } => update_character_list_position(characters, edit),
            StoryNode::ScenePatch(patch) => update_character_list_position(&mut patch.add, edit),
            StoryNode::Generic(visual_novel_engine::EventRaw::SetCharacterPosition(pos)) => {
                let changed = pos.name != edit.name
                    || pos.x != edit.x
                    || pos.y != edit.y
                    || pos.scale != edit.scale;
                if changed {
                    pos.name = edit.name;
                    pos.x = edit.x;
                    pos.y = edit.y;
                    pos.scale = edit.scale;
                }
                changed
            }
            _ => false,
        };
        changed.then(|| MutationRecord {
            kind: "composer_drag_entity",
            details: format!("Moved Visual Composer entity for node {node_id}"),
            field_path: format!("graph.nodes[{node_id}].visual.transform"),
        })
    }

    fn remap_choice_connections_after_move(
        &mut self,
        node_id: u32,
        option_count: usize,
        from_index: usize,
        to_index: usize,
    ) {
        let mut connections = self
            .node_graph
            .connections()
            .filter(|conn| conn.from == node_id && conn.from_port < option_count)
            .map(|conn| (conn.from_port, conn.to))
            .collect::<Vec<_>>();
        connections.sort_unstable();
        for (port, _) in &connections {
            self.node_graph.disconnect_port(node_id, *port);
        }
        for (old_port, target) in connections {
            let new_port = remapped_port(old_port, from_index, to_index);
            self.node_graph.connect_port(node_id, new_port, target);
        }
    }
}

fn update_character_placement_node(
    node_name: &mut String,
    node_x: &mut i32,
    node_y: &mut i32,
    node_scale: &mut Option<f32>,
    edit: CharacterPositionEdit,
) -> bool {
    let changed = *node_name != edit.name
        || *node_x != edit.x
        || *node_y != edit.y
        || *node_scale != edit.scale;
    if changed {
        *node_name = edit.name;
        *node_x = edit.x;
        *node_y = edit.y;
        *node_scale = edit.scale;
    }
    changed
}

fn update_character_list_position(
    characters: &mut Vec<visual_novel_engine::CharacterPlacementRaw>,
    edit: CharacterPositionEdit,
) -> bool {
    if let Some(character) = find_character_placement_mut(
        characters,
        &edit.name,
        edit.expression.as_deref(),
        edit.source_instance_index,
    ) {
        let changed = character.x != Some(edit.x)
            || character.y != Some(edit.y)
            || character.scale != edit.scale;
        if changed {
            character.x = Some(edit.x);
            character.y = Some(edit.y);
            character.scale = edit.scale;
        }
        return changed;
    }
    characters.push(visual_novel_engine::CharacterPlacementRaw {
        name: edit.name,
        expression: edit.expression,
        position: None,
        x: Some(edit.x),
        y: Some(edit.y),
        scale: edit.scale,
    });
    true
}

fn find_character_placement_mut<'a>(
    characters: &'a mut [visual_novel_engine::CharacterPlacementRaw],
    name: &str,
    expression: Option<&str>,
    source_instance_index: usize,
) -> Option<&'a mut visual_novel_engine::CharacterPlacementRaw> {
    let mut seen = 0usize;
    for character in characters {
        if character.name == name && character.expression.as_deref() == expression {
            if seen == source_instance_index {
                return Some(character);
            }
            seen += 1;
        }
    }
    None
}

fn remapped_port(old_port: usize, from_index: usize, to_index: usize) -> usize {
    match from_index.cmp(&to_index) {
        std::cmp::Ordering::Less if old_port == from_index => to_index,
        std::cmp::Ordering::Less if old_port > from_index && old_port <= to_index => old_port - 1,
        std::cmp::Ordering::Greater if old_port == from_index => to_index,
        std::cmp::Ordering::Greater if old_port >= to_index && old_port < from_index => {
            old_port + 1
        }
        _ => old_port,
    }
}

fn field_record(node_id: u32, field: &str, details: &str) -> MutationRecord {
    MutationRecord {
        kind: "field_edited",
        details: format!("{details} for node {node_id}"),
        field_path: format!("graph.nodes[{node_id}].{field}"),
    }
}
