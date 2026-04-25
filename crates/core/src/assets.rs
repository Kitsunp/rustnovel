use std::hash::{Hash, Hasher};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Opaque asset identifier.
///
/// Uses a deterministic non-cryptographic u64 hash for stable IDs across runs.
/// This is not collision-resistant in the cryptographic sense.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct AssetId(u64);

impl AssetId {
    /// Creates an `AssetId` from a path string using deterministic FNV-1a 64-bit.
    ///
    /// Intended for stable lookup keys, not for security decisions.
    pub fn from_path(path: &str) -> Self {
        let mut hasher = FnvHasher64::default();
        path.hash(&mut hasher);
        AssetId(hasher.finish())
    }

    /// Returns the raw u64 value for serialization purposes only.
    pub fn as_u64(&self) -> u64 {
        self.0
    }

    /// Returns an upgraded 128-bit deterministic identifier derived from the same path.
    ///
    /// This supports incremental migration away from 64-bit IDs while preserving
    /// backward compatibility in existing save formats and bindings.
    pub fn strong_id_from_path(path: &str) -> AssetId128 {
        AssetId128::from_path(path)
    }
}

/// Strong deterministic 128-bit asset identifier (SHA-256 truncated to 16 bytes).
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct AssetId128([u8; 16]);

impl AssetId128 {
    pub fn from_path(path: &str) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(path.as_bytes());
        let digest = hasher.finalize();
        let mut bytes = [0u8; 16];
        bytes.copy_from_slice(&digest[..16]);
        Self(bytes)
    }

    pub fn as_bytes(&self) -> &[u8; 16] {
        &self.0
    }
}

/// Fowler-Noll-Vo 1a 64-bit Hasher.
/// Used for deterministic AssetId generation independent of process seed.
struct FnvHasher64 {
    state: u64,
}

impl Default for FnvHasher64 {
    fn default() -> Self {
        Self {
            state: 0xcbf29ce484222325,
        }
    }
}

impl Hasher for FnvHasher64 {
    fn write(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            self.state ^= u64::from(byte);
            self.state = self.state.wrapping_mul(0x100000001b3);
        }
    }

    fn finish(&self) -> u64 {
        self.state
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
pub struct AssetManifest {
    pub entries: std::collections::BTreeMap<String, String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn asset_id_128_is_deterministic() {
        let a = AssetId128::from_path("bg/room.png");
        let b = AssetId128::from_path("bg/room.png");
        assert_eq!(a, b);
    }

    #[test]
    fn asset_id_128_distinguishes_different_paths() {
        let a = AssetId128::from_path("bg/room.png");
        let b = AssetId128::from_path("bg/forest.png");
        assert_ne!(a, b);
    }
}
