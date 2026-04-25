use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use hmac::{Hmac, Mac};
use sha2::Sha256;

use super::{SaveData, SaveError, ScriptId, AUTH_SAVE_MAGIC};

type HmacSha256 = Hmac<Sha256>;

pub(super) fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

pub(super) fn script_id_hex(script_id: &ScriptId) -> String {
    let mut output = String::with_capacity(script_id.len() * 2);
    for byte in script_id {
        use std::fmt::Write as _;
        let _ = write!(&mut output, "{byte:02x}");
    }
    output
}

pub(super) fn chapter_label_hint(save: &SaveData) -> Option<String> {
    let background = save.state.visual.background.as_ref()?;
    let stem = Path::new(background.as_ref()).file_stem()?.to_str()?;
    let cleaned = stem.replace(['_', '-'], " ").trim().to_string();
    if cleaned.is_empty() {
        None
    } else {
        Some(cleaned)
    }
}

pub(super) fn summary_line_hint(save: &SaveData) -> Option<String> {
    let dialogue = save.state.history.back()?;
    let speaker = dialogue.speaker.as_ref().trim();
    let text = dialogue.text.as_ref().trim();
    if text.is_empty() {
        return None;
    }
    let mut line = if speaker.is_empty() {
        text.to_string()
    } else {
        format!("{speaker}: {text}")
    };
    const MAX_CHARS: usize = 96;
    if line.chars().count() > MAX_CHARS {
        let mut truncated = line
            .chars()
            .take(MAX_CHARS.saturating_sub(3))
            .collect::<String>();
        truncated.push_str("...");
        line = truncated;
    }
    Some(line)
}

pub(super) fn backup_path(path: &Path) -> PathBuf {
    let mut output = path.as_os_str().to_os_string();
    output.push(".bak");
    PathBuf::from(output)
}

pub(super) fn is_authenticated_binary(input: &[u8]) -> bool {
    input.starts_with(&AUTH_SAVE_MAGIC)
}

pub(super) fn compute_hmac_sha256(key: &[u8], payload: &[u8]) -> Result<[u8; 32], SaveError> {
    let mut mac = HmacSha256::new_from_slice(key).map_err(|_| SaveError::AuthKeyInvalid)?;
    mac.update(payload);
    let bytes = mac.finalize().into_bytes();
    let mut out = [0u8; 32];
    out.copy_from_slice(bytes.as_slice());
    Ok(out)
}

pub(super) fn verify_hmac_sha256(key: &[u8], payload: &[u8], tag: &[u8]) -> Result<(), SaveError> {
    let mut mac = HmacSha256::new_from_slice(key).map_err(|_| SaveError::AuthKeyInvalid)?;
    mac.update(payload);
    mac.verify_slice(tag)
        .map_err(|_| SaveError::AuthenticationFailed)
}
