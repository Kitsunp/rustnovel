use directories::ProjectDirs;
use eframe::egui;
use serde::{Deserialize, Serialize};
use visual_novel_engine::{Engine, LocalizationCatalog, ScriptRaw};

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
    pub asset_width: Option<f32>,
    pub graph_width: Option<f32>,
    pub inspector_width: Option<f32>,
    pub validation_height: Option<f32>,
    pub timeline_height: Option<f32>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
struct LayoutPreferences {
    show_graph: bool,
    show_inspector: bool,
    show_timeline: bool,
    show_asset_browser: bool,
    node_editor_window_open: bool,
    #[serde(default)]
    layout_overrides: LayoutOverrides,
    #[serde(default)]
    composer_preview_quality: crate::editor::PreviewQuality,
    #[serde(default)]
    composer_stage_fit: crate::editor::StageFit,
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

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AutoFixBatchResult {
    pub applied: usize,
    pub skipped: usize,
}

/// Main editor workbench state and UI.
pub struct EditorWorkbench {
    pub config: VnConfig,
    pub node_graph: NodeGraph,
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

    // Selection
    pub selected_node: Option<u32>,
    pub selected_entity: Option<u32>,

    // Scene Data
    pub scene: visual_novel_engine::SceneState,
    pub composer_entity_owners: std::collections::HashMap<u32, u32>,
    pub composer_image_cache: std::collections::HashMap<String, egui::TextureHandle>,
    pub composer_image_failures: std::collections::HashMap<String, String>,
    pub composer_preview_quality: crate::editor::PreviewQuality,
    pub composer_stage_fit: crate::editor::StageFit,
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
    pub last_operation_fingerprint:
        Option<visual_novel_engine::authoring::AuthoringReportFingerprint>,
    pending_editor_operation: Option<(String, String, Option<String>)>,
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

        let layout_prefs_path = Self::layout_prefs_path();
        let loaded_prefs = Self::load_layout_prefs(&layout_prefs_path);

        let mut workbench = Self {
            config,
            node_graph: graph,
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
            selected_node: None,
            selected_entity: None,
            scene: visual_novel_engine::SceneState::default(),
            composer_entity_owners: std::collections::HashMap::new(),
            composer_image_cache: std::collections::HashMap::new(),
            composer_image_failures: std::collections::HashMap::new(),
            composer_preview_quality: crate::editor::PreviewQuality::default(),
            composer_stage_fit: crate::editor::StageFit::default(),
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
            },
        };

        if let Some(prefs) = loaded_prefs {
            workbench.apply_layout_prefs(&prefs);
        }
        workbench.last_layout_prefs = workbench.collect_layout_prefs();

        workbench
    }

    pub fn update(&mut self, _dt: usize) {
        if self.is_playing {
            // Simple tick approx 60fps or whatever dt implies
            self.current_time += 1.0;
            if self.current_time > self.timeline.duration() as f32 {
                self.current_time = 0.0;
                self.is_playing = false;
            }
        }
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
    }

    fn collect_layout_prefs(&self) -> LayoutPreferences {
        LayoutPreferences {
            show_graph: self.show_graph,
            show_inspector: self.show_inspector,
            show_timeline: self.show_timeline,
            show_asset_browser: self.show_asset_browser,
            node_editor_window_open: self.node_editor_window_open,
            layout_overrides: self.layout_overrides.clone(),
            composer_preview_quality: self.composer_preview_quality,
            composer_stage_fit: self.composer_stage_fit,
        }
    }

    fn persist_layout_prefs_if_changed(&mut self) {
        let now = self.collect_layout_prefs();
        if now == self.last_layout_prefs {
            return;
        }
        self.last_layout_prefs = now.clone();

        if let Some(parent) = self.layout_prefs_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(payload) = serde_json::to_string_pretty(&now) {
            let _ = std::fs::write(&self.layout_prefs_path, payload);
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
        self.composer_layer_overrides.clear();
        self.selected_entity = None;
        self.layout_generation = self.layout_generation.wrapping_add(1);
        ctx.memory_mut(|memory| memory.reset_areas());
        self.toast = Some(ToastState::success("Layout restablecido"));
    }

    pub(crate) fn queue_editor_operation(
        &mut self,
        kind: impl Into<String>,
        details: impl Into<String>,
        field_path: Option<String>,
    ) {
        self.pending_editor_operation = Some((kind.into(), details.into(), field_path));
    }

    pub(crate) fn refresh_operation_fingerprint(&mut self) {
        self.last_operation_fingerprint = self.current_authoring_fingerprint();
    }

    pub(crate) fn record_pending_editor_operation(&mut self) {
        let after = match self.current_authoring_fingerprint() {
            Some(after) => after,
            None => return,
        };
        let (kind, details, field_path) =
            self.pending_editor_operation.take().unwrap_or_else(|| {
                (
                    "editor_graph_mutation".to_string(),
                    "Graph changed through editor UI".to_string(),
                    None,
                )
            });
        let operation_id = format!("editor:{}:{}", kind, self.operation_log.len() + 1);
        let mut entry = visual_novel_engine::authoring::OperationLogEntry::new(
            operation_id,
            kind,
            "applied",
            details,
        );
        if let Some(before) = self.last_operation_fingerprint.as_ref() {
            entry = entry.with_before_after_fingerprints(before, &after);
        } else {
            entry = entry.with_fingerprint(&after);
        }
        if let Some(field_path) = field_path {
            entry = entry.with_field_path(field_path);
        }
        self.last_operation_fingerprint = Some(after);
        self.operation_log.push(entry);
    }

    fn current_authoring_fingerprint(
        &self,
    ) -> Option<visual_novel_engine::authoring::AuthoringReportFingerprint> {
        let script = self.node_graph.to_script();
        Some(
            visual_novel_engine::authoring::build_authoring_report_fingerprint(
                self.node_graph.authoring_graph(),
                &script,
            ),
        )
    }
}

mod app_ui;
mod asset_import_ops;
mod audio_preview_store;
mod compile_cache;
mod compile_ops;
mod composer_ops;
mod import_ops;
mod layout;
mod player_audio_ops;
mod player_audio_path;
mod player_mode_ops;
mod preview_fallback_ops;
mod project_ops;
mod quick_fix_ops;
mod report_ops;
mod repro_ops;
#[cfg(test)]
#[path = "tests/workbench_tests.rs"]
mod tests;
mod ui;
