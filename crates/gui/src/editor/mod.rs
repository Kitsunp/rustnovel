//! Editor module for the Visual Novel Engine.
//!
//! This module provides a visual editor workbench with:
//! - Timeline panel for keyframe editing
//! - Graph panel for story flow visualization
//! - Viewport for scene preview
//! - Inspector for entity properties

pub mod asset_browser;
pub mod asset_candidates;
pub mod asset_import;
pub mod atomic_io;
pub mod authoring_adapter;
pub mod compiler;
pub mod diagnostics;
pub mod diff_dialog;
pub mod errors;
pub mod execution_contract;
pub mod graph_panel;
pub mod image_asset_cache;
pub mod inspector_panel;
pub mod lint_panel;
pub mod menu_bar;
pub mod node_editor;
pub mod node_graph;
pub mod node_rendering;
pub mod node_types;
pub mod player_ui;
pub mod preview_policy;
pub mod project_io;
pub mod quick_fix;
pub mod resource_service;
pub mod route_tree_view;
pub mod scene_frame_presenter;
pub mod scene_stage;
pub mod script_sync;
pub mod timeline_panel;
pub mod undo;
pub mod validator;
pub mod viewport_panel;
pub mod visual_composer;
pub mod visual_composer_preview;
pub mod workbench;

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
pub use node_types::{ContextMenu, StoryNode, StoryNodeVisualExt, ToastKind, ToastState};
pub use preview_policy::{BackgroundFit, ComposerPreviewMode};
pub use route_tree_view::RouteTreeView;
pub use scene_frame_presenter::EguiSceneFramePresenter;
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
pub struct PlayerVisualPreferences {
    pub preview_quality: PreviewQuality,
    pub stage_fit: StageFit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorMode {
    Editor,
    Player,
}

use eframe::egui;

/// Runs the editor workbench as a standalone application.
pub fn run_editor() -> Result<(), eframe::Error> {
    run_editor_with_project(None)
}

/// Runs the editor workbench and optionally loads a project manifest on startup.
pub fn run_editor_with_project(
    initial_project: Option<std::path::PathBuf>,
) -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 720.0])
            .with_title("Visual Novel Editor"),
        ..Default::default()
    };

    eframe::run_native(
        "Visual Novel Editor",
        options,
        Box::new(move |cc| {
            cc.egui_ctx.set_visuals(egui::Visuals::dark());
            Box::new(EditorApp::new(initial_project.clone()))
        }),
    )
}

/// The editor application wrapper for eframe.
struct EditorApp {
    workbench: EditorWorkbench,
}

impl EditorApp {
    fn new(initial_project: Option<std::path::PathBuf>) -> Self {
        let mut workbench = EditorWorkbench::new(crate::VnConfig::default());
        if let Some(project) = initial_project {
            workbench.load_project(project);
        }
        Self { workbench }
    }
}

impl Default for EditorApp {
    fn default() -> Self {
        Self::new(None)
    }
}

impl eframe::App for EditorApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.workbench.is_playing {
            let dt = ctx.input(|input| input.stable_dt);
            self.workbench.update_seconds(dt);
            ctx.request_repaint();
        }

        self.workbench.ui(ctx);
    }
}
