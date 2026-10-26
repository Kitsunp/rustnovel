use std::io;

use serde::Serialize;
use sha2::{Digest, Sha256};

pub(crate) fn hex_digest(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    hex_digest(&Sha256::digest(bytes))
}

/// Hashes the same compact JSON bytes as `to_vec`, without retaining that buffer.
pub(crate) fn sha256_json<T: Serialize + ?Sized>(value: &T) -> serde_json::Result<String> {
    let mut writer = Sha256Writer(Sha256::new());
    serde_json::to_writer(&mut writer, value)?;
    Ok(hex_digest(&writer.0.finalize()))
}

pub(crate) struct Sha256Writer(pub(crate) Sha256);

impl io::Write for Sha256Writer {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.update(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn streamed_json_hash_matches_compact_wire_bytes() {
        for value in [
            serde_json::Value::Null,
            serde_json::json!({"speaker": "Ava", "text": "Español\n\"🙂\""}),
            serde_json::json!({"nodes": [0, 255, 65535], "nested": {"flag": false}}),
        ] {
            let bytes = serde_json::to_vec(&value).unwrap();
            let expected = format!("{:x}", Sha256::digest(&bytes));
            assert_eq!(sha256_json(&value).unwrap(), expected);
            assert_eq!(sha256_hex(&bytes), expected);
        }
    }
}
