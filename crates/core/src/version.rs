//! Format versioning constants for scripts and saves.
//!
//! This module defines explicit versions for all serialized formats,
//! enabling backward compatibility checks and clear upgrade paths.

/// Current schema version for JSON scripts.  
/// Increment MINOR for compatible changes, MAJOR for breaking changes.
pub const SCRIPT_SCHEMA_VERSION: &str = "1.0";

/// Current binary format version for compiled scripts.
/// Increment when the binary layout changes.
/// v2: Migrated from bincode to postcard serialization.
pub const COMPILED_FORMAT_VERSION: u16 = 2;

/// Current format version for save files.
/// Increment when EngineState serialization changes.
/// v3: Migrated save payload encoding from bincode to postcard.
pub const SAVE_FORMAT_VERSION: u16 = 3;

/// Magic bytes for compiled script binaries.
pub const SCRIPT_BINARY_MAGIC: [u8; 4] = *b"VNSC";

/// Magic bytes for save files.
pub const SAVE_BINARY_MAGIC: [u8; 4] = *b"VNSV";
