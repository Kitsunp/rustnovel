use super::*;

impl EditorWorkbench {
    pub(super) fn clear_runtime_preview(&mut self) {
        self.engine = None;
        self.scene = visual_novel_engine::SceneState::default();
        self.composer_entity_owners.clear();
        self.selected_entity = None;
        self.player_audio_backend = None;
        self.player_audio_root = None;
        self.timeline = visual_novel_engine::Timeline::new(60);
        self.current_time = 0.0;
        self.timeline_fractional_ticks = 0.0;
        self.is_playing = false;
        self.player_state = crate::editor::player_ui::PlayerSessionState::default();
    }

    pub fn prepare_player_mode(&mut self) -> bool {
        if self.node_graph.is_empty() {
            self.clear_runtime_preview();
            self.current_script = None;
            self.toast = Some(ToastState::warning(
                "Abre o crea un proyecto antes de iniciar el modo Player",
            ));
            return false;
        }

        if let Err(err) = self.sync_graph_to_script() {
            self.toast = Some(ToastState::error(format!(
                "No se pudo preparar el Player: {err}"
            )));
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
        self.player_state.menu_initialized = false;
        self.player_state.quick_save_state = None;
        self.ensure_player_audio_backend();
        self.refresh_scene_from_engine_preview();
        true
    }
    pub fn refresh_scene_from_engine_preview(&mut self) {
        use visual_novel_engine::authoring::composer::{compose_scene_snapshot, StageLayerKind};
        use visual_novel_engine::{runtime::CharacterPlacementCompiled, EntityKind};

        if self.composer_preview_mode == crate::editor::ComposerPreviewMode::IsolatedNode {
            self.refresh_scene_from_selected_node();
            return;
        }
        let Some(engine) = self.engine.as_ref() else {
            self.refresh_scene_from_selected_node();
            return;
        };
        let selected = self.selected_node.or(self.node_graph.selected);
        if selected.is_some_and(|id| self.node_graph.event_ip_for_node(id).is_none()) {
            self.refresh_scene_from_selected_node();
            return;
        }
        let snapshot = compose_scene_snapshot(
            self.node_graph.authoring_graph(),
            selected,
            None,
            Some(engine),
            None,
            None,
        );
        let mut visual = visual_novel_engine::runtime::VisualState::default();
        let mut sources = Vec::new();
        let mut music = None;
        for object in &snapshot.objects {
            if let Some(name) = &object.character_name {
                visual.characters.push(CharacterPlacementCompiled {
                    name: name.as_str().into(),
                    expression: object.expression.as_deref().map(Into::into),
                    x: object.x,
                    y: object.y,
                    scale: object.scale,
                    ..Default::default()
                });
                sources.push(object.source_node_id);
            } else if object.kind == StageLayerKind::Background {
                visual.background = object.asset_path.as_deref().map(Into::into);
                sources.insert(0, object.source_node_id);
            } else if object.kind == StageLayerKind::DebugTrace && object.asset_path.is_some() {
                music = Some(object);
            }
        }
        self.scene = crate::editor::scene_stage::scene_from_visual_state(&visual);
        self.composer_entity_owners = self
            .scene
            .iter()
            .zip(sources)
            .filter_map(|(entity, source)| source.map(|node| (entity.id.raw(), node)))
            .collect();
        if let Some(music) = music {
            let (volume, looping) = music
                .source_node_id
                .and_then(|id| self.node_graph.get_node(id))
                .and_then(|node| match node {
                    crate::editor::StoryNode::AudioAction {
                        volume,
                        loop_playback,
                        ..
                    } => Some((volume.unwrap_or(1.0), loop_playback.unwrap_or(true))),
                    _ => None,
                })
                .unwrap_or((1.0, true));
            let mut transform = visual_novel_engine::Transform::at(12, 12);
            transform.z_order = 500;
            if let Some(id) = self.scene.spawn_with_transform(
                transform,
                EntityKind::Audio(visual_novel_engine::AudioData {
                    path: music.asset_path.as_deref().unwrap_or_default().into(),
                    volume: (volume.clamp(0.0, 1.0) * 1000.0) as u32,
                    looping,
                }),
            ) {
                if let Some(owner) = music.source_node_id {
                    self.composer_entity_owners.insert(id.raw(), owner);
                }
            }
        }
        if self.selected_entity.is_some_and(|id| {
            self.scene
                .get(visual_novel_engine::EntityId::new(id))
                .is_none()
        }) {
            self.selected_entity = None;
        }
    }
}
