use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use visual_novel_engine::{SaveData, SaveError, AUTH_SAVE_KEY};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct UserPreferences {
    pub fullscreen: bool,
    pub ui_scale: f32,
    pub vsync: bool,
}

impl Default for UserPreferences {
    fn default() -> Self {
        Self {
            fullscreen: false,
            ui_scale: 1.0,
            vsync: true,
        }
    }
}

impl UserPreferences {
    pub fn load_from(path: &Path) -> std::io::Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
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
        fs::write(path, payload)
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
    fs::write(path, payload)?;
    Ok(())
}

pub fn load_state_from(path: &Path) -> Result<SaveData, PersistError> {
    let raw = fs::read(path)?;
    Ok(SaveData::from_any_binary(&raw, AUTH_SAVE_KEY)?)
}
