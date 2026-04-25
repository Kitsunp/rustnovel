use std::collections::BTreeMap;

use crate::error::{VnError, VnResult};
use crate::event::EventCompiled;
use crate::version::{COMPILED_FORMAT_VERSION, SCRIPT_BINARY_MAGIC};

/// Runtime-ready script that resolves labels and interns strings.
///
/// # Binary Format
/// Uses Postcard serialization (compact, no-std compatible) with:
/// - Magic bytes (4): "VNSC"
/// - Version (2): LE u16
/// - Checksum (4): CRC32 of payload
/// - Length (4): LE u32
/// - Payload: Postcard-serialized data
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ScriptCompiled {
    pub events: Vec<EventCompiled>,
    pub labels: BTreeMap<String, u32>,
    pub start_ip: u32,
    pub flag_count: u32,
}

impl ScriptCompiled {
    /// Serializes the compiled script to a binary format with magic bytes, version, and checksum.
    ///
    /// # Preconditions
    /// - `self` must be a valid, complete ScriptCompiled instance.
    ///
    /// # Postconditions
    /// - Returns a binary blob that can be deserialized with `from_binary`.
    /// - The binary is prefixed with magic bytes and includes a checksum.
    pub fn to_binary(&self) -> VnResult<Vec<u8>> {
        let payload = postcard::to_allocvec(self).map_err(binary_serialize_error)?;
        let checksum = crc32fast::hash(&payload);
        let payload_len = u32::try_from(payload.len()).map_err(|_| {
            VnError::BinaryFormat("compiled script too large for binary format".to_string())
        })?;
        let mut output = Vec::with_capacity(4 + 2 + 4 + 4 + payload.len());
        output.extend_from_slice(&SCRIPT_BINARY_MAGIC);
        output.extend_from_slice(&COMPILED_FORMAT_VERSION.to_le_bytes());
        output.extend_from_slice(&checksum.to_le_bytes());
        output.extend_from_slice(&payload_len.to_le_bytes());
        output.extend_from_slice(&payload);
        Ok(output)
    }

    /// Deserializes a compiled script from binary data.
    ///
    /// # Preconditions
    /// - `input` must be a valid binary produced by `to_binary`.
    /// - The version must match `COMPILED_FORMAT_VERSION`.
    ///
    /// # Postconditions
    /// - Returns a fully reconstructed `ScriptCompiled`.
    ///
    /// # Errors
    /// - `VnError::BinaryFormat` if magic bytes, version, or checksum are invalid.
    pub fn from_binary(input: &[u8]) -> VnResult<Self> {
        if input.len() < 14 {
            return Err(binary_format_error("binary payload too small"));
        }
        if input[0..4] != SCRIPT_BINARY_MAGIC {
            return Err(binary_format_error("missing script magic bytes"));
        }
        let version = u16::from_le_bytes([input[4], input[5]]);
        if version != COMPILED_FORMAT_VERSION {
            return Err(binary_format_error(format!(
                "unsupported script version {version} (expected {COMPILED_FORMAT_VERSION})"
            )));
        }
        let checksum = u32::from_le_bytes([input[6], input[7], input[8], input[9]]);
        let payload_len = u32::from_le_bytes([input[10], input[11], input[12], input[13]]) as usize;
        let payload = input
            .get(14..)
            .ok_or_else(|| binary_format_error("missing payload"))?;
        if payload.len() != payload_len {
            return Err(binary_format_error("payload length mismatch"));
        }
        let payload_checksum = crc32fast::hash(payload);
        if payload_checksum != checksum {
            return Err(binary_format_error("payload checksum mismatch"));
        }
        postcard::from_bytes(payload).map_err(binary_serialize_error)
    }
}

#[cold]
#[inline(never)]
fn binary_format_error(message: impl Into<String>) -> VnError {
    VnError::BinaryFormat(message.into())
}

#[cold]
#[inline(never)]
fn binary_serialize_error(error: impl std::fmt::Display) -> VnError {
    VnError::BinaryFormat(format!("binary serialization error: {error}"))
}
