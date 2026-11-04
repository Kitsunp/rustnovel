use std::fs::File;
use std::io::Read;
use std::path::Path;

use crate::engine::Engine;
use crate::error::{VnError, VnResult};
use crate::version::SCRIPT_BINARY_MAGIC;
use crate::{ResourceLimiter, SecurityPolicy};

use super::{ScriptCompiled, ScriptRaw};

/// Runtime input retains its format until engine initialization, so hosts can
/// report parsing failures separately from policy and execution failures.
#[derive(Debug)]
pub enum RuntimeScriptInput {
    Json(ScriptRaw),
    Compiled(ScriptCompiled),
}

impl RuntimeScriptInput {
    pub fn parse(bytes: &[u8], limits: ResourceLimiter) -> VnResult<Self> {
        if bytes.len() > limits.max_script_bytes {
            return Err(VnError::resource_limit("runtime script input budget"));
        }
        if bytes.starts_with(&SCRIPT_BINARY_MAGIC) {
            return ScriptCompiled::from_binary(bytes).map(Self::Compiled);
        }
        let source = std::str::from_utf8(bytes).map_err(|error| {
            VnError::invalid_script(format!("runtime script is not UTF-8: {error}"))
        })?;
        ScriptRaw::from_json_with_limits(source, limits).map(Self::Json)
    }

    pub fn load(path: impl AsRef<Path>, limits: ResourceLimiter) -> VnResult<Self> {
        let path = path.as_ref();
        let file = File::open(path).map_err(|error| {
            VnError::invalid_script(format!("read runtime script '{}': {error}", path.display()))
        })?;
        Self::from_reader(file, limits)
    }

    /// Reads at most the input budget plus one byte, including from growing files.
    pub fn from_reader(reader: impl Read, limits: ResourceLimiter) -> VnResult<Self> {
        let mut bytes = Vec::new();
        reader
            .take((limits.max_script_bytes as u64).saturating_add(1))
            .read_to_end(&mut bytes)
            .map_err(|error| VnError::invalid_script(format!("read runtime script: {error}")))?;
        Self::parse(&bytes, limits)
    }

    /// Validates either format before creating runtime state or publishing a binary.
    pub fn into_compiled(
        self,
        policy: &SecurityPolicy,
        limits: ResourceLimiter,
    ) -> VnResult<ScriptCompiled> {
        let script = match self {
            Self::Json(script) => {
                policy.validate_raw(&script, limits)?;
                script.compile()?
            }
            Self::Compiled(script) => script,
        };
        policy.validate_compiled(&script, limits)?;
        Ok(script)
    }

    pub fn into_engine(self, policy: SecurityPolicy, limits: ResourceLimiter) -> VnResult<Engine> {
        Engine::from_input(self, policy, limits)
    }
}
