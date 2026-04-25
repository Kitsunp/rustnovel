//! Node types and constants for the visual editor.
//!
//! This module defines the visual representation of story nodes and
//! shared constants used throughout the node editor.
//!
//! # Design Principles
//! - **Modularity**: Separated from graph logic per Criterio J (500 line limit)
//! - **Single Responsibility**: Only node type definitions and their presentation

use serde::{Deserialize, Serialize};

// =============================================================================
// Constants
// =============================================================================

/// Minimum zoom level (25%)
pub const ZOOM_MIN: f32 = 0.25;
/// Maximum zoom level (400%)
pub const ZOOM_MAX: f32 = 4.0;
/// Default zoom level (100%)
pub const ZOOM_DEFAULT: f32 = 1.0;
/// Node visual width in pixels
pub const NODE_WIDTH: f32 = 140.0;
/// Node visual height in pixels
pub const NODE_HEIGHT: f32 = 70.0;
/// Vertical spacing for auto-layout
pub const NODE_VERTICAL_SPACING: f32 = 90.0;

// =============================================================================
// StoryNode - Visual representation of script events
// =============================================================================

/// Types of nodes in the story graph. Maps to `EventRaw` variants.
///
/// # Variants
/// - `Dialogue`: A character speaking text
/// - `Choice`: A branching point with multiple options
/// - `Scene`: A scene change (background, music, characters)
/// - `Jump`: An unconditional jump to a label
/// - `Start`: Entry point marker (not a real event)
/// - `End`: Terminal marker (not a real event)
///
/// # Invariant
/// Start and End nodes are markers only; they don't generate script events.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum StoryNode {
    /// Dialogue node with speaker and text.
    Dialogue { speaker: String, text: String },
    /// Choice node with prompt and options.
    Choice {
        prompt: String,
        options: Vec<String>,
    },
    /// Scene change node (full state update).
    Scene {
        profile: Option<String>,
        background: Option<String>,
        music: Option<String>,
        characters: Vec<visual_novel_engine::CharacterPlacementRaw>,
    },
    /// Jump to label.
    Jump { target: String },
    /// Set a variable.
    SetVariable { key: String, value: i32 },
    /// Patch/Update scene (audio, characters, etc.) without full replacement.
    ScenePatch(visual_novel_engine::ScenePatchRaw),
    /// Conditional jump.
    JumpIf {
        target: String,
        cond: visual_novel_engine::CondRaw,
    },
    /// Start node (entry point marker, not a real event).
    Start,
    /// End node (terminal marker).
    End,
    /// Audio action (BGM/SFX).
    AudioAction {
        channel: String,
        action: String,
        asset: Option<String>,
        volume: Option<f32>,
        fade_duration_ms: Option<u64>,
        loop_playback: Option<bool>,
    },
    /// Scene transition.
    Transition {
        kind: String,
        duration_ms: u32,
        color: Option<String>,
    },

    /// Character placement (Visual Composer).
    CharacterPlacement {
        name: String,
        x: i32,
        y: i32,
        scale: Option<f32>,
    },
    /// Generic node for unhandled events (preserves data).
    Generic(visual_novel_engine::EventRaw),
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
    /// Returns the display name for this node type.
    ///
    /// # Returns
    /// A human-readable name for the node type.
    #[inline]
    pub fn type_name(&self) -> &'static str {
        match self {
            StoryNode::Dialogue { .. } => "Dialogue",
            StoryNode::Choice { .. } => "Choice",
            StoryNode::Scene { .. } => "Scene",
            StoryNode::Jump { .. } => "Jump",
            StoryNode::SetVariable { .. } => "Set Var",
            StoryNode::ScenePatch(_) => "Scene Patch",
            StoryNode::JumpIf { .. } => "Branch (If)",
            StoryNode::Start => "Start",
            StoryNode::End => "End",
            StoryNode::AudioAction { .. } => "Audio",
            StoryNode::Transition { .. } => "Transition",
            StoryNode::CharacterPlacement { .. } => "Placement",
            StoryNode::Generic(visual_novel_engine::EventRaw::ExtCall { .. }) => "ExtCall",
            StoryNode::Generic(_) => "Generic Event",
        }
    }

    /// Returns the icon for this node type.
    ///
    /// # Returns
    /// An emoji representing the node type.
    #[inline]
    pub fn icon(&self) -> &'static str {
        match self {
            StoryNode::Dialogue { .. } => "💬",
            StoryNode::Choice { .. } => "🔀",
            StoryNode::Scene { .. } => "🎬",
            StoryNode::Jump { .. } => "↪",
            StoryNode::SetVariable { .. } => "💾",
            StoryNode::ScenePatch(_) => "🎭",
            StoryNode::JumpIf { .. } => "❓",
            StoryNode::Start => "▶",
            StoryNode::End => "⏹",
            StoryNode::AudioAction { .. } => "🔊",
            StoryNode::Transition { .. } => "⏳",
            StoryNode::CharacterPlacement { .. } => "🧍",
            StoryNode::Generic(visual_novel_engine::EventRaw::ExtCall { .. }) => "🧩",
            StoryNode::Generic(_) => "📦",
        }
    }

    /// Returns the background color for this node type.
    ///
    /// # Returns
    /// An egui Color32 for the node's visual representation.
    #[inline]
    pub fn color(&self) -> egui::Color32 {
        match self {
            StoryNode::Dialogue { .. } => egui::Color32::from_rgb(60, 80, 100),
            StoryNode::Choice { .. } => egui::Color32::from_rgb(100, 70, 90),
            StoryNode::Scene { .. } => egui::Color32::from_rgb(70, 90, 60),
            StoryNode::Jump { .. } => egui::Color32::from_rgb(90, 80, 50),
            StoryNode::SetVariable { .. } => egui::Color32::from_rgb(80, 50, 90),
            StoryNode::ScenePatch(_) => egui::Color32::from_rgb(60, 90, 80),
            StoryNode::JumpIf { .. } => egui::Color32::from_rgb(90, 60, 20),
            StoryNode::Start => egui::Color32::from_rgb(50, 100, 50),
            StoryNode::End => egui::Color32::from_rgb(100, 50, 50),
            StoryNode::AudioAction { .. } => egui::Color32::from_rgb(100, 100, 60), // Gold/Yellowish
            StoryNode::Transition { .. } => egui::Color32::from_rgb(60, 100, 100),  // Cyan/Teal
            StoryNode::CharacterPlacement { .. } => egui::Color32::from_rgb(100, 60, 100), // Purple-ish
            StoryNode::Generic(visual_novel_engine::EventRaw::ExtCall { .. }) => {
                egui::Color32::from_rgb(90, 85, 110)
            }
            StoryNode::Generic(_) => egui::Color32::from_rgb(80, 80, 80), // Gray for generic
        }
    }

    /// Returns whether this node is a marker (Start/End) that doesn't generate events.
    #[inline]
    pub fn is_marker(&self) -> bool {
        matches!(self, StoryNode::Start | StoryNode::End)
    }

    /// Returns whether this node can have outgoing connections.
    ///
    /// # Invariant
    /// End nodes should not have outgoing connections.
    #[inline]
    pub fn can_connect_from(&self) -> bool {
        !matches!(self, StoryNode::End)
    }

    /// Returns whether this node can receive incoming connections.
    ///
    /// # Invariant
    /// Start nodes should not have incoming connections.
    #[inline]
    pub fn can_connect_to(&self) -> bool {
        !matches!(self, StoryNode::Start)
    }
}

/// Returns the visual height used by graph hit-testing/layout for a node.
#[inline]
pub fn node_visual_height(node: &StoryNode) -> f32 {
    match node {
        StoryNode::Choice { options, .. } => {
            let header = 40.0;
            let option_h = 30.0;
            header + ((options.len() + 1).max(1) as f32 * option_h) + 10.0
        }
        _ => NODE_HEIGHT,
    }
}

// =============================================================================
// ContextMenu - State for right-click menu
// =============================================================================

/// Context menu state for node operations.
///
/// When the user right-clicks on a node, this struct stores which node
/// was clicked and where the menu should appear.
#[derive(Clone, Debug)]
pub struct ContextMenu {
    /// The ID of the node that was right-clicked.
    pub node_id: u32,
    /// The screen position where the menu should appear.
    pub position: egui::Pos2,
}

// =============================================================================
// Toast - Visual feedback notifications
// =============================================================================

/// Kind of toast notification.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ToastKind {
    /// Success - action completed
    Success,
    /// Warning - potential issue
    Warning,
    /// Error - action failed
    Error,
    /// Info - general information
    Info,
}

impl ToastKind {
    /// Returns the color for this toast kind.
    pub fn color(&self) -> egui::Color32 {
        match self {
            ToastKind::Success => egui::Color32::from_rgb(80, 180, 80),
            ToastKind::Warning => egui::Color32::from_rgb(200, 160, 60),
            ToastKind::Error => egui::Color32::from_rgb(200, 80, 80),
            ToastKind::Info => egui::Color32::from_rgb(80, 140, 200),
        }
    }

    /// Returns the icon for this toast kind.
    pub fn icon(&self) -> &'static str {
        match self {
            ToastKind::Success => "✓",
            ToastKind::Warning => "⚠",
            ToastKind::Error => "✗",
            ToastKind::Info => "ℹ",
        }
    }
}

/// State for a toast notification.
#[derive(Clone, Debug)]
pub struct ToastState {
    /// The message to display.
    pub message: String,
    /// The kind of toast.
    pub kind: ToastKind,
    /// Frames remaining before the toast disappears (~60 frames/sec).
    pub frames_remaining: u32,
}

impl ToastState {
    /// Creates a new toast with default duration (~2 seconds).
    pub fn new(message: impl Into<String>, kind: ToastKind) -> Self {
        Self {
            message: message.into(),
            kind,
            frames_remaining: 120, // ~2 seconds at 60fps
        }
    }

    /// Creates a success toast.
    pub fn success(message: impl Into<String>) -> Self {
        Self::new(message, ToastKind::Success)
    }

    /// Creates a warning toast.
    pub fn warning(message: impl Into<String>) -> Self {
        Self::new(message, ToastKind::Warning)
    }

    /// Creates an error toast.
    pub fn error(message: impl Into<String>) -> Self {
        Self::new(message, ToastKind::Error)
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_story_node_type_name() {
        assert_eq!(StoryNode::Start.type_name(), "Start");
        assert_eq!(StoryNode::End.type_name(), "End");
        assert_eq!(
            StoryNode::Dialogue {
                speaker: "A".to_string(),
                text: "B".to_string()
            }
            .type_name(),
            "Dialogue"
        );
    }

    #[test]
    fn test_story_node_is_marker() {
        assert!(StoryNode::Start.is_marker());
        assert!(StoryNode::End.is_marker());
        assert!(!StoryNode::default().is_marker());
    }

    #[test]
    fn test_story_node_connection_rules() {
        // Start can connect from but not to
        assert!(StoryNode::Start.can_connect_from());
        assert!(!StoryNode::Start.can_connect_to());

        // End can connect to but not from
        assert!(!StoryNode::End.can_connect_from());
        assert!(StoryNode::End.can_connect_to());

        // Dialogue can do both
        assert!(StoryNode::default().can_connect_from());
        assert!(StoryNode::default().can_connect_to());
    }

    #[test]
    fn test_default_dialogue() {
        let node = StoryNode::default();
        if let StoryNode::Dialogue { speaker, text } = node {
            assert_eq!(speaker, "Character");
            assert_eq!(text, "Enter dialogue...");
        } else {
            panic!("Default should be Dialogue");
        }
    }
}
