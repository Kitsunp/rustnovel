use std::fs::File;
use std::io::Read;
use std::path::Path;

use sha2::{Digest, Sha256};

use crate::AssetError;

/// Reads an already resolved path with a byte limit, including files that grow
/// during the read. Callers remain responsible for path and manifest checks.
pub fn read_asset_bytes(path: &Path, max_bytes: u64) -> Result<Vec<u8>, AssetError> {
    let file = open_asset(path, max_bytes)?;
    let mut bytes = Vec::new();
    file.take(max_bytes.saturating_add(1))
        .read_to_end(&mut bytes)?;
    let size = bytes.len() as u64;
    check_size(size, max_bytes)?;
    Ok(bytes)
}

/// Hashes an already resolved file without buffering its payload, enforcing the
/// same limit as byte reads even if the file grows while it is being inspected.
pub fn hash_asset_file(path: &Path, max_bytes: u64) -> Result<(String, u64), AssetError> {
    let mut reader = open_asset(path, max_bytes)?.take(max_bytes.saturating_add(1));
    let mut hasher = Sha256::new();
    let mut total = 0u64;
    let mut buffer = [0u8; 16 * 1024];
    loop {
        let count = reader.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        total = total.saturating_add(count as u64);
        check_size(total, max_bytes)?;
        hasher.update(&buffer[..count]);
    }
    Ok((format!("{:x}", hasher.finalize()), total))
}

fn open_asset(path: &Path, max_bytes: u64) -> Result<File, AssetError> {
    let file = File::open(path)?;
    check_size(file.metadata()?.len(), max_bytes)?;
    Ok(file)
}

fn check_size(size: u64, max: u64) -> Result<(), AssetError> {
    if size > max {
        Err(AssetError::TooLarge { size, max })
    } else {
        Ok(())
    }
}
