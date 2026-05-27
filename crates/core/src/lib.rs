mod asset_refs;
mod assets;
mod audio;
pub mod authoring;
mod bundle;
mod clock;
mod engine;
mod entity;
mod error;
mod event;
mod event_signature;
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
pub mod runtime;
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
pub use authoring::{
    asset_exists_from_project_root, export_runtime_script_from_authoring, is_unsafe_asset_ref,
    load_authoring_document_or_script, load_runtime_script_from_entry,
    parse_authoring_document_or_script, parse_runtime_script_from_entry,
    quick_fix as authoring_quick_fix, validate_authoring_graph, validate_authoring_graph_no_io,
    validate_authoring_graph_with_probe, validate_authoring_graph_with_project_root,
    validate_authoring_graph_with_resolver, AuthoringDocument, AuthoringPosition,
    CharacterPoseBinding, GraphConnection as AuthoringGraphConnection,
    LintCode as AuthoringLintCode, LintIssue as AuthoringLintIssue,
    LintSeverity as AuthoringLintSeverity, NodeGraph as AuthoringGraph, QuickFixCandidate,
    QuickFixRisk, SceneLayer, SceneProfile, StoryNode as AuthoringStoryNode, ValidationPhase,
    AUTHORING_DOCUMENT_SCHEMA_VERSION,
};
pub use bundle::{
    build_export_plan, export_bundle, BundleAssetEntry, BundleIntegrity, ExportBundleReport,
    ExportBundleSpec, ExportPlan, ExportTargetPlatform,
};
pub use error::{VnError, VnResult};
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
pub use security::SecurityPolicy;
pub use storage::{
    compute_script_id, SaveData, SaveError, SaveSlotEntry, SaveSlotMetadata, SaveSlotStore,
    SaveStoreError, ScriptId, AUTH_SAVE_KEY,
};
pub use version::{COMPILED_FORMAT_VERSION, SAVE_FORMAT_VERSION, SCRIPT_SCHEMA_VERSION};

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

// Python bindings are now handled in the `vnengine_py` crate.
// Core remains agnostic to the language binding layer.
