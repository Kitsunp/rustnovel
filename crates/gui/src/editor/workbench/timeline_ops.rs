use std::collections::HashMap;

use super::EditorWorkbench;
use visual_novel_engine::EntityKind;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct TimelineBinding {
    owner: Option<u32>,
    object_id: Option<String>,
    kind: EntityKind,
    // Without an authoring object ID, a rebuilt scene cannot prove identity.
    scene_generation: Option<u64>,
}

impl EditorWorkbench {
    pub(super) fn timeline_scene_identities(&self) -> HashMap<u32, TimelineBinding> {
        let object_ids =
            crate::editor::visual_composer::layers::authoring_entity_object_ids_from_layers(
                &self.scene,
                &self.composer_entity_owners,
                self.node_graph
                    .authoring_session
                    .read_model()
                    .composer_layers(),
            );
        self.timeline_scene_identities_with_object_ids(&object_ids)
    }

    pub(super) fn timeline_scene_identities_with_object_ids(
        &self,
        object_ids: &HashMap<u32, String>,
    ) -> HashMap<u32, TimelineBinding> {
        let targets = self
            .timeline
            .tracks()
            .map(|track| track.target.raw())
            .collect::<std::collections::HashSet<_>>();
        let mut identities = HashMap::new();
        for entity in self.scene.iter() {
            let raw = entity.id.raw();
            if !targets.contains(&raw) {
                continue;
            }
            let owner = self.composer_entity_owners.get(&raw).copied();
            // Equal objects from the same source cannot safely be distinguished
            // after one of them is removed or reordered.
            if self
                .scene
                .iter()
                .filter(|other| {
                    self.composer_entity_owners.get(&other.id.raw()).copied() == owner
                        && other.kind == entity.kind
                })
                .count()
                != 1
            {
                continue;
            }
            let object_id = object_ids.get(&raw).cloned();
            identities.insert(
                raw,
                TimelineBinding {
                    owner,
                    scene_generation: object_id
                        .is_none()
                        .then_some(self.timeline_scene_generation),
                    object_id,
                    kind: entity.kind.clone(),
                },
            );
        }
        identities
    }

    pub(super) fn capture_timeline_bindings(&mut self) {
        // Public direct clear-and-readd of the same ID between captures is
        // indistinguishable from retaining the original track.
        let targets = self
            .timeline
            .tracks()
            .map(|track| track.target.raw())
            .collect::<std::collections::HashSet<_>>();
        self.timeline_bindings.retain(|id, _| targets.contains(id));
        if targets
            .iter()
            .all(|id| self.timeline_bindings.contains_key(id))
        {
            return;
        }
        let identities = self.timeline_scene_identities();
        for id in targets {
            self.timeline_bindings
                .entry(id)
                .or_insert_with(|| identities.get(&id).cloned());
        }
    }

    pub(super) fn timeline_binding_matches(
        &self,
        id: u32,
        identities: &HashMap<u32, TimelineBinding>,
    ) -> bool {
        match self.timeline_bindings.get(&id) {
            Some(Some(expected)) => identities.get(&id) == Some(expected),
            Some(None) => false,
            None => identities.contains_key(&id),
        }
    }

    pub(super) fn suspended_timeline_tracks(&self) -> usize {
        if self.timeline.track_count() == 0 {
            return 0;
        }
        let identities = self.timeline_scene_identities();
        self.timeline
            .tracks()
            .filter(|track| !self.timeline_binding_matches(track.target.raw(), &identities))
            .count()
    }

    pub(super) fn before_rebuilding_preview_scene(&mut self) {
        self.capture_timeline_bindings();
        self.timeline_scene_generation = self.timeline_scene_generation.wrapping_add(1);
    }

    pub(super) fn reset_document_timeline(&mut self) {
        self.timeline = visual_novel_engine::Timeline::new(60);
        self.timeline_bindings.clear();
        self.current_time = 0.0;
        self.timeline_fractional_ticks = 0.0;
        self.is_playing = false;
    }
}
