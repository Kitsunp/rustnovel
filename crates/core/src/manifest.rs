use std::collections::HashMap;
use std::path::{Path, PathBuf};

use miette::Diagnostic;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::PlayerMenuConfig;

pub const MANIFEST_SCHEMA_VERSION: &str = "1.0";

fn default_manifest_schema_version() -> String {
    MANIFEST_SCHEMA_VERSION.to_string()
}

fn default_language() -> String {
    "en".to_string()
}

fn default_supported_languages() -> Vec<String> {
    vec!["en".to_string()]
}

fn default_entry_point() -> String {
    "main.json".to_string()
}

/// The single source of truth for a visual novel project.
///
/// The manifest acts as a "compass", guiding the loading of assets and configuration.
/// Anything not strictly declared here is considered non-existent.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProjectManifest {
    #[serde(default = "default_manifest_schema_version")]
    pub manifest_schema_version: String,
    pub metadata: ProjectMetadata,
    pub settings: ProjectSettings,
    pub assets: AssetManifest,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProjectMetadata {
    pub name: String,
    pub author: String,
    pub version: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProjectSettings {
    pub resolution: (u32, u32),
    #[serde(default = "default_language")]
    pub default_language: String,
    #[serde(default = "default_supported_languages")]
    pub supported_languages: Vec<String>,
    /// Main script file to load (e.g. "main.json")
    #[serde(default = "default_entry_point")]
    pub entry_point: String,
    #[serde(default)]
    pub player_menu: PlayerMenuConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct AssetManifest {
    #[serde(default)]
    pub backgrounds: HashMap<String, PathBuf>,
    #[serde(default)]
    pub characters: HashMap<String, CharacterAsset>,
    #[serde(default)]
    pub audio: HashMap<String, PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CharacterAsset {
    pub path: PathBuf,
    /// Default scale for this character (1.0 = normal)
    pub scale: Option<f32>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManifestMigrationTraceEntry {
    pub step_id: String,
    pub from_version: String,
    pub to_version: String,
    pub changed: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManifestMigrationReport {
    pub from_version: String,
    pub to_version: String,
    pub entries: Vec<ManifestMigrationTraceEntry>,
}

impl ManifestMigrationReport {
    pub fn changed(&self) -> bool {
        self.entries.iter().any(|entry| entry.changed)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ManifestMigrationError {
    ParseToml(String),
    SerializeToml(String),
    InvalidEnvelope(String),
    UnsupportedVersion(String),
    StepFailed {
        step_id: String,
        from_version: String,
        to_version: String,
        message: String,
    },
}

impl std::fmt::Display for ManifestMigrationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ManifestMigrationError::ParseToml(err) => {
                write!(f, "manifest migration parse error: {err}")
            }
            ManifestMigrationError::SerializeToml(err) => {
                write!(f, "manifest migration serialize error: {err}")
            }
            ManifestMigrationError::InvalidEnvelope(message) => {
                write!(f, "invalid manifest envelope: {message}")
            }
            ManifestMigrationError::UnsupportedVersion(version) => {
                write!(f, "unsupported manifest schema version '{version}'")
            }
            ManifestMigrationError::StepFailed {
                step_id,
                from_version,
                to_version,
                message,
            } => write!(
                f,
                "manifest migration step '{step_id}' failed ({from_version} -> {to_version}): {message}"
            ),
        }
    }
}

impl std::error::Error for ManifestMigrationError {}

#[derive(Debug, Error, Diagnostic)]
pub enum ManifestError {
    #[error("manifest file not found at {0}")]
    #[diagnostic(
        code(manifest::not_found),
        help("Create a 'project.vnm' file in the root directory")
    )]
    NotFound(PathBuf),

    #[error("failed to parse manifest: {0}")]
    #[diagnostic(code(manifest::parse_error))]
    ParseError(#[from] toml::de::Error),

    #[error("io error: {0}")]
    #[diagnostic(code(manifest::io_error))]
    IoError(#[from] std::io::Error),

    #[error("manifest migration failed: {0}")]
    #[diagnostic(code(manifest::migration_error))]
    MigrationError(String),
}

pub fn migrate_manifest_toml_to_current(
    input: &str,
) -> Result<(String, ManifestMigrationReport), ManifestMigrationError> {
    let mut value: toml::Value = input
        .parse()
        .map_err(|err: toml::de::Error| ManifestMigrationError::ParseToml(err.to_string()))?;
    let report = migrate_manifest_value_to_current(&mut value)?;
    let output = toml::to_string_pretty(&value)
        .map_err(|err| ManifestMigrationError::SerializeToml(err.to_string()))?;
    Ok((output, report))
}

pub fn migrate_manifest_value_to_current(
    value: &mut toml::Value,
) -> Result<ManifestMigrationReport, ManifestMigrationError> {
    let snapshot = value.clone();
    match migrate_manifest_value_to_current_inner(value) {
        Ok(report) => Ok(report),
        Err(err) => {
            *value = snapshot;
            Err(err)
        }
    }
}

fn migrate_manifest_value_to_current_inner(
    value: &mut toml::Value,
) -> Result<ManifestMigrationReport, ManifestMigrationError> {
    let from_version = detect_manifest_version(value)?;
    if from_version == MANIFEST_SCHEMA_VERSION {
        return Ok(ManifestMigrationReport {
            from_version: from_version.clone(),
            to_version: from_version,
            entries: Vec::new(),
        });
    }

    Err(ManifestMigrationError::UnsupportedVersion(from_version))
}

fn detect_manifest_version(value: &toml::Value) -> Result<String, ManifestMigrationError> {
    let root = value.as_table().ok_or_else(|| {
        ManifestMigrationError::InvalidEnvelope("manifest payload must be a TOML table".to_string())
    })?;

    if let Some(raw) = root.get("manifest_schema_version") {
        let version = raw.as_str().ok_or_else(|| {
            ManifestMigrationError::InvalidEnvelope(
                "manifest schema version field must be a string".to_string(),
            )
        })?;
        return Ok(version.to_string());
    }

    Err(ManifestMigrationError::InvalidEnvelope(
        "missing manifest_schema_version".to_string(),
    ))
}

impl ProjectManifest {
    pub fn from_toml_with_migration(
        input: &str,
    ) -> Result<(Self, ManifestMigrationReport), ManifestError> {
        let (migrated, report) = migrate_manifest_toml_to_current(input)
            .map_err(|err| ManifestError::MigrationError(err.to_string()))?;
        let manifest: ProjectManifest = toml::from_str(&migrated)?;
        Ok((manifest, report))
    }

    /// load a manifest from a file path.
    pub fn load(path: &Path) -> Result<Self, ManifestError> {
        let content = std::fs::read_to_string(path)?;
        let (manifest, _) = Self::from_toml_with_migration(&content)?;
        Ok(manifest)
    }

    /// save the manifest to a file path.
    pub fn save(&self, path: &Path) -> Result<(), ManifestError> {
        let mut normalized = self.clone();
        normalized.manifest_schema_version = MANIFEST_SCHEMA_VERSION.to_string();
        let content = toml::to_string_pretty(&normalized)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        std::fs::write(path, content)?;
        Ok(())
    }

    /// creates a default new project manifest.
    pub fn new(name: &str, author: &str) -> Self {
        Self {
            manifest_schema_version: MANIFEST_SCHEMA_VERSION.to_string(),
            metadata: ProjectMetadata {
                name: name.to_string(),
                author: author.to_string(),
                version: "0.1.0".to_string(),
                description: None,
            },
            settings: ProjectSettings {
                resolution: (1280, 720),
                default_language: default_language(),
                supported_languages: default_supported_languages(),
                entry_point: default_entry_point(),
                player_menu: PlayerMenuConfig::default(),
            },
            assets: AssetManifest::default(),
        }
    }
}
