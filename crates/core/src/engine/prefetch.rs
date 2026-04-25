use std::collections::HashSet;

use crate::assets::AssetId;
use crate::event::EventCompiled;

use super::runtime::Engine;

impl Engine {
    /// Returns unique upcoming asset paths that can be prefetched safely.
    ///
    /// This intentionally excludes non-path semantic fields to avoid prefetching invalid resources.
    pub fn peek_next_asset_paths(&self, depth: usize) -> Vec<String> {
        let mut seen = HashSet::new();
        let mut paths = Vec::new();
        let start = self.state().position as usize;
        let end = (start + depth).min(self.script().events.len());
        for event in &self.script().events[start..end] {
            collect_prefetch_paths_from_event(event, &mut seen, &mut paths);
        }
        paths
    }

    /// Returns the unique upcoming asset ids that can be prefetched safely.
    pub fn peek_next_assets(&self, depth: usize) -> Vec<AssetId> {
        let mut seen = HashSet::new();
        let mut assets = Vec::new();
        let start = self.state().position as usize;
        let end = (start + depth).min(self.script().events.len());
        for event in &self.script().events[start..end] {
            match event {
                EventCompiled::Scene(scene) => {
                    if let Some(background) = &scene.background {
                        let id = AssetId::from_path(background.as_ref());
                        if seen.insert(id) {
                            assets.push(id);
                        }
                    }
                    if let Some(music) = &scene.music {
                        let id = AssetId::from_path(music.as_ref());
                        if seen.insert(id) {
                            assets.push(id);
                        }
                    }
                    for character in &scene.characters {
                        let id = AssetId::from_path(character.name.as_ref());
                        if seen.insert(id) {
                            assets.push(id);
                        }
                        if let Some(expression) = &character.expression {
                            let id = AssetId::from_path(expression.as_ref());
                            if seen.insert(id) {
                                assets.push(id);
                            }
                        }
                    }
                }
                EventCompiled::Patch(patch) => {
                    if let Some(background) = &patch.background {
                        let id = AssetId::from_path(background.as_ref());
                        if seen.insert(id) {
                            assets.push(id);
                        }
                    }
                    if let Some(music) = &patch.music {
                        let id = AssetId::from_path(music.as_ref());
                        if seen.insert(id) {
                            assets.push(id);
                        }
                    }
                    for character in &patch.add {
                        let id = AssetId::from_path(character.name.as_ref());
                        if seen.insert(id) {
                            assets.push(id);
                        }
                        if let Some(expression) = &character.expression {
                            let id = AssetId::from_path(expression.as_ref());
                            if seen.insert(id) {
                                assets.push(id);
                            }
                        }
                    }
                    for character in &patch.update {
                        let id = AssetId::from_path(character.name.as_ref());
                        if seen.insert(id) {
                            assets.push(id);
                        }
                        if let Some(expression) = &character.expression {
                            let id = AssetId::from_path(expression.as_ref());
                            if seen.insert(id) {
                                assets.push(id);
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        assets
    }
}

fn collect_prefetch_paths_from_event(
    event: &EventCompiled,
    seen: &mut HashSet<String>,
    output: &mut Vec<String>,
) {
    match event {
        EventCompiled::Scene(scene) => {
            if let Some(background) = &scene.background {
                push_unique_prefetch_path(background.as_ref(), seen, output);
            }
            if let Some(music) = &scene.music {
                push_unique_prefetch_path(music.as_ref(), seen, output);
            }
            for character in &scene.characters {
                if let Some(expression) = &character.expression {
                    push_unique_prefetch_path(expression.as_ref(), seen, output);
                }
            }
        }
        EventCompiled::Patch(patch) => {
            if let Some(background) = &patch.background {
                push_unique_prefetch_path(background.as_ref(), seen, output);
            }
            if let Some(music) = &patch.music {
                push_unique_prefetch_path(music.as_ref(), seen, output);
            }
            for character in &patch.add {
                if let Some(expression) = &character.expression {
                    push_unique_prefetch_path(expression.as_ref(), seen, output);
                }
            }
            for character in &patch.update {
                if let Some(expression) = &character.expression {
                    push_unique_prefetch_path(expression.as_ref(), seen, output);
                }
            }
        }
        EventCompiled::AudioAction(action) => {
            if action.action == 0 {
                if let Some(asset) = &action.asset {
                    push_unique_prefetch_path(asset.as_ref(), seen, output);
                }
            }
        }
        _ => {}
    }
}

fn push_unique_prefetch_path(value: &str, seen: &mut HashSet<String>, output: &mut Vec<String>) {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return;
    }
    if seen.insert(trimmed.to_string()) {
        output.push(trimmed.to_string());
    }
}
