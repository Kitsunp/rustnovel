use super::{CharacterAsset, ManifestError, ProjectManifest};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{self, Read},
    path::{Component, Path, PathBuf},
};

pub const DEFAULT_ASSET_IMPORT_LIMIT: u64 = 15 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssetImportKind {
    Background,
    Character,
    Audio,
}

impl std::str::FromStr for AssetImportKind {
    type Err = String;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.to_ascii_lowercase().as_str() {
            "background" => Ok(Self::Background),
            "character" => Ok(Self::Character),
            "audio" => Ok(Self::Audio),
            _ => Err("asset kind must be background, character, or audio".into()),
        }
    }
}

impl AssetImportKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Background => "Background",
            Self::Character => "Character",
            Self::Audio => "Audio",
        }
    }
    pub fn destination_dir(self) -> &'static str {
        match self {
            Self::Background => "assets/backgrounds",
            Self::Character => "assets/characters",
            Self::Audio => "assets/audio",
        }
    }
    pub fn allowed_extensions(self) -> &'static [&'static str] {
        match self {
            Self::Background | Self::Character => &["png", "jpg", "jpeg"],
            Self::Audio => &["ogg", "wav", "flac", "mp3"],
        }
    }
    pub fn accepts_extension(self, extension: &str) -> bool {
        self.allowed_extensions()
            .iter()
            .any(|allowed| extension.eq_ignore_ascii_case(allowed))
    }
    pub fn field_path(self, name: &str) -> String {
        let table = match self {
            Self::Background => "backgrounds",
            Self::Character => "characters",
            Self::Audio => "audio",
        };
        format!("manifest.assets.{table}[{name}]")
    }
    fn entries(self, manifest: &ProjectManifest) -> Vec<(&str, &Path)> {
        let mut entries: Vec<_> = match self {
            Self::Background => manifest
                .assets
                .backgrounds
                .iter()
                .map(|(name, path)| (name.as_str(), path.as_path()))
                .collect(),
            Self::Character => manifest
                .assets
                .characters
                .iter()
                .map(|(name, asset)| (name.as_str(), asset.path.as_path()))
                .collect(),
            Self::Audio => manifest
                .assets
                .audio
                .iter()
                .map(|(name, path)| (name.as_str(), path.as_path()))
                .collect(),
        };
        entries.sort_by_key(|(name, _)| *name);
        entries
    }
}

#[derive(Debug, thiserror::Error)]
pub enum AssetImportError {
    #[error("{0}")]
    Invalid(String),
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    Manifest(#[from] ManifestError),
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct AssetImportResult {
    pub name: String,
    pub path: String,
    pub source: PathBuf,
    pub imported: bool,
    pub copied: bool,
    pub bytes: u64,
}

/// Imports and registers a resource, reusing identical registered content.
/// On failure, the in-memory manifest stays unchanged and a new copy is removed.
pub fn import_project_asset(
    project_root: &Path,
    manifest_path: &Path,
    manifest: &mut ProjectManifest,
    source: &Path,
    kind: AssetImportKind,
    max_bytes: u64,
) -> Result<AssetImportResult, AssetImportError> {
    let root = project_root.canonicalize()?;
    let parent = manifest_path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    if !parent.canonicalize()?.starts_with(&root) {
        return Err(AssetImportError::Invalid(
            "manifest destination escapes project root".into(),
        ));
    }
    let source = source.canonicalize()?;
    let metadata = source.metadata()?;
    if !metadata.is_file() {
        return Err(AssetImportError::Invalid("source is not a file".into()));
    }
    let extension = source
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if !kind.accepts_extension(&extension) {
        return Err(AssetImportError::Invalid(format!(
            "unsupported .{extension} file for {}; allowed: {}",
            kind.label(),
            kind.allowed_extensions().join(", ")
        )));
    }
    if metadata.len() > max_bytes {
        return Err(AssetImportError::Invalid(format!(
            "asset is too large: {} bytes (max {max_bytes})",
            metadata.len()
        )));
    }
    let entries = kind.entries(manifest);
    let mut source_hash = None;
    for (name, path) in &entries {
        let Ok(relative) = safe_relative(path) else {
            continue;
        };
        let Ok(candidate) = root.join(&relative).canonicalize() else {
            continue;
        };
        if !candidate.starts_with(&root) || !candidate.is_file() {
            continue;
        }
        let identical = candidate == source
            || (candidate.metadata()?.len() == metadata.len() && {
                let hash = match &source_hash {
                    Some(hash) => hash,
                    None => source_hash.insert(crate::atomic_io::sha256_file(&source)?.0),
                };
                &crate::atomic_io::sha256_file(&candidate)?.0 == hash
            });
        if identical {
            return Ok(AssetImportResult {
                name: (*name).into(),
                path: relative.to_string_lossy().replace('\\', "/"),
                source,
                imported: false,
                copied: false,
                bytes: metadata.len(),
            });
        }
    }
    let base = sanitized_stem(&source);
    let mut name = base.clone();
    let mut suffix = 1usize;
    while entries.iter().any(|(existing, _)| *existing == name) {
        name = format!("{base}_{suffix}");
        suffix += 1;
    }
    drop(entries);
    let mut pending = PendingAsset(None);
    let relative = if source.starts_with(&root) {
        safe_relative(source.strip_prefix(&root).map_err(io::Error::other)?)?
    } else {
        let mut directory = root.clone();
        for part in kind.destination_dir().split('/') {
            directory.push(part);
            if directory.exists() {
                if !directory.canonicalize()?.starts_with(&root) {
                    return Err(AssetImportError::Invalid(
                        "asset destination escapes project root".into(),
                    ));
                }
            } else {
                fs::create_dir(&directory)?;
            }
        }
        let mut suffix = 0usize;
        loop {
            let file_name = if suffix == 0 {
                format!("{base}.{extension}")
            } else {
                format!("{base}-{suffix}.{extension}")
            };
            let relative = safe_relative(&Path::new(kind.destination_dir()).join(file_name))?;
            let target = root.join(&relative);
            match fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&target)
            {
                Ok(mut output) => {
                    pending.0 = Some(target);
                    let mut input = fs::File::open(&source)?.take(max_bytes.saturating_add(1));
                    if io::copy(&mut input, &mut output)? > max_bytes {
                        return Err(AssetImportError::Invalid(
                            "asset grew beyond import limit".into(),
                        ));
                    }
                    output.sync_all()?;
                    break relative;
                }
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                    suffix += 1;
                }
                Err(error) => return Err(error.into()),
            }
        }
    };
    let path = relative.to_string_lossy().replace('\\', "/");
    let mut updated = manifest.clone();
    match kind {
        AssetImportKind::Background => {
            updated.assets.backgrounds.insert(name.clone(), relative);
        }
        AssetImportKind::Character => {
            updated.assets.characters.insert(
                name.clone(),
                CharacterAsset {
                    path: relative,
                    scale: None,
                },
            );
        }
        AssetImportKind::Audio => {
            updated.assets.audio.insert(name.clone(), relative);
        }
    }
    updated.save(manifest_path)?;
    *manifest = updated;
    let copied = pending.0.take().is_some();
    Ok(AssetImportResult {
        name,
        path,
        source,
        imported: true,
        copied,
        bytes: metadata.len(),
    })
}

fn safe_relative(path: &Path) -> Result<PathBuf, AssetImportError> {
    if !cfg!(windows) && path.as_os_str().to_string_lossy().contains('\\') {
        return Err(AssetImportError::Invalid(
            "unsafe asset path rejected".into(),
        ));
    }
    let mut normalized = PathBuf::new();
    for part in path.components() {
        match part {
            Component::CurDir => {}
            Component::Normal(part) => normalized.push(part),
            _ => {
                return Err(AssetImportError::Invalid(
                    "unsafe asset path rejected".into(),
                ))
            }
        }
    }
    let value = normalized
        .to_str()
        .ok_or_else(|| AssetImportError::Invalid("asset path must be UTF-8".into()))?
        .replace('\\', "/");
    if value.is_empty()
        || value.ends_with(' ')
        || value.chars().any(char::is_control)
        || crate::authoring::is_unsafe_asset_ref(&value)
    {
        return Err(AssetImportError::Invalid(
            "unsafe asset path rejected".into(),
        ));
    }
    Ok(normalized)
}

fn sanitized_stem(path: &Path) -> String {
    let raw = path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("asset");
    let mut name = String::new();
    for ch in raw.chars() {
        if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_') {
            name.push(ch);
        } else if !name.ends_with('_') {
            name.push('_');
        }
    }
    let name = name.trim_matches('_');
    if name.is_empty() {
        "asset".into()
    } else {
        name.into()
    }
}

struct PendingAsset(Option<PathBuf>);
impl Drop for PendingAsset {
    fn drop(&mut self) {
        if let Some(path) = &self.0 {
            let _ = fs::remove_file(path);
        }
    }
}
