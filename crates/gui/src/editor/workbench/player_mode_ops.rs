use super::*;
use std::collections::{BTreeMap, HashMap};
impl EditorWorkbench {
    pub(super) fn prepare_player_mode(&mut self) -> bool {
        if self.engine.is_none() && self.sync_graph_to_script().is_err() {
            self.toast = Some(ToastState::error(
                "No se pudo preparar el Player: corrige errores del grafo/importacion",
            ));
            return false;
        }
        {
            let Some(engine) = self.engine.as_mut() else {
                self.toast = Some(ToastState::error(
                    "Player no disponible: no hay engine inicializado",
                ));
                return false;
            };

            if let Err(err) = engine.jump_to_label("start") {
                self.toast = Some(ToastState::error(format!(
                    "Player no pudo iniciar en 'start': {err}"
                )));
                return false;
            }
            engine.clear_session_history();
        }
        self.player_state.reset_for_restart(0.0);
        self.ensure_player_audio_backend();
        self.refresh_scene_from_engine_preview();
        true
    }
    pub(crate) fn refresh_scene_from_engine_preview(&mut self) {
        let Some(engine) = self.engine.as_ref() else {
            self.refresh_scene_from_selected_node();
            return;
        };
        let target_ip = self
            .selected_node
            .and_then(|node_id| self.node_graph.event_ip_for_node(node_id));
        if target_ip.is_none()
            && self
                .selected_node
                .and_then(|node_id| self.node_graph.get_node(node_id))
                .is_some_and(|node| !node.is_marker())
        {
            self.refresh_scene_from_selected_node();
            return;
        }
        let visual = Self::preview_visual_for_target(engine, target_ip);
        let script_hints = Self::preview_script_hints(engine, target_ip, &self.node_graph);
        let snapshot = Self::scene_from_visual_state(
            &visual,
            script_hints.audio_hint,
            &script_hints.owner_hints,
            &self.node_graph,
        );
        self.scene = snapshot.scene;
        self.composer_entity_owners = snapshot.owners;
        if self.selected_entity.is_some_and(|id| {
            self.scene
                .get(visual_novel_engine::EntityId::new(id))
                .is_none()
        }) {
            self.selected_entity = None;
        }
    }

    fn preview_visual_for_target(
        engine: &Engine,
        target_ip: Option<u32>,
    ) -> visual_novel_engine::VisualState {
        let mut preview = engine.clone();
        let max_steps = target_ip
            .map(|ip| (ip as usize).saturating_add(64))
            .unwrap_or(256usize)
            .min(4096);
        for _ in 0..max_steps {
            let current_ip = preview.state().position;
            if let Some(target) = target_ip {
                if current_ip > target {
                    break;
                }
            }
            let Ok(event) = preview.current_event() else {
                break;
            };
            let advanced_ok = match &event {
                visual_novel_engine::EventCompiled::ExtCall { .. } => preview.resume().is_ok(),
                visual_novel_engine::EventCompiled::Choice(choice) => {
                    if target_ip.is_none() || choice.options.is_empty() {
                        false
                    } else {
                        preview.choose(0).is_ok()
                    }
                }
                visual_novel_engine::EventCompiled::Dialogue(_)
                | visual_novel_engine::EventCompiled::Scene(_)
                | visual_novel_engine::EventCompiled::Patch(_)
                | visual_novel_engine::EventCompiled::SetCharacterPosition(_)
                | visual_novel_engine::EventCompiled::Transition(_)
                | visual_novel_engine::EventCompiled::Jump { .. }
                | visual_novel_engine::EventCompiled::SetFlag { .. }
                | visual_novel_engine::EventCompiled::SetVar { .. }
                | visual_novel_engine::EventCompiled::JumpIf { .. }
                | visual_novel_engine::EventCompiled::AudioAction(_) => preview.step().is_ok(),
            };
            if !advanced_ok {
                break;
            }
            if target_ip.is_none() {
                match event {
                    visual_novel_engine::EventCompiled::Scene(_)
                    | visual_novel_engine::EventCompiled::Patch(_)
                    | visual_novel_engine::EventCompiled::SetCharacterPosition(_)
                    | visual_novel_engine::EventCompiled::Dialogue(_) => break,
                    _ => {}
                }
            } else if let Some(target) = target_ip {
                if preview.state().position > target {
                    break;
                }
            }
        }
        preview.visual_state().clone()
    }
    fn scene_from_visual_state(
        visual: &visual_novel_engine::VisualState,
        audio_hint: AudioPreviewHint,
        owner_hints: &PreviewOwnerHints,
        graph: &crate::editor::node_graph::NodeGraph,
    ) -> PreviewSceneSnapshot {
        let mut scene = visual_novel_engine::SceneState::new();
        let mut owners = HashMap::new();
        if let Some(background) = &visual.background {
            let mut transform = visual_novel_engine::Transform::at(0, 0);
            transform.z_order = -100;
            if let Some(entity_id) = scene.spawn_with_transform(
                transform,
                visual_novel_engine::EntityKind::Image(visual_novel_engine::ImageData {
                    path: background.clone(),
                    tint: None,
                }),
            ) {
                let owner = owner_hints
                    .background_owner
                    .or_else(|| graph.first_node_referencing_asset(background.as_ref()));
                if let Some(owner_id) = owner {
                    owners.insert(entity_id.raw(), owner_id);
                }
            }
        }
        for (index, character) in visual.characters.iter().enumerate() {
            let default_x = 220 + (index as i32) * 180;
            let default_y = 260;
            let mut transform = visual_novel_engine::Transform::at(
                character.x.unwrap_or(default_x),
                character.y.unwrap_or(default_y),
            );
            transform.z_order = index as i32;
            let scale = character.scale.unwrap_or(1.0).clamp(0.1, 4.0);
            transform.scale = (scale * 1000.0) as u32;
            if let Some(entity_id) = scene.spawn_with_transform(
                transform,
                visual_novel_engine::EntityKind::Character(visual_novel_engine::CharacterData {
                    name: character.name.clone(),
                    expression: character.expression.clone(),
                }),
            ) {
                let owner = owner_hints
                    .character_owners
                    .get(character.name.as_ref())
                    .copied()
                    .or_else(|| {
                        character
                            .expression
                            .as_ref()
                            .and_then(|expr| graph.first_node_referencing_asset(expr.as_ref()))
                    });
                if let Some(owner_id) = owner {
                    owners.insert(entity_id.raw(), owner_id);
                }
            }
        }
        let preview_music = match audio_hint {
            AudioPreviewHint::Unknown => visual.music.clone(),
            AudioPreviewHint::Resolved(value) => value,
        };
        if let Some(music) = &preview_music {
            let mut transform = visual_novel_engine::Transform::at(12, 12);
            transform.z_order = 500;
            if let Some(entity_id) = scene.spawn_with_transform(
                transform,
                visual_novel_engine::EntityKind::Audio(visual_novel_engine::AudioData {
                    path: music.clone(),
                    volume: 1000,
                    looping: true,
                }),
            ) {
                let owner = owner_hints
                    .music_owner
                    .or_else(|| graph.first_node_referencing_asset(music.as_ref()));
                if let Some(owner_id) = owner {
                    owners.insert(entity_id.raw(), owner_id);
                }
            }
        }

        PreviewSceneSnapshot { scene, owners }
    }
    fn preview_script_hints(
        engine: &Engine,
        target_ip: Option<u32>,
        graph: &crate::editor::node_graph::NodeGraph,
    ) -> PreviewScriptHints {
        let upper_bound = target_ip.unwrap_or(engine.state().position);
        let mut owner_hints = PreviewOwnerHints::default();
        let mut current_audio = None;
        let mut audio_resolved = false;
        for (idx, event) in engine.script().events.iter().enumerate() {
            let ip = idx as u32;
            if ip > upper_bound {
                break;
            }
            let owner = graph.node_for_event_ip(ip);
            match event {
                visual_novel_engine::EventCompiled::Scene(scene) => {
                    if scene.background.is_some() {
                        owner_hints.background_owner = owner;
                    }
                    if let Some(music) = &scene.music {
                        audio_resolved = true;
                        current_audio = Some(music.clone());
                        owner_hints.music_owner = owner;
                    }
                    if let Some(owner_id) = owner {
                        for character in &scene.characters {
                            owner_hints
                                .character_owners
                                .insert(character.name.to_string(), owner_id);
                        }
                    }
                }
                visual_novel_engine::EventCompiled::Patch(patch) => {
                    if patch.background.is_some() {
                        owner_hints.background_owner = owner;
                    }
                    if let Some(music) = &patch.music {
                        audio_resolved = true;
                        current_audio = Some(music.clone());
                        owner_hints.music_owner = owner;
                    }
                    if let Some(owner_id) = owner {
                        for character in &patch.add {
                            owner_hints
                                .character_owners
                                .insert(character.name.to_string(), owner_id);
                        }
                        for character in &patch.update {
                            owner_hints
                                .character_owners
                                .insert(character.name.to_string(), owner_id);
                        }
                    }
                    for removed_name in &patch.remove {
                        owner_hints.character_owners.remove(removed_name.as_ref());
                    }
                }
                visual_novel_engine::EventCompiled::AudioAction(action) => {
                    if action.channel == 0 {
                        audio_resolved = true;
                        owner_hints.music_owner = owner;
                        match action.action {
                            0 => {
                                if let Some(asset) = &action.asset {
                                    current_audio = Some(asset.clone());
                                }
                            }
                            1 | 2 => current_audio = None,
                            _ => {}
                        }
                    }
                }
                visual_novel_engine::EventCompiled::SetCharacterPosition(pos) => {
                    if let Some(owner_id) = owner {
                        owner_hints
                            .character_owners
                            .insert(pos.name.to_string(), owner_id);
                    }
                }
                visual_novel_engine::EventCompiled::Dialogue(_) => {}
                _ => {}
            }
        }

        PreviewScriptHints {
            owner_hints,
            audio_hint: if audio_resolved {
                AudioPreviewHint::Resolved(current_audio)
            } else {
                AudioPreviewHint::Unknown
            },
        }
    }
}

struct PreviewSceneSnapshot {
    scene: visual_novel_engine::SceneState,
    owners: HashMap<u32, u32>,
}

#[derive(Default)]
struct PreviewOwnerHints {
    background_owner: Option<u32>,
    music_owner: Option<u32>,
    character_owners: BTreeMap<String, u32>,
}

struct PreviewScriptHints {
    owner_hints: PreviewOwnerHints,
    audio_hint: AudioPreviewHint,
}

enum AudioPreviewHint {
    Unknown,
    Resolved(Option<visual_novel_engine::SharedStr>),
}
