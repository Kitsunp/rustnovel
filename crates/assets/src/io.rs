use std::fs::File;
use std::io::Read;
use std::path::Path;

use crate::AssetError;

/// Reads an already resolved path with a byte limit, including files that grow
/// during the read. Callers remain responsible for path and manifest checks.
pub fn read_asset_bytes(path: &Path, max_bytes: u64) -> Result<Vec<u8>, AssetError> {
    let file = File::open(path)?;
    let size = file.metadata()?.len();
    if size > max_bytes {
        return Err(AssetError::TooLarge {
            size,
            max: max_bytes,
        });
    }
    let mut bytes = Vec::new();
    file.take(max_bytes.saturating_add(1))
        .read_to_end(&mut bytes)?;
    let size = bytes.len() as u64;
    if size > max_bytes {
        return Err(AssetError::TooLarge {
            size,
            max: max_bytes,
        });
    }
    Ok(bytes)
}
