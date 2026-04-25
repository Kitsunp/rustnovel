use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::resource::StringBudget;

use super::SharedStr;

/// Choice prompt and options in raw form.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default, JsonSchema)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub struct ChoiceRaw {
    pub prompt: String,
    pub options: Vec<ChoiceOptionRaw>,
}

impl StringBudget for ChoiceRaw {
    fn string_bytes(&self) -> usize {
        self.prompt.string_bytes() + self.options.string_bytes()
    }
}

/// Choice prompt and options with pre-resolved targets.
#[derive(Clone, Debug, Serialize, Deserialize, Default, JsonSchema)]
pub struct ChoiceCompiled {
    pub prompt: SharedStr,
    pub options: Vec<ChoiceOptionCompiled>,
}

/// Choice option with label target in raw form.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default, JsonSchema)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub struct ChoiceOptionRaw {
    pub text: String,
    pub target: String,
}

impl StringBudget for ChoiceOptionRaw {
    fn string_bytes(&self) -> usize {
        self.text.string_bytes() + self.target.string_bytes()
    }
}

/// Choice option with pre-resolved target instruction pointer.
#[derive(Clone, Debug, Serialize, Deserialize, Default, JsonSchema)]
pub struct ChoiceOptionCompiled {
    pub text: SharedStr,
    pub target_ip: u32,
}
