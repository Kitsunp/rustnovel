mod assets;
mod audio;
pub mod authoring;
mod bundle;
mod engine;
mod entity;
mod error;
mod event;
mod execution_contract;
mod graph;
mod localization;
pub mod manifest;
mod migration;
mod protected_content;
mod render;
mod renpy_import;
mod repro;
mod resource;
mod script;
mod security;
mod state;
mod storage;
mod timeline;
mod trace;
mod ui;
mod version;
mod visual;

pub use assets::{AssetId, AssetId128, AssetManifest};
pub use audio::AudioCommand;
pub use authoring::{
    asset_exists_from_project_root, default_asset_exists, export_runtime_script_from_authoring,
    is_unsafe_asset_ref, load_authoring_document_or_script, load_runtime_script_from_entry,
    parse_authoring_document_or_script, parse_runtime_script_from_entry,
    quick_fix as authoring_quick_fix, should_probe_asset_exists, validate_authoring_graph,
    validate_authoring_graph_no_io, validate_authoring_graph_with_probe,
    validate_authoring_graph_with_project_root, validate_authoring_graph_with_resolver,
    AuthoringDocument, AuthoringPosition, CharacterPoseBinding,
    GraphConnection as AuthoringGraphConnection, LintCode as AuthoringLintCode,
    LintIssue as AuthoringLintIssue, LintSeverity as AuthoringLintSeverity,
    NodeGraph as AuthoringGraph, QuickFixCandidate, QuickFixRisk, SceneLayer, SceneProfile,
    StoryNode as AuthoringStoryNode, ValidationPhase, AUTHORING_DOCUMENT_SCHEMA_VERSION,
};
pub use bundle::{
    export_bundle, BundleAssetEntry, BundleIntegrity, ExportBundleReport, ExportBundleSpec,
    ExportTargetPlatform,
};
pub use engine::{ChoiceHistoryEntry, Engine, StateChange};
pub use error::{VnError, VnResult};
pub use event::{
    AudioActionCompiled, AudioActionRaw, CharacterPatchCompiled, CharacterPatchRaw,
    CharacterPlacementCompiled, CharacterPlacementRaw, ChoiceCompiled, ChoiceOptionCompiled,
    ChoiceOptionRaw, ChoiceRaw, CmpOp, CondCompiled, CondRaw, DialogueCompiled, DialogueRaw,
    EventCompiled, EventRaw, ScenePatchCompiled, ScenePatchRaw, SceneTransitionCompiled,
    SceneTransitionRaw, SceneUpdateCompiled, SceneUpdateRaw, SetCharacterPositionCompiled,
    SetCharacterPositionRaw, SharedStr,
};
pub use execution_contract::{
    contract_for_authoring_node, contract_for_event_raw, contract_matrix,
    is_preview_only_authoring_node, EventExecutionContract, FidelityClass,
};
pub use localization::{
    collect_script_localization_keys, localization_key, LocalizationCatalog, LocalizationIssue,
    LocalizationIssueKind,
};
pub use manifest::ProjectManifest;
pub use migration::{
    migrate_script_json_to_current, migrate_script_json_value, MigrationError, MigrationReport,
    MigrationTraceEntry,
};
pub use protected_content::{
    open_protected_content, protect_content, ProtectedContentChunk, ProtectedContentError,
    PROTECTED_CONTENT_VERSION,
};
pub use render::{RenderBackend, RenderOutput, TextRenderer};
pub use renpy_import::{
    import_renpy_project, ImportArea, ImportFallbackPolicy, ImportIssue, ImportPhase,
    ImportProfile, ImportRenpyOptions, ImportReport,
};
pub use repro::{
    run_repro_case, run_repro_case_with_limits, ReproCase, ReproMonitor, ReproMonitorResult,
    ReproOracle, ReproRunReport, ReproStepTrace, ReproStopReason, REPRO_CASE_SCHEMA,
};
pub use resource::{LruCache, ResourceLimiter};
pub use script::{ScriptCompiled, ScriptRaw};
pub use security::SecurityPolicy;
pub use state::EngineState;
pub use storage::{
    compute_script_id, SaveData, SaveError, SaveSlotEntry, SaveSlotMetadata, SaveSlotStore,
    SaveStoreError, ScriptId, AUTH_SAVE_KEY,
};
pub use trace::{StateDigest, UiTrace, UiTraceStep, UiView as TraceUiView, VisualDigest};
pub use ui::{UiState, UiView};
pub use version::{COMPILED_FORMAT_VERSION, SAVE_FORMAT_VERSION, SCRIPT_SCHEMA_VERSION};
pub use visual::VisualState;

// Phase 1: Entity System exports
pub use entity::{
    AudioData, CharacterData, Entity, EntityId, EntityKind, ImageData, SceneState, TextData,
    Transform, VideoData, MAX_ENTITIES,
};

// Phase 2: Timeline System exports
pub use timeline::{
    Easing, Fixed, Keyframe, PropertyType, PropertyValue, Timeline, TimelineError, Track,
    MAX_KEYFRAMES_PER_TRACK, MAX_TRACKS,
};

// Phase 3: Story Graph exports
pub use graph::{
    analyze_flow_graph, EdgeType, FlowGraphAnalysis, GraphEdge, GraphNode, GraphStats, NodeType,
    StoryGraph,
};

pub type Event = EventCompiled;
pub type Script = ScriptRaw;

// Python bindings are now handled in the `vnengine_py` crate.
// Core remains agnostic to the language binding layer.
