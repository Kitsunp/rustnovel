use super::super::{DiagnosticTarget, OperationKind, StoryNode};
use super::{AuthoringCommandBus, AuthoringDelta, CommandApplyResult};

impl AuthoringCommandBus {
    pub(super) fn edit_dialogue(
        &mut self,
        node_id: u32,
        speaker: &str,
        text: &str,
    ) -> CommandApplyResult {
        let Some(StoryNode::Dialogue {
            speaker: current_speaker,
            text: current_text,
        }) = self.graph.get_node(node_id)
        else {
            return Err(format!("node {node_id} is not a dialogue"));
        };
        if current_speaker == speaker && current_text == text {
            return Err(format!("dialogue node {node_id} is unchanged"));
        }
        self.replace_node(
            node_id,
            StoryNode::Dialogue {
                speaker: speaker.to_string(),
                text: text.to_string(),
            },
            format!("graph.nodes[{node_id}].dialogue"),
        )
    }

    pub(super) fn edit_choice_prompt(&mut self, node_id: u32, prompt: &str) -> CommandApplyResult {
        let Some(StoryNode::Choice {
            prompt: current, ..
        }) = self.graph.get_node(node_id)
        else {
            return Err(format!("node {node_id} is not a choice"));
        };
        if current == prompt {
            return Err(format!("choice node {node_id} prompt is unchanged"));
        }
        let replacement = match self.graph.get_node(node_id).cloned() {
            Some(StoryNode::Choice { options, .. }) => StoryNode::Choice {
                prompt: prompt.to_string(),
                options,
            },
            _ => unreachable!("choice node checked above"),
        };
        self.replace_node(
            node_id,
            replacement,
            format!("graph.nodes[{node_id}].choice.prompt"),
        )
    }

    pub(super) fn edit_choice_option_text(
        &mut self,
        node_id: u32,
        option_index: usize,
        text: &str,
    ) -> CommandApplyResult {
        let Some(StoryNode::Choice { options, .. }) = self.graph.get_node(node_id) else {
            return Err(format!("node {node_id} is not a choice"));
        };
        if option_index >= options.len() {
            return Err(format!(
                "choice node {node_id} has no option {option_index}"
            ));
        }
        if options[option_index] == text {
            return Err(format!(
                "choice node {node_id} option {option_index} text is unchanged"
            ));
        }
        let replacement = match self.graph.get_node(node_id).cloned() {
            Some(StoryNode::Choice {
                prompt,
                mut options,
            }) => {
                options[option_index] = text.to_string();
                StoryNode::Choice { prompt, options }
            }
            _ => unreachable!("choice node checked above"),
        };
        self.replace_node(
            node_id,
            replacement,
            format!("graph.nodes[{node_id}].choice.options[{option_index}].text"),
        )
    }

    pub(super) fn replace_node(
        &mut self,
        node_id: u32,
        replacement: StoryNode,
        field_path: String,
    ) -> CommandApplyResult {
        let node = self
            .graph
            .get_node(node_id)
            .ok_or_else(|| format!("node {node_id} not found"))?;
        if *node == replacement {
            return Err(format!("node {node_id} is unchanged"));
        }
        let before = std::mem::replace(
            self.graph
                .get_node_mut(node_id)
                .expect("node checked above"),
            replacement.clone(),
        );
        let removed_connections = self.graph.prune_node_connections(node_id, &replacement);
        self.graph.mark_modified();
        Ok((
            AuthoringDelta::NodeEdited {
                node_id,
                before,
                after: replacement,
                removed_connections,
            },
            OperationKind::FieldEdited,
            Some(field_path),
            Some(DiagnosticTarget::Node { node_id }),
        ))
    }
}
