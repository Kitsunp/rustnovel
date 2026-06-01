use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::assets::AssetId;
use crate::audio::AudioCommand;
use crate::authoring::validation::event_details::{
    validate_audio, validate_character, validate_transition, AudioValidation,
};
use crate::authoring::validation::scene::{validate_scene, validate_scene_patch};
use crate::authoring::validation::trace::parse_import_trace_context;
use crate::authoring::{
    is_unsafe_asset_ref, DiagnosticTarget, FieldPath, LintCode, LintIssue, NodeGraph,
    SemanticValue, SemanticValueKind, StoryNode, ValidationPhase,
};
use crate::error::{VnError, VnResult};
use crate::event::{
    AudioActionCompiled, AudioActionRaw, CharacterPatchCompiled, CharacterPlacementCompiled,
    ChoiceCompiled, ChoiceOptionCompiled, CmpOp, CondCompiled, CondRaw, DialogueCompiled,
    DialogueRaw, EventCompiled, EventRaw, ScenePatchCompiled, SceneTransitionCompiled,
    SceneTransitionRaw, SceneUpdateCompiled, SceneUpdateRaw, SetCharacterPositionCompiled,
    SetCharacterPositionRaw, SharedStr,
};
use crate::execution_contract::FidelityClass;
use crate::scene_frame::{InteractionSpec, LayoutRect, RenderCommand};
use crate::state::EngineState;
use crate::visual::VisualState;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    Dialogue,
    Choice,
    Scene,
    Jump,
    SetFlag,
    SetVar,
    JumpIf,
    Patch,
    ExtCall,
    AudioAction,
    Transition,
    SetCharacterPosition,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeKind {
    Start,
    End,
    Dialogue,
    Choice,
    Scene,
    Jump,
    SetVariable,
    SetFlag,
    ScenePatch,
    JumpIf,
    AudioAction,
    Transition,
    CharacterPlacement,
    ExtCall,
    SubgraphCall,
    GenericEvent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BehaviorSupport {
    Native,
    Simulated,
    HostRequired,
    PreviewOnly,
    Fallback,
    Unsupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventCapabilities {
    pub editor_supported: bool,
    pub preview_supported: bool,
    pub runtime_supported: bool,
    pub headless_supported: bool,
    pub export_supported: bool,
    pub python_supported: bool,
    pub cli_supported: bool,
    pub fidelity: FidelityClass,
    pub compile: BehaviorSupport,
    pub execute: BehaviorSupport,
    pub preview: BehaviorSupport,
    pub visual_state: BehaviorSupport,
    pub scene_frame: BehaviorSupport,
    pub interactions: BehaviorSupport,
    pub asset_refs: BehaviorSupport,
    pub validation: BehaviorSupport,
    pub quick_fixes: BehaviorSupport,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventFlow {
    Linear,
    SingleTarget,
    ConditionalTarget,
    ChoiceTargets,
}

impl EventFlow {
    pub const fn has_explicit_targets(self) -> bool {
        matches!(
            self,
            Self::SingleTarget | Self::ConditionalTarget | Self::ChoiceTargets
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct EventSpec {
    pub kind: EventKind,
    pub stable_name: &'static str,
    pub contract_name: &'static str,
    pub raw_schema: &'static str,
    pub compiled_schema: &'static str,
    pub flow: EventFlow,
    pub trace_kind: &'static str,
    pub raw_field_paths: &'static [&'static str],
    pub compiled_field_paths: &'static [&'static str],
    pub asset_ref_field_paths: &'static [&'static str],
    pub capabilities: EventCapabilities,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PortOutput {
    None,
    Linear,
    SingleTarget,
    ConditionalTrueFalse,
    ChoiceOptions,
    SubgraphExit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PortSpec {
    pub accepts_incoming: bool,
    pub output: PortOutput,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct InspectorSchema {
    pub field_paths: &'static [&'static str],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct NodeSpec {
    pub kind: NodeKind,
    pub stable_name: &'static str,
    pub display_name: &'static str,
    pub contract_name: &'static str,
    pub event_kind: Option<EventKind>,
    pub ports: PortSpec,
    pub editor_schema: InspectorSchema,
    pub asset_ref_field_paths: &'static [&'static str],
    pub capabilities: EventCapabilities,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeToEventError {
    KindMismatch,
    RequiresExportContext,
    PreviewOnlyMarker,
}

#[path = "event_behavior/asset_refs.rs"]
mod asset_refs;
#[path = "event_behavior/capabilities.rs"]
mod capabilities;
#[path = "event_behavior/context.rs"]
mod context;
#[path = "event_behavior/event_catalog.rs"]
mod event_catalog;
#[path = "event_behavior/fields.rs"]
mod fields;
#[path = "event_behavior/node_catalog.rs"]
mod node_catalog;
#[path = "event_behavior/quick_fix.rs"]
mod quick_fix;
#[path = "event_behavior/runtime.rs"]
mod runtime;
#[path = "event_behavior/validation.rs"]
mod validation;

pub use context::{
    BehaviorQuickFix, BehaviorQuickFixRisk, CompileCtx, ExecutionCtx, PreviewCtx, QuickFixCtx,
    SceneFrameCtx, ValidationCtx,
};
pub use event_catalog::{
    event_asset_refs_for_raw, event_behavior, event_behavior_for_compiled, event_behavior_for_raw,
    event_kind_for_compiled, event_kind_for_raw, event_spec, event_spec_for_compiled,
    event_spec_for_raw, event_specs,
};
pub use node_catalog::{
    node_asset_refs_for_authoring_node, node_behavior, node_behavior_for_authoring_node,
    node_from_event_raw, node_kind_for_authoring_node, node_kind_for_event_raw, node_spec,
    node_spec_for_authoring_node, node_specs, node_to_event_raw_without_export_context,
};
pub(crate) use quick_fix::{normalized_audio_action, normalized_audio_channel};
pub use validation::{node_quick_fixes_for_issue, validate_authoring_node};

use quick_fix::suggest_node_quick_fixes;
use runtime::{
    append_scene_frame_for_event, compile_event_raw, execute_event_compiled, preview_event_compiled,
};

pub trait EventBehavior {
    fn kind(&self) -> EventKind;

    fn spec(&self) -> &'static EventSpec {
        event_spec(self.kind())
    }

    fn capabilities(&self) -> EventCapabilities {
        self.spec().capabilities
    }

    fn compile_support(&self) -> BehaviorSupport {
        self.capabilities().compile
    }

    fn execute_support(&self) -> BehaviorSupport {
        self.capabilities().execute
    }

    fn preview_support(&self) -> BehaviorSupport {
        self.capabilities().preview
    }

    fn compile(&self, ctx: &mut CompileCtx<'_>, event: &EventRaw) -> VnResult<EventCompiled> {
        if self.kind() != event_kind_for_raw(event) {
            return Err(VnError::InvalidScript(format!(
                "event behavior kind mismatch: expected {}, got {}",
                self.spec().stable_name,
                event_spec_for_raw(event).stable_name
            )));
        }
        compile_event_raw(ctx, event)
    }

    fn execute(&self, ctx: &mut ExecutionCtx<'_>, event: &EventCompiled) -> VnResult<()> {
        if self.kind() != event_kind_for_compiled(event) {
            return Err(VnError::InvalidScript(format!(
                "event behavior kind mismatch: expected {}, got {}",
                self.spec().stable_name,
                event_spec_for_compiled(event).stable_name
            )));
        }
        ctx.begin_event();
        execute_event_compiled(ctx, event)
    }

    fn preview(&self, ctx: &mut PreviewCtx<'_>, event: &EventCompiled) -> VnResult<Option<String>> {
        if self.kind() != event_kind_for_compiled(event) {
            return Err(VnError::InvalidScript(format!(
                "event behavior kind mismatch: expected {}, got {}",
                self.spec().stable_name,
                event_spec_for_compiled(event).stable_name
            )));
        }
        Ok(preview_event_compiled(ctx, event))
    }

    fn append_scene_frame(
        &self,
        ctx: &mut SceneFrameCtx<'_>,
        event: &EventCompiled,
    ) -> VnResult<()> {
        if self.kind() != event_kind_for_compiled(event) {
            return Err(VnError::InvalidScript(format!(
                "event behavior kind mismatch: expected {}, got {}",
                self.spec().stable_name,
                event_spec_for_compiled(event).stable_name
            )));
        }
        append_scene_frame_for_event(ctx, event);
        Ok(())
    }

    fn trace_kind(&self) -> &'static str {
        self.spec().trace_kind
    }

    fn raw_field_paths(&self) -> &'static [&'static str] {
        self.spec().raw_field_paths
    }

    fn compiled_field_paths(&self) -> &'static [&'static str] {
        self.spec().compiled_field_paths
    }

    fn asset_ref_field_paths(&self) -> &'static [&'static str] {
        self.spec().asset_ref_field_paths
    }

    fn asset_refs(&self, event: &EventRaw) -> Vec<String> {
        debug_assert_eq!(self.kind(), event_kind_for_raw(event));
        event_asset_refs_for_raw(event)
    }

    fn asset_refs_support(&self) -> BehaviorSupport {
        self.capabilities().asset_refs
    }

    fn validation_support(&self) -> BehaviorSupport {
        self.capabilities().validation
    }

    fn quick_fixes_support(&self) -> BehaviorSupport {
        self.capabilities().quick_fixes
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StaticEventBehavior {
    kind: EventKind,
}

impl StaticEventBehavior {
    pub const fn new(kind: EventKind) -> Self {
        Self { kind }
    }
}

impl EventBehavior for StaticEventBehavior {
    fn kind(&self) -> EventKind {
        self.kind
    }
}

pub trait NodeBehavior {
    fn node_kind(&self) -> NodeKind;

    fn spec(&self) -> &'static NodeSpec {
        node_spec(self.node_kind())
    }

    fn ports(&self) -> PortSpec {
        self.spec().ports
    }

    fn editor_schema(&self) -> InspectorSchema {
        self.spec().editor_schema
    }

    fn event_kind(&self) -> Option<EventKind> {
        self.spec().event_kind
    }

    fn field_paths(&self) -> &'static [&'static str] {
        self.spec().editor_schema.field_paths
    }

    fn asset_ref_field_paths(&self) -> &'static [&'static str] {
        self.spec().asset_ref_field_paths
    }

    fn asset_refs(&self, node: &StoryNode) -> Vec<String> {
        debug_assert_eq!(self.node_kind(), node_kind_for_authoring_node(node));
        node_asset_refs_for_authoring_node(node)
    }

    fn event_to_node(&self, event: &EventRaw) -> Option<StoryNode> {
        (self.node_kind() == node_kind_for_event_raw(event)).then(|| node_from_event_raw(event))
    }

    fn to_event(&self, node: &StoryNode) -> Result<EventRaw, NodeToEventError> {
        if self.node_kind() != node_kind_for_authoring_node(node) {
            return Err(NodeToEventError::KindMismatch);
        }
        node_to_event_raw_without_export_context(node)
    }

    fn validate(&self, ctx: &ValidationCtx<'_>, node: &StoryNode) -> Vec<LintIssue> {
        debug_assert_eq!(self.node_kind(), node_kind_for_authoring_node(node));
        validate_authoring_node(ctx, node)
    }

    fn quick_fixes(&self, ctx: &QuickFixCtx<'_>, issue: &LintIssue) -> Vec<BehaviorQuickFix> {
        suggest_node_quick_fixes(ctx, self.node_kind(), issue)
    }

    fn capabilities(&self) -> EventCapabilities {
        self.spec().capabilities
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StaticNodeBehavior {
    kind: NodeKind,
}

impl StaticNodeBehavior {
    pub const fn new(kind: NodeKind) -> Self {
        Self { kind }
    }
}

impl NodeBehavior for StaticNodeBehavior {
    fn node_kind(&self) -> NodeKind {
        self.kind
    }
}
