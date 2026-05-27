use std::fs;
use std::path::{Path, PathBuf};

pub struct GuiAudioAssetStore {
    project_root: Option<PathBuf>,
    trusted_store: Option<vnengine_assets::AssetStore>,
}

impl GuiAudioAssetStore {
    pub fn new(project_root: Option<PathBuf>) -> Result<Self, String> {
        let trusted_store = match project_root.as_ref() {
            Some(root) => Some(
                vnengine_assets::AssetStore::new(
                    root.clone(),
                    vnengine_assets::SecurityMode::Trusted,
                    None,
                    false,
                )
                .map_err(|err| err.to_string())?,
            ),
            None => None,
        };
        Ok(Self {
            project_root,
            trusted_store,
        })
    }
}

impl visual_novel_runtime::AssetStore for GuiAudioAssetStore {
    fn load_bytes(&self, id: &str) -> Result<Vec<u8>, String> {
        let path = Path::new(id);
        if path.is_absolute() {
            return fs::read(path).map_err(|err| format!("audio file '{id}' read failed: {err}"));
        }

        if let Some(store) = &self.trusted_store {
            if let Ok(bytes) = store.load_bytes(id) {
                return Ok(bytes);
            }
        }

        if let Some(root) = &self.project_root {
            let candidate = root.join(id);
            if candidate.is_file() {
                return fs::read(&candidate)
                    .map_err(|err| format!("audio file '{id}' read failed: {err}"));
            }
        }

        Err(format!("audio asset '{id}' not found"))
    }
}
