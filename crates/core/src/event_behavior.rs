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

pub struct CompileCtx<'a> {
    labels: &'a BTreeMap<String, u32>,
    pool: StringPool,
    flag_map: HashMap<String, u32>,
    var_map: HashMap<String, u32>,
}

impl<'a> CompileCtx<'a> {
    pub fn new(labels: &'a BTreeMap<String, u32>) -> Self {
        Self {
            labels,
            pool: StringPool::default(),
            flag_map: HashMap::new(),
            var_map: HashMap::new(),
        }
    }

    pub fn flag_count(&self) -> u32 {
        self.flag_map.len() as u32
    }

    pub fn intern(&mut self, value: &str) -> SharedStr {
        self.pool.intern(value)
    }

    pub fn resolve_target(&self, event_label: &str, target: &str) -> VnResult<u32> {
        self.labels.get(target).copied().ok_or_else(|| {
            VnError::InvalidScript(format!("{event_label} target '{target}' not found"))
        })
    }

    pub fn flag_id(&mut self, key: &str) -> VnResult<u32> {
        get_or_insert_id(&mut self.flag_map, key)
    }

    pub fn var_id(&mut self, key: &str) -> VnResult<u32> {
        get_or_insert_id(&mut self.var_map, key)
    }

    pub fn compile_cond(&mut self, cond: &CondRaw) -> VnResult<CondCompiled> {
        match cond {
            CondRaw::Flag { key, is_set } => {
                let flag_id = self.flag_id(key)?;
                Ok(CondCompiled::Flag {
                    flag_id,
                    is_set: *is_set,
                })
            }
            CondRaw::VarCmp { key, op, value } => {
                let var_id = self.var_id(key)?;
                Ok(CondCompiled::VarCmp {
                    var_id,
                    op: *op,
                    value: *value,
                })
            }
        }
    }
}

pub struct ExecutionCtx<'a> {
    state: &'a mut EngineState,
    script_events: &'a [EventCompiled],
    audio_commands: &'a mut Vec<AudioCommand>,
    read_dialogue_ips: &'a mut BTreeSet<u32>,
    route_visited_ips: &'a mut BTreeSet<u32>,
    pending_transition: &'a mut Option<SceneTransitionCompiled>,
}

impl<'a> ExecutionCtx<'a> {
    pub fn new(
        state: &'a mut EngineState,
        script_events: &'a [EventCompiled],
        audio_commands: &'a mut Vec<AudioCommand>,
        read_dialogue_ips: &'a mut BTreeSet<u32>,
        route_visited_ips: &'a mut BTreeSet<u32>,
        pending_transition: &'a mut Option<SceneTransitionCompiled>,
    ) -> Self {
        Self {
            state,
            script_events,
            audio_commands,
            read_dialogue_ips,
            route_visited_ips,
            pending_transition,
        }
    }

    pub fn begin_event(&mut self) {
        let current_ip = self.state.position;
        self.route_visited_ips.insert(current_ip);
        *self.pending_transition = None;
    }

    pub fn advance_position(&mut self) -> VnResult<()> {
        let next = self.state.position.saturating_add(1);
        if next as usize >= self.script_events.len() {
            self.state.position = self.script_events.len() as u32;
            return Ok(());
        }
        self.state.position = next;
        self.route_visited_ips.insert(self.state.position);
        Ok(())
    }

    pub fn jump_to_ip(&mut self, target_ip: u32) -> VnResult<()> {
        if target_ip as usize > self.script_events.len() {
            return Err(VnError::InvalidScript(format!(
                "jump target '{target_ip}' outside script"
            )));
        }
        if target_ip as usize == self.script_events.len() {
            self.state.position = target_ip;
            return Ok(());
        }
        let scene = match self.script_events.get(target_ip as usize) {
            Some(EventCompiled::Scene(scene)) => Some(scene.clone()),
            _ => None,
        };
        self.state.position = target_ip;
        self.route_visited_ips.insert(target_ip);
        if let Some(scene) = scene {
            let before_music = self.state.visual.music.clone();
            self.state.visual.apply_scene(&scene);
            append_music_delta(before_music, &self.state.visual.music, self.audio_commands);
        }
        Ok(())
    }

    fn current_ip(&self) -> u32 {
        self.state.position
    }

    fn evaluate_cond(&self, cond: &CondCompiled) -> bool {
        match cond {
            CondCompiled::Flag { flag_id, is_set } => self.state.get_flag(*flag_id) == *is_set,
            CondCompiled::VarCmp { var_id, op, value } => {
                let var_val = self.state.get_var(*var_id);
                match op {
                    CmpOp::Eq => var_val == *value,
                    CmpOp::Ne => var_val != *value,
                    CmpOp::Lt => var_val < *value,
                    CmpOp::Le => var_val <= *value,
                    CmpOp::Gt => var_val > *value,
                    CmpOp::Ge => var_val >= *value,
                }
            }
        }
    }
}

pub struct PreviewCtx<'a> {
    visual: &'a mut VisualState,
}

impl<'a> PreviewCtx<'a> {
    pub fn new(visual: &'a mut VisualState) -> Self {
        Self { visual }
    }

    pub fn visual(&self) -> &VisualState {
        self.visual
    }

    pub fn visual_mut(&mut self) -> &mut VisualState {
        self.visual
    }
}

pub struct SceneFrameCtx<'a> {
    commands: &'a mut Vec<RenderCommand>,
    interactions: &'a mut Vec<InteractionSpec>,
}

impl<'a> SceneFrameCtx<'a> {
    pub fn new(
        commands: &'a mut Vec<RenderCommand>,
        interactions: &'a mut Vec<InteractionSpec>,
    ) -> Self {
        Self {
            commands,
            interactions,
        }
    }
}

pub struct ValidationCtx<'a> {
    graph: &'a NodeGraph,
    node_id: u32,
    script_labels: &'a BTreeSet<String>,
    asset_exists: &'a dyn Fn(&str) -> bool,
}

impl<'a> ValidationCtx<'a> {
    pub fn new(
        graph: &'a NodeGraph,
        node_id: u32,
        script_labels: &'a BTreeSet<String>,
        asset_exists: &'a dyn Fn(&str) -> bool,
    ) -> Self {
        Self {
            graph,
            node_id,
            script_labels,
            asset_exists,
        }
    }

    pub fn graph(&self) -> &NodeGraph {
        self.graph
    }

    pub fn node_id(&self) -> u32 {
        self.node_id
    }

    pub fn script_labels(&self) -> &BTreeSet<String> {
        self.script_labels
    }

    pub fn asset_exists(&self, asset: &str) -> bool {
        (self.asset_exists)(asset)
    }
}

pub struct QuickFixCtx<'a> {
    graph: &'a NodeGraph,
}

impl<'a> QuickFixCtx<'a> {
    pub fn new(graph: &'a NodeGraph) -> Self {
        Self { graph }
    }

    pub fn graph(&self) -> &NodeGraph {
        self.graph
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BehaviorQuickFixRisk {
    Safe,
    Review,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BehaviorQuickFix {
    pub fix_id: &'static str,
    pub title_es: &'static str,
    pub title_en: &'static str,
    pub risk: BehaviorQuickFixRisk,
    pub structural: bool,
}

impl BehaviorQuickFix {
    pub const fn new(
        fix_id: &'static str,
        title_es: &'static str,
        title_en: &'static str,
        risk: BehaviorQuickFixRisk,
        structural: bool,
    ) -> Self {
        Self {
            fix_id,
            title_es,
            title_en,
            risk,
            structural,
        }
    }
}

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

pub fn validate_authoring_node(ctx: &ValidationCtx<'_>, node: &StoryNode) -> Vec<LintIssue> {
    let mut issues = Vec::new();
    let id = ctx.node_id();
    if !node.is_marker() && !node.export_supported() {
        issues.push(
            LintIssue::error(
                Some(id),
                ValidationPhase::Graph,
                LintCode::ContractUnsupportedExport,
                "Node is not export-compatible",
            )
            .with_target(DiagnosticTarget::Node { node_id: id })
            .with_field_path(format!("graph.nodes[{id}]"))
            .with_evidence_trace(),
        );
    }

    match node {
        StoryNode::Dialogue { speaker, .. } if speaker.trim().is_empty() => {
            issues.push(
                LintIssue::warning(
                    Some(id),
                    ValidationPhase::Graph,
                    LintCode::EmptySpeakerName,
                    "Dialogue speaker is empty",
                )
                .with_target(DiagnosticTarget::Character {
                    node_id: Some(id),
                    name: speaker.clone(),
                    field_path: Some(FieldPath::new(format!("graph.nodes[{id}].speaker"))),
                })
                .with_field_path(format!("graph.nodes[{id}].speaker"))
                .with_semantic_value(SemanticValue::new(
                    SemanticValueKind::CharacterRef,
                    speaker.clone(),
                    format!("graph.nodes[{id}].speaker"),
                ))
                .with_evidence_trace(),
            );
        }
        StoryNode::Choice { options, .. } => validate_choice(ctx.graph(), id, options, &mut issues),
        StoryNode::Scene {
            profile,
            background,
            music,
            characters,
        } => {
            if let Some(profile) = profile {
                if ctx.graph().scene_profile(profile).is_none() {
                    issues.push(
                        LintIssue::error(
                            Some(id),
                            ValidationPhase::Graph,
                            LintCode::AssetReferenceMissing,
                            "Scene profile does not exist",
                        )
                        .with_asset_path(Some(profile.clone()))
                        .with_target(DiagnosticTarget::SceneProfile {
                            profile_id: profile.clone(),
                        })
                        .with_field_path(format!("graph.nodes[{id}].profile"))
                        .with_semantic_value(SemanticValue::new(
                            SemanticValueKind::AssetRef,
                            profile.clone(),
                            format!("graph.nodes[{id}].profile"),
                        ))
                        .with_evidence_trace(),
                    );
                }
            }
            let asset_exists = |asset: &str| ctx.asset_exists(asset);
            validate_scene(
                id,
                background,
                music,
                characters,
                &asset_exists,
                &mut issues,
            );
        }
        StoryNode::ScenePatch(patch) => {
            let asset_exists = |asset: &str| ctx.asset_exists(asset);
            validate_scene_patch(id, patch, &asset_exists, &mut issues);
        }
        StoryNode::Jump { target } if !has_exportable_connected_target(ctx.graph(), id) => {
            validate_jump_target(id, target, ctx.script_labels(), &mut issues);
        }
        StoryNode::JumpIf { target, cond } => {
            if cond_key_empty(cond) {
                issues.push(
                    LintIssue::error(
                        Some(id),
                        ValidationPhase::Graph,
                        LintCode::EmptyStateKey,
                        "JumpIf condition key is empty",
                    )
                    .with_target(DiagnosticTarget::JumpTarget {
                        node_id: id,
                        target: target.clone(),
                    })
                    .with_field_path(format!("graph.nodes[{id}].cond.key"))
                    .with_evidence_trace(),
                );
            }
            if !has_exportable_connected_target(ctx.graph(), id) {
                validate_jump_target(id, target, ctx.script_labels(), &mut issues);
            }
        }
        StoryNode::SetVariable { key, .. } | StoryNode::SetFlag { key, .. }
            if key.trim().is_empty() =>
        {
            issues.push(
                LintIssue::error(
                    Some(id),
                    ValidationPhase::Graph,
                    LintCode::EmptyStateKey,
                    "State key is empty",
                )
                .with_field_path(format!("graph.nodes[{id}].key"))
                .with_semantic_value(SemanticValue::new(
                    SemanticValueKind::VariableRef,
                    key.clone(),
                    format!("graph.nodes[{id}].key"),
                ))
                .with_evidence_trace(),
            );
        }
        StoryNode::AudioAction {
            channel,
            action,
            asset,
            volume,
            fade_duration_ms,
            ..
        } => {
            let asset_exists = |asset: &str| ctx.asset_exists(asset);
            validate_audio(
                AudioValidation {
                    id,
                    channel,
                    action,
                    asset,
                    volume,
                    fade_duration_ms,
                },
                &asset_exists,
                &mut issues,
            );
        }
        StoryNode::Transition {
            kind, duration_ms, ..
        } => validate_transition(id, kind, *duration_ms, &mut issues),
        StoryNode::CharacterPlacement { name, scale, .. } => {
            validate_character(id, name, scale, &mut issues)
        }
        StoryNode::SubgraphCall { .. } => {}
        StoryNode::Generic(event) => validate_generic_node(id, event, &mut issues),
        _ => {}
    }

    if !matches!(node, StoryNode::End) && !ctx.graph().connections().any(|conn| conn.from == id) {
        issues.push(
            LintIssue::warning(
                Some(id),
                ValidationPhase::Graph,
                LintCode::DeadEnd,
                "Node has no outgoing transition",
            )
            .with_target(DiagnosticTarget::Node { node_id: id })
            .with_evidence_trace(),
        );
    }
    issues
}

pub fn node_quick_fixes_for_issue(
    ctx: &QuickFixCtx<'_>,
    issue: &LintIssue,
) -> Vec<BehaviorQuickFix> {
    let Some(node_id) = issue.node_id else {
        return Vec::new();
    };
    let Some(node) = ctx.graph().get_node(node_id) else {
        return Vec::new();
    };
    node_behavior_for_authoring_node(node).quick_fixes(ctx, issue)
}

fn validate_generic_node(id: u32, event: &EventRaw, issues: &mut Vec<LintIssue>) {
    let mut issue = LintIssue::warning(
        Some(id),
        ValidationPhase::Graph,
        LintCode::GenericEventUnchecked,
        "Generic event has limited semantic validation",
    );
    if let EventRaw::ExtCall { command, args } = event {
        if let Some(trace) = parse_import_trace_context(args) {
            let ip_segment = trace
                .event_ip
                .map(|ip| format!(" ip={ip}"))
                .unwrap_or_default();
            let snippet_segment = trace
                .snippet
                .as_deref()
                .filter(|value| !value.trim().is_empty())
                .map(|value| format!(" snippet='{}'", value.trim()))
                .unwrap_or_default();
            issue.message = format!(
                "Import fallback extcall '{}' requires review (trace_id={}, code={}, source={}, area={}, phase={}{}{})",
                command,
                trace.trace_id,
                trace.issue_code,
                trace.source_command,
                trace.area,
                trace.phase,
                ip_segment,
                snippet_segment
            );
            issue = issue
                .with_blocked_by(trace.blocked_by)
                .with_target(DiagnosticTarget::Generic {
                    field_path: Some(FieldPath::new(format!("graph.nodes[{id}].generic"))),
                })
                .with_field_path(format!("graph.nodes[{id}].generic"))
                .with_semantic_value(SemanticValue::new(
                    SemanticValueKind::PluginRef,
                    command.clone(),
                    format!("graph.nodes[{id}].generic.command"),
                ))
                .with_evidence_trace();
        }
    }
    issues.push(issue);
}

fn has_exportable_connected_target(graph: &NodeGraph, id: u32) -> bool {
    graph.connections().any(|conn| {
        conn.from == id
            && conn.from_port == 0
            && graph
                .get_node(conn.to)
                .is_some_and(|node| node.is_marker() || node.export_supported())
    })
}

fn validate_jump_target(
    id: u32,
    target: &str,
    script_labels: &BTreeSet<String>,
    issues: &mut Vec<LintIssue>,
) {
    let target = target.trim();
    if target.is_empty() {
        issues.push(
            LintIssue::warning(
                Some(id),
                ValidationPhase::Graph,
                LintCode::EmptyJumpTarget,
                "Jump target is empty",
            )
            .with_target(DiagnosticTarget::JumpTarget {
                node_id: id,
                target: target.to_string(),
            })
            .with_field_path(format!("graph.nodes[{id}].target"))
            .with_semantic_value(SemanticValue::new(
                SemanticValueKind::LabelRef,
                target,
                format!("graph.nodes[{id}].target"),
            ))
            .with_evidence_trace(),
        );
    } else if !script_labels.contains(target) {
        issues.push(
            LintIssue::error(
                Some(id),
                ValidationPhase::Graph,
                LintCode::MissingJumpTarget,
                format!("Jump target '{target}' does not exist"),
            )
            .with_target(DiagnosticTarget::JumpTarget {
                node_id: id,
                target: target.to_string(),
            })
            .with_field_path(format!("graph.nodes[{id}].target"))
            .with_semantic_value(SemanticValue::new(
                SemanticValueKind::LabelRef,
                target,
                format!("graph.nodes[{id}].target"),
            ))
            .with_evidence_trace(),
        );
    }
}

fn cond_key_empty(cond: &CondRaw) -> bool {
    match cond {
        CondRaw::Flag { key, .. } | CondRaw::VarCmp { key, .. } => key.trim().is_empty(),
    }
}

fn validate_choice(graph: &NodeGraph, id: u32, options: &[String], issues: &mut Vec<LintIssue>) {
    if options.is_empty() {
        issues.push(
            LintIssue::error(
                Some(id),
                ValidationPhase::Graph,
                LintCode::ChoiceNoOptions,
                "Choice has no options",
            )
            .with_target(DiagnosticTarget::Node { node_id: id })
            .with_field_path(format!("graph.nodes[{id}].options"))
            .with_evidence_trace(),
        );
    }
    for (idx, option) in options.iter().enumerate() {
        if is_placeholder_option(option, idx) {
            issues.push(
                LintIssue::warning(
                    Some(id),
                    ValidationPhase::Graph,
                    LintCode::PlaceholderChoiceOption,
                    format!("Choice option {idx} still uses placeholder text"),
                )
                .with_target(DiagnosticTarget::ChoiceOption {
                    node_id: id,
                    option_index: idx,
                })
                .with_field_path(format!("graph.nodes[{id}].options[{idx}].text"))
                .with_semantic_value(SemanticValue::new(
                    SemanticValueKind::Text,
                    option.clone(),
                    format!("graph.nodes[{id}].options[{idx}].text"),
                ))
                .with_evidence_trace(),
            );
        }
    }
    let outgoing = graph
        .connections()
        .filter(|conn| conn.from == id)
        .collect::<Vec<_>>();
    for idx in 0..options.len() {
        if !outgoing.iter().any(|conn| conn.from_port == idx) {
            issues.push(
                LintIssue::warning(
                    Some(id),
                    ValidationPhase::Graph,
                    LintCode::ChoiceOptionUnlinked,
                    format!("Choice option {idx} is unlinked"),
                )
                .with_edge(Some(id), None)
                .with_target(DiagnosticTarget::ChoiceOption {
                    node_id: id,
                    option_index: idx,
                })
                .with_field_path(format!("graph.nodes[{id}].options[{idx}].target"))
                .with_evidence_trace(),
            );
        }
    }
    for conn in outgoing {
        if conn.from_port >= options.len() {
            issues.push(
                LintIssue::warning(
                    Some(id),
                    ValidationPhase::Graph,
                    LintCode::ChoicePortOutOfRange,
                    "Choice connection port is out of range",
                )
                .with_edge(Some(conn.from), Some(conn.to))
                .with_target(DiagnosticTarget::Edge {
                    from: conn.from,
                    from_port: conn.from_port,
                    to: Some(conn.to),
                })
                .with_evidence_trace(),
            );
        }
    }
}

fn is_placeholder_option(option: &str, index: usize) -> bool {
    option.trim() == format!("Option {}", index + 1)
}

fn suggest_node_quick_fixes(
    ctx: &QuickFixCtx<'_>,
    kind: NodeKind,
    issue: &LintIssue,
) -> Vec<BehaviorQuickFix> {
    let mut fixes = Vec::new();
    match (kind, issue.code) {
        (NodeKind::Dialogue, LintCode::EmptySpeakerName) => fixes.push(behavior_fix(
            "dialogue_fill_speaker",
            "Rellenar speaker",
            "Fill speaker",
            BehaviorQuickFixRisk::Safe,
            false,
        )),
        (NodeKind::Choice, LintCode::ChoiceNoOptions) => {
            fixes.push(behavior_fix(
                "choice_add_default_option",
                "Agregar opcion placeholder",
                "Add placeholder option",
                BehaviorQuickFixRisk::Review,
                false,
            ));
            fixes.push(behavior_fix(
                "choice_add_default_option_to_end",
                "Agregar opcion y conectar a End",
                "Add option and connect to End",
                BehaviorQuickFixRisk::Safe,
                true,
            ));
        }
        (NodeKind::Choice, LintCode::ChoiceOptionUnlinked) => fixes.push(behavior_fix(
            "choice_link_unlinked_to_end",
            "Conectar opciones sin salida",
            "Connect unlinked options",
            BehaviorQuickFixRisk::Review,
            true,
        )),
        (NodeKind::Choice, LintCode::ChoicePortOutOfRange) => fixes.push(behavior_fix(
            "choice_expand_options_to_ports",
            "Sincronizar opciones con puertos",
            "Sync options with ports",
            BehaviorQuickFixRisk::Safe,
            false,
        )),
        (NodeKind::Jump, LintCode::EmptyJumpTarget)
        | (NodeKind::JumpIf, LintCode::EmptyJumpTarget)
            if existing_jump_target(ctx.graph()).is_some() =>
        {
            fixes.push(behavior_fix(
                "jump_set_existing_target",
                "Usar destino existente",
                "Use existing target",
                BehaviorQuickFixRisk::Review,
                false,
            ));
        }
        (NodeKind::Transition, LintCode::InvalidTransitionKind) => fixes.push(behavior_fix(
            "transition_set_fade",
            "Usar fade",
            "Use fade",
            BehaviorQuickFixRisk::Safe,
            false,
        )),
        (NodeKind::Transition, LintCode::InvalidTransitionDuration) => fixes.push(behavior_fix(
            "transition_set_default_duration",
            "Usar duracion por defecto",
            "Use default duration",
            BehaviorQuickFixRisk::Safe,
            false,
        )),
        (NodeKind::Scene, LintCode::SceneBackgroundEmpty) => fixes.push(behavior_fix(
            "scene_clear_empty_background",
            "Limpiar background vacio",
            "Clear empty background",
            BehaviorQuickFixRisk::Safe,
            false,
        )),
        (NodeKind::Scene, LintCode::AudioAssetEmpty) if scene_music_is_empty(ctx, issue) => {
            fixes.push(behavior_fix(
                "scene_clear_empty_music",
                "Limpiar musica vacia",
                "Clear empty music",
                BehaviorQuickFixRisk::Safe,
                false,
            ));
        }
        (NodeKind::AudioAction, LintCode::AudioAssetEmpty) if audio_asset_is_empty(ctx, issue) => {
            fixes.push(behavior_fix(
                "audio_clear_empty_asset",
                "Limpiar asset de audio vacio",
                "Clear empty audio asset",
                BehaviorQuickFixRisk::Safe,
                false,
            ));
        }
        (NodeKind::AudioAction, LintCode::AudioAssetMissing)
            if audio_play_is_missing_asset(ctx, issue) =>
        {
            fixes.push(behavior_fix(
                "audio_missing_asset_to_stop",
                "Normalizar play sin asset a stop",
                "Normalize play without asset to stop",
                BehaviorQuickFixRisk::Review,
                false,
            ));
        }
        (NodeKind::Scene, LintCode::AssetReferenceMissing)
        | (NodeKind::ScenePatch, LintCode::AssetReferenceMissing)
        | (NodeKind::AudioAction, LintCode::AssetReferenceMissing)
            if has_clearable_asset_field(ctx.graph(), issue) =>
        {
            fixes.push(behavior_fix(
                "clear_missing_asset_reference",
                "Limpiar asset inexistente",
                "Clear missing asset",
                BehaviorQuickFixRisk::Review,
                false,
            ));
        }
        (NodeKind::Scene, LintCode::UnsafeAssetPath)
        | (NodeKind::ScenePatch, LintCode::UnsafeAssetPath)
        | (NodeKind::AudioAction, LintCode::UnsafeAssetPath)
            if has_clearable_asset_field(ctx.graph(), issue) =>
        {
            fixes.push(behavior_fix(
                "clear_unsafe_asset_reference",
                "Limpiar asset inseguro",
                "Clear unsafe asset",
                BehaviorQuickFixRisk::Review,
                false,
            ));
        }
        (NodeKind::AudioAction, LintCode::InvalidAudioChannel)
            if audio_channel_can_normalize(ctx, issue) =>
        {
            fixes.push(behavior_fix(
                "audio_normalize_channel",
                "Normalizar alias de canal",
                "Normalize channel alias",
                BehaviorQuickFixRisk::Safe,
                false,
            ));
        }
        (NodeKind::AudioAction, LintCode::InvalidAudioAction)
            if audio_action_can_normalize(ctx, issue) =>
        {
            fixes.push(behavior_fix(
                "audio_normalize_action",
                "Normalizar alias de accion",
                "Normalize action alias",
                BehaviorQuickFixRisk::Safe,
                false,
            ));
        }
        (NodeKind::AudioAction, LintCode::InvalidAudioVolume) => fixes.push(behavior_fix(
            "audio_clamp_volume",
            "Ajustar volumen",
            "Clamp volume",
            BehaviorQuickFixRisk::Safe,
            false,
        )),
        (NodeKind::AudioAction, LintCode::InvalidAudioFade) => fixes.push(behavior_fix(
            "audio_set_default_fade",
            "Usar fade por defecto",
            "Use default fade",
            BehaviorQuickFixRisk::Safe,
            false,
        )),
        (NodeKind::CharacterPlacement, LintCode::EmptyCharacterName) => fixes.push(behavior_fix(
            "character_prune_or_fill_invalid_names",
            "Corregir nombres vacios",
            "Fix empty names",
            BehaviorQuickFixRisk::Review,
            false,
        )),
        (NodeKind::CharacterPlacement, LintCode::InvalidCharacterScale) => {
            fixes.push(behavior_fix(
                "character_set_default_scale",
                "Usar escala por defecto",
                "Use default scale",
                BehaviorQuickFixRisk::Safe,
                false,
            ))
        }
        _ => {}
    }
    fixes
}

pub(crate) fn normalized_audio_channel(channel: &str) -> Option<&'static str> {
    match channel.trim().to_ascii_lowercase().as_str() {
        "bgm" | "music" => Some("bgm"),
        "sfx" | "fx" | "sound" => Some("sfx"),
        "voice" | "vo" => Some("voice"),
        _ => None,
    }
}

pub(crate) fn normalized_audio_action(action: &str) -> Option<&'static str> {
    match action.trim().to_ascii_lowercase().as_str() {
        "play" | "start" => Some("play"),
        "stop" => Some("stop"),
        "fade" | "fadeout" | "fade_out" => Some("fade_out"),
        _ => None,
    }
}

fn scene_music_is_empty(ctx: &QuickFixCtx<'_>, issue: &LintIssue) -> bool {
    matches!(
        issue.node_id.and_then(|node_id| ctx.graph().get_node(node_id)),
        Some(StoryNode::Scene { music, .. })
            if music.as_deref().is_some_and(|val| val.trim().is_empty())
    )
}

fn audio_asset_is_empty(ctx: &QuickFixCtx<'_>, issue: &LintIssue) -> bool {
    matches!(
        issue.node_id.and_then(|node_id| ctx.graph().get_node(node_id)),
        Some(StoryNode::AudioAction { asset, .. })
            if asset.as_deref().is_some_and(|val| val.trim().is_empty())
    )
}

fn audio_play_is_missing_asset(ctx: &QuickFixCtx<'_>, issue: &LintIssue) -> bool {
    matches!(
        issue.node_id.and_then(|node_id| ctx.graph().get_node(node_id)),
        Some(StoryNode::AudioAction { action, asset, .. })
            if action.trim().eq_ignore_ascii_case("play")
                && asset.as_deref().is_none_or(|val| val.trim().is_empty())
    )
}

fn audio_channel_can_normalize(ctx: &QuickFixCtx<'_>, issue: &LintIssue) -> bool {
    matches!(
        issue.node_id.and_then(|node_id| ctx.graph().get_node(node_id)),
        Some(StoryNode::AudioAction { channel, .. })
            if normalized_audio_channel(channel).is_some_and(|normalized| normalized != channel)
    )
}

fn audio_action_can_normalize(ctx: &QuickFixCtx<'_>, issue: &LintIssue) -> bool {
    matches!(
        issue.node_id.and_then(|node_id| ctx.graph().get_node(node_id)),
        Some(StoryNode::AudioAction { action, .. })
            if normalized_audio_action(action).is_some_and(|normalized| normalized != action)
    )
}

fn has_clearable_asset_field(graph: &NodeGraph, issue: &LintIssue) -> bool {
    let Some(node_id) = issue.node_id else {
        return false;
    };
    let Some(node) = graph.get_node(node_id) else {
        return false;
    };
    let target = issue.asset_path.as_deref();
    let mut matches = 0;
    match node {
        StoryNode::Scene {
            background, music, ..
        } => {
            count_asset_match(&mut matches, background, target);
            count_asset_match(&mut matches, music, target);
        }
        StoryNode::ScenePatch(patch) => {
            count_asset_match(&mut matches, &patch.background, target);
            count_asset_match(&mut matches, &patch.music, target);
        }
        StoryNode::AudioAction { asset, .. } => {
            count_asset_match(&mut matches, asset, target);
        }
        _ => {}
    }
    matches == 1
}

fn count_asset_match(matches: &mut usize, val: &Option<String>, target: Option<&str>) {
    let Some(val) = val.as_deref() else {
        return;
    };
    let matched = match target {
        Some(target) => val == target,
        None => is_unsafe_asset_ref(val),
    };
    if matched {
        *matches += 1;
    }
}

const fn behavior_fix(
    fix_id: &'static str,
    title_es: &'static str,
    title_en: &'static str,
    risk: BehaviorQuickFixRisk,
    structural: bool,
) -> BehaviorQuickFix {
    BehaviorQuickFix::new(fix_id, title_es, title_en, risk, structural)
}

fn existing_jump_target(graph: &NodeGraph) -> Option<String> {
    if graph
        .nodes()
        .any(|(_, node, _)| matches!(node, StoryNode::Start))
    {
        return Some("start".to_string());
    }
    graph
        .nodes()
        .find(|(_, node, _)| !node.is_marker())
        .map(|(id, _, _)| format!("node_{id}"))
}

const EMPTY_FIELDS: &[&str] = &[];
const DIALOGUE_RAW_FIELDS: &[&str] = &["speaker", "text"];
const CHOICE_RAW_FIELDS: &[&str] = &["prompt", "options[].text", "options[].target"];
const SCENE_RAW_FIELDS: &[&str] = &[
    "background",
    "music",
    "characters[].name",
    "characters[].expression",
    "characters[].position",
    "characters[].x",
    "characters[].y",
    "characters[].scale",
];
const JUMP_RAW_FIELDS: &[&str] = &["target"];
const STATE_RAW_FIELDS: &[&str] = &["key", "value"];
const JUMP_IF_RAW_FIELDS: &[&str] = &["cond", "target"];
const PATCH_RAW_FIELDS: &[&str] = &[
    "background",
    "music",
    "add[].name",
    "add[].expression",
    "add[].position",
    "update[].name",
    "update[].expression",
    "update[].position",
    "remove[]",
];
const EXT_CALL_RAW_FIELDS: &[&str] = &["command", "args[]"];
const AUDIO_RAW_FIELDS: &[&str] = &[
    "channel",
    "action",
    "asset",
    "volume",
    "fade_duration_ms",
    "loop_playback",
];
const TRANSITION_RAW_FIELDS: &[&str] = &["kind", "duration_ms", "color"];
const CHARACTER_POSITION_RAW_FIELDS: &[&str] = &["name", "x", "y", "scale"];
const SCENE_ASSET_FIELDS: &[&str] = &["background", "music", "characters[].expression"];
const PATCH_ASSET_FIELDS: &[&str] = &[
    "background",
    "music",
    "add[].expression",
    "update[].expression",
];
const AUDIO_ASSET_FIELDS: &[&str] = &["asset"];
const COMPILED_TARGET_FIELDS: &[&str] = &["target_ip"];
const COMPILED_STATE_FIELDS: &[&str] = &["flag_id", "var_id", "value"];
const COMPILED_AUDIO_FIELDS: &[&str] = &[
    "channel",
    "action",
    "asset",
    "volume",
    "fade_duration_ms",
    "loop_playback",
];

const START_FIELDS: &[&str] = &[];
const END_FIELDS: &[&str] = &[];
const SCENE_NODE_FIELDS: &[&str] = &[
    "profile",
    "background",
    "music",
    "characters[].name",
    "characters[].expression",
    "characters[].position",
    "characters[].x",
    "characters[].y",
    "characters[].scale",
];
const SUBGRAPH_FIELDS: &[&str] = &["fragment_id", "entry_port", "exit_port"];
const GENERIC_FIELDS: &[&str] = &["event"];

const EVENT_SPECS: [EventSpec; 12] = [
    EventKind::Dialogue.spec(),
    EventKind::Choice.spec(),
    EventKind::Scene.spec(),
    EventKind::Jump.spec(),
    EventKind::SetFlag.spec(),
    EventKind::SetVar.spec(),
    EventKind::JumpIf.spec(),
    EventKind::Patch.spec(),
    EventKind::ExtCall.spec(),
    EventKind::AudioAction.spec(),
    EventKind::Transition.spec(),
    EventKind::SetCharacterPosition.spec(),
];

const NODE_SPECS: [NodeSpec; 16] = [
    NodeKind::Start.spec(),
    NodeKind::End.spec(),
    NodeKind::Dialogue.spec(),
    NodeKind::Choice.spec(),
    NodeKind::Scene.spec(),
    NodeKind::Jump.spec(),
    NodeKind::SetVariable.spec(),
    NodeKind::SetFlag.spec(),
    NodeKind::ScenePatch.spec(),
    NodeKind::JumpIf.spec(),
    NodeKind::AudioAction.spec(),
    NodeKind::Transition.spec(),
    NodeKind::CharacterPlacement.spec(),
    NodeKind::ExtCall.spec(),
    NodeKind::SubgraphCall.spec(),
    NodeKind::GenericEvent.spec(),
];

impl EventKind {
    pub const ALL: &'static [Self] = &[
        Self::Dialogue,
        Self::Choice,
        Self::Scene,
        Self::Jump,
        Self::SetFlag,
        Self::SetVar,
        Self::JumpIf,
        Self::Patch,
        Self::ExtCall,
        Self::AudioAction,
        Self::Transition,
        Self::SetCharacterPosition,
    ];

    pub const STABLE_NAMES: &'static [&'static str] = &[
        "dialogue",
        "choice",
        "scene",
        "jump",
        "set_flag",
        "set_var",
        "jump_if",
        "patch",
        "ext_call",
        "audio_action",
        "transition",
        "set_character_position",
    ];

    pub const fn stable_name(self) -> &'static str {
        match self {
            Self::Dialogue => "dialogue",
            Self::Choice => "choice",
            Self::Scene => "scene",
            Self::Jump => "jump",
            Self::SetFlag => "set_flag",
            Self::SetVar => "set_var",
            Self::JumpIf => "jump_if",
            Self::Patch => "patch",
            Self::ExtCall => "ext_call",
            Self::AudioAction => "audio_action",
            Self::Transition => "transition",
            Self::SetCharacterPosition => "set_character_position",
        }
    }

    pub const fn contract_name(self) -> &'static str {
        match self {
            Self::Dialogue => "Dialogue",
            Self::Choice => "Choice",
            Self::Scene => "Scene",
            Self::Jump => "Jump",
            Self::SetFlag => "SetFlag",
            Self::SetVar => "SetVariable",
            Self::JumpIf => "JumpIf",
            Self::Patch => "ScenePatch",
            Self::ExtCall => "ExtCall",
            Self::AudioAction => "AudioAction",
            Self::Transition => "Transition",
            Self::SetCharacterPosition => "SetCharacterPosition",
        }
    }

    pub const fn spec(self) -> EventSpec {
        EventSpec {
            kind: self,
            stable_name: self.stable_name(),
            contract_name: self.contract_name(),
            raw_schema: self.raw_schema(),
            compiled_schema: self.compiled_schema(),
            flow: self.flow(),
            trace_kind: self.stable_name(),
            raw_field_paths: self.raw_field_paths(),
            compiled_field_paths: self.compiled_field_paths(),
            asset_ref_field_paths: self.asset_ref_field_paths(),
            capabilities: self.capabilities(),
        }
    }

    pub const fn capabilities(self) -> EventCapabilities {
        match self {
            Self::Dialogue => runtime_real_cap(false, true, false, false, true),
            Self::Choice => runtime_real_cap(false, true, true, false, true),
            Self::Scene => runtime_real_cap(true, true, false, true, true),
            Self::Jump => runtime_real_cap(false, false, false, false, true),
            Self::SetFlag => runtime_real_cap(false, false, false, false, true),
            Self::SetVar => runtime_real_cap(false, false, false, false, true),
            Self::JumpIf => runtime_real_cap(false, false, false, false, true),
            Self::Patch => runtime_real_cap(true, true, false, true, true),
            Self::ExtCall => host_required_cap(),
            Self::AudioAction => runtime_real_cap(true, false, false, true, true),
            Self::Transition => runtime_real_cap(true, true, false, false, false),
            Self::SetCharacterPosition => runtime_real_cap(true, true, false, false, false),
        }
    }

    const fn raw_schema(self) -> &'static str {
        match self {
            Self::Dialogue => "EventRaw::Dialogue(DialogueRaw)",
            Self::Choice => "EventRaw::Choice(ChoiceRaw)",
            Self::Scene => "EventRaw::Scene(SceneUpdateRaw)",
            Self::Jump => "EventRaw::Jump { target }",
            Self::SetFlag => "EventRaw::SetFlag { key, value }",
            Self::SetVar => "EventRaw::SetVar { key, value }",
            Self::JumpIf => "EventRaw::JumpIf { cond, target }",
            Self::Patch => "EventRaw::Patch(ScenePatchRaw)",
            Self::ExtCall => "EventRaw::ExtCall { command, args }",
            Self::AudioAction => "EventRaw::AudioAction(AudioActionRaw)",
            Self::Transition => "EventRaw::Transition(SceneTransitionRaw)",
            Self::SetCharacterPosition => "EventRaw::SetCharacterPosition(SetCharacterPositionRaw)",
        }
    }

    const fn compiled_schema(self) -> &'static str {
        match self {
            Self::Dialogue => "EventCompiled::Dialogue(DialogueCompiled)",
            Self::Choice => "EventCompiled::Choice(ChoiceCompiled)",
            Self::Scene => "EventCompiled::Scene(SceneUpdateCompiled)",
            Self::Jump => "EventCompiled::Jump { target_ip }",
            Self::SetFlag => "EventCompiled::SetFlag { flag_id, value }",
            Self::SetVar => "EventCompiled::SetVar { var_id, value }",
            Self::JumpIf => "EventCompiled::JumpIf { cond, target_ip }",
            Self::Patch => "EventCompiled::Patch(ScenePatchCompiled)",
            Self::ExtCall => "EventCompiled::ExtCall { command, args }",
            Self::AudioAction => "EventCompiled::AudioAction(AudioActionCompiled)",
            Self::Transition => "EventCompiled::Transition(SceneTransitionCompiled)",
            Self::SetCharacterPosition => {
                "EventCompiled::SetCharacterPosition(SetCharacterPositionCompiled)"
            }
        }
    }

    const fn flow(self) -> EventFlow {
        match self {
            Self::Jump => EventFlow::SingleTarget,
            Self::JumpIf => EventFlow::ConditionalTarget,
            Self::Choice => EventFlow::ChoiceTargets,
            _ => EventFlow::Linear,
        }
    }

    const fn raw_field_paths(self) -> &'static [&'static str] {
        match self {
            Self::Dialogue => DIALOGUE_RAW_FIELDS,
            Self::Choice => CHOICE_RAW_FIELDS,
            Self::Scene => SCENE_RAW_FIELDS,
            Self::Jump => JUMP_RAW_FIELDS,
            Self::SetFlag | Self::SetVar => STATE_RAW_FIELDS,
            Self::JumpIf => JUMP_IF_RAW_FIELDS,
            Self::Patch => PATCH_RAW_FIELDS,
            Self::ExtCall => EXT_CALL_RAW_FIELDS,
            Self::AudioAction => AUDIO_RAW_FIELDS,
            Self::Transition => TRANSITION_RAW_FIELDS,
            Self::SetCharacterPosition => CHARACTER_POSITION_RAW_FIELDS,
        }
    }

    const fn compiled_field_paths(self) -> &'static [&'static str] {
        match self {
            Self::Dialogue => DIALOGUE_RAW_FIELDS,
            Self::Choice => CHOICE_RAW_FIELDS,
            Self::Scene => SCENE_RAW_FIELDS,
            Self::Jump => COMPILED_TARGET_FIELDS,
            Self::SetFlag | Self::SetVar => COMPILED_STATE_FIELDS,
            Self::JumpIf => JUMP_IF_RAW_FIELDS,
            Self::Patch => PATCH_RAW_FIELDS,
            Self::ExtCall => EXT_CALL_RAW_FIELDS,
            Self::AudioAction => COMPILED_AUDIO_FIELDS,
            Self::Transition => TRANSITION_RAW_FIELDS,
            Self::SetCharacterPosition => CHARACTER_POSITION_RAW_FIELDS,
        }
    }

    const fn asset_ref_field_paths(self) -> &'static [&'static str] {
        match self {
            Self::Scene => SCENE_ASSET_FIELDS,
            Self::Patch => PATCH_ASSET_FIELDS,
            Self::AudioAction => AUDIO_ASSET_FIELDS,
            _ => EMPTY_FIELDS,
        }
    }
}

impl NodeKind {
    pub const ALL: &'static [Self] = &[
        Self::Start,
        Self::End,
        Self::Dialogue,
        Self::Choice,
        Self::Scene,
        Self::Jump,
        Self::SetVariable,
        Self::SetFlag,
        Self::ScenePatch,
        Self::JumpIf,
        Self::AudioAction,
        Self::Transition,
        Self::CharacterPlacement,
        Self::ExtCall,
        Self::SubgraphCall,
        Self::GenericEvent,
    ];

    pub const fn stable_name(self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::End => "end",
            Self::Dialogue => "dialogue",
            Self::Choice => "choice",
            Self::Scene => "scene",
            Self::Jump => "jump",
            Self::SetVariable => "set_variable",
            Self::SetFlag => "set_flag",
            Self::ScenePatch => "scene_patch",
            Self::JumpIf => "jump_if",
            Self::AudioAction => "audio_action",
            Self::Transition => "transition",
            Self::CharacterPlacement => "character_placement",
            Self::ExtCall => "ext_call",
            Self::SubgraphCall => "subgraph_call",
            Self::GenericEvent => "generic_event",
        }
    }

    pub const fn spec(self) -> NodeSpec {
        NodeSpec {
            kind: self,
            stable_name: self.stable_name(),
            display_name: self.display_name(),
            contract_name: self.contract_name(),
            event_kind: self.event_kind(),
            ports: self.ports(),
            editor_schema: InspectorSchema {
                field_paths: self.field_paths(),
            },
            asset_ref_field_paths: self.asset_ref_field_paths(),
            capabilities: self.capabilities(),
        }
    }

    const fn display_name(self) -> &'static str {
        match self {
            Self::Start => "Start",
            Self::End => "End",
            Self::Dialogue => "Dialogue",
            Self::Choice => "Choice",
            Self::Scene => "Scene",
            Self::Jump => "Jump",
            Self::SetVariable => "Set Var",
            Self::SetFlag => "Set Flag",
            Self::ScenePatch => "Scene Patch",
            Self::JumpIf => "Branch (If)",
            Self::AudioAction => "Audio",
            Self::Transition => "Transition",
            Self::CharacterPlacement => "Placement",
            Self::ExtCall => "ExtCall",
            Self::SubgraphCall => "Subgraph Call",
            Self::GenericEvent => "Generic Event",
        }
    }

    const fn contract_name(self) -> &'static str {
        match self {
            Self::Start => "Start",
            Self::End => "End",
            Self::SetVariable => "SetVariable",
            Self::ScenePatch => "ScenePatch",
            Self::CharacterPlacement => "SetCharacterPosition",
            Self::SubgraphCall => "SubgraphCall",
            Self::GenericEvent => "Generic/EventRaw",
            _ => match self.event_kind() {
                Some(kind) => kind.contract_name(),
                None => self.display_name(),
            },
        }
    }

    const fn event_kind(self) -> Option<EventKind> {
        match self {
            Self::Dialogue => Some(EventKind::Dialogue),
            Self::Choice => Some(EventKind::Choice),
            Self::Scene => Some(EventKind::Scene),
            Self::Jump => Some(EventKind::Jump),
            Self::SetVariable => Some(EventKind::SetVar),
            Self::SetFlag => Some(EventKind::SetFlag),
            Self::ScenePatch => Some(EventKind::Patch),
            Self::JumpIf => Some(EventKind::JumpIf),
            Self::AudioAction => Some(EventKind::AudioAction),
            Self::Transition => Some(EventKind::Transition),
            Self::CharacterPlacement => Some(EventKind::SetCharacterPosition),
            Self::ExtCall => Some(EventKind::ExtCall),
            Self::Start | Self::End | Self::SubgraphCall | Self::GenericEvent => None,
        }
    }

    const fn ports(self) -> PortSpec {
        match self {
            Self::Start => PortSpec {
                accepts_incoming: false,
                output: PortOutput::Linear,
            },
            Self::End => PortSpec {
                accepts_incoming: true,
                output: PortOutput::None,
            },
            Self::Choice => PortSpec {
                accepts_incoming: true,
                output: PortOutput::ChoiceOptions,
            },
            Self::Jump => PortSpec {
                accepts_incoming: true,
                output: PortOutput::SingleTarget,
            },
            Self::JumpIf => PortSpec {
                accepts_incoming: true,
                output: PortOutput::ConditionalTrueFalse,
            },
            Self::SubgraphCall => PortSpec {
                accepts_incoming: true,
                output: PortOutput::SubgraphExit,
            },
            _ => PortSpec {
                accepts_incoming: true,
                output: PortOutput::Linear,
            },
        }
    }

    const fn field_paths(self) -> &'static [&'static str] {
        match self {
            Self::Start => START_FIELDS,
            Self::End => END_FIELDS,
            Self::Dialogue => DIALOGUE_RAW_FIELDS,
            Self::Choice => CHOICE_RAW_FIELDS,
            Self::Scene => SCENE_NODE_FIELDS,
            Self::Jump => JUMP_RAW_FIELDS,
            Self::SetVariable | Self::SetFlag => STATE_RAW_FIELDS,
            Self::ScenePatch => PATCH_RAW_FIELDS,
            Self::JumpIf => JUMP_IF_RAW_FIELDS,
            Self::AudioAction => AUDIO_RAW_FIELDS,
            Self::Transition => TRANSITION_RAW_FIELDS,
            Self::CharacterPlacement => CHARACTER_POSITION_RAW_FIELDS,
            Self::ExtCall => EXT_CALL_RAW_FIELDS,
            Self::SubgraphCall => SUBGRAPH_FIELDS,
            Self::GenericEvent => GENERIC_FIELDS,
        }
    }

    const fn asset_ref_field_paths(self) -> &'static [&'static str] {
        match self {
            Self::Scene => SCENE_ASSET_FIELDS,
            Self::ScenePatch => PATCH_ASSET_FIELDS,
            Self::AudioAction => AUDIO_ASSET_FIELDS,
            _ => match self.event_kind() {
                Some(kind) => kind.asset_ref_field_paths(),
                None => EMPTY_FIELDS,
            },
        }
    }

    const fn capabilities(self) -> EventCapabilities {
        match self {
            Self::Start | Self::End => preview_only_cap(false),
            Self::SubgraphCall => subgraph_cap(),
            Self::GenericEvent => fallback_cap(),
            _ => match self.event_kind() {
                Some(kind) => kind.capabilities(),
                None => fallback_cap(),
            },
        }
    }
}

pub fn event_specs() -> &'static [EventSpec] {
    &EVENT_SPECS
}

pub fn event_spec(kind: EventKind) -> &'static EventSpec {
    &EVENT_SPECS[event_kind_index(kind)]
}

pub fn event_behavior(kind: EventKind) -> StaticEventBehavior {
    StaticEventBehavior::new(kind)
}

pub fn event_behavior_for_raw(event: &EventRaw) -> StaticEventBehavior {
    event_behavior(event_kind_for_raw(event))
}

pub fn event_behavior_for_compiled(event: &EventCompiled) -> StaticEventBehavior {
    event_behavior(event_kind_for_compiled(event))
}

pub fn event_spec_for_raw(event: &EventRaw) -> &'static EventSpec {
    event_spec(event_kind_for_raw(event))
}

pub fn event_spec_for_compiled(event: &EventCompiled) -> &'static EventSpec {
    event_spec(event_kind_for_compiled(event))
}

pub fn event_asset_refs_for_raw(event: &EventRaw) -> Vec<String> {
    let mut refs = AssetRefCollector::default();
    match event {
        EventRaw::Scene(scene) => {
            refs.push_optional(&scene.background);
            refs.push_optional(&scene.music);
            collect_character_asset_refs(&scene.characters, &mut refs);
        }
        EventRaw::Patch(patch) => collect_scene_patch_asset_refs(patch, &mut refs),
        EventRaw::AudioAction(action) => refs.push_optional(&action.asset),
        _ => {}
    }
    refs.into_vec()
}

pub fn event_kind_for_raw(event: &EventRaw) -> EventKind {
    match event {
        EventRaw::Dialogue(_) => EventKind::Dialogue,
        EventRaw::Choice(_) => EventKind::Choice,
        EventRaw::Scene(_) => EventKind::Scene,
        EventRaw::Jump { .. } => EventKind::Jump,
        EventRaw::SetFlag { .. } => EventKind::SetFlag,
        EventRaw::SetVar { .. } => EventKind::SetVar,
        EventRaw::JumpIf { .. } => EventKind::JumpIf,
        EventRaw::Patch(_) => EventKind::Patch,
        EventRaw::ExtCall { .. } => EventKind::ExtCall,
        EventRaw::AudioAction(_) => EventKind::AudioAction,
        EventRaw::Transition(_) => EventKind::Transition,
        EventRaw::SetCharacterPosition(_) => EventKind::SetCharacterPosition,
    }
}

pub fn event_kind_for_compiled(event: &EventCompiled) -> EventKind {
    match event {
        EventCompiled::Dialogue(_) => EventKind::Dialogue,
        EventCompiled::Choice(_) => EventKind::Choice,
        EventCompiled::Scene(_) => EventKind::Scene,
        EventCompiled::Jump { .. } => EventKind::Jump,
        EventCompiled::SetFlag { .. } => EventKind::SetFlag,
        EventCompiled::SetVar { .. } => EventKind::SetVar,
        EventCompiled::JumpIf { .. } => EventKind::JumpIf,
        EventCompiled::Patch(_) => EventKind::Patch,
        EventCompiled::ExtCall { .. } => EventKind::ExtCall,
        EventCompiled::AudioAction(_) => EventKind::AudioAction,
        EventCompiled::Transition(_) => EventKind::Transition,
        EventCompiled::SetCharacterPosition(_) => EventKind::SetCharacterPosition,
    }
}

fn compile_event_raw(ctx: &mut CompileCtx<'_>, event: &EventRaw) -> VnResult<EventCompiled> {
    Ok(match event {
        EventRaw::Dialogue(dialogue) => EventCompiled::Dialogue(DialogueCompiled {
            speaker: ctx.intern(&dialogue.speaker),
            text: ctx.intern(&dialogue.text),
        }),
        EventRaw::Choice(choice) => {
            let options = choice
                .options
                .iter()
                .map(|option| {
                    let target_ip = ctx.resolve_target("choice", &option.target)?;
                    Ok(ChoiceOptionCompiled {
                        text: ctx.intern(&option.text),
                        target_ip,
                    })
                })
                .collect::<VnResult<Vec<_>>>()?;
            EventCompiled::Choice(ChoiceCompiled {
                prompt: ctx.intern(&choice.prompt),
                options,
            })
        }
        EventRaw::Scene(scene) => EventCompiled::Scene(SceneUpdateCompiled {
            background: scene.background.as_deref().map(|value| ctx.intern(value)),
            music: scene.music.as_deref().map(|value| ctx.intern(value)),
            characters: scene
                .characters
                .iter()
                .map(|character| CharacterPlacementCompiled {
                    name: ctx.intern(&character.name),
                    expression: character
                        .expression
                        .as_deref()
                        .map(|value| ctx.intern(value)),
                    position: character.position.as_deref().map(|value| ctx.intern(value)),
                    x: character.x,
                    y: character.y,
                    scale: character.scale,
                })
                .collect(),
        }),
        EventRaw::Jump { target } => EventCompiled::Jump {
            target_ip: ctx.resolve_target("jump", target)?,
        },
        EventRaw::SetFlag { key, value } => EventCompiled::SetFlag {
            flag_id: ctx.flag_id(key)?,
            value: *value,
        },
        EventRaw::SetVar { key, value } => EventCompiled::SetVar {
            var_id: ctx.var_id(key)?,
            value: *value,
        },
        EventRaw::JumpIf { cond, target } => EventCompiled::JumpIf {
            cond: ctx.compile_cond(cond)?,
            target_ip: ctx.resolve_target("jump_if", target)?,
        },
        EventRaw::Patch(patch) => EventCompiled::Patch(ScenePatchCompiled {
            background: patch.background.as_deref().map(|value| ctx.intern(value)),
            music: patch.music.as_deref().map(|value| ctx.intern(value)),
            add: patch
                .add
                .iter()
                .map(|character| CharacterPlacementCompiled {
                    name: ctx.intern(&character.name),
                    expression: character
                        .expression
                        .as_deref()
                        .map(|value| ctx.intern(value)),
                    position: character.position.as_deref().map(|value| ctx.intern(value)),
                    x: character.x,
                    y: character.y,
                    scale: character.scale,
                })
                .collect(),
            update: patch
                .update
                .iter()
                .map(|character| CharacterPatchCompiled {
                    name: ctx.intern(&character.name),
                    expression: character
                        .expression
                        .as_deref()
                        .map(|value| ctx.intern(value)),
                    position: character.position.as_deref().map(|value| ctx.intern(value)),
                    x: character.x,
                    y: character.y,
                    scale: character.scale,
                })
                .collect(),
            remove: patch.remove.iter().map(|name| ctx.intern(name)).collect(),
        }),
        EventRaw::ExtCall { command, args } => EventCompiled::ExtCall {
            command: command.clone(),
            args: args.clone(),
        },
        EventRaw::AudioAction(action) => {
            let channel = compile_audio_channel(&action.channel)?;
            let action_kind = compile_audio_action(&action.action)?;
            if action_kind == 0
                && action
                    .asset
                    .as_deref()
                    .is_none_or(|asset| asset.trim().is_empty())
            {
                return Err(VnError::InvalidScript(
                    "audio play action requires a non-empty asset".to_string(),
                ));
            }
            EventCompiled::AudioAction(AudioActionCompiled {
                channel,
                action: action_kind,
                asset: action.asset.as_deref().map(|asset| ctx.intern(asset)),
                volume: action.volume,
                fade_duration_ms: action.fade_duration_ms,
                loop_playback: action.loop_playback,
            })
        }
        EventRaw::Transition(transition) => EventCompiled::Transition(SceneTransitionCompiled {
            kind: compile_transition_kind(&transition.kind)?,
            duration_ms: transition.duration_ms,
            color: transition.color.as_deref().map(|color| ctx.intern(color)),
        }),
        EventRaw::SetCharacterPosition(pos) => {
            EventCompiled::SetCharacterPosition(SetCharacterPositionCompiled {
                name: ctx.intern(&pos.name),
                x: pos.x,
                y: pos.y,
                scale: pos.scale,
            })
        }
    })
}

fn execute_event_compiled(ctx: &mut ExecutionCtx<'_>, event: &EventCompiled) -> VnResult<()> {
    match event {
        EventCompiled::Jump { target_ip } => ctx.jump_to_ip(*target_ip),
        EventCompiled::SetFlag { flag_id, value } => {
            ctx.state.set_flag(*flag_id, *value);
            ctx.advance_position()
        }
        EventCompiled::Scene(scene) => {
            let before_music = ctx.state.visual.music.clone();
            ctx.state.visual.apply_scene(scene);
            append_music_delta(before_music, &ctx.state.visual.music, ctx.audio_commands);
            ctx.advance_position()
        }
        EventCompiled::Choice(_) => Ok(()),
        EventCompiled::Dialogue(dialogue) => {
            let current_ip = ctx.current_ip();
            ctx.state.record_dialogue(dialogue);
            ctx.read_dialogue_ips.insert(current_ip);
            ctx.advance_position()
        }
        EventCompiled::SetVar { var_id, value } => {
            ctx.state.set_var(*var_id, *value);
            ctx.advance_position()
        }
        EventCompiled::JumpIf { cond, target_ip } => {
            if ctx.evaluate_cond(cond) {
                ctx.jump_to_ip(*target_ip)
            } else {
                ctx.advance_position()
            }
        }
        EventCompiled::Patch(patch) => {
            let before_music = ctx.state.visual.music.clone();
            ctx.state.visual.apply_patch(patch);
            append_music_delta(before_music, &ctx.state.visual.music, ctx.audio_commands);
            ctx.advance_position()
        }
        EventCompiled::ExtCall { .. } => Ok(()),
        EventCompiled::AudioAction(action) => {
            if let Some(command) = audio_command_from_action(action) {
                ctx.audio_commands.push(command);
            }
            ctx.advance_position()
        }
        EventCompiled::SetCharacterPosition(pos) => {
            ctx.state.visual.set_character_position(pos)?;
            ctx.advance_position()
        }
        EventCompiled::Transition(transition) => {
            *ctx.pending_transition = Some(transition.clone());
            ctx.advance_position()
        }
    }
}

fn preview_event_compiled(ctx: &mut PreviewCtx<'_>, event: &EventCompiled) -> Option<String> {
    match event {
        EventCompiled::Scene(scene) => ctx.visual.apply_scene(scene),
        EventCompiled::Patch(patch) => ctx.visual.apply_patch(patch),
        EventCompiled::SetCharacterPosition(position) => {
            match ctx.visual.set_character_position(position) {
                Ok(()) => {}
                Err(err) => return Some(format!("scene frame visual update failed: {err}")),
            }
        }
        _ => {}
    }
    None
}

fn append_scene_frame_for_event(ctx: &mut SceneFrameCtx<'_>, event: &EventCompiled) {
    match event {
        EventCompiled::Dialogue(dialogue) => {
            ctx.commands.push(RenderCommand::Panel {
                style: "dialogue_box".to_string(),
                rect: dialogue_panel_rect(),
            });
            if !dialogue.speaker.is_empty() {
                ctx.commands.push(RenderCommand::Text {
                    text: dialogue.speaker.to_string(),
                    style: "dialogue.speaker".to_string(),
                    rect: LayoutRect {
                        x: 96.0,
                        y: 512.0,
                        width: 1088.0,
                        height: 32.0,
                    },
                });
            }
            ctx.commands.push(RenderCommand::Text {
                text: dialogue.text.to_string(),
                style: "dialogue.text".to_string(),
                rect: LayoutRect {
                    x: 96.0,
                    y: 552.0,
                    width: 1088.0,
                    height: 96.0,
                },
            });
            ctx.commands.push(RenderCommand::Button {
                id: "continue".to_string(),
                label: "Continue".to_string(),
                style: "button.primary".to_string(),
                rect: LayoutRect {
                    x: 1040.0,
                    y: 656.0,
                    width: 144.0,
                    height: 40.0,
                },
            });
            ctx.interactions.push(InteractionSpec {
                id: "continue".to_string(),
                label: "Continue".to_string(),
                action: "advance".to_string(),
            });
        }
        EventCompiled::Choice(choice) => {
            ctx.commands.push(RenderCommand::Panel {
                style: "choice_list".to_string(),
                rect: LayoutRect {
                    x: 336.0,
                    y: 160.0,
                    width: 608.0,
                    height: (96.0 + choice.options.len() as f32 * 56.0).min(480.0),
                },
            });
            ctx.commands.push(RenderCommand::Text {
                text: choice.prompt.to_string(),
                style: "choice.prompt".to_string(),
                rect: LayoutRect {
                    x: 368.0,
                    y: 192.0,
                    width: 544.0,
                    height: 48.0,
                },
            });
            for (index, option) in choice.options.iter().enumerate() {
                let id = format!("choice:{index}");
                let y = 256.0 + index as f32 * 56.0;
                ctx.commands.push(RenderCommand::Button {
                    id: id.clone(),
                    label: option.text.to_string(),
                    style: "button.choice".to_string(),
                    rect: LayoutRect {
                        x: 384.0,
                        y,
                        width: 512.0,
                        height: 44.0,
                    },
                });
                ctx.interactions.push(InteractionSpec {
                    id,
                    label: option.text.to_string(),
                    action: format!("choose:{index}"),
                });
            }
        }
        EventCompiled::ExtCall { command, .. } => {
            ctx.commands.push(RenderCommand::Panel {
                style: "system_overlay".to_string(),
                rect: dialogue_panel_rect(),
            });
            ctx.commands.push(RenderCommand::Text {
                text: format!("External command: {command}"),
                style: "system.text".to_string(),
                rect: LayoutRect {
                    x: 96.0,
                    y: 552.0,
                    width: 1088.0,
                    height: 96.0,
                },
            });
            ctx.interactions.push(InteractionSpec {
                id: "resume".to_string(),
                label: "Resume".to_string(),
                action: "resume".to_string(),
            });
        }
        _ => {}
    }
}

pub fn node_specs() -> &'static [NodeSpec] {
    &NODE_SPECS
}

pub fn node_spec(kind: NodeKind) -> &'static NodeSpec {
    &NODE_SPECS[node_kind_index(kind)]
}

pub fn node_behavior(kind: NodeKind) -> StaticNodeBehavior {
    StaticNodeBehavior::new(kind)
}

pub fn node_behavior_for_authoring_node(node: &StoryNode) -> StaticNodeBehavior {
    node_behavior(node_kind_for_authoring_node(node))
}

pub fn node_spec_for_authoring_node(node: &StoryNode) -> &'static NodeSpec {
    node_spec(node_kind_for_authoring_node(node))
}

pub fn node_asset_refs_for_authoring_node(node: &StoryNode) -> Vec<String> {
    let mut refs = AssetRefCollector::default();
    match node {
        StoryNode::Scene {
            background,
            music,
            characters,
            ..
        } => {
            refs.push_optional(background);
            refs.push_optional(music);
            collect_character_asset_refs(characters, &mut refs);
        }
        StoryNode::ScenePatch(patch) => collect_scene_patch_asset_refs(patch, &mut refs),
        StoryNode::AudioAction { asset, .. } => refs.push_optional(asset),
        StoryNode::Generic(event) => refs.extend(event_asset_refs_for_raw(event)),
        _ => {}
    }
    refs.into_vec()
}

pub fn node_from_event_raw(event: &EventRaw) -> StoryNode {
    match event {
        EventRaw::Dialogue(dialogue) => StoryNode::Dialogue {
            speaker: dialogue.speaker.clone(),
            text: dialogue.text.clone(),
        },
        EventRaw::Choice(choice) => StoryNode::Choice {
            prompt: choice.prompt.clone(),
            options: choice
                .options
                .iter()
                .map(|option| option.text.clone())
                .collect(),
        },
        EventRaw::Scene(scene) => StoryNode::Scene {
            profile: None,
            background: scene.background.clone(),
            music: scene.music.clone(),
            characters: scene.characters.clone(),
        },
        EventRaw::Jump { target } => StoryNode::Jump {
            target: target.clone(),
        },
        EventRaw::SetFlag { key, value } => StoryNode::SetFlag {
            key: key.clone(),
            value: *value,
        },
        EventRaw::SetVar { key, value } => StoryNode::SetVariable {
            key: key.clone(),
            value: *value,
        },
        EventRaw::JumpIf { cond, target } => StoryNode::JumpIf {
            target: target.clone(),
            cond: cond.clone(),
        },
        EventRaw::Patch(patch) => StoryNode::ScenePatch(patch.clone()),
        EventRaw::AudioAction(action) => StoryNode::AudioAction {
            channel: action.channel.clone(),
            action: action.action.clone(),
            asset: action.asset.clone(),
            volume: action.volume,
            fade_duration_ms: action.fade_duration_ms,
            loop_playback: action.loop_playback,
        },
        EventRaw::Transition(transition) => StoryNode::Transition {
            kind: transition.kind.clone(),
            duration_ms: transition.duration_ms,
            color: transition.color.clone(),
        },
        EventRaw::SetCharacterPosition(pos) => StoryNode::CharacterPlacement {
            name: pos.name.clone(),
            x: pos.x,
            y: pos.y,
            scale: pos.scale,
        },
        EventRaw::ExtCall { .. } => StoryNode::Generic(event.clone()),
    }
}

pub fn node_to_event_raw_without_export_context(
    node: &StoryNode,
) -> Result<EventRaw, NodeToEventError> {
    Ok(match node {
        StoryNode::Dialogue { speaker, text } => EventRaw::Dialogue(DialogueRaw {
            speaker: speaker.clone(),
            text: text.clone(),
        }),
        StoryNode::Scene {
            background,
            music,
            characters,
            ..
        } => EventRaw::Scene(SceneUpdateRaw {
            background: background.clone(),
            music: music.clone(),
            characters: characters.clone(),
        }),
        StoryNode::SetVariable { key, value } => EventRaw::SetVar {
            key: key.clone(),
            value: *value,
        },
        StoryNode::SetFlag { key, value } => EventRaw::SetFlag {
            key: key.clone(),
            value: *value,
        },
        StoryNode::ScenePatch(patch) => EventRaw::Patch(patch.clone()),
        StoryNode::AudioAction {
            channel,
            action,
            asset,
            volume,
            fade_duration_ms,
            loop_playback,
        } => EventRaw::AudioAction(AudioActionRaw {
            channel: channel.clone(),
            action: action.clone(),
            asset: asset.clone(),
            volume: *volume,
            fade_duration_ms: *fade_duration_ms,
            loop_playback: *loop_playback,
        }),
        StoryNode::Transition {
            kind,
            duration_ms,
            color,
        } => EventRaw::Transition(SceneTransitionRaw {
            kind: kind.clone(),
            duration_ms: *duration_ms,
            color: color.clone(),
        }),
        StoryNode::CharacterPlacement { name, x, y, scale } => {
            EventRaw::SetCharacterPosition(SetCharacterPositionRaw {
                name: name.clone(),
                x: *x,
                y: *y,
                scale: *scale,
            })
        }
        StoryNode::Generic(event) => event.clone(),
        StoryNode::Choice { .. }
        | StoryNode::Jump { .. }
        | StoryNode::JumpIf { .. }
        | StoryNode::SubgraphCall { .. } => return Err(NodeToEventError::RequiresExportContext),
        StoryNode::Start | StoryNode::End => return Err(NodeToEventError::PreviewOnlyMarker),
    })
}

pub fn node_kind_for_event_raw(event: &EventRaw) -> NodeKind {
    match event {
        EventRaw::Dialogue(_) => NodeKind::Dialogue,
        EventRaw::Choice(_) => NodeKind::Choice,
        EventRaw::Scene(_) => NodeKind::Scene,
        EventRaw::Jump { .. } => NodeKind::Jump,
        EventRaw::SetFlag { .. } => NodeKind::SetFlag,
        EventRaw::SetVar { .. } => NodeKind::SetVariable,
        EventRaw::JumpIf { .. } => NodeKind::JumpIf,
        EventRaw::Patch(_) => NodeKind::ScenePatch,
        EventRaw::ExtCall { .. } => NodeKind::ExtCall,
        EventRaw::AudioAction(_) => NodeKind::AudioAction,
        EventRaw::Transition(_) => NodeKind::Transition,
        EventRaw::SetCharacterPosition(_) => NodeKind::CharacterPlacement,
    }
}

pub fn node_kind_for_authoring_node(node: &StoryNode) -> NodeKind {
    match node {
        StoryNode::Dialogue { .. } => NodeKind::Dialogue,
        StoryNode::Choice { .. } => NodeKind::Choice,
        StoryNode::Scene { .. } => NodeKind::Scene,
        StoryNode::Jump { .. } => NodeKind::Jump,
        StoryNode::SetVariable { .. } => NodeKind::SetVariable,
        StoryNode::SetFlag { .. } => NodeKind::SetFlag,
        StoryNode::ScenePatch(_) => NodeKind::ScenePatch,
        StoryNode::JumpIf { .. } => NodeKind::JumpIf,
        StoryNode::Start => NodeKind::Start,
        StoryNode::End => NodeKind::End,
        StoryNode::AudioAction { .. } => NodeKind::AudioAction,
        StoryNode::Transition { .. } => NodeKind::Transition,
        StoryNode::CharacterPlacement { .. } => NodeKind::CharacterPlacement,
        StoryNode::SubgraphCall { .. } => NodeKind::SubgraphCall,
        StoryNode::Generic(EventRaw::ExtCall { .. }) => NodeKind::ExtCall,
        StoryNode::Generic(EventRaw::SetFlag { .. }) => NodeKind::SetFlag,
        StoryNode::Generic(_) => NodeKind::GenericEvent,
    }
}

const fn event_kind_index(kind: EventKind) -> usize {
    match kind {
        EventKind::Dialogue => 0,
        EventKind::Choice => 1,
        EventKind::Scene => 2,
        EventKind::Jump => 3,
        EventKind::SetFlag => 4,
        EventKind::SetVar => 5,
        EventKind::JumpIf => 6,
        EventKind::Patch => 7,
        EventKind::ExtCall => 8,
        EventKind::AudioAction => 9,
        EventKind::Transition => 10,
        EventKind::SetCharacterPosition => 11,
    }
}

const fn node_kind_index(kind: NodeKind) -> usize {
    match kind {
        NodeKind::Start => 0,
        NodeKind::End => 1,
        NodeKind::Dialogue => 2,
        NodeKind::Choice => 3,
        NodeKind::Scene => 4,
        NodeKind::Jump => 5,
        NodeKind::SetVariable => 6,
        NodeKind::SetFlag => 7,
        NodeKind::ScenePatch => 8,
        NodeKind::JumpIf => 9,
        NodeKind::AudioAction => 10,
        NodeKind::Transition => 11,
        NodeKind::CharacterPlacement => 12,
        NodeKind::ExtCall => 13,
        NodeKind::SubgraphCall => 14,
        NodeKind::GenericEvent => 15,
    }
}

#[derive(Default)]
struct StringPool {
    cache: HashMap<String, SharedStr>,
}

impl StringPool {
    fn intern(&mut self, value: &str) -> SharedStr {
        if let Some(existing) = self.cache.get(value) {
            return existing.clone();
        }
        let shared: SharedStr = Arc::from(value);
        self.cache.insert(value.to_string(), shared.clone());
        shared
    }
}

fn get_or_insert_id(map: &mut HashMap<String, u32>, key: &str) -> VnResult<u32> {
    if let Some(id) = map.get(key) {
        return Ok(*id);
    }
    let next_id =
        u32::try_from(map.len()).map_err(|_| VnError::InvalidScript("too many ids".to_string()))?;
    map.insert(key.to_string(), next_id);
    Ok(next_id)
}

fn compile_audio_channel(channel: &str) -> VnResult<u8> {
    let normalized = channel.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "bgm" => Ok(0),
        "sfx" => Ok(1),
        "voice" => Ok(2),
        _ => Err(VnError::InvalidScript(format!(
            "invalid audio channel '{channel}' (expected bgm|sfx|voice)"
        ))),
    }
}

fn compile_audio_action(action: &str) -> VnResult<u8> {
    let normalized = action.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "play" => Ok(0),
        "stop" => Ok(1),
        "fade_out" => Ok(2),
        _ => Err(VnError::InvalidScript(format!(
            "invalid audio action '{action}' (expected play|stop|fade_out)"
        ))),
    }
}

fn compile_transition_kind(kind: &str) -> VnResult<u8> {
    let normalized = kind.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "fade" | "fade_black" => Ok(0),
        "dissolve" => Ok(1),
        "cut" => Ok(2),
        _ => Err(VnError::InvalidScript(format!(
            "invalid transition kind '{kind}' (expected fade|fade_black|dissolve|cut)"
        ))),
    }
}

const DEFAULT_AUDIO_FADE_MS: u64 = 500;

fn append_music_delta(
    before: Option<SharedStr>,
    after: &Option<SharedStr>,
    audio_commands: &mut Vec<AudioCommand>,
) {
    if before.as_deref() == after.as_deref() {
        return;
    }
    match after {
        Some(music) => audio_commands.push(AudioCommand::PlayBgm {
            resource: AssetId::from_path(music.as_ref()),
            path: music.clone(),
            r#loop: true,
            volume: None,
            fade_in: Duration::from_millis(DEFAULT_AUDIO_FADE_MS),
        }),
        None => audio_commands.push(AudioCommand::StopBgm {
            fade_out: Duration::from_millis(DEFAULT_AUDIO_FADE_MS),
        }),
    }
}

fn audio_command_from_action(action: &AudioActionCompiled) -> Option<AudioCommand> {
    match action.action {
        0 => audio_play_command(action),
        1 | 2 => audio_stop_command(action),
        _ => None,
    }
}

fn audio_play_command(action: &AudioActionCompiled) -> Option<AudioCommand> {
    let path = action.asset.as_ref()?;
    match action.channel {
        0 => Some(AudioCommand::PlayBgm {
            resource: AssetId::from_path(path.as_ref()),
            path: path.clone(),
            r#loop: action.loop_playback.unwrap_or(true),
            volume: action.volume,
            fade_in: Duration::from_millis(
                action.fade_duration_ms.unwrap_or(DEFAULT_AUDIO_FADE_MS),
            ),
        }),
        1 => Some(AudioCommand::PlaySfx {
            resource: AssetId::from_path(path.as_ref()),
            path: path.clone(),
            volume: action.volume,
        }),
        2 => Some(AudioCommand::PlayVoice {
            resource: AssetId::from_path(path.as_ref()),
            path: path.clone(),
            volume: action.volume,
        }),
        _ => None,
    }
}

fn audio_stop_command(action: &AudioActionCompiled) -> Option<AudioCommand> {
    match action.channel {
        0 => Some(AudioCommand::StopBgm {
            fade_out: Duration::from_millis(
                action.fade_duration_ms.unwrap_or(DEFAULT_AUDIO_FADE_MS),
            ),
        }),
        1 => Some(AudioCommand::StopSfx),
        2 => Some(AudioCommand::StopVoice),
        _ => None,
    }
}

fn dialogue_panel_rect() -> LayoutRect {
    LayoutRect {
        x: 64.0,
        y: 496.0,
        width: 1152.0,
        height: 200.0,
    }
}

#[derive(Default)]
struct AssetRefCollector {
    refs: BTreeSet<String>,
}

impl AssetRefCollector {
    fn push_optional(&mut self, value: &Option<String>) {
        if let Some(value) = value
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            self.refs.insert(value.to_string());
        }
    }

    fn extend(&mut self, values: Vec<String>) {
        for value in values {
            self.push(&value);
        }
    }

    fn push(&mut self, value: &str) {
        let value = value.trim();
        if !value.is_empty() {
            self.refs.insert(value.to_string());
        }
    }

    fn into_vec(self) -> Vec<String> {
        self.refs.into_iter().collect()
    }
}

fn collect_scene_patch_asset_refs(
    patch: &crate::event::ScenePatchRaw,
    refs: &mut AssetRefCollector,
) {
    refs.push_optional(&patch.background);
    refs.push_optional(&patch.music);
    collect_character_asset_refs(&patch.add, refs);
    for character in &patch.update {
        refs.push_optional(&character.expression);
    }
}

fn collect_character_asset_refs(
    characters: &[crate::event::CharacterPlacementRaw],
    refs: &mut AssetRefCollector,
) {
    for character in characters {
        refs.push_optional(&character.expression);
    }
}

const fn support_when(enabled: bool, support: BehaviorSupport) -> BehaviorSupport {
    if enabled {
        support
    } else {
        BehaviorSupport::Unsupported
    }
}

const fn runtime_real_cap(
    visual_state: bool,
    scene_frame: bool,
    interactions: bool,
    asset_refs: bool,
    quick_fixes: bool,
) -> EventCapabilities {
    EventCapabilities {
        editor_supported: true,
        preview_supported: true,
        runtime_supported: true,
        headless_supported: true,
        export_supported: true,
        python_supported: true,
        cli_supported: true,
        fidelity: FidelityClass::RuntimeReal,
        compile: BehaviorSupport::Native,
        execute: BehaviorSupport::Native,
        preview: BehaviorSupport::Native,
        visual_state: support_when(visual_state, BehaviorSupport::Native),
        scene_frame: support_when(scene_frame, BehaviorSupport::Native),
        interactions: support_when(interactions, BehaviorSupport::Native),
        asset_refs: support_when(asset_refs, BehaviorSupport::Native),
        validation: BehaviorSupport::Native,
        quick_fixes: support_when(quick_fixes, BehaviorSupport::Native),
    }
}

const fn host_required_cap() -> EventCapabilities {
    EventCapabilities {
        editor_supported: true,
        preview_supported: true,
        runtime_supported: true,
        headless_supported: false,
        export_supported: false,
        python_supported: true,
        cli_supported: false,
        fidelity: FidelityClass::HostRequired,
        compile: BehaviorSupport::Native,
        execute: BehaviorSupport::HostRequired,
        preview: BehaviorSupport::HostRequired,
        visual_state: BehaviorSupport::Unsupported,
        scene_frame: BehaviorSupport::Unsupported,
        interactions: BehaviorSupport::Unsupported,
        asset_refs: BehaviorSupport::Unsupported,
        validation: BehaviorSupport::Native,
        quick_fixes: BehaviorSupport::Fallback,
    }
}

const fn preview_only_cap(export_supported: bool) -> EventCapabilities {
    EventCapabilities {
        editor_supported: true,
        preview_supported: true,
        runtime_supported: false,
        headless_supported: false,
        export_supported,
        python_supported: false,
        cli_supported: export_supported,
        fidelity: FidelityClass::PreviewOnly,
        compile: BehaviorSupport::PreviewOnly,
        execute: BehaviorSupport::Unsupported,
        preview: BehaviorSupport::PreviewOnly,
        visual_state: BehaviorSupport::Unsupported,
        scene_frame: BehaviorSupport::PreviewOnly,
        interactions: BehaviorSupport::Unsupported,
        asset_refs: BehaviorSupport::Unsupported,
        validation: BehaviorSupport::Native,
        quick_fixes: BehaviorSupport::Unsupported,
    }
}

const fn subgraph_cap() -> EventCapabilities {
    EventCapabilities {
        editor_supported: true,
        preview_supported: true,
        runtime_supported: false,
        headless_supported: false,
        export_supported: true,
        python_supported: false,
        cli_supported: true,
        fidelity: FidelityClass::PreviewOnly,
        compile: BehaviorSupport::Fallback,
        execute: BehaviorSupport::Unsupported,
        preview: BehaviorSupport::PreviewOnly,
        visual_state: BehaviorSupport::Unsupported,
        scene_frame: BehaviorSupport::PreviewOnly,
        interactions: BehaviorSupport::Unsupported,
        asset_refs: BehaviorSupport::Unsupported,
        validation: BehaviorSupport::Native,
        quick_fixes: BehaviorSupport::Unsupported,
    }
}

const fn fallback_cap() -> EventCapabilities {
    EventCapabilities {
        editor_supported: true,
        preview_supported: true,
        runtime_supported: false,
        headless_supported: false,
        export_supported: false,
        python_supported: false,
        cli_supported: false,
        fidelity: FidelityClass::FallbackDegraded,
        compile: BehaviorSupport::Fallback,
        execute: BehaviorSupport::Unsupported,
        preview: BehaviorSupport::Fallback,
        visual_state: BehaviorSupport::Unsupported,
        scene_frame: BehaviorSupport::Fallback,
        interactions: BehaviorSupport::Unsupported,
        asset_refs: BehaviorSupport::Fallback,
        validation: BehaviorSupport::Fallback,
        quick_fixes: BehaviorSupport::Unsupported,
    }
}
