//! Canonical script identity and save data structures.
//!
//! Provides SHA-256 based script identification for save integrity.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use crate::state::EngineState;
use crate::version::{SAVE_BINARY_MAGIC, SAVE_FORMAT_VERSION};

/// Unique identifier for a compiled script, computed as SHA-256 of its binary representation.
pub type ScriptId = [u8; 32];
pub const AUTH_SAVE_MAGIC: [u8; 4] = *b"VNSA";
pub const AUTH_SAVE_KEY: &[u8] = b"vnengine.save.v1";

/// Default encoded-save budget. Hosts may supply a different explicit limit.
pub const DEFAULT_MAX_SAVE_BYTES: usize = 16 * 1024 * 1024;
/// Maximum size of an individual slot metadata JSON file.
pub const MAX_SAVE_METADATA_BYTES: usize = 64 * 1024;

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
        self.to_authenticated_binary_with_limit(key, usize::MAX)
    }

    /// Encodes a save only if its authenticated representation fits the limit.
    pub fn to_authenticated_binary_with_limit(
        &self,
        key: &[u8],
        max_bytes: usize,
    ) -> Result<Vec<u8>, SaveError> {
        if key.is_empty() {
            return Err(SaveError::AuthKeyInvalid);
        }
        let payload = self.to_binary()?;
        if payload
            .len()
            .checked_add(42)
            .is_none_or(|size| size > max_bytes)
        {
            return Err(SaveError::TooLarge);
        }
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

    /// Reads at most `max_bytes + 1` bytes before authenticating the payload.
    pub fn read_authenticated(
        reader: impl Read,
        key: &[u8],
        max_bytes: usize,
    ) -> Result<Self, SaveStoreError> {
        if key.is_empty() {
            return Err(SaveStoreError::Save(SaveError::AuthKeyInvalid));
        }
        let bytes = read_bounded_save_bytes(reader, max_bytes)?;
        Self::from_authenticated_binary(&bytes, key).map_err(SaveStoreError::Save)
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

/// Successful slot load, including whether the backup was needed.
#[derive(Debug)]
pub struct SaveSlotLoad {
    pub data: SaveData,
    pub recovery: Option<SaveRecovery>,
}

/// The primary failure that led to backup recovery, or `None` if it was absent.
#[derive(Debug)]
pub struct SaveRecovery {
    pub primary_error: Option<SaveError>,
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
    max_save_bytes: usize,
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

impl std::error::Error for SaveStoreError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Save(error) => Some(error),
            Self::RecoveryFailed { primary, .. } => Some(primary),
        }
    }
}

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
        Self::with_max_save_bytes(root, DEFAULT_MAX_SAVE_BYTES)
    }

    /// Creates a store with a host-defined limit for each encoded save.
    pub fn with_max_save_bytes(root: PathBuf, max_save_bytes: usize) -> Self {
        Self {
            root,
            max_save_bytes,
        }
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
        let metadata = self.build_metadata(slot_id, quick, save);
        let metadata_bytes = serde_json::to_vec_pretty(&metadata)
            .map_err(|err| SaveError::Serialization(err.to_string()))?;
        if metadata_bytes.len() > MAX_SAVE_METADATA_BYTES {
            return Err(oversized_metadata(&metadata_path));
        }
        store_path_exists(&metadata_path, "slot metadata target")?;
        store_path_exists(&backup_path(&metadata_path), "slot metadata backup")?;
        let bytes = save.to_authenticated_binary_with_limit(AUTH_SAVE_KEY, self.max_save_bytes)?;
        self.atomic_write_save(&slot_path, &bytes, &save.script_id)?;
        self.atomic_write_metadata(&metadata_path, &metadata_bytes)?;
        Ok(SaveSlotEntry {
            metadata,
            path: slot_path,
        })
    }

    pub fn load_slot(&self, slot_id: u16) -> Result<SaveData, SaveStoreError> {
        self.load_slot_with_recovery(slot_id)
            .map(|loaded| loaded.data)
    }

    pub fn load_slot_with_recovery(&self, slot_id: u16) -> Result<SaveSlotLoad, SaveStoreError> {
        self.read_slot(slot_id, false)
    }

    fn read_slot(&self, slot_id: u16, quick: bool) -> Result<SaveSlotLoad, SaveStoreError> {
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
        self.quickload_with_recovery().map(|loaded| loaded.data)
    }

    pub fn quickload_with_recovery(&self) -> Result<SaveSlotLoad, SaveStoreError> {
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
                Ok(entry) => listing.entries.push(entry),
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

    fn read_slot_entry(&self, path: &Path) -> Result<SaveSlotEntry, SaveStoreError> {
        let bytes = read_slot_metadata_bytes(path)?;
        let metadata: SaveSlotMetadata = serde_json::from_slice(&bytes)
            .map_err(|err| SaveError::Serialization(err.to_string()))?;
        if (metadata.quick && metadata.slot_id != 0)
            || self.metadata_path(metadata.slot_id, metadata.quick) != path
        {
            return Err(SaveStoreError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!(
                    "slot metadata filename does not match its slot identity: {}",
                    path.display()
                ),
            )));
        }
        let slot_path = self.slot_path(metadata.slot_id, metadata.quick);
        if !save_file_available(&slot_path)? {
            return Err(missing_store_file(
                &slot_path,
                "slot referenced by metadata",
            ));
        }
        Ok(SaveSlotEntry {
            metadata,
            path: slot_path,
        })
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
    ) -> Result<SaveSlotLoad, SaveStoreError> {
        let primary_error = match self.read_optional_save(primary_path, "save slot") {
            Ok(Some((save, _))) => {
                return Ok(SaveSlotLoad {
                    data: save,
                    recovery: None,
                })
            }
            Ok(None) => None,
            Err(SaveStoreError::Save(err)) => Some(err),
            Err(err) => return Err(err),
        };
        match self.read_optional_save(backup_path, "save slot backup") {
            Ok(Some((save, _))) => {
                return Ok(SaveSlotLoad {
                    data: save,
                    recovery: Some(SaveRecovery { primary_error }),
                })
            }
            Ok(None) => {}
            Err(SaveStoreError::Save(backup)) => {
                return Err(match primary_error {
                    Some(primary) => SaveStoreError::RecoveryFailed {
                        primary,
                        backup: Some(backup),
                    },
                    None => SaveStoreError::Save(backup),
                })
            }
            Err(err) => return Err(err),
        }
        Err(match primary_error {
            Some(primary) => SaveStoreError::RecoveryFailed {
                primary,
                backup: None,
            },
            None => missing_store_file(primary_path, "save slot"),
        })
    }

    fn read_optional_save(
        &self,
        path: &Path,
        role: &str,
    ) -> Result<Option<(SaveData, Vec<u8>)>, SaveStoreError> {
        if !store_path_exists(path, role)? {
            return Ok(None);
        }
        let file = fs::File::open(path)?;
        let bytes = read_bounded_save_bytes(file, self.max_save_bytes)?;
        let data = SaveData::from_authenticated_binary(&bytes, AUTH_SAVE_KEY)?;
        Ok(Some((data, bytes)))
    }

    fn atomic_write_save(
        &self,
        path: &Path,
        bytes: &[u8],
        script_id: &ScriptId,
    ) -> Result<(), SaveStoreError> {
        match self.read_optional_save(path, "save slot target") {
            Ok(Some((save, primary_bytes))) if &save.script_id == script_id => {
                crate::atomic_io::atomic_replace(&backup_path(path), &primary_bytes)?;
            }
            Ok(Some((_, primary_bytes))) => {
                let backup_matches =
                    match self.read_optional_save(&backup_path(path), "save slot backup") {
                        Ok(Some((backup, _))) => &backup.script_id == script_id,
                        Ok(None) | Err(SaveStoreError::Save(_)) => false,
                        Err(error) => return Err(error),
                    };
                if !backup_matches {
                    crate::atomic_io::atomic_replace(&backup_path(path), &primary_bytes)?;
                }
            }
            Ok(None) => {}
            Err(SaveStoreError::Save(primary)) => {
                self.require_compatible_backup(path, primary, script_id)?
            }
            Err(error) => return Err(error),
        }
        crate::atomic_io::atomic_replace(path, bytes)?;
        Ok(())
    }

    fn require_compatible_backup(
        &self,
        path: &Path,
        primary: SaveError,
        script_id: &ScriptId,
    ) -> Result<(), SaveStoreError> {
        match self.read_optional_save(&backup_path(path), "save slot backup") {
            Ok(Some((backup, _))) if &backup.script_id == script_id => Ok(()),
            Ok(Some(_)) => Err(SaveStoreError::RecoveryFailed {
                primary,
                backup: Some(SaveError::ScriptMismatch),
            }),
            Ok(None) => Err(SaveStoreError::RecoveryFailed {
                primary,
                backup: None,
            }),
            Err(SaveStoreError::Save(backup)) => Err(SaveStoreError::RecoveryFailed {
                primary,
                backup: Some(backup),
            }),
            Err(error) => Err(error),
        }
    }

    fn atomic_write_metadata(&self, path: &Path, bytes: &[u8]) -> Result<(), SaveStoreError> {
        if store_path_exists(path, "slot metadata target")? {
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

fn read_slot_metadata_bytes(path: &Path) -> Result<Vec<u8>, SaveStoreError> {
    if !store_path_exists(path, "slot metadata")? {
        return Err(missing_store_file(path, "slot metadata"));
    }
    read_bounded_save_bytes(fs::File::open(path)?, MAX_SAVE_METADATA_BYTES).map_err(|error| {
        if matches!(error, SaveStoreError::Save(SaveError::TooLarge)) {
            oversized_metadata(path)
        } else {
            error
        }
    })
}

fn oversized_metadata(path: &Path) -> SaveStoreError {
    SaveStoreError::Io(std::io::Error::new(
        std::io::ErrorKind::InvalidData,
        format!(
            "slot metadata exceeds {MAX_SAVE_METADATA_BYTES} bytes: {}",
            path.display()
        ),
    ))
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

fn read_bounded_save_bytes(reader: impl Read, max_bytes: usize) -> Result<Vec<u8>, SaveStoreError> {
    let mut bytes = Vec::new();
    let mut bounded = reader.take(max_bytes as u64);
    bounded.read_to_end(&mut bytes)?;
    if bounded.limit() == 0 {
        // Probe past the limit without growing the buffer for a rejected input.
        match bounded.into_inner().read_exact(&mut [0]) {
            Ok(()) => return Err(SaveError::TooLarge.into()),
            Err(error) if error.kind() == std::io::ErrorKind::UnexpectedEof => {}
            Err(error) => return Err(error.into()),
        }
    }
    Ok(bytes)
}

fn non_regular_store_path(path: &Path, role: &str) -> SaveStoreError {
    SaveStoreError::Io(std::io::Error::new(
        std::io::ErrorKind::InvalidInput,
        format!("{role} is not a regular file: {}", path.display()),
    ))
}
