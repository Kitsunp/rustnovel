use super::*;

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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AutoFixBatchSkip {
    pub diagnostic_id: String,
    pub fix_id: String,
    pub reason: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AutoFixBatchResult {
    pub applied: usize,
    pub skipped: usize,
    pub skipped_details: Vec<AutoFixBatchSkip>,
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
