use pyo3::prelude::*;
use visual_novel_engine::runtime::{CharacterPatchRaw, CharacterPlacementRaw};

/// Keep legacy tuples while exposing the same coordinates as core authoring.
#[derive(FromPyObject)]
pub(super) enum CharacterInput {
    Full(
        String,
        Option<String>,
        Option<String>,
        Option<i32>,
        Option<i32>,
        Option<f32>,
    ),
    Legacy(String, Option<String>, Option<String>),
}

impl From<CharacterInput> for CharacterPlacementRaw {
    fn from(input: CharacterInput) -> Self {
        let (name, expression, position, x, y, scale) = match input {
            CharacterInput::Full(name, expression, position, x, y, scale) => {
                (name, expression, position, x, y, scale)
            }
            CharacterInput::Legacy(name, expression, position) => {
                (name, expression, position, None, None, None)
            }
        };
        Self {
            name,
            expression,
            position,
            x,
            y,
            scale,
        }
    }
}

impl From<CharacterInput> for CharacterPatchRaw {
    fn from(input: CharacterInput) -> Self {
        let CharacterPlacementRaw {
            name,
            expression,
            position,
            x,
            y,
            scale,
        } = input.into();
        Self {
            name,
            expression,
            position,
            x,
            y,
            scale,
        }
    }
}
