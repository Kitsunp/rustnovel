//! Editor module for the Visual Novel Engine.
//!
//! This module provides a visual editor workbench with:
//! - Timeline panel for keyframe editing
//! - Graph panel for story flow visualization
//! - Viewport for scene preview
//! - Inspector for entity properties

mod asset_browser;
mod asset_candidates;
mod asset_import;
pub mod compiler;
mod diagnostics;
mod diff_dialog;
mod errors;
pub mod execution_contract;
mod graph_panel;
mod inspector_panel;
mod lint_panel;
mod menu_bar;
mod node_editor;
mod node_graph;
mod node_rendering;
mod node_types;
mod player_ui;
pub mod project_io;
pub mod quick_fix;
mod scene_stage;
mod script_sync;
mod timeline_panel;
mod undo;
mod validator;
mod viewport_panel;
pub mod visual_composer;
mod visual_composer_preview;
mod workbench;

pub use asset_browser::{AssetBrowserAction, AssetBrowserPanel};
pub use asset_import::{AssetFieldTarget, AssetImportKind};
pub use diagnostics::{DiagnosticExplanation, DiagnosticLanguage};
pub use diff_dialog::DiffDialog;
pub use errors::EditorError;
pub use graph_panel::GraphPanel;
pub use inspector_panel::{InspectorAction, InspectorPanel};
pub use lint_panel::LintPanel;
pub use node_editor::NodeEditorPanel;
pub use node_graph::NodeGraph;
pub use node_types::{ContextMenu, StoryNode, ToastKind, ToastState};
pub use timeline_panel::TimelinePanel;
pub use undo::UndoStack;
pub use validator::{
    validate as validate_graph, LintCode, LintIssue, LintSeverity, ValidationPhase,
};
pub use viewport_panel::ViewportPanel;
pub use visual_composer::VisualComposerPanel;
pub use visual_composer_preview::{PreviewQuality, StageFit};
pub use workbench::EditorWorkbench;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorMode {
    Editor,
    Player,
}

use eframe::egui;

/// Runs the editor workbench as a standalone application.
pub fn run_editor() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 720.0])
            .with_title("Visual Novel Editor"),
        ..Default::default()
    };

    eframe::run_native(
        "Visual Novel Editor",
        options,
        Box::new(|cc| {
            cc.egui_ctx.set_visuals(egui::Visuals::dark());
            Box::new(EditorApp::default())
        }),
    )
}

/// The editor application wrapper for eframe.
struct EditorApp {
    workbench: EditorWorkbench,
}

impl Default for EditorApp {
    fn default() -> Self {
        Self {
            workbench: EditorWorkbench::new(crate::VnConfig::default()),
        }
    }
}

impl eframe::App for EditorApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Update timeline if playing (approximately 60 fps)
        if self.workbench.is_playing {
            self.workbench.update(1);
            ctx.request_repaint();
        }

        self.workbench.ui(ctx);
    }
}
