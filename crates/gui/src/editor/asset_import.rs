#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AssetImportKind {
    Background,
    Character,
    Audio,
}

impl AssetImportKind {
    pub fn label(self) -> &'static str {
        match self {
            AssetImportKind::Background => "Background",
            AssetImportKind::Character => "Character",
            AssetImportKind::Audio => "Audio",
        }
    }

    pub fn dialog_title(self) -> &'static str {
        match self {
            AssetImportKind::Background => "Import background image",
            AssetImportKind::Character => "Import character image",
            AssetImportKind::Audio => "Import audio",
        }
    }

    pub fn destination_dir(self) -> &'static str {
        match self {
            AssetImportKind::Background => "assets/backgrounds",
            AssetImportKind::Character => "assets/characters",
            AssetImportKind::Audio => "assets/audio",
        }
    }

    pub fn allowed_extensions(self) -> &'static [&'static str] {
        match self {
            AssetImportKind::Background | AssetImportKind::Character => &["png", "jpg", "jpeg"],
            AssetImportKind::Audio => &["ogg", "wav", "flac", "mp3"],
        }
    }

    pub fn file_dialog_extensions(self) -> &'static [&'static str] {
        self.allowed_extensions()
    }

    pub fn field_button_label(self) -> &'static str {
        match self {
            AssetImportKind::Background | AssetImportKind::Character => "Elegir imagen...",
            AssetImportKind::Audio => "Elegir audio...",
        }
    }

    pub fn accepts_extension(self, extension: &str) -> bool {
        self.allowed_extensions()
            .iter()
            .any(|allowed| extension.eq_ignore_ascii_case(allowed))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AssetFieldTarget {
    SceneBackground,
    SceneMusic,
    SceneCharacterExpression(usize),
    ScenePatchBackground,
    ScenePatchMusic,
    ScenePatchAddCharacterExpression(usize),
    AudioActionAsset,
}
