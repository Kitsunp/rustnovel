use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::resource::StringBudget;

use super::SharedStr;

/// Dialogue line with speaker and text in raw form.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default, JsonSchema)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub struct DialogueRaw {
    pub speaker: String,
    pub text: String,
}

impl StringBudget for DialogueRaw {
    fn string_bytes(&self) -> usize {
        self.speaker.string_bytes() + self.text.string_bytes()
    }
}

/// Dialogue line with interned speaker and text.
#[derive(Clone, Debug, Serialize, Deserialize, Default, JsonSchema)]
pub struct DialogueCompiled {
    pub speaker: SharedStr,
    pub text: SharedStr,
}
