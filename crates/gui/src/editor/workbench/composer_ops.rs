use super::*;
use crate::editor::StoryNode;
use visual_novel_engine::EventCompiled;

impl EditorWorkbench {
    pub(crate) fn add_composer_created_node(
        &mut self,
        node: StoryNode,
        pos: eframe::egui::Pos2,
    ) -> u32 {
        let source = self.node_graph.selected;
        let new_id = self.node_graph.add_node(node, pos);
        if let Some(source_id) = source {
            self.connect_composer_node_from_selection(source_id, new_id);
        }
        self.node_graph.set_single_selection(Some(new_id));
        self.selected_node = Some(new_id);
        self.node_graph.mark_modified();
        new_id
    }

    fn connect_composer_node_from_selection(&mut self, source_id: u32, new_id: u32) {
        let Some(source_node) = self.node_graph.get_node(source_id).cloned() else {
            return;
        };
        let port = match source_node {
            StoryNode::Choice { options, .. } => {
                let used = self
                    .node_graph
                    .connections()
                    .filter(|connection| connection.from == source_id)
                    .map(|connection| connection.from_port)
                    .collect::<std::collections::BTreeSet<_>>();
                (0..=options.len())
                    .find(|candidate| !used.contains(candidate))
                    .unwrap_or(options.len())
            }
            StoryNode::End => return,
            _ => 0,
        };
        self.node_graph.connect_or_branch(source_id, port, new_id);
    }

    pub(super) fn build_entity_node_map(&self) -> std::collections::HashMap<u32, u32> {
        let mut map = std::collections::HashMap::new();
        use crate::editor::node_types::StoryNode;
        use std::collections::{HashMap, VecDeque};

        let mut characters_by_key: HashMap<String, VecDeque<u32>> = HashMap::new();
        let mut images_by_path: HashMap<String, Vec<u32>> = HashMap::new();
        let mut audio_by_path: HashMap<String, Vec<u32>> = HashMap::new();

        for entity in self.scene.iter() {
            match &entity.kind {
                visual_novel_engine::EntityKind::Character(character) => {
                    characters_by_key
                        .entry(character_match_key(
                            character.name.as_ref(),
                            character.expression.as_deref(),
                        ))
                        .or_default()
                        .push_back(entity.id.raw());
                }
                visual_novel_engine::EntityKind::Image(image) => {
                    images_by_path
                        .entry(image.path.to_string())
                        .or_default()
                        .push(entity.id.raw());
                }
                visual_novel_engine::EntityKind::Audio(audio) => {
                    audio_by_path
                        .entry(audio.path.to_string())
                        .or_default()
                        .push(entity.id.raw());
                }
                _ => {}
            }
        }

        // Fallback ownership map when preview trace ownership is unavailable.
        for (nid, node, _) in self.node_graph.nodes() {
            match node {
                StoryNode::Scene {
                    background,
                    music,
                    characters,
                    ..
                } => {
                    if let Some(background) = background {
                        bind_matches(
                            &mut map,
                            images_by_path.get(background.as_str()),
                            nid,
                            false,
                        );
                    }
                    if let Some(music) = music {
                        bind_matches(&mut map, audio_by_path.get(music.as_str()), nid, false);
                    }
                    for character in characters {
                        bind_one_character(
                            &mut map,
                            &mut characters_by_key,
                            character.name.as_str(),
                            character.expression.as_deref(),
                            nid,
                        );
                        if let Some(expression) = &character.expression {
                            bind_matches(
                                &mut map,
                                images_by_path.get(expression.as_str()),
                                nid,
                                false,
                            );
                        }
                    }
                }
                StoryNode::ScenePatch(patch) => {
                    if let Some(background) = &patch.background {
                        bind_matches(
                            &mut map,
                            images_by_path.get(background.as_str()),
                            nid,
                            false,
                        );
                    }
                    if let Some(music) = &patch.music {
                        bind_matches(&mut map, audio_by_path.get(music.as_str()), nid, false);
                    }
                    for character in &patch.add {
                        bind_one_character(
                            &mut map,
                            &mut characters_by_key,
                            character.name.as_str(),
                            character.expression.as_deref(),
                            nid,
                        );
                    }
                    for character in &patch.update {
                        bind_one_character(
                            &mut map,
                            &mut characters_by_key,
                            character.name.as_str(),
                            character.expression.as_deref(),
                            nid,
                        );
                    }
                }
                StoryNode::CharacterPlacement { name, .. } => {
                    bind_one_character(&mut map, &mut characters_by_key, name.as_str(), None, nid);
                }
                StoryNode::AudioAction {
                    asset: Some(asset), ..
                } => {
                    // Keep scene/patch ownership when already resolved.
                    bind_matches(&mut map, audio_by_path.get(asset.as_str()), nid, true);
                }
                StoryNode::Generic(visual_novel_engine::EventRaw::SetCharacterPosition(pos)) => {
                    bind_one_character(
                        &mut map,
                        &mut characters_by_key,
                        pos.name.as_str(),
                        None,
                        nid,
                    );
                }
                _ => {}
            }
        }
        map
    }

    pub(crate) fn apply_composer_node_mutation(
        &mut self,
        node_id: u32,
        mutation: crate::editor::visual_composer::ComposerNodeMutation,
    ) -> bool {
        let before_value = self
            .node_graph
            .get_node(node_id)
            .and_then(|node| serde_json::to_string(node).ok());
        let changed = match mutation {
            crate::editor::visual_composer::ComposerNodeMutation::CharacterPosition {
                name,
                expression,
                source_instance_index,
                x,
                y,
                scale,
            } => {
                let Some(node) = self.node_graph.get_node_mut(node_id) else {
                    return false;
                };
                match node {
                    StoryNode::CharacterPlacement {
                        name: node_name,
                        x: node_x,
                        y: node_y,
                        scale: node_scale,
                    } => {
                        let changed = *node_name != name
                            || *node_x != x
                            || *node_y != y
                            || *node_scale != scale;
                        if changed {
                            *node_name = name;
                            *node_x = x;
                            *node_y = y;
                            *node_scale = scale;
                        }
                        changed
                    }
                    StoryNode::Scene { characters, .. } => {
                        if let Some(character) = find_character_placement_mut(
                            characters,
                            &name,
                            expression.as_deref(),
                            source_instance_index,
                        ) {
                            let changed = character.x != Some(x)
                                || character.y != Some(y)
                                || character.scale != scale;
                            if changed {
                                character.x = Some(x);
                                character.y = Some(y);
                                character.scale = scale;
                            }
                            changed
                        } else {
                            characters.push(visual_novel_engine::CharacterPlacementRaw {
                                name,
                                expression,
                                position: None,
                                x: Some(x),
                                y: Some(y),
                                scale,
                            });
                            true
                        }
                    }
                    StoryNode::ScenePatch(patch) => {
                        if let Some(character) = find_character_placement_mut(
                            &mut patch.add,
                            &name,
                            expression.as_deref(),
                            source_instance_index,
                        ) {
                            let changed = character.x != Some(x)
                                || character.y != Some(y)
                                || character.scale != scale;
                            if changed {
                                character.x = Some(x);
                                character.y = Some(y);
                                character.scale = scale;
                            }
                            changed
                        } else {
                            patch.add.push(visual_novel_engine::CharacterPlacementRaw {
                                name,
                                expression,
                                position: None,
                                x: Some(x),
                                y: Some(y),
                                scale,
                            });
                            true
                        }
                    }
                    StoryNode::Generic(visual_novel_engine::EventRaw::SetCharacterPosition(
                        pos,
                    )) => {
                        let changed =
                            pos.name != name || pos.x != x || pos.y != y || pos.scale != scale;
                        if changed {
                            pos.name = name;
                            pos.x = x;
                            pos.y = y;
                            pos.scale = scale;
                        }
                        changed
                    }
                    _ => false,
                }
            }
        };
        if changed {
            let after_value = self
                .node_graph
                .get_node(node_id)
                .and_then(|node| serde_json::to_string(node).ok());
            self.node_graph.queue_operation_hint_with_values(
                "composer_drag_entity",
                format!("Moved Visual Composer entity for node {node_id}"),
                Some(format!("graph.nodes[{node_id}].visual.transform")),
                before_value,
                after_value,
                true,
            );
        }
        changed
    }

    #[cfg(test)]
    pub(crate) fn start_composer_runtime_preview_from_selection(&mut self) {
        self.start_composer_runtime_preview_from_node(
            self.selected_node.or(self.node_graph.selected),
        );
    }

    pub(crate) fn start_composer_runtime_preview_from_node(&mut self, selected_node: Option<u32>) {
        if let Err(err) = self.sync_graph_to_script() {
            self.toast = Some(ToastState::error(format!("Composer test failed: {err}")));
            return;
        }

        let target_label = selected_node
            .filter(|node_id| {
                self.node_graph
                    .get_node(*node_id)
                    .is_some_and(|node| !node.is_marker())
            })
            .map(|node_id| format!("node_{node_id}"))
            .unwrap_or_else(|| "start".to_string());
        self.jump_composer_runtime_preview(&target_label);
    }

    pub(crate) fn restart_composer_runtime_preview(&mut self) {
        if self.engine.is_none() {
            if let Err(err) = self.sync_graph_to_script() {
                self.toast = Some(ToastState::error(format!("Composer restart failed: {err}")));
                return;
            }
        }
        self.jump_composer_runtime_preview("start");
    }

    pub(crate) fn advance_composer_runtime_preview(&mut self, choice: Option<usize>) {
        if self.engine.is_none() {
            if let Err(err) = self.sync_graph_to_script() {
                self.toast = Some(ToastState::error(format!("Composer test failed: {err}")));
                return;
            }
        }

        let result = {
            let Some(engine) = self.engine.as_mut() else {
                self.toast = Some(ToastState::error("Composer test unavailable"));
                return;
            };
            match engine.current_event() {
                Ok(EventCompiled::Choice(_)) => match choice {
                    Some(index) => engine.choose(index).map(|_| engine.take_audio_commands()),
                    None => {
                        self.toast = Some(ToastState::warning("Choose an option to continue"));
                        return;
                    }
                },
                Ok(EventCompiled::ExtCall { .. }) => engine.resume().map(|_| Vec::new()),
                Ok(_) => engine.step().map(|(audio, _)| audio),
                Err(err) => Err(err),
            }
        };

        match result {
            Ok(audio_commands) => {
                self.apply_composer_audio_commands(audio_commands);
                self.refresh_scene_from_engine_preview();
                self.select_current_composer_runtime_node();
            }
            Err(err) => {
                self.toast = Some(ToastState::warning(format!("Composer test stopped: {err}")));
            }
        }
    }

    fn jump_composer_runtime_preview(&mut self, label: &str) {
        let result = {
            let Some(engine) = self.engine.as_mut() else {
                self.toast = Some(ToastState::error("Composer test unavailable"));
                return;
            };
            engine.jump_to_label(label).map(|_| {
                engine.clear_session_history();
                engine.take_audio_commands()
            })
        };

        match result {
            Ok(audio_commands) => {
                self.player_state.reset_for_restart(0.0);
                self.apply_composer_audio_commands(audio_commands);
                self.refresh_scene_from_engine_preview();
                self.select_current_composer_runtime_node();
                self.toast = Some(ToastState::success("Composer test ready"));
            }
            Err(err) => {
                self.toast = Some(ToastState::error(format!(
                    "Composer test could not jump to '{label}': {err}"
                )));
            }
        }
    }

    fn apply_composer_audio_commands(
        &mut self,
        audio_commands: Vec<visual_novel_engine::AudioCommand>,
    ) {
        if audio_commands.is_empty() {
            return;
        }
        self.ensure_player_audio_backend();
        self.apply_player_audio_commands(audio_commands);
    }

    fn select_current_composer_runtime_node(&mut self) {
        let Some(engine) = self.engine.as_ref() else {
            return;
        };
        let Some(node_id) = self
            .node_graph
            .authoring_graph()
            .node_for_event_ip(engine.state().position)
        else {
            return;
        };
        self.node_graph.set_single_selection(Some(node_id));
        self.selected_node = Some(node_id);
        self.selected_entity = None;
    }
}

fn character_match_key(name: &str, expression: Option<&str>) -> String {
    format!("{}|{}", name.trim(), expression.unwrap_or("").trim())
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

fn bind_owner(
    map: &mut std::collections::HashMap<u32, u32>,
    entity_id: u32,
    node_id: u32,
    prefer_existing: bool,
) {
    if prefer_existing {
        map.entry(entity_id).or_insert(node_id);
    } else {
        map.insert(entity_id, node_id);
    }
}

fn bind_matches(
    map: &mut std::collections::HashMap<u32, u32>,
    matches: Option<&Vec<u32>>,
    node_id: u32,
    prefer_existing: bool,
) {
    if let Some(entity_ids) = matches {
        for &entity_id in entity_ids {
            bind_owner(map, entity_id, node_id, prefer_existing);
        }
    }
}

fn bind_one_character(
    map: &mut std::collections::HashMap<u32, u32>,
    characters_by_key: &mut std::collections::HashMap<String, std::collections::VecDeque<u32>>,
    name: &str,
    expression: Option<&str>,
    node_id: u32,
) {
    let key = character_match_key(name, expression);
    if let Some(entity_ids) = characters_by_key.get_mut(&key) {
        if let Some(entity_id) = entity_ids.pop_front() {
            bind_owner(map, entity_id, node_id, false);
            return;
        }
    }
    let fallback_key = character_match_key(name, None);
    if let Some(entity_ids) = characters_by_key.get_mut(&fallback_key) {
        if let Some(entity_id) = entity_ids.pop_front() {
            bind_owner(map, entity_id, node_id, false);
        }
    }
}
