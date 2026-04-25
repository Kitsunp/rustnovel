use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::resource::StringBudget;

use super::SharedStr;

/// Scene update payload in raw form.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default, JsonSchema)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub struct SceneUpdateRaw {
    pub background: Option<String>,
    pub music: Option<String>,
    #[serde(default)]
    pub characters: Vec<CharacterPlacementRaw>,
}

impl StringBudget for SceneUpdateRaw {
    fn string_bytes(&self) -> usize {
        self.background.string_bytes() + self.music.string_bytes() + self.characters.string_bytes()
    }
}

/// Scene update payload with interned strings.
#[derive(Clone, Debug, Serialize, Deserialize, Default, JsonSchema)]
pub struct SceneUpdateCompiled {
    pub background: Option<SharedStr>,
    pub music: Option<SharedStr>,
    pub characters: Vec<CharacterPlacementCompiled>,
}

/// Character placement in raw form.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default, JsonSchema)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub struct CharacterPlacementRaw {
    pub name: String,
    pub expression: Option<String>,
    pub position: Option<String>,
    #[serde(default)]
    pub x: Option<i32>,
    #[serde(default)]
    pub y: Option<i32>,
    #[serde(default)]
    pub scale: Option<f32>,
}

impl StringBudget for CharacterPlacementRaw {
    fn string_bytes(&self) -> usize {
        self.name.string_bytes() + self.expression.string_bytes() + self.position.string_bytes()
    }
}

/// Character placement with interned strings.
#[derive(Clone, Debug, Serialize, Deserialize, Default, JsonSchema)]
pub struct CharacterPlacementCompiled {
    pub name: SharedStr,
    pub expression: Option<SharedStr>,
    pub position: Option<SharedStr>,
    pub x: Option<i32>,
    pub y: Option<i32>,
    pub scale: Option<f32>,
}

/// Character patch for partial updates.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default, JsonSchema)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub struct CharacterPatchRaw {
    pub name: String,
    pub expression: Option<String>,
    pub position: Option<String>,
}

impl StringBudget for CharacterPatchRaw {
    fn string_bytes(&self) -> usize {
        self.name.string_bytes() + self.expression.string_bytes() + self.position.string_bytes()
    }
}

/// Character patch with interned strings.
#[derive(Clone, Debug, Serialize, Deserialize, Default, JsonSchema)]
pub struct CharacterPatchCompiled {
    pub name: SharedStr,
    pub expression: Option<SharedStr>,
    pub position: Option<SharedStr>,
}

/// Scene patch in raw form (handling partial updates).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default, JsonSchema)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub struct ScenePatchRaw {
    pub background: Option<String>,
    pub music: Option<String>,
    #[serde(default)]
    pub add: Vec<CharacterPlacementRaw>,
    #[serde(default)]
    pub update: Vec<CharacterPatchRaw>,
    #[serde(default)]
    pub remove: Vec<String>,
}

impl StringBudget for ScenePatchRaw {
    fn string_bytes(&self) -> usize {
        self.background.string_bytes()
            + self.music.string_bytes()
            + self.add.string_bytes()
            + self.update.string_bytes()
            + self.remove.string_bytes()
    }
}

/// Scene patch with interned strings.
#[derive(Clone, Debug, Serialize, Deserialize, Default, JsonSchema)]
pub struct ScenePatchCompiled {
    pub background: Option<SharedStr>,
    pub music: Option<SharedStr>,
    pub add: Vec<CharacterPlacementCompiled>,
    pub update: Vec<CharacterPatchCompiled>,
    pub remove: Vec<SharedStr>,
}

/// Precise character positioning for Visual Composer.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default, JsonSchema)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub struct SetCharacterPositionRaw {
    pub name: String,
    pub x: i32,
    pub y: i32,
    pub scale: Option<f32>,
}

impl StringBudget for SetCharacterPositionRaw {
    fn string_bytes(&self) -> usize {
        self.name.string_bytes()
    }
}

/// Compiled precise character positioning.
#[derive(Clone, Debug, Serialize, Deserialize, Default, JsonSchema)]
pub struct SetCharacterPositionCompiled {
    pub name: SharedStr,
    pub x: i32,
    pub y: i32,
    pub scale: Option<f32>,
}
