//! Runtime content protection primitives.
//!
//! This is a compact encrypt-then-MAC stream built from HMAC-SHA256 so text and
//! graph payloads do not need to be stored as plain JSON in packaged builds.
//! It raises extraction cost, but it is not DRM: a fully client-side game must
//! reconstruct plaintext in memory to render it.

use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

pub const PROTECTED_CONTENT_VERSION: u32 = 1;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtectedContentChunk {
    pub version: u32,
    pub domain: String,
    pub salt: [u8; 16],
    pub nonce: u64,
    pub ciphertext: Vec<u8>,
    pub tag: [u8; 32],
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProtectedContentError {
    AuthenticationFailed,
    UnsupportedVersion(u32),
}

impl std::fmt::Display for ProtectedContentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AuthenticationFailed => write!(f, "protected content authentication failed"),
            Self::UnsupportedVersion(version) => {
                write!(f, "unsupported protected content version {version}")
            }
        }
    }
}

impl std::error::Error for ProtectedContentError {}

pub fn protect_content(
    key: &[u8],
    domain: &str,
    salt: [u8; 16],
    nonce: u64,
    plaintext: &[u8],
) -> ProtectedContentChunk {
    let ciphertext = xor_keystream(key, domain, &salt, nonce, plaintext);
    let tag = auth_tag(key, domain, &salt, nonce, &ciphertext);
    ProtectedContentChunk {
        version: PROTECTED_CONTENT_VERSION,
        domain: domain.to_string(),
        salt,
        nonce,
        ciphertext,
        tag,
    }
}

pub fn open_protected_content(
    key: &[u8],
    chunk: &ProtectedContentChunk,
) -> Result<Vec<u8>, ProtectedContentError> {
    if chunk.version != PROTECTED_CONTENT_VERSION {
        return Err(ProtectedContentError::UnsupportedVersion(chunk.version));
    }
    let expected = auth_tag(
        key,
        &chunk.domain,
        &chunk.salt,
        chunk.nonce,
        &chunk.ciphertext,
    );
    if expected != chunk.tag {
        return Err(ProtectedContentError::AuthenticationFailed);
    }
    Ok(xor_keystream(
        key,
        &chunk.domain,
        &chunk.salt,
        chunk.nonce,
        &chunk.ciphertext,
    ))
}

fn xor_keystream(key: &[u8], domain: &str, salt: &[u8; 16], nonce: u64, input: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(input.len());
    for (block_index, block) in input.chunks(32).enumerate() {
        let stream = stream_block(key, domain, salt, nonce, block_index as u64);
        out.extend(block.iter().zip(stream).map(|(byte, mask)| byte ^ mask));
    }
    out
}

fn stream_block(key: &[u8], domain: &str, salt: &[u8; 16], nonce: u64, counter: u64) -> [u8; 32] {
    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC accepts any key length");
    mac.update(b"vnengine.content.stream.v1");
    mac.update(domain.as_bytes());
    mac.update(salt);
    mac.update(&nonce.to_le_bytes());
    mac.update(&counter.to_le_bytes());
    mac.finalize().into_bytes().into()
}

fn auth_tag(key: &[u8], domain: &str, salt: &[u8; 16], nonce: u64, ciphertext: &[u8]) -> [u8; 32] {
    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC accepts any key length");
    mac.update(b"vnengine.content.tag.v1");
    mac.update(domain.as_bytes());
    mac.update(salt);
    mac.update(&nonce.to_le_bytes());
    mac.update(&(ciphertext.len() as u64).to_le_bytes());
    mac.update(ciphertext);
    mac.finalize().into_bytes().into()
}

#[cfg(test)]
#[path = "tests/protected_content_tests.rs"]
mod tests;
