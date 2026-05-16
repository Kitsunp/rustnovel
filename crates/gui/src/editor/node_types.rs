//! Visual constants and presentation helpers for editor nodes.
//!
//! Semantic story nodes live in `visual_novel_engine::authoring`. The GUI keeps
//! only view-specific details here.

pub use visual_novel_engine::authoring::StoryNode;

pub const ZOOM_MIN: f32 = 0.25;
pub const ZOOM_MAX: f32 = 4.0;
pub const ZOOM_DEFAULT: f32 = 1.0;
pub const NODE_WIDTH: f32 = 140.0;
pub const NODE_HEIGHT: f32 = 70.0;
pub const NODE_VERTICAL_SPACING: f32 = 90.0;

pub trait StoryNodeVisualExt {
    fn icon(&self) -> &'static str;
    fn color(&self) -> egui::Color32;
}

impl StoryNodeVisualExt for StoryNode {
    #[inline]
    fn icon(&self) -> &'static str {
        match self {
            StoryNode::Dialogue { .. } => "D",
            StoryNode::Choice { .. } => "?",
            StoryNode::Scene { .. } => "S",
            StoryNode::Jump { .. } => "J",
            StoryNode::SetVariable { .. } => "$",
            StoryNode::SetFlag { .. } => "F",
            StoryNode::ScenePatch(_) => "P",
            StoryNode::JumpIf { .. } => "IF",
            StoryNode::Start => ">",
            StoryNode::End => "X",
            StoryNode::AudioAction { .. } => "A",
            StoryNode::Transition { .. } => "T",
            StoryNode::CharacterPlacement { .. } => "C",
            StoryNode::SubgraphCall { .. } => "SG",
            StoryNode::Generic(visual_novel_engine::EventRaw::ExtCall { .. }) => "EXT",
            StoryNode::Generic(_) => "G",
        }
    }

    #[inline]
    fn color(&self) -> egui::Color32 {
        match self {
            StoryNode::Dialogue { .. } => egui::Color32::from_rgb(60, 80, 100),
            StoryNode::Choice { .. } => egui::Color32::from_rgb(100, 70, 90),
            StoryNode::Scene { .. } => egui::Color32::from_rgb(70, 90, 60),
            StoryNode::Jump { .. } => egui::Color32::from_rgb(90, 80, 50),
            StoryNode::SetVariable { .. } => egui::Color32::from_rgb(80, 50, 90),
            StoryNode::SetFlag { .. } => egui::Color32::from_rgb(80, 70, 95),
            StoryNode::ScenePatch(_) => egui::Color32::from_rgb(60, 90, 80),
            StoryNode::JumpIf { .. } => egui::Color32::from_rgb(90, 60, 20),
            StoryNode::Start => egui::Color32::from_rgb(50, 100, 50),
            StoryNode::End => egui::Color32::from_rgb(100, 50, 50),
            StoryNode::AudioAction { .. } => egui::Color32::from_rgb(100, 100, 60),
            StoryNode::Transition { .. } => egui::Color32::from_rgb(60, 100, 100),
            StoryNode::CharacterPlacement { .. } => egui::Color32::from_rgb(100, 60, 100),
            StoryNode::SubgraphCall { .. } => egui::Color32::from_rgb(85, 95, 140),
            StoryNode::Generic(visual_novel_engine::EventRaw::ExtCall { .. }) => {
                egui::Color32::from_rgb(90, 85, 110)
            }
            StoryNode::Generic(_) => egui::Color32::from_rgb(80, 80, 80),
        }
    }
}

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

#[derive(Clone, Debug)]
pub struct ContextMenu {
    pub node_id: Option<u32>,
    pub position: egui::Pos2,
    pub graph_position: Option<egui::Pos2>,
}

impl ContextMenu {
    pub fn for_node(node_id: u32, position: egui::Pos2) -> Self {
        Self {
            node_id: Some(node_id),
            position,
            graph_position: None,
        }
    }

    pub fn for_canvas(position: egui::Pos2, graph_position: egui::Pos2) -> Self {
        Self {
            node_id: None,
            position,
            graph_position: Some(graph_position),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ToastKind {
    Success,
    Warning,
    Error,
    Info,
}

impl ToastKind {
    pub fn color(&self) -> egui::Color32 {
        match self {
            ToastKind::Success => egui::Color32::from_rgb(80, 180, 80),
            ToastKind::Warning => egui::Color32::from_rgb(200, 160, 60),
            ToastKind::Error => egui::Color32::from_rgb(200, 80, 80),
            ToastKind::Info => egui::Color32::from_rgb(80, 140, 200),
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            ToastKind::Success => "OK",
            ToastKind::Warning => "!",
            ToastKind::Error => "X",
            ToastKind::Info => "i",
        }
    }
}

#[derive(Clone, Debug)]
pub struct ToastState {
    pub message: String,
    pub kind: ToastKind,
    pub frames_remaining: u32,
}

impl ToastState {
    pub fn new(message: impl Into<String>, kind: ToastKind) -> Self {
        Self {
            message: message.into(),
            kind,
            frames_remaining: 120,
        }
    }

    pub fn success(message: impl Into<String>) -> Self {
        Self::new(message, ToastKind::Success)
    }

    pub fn warning(message: impl Into<String>) -> Self {
        Self::new(message, ToastKind::Warning)
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self::new(message, ToastKind::Error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn story_node_type_name_comes_from_core() {
        assert_eq!(StoryNode::Start.type_name(), "Start");
        assert_eq!(StoryNode::End.type_name(), "End");
        assert_eq!(
            StoryNode::Dialogue {
                speaker: "A".to_string(),
                text: "B".to_string(),
            }
            .type_name(),
            "Dialogue"
        );
    }

    #[test]
    fn story_node_connection_rules_come_from_core() {
        assert!(StoryNode::Start.can_connect_from());
        assert!(!StoryNode::Start.can_connect_to());
        assert!(!StoryNode::End.can_connect_from());
        assert!(StoryNode::End.can_connect_to());
        assert!(StoryNode::default().can_connect_from());
        assert!(StoryNode::default().can_connect_to());
    }

    #[test]
    fn story_node_visual_extension_is_gui_only() {
        assert_eq!(StoryNode::Start.icon(), ">");
        assert_eq!(
            StoryNode::Dialogue {
                speaker: "A".to_string(),
                text: "B".to_string(),
            }
            .color(),
            egui::Color32::from_rgb(60, 80, 100)
        );
    }

    #[test]
    fn context_menu_distinguishes_node_and_canvas_targets() {
        let node_menu = ContextMenu::for_node(7, egui::pos2(10.0, 20.0));
        assert_eq!(node_menu.node_id, Some(7));
        assert_eq!(node_menu.graph_position, None);

        let canvas_menu = ContextMenu::for_canvas(egui::pos2(10.0, 20.0), egui::pos2(30.0, 40.0));
        assert_eq!(canvas_menu.node_id, None);
        assert_eq!(canvas_menu.graph_position, Some(egui::pos2(30.0, 40.0)));
    }
}
