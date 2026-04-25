use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::resource::StringBudget;

/// Condition for conditional jumps (raw form).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CondRaw {
    Flag { key: String, is_set: bool },
    VarCmp { key: String, op: CmpOp, value: i32 },
}

impl StringBudget for CondRaw {
    fn string_bytes(&self) -> usize {
        match self {
            CondRaw::Flag { key, .. } => key.string_bytes(),
            CondRaw::VarCmp { key, .. } => key.string_bytes(),
        }
    }
}

/// Condition for conditional jumps (compiled form).
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub enum CondCompiled {
    Flag { flag_id: u32, is_set: bool },
    VarCmp { var_id: u32, op: CmpOp, value: i32 },
}

/// Comparison operators for variable conditions.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[serde(rename_all = "snake_case")]
pub enum CmpOp {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}
