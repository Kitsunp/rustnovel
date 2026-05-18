use serde::{Deserialize, Serialize};

pub use visual_novel_engine::authoring::composer::BackgroundFit;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComposerPreviewMode {
    IsolatedNode,
    #[default]
    RuntimeInherited,
    RuntimeFromStart,
    SelectedRoute,
}

impl ComposerPreviewMode {
    pub const ALL: &'static [Self] = &[
        Self::IsolatedNode,
        Self::RuntimeInherited,
        Self::RuntimeFromStart,
        Self::SelectedRoute,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::IsolatedNode => "Isolated",
            Self::RuntimeInherited => "Runtime inherited",
            Self::RuntimeFromStart => "Runtime from start",
            Self::SelectedRoute => "Selected route",
        }
    }

    pub fn uses_runtime_state(self) -> bool {
        !matches!(self, Self::IsolatedNode)
    }
}
