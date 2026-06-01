use super::*;

#[derive(Default)]
pub(super) struct AssetRefCollector {
    refs: BTreeSet<String>,
}

impl AssetRefCollector {
    pub(super) fn push_optional(&mut self, value: &Option<String>) {
        if let Some(value) = value
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            self.refs.insert(value.to_string());
        }
    }

    pub(super) fn extend(&mut self, values: Vec<String>) {
        for value in values {
            self.push(&value);
        }
    }

    pub(super) fn push(&mut self, value: &str) {
        let value = value.trim();
        if !value.is_empty() {
            self.refs.insert(value.to_string());
        }
    }

    pub(super) fn into_vec(self) -> Vec<String> {
        self.refs.into_iter().collect()
    }
}

pub(super) fn collect_scene_patch_asset_refs(
    patch: &crate::event::ScenePatchRaw,
    refs: &mut AssetRefCollector,
) {
    refs.push_optional(&patch.background);
    refs.push_optional(&patch.music);
    collect_character_asset_refs(&patch.add, refs);
    for character in &patch.update {
        refs.push_optional(&character.expression);
    }
}

pub(super) fn collect_character_asset_refs(
    characters: &[crate::event::CharacterPlacementRaw],
    refs: &mut AssetRefCollector,
) {
    for character in characters {
        refs.push_optional(&character.expression);
    }
}
