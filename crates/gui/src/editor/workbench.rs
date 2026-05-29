use directories::ProjectDirs;
use eframe::egui;
use serde::{Deserialize, Serialize};
use visual_novel_engine::{
    runtime::{Engine, ScriptRaw},
    LocalizationCatalog,
};

use crate::editor::{
    asset_browser::AssetBrowserPanel,
    diagnostics::DiagnosticLanguage,
    diff_dialog::DiffDialog,
    inspector_panel::InspectorPanel,
    lint_panel::LintPanel,
    node_editor::NodeEditorPanel,
    node_graph::NodeGraph,
    node_types::ToastState,
    timeline_panel::TimelinePanel,
    undo::UndoStack,
    EditorMode,
    LintCode,
    LintIssue,
    LintSeverity, // Imported from mod.rs export
    ValidationPhase,
};
use crate::VnConfig;

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct LayoutOverrides {
    #[serde(default)]
    pub dock_reference_width: Option<f32>,
    pub asset_width: Option<f32>,
    pub graph_width: Option<f32>,
    pub inspector_width: Option<f32>,
    pub validation_height: Option<f32>,
    pub timeline_height: Option<f32>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct LayoutPreferences {
    pub show_graph: bool,
    pub show_inspector: bool,
    pub show_timeline: bool,
    pub show_asset_browser: bool,
    pub node_editor_window_open: bool,
    #[serde(default)]
    pub layout_overrides: LayoutOverrides,
    #[serde(default)]
    pub composer_preview_quality: crate::editor::PreviewQuality,
    #[serde(default)]
    pub composer_stage_fit: crate::editor::StageFit,
    #[serde(default)]
    pub composer_preview_mode: crate::editor::ComposerPreviewMode,
    #[serde(default)]
    pub composer_default_background_fit: crate::editor::BackgroundFit,
    #[serde(default)]
    pub workspace_layout: layout::WorkspaceLayout,
}

#[derive(Clone, Debug)]
pub struct QuickFixAuditEntry {
    pub operation_id: String,
    pub diagnostic_id: String,
    pub fix_id: String,
    pub node_id: Option<u32>,
    pub event_ip: Option<u32>,
    pub before_sha256: String,
    pub after_sha256: String,
}

#[derive(Clone, Debug)]
pub struct PendingStructuralFix {
    pub issue_index: usize,
    pub fix_id: String,
}

#[derive(Clone, Debug)]
pub struct PendingAutoFixOperation {
    pub issue: LintIssue,
    pub fix_id: String,
}

#[derive(Clone, Debug)]
pub struct PendingAutoFixBatch {
    pub include_review: bool,
    pub operations: Vec<PendingAutoFixOperation>,
}

#[derive(Clone, Debug)]
pub struct PendingEditorOperation {
    pub kind: String,
    pub details: String,
    pub field_path: Option<String>,
    pub before_value: Option<String>,
    pub after_value: Option<String>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AutoFixBatchResult {
    pub applied: usize,
    pub skipped: usize,
}

#[derive(Clone, Debug)]
pub struct ThemeEditorDraft {
    pub original: visual_novel_engine::UiTheme,
    pub draft: visual_novel_engine::UiTheme,
    pub preview_applied: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExportWizardKind {
    ExecutableGame,
    CompiledScriptBundle,
}

#[derive(Clone, Debug)]
pub struct ExportWizardState {
    pub export_kind: ExportWizardKind,
    pub target: visual_novel_engine::ExportTargetPlatform,
    pub output_root: String,
    pub runtime_artifact: String,
    pub entry_script: String,
    pub require_executable: bool,
    pub integrity: visual_novel_engine::BundleIntegrity,
    pub hmac_key: String,
    pub last_plan: Option<visual_novel_engine::ExportPlan>,
    pub last_error: Option<String>,
    pub last_report: Option<visual_novel_engine::ExportBundleReport>,
    pub dry_run: bool,
    pub logs: Vec<String>,
}

impl Default for ExportWizardState {
    fn default() -> Self {
        Self {
            export_kind: ExportWizardKind::ExecutableGame,
            target: if cfg!(target_os = "windows") {
                visual_novel_engine::ExportTargetPlatform::Windows
            } else if cfg!(target_os = "macos") {
                visual_novel_engine::ExportTargetPlatform::Macos
            } else {
                visual_novel_engine::ExportTargetPlatform::Linux
            },
            output_root: String::new(),
            runtime_artifact: String::new(),
            entry_script: String::new(),
            require_executable: true,
            integrity: visual_novel_engine::BundleIntegrity::None,
            hmac_key: String::new(),
            last_plan: None,
            last_error: None,
            last_report: None,
            dry_run: true,
            logs: Vec::new(),
        }
    }
}

/// Main editor workbench state and UI.
pub struct EditorWorkbench {
    pub config: VnConfig,
    pub node_graph: NodeGraph,
    pub authoring_session: visual_novel_engine::authoring::AuthoringDocumentSession,
    pub undo_stack: UndoStack,
    pub manifest: Option<visual_novel_engine::manifest::ProjectManifest>,
    pub manifest_path: Option<std::path::PathBuf>,
    pub project_root: Option<std::path::PathBuf>,
    pub current_script: Option<ScriptRaw>,
    pub saved_script_snapshot: Option<ScriptRaw>,
    pub pending_save_path: Option<std::path::PathBuf>,

    // UI State
    pub mode: EditorMode,
    pub show_graph: bool,
    pub show_inspector: bool,
    pub show_timeline: bool,
    pub show_node_editor: bool,
    pub show_asset_browser: bool,
    pub show_validation: bool,
    pub validation_collapsed: bool,
    pub show_save_confirm: bool,
    pub show_player_menu_settings: bool,
    pub show_theme_editor: bool,
    pub show_layout_debug_overlay: bool,
    pub show_scene_frame_inspector: bool,
    pub show_export_report_panel: bool,
    pub show_export_wizard: bool,
    pub show_profiler_cache_panel: bool,
    pub active_ui_theme: visual_novel_engine::UiTheme,
    pub theme_editor_draft: Option<ThemeEditorDraft>,
    pub export_wizard: ExportWizardState,
    pub last_export_report: Option<visual_novel_engine::ExportBundleReport>,

    // Selection
    pub selected_node: Option<u32>,
    pub selected_entity: Option<u32>,

    // Scene Data
    pub scene: visual_novel_engine::SceneState,
    pub composer_entity_owners: std::collections::HashMap<u32, u32>,
    pub composer_image_cache: std::collections::HashMap<String, egui::TextureHandle>,
    pub composer_image_failures: std::collections::HashMap<String, String>,
    pub resource_service: crate::editor::resource_service::EditorResourceService,
    pub composer_preview_quality: crate::editor::PreviewQuality,
    pub composer_stage_fit: crate::editor::StageFit,
    pub composer_preview_mode: crate::editor::ComposerPreviewMode,
    pub composer_default_background_fit: crate::editor::BackgroundFit,
    pub composer_background_fit_overrides:
        std::collections::HashMap<String, crate::editor::BackgroundFit>,
    pub composer_layer_overrides:
        std::collections::HashMap<String, crate::editor::visual_composer::LayerOverride>,

    // Timeline/Playback
    pub timeline: visual_novel_engine::Timeline,
    pub current_time: f32,
    pub is_playing: bool,
    pub player_state: crate::editor::player_ui::PlayerSessionState,

    // Engine Instance (for Player Mode)
    pub engine: Option<Engine>,
    pub player_audio_backend: Option<Box<dyn visual_novel_runtime::Audio>>,
    pub player_audio_root: Option<std::path::PathBuf>,

    // Validation
    pub validation_issues: Vec<LintIssue>,
    pub last_dry_run_report: Option<crate::editor::compiler::DryRunReport>,
    pub loaded_repro_case: Option<visual_novel_engine::ReproCase>,
    pub last_repro_report: Option<visual_novel_engine::ReproRunReport>,
    compilation_cache: compile_cache::CompilationCache,
    pub diagnostic_language: DiagnosticLanguage,
    pub player_locale: String,
    pub localization_catalog: LocalizationCatalog,
    pub selected_issue: Option<usize>,
    pub imported_report_stale: bool,
    pub imported_report_untrusted: bool,
    pub last_fix_snapshot: Option<NodeGraph>,
    pub quick_fix_audit: Vec<QuickFixAuditEntry>,
    pub operation_log: Vec<visual_novel_engine::authoring::OperationLogEntry>,
    pub verification_runs: Vec<visual_novel_engine::authoring::VerificationRun>,
    pub last_operation_fingerprint:
        Option<visual_novel_engine::authoring::AuthoringReportFingerprint>,
    pending_editor_operation: Option<PendingEditorOperation>,
    pub show_fix_confirm: bool,
    pub fix_diff_dialog: Option<DiffDialog>,
    pub pending_structural_fix: Option<PendingStructuralFix>,
    pub pending_auto_fix_batch: Option<PendingAutoFixBatch>,

    // Feedback
    pub toast: Option<ToastState>,
    pub diff_dialog: Option<DiffDialog>,

    // New layout flags
    pub node_editor_window_open: bool,
    pub layout_overrides: LayoutOverrides,
    workspace_layout: layout::WorkspaceLayout,
    layout_generation: u64,
    layout_prefs_path: std::path::PathBuf,
    last_layout_prefs: LayoutPreferences,
}

impl EditorWorkbench {
    fn append_phase_trace_issues(
        issues: &mut Vec<LintIssue>,
        traces: &[crate::editor::compiler::PhaseTrace],
    ) {
        for trace in traces {
            let phase = match trace.phase {
                crate::editor::compiler::CompilationPhase::GraphSync => ValidationPhase::Graph,
                crate::editor::compiler::CompilationPhase::GraphValidation => {
                    ValidationPhase::Graph
                }
                crate::editor::compiler::CompilationPhase::ScriptCompile => {
                    ValidationPhase::Compile
                }
                crate::editor::compiler::CompilationPhase::RuntimeInit => ValidationPhase::Runtime,
                crate::editor::compiler::CompilationPhase::DryRun => ValidationPhase::DryRun,
            };

            let entry = if trace.ok {
                LintIssue::info(
                    None,
                    phase,
                    LintCode::DryRunFinished,
                    format!("Phase {} OK: {}", trace.phase.label(), trace.detail),
                )
            } else {
                LintIssue::warning(
                    None,
                    phase,
                    LintCode::RuntimeInitError,
                    format!("Phase {} FAILED: {}", trace.phase.label(), trace.detail),
                )
            };
            issues.push(entry);
        }
    }

    pub fn new(config: VnConfig) -> Self {
        // Initialize with default/empty state
        let graph = NodeGraph::default();
        if graph.is_empty() {
            // Optional: graph.add_node(...)
        }

        let mut undo_stack = UndoStack::new();
        undo_stack.push(graph.clone());
        let authoring_session = visual_novel_engine::authoring::AuthoringDocumentSession::new(
            visual_novel_engine::authoring::AuthoringDocument::new(graph.authoring_graph().clone()),
        );

        let layout_prefs_path = Self::layout_prefs_path();
        let loaded_prefs = Self::load_layout_prefs(&layout_prefs_path);

        let mut workbench = Self {
            config,
            node_graph: graph,
            authoring_session,
            undo_stack,
            manifest: None,
            manifest_path: None,
            project_root: None,
            current_script: None,
            saved_script_snapshot: None,
            pending_save_path: None,
            mode: EditorMode::Editor,
            show_graph: true,
            show_inspector: true,
            show_timeline: true,
            show_node_editor: false,
            show_asset_browser: true,
            show_validation: false,
            validation_collapsed: false,
            show_save_confirm: false,
            show_player_menu_settings: false,
            show_theme_editor: false,
            show_layout_debug_overlay: false,
            show_scene_frame_inspector: false,
            show_export_report_panel: false,
            show_export_wizard: false,
            show_profiler_cache_panel: false,
            active_ui_theme: visual_novel_engine::UiTheme::default(),
            theme_editor_draft: None,
            export_wizard: ExportWizardState::default(),
            last_export_report: None,
            selected_node: None,
            selected_entity: None,
            scene: visual_novel_engine::SceneState::default(),
            composer_entity_owners: std::collections::HashMap::new(),
            composer_image_cache: std::collections::HashMap::new(),
            composer_image_failures: std::collections::HashMap::new(),
            resource_service: crate::editor::resource_service::EditorResourceService::new(),
            composer_preview_quality: crate::editor::PreviewQuality::default(),
            composer_stage_fit: crate::editor::StageFit::default(),
            composer_preview_mode: crate::editor::ComposerPreviewMode::default(),
            composer_default_background_fit: crate::editor::BackgroundFit::default(),
            composer_background_fit_overrides: std::collections::HashMap::new(),
            composer_layer_overrides: std::collections::HashMap::new(),
            timeline: visual_novel_engine::Timeline::new(60), // 60 ticks per second
            current_time: 0.0,
            is_playing: false,
            player_state: crate::editor::player_ui::PlayerSessionState::default(),
            engine: None,
            player_audio_backend: None,
            player_audio_root: None,
            validation_issues: Vec::new(),
            last_dry_run_report: None,
            loaded_repro_case: None,
            last_repro_report: None,
            compilation_cache: compile_cache::CompilationCache::default(),
            diagnostic_language: DiagnosticLanguage::Es,
            player_locale: "en".to_string(),
            localization_catalog: LocalizationCatalog::default(),
            selected_issue: None,
            imported_report_stale: false,
            imported_report_untrusted: false,
            last_fix_snapshot: None,
            quick_fix_audit: Vec::new(),
            operation_log: Vec::new(),
            verification_runs: Vec::new(),
            last_operation_fingerprint: None,
            pending_editor_operation: None,
            show_fix_confirm: false,
            fix_diff_dialog: None,
            pending_structural_fix: None,
            pending_auto_fix_batch: None,
            toast: None,
            diff_dialog: None,
            node_editor_window_open: false,
            layout_overrides: LayoutOverrides::default(),
            workspace_layout: layout::WorkspaceLayout::default(),
            layout_generation: 0,
            layout_prefs_path,
            last_layout_prefs: LayoutPreferences {
                show_graph: true,
                show_inspector: true,
                show_timeline: true,
                show_asset_browser: true,
                node_editor_window_open: false,
                layout_overrides: LayoutOverrides::default(),
                composer_preview_quality: crate::editor::PreviewQuality::default(),
                composer_stage_fit: crate::editor::StageFit::default(),
                composer_preview_mode: crate::editor::ComposerPreviewMode::default(),
                composer_default_background_fit: crate::editor::BackgroundFit::default(),
                workspace_layout: layout::WorkspaceLayout::default(),
            },
        };

        if let Some(prefs) = loaded_prefs {
            workbench.apply_layout_prefs(&prefs);
        }
        workbench.last_layout_prefs = workbench.collect_layout_prefs();

        workbench
    }

    pub fn update(&mut self, dt: usize) {
        if self.is_playing {
            let delta_ticks = u32::try_from(dt).unwrap_or(u32::MAX).max(1);
            self.timeline.advance(delta_ticks);
            self.current_time = self.timeline.current_time() as f32;
            if self.timeline.current_time() > self.timeline.duration() {
                self.current_time = 0.0;
                self.timeline.seek(0);
                self.is_playing = false;
            }
        }
    }

    pub fn update_seconds(&mut self, dt_seconds: f32) {
        let ticks_per_second = self.timeline.ticks_per_second.max(1) as f32;
        let ticks = (dt_seconds.max(0.0) * ticks_per_second).round().max(1.0) as usize;
        self.update(ticks);
    }

    fn layout_prefs_path() -> std::path::PathBuf {
        if let Some(project_dirs) = ProjectDirs::from("com", "vnengine", "editor") {
            project_dirs.config_dir().join("layout.json")
        } else {
            std::path::PathBuf::from("editor_layout.json")
        }
    }

    fn load_layout_prefs(path: &std::path::Path) -> Option<LayoutPreferences> {
        let raw = std::fs::read_to_string(path).ok()?;
        serde_json::from_str(&raw).ok()
    }

    fn apply_layout_prefs(&mut self, prefs: &LayoutPreferences) {
        self.show_graph = prefs.show_graph;
        self.show_inspector = prefs.show_inspector;
        self.show_timeline = prefs.show_timeline;
        self.show_asset_browser = prefs.show_asset_browser;
        self.node_editor_window_open = prefs.node_editor_window_open;
        self.layout_overrides = prefs.layout_overrides.clone();
        self.composer_preview_quality = prefs.composer_preview_quality;
        self.composer_stage_fit = prefs.composer_stage_fit;
        self.composer_preview_mode = prefs.composer_preview_mode;
        self.composer_default_background_fit = prefs.composer_default_background_fit;
        self.workspace_layout = prefs.workspace_layout.clone();
        self.workspace_layout.normalize();
        self.apply_workspace_layout_flags();
    }

    pub fn collect_layout_prefs(&self) -> LayoutPreferences {
        LayoutPreferences {
            show_graph: self.show_graph,
            show_inspector: self.show_inspector,
            show_timeline: self.show_timeline,
            show_asset_browser: self.show_asset_browser,
            node_editor_window_open: self.node_editor_window_open,
            layout_overrides: self.layout_overrides.clone(),
            composer_preview_quality: self.composer_preview_quality,
            composer_stage_fit: self.composer_stage_fit,
            composer_preview_mode: self.composer_preview_mode,
            composer_default_background_fit: self.composer_default_background_fit,
            workspace_layout: self.workspace_layout_from_current_flags(),
        }
    }

    fn persist_layout_prefs_if_changed(&mut self) {
        let now = self.collect_layout_prefs();
        if now == self.last_layout_prefs {
            return;
        }
        self.last_layout_prefs = now.clone();

        if let Some(parent) = self.layout_prefs_path.parent() {
            if let Err(err) = std::fs::create_dir_all(parent) {
                self.toast = Some(ToastState::warning(format!(
                    "Layout preferences folder could not be created: {err}"
                )));
                return;
            }
        }
        let payload = match serde_json::to_string_pretty(&now) {
            Ok(payload) => payload,
            Err(err) => {
                self.toast = Some(ToastState::warning(format!(
                    "Layout preferences could not be serialized: {err}"
                )));
                return;
            }
        };
        if let Err(err) =
            crate::editor::atomic_io::atomic_replace(&self.layout_prefs_path, payload.as_bytes())
        {
            self.toast = Some(ToastState::warning(format!(
                "Layout preferences could not be saved: {err}"
            )));
        }
    }

    pub fn apply_layout_size_overrides(&mut self) {
        self.layout_generation = self.layout_generation.wrapping_add(1);
    }

    pub fn clear_layout_size_overrides(&mut self) {
        self.layout_overrides = LayoutOverrides::default();
        self.apply_layout_size_overrides();
    }

    pub fn reset_layout_state(&mut self, ctx: &egui::Context) {
        self.show_graph = true;
        self.show_inspector = true;
        self.show_timeline = true;
        self.show_asset_browser = true;
        self.node_editor_window_open = false;
        self.validation_collapsed = false;
        self.layout_overrides = LayoutOverrides::default();
        self.composer_preview_quality = crate::editor::PreviewQuality::default();
        self.composer_stage_fit = crate::editor::StageFit::default();
        self.composer_preview_mode = crate::editor::ComposerPreviewMode::default();
        self.composer_default_background_fit = crate::editor::BackgroundFit::default();
        self.composer_background_fit_overrides.clear();
        self.composer_layer_overrides.clear();
        self.rebuild_authoring_session_from_fields();
        self.selected_entity = None;
        self.sync_workspace_layout_from_flags();
        self.layout_generation = self.layout_generation.wrapping_add(1);
        ctx.memory_mut(|memory| memory.reset_areas());
        self.toast = Some(ToastState::success("Layout restablecido"));
    }
}

mod app_ui;
mod asset_import_ops;
pub mod audio_preview_store;
mod compile_cache;
mod compile_ops;
mod composer_mutations;
mod composer_ops;
mod fragments_ui;
mod import_ops;
pub mod layout;
mod operation_ops;
mod player_audio_ops;
mod player_audio_path;
mod player_mode_ops;
mod player_render_ops;
mod preview_fallback_ops;
mod preview_policy_ops;
mod project_ops;
mod quick_fix_ops;
mod report_ops;
mod repro_ops;
mod ui;
mod ui_actions;
mod workspace_layout_ops;
