use std::collections::BTreeSet;

use crate::authoring::StoryNode;
use crate::event::{CharacterPlacementRaw, EventRaw};
use crate::event_behavior::{event_asset_refs_for_raw, node_asset_refs_for_authoring_node};
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
    for asset_ref in event_asset_refs_for_raw(event) {
        refs.push(&asset_ref);
    }
}

pub(crate) fn collect_node_asset_refs(node: &StoryNode, refs: &mut AssetRefSet) {
    for asset_ref in node_asset_refs_for_authoring_node(node) {
        refs.push(&asset_ref);
    }
}

pub(crate) fn collect_character_assets(
    characters: &[CharacterPlacementRaw],
    refs: &mut AssetRefSet,
) {
    for character in characters {
        refs.push_optional(&character.expression);
    }
}
