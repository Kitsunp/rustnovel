#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AssetImportKind {
    Background,
    Character,
    Audio,
}

impl AssetImportKind {
    pub fn label(self) -> &'static str {
        visual_novel_engine::manifest::AssetImportKind::from(self).label()
    }

    pub fn dialog_title(self) -> &'static str {
        match self {
            AssetImportKind::Background => "Import background image",
            AssetImportKind::Character => "Import character image",
            AssetImportKind::Audio => "Import audio",
        }
    }

    pub fn destination_dir(self) -> &'static str {
        visual_novel_engine::manifest::AssetImportKind::from(self).destination_dir()
    }

    pub fn allowed_extensions(self) -> &'static [&'static str] {
        visual_novel_engine::manifest::AssetImportKind::from(self).allowed_extensions()
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
impl From<AssetImportKind> for visual_novel_engine::manifest::AssetImportKind {
    fn from(kind: AssetImportKind) -> Self {
        match kind {
            AssetImportKind::Background => Self::Background,
            AssetImportKind::Character => Self::Character,
            AssetImportKind::Audio => Self::Audio,
        }
    }
}

impl AssetImportKind {
    pub fn field_path(self, name: &str) -> String {
        visual_novel_engine::manifest::AssetImportKind::from(self).field_path(name)
    }
}
