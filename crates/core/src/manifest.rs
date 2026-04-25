use std::collections::HashMap;
use std::path::{Path, PathBuf};

use miette::Diagnostic;
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const MANIFEST_SCHEMA_VERSION: &str = "1.0";
const LEGACY_MANIFEST_VERSION: &str = "0.x";
const MANIFEST_MIGRATION_GUARD_LIMIT: usize = 8;

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
    #[serde(default = "default_manifest_schema_version", alias = "schema_version")]
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

#[derive(Clone, Copy)]
struct ManifestMigrationStep {
    from_version: &'static str,
    to_version: &'static str,
    step_id: &'static str,
    apply: fn(&mut toml::Value) -> Result<bool, String>,
}

const MANIFEST_MIGRATION_STEPS: &[ManifestMigrationStep] = &[ManifestMigrationStep {
    from_version: LEGACY_MANIFEST_VERSION,
    to_version: MANIFEST_SCHEMA_VERSION,
    step_id: "manifest_legacy_to_1_0",
    apply: migrate_manifest_legacy_to_1_0,
}];

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

    if !from_version.starts_with("0.") {
        return Ok(ManifestMigrationReport {
            from_version: from_version.clone(),
            to_version: from_version,
            entries: Vec::new(),
        });
    }

    let mut current_version = from_version.clone();
    let mut entries = Vec::new();
    let mut guard = 0usize;

    while current_version != MANIFEST_SCHEMA_VERSION {
        guard += 1;
        if guard > MANIFEST_MIGRATION_GUARD_LIMIT {
            return Err(ManifestMigrationError::UnsupportedVersion(current_version));
        }

        let step = select_manifest_step_for(&current_version)
            .ok_or_else(|| ManifestMigrationError::UnsupportedVersion(current_version.clone()))?;
        let changed =
            (step.apply)(value).map_err(|message| ManifestMigrationError::StepFailed {
                step_id: step.step_id.to_string(),
                from_version: current_version.clone(),
                to_version: step.to_version.to_string(),
                message,
            })?;

        let root = value.as_table_mut().ok_or_else(|| {
            ManifestMigrationError::InvalidEnvelope(
                "manifest payload must be a TOML table".to_string(),
            )
        })?;
        root.insert(
            "manifest_schema_version".to_string(),
            toml::Value::String(step.to_version.to_string()),
        );

        entries.push(ManifestMigrationTraceEntry {
            step_id: step.step_id.to_string(),
            from_version: current_version.clone(),
            to_version: step.to_version.to_string(),
            changed,
        });
        current_version = step.to_version.to_string();
    }

    Ok(ManifestMigrationReport {
        from_version,
        to_version: current_version,
        entries,
    })
}

fn detect_manifest_version(value: &toml::Value) -> Result<String, ManifestMigrationError> {
    let root = value.as_table().ok_or_else(|| {
        ManifestMigrationError::InvalidEnvelope("manifest payload must be a TOML table".to_string())
    })?;

    if let Some(raw) = root
        .get("manifest_schema_version")
        .or_else(|| root.get("schema_version"))
    {
        let version = raw.as_str().ok_or_else(|| {
            ManifestMigrationError::InvalidEnvelope(
                "manifest schema version field must be a string".to_string(),
            )
        })?;
        return Ok(version.to_string());
    }

    Ok("0.9".to_string())
}

fn select_manifest_step_for(version: &str) -> Option<&'static ManifestMigrationStep> {
    MANIFEST_MIGRATION_STEPS
        .iter()
        .find(|step| step.from_version == version)
        .or_else(|| {
            if version.starts_with("0.") {
                MANIFEST_MIGRATION_STEPS
                    .iter()
                    .find(|step| step.from_version == LEGACY_MANIFEST_VERSION)
            } else {
                None
            }
        })
}

fn migrate_manifest_legacy_to_1_0(value: &mut toml::Value) -> Result<bool, String> {
    let root = value
        .as_table_mut()
        .ok_or_else(|| "manifest payload must be a TOML table".to_string())?;
    let mut changed = false;

    if let Some(old) = root.remove("schema_version") {
        root.insert("manifest_schema_version".to_string(), old);
        changed = true;
    }

    if !root.contains_key("manifest_schema_version") {
        root.insert(
            "manifest_schema_version".to_string(),
            toml::Value::String(MANIFEST_SCHEMA_VERSION.to_string()),
        );
        changed = true;
    }

    if !root.contains_key("assets") {
        root.insert(
            "assets".to_string(),
            toml::Value::Table(toml::map::Map::new()),
        );
        changed = true;
    }

    if !root.contains_key("settings") {
        let mut settings = toml::map::Map::new();
        settings.insert(
            "resolution".to_string(),
            toml::Value::Array(vec![toml::Value::Integer(1280), toml::Value::Integer(720)]),
        );
        settings.insert(
            "default_language".to_string(),
            toml::Value::String(default_language()),
        );
        settings.insert(
            "supported_languages".to_string(),
            toml::Value::Array(vec![toml::Value::String("en".to_string())]),
        );
        settings.insert(
            "entry_point".to_string(),
            toml::Value::String(default_entry_point()),
        );
        root.insert("settings".to_string(), toml::Value::Table(settings));
        changed = true;
    }

    if let Some(settings) = root.get_mut("settings").and_then(toml::Value::as_table_mut) {
        if !settings.contains_key("default_language") {
            settings.insert(
                "default_language".to_string(),
                toml::Value::String(default_language()),
            );
            changed = true;
        }
        if !settings.contains_key("supported_languages") {
            let default_lang = settings
                .get("default_language")
                .and_then(toml::Value::as_str)
                .unwrap_or("en")
                .to_string();
            settings.insert(
                "supported_languages".to_string(),
                toml::Value::Array(vec![toml::Value::String(default_lang)]),
            );
            changed = true;
        }
        if !settings.contains_key("entry_point") {
            settings.insert(
                "entry_point".to_string(),
                toml::Value::String(default_entry_point()),
            );
            changed = true;
        }
    }

    Ok(changed)
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
            },
            assets: AssetManifest::default(),
        }
    }
}

#[cfg(test)]
#[path = "tests/manifest_tests.rs"]
mod tests;
