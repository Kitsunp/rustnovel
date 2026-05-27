use std::collections::BTreeSet;

use crate::event::{CharacterPatchRaw, CharacterPlacementRaw, EventRaw, ScenePatchRaw};
use crate::script::ScriptRaw;

#[derive(Default)]
pub(crate) struct AssetRefSet {
    refs: BTreeSet<String>,
}

impl AssetRefSet {
    pub(crate) fn push_optional(&mut self, value: &Option<String>) {
        if let Some(value) = value
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            self.refs.insert(value.to_string());
        }
    }

    pub(crate) fn push(&mut self, value: &str) {
        let value = value.trim();
        if !value.is_empty() {
            self.refs.insert(value.to_string());
        }
    }

    pub(crate) fn into_vec(self) -> Vec<String> {
        self.refs.into_iter().collect()
    }
}

pub(crate) fn collect_script_asset_refs(script: &ScriptRaw) -> Vec<String> {
    let mut refs = AssetRefSet::default();
    for event in &script.events {
        collect_event_asset_refs(event, &mut refs);
    }
    refs.into_vec()
}

pub(crate) fn collect_event_asset_refs(event: &EventRaw, refs: &mut AssetRefSet) {
    match event {
        EventRaw::Scene(scene) => {
            refs.push_optional(&scene.background);
            refs.push_optional(&scene.music);
            collect_character_assets(&scene.characters, refs);
        }
        EventRaw::Patch(patch) => collect_scene_patch_asset_refs(patch, refs),
        EventRaw::AudioAction(action) => refs.push_optional(&action.asset),
        _ => {}
    }
}

pub(crate) fn collect_scene_patch_asset_refs(patch: &ScenePatchRaw, refs: &mut AssetRefSet) {
    refs.push_optional(&patch.background);
    refs.push_optional(&patch.music);
    collect_character_assets(&patch.add, refs);
    collect_character_patch_assets(&patch.update, refs);
}

pub(crate) fn collect_character_assets(
    characters: &[CharacterPlacementRaw],
    refs: &mut AssetRefSet,
) {
    for character in characters {
        refs.push_optional(&character.expression);
    }
}

fn collect_character_patch_assets(characters: &[CharacterPatchRaw], refs: &mut AssetRefSet) {
    for character in characters {
        refs.push_optional(&character.expression);
    }
}
