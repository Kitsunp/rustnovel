use super::*;
use crate::editor::StoryNode;
use visual_novel_engine::authoring::{
    composer, AuthoringCommand, AuthoringCommandBus, AuthoringDelta,
};

struct MutationRecord {
    kind: &'static str,
    details: String,
    field_path: String,
}

#[derive(Clone)]
struct CharacterPositionEdit {
    name: String,
    expression: Option<String>,
    source_instance_index: usize,
    x: i32,
    y: i32,
    scale: Option<f32>,
}

impl EditorWorkbench {
    pub fn apply_composer_node_mutation(
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
        self.apply_node_graph_command(AuthoringCommand::EditDialogue {
            node_id,
            speaker,
            text,
        })?;
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
        self.apply_node_graph_command(AuthoringCommand::EditChoicePrompt { node_id, prompt })?;
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
        self.apply_node_graph_command(AuthoringCommand::EditChoiceOptionText {
            node_id,
            option_index,
            text,
        })?;
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
        self.apply_node_graph_command(AuthoringCommand::ReorderChoiceOption {
            node_id,
            from_index,
            to_index,
        })?;
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
                self.apply_node_graph_command(AuthoringCommand::SetChoiceOptionTarget {
                    node_id,
                    option_index,
                    target_node_id: Some(target),
                })?;
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
                self.apply_node_graph_command(AuthoringCommand::SetChoiceOptionTarget {
                    node_id,
                    option_index,
                    target_node_id: None,
                })?;
            }
        }
        Some(choice_target_record(node_id, option_index, target_node_id))
    }

    fn apply_character_position_mutation(
        &mut self,
        node_id: u32,
        edit: CharacterPositionEdit,
    ) -> Option<MutationRecord> {
        match self.apply_character_position_with_command_bus(node_id, &edit) {
            CommandBusMoveResult::Applied => {
                return Some(character_position_record(node_id));
            }
            CommandBusMoveResult::NoChange => return None,
            CommandBusMoveResult::Unsupported => {}
        }

        let replacement = character_position_replacement(self.node_graph.get_node(node_id)?, edit)?;
        self.apply_node_graph_command(AuthoringCommand::EditNode {
            node_id,
            replacement,
        })?;
        Some(character_position_record(node_id))
    }

    fn apply_character_position_with_command_bus(
        &mut self,
        node_id: u32,
        edit: &CharacterPositionEdit,
    ) -> CommandBusMoveResult {
        if !matches!(
            self.node_graph.get_node(node_id),
            Some(StoryNode::Scene { .. } | StoryNode::ScenePatch(_))
        ) {
            return CommandBusMoveResult::Unsupported;
        }
        let Some((object_id, before_pose)) =
            command_bus_character_object(self.node_graph.authoring_graph(), node_id, edit)
        else {
            return CommandBusMoveResult::Unsupported;
        };
        if before_pose == (Some(edit.x), Some(edit.y), edit.scale) {
            return CommandBusMoveResult::NoChange;
        }

        let mut bus = AuthoringCommandBus::new(self.node_graph.authoring_graph().clone());
        bus.apply(AuthoringCommand::MoveLayer {
            object_id,
            x: edit.x,
            y: edit.y,
            scale: edit.scale,
        })
        .expect("matched composer layer object should be movable through AuthoringCommandBus");
        self.node_graph.replace_authoring_graph(bus.graph().clone());
        CommandBusMoveResult::Applied
    }

    fn apply_node_graph_command(&mut self, command: AuthoringCommand) -> Option<AuthoringDelta> {
        let mut bus = AuthoringCommandBus::new(self.node_graph.authoring_graph().clone());
        let outcome = bus.apply(command).ok()?;
        self.node_graph.replace_authoring_graph(bus.graph().clone());
        Some(outcome.delta)
    }
}

enum CommandBusMoveResult {
    Applied,
    NoChange,
    Unsupported,
}

type CharacterLayerPose = (Option<i32>, Option<i32>, Option<f32>);
type CharacterLayerTarget = (String, CharacterLayerPose);

fn command_bus_character_object(
    graph: &visual_novel_engine::authoring::NodeGraph,
    node_id: u32,
    edit: &CharacterPositionEdit,
) -> Option<CharacterLayerTarget> {
    let mut seen = 0usize;
    for object in composer::list_layered_objects(graph, Some(node_id)) {
        if object.source_node_id != Some(node_id)
            || object.character_name.as_deref() != Some(edit.name.as_str())
            || object.expression.as_deref() != edit.expression.as_deref()
        {
            continue;
        }
        if seen == edit.source_instance_index {
            return Some((object.object_id, (object.x, object.y, object.scale)));
        }
        seen += 1;
    }
    None
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
    characters: &mut Vec<visual_novel_engine::runtime::CharacterPlacementRaw>,
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
    characters.push(visual_novel_engine::runtime::CharacterPlacementRaw {
        name: edit.name,
        expression: edit.expression,
        position: None,
        x: Some(edit.x),
        y: Some(edit.y),
        scale: edit.scale,
    });
    true
}

fn character_position_replacement(
    node: &StoryNode,
    edit: CharacterPositionEdit,
) -> Option<StoryNode> {
    let mut replacement = node.clone();
    let changed = match &mut replacement {
        StoryNode::CharacterPlacement { name, x, y, scale } => {
            update_character_placement_node(name, x, y, scale, edit)
        }
        StoryNode::Scene { characters, .. } => update_character_list_position(characters, edit),
        StoryNode::ScenePatch(patch) => update_character_list_position(&mut patch.add, edit),
        StoryNode::Generic(visual_novel_engine::runtime::EventRaw::SetCharacterPosition(pos)) => {
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
    changed.then_some(replacement)
}

fn find_character_placement_mut<'a>(
    characters: &'a mut [visual_novel_engine::runtime::CharacterPlacementRaw],
    name: &str,
    expression: Option<&str>,
    source_instance_index: usize,
) -> Option<&'a mut visual_novel_engine::runtime::CharacterPlacementRaw> {
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

fn field_record(node_id: u32, field: &str, details: &str) -> MutationRecord {
    MutationRecord {
        kind: "field_edited",
        details: format!("{details} for node {node_id}"),
        field_path: format!("graph.nodes[{node_id}].{field}"),
    }
}

fn character_position_record(node_id: u32) -> MutationRecord {
    MutationRecord {
        kind: "composer_drag_entity",
        details: format!("Moved Visual Composer entity for node {node_id}"),
        field_path: format!("graph.nodes[{node_id}].visual.transform"),
    }
}

fn choice_target_record(
    node_id: u32,
    option_index: usize,
    target_node_id: Option<u32>,
) -> MutationRecord {
    match target_node_id {
        Some(target) => MutationRecord {
            kind: "node_connected",
            details: format!(
                "Connected Visual Composer choice {node_id} option {option_index} to node {target}"
            ),
            field_path: format!("graph.edges[{node_id}:{option_index}]"),
        },
        None => MutationRecord {
            kind: "node_disconnected",
            details: format!("Disconnected Visual Composer choice {node_id} option {option_index}"),
            field_path: format!("graph.edges[{node_id}:{option_index}]"),
        },
    }
}
