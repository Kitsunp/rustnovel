//! Visual state handling for scenes.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::error::{VnError, VnResult};
use crate::event::{
    CharacterPlacementCompiled, ScenePatchCompiled, SceneUpdateCompiled,
    SetCharacterPositionCompiled, SharedStr,
};

/// Current visual state for rendering.
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
pub struct VisualState {
    pub background: Option<SharedStr>,
    pub music: Option<SharedStr>,
    pub characters: Vec<CharacterPlacementCompiled>,
}

impl VisualState {
    /// Applies a scene update to the visual state.
    ///
    /// Note: Scene events preserve existing values when fields are None.
    /// To fully replace/clear values, use Patch events with explicit null.
    pub fn apply_scene(&mut self, update: &SceneUpdateCompiled) {
        if let Some(background) = &update.background {
            self.background = Some(background.clone());
        }
        if let Some(music) = &update.music {
            self.music = Some(music.clone());
        }
        if !update.characters.is_empty() {
            self.characters.clear();
            self.characters.extend_from_slice(&update.characters);
        }
    }

    /// Applies a partial scene patch to the visual state.
    pub fn apply_patch(&mut self, patch: &ScenePatchCompiled) {
        if let Some(background) = &patch.background {
            if background.as_ref().is_empty() {
                self.background = None;
            } else {
                self.background = Some(background.clone());
            }
        }
        if let Some(music) = &patch.music {
            if music.as_ref().is_empty() {
                self.music = None;
            } else {
                self.music = Some(music.clone());
            }
        }
        if !patch.remove.is_empty() {
            let remove = patch
                .remove
                .iter()
                .map(|name| name.as_ref())
                .collect::<HashSet<_>>();
            self.characters
                .retain(|character| !remove.contains(character.name.as_ref()));
        }
        for patch_update in &patch.update {
            if let Some(existing) =
                unique_character_by_name_mut(&mut self.characters, patch_update.name.as_ref())
            {
                if let Some(expression) = &patch_update.expression {
                    existing.expression = Some(expression.clone());
                }
                if let Some(position) = &patch_update.position {
                    existing.position = Some(position.clone());
                }
                if patch_update.x.is_some() {
                    existing.x = patch_update.x;
                }
                if patch_update.y.is_some() {
                    existing.y = patch_update.y;
                }
                if patch_update.scale.is_some() {
                    existing.scale = patch_update.scale;
                }
            }
        }
        if !patch.add.is_empty() {
            for new_character in &patch.add {
                match self
                    .characters
                    .iter_mut()
                    .find(|entry| same_character_instance(entry, new_character))
                {
                    Some(existing) => {
                        existing.expression = new_character.expression.clone();
                        existing.position = new_character.position.clone();
                        existing.x = new_character.x;
                        existing.y = new_character.y;
                        existing.scale = new_character.scale;
                    }
                    None => self.characters.push(new_character.clone()),
                }
            }
        }
    }

    /// Sets a character's absolute position and scale.
    pub fn set_character_position(&mut self, pos: &SetCharacterPositionCompiled) -> VnResult<()> {
        let matching_count = character_name_match_count(&self.characters, pos.name.as_ref());
        if matching_count > 1 {
            return Err(VnError::InvalidScript(format!(
                "ambiguous character position update for '{}' matched {matching_count} instances",
                pos.name.as_ref()
            )));
        }
        if let Some(existing) =
            unique_character_by_name_mut(&mut self.characters, pos.name.as_ref())
        {
            existing.x = Some(pos.x);
            existing.y = Some(pos.y);
            existing.scale = pos.scale;
            return Ok(());
        }

        self.characters.push(CharacterPlacementCompiled {
            name: pos.name.clone(),
            expression: None,
            position: None,
            x: Some(pos.x),
            y: Some(pos.y),
            scale: pos.scale,
        });
        Ok(())
    }
}

fn same_character_instance(
    left: &CharacterPlacementCompiled,
    right: &CharacterPlacementCompiled,
) -> bool {
    left.name.as_ref() == right.name.as_ref() && left.expression == right.expression
}

fn character_name_match_count(characters: &[CharacterPlacementCompiled], name: &str) -> usize {
    characters
        .iter()
        .filter(|entry| entry.name.as_ref() == name)
        .take(2)
        .count()
}

fn unique_character_by_name_mut<'a>(
    characters: &'a mut [CharacterPlacementCompiled],
    name: &str,
) -> Option<&'a mut CharacterPlacementCompiled> {
    let mut matches = characters
        .iter()
        .enumerate()
        .filter(|(_, entry)| entry.name.as_ref() == name);
    let (index, _) = matches.next()?;
    if matches.next().is_some() {
        return None;
    }
    characters.get_mut(index)
}
