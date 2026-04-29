use serde::{Deserialize, Serialize};

use crate::{CharacterPlacementRaw, CondRaw, EventRaw, ScenePatchRaw};

pub const NODE_VERTICAL_SPACING: f32 = 90.0;

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct AuthoringPosition {
    pub x: f32,
    pub y: f32,
}

impl AuthoringPosition {
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum StoryNode {
    Dialogue {
        speaker: String,
        text: String,
    },
    Choice {
        prompt: String,
        options: Vec<String>,
    },
    Scene {
        profile: Option<String>,
        background: Option<String>,
        music: Option<String>,
        characters: Vec<CharacterPlacementRaw>,
    },
    Jump {
        target: String,
    },
    SetVariable {
        key: String,
        value: i32,
    },
    SetFlag {
        key: String,
        value: bool,
    },
    ScenePatch(ScenePatchRaw),
    JumpIf {
        target: String,
        cond: CondRaw,
    },
    Start,
    End,
    AudioAction {
        channel: String,
        action: String,
        asset: Option<String>,
        volume: Option<f32>,
        fade_duration_ms: Option<u64>,
        loop_playback: Option<bool>,
    },
    Transition {
        kind: String,
        duration_ms: u32,
        color: Option<String>,
    },
    CharacterPlacement {
        name: String,
        x: i32,
        y: i32,
        scale: Option<f32>,
    },
    Generic(EventRaw),
}

impl Default for StoryNode {
    fn default() -> Self {
        StoryNode::Dialogue {
            speaker: "Character".to_string(),
            text: "Enter dialogue...".to_string(),
        }
    }
}

impl StoryNode {
    pub fn type_name(&self) -> &'static str {
        match self {
            StoryNode::Dialogue { .. } => "Dialogue",
            StoryNode::Choice { .. } => "Choice",
            StoryNode::Scene { .. } => "Scene",
            StoryNode::Jump { .. } => "Jump",
            StoryNode::SetVariable { .. } => "Set Var",
            StoryNode::SetFlag { .. } => "Set Flag",
            StoryNode::ScenePatch(_) => "Scene Patch",
            StoryNode::JumpIf { .. } => "Branch (If)",
            StoryNode::Start => "Start",
            StoryNode::End => "End",
            StoryNode::AudioAction { .. } => "Audio",
            StoryNode::Transition { .. } => "Transition",
            StoryNode::CharacterPlacement { .. } => "Placement",
            StoryNode::Generic(EventRaw::ExtCall { .. }) => "ExtCall",
            StoryNode::Generic(_) => "Generic Event",
        }
    }

    pub fn is_marker(&self) -> bool {
        matches!(self, StoryNode::Start | StoryNode::End)
    }

    pub fn can_connect_from(&self) -> bool {
        !matches!(self, StoryNode::End)
    }

    pub fn can_connect_to(&self) -> bool {
        !matches!(self, StoryNode::Start)
    }

    pub fn export_supported(&self) -> bool {
        !matches!(
            self,
            StoryNode::Start | StoryNode::End | StoryNode::Generic(_)
        ) || matches!(
            self,
            StoryNode::Generic(EventRaw::ExtCall { .. } | EventRaw::SetFlag { .. })
        )
    }
}
