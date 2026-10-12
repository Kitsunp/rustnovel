//! Canonical script identity and save data structures.
//!
//! Provides SHA-256 based script identification for save integrity.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

use crate::state::EngineState;
use crate::version::{SAVE_BINARY_MAGIC, SAVE_FORMAT_VERSION};

/// Unique identifier for a compiled script, computed as SHA-256 of its binary representation.
pub type ScriptId = [u8; 32];
pub const AUTH_SAVE_MAGIC: [u8; 4] = *b"VNSA";
pub const AUTH_SAVE_KEY: &[u8] = b"vnengine.save.v1";

#[path = "storage/helpers.rs"]
mod helpers;
use helpers::*;

/// Computes the canonical script_id from compiled script bytes.
pub fn compute_script_id(compiled_bytes: &[u8]) -> ScriptId {
    let mut hasher = Sha256::new();
    hasher.update(compiled_bytes);
    hasher.finalize().into()
}

/// Save data structure with script identity for integrity validation.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SaveData {
    /// SHA-256 of the compiled script this save belongs to.
    pub script_id: ScriptId,
    /// The engine state at the time of saving.
    pub state: EngineState,
}

impl SaveData {
    /// Creates a new save data bundle.
    pub fn new(script_id: ScriptId, state: EngineState) -> Self {
        Self { script_id, state }
    }

    /// Serializes save data to binary format with magic bytes and version.
    pub fn to_binary(&self) -> Result<Vec<u8>, SaveError> {
        let payload =
            postcard::to_allocvec(self).map_err(|e| SaveError::Serialization(e.to_string()))?;
        let checksum = crc32fast::hash(&payload);
        let payload_len = u32::try_from(payload.len()).map_err(|_| SaveError::TooLarge)?;

        let mut output = Vec::with_capacity(4 + 2 + 4 + 4 + payload.len());
        output.extend_from_slice(&SAVE_BINARY_MAGIC);
        output.extend_from_slice(&SAVE_FORMAT_VERSION.to_le_bytes());
        output.extend_from_slice(&checksum.to_le_bytes());
        output.extend_from_slice(&payload_len.to_le_bytes());
        output.extend_from_slice(&payload);
        Ok(output)
    }

    /// Deserializes save data from binary format, validating magic, version, and checksum.
    pub fn from_binary(input: &[u8]) -> Result<Self, SaveError> {
        if input.len() < 14 {
            return Err(SaveError::TooSmall);
        }
        if input[0..4] != SAVE_BINARY_MAGIC {
            return Err(SaveError::InvalidMagic);
        }
        let version = u16::from_le_bytes([input[4], input[5]]);
        if version != SAVE_FORMAT_VERSION {
            return Err(SaveError::IncompatibleVersion {
                found: version,
                expected: SAVE_FORMAT_VERSION,
            });
        }
        let checksum = u32::from_le_bytes([input[6], input[7], input[8], input[9]]);
        let payload_len = u32::from_le_bytes([input[10], input[11], input[12], input[13]]) as usize;
        let payload = input.get(14..).ok_or(SaveError::MissingPayload)?;
        if payload.len() != payload_len {
            return Err(SaveError::LengthMismatch);
        }
        if crc32fast::hash(payload) != checksum {
            return Err(SaveError::ChecksumMismatch);
        }
        postcard::from_bytes(payload).map_err(|e| SaveError::Serialization(e.to_string()))
    }

    /// Serializes save data to authenticated binary format.
    ///
    /// This wraps the regular save payload with a MAC to detect tampering.
    pub fn to_authenticated_binary(&self, key: &[u8]) -> Result<Vec<u8>, SaveError> {
        if key.is_empty() {
            return Err(SaveError::AuthKeyInvalid);
        }
        let payload = self.to_binary()?;
        let tag = compute_hmac_sha256(key, &payload)?;
        let payload_len = u32::try_from(payload.len()).map_err(|_| SaveError::TooLarge)?;

        let mut output = Vec::with_capacity(4 + 2 + 4 + 32 + payload.len());
        output.extend_from_slice(&AUTH_SAVE_MAGIC);
        output.extend_from_slice(&SAVE_FORMAT_VERSION.to_le_bytes());
        output.extend_from_slice(&payload_len.to_le_bytes());
        output.extend_from_slice(&tag);
        output.extend_from_slice(&payload);
        Ok(output)
    }

    /// Parses authenticated save payload and validates integrity before decoding.
    pub fn from_authenticated_binary(input: &[u8], key: &[u8]) -> Result<Self, SaveError> {
        if key.is_empty() {
            return Err(SaveError::AuthKeyInvalid);
        }
        if input.len() < 42 {
            return Err(SaveError::TooSmall);
        }
        if input[0..4] != AUTH_SAVE_MAGIC {
            return Err(SaveError::InvalidMagic);
        }

        let version = u16::from_le_bytes([input[4], input[5]]);
        if version != SAVE_FORMAT_VERSION {
            return Err(SaveError::IncompatibleVersion {
                found: version,
                expected: SAVE_FORMAT_VERSION,
            });
        }

        let payload_len = u32::from_le_bytes([input[6], input[7], input[8], input[9]]) as usize;
        let tag = input.get(10..42).ok_or(SaveError::MissingPayload)?;
        let payload = input.get(42..).ok_or(SaveError::MissingPayload)?;
        if payload.len() != payload_len {
            return Err(SaveError::LengthMismatch);
        }

        verify_hmac_sha256(key, payload, tag)?;

        SaveData::from_binary(payload)
    }

    pub fn from_any_binary(input: &[u8], key: &[u8]) -> Result<Self, SaveError> {
        Self::from_authenticated_binary(input, key)
    }

    /// Validates that this save matches the given script_id.
    pub fn validate_script_id(&self, expected: &ScriptId) -> Result<(), SaveError> {
        if &self.script_id != expected {
            return Err(SaveError::ScriptMismatch);
        }
        Ok(())
    }
}

/// Errors that can occur during save/load operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SaveError {
    TooSmall,
    TooLarge,
    InvalidMagic,
    IncompatibleVersion { found: u16, expected: u16 },
    ChecksumMismatch,
    LengthMismatch,
    MissingPayload,
    ScriptMismatch,
    AuthKeyInvalid,
    AuthenticationFailed,
    Serialization(String),
}

impl std::fmt::Display for SaveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TooSmall => write!(f, "save data too small"),
            Self::TooLarge => write!(f, "save data too large"),
            Self::InvalidMagic => write!(f, "invalid save file magic bytes"),
            Self::IncompatibleVersion { found, expected } => {
                write!(
                    f,
                    "incompatible save version: found {found}, expected {expected}"
                )
            }
            Self::ChecksumMismatch => write!(f, "save file checksum mismatch"),
            Self::LengthMismatch => write!(f, "save file length mismatch"),
            Self::MissingPayload => write!(f, "save file missing payload"),
            Self::ScriptMismatch => write!(f, "save does not match current script"),
            Self::AuthKeyInvalid => write!(f, "authentication key is empty or invalid"),
            Self::AuthenticationFailed => write!(f, "save authentication failed"),
            Self::Serialization(msg) => write!(f, "serialization error: {msg}"),
        }
    }
}

impl std::error::Error for SaveError {}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SaveSlotMetadata {
    pub slot_id: u16,
    pub quick: bool,
    pub updated_unix_ms: u64,
    pub script_id_hex: String,
    pub position: u32,
    pub flags_words: usize,
    pub vars_count: usize,
    #[serde(default)]
    pub chapter_label: Option<String>,
    #[serde(default)]
    pub summary_line: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SaveSlotEntry {
    pub metadata: SaveSlotMetadata,
    pub path: PathBuf,
}

#[derive(Debug)]
pub struct SaveSlotListIssue {
    pub path: PathBuf,
    pub error: SaveStoreError,
}

#[derive(Debug, Default)]
pub struct SaveSlotListing {
    pub entries: Vec<SaveSlotEntry>,
    pub issues: Vec<SaveSlotListIssue>,
}

#[derive(Debug)]
pub struct SaveSlotStore {
    root: PathBuf,
}

#[derive(Debug)]
pub enum SaveStoreError {
    Io(std::io::Error),
    Save(SaveError),
    RecoveryFailed {
        primary: SaveError,
        backup: Option<SaveError>,
    },
}

impl std::fmt::Display for SaveStoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SaveStoreError::Io(err) => write!(f, "save store io error: {err}"),
            SaveStoreError::Save(err) => write!(f, "save store serialization error: {err}"),
            SaveStoreError::RecoveryFailed { primary, backup } => match backup {
                Some(backup) => write!(
                    f,
                    "save store recovery failed (primary: {primary}, backup: {backup})"
                ),
                None => write!(
                    f,
                    "save store recovery failed (primary: {primary}, backup missing)"
                ),
            },
        }
    }
}

impl std::error::Error for SaveStoreError {}

impl From<std::io::Error> for SaveStoreError {
    fn from(value: std::io::Error) -> Self {
        SaveStoreError::Io(value)
    }
}

impl From<SaveError> for SaveStoreError {
    fn from(value: SaveError) -> Self {
        SaveStoreError::Save(value)
    }
}

impl SaveSlotStore {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn ensure_layout(&self) -> Result<(), SaveStoreError> {
        fs::create_dir_all(self.root.join("slots"))?;
        fs::create_dir_all(self.root.join("meta"))?;
        Ok(())
    }

    pub fn save_slot(
        &self,
        slot_id: u16,
        save: &SaveData,
    ) -> Result<SaveSlotEntry, SaveStoreError> {
        self.write_slot(slot_id, false, save)
    }

    fn write_slot(
        &self,
        slot_id: u16,
        quick: bool,
        save: &SaveData,
    ) -> Result<SaveSlotEntry, SaveStoreError> {
        self.ensure_layout()?;
        let slot_path = self.slot_path(slot_id, quick);
        let metadata_path = self.metadata_path(slot_id, quick);
        self.atomic_write_binary(&slot_path, &save.to_authenticated_binary(AUTH_SAVE_KEY)?)?;
        let metadata = self.build_metadata(slot_id, quick, save);
        self.atomic_write_binary(
            &metadata_path,
            serde_json::to_vec_pretty(&metadata)
                .map_err(|err| SaveError::Serialization(err.to_string()))?
                .as_slice(),
        )?;
        Ok(SaveSlotEntry {
            metadata,
            path: slot_path,
        })
    }

    pub fn load_slot(&self, slot_id: u16) -> Result<SaveData, SaveStoreError> {
        self.read_slot(slot_id, false)
    }

    fn read_slot(&self, slot_id: u16, quick: bool) -> Result<SaveData, SaveStoreError> {
        let slot_path = self.slot_path(slot_id, quick);
        let backup_path = backup_path(&slot_path);
        self.load_binary_with_recovery(&slot_path, &backup_path)
    }

    /// Removes a numbered slot, its metadata and their recovery backups.
    /// Missing files are accepted; filesystem failures can leave a partial removal.
    pub fn remove_slot(&self, slot_id: u16) -> Result<(), SaveStoreError> {
        let slot_path = self.slot_path(slot_id, false);
        let metadata_path = self.metadata_path(slot_id, false);
        let paths = [
            (backup_path(&slot_path), "slot backup"),
            (backup_path(&metadata_path), "slot metadata backup"),
            (slot_path, "slot file"),
            (metadata_path, "slot metadata"),
        ];
        // Reject unexpected directories or symlinks before deleting any slot data.
        for (path, role) in &paths {
            store_path_exists(path, role)?;
        }
        for (path, _) in paths {
            match fs::remove_file(path) {
                Ok(()) => {}
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
                Err(err) => return Err(SaveStoreError::Io(err)),
            }
        }
        Ok(())
    }

    pub fn quicksave(&self, save: &SaveData) -> Result<SaveSlotEntry, SaveStoreError> {
        self.write_slot(0, true, save)
    }

    pub fn quickload(&self) -> Result<SaveData, SaveStoreError> {
        self.read_slot(0, true)
    }

    pub fn has_quicksave(&self) -> Result<bool, SaveStoreError> {
        self.ensure_layout()?;
        save_file_available(&self.slot_path(0, true))
    }

    /// Lists slots strictly, returning an error if any metadata entry cannot be read.
    pub fn list_slots(&self) -> Result<Vec<SaveSlotEntry>, SaveStoreError> {
        let listing = self.list_slots_with_issues()?;
        if let Some(issue) = listing.issues.into_iter().next() {
            return Err(issue.error);
        }
        Ok(listing.entries)
    }

    /// Lists readable entries and their individual failures without hiding healthy saves.
    /// Failure to open or enumerate the metadata directory still returns an error.
    pub fn list_slots_with_issues(&self) -> Result<SaveSlotListing, SaveStoreError> {
        self.ensure_layout()?;
        let mut listing = SaveSlotListing::default();

        let meta_dir = self.root.join("meta");
        for entry in fs::read_dir(meta_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
                continue;
            }

            match self.read_slot_entry(&path) {
                Ok(Some(entry)) => listing.entries.push(entry),
                Ok(None) => {}
                Err(error) => listing.issues.push(SaveSlotListIssue { path, error }),
            }
        }

        listing.entries.sort_by_key(|entry| {
            (
                std::cmp::Reverse(entry.metadata.updated_unix_ms),
                !entry.metadata.quick,
                entry.metadata.slot_id,
            )
        });
        listing
            .issues
            .sort_by(|left, right| left.path.cmp(&right.path));
        Ok(listing)
    }

    fn read_slot_entry(&self, path: &Path) -> Result<Option<SaveSlotEntry>, SaveStoreError> {
        let bytes = read_existing_regular_file(path, "slot metadata")?;
        let metadata: SaveSlotMetadata = serde_json::from_slice(&bytes)
            .map_err(|err| SaveError::Serialization(err.to_string()))?;
        let slot_path = self.slot_path(metadata.slot_id, metadata.quick);
        Ok(save_file_available(&slot_path)?.then_some(SaveSlotEntry {
            metadata,
            path: slot_path,
        }))
    }

    fn build_metadata(&self, slot_id: u16, quick: bool, save: &SaveData) -> SaveSlotMetadata {
        SaveSlotMetadata {
            slot_id,
            quick,
            updated_unix_ms: now_unix_ms(),
            script_id_hex: script_id_hex(&save.script_id),
            position: save.state.position,
            flags_words: save.state.flags.len(),
            vars_count: save.state.vars.len(),
            chapter_label: chapter_label_hint(save),
            summary_line: summary_line_hint(save),
        }
    }

    fn load_binary_with_recovery(
        &self,
        primary_path: &Path,
        backup_path: &Path,
    ) -> Result<SaveData, SaveStoreError> {
        let primary_error = match read_optional_regular_file(primary_path, "save slot")? {
            Some(bytes) => match SaveData::from_any_binary(&bytes, AUTH_SAVE_KEY) {
                Ok(save) => return Ok(save),
                Err(err) => Some(err),
            },
            None => None,
        };
        if let Some(bytes) = read_optional_regular_file(backup_path, "save slot backup")? {
            return SaveData::from_any_binary(&bytes, AUTH_SAVE_KEY).map_err(|backup| {
                match primary_error {
                    Some(primary) => SaveStoreError::RecoveryFailed {
                        primary,
                        backup: Some(backup),
                    },
                    None => SaveStoreError::Save(backup),
                }
            });
        }
        Err(match primary_error {
            Some(primary) => SaveStoreError::RecoveryFailed {
                primary,
                backup: None,
            },
            None => missing_store_file(primary_path, "save slot"),
        })
    }

    fn atomic_write_binary(&self, path: &Path, bytes: &[u8]) -> Result<(), SaveStoreError> {
        if store_path_exists(path, "save slot target")? {
            crate::atomic_io::atomic_copy(path, &backup_path(path))?;
        }
        crate::atomic_io::atomic_replace(path, bytes)?;
        Ok(())
    }

    fn slot_path(&self, slot_id: u16, quick: bool) -> PathBuf {
        if quick {
            self.root.join("slots").join("quicksave.vnsav")
        } else {
            self.root
                .join("slots")
                .join(format!("slot_{slot_id:03}.vnsav"))
        }
    }

    fn metadata_path(&self, slot_id: u16, quick: bool) -> PathBuf {
        if quick {
            self.root.join("meta").join("quicksave.json")
        } else {
            self.root
                .join("meta")
                .join(format!("slot_{slot_id:03}.json"))
        }
    }
}

fn store_path_exists(path: &Path, role: &str) -> Result<bool, SaveStoreError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_file() => Ok(true),
        Ok(_) => Err(non_regular_store_path(path, role)),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(err) => Err(SaveStoreError::Io(err)),
    }
}

fn read_existing_regular_file(path: &Path, role: &str) -> Result<Vec<u8>, SaveStoreError> {
    read_optional_regular_file(path, role)?.ok_or_else(|| missing_store_file(path, role))
}

fn missing_store_file(path: &Path, role: &str) -> SaveStoreError {
    SaveStoreError::Io(std::io::Error::new(
        std::io::ErrorKind::NotFound,
        format!("missing {role}: {}", path.display()),
    ))
}

fn save_file_available(path: &Path) -> Result<bool, SaveStoreError> {
    if store_path_exists(path, "save slot")? {
        Ok(true)
    } else {
        store_path_exists(&backup_path(path), "save slot backup")
    }
}

fn read_optional_regular_file(path: &Path, role: &str) -> Result<Option<Vec<u8>>, SaveStoreError> {
    if store_path_exists(path, role)? {
        fs::read(path).map(Some).map_err(SaveStoreError::Io)
    } else {
        Ok(None)
    }
}

fn non_regular_store_path(path: &Path, role: &str) -> SaveStoreError {
    SaveStoreError::Io(std::io::Error::new(
        std::io::ErrorKind::InvalidInput,
        format!("{role} is not a regular file: {}", path.display()),
    ))
}
