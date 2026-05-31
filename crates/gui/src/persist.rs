use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use visual_novel_engine::{SaveData, SaveError, AUTH_SAVE_KEY};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct UserPreferences {
    pub fullscreen: bool,
    pub ui_scale: f32,
    pub vsync: bool,
    pub audio_muted: bool,
    pub master_volume: f32,
    pub bgm_volume: f32,
    pub sfx_volume: f32,
    pub voice_volume: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub advance_on_text_panel_click: Option<bool>,
}

impl Default for UserPreferences {
    fn default() -> Self {
        Self {
            fullscreen: false,
            ui_scale: 1.0,
            vsync: true,
            audio_muted: false,
            master_volume: 1.0,
            bgm_volume: 1.0,
            sfx_volume: 1.0,
            voice_volume: 1.0,
            advance_on_text_panel_click: None,
        }
    }
}

impl UserPreferences {
    pub fn load_from(path: &Path) -> std::io::Result<Self> {
        match fs::symlink_metadata(path) {
            Ok(metadata) if metadata.file_type().is_dir() => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    format!("preferences path is a directory: {}", path.display()),
                ));
            }
            Ok(_) => {}
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Self::default());
            }
            Err(err) => return Err(err),
        }
        let raw = fs::read_to_string(path)?;
        let parsed = serde_json::from_str(&raw)
            .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err.to_string()))?;
        Ok(parsed)
    }

    pub fn save_to(&self, path: &Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let payload = serde_json::to_string_pretty(self)
            .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err))?;
        crate::editor::atomic_io::atomic_replace(path, payload.as_bytes())
    }
}

#[derive(Debug, Error)]
pub enum PersistError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("save error: {0}")]
    Save(#[from] SaveError),
}

pub fn save_state_to(path: &Path, data: &SaveData) -> Result<(), PersistError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let payload = data.to_authenticated_binary(AUTH_SAVE_KEY)?;
    crate::editor::atomic_io::atomic_replace(path, &payload)?;
    Ok(())
}

pub fn load_state_from(path: &Path) -> Result<SaveData, PersistError> {
    let raw = fs::read(path)?;
    Ok(SaveData::from_any_binary(&raw, AUTH_SAVE_KEY)?)
}
