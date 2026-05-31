use std::collections::{BTreeSet, VecDeque};

use crate::audio::AudioCommand;
use crate::error::{VnError, VnResult};
use crate::event::{EventCompiled, SceneTransitionCompiled};
use crate::event_behavior::{
    event_behavior_for_compiled, EventBehavior, ExecutionCtx, PreviewCtx, SceneFrameCtx,
};
use crate::render::{RenderBackend, RenderOutput};
use crate::resource::ResourceLimiter;
use crate::route_tree::{
    build_route_tree_with_progress, resolve_visual_at_ip, ChoiceProgressSnapshot,
    ReadModelSnapshot, RouteProgressSnapshot, RouteTree, VisualResolveStrategy,
};
use crate::scene_frame::{ImageFit, LayoutRect, RenderCommand, SceneFrame};
use crate::script::{ScriptCompiled, ScriptRaw};
use crate::security::SecurityPolicy;
use crate::state::EngineState;
use crate::visual::VisualState;

use super::audio::initial_audio_commands;

const CHOICE_HISTORY_LIMIT: usize = 512;

/// Recorded decision made by the player at a Choice event.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChoiceHistoryEntry {
    pub event_ip: u32,
    pub prompt: String,
    pub option_index: usize,
    pub option_text: String,
    pub target_ip: u32,
}

/// External call request currently blocking the engine.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExternalCallRequest {
    pub event_ip: u32,
    pub command: String,
    pub args: Vec<String>,
}

/// Host-reported result for a pending external call.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExternalCallOutcome {
    pub event_ip: u32,
    pub command: String,
    pub status: ExternalCallStatus,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExternalCallStatus {
    Succeeded,
    Failed(String),
}

impl ExternalCallOutcome {
    pub fn succeeded(event_ip: u32, command: impl Into<String>) -> Self {
        Self {
            event_ip,
            command: command.into(),
            status: ExternalCallStatus::Succeeded,
        }
    }

    pub fn failed(event_ip: u32, command: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            event_ip,
            command: command.into(),
            status: ExternalCallStatus::Failed(message.into()),
        }
    }
}

/// Execution engine for compiled scripts.
#[derive(Clone, Debug)]
pub struct Engine {
    script: ScriptCompiled,
    state: EngineState,
    policy: SecurityPolicy,
    limits: ResourceLimiter,
    queued_audio: Vec<AudioCommand>,
    pending_transition: Option<SceneTransitionCompiled>,
    read_dialogue_ips: BTreeSet<u32>,
    route_visited_ips: BTreeSet<u32>,
    choice_history: VecDeque<ChoiceHistoryEntry>,
}

impl Engine {
    /// Builds an engine by validating and compiling a raw script.
    pub fn new(
        script: ScriptRaw,
        policy: SecurityPolicy,
        limits: ResourceLimiter,
    ) -> VnResult<Self> {
        policy.validate_raw(&script, limits)?;
        let script = script.compile()?;
        Self::from_compiled(script, policy, limits)
    }

    /// Builds an engine directly from a compiled script.
    pub fn from_compiled(
        script: ScriptCompiled,
        policy: SecurityPolicy,
        limits: ResourceLimiter,
    ) -> VnResult<Self> {
        policy.validate_compiled(&script, limits)?;
        Ok(Self::from_validated_compiled(script, policy, limits))
    }

    fn from_validated_compiled(
        script: ScriptCompiled,
        policy: SecurityPolicy,
        limits: ResourceLimiter,
    ) -> Self {
        let state = initialize_state(&script);
        let queued_audio = initial_audio_commands(&state);
        let mut route_visited_ips = BTreeSet::new();
        route_visited_ips.insert(script.start_ip);
        let mut engine = Self {
            script,
            state,
            policy,
            limits,
            queued_audio,
            pending_transition: None,
            read_dialogue_ips: BTreeSet::new(),
            route_visited_ips,
            choice_history: VecDeque::with_capacity(64),
        };
        engine.sync_progress_snapshots();
        engine
    }

    /// Returns a reference to the compiled script.
    pub fn script(&self) -> &ScriptCompiled {
        &self.script
    }

    /// Returns a reference to the current compiled event.
    pub fn current_event_ref(&self) -> VnResult<&EventCompiled> {
        if self.state.position as usize >= self.script.events.len() {
            return Err(VnError::EndOfScript);
        }
        self.script
            .events
            .get(self.state.position as usize)
            .ok_or(VnError::EndOfScript)
    }

    /// Returns a clone of the current compiled event.
    pub fn current_event(&self) -> VnResult<EventCompiled> {
        self.current_event_ref().cloned()
    }

    /// Advances the engine by applying the current event.
    pub fn step(&mut self) -> VnResult<(Vec<AudioCommand>, StateChange)> {
        let event = self.current_event()?;
        let mut audio_commands = self.take_audio_commands();
        self.advance_from(&event, &mut audio_commands)?;
        self.sync_progress_snapshots();
        let change = StateChange {
            event,
            visual: self.state.visual.clone(),
        };
        Ok((audio_commands, change))
    }

    /// Returns the current event and advances the engine.
    pub fn step_event(&mut self) -> VnResult<EventCompiled> {
        let (_audio, change) = self.step()?;
        Ok(change.event)
    }

    /// Applies a choice selection on the current choice event.
    pub fn choose(&mut self, option_index: usize) -> VnResult<EventCompiled> {
        let event = self.current_event()?;
        match &event {
            EventCompiled::Choice(choice) => {
                let option = choice
                    .options
                    .get(option_index)
                    .ok_or(VnError::InvalidChoice)?;
                self.record_choice_decision(
                    self.state.position,
                    choice.prompt.as_ref(),
                    option_index,
                    option.text.as_ref(),
                    option.target_ip,
                );
                self.jump_to_ip(option.target_ip)?;
            }
            _ => return Err(VnError::InvalidChoice),
        }
        Ok(event)
    }

    fn advance_from(
        &mut self,
        event: &EventCompiled,
        audio_commands: &mut Vec<AudioCommand>,
    ) -> VnResult<()> {
        let mut ctx = ExecutionCtx::new(
            &mut self.state,
            &self.script.events,
            audio_commands,
            &mut self.read_dialogue_ips,
            &mut self.route_visited_ips,
            &mut self.pending_transition,
        );
        event_behavior_for_compiled(event).execute(&mut ctx, event)
    }

    fn advance_position(&mut self) -> VnResult<()> {
        let mut audio_commands = Vec::new();
        let mut ctx = ExecutionCtx::new(
            &mut self.state,
            &self.script.events,
            &mut audio_commands,
            &mut self.read_dialogue_ips,
            &mut self.route_visited_ips,
            &mut self.pending_transition,
        );
        ctx.advance_position()
    }

    fn jump_to_ip(&mut self, target_ip: u32) -> VnResult<()> {
        let mut audio_commands = self.take_audio_commands();
        let result = self.jump_to_ip_with_audio(target_ip, &mut audio_commands);
        self.queued_audio = audio_commands;
        if result.is_ok() {
            self.sync_progress_snapshots();
        }
        result
    }

    fn jump_to_ip_with_audio(
        &mut self,
        target_ip: u32,
        audio_commands: &mut Vec<AudioCommand>,
    ) -> VnResult<()> {
        let mut ctx = ExecutionCtx::new(
            &mut self.state,
            &self.script.events,
            audio_commands,
            &mut self.read_dialogue_ips,
            &mut self.route_visited_ips,
            &mut self.pending_transition,
        );
        ctx.jump_to_ip(target_ip)
    }

    /// Returns the full engine state.
    pub fn state(&self) -> &EngineState {
        &self.state
    }

    /// Returns the security policy in use.
    pub fn policy(&self) -> &SecurityPolicy {
        &self.policy
    }

    /// Returns the resource limits used to validate this engine.
    pub fn limits(&self) -> ResourceLimiter {
        self.limits
    }

    /// Returns the current visual state.
    pub fn visual_state(&self) -> &crate::visual::VisualState {
        &self.state.visual
    }

    /// Returns a snapshot of the dialogue/read model suitable for saves.
    pub fn read_model_snapshot(&self) -> ReadModelSnapshot {
        ReadModelSnapshot {
            visited_dialogue_ips: self.read_dialogue_ips.clone(),
        }
    }

    /// Returns a route progress snapshot suitable for saves and route-tree views.
    pub fn route_progress_snapshot(&self) -> RouteProgressSnapshot {
        RouteProgressSnapshot {
            current_ip: self.state.position,
            visited_ips: self.route_visited_ips.clone(),
            selected_choices: self
                .choice_history
                .iter()
                .map(|entry| ChoiceProgressSnapshot {
                    event_ip: entry.event_ip,
                    prompt: entry.prompt.clone(),
                    option_index: entry.option_index,
                    option_text: entry.option_text.clone(),
                    target_ip: entry.target_ip,
                })
                .collect(),
            reached_endings: BTreeSet::new(),
        }
    }

    /// Builds a route tree with current progress overlaid.
    pub fn route_tree(&self) -> RouteTree {
        build_route_tree_with_progress(&self.script, self.route_progress_snapshot())
    }

    /// Resolves the visual state that previews should use at an instruction pointer.
    pub fn resolve_visual_at_ip(&self, ip: u32) -> VisualState {
        resolve_visual_at_ip(&self.script, ip, VisualResolveStrategy::LinearReplay)
    }

    /// Returns a renderer-agnostic frame for player/API/GUI adapters.
    pub fn scene_frame(&self) -> SceneFrame {
        let event = self.current_event_ref().ok();
        let mut visual = self.state.visual.clone();
        let mut diagnostics = Vec::new();
        if let Some(event) = event {
            let mut preview_ctx = PreviewCtx::new(&mut visual);
            match event_behavior_for_compiled(event).preview(&mut preview_ctx, event) {
                Ok(Some(diagnostic)) => diagnostics.push(diagnostic),
                Ok(None) => {}
                Err(err) => diagnostics.push(format!("scene frame preview failed: {err}")),
            }
        }
        let mut commands = visual_render_commands(&visual);
        let mut interactions = Vec::new();
        if let Some(event) = event {
            let mut frame_ctx = SceneFrameCtx::new(&mut commands, &mut interactions);
            if let Err(err) =
                event_behavior_for_compiled(event).append_scene_frame(&mut frame_ctx, event)
            {
                diagnostics.push(format!("scene frame overlay failed: {err}"));
            }
        }
        for diagnostic in diagnostics {
            commands.push(RenderCommand::Text {
                text: diagnostic,
                style: "system.warning".to_string(),
                rect: LayoutRect {
                    x: 32.0,
                    y: 32.0,
                    width: 1216.0,
                    height: 32.0,
                },
            });
        }
        SceneFrame {
            frame_schema: "vnengine.scene_frame.v1".to_string(),
            visual,
            commands,
            interactions,
            route: Some(self.route_tree()),
            theme_id: Some("default".to_string()),
            layout: None,
        }
    }

    pub fn pending_transition(&self) -> Option<&SceneTransitionCompiled> {
        self.pending_transition.as_ref()
    }

    /// Returns the configured flag count.
    pub fn flag_count(&self) -> u32 {
        self.script.flag_count
    }

    pub fn take_audio_commands(&mut self) -> Vec<AudioCommand> {
        std::mem::take(&mut self.queued_audio)
    }

    pub fn queue_audio_command(&mut self, command: AudioCommand) {
        self.queued_audio.push(command);
    }

    /// Returns the external call currently waiting for host execution, if any.
    pub fn pending_external_call(&self) -> VnResult<Option<ExternalCallRequest>> {
        match self.current_event_ref() {
            Ok(EventCompiled::ExtCall { command, args }) => Ok(Some(ExternalCallRequest {
                event_ip: self.state.position,
                command: command.clone(),
                args: args.clone(),
            })),
            Ok(_) | Err(VnError::EndOfScript) => Ok(None),
            Err(err) => Err(err),
        }
    }

    /// Completes the currently pending external call with an explicit host outcome.
    ///
    /// The engine advances only after a matching successful outcome. Failed or stale outcomes are
    /// returned to the caller and leave the instruction pointer unchanged.
    pub fn complete_external_call(&mut self, outcome: ExternalCallOutcome) -> VnResult<()> {
        let request = self
            .pending_external_call()?
            .ok_or_else(|| VnError::InvalidScript("no external call is pending".to_string()))?;
        if request.event_ip != outcome.event_ip || request.command != outcome.command {
            return Err(VnError::InvalidScript(format!(
                "external call outcome mismatch: pending ip {} command '{}', got ip {} command '{}'",
                request.event_ip, request.command, outcome.event_ip, outcome.command
            )));
        }
        match outcome.status {
            ExternalCallStatus::Succeeded => {
                self.advance_position()?;
                self.sync_progress_snapshots();
                Ok(())
            }
            ExternalCallStatus::Failed(message) => Err(VnError::external_call_failed(
                request.event_ip,
                request.command,
                message,
            )),
        }
    }

    pub fn resume(&mut self) -> VnResult<()> {
        let event = self.current_event()?;
        match event {
            EventCompiled::ExtCall { command, .. } => {
                Err(VnError::external_call_pending(self.state.position, command))
            }
            _ => Ok(()),
        }
    }

    /// Returns compiled script labels.
    pub fn labels(&self) -> &std::collections::BTreeMap<String, u32> {
        &self.script.labels
    }

    /// Sets a flag value by id.
    pub fn set_flag(&mut self, id: u32, value: bool) {
        self.state.set_flag(id, value);
    }

    /// Jumps to a label by name.
    pub fn jump_to_label(&mut self, label: &str) -> VnResult<()> {
        let target_ip = self
            .script
            .labels
            .get(label)
            .copied()
            .ok_or_else(|| VnError::InvalidScript(format!("label '{label}' not found")))?;
        self.jump_to_ip(target_ip)
    }

    /// Restores the engine state from a saved snapshot.
    pub fn set_state(&mut self, state: EngineState) -> VnResult<()> {
        if state.position as usize > self.script.events.len() {
            return Err(VnError::InvalidScript(format!(
                "state position '{}' outside script",
                state.position
            )));
        }
        self.state = state;
        self.pending_transition = None;
        if let Some(read_model) = self.state.read_model.clone() {
            self.read_dialogue_ips = read_model.visited_dialogue_ips;
        }
        if let Some(route_progress) = self.state.route_progress.clone() {
            self.route_visited_ips = route_progress.visited_ips;
            self.choice_history = route_progress
                .selected_choices
                .into_iter()
                .map(|choice| ChoiceHistoryEntry {
                    event_ip: choice.event_ip,
                    prompt: choice.prompt,
                    option_index: choice.option_index,
                    option_text: choice.option_text,
                    target_ip: choice.target_ip,
                })
                .collect();
        } else {
            self.route_visited_ips.insert(self.state.position);
        }
        self.sync_progress_snapshots();
        Ok(())
    }

    /// Returns `true` if a dialogue at the given instruction pointer was already displayed.
    pub fn is_dialogue_read(&self, ip: u32) -> bool {
        self.read_dialogue_ips.contains(&ip)
    }

    /// Returns `true` when the current event is a dialogue previously displayed.
    pub fn is_current_dialogue_read(&self) -> bool {
        matches!(self.current_event_ref(), Ok(EventCompiled::Dialogue(_)))
            && self.read_dialogue_ips.contains(&self.state.position)
    }

    /// Returns the current in-memory choice history.
    pub fn choice_history(&self) -> &VecDeque<ChoiceHistoryEntry> {
        &self.choice_history
    }

    /// Clears runtime-only session history (read dialogue marks and choice history).
    pub fn clear_session_history(&mut self) {
        self.read_dialogue_ips.clear();
        self.route_visited_ips.clear();
        self.route_visited_ips.insert(self.state.position);
        self.choice_history.clear();
        self.sync_progress_snapshots();
    }

    /// Renders the current event using the provided renderer.
    pub fn render_current<R: RenderBackend>(&self, renderer: &R) -> VnResult<RenderOutput> {
        let event = self.current_event_ref()?;
        Ok(renderer.render(event, &self.state.visual))
    }

    /// Returns the current compiled event serialized as JSON.
    pub fn current_event_json(&self) -> VnResult<String> {
        let event = self.current_event()?;
        Ok(event.to_json_string())
    }

    fn record_choice_decision(
        &mut self,
        event_ip: u32,
        prompt: &str,
        option_index: usize,
        option_text: &str,
        target_ip: u32,
    ) {
        if self.choice_history.len() >= CHOICE_HISTORY_LIMIT {
            self.choice_history.pop_front();
        }
        self.choice_history.push_back(ChoiceHistoryEntry {
            event_ip,
            prompt: prompt.to_string(),
            option_index,
            option_text: option_text.to_string(),
            target_ip,
        });
    }

    fn sync_progress_snapshots(&mut self) {
        self.state.read_model = Some(self.read_model_snapshot());
        self.state.route_progress = Some(self.route_progress_snapshot());
    }
}

#[derive(Clone, Debug)]
pub struct StateChange {
    pub event: EventCompiled,
    pub visual: crate::visual::VisualState,
}

fn initialize_state(script: &ScriptCompiled) -> EngineState {
    let position = script.start_ip;
    let mut state = EngineState::new(position, script.flag_count);
    if let Some(EventCompiled::Scene(scene)) = script.events.get(position as usize) {
        state.visual.apply_scene(scene);
    }
    state
}

fn visual_render_commands(visual: &VisualState) -> Vec<RenderCommand> {
    let mut commands = vec![RenderCommand::Clear {
        color: "stage.background".to_string(),
    }];
    if let Some(background) = &visual.background {
        commands.push(RenderCommand::Image {
            asset: background.to_string(),
            rect: LayoutRect {
                x: 0.0,
                y: 0.0,
                width: 1280.0,
                height: 720.0,
            },
            fit: ImageFit::Cover,
            z: -100,
        });
    }
    let count = visual.characters.len().max(1);
    for (index, character) in visual.characters.iter().enumerate() {
        let x = character
            .x
            .map(|value| value as f32)
            .unwrap_or_else(|| character_slot_x(index, count));
        let y = character.y.map(|value| value as f32).unwrap_or(140.0);
        let scale = character.scale.unwrap_or(1.0).clamp(0.25, 4.0);
        let width = 260.0 * scale;
        let height = 420.0 * scale;
        let asset = character
            .expression
            .as_ref()
            .map(ToString::to_string)
            .unwrap_or_else(|| character.name.to_string());
        commands.push(RenderCommand::Image {
            asset,
            rect: LayoutRect {
                x: x - width * 0.5,
                y,
                width,
                height,
            },
            fit: ImageFit::Contain,
            z: index as i32,
        });
    }
    commands
}

fn character_slot_x(index: usize, count: usize) -> f32 {
    if count == 1 {
        return 640.0;
    }
    let left = 360.0;
    let right = 920.0;
    left + (right - left) * (index as f32 / (count.saturating_sub(1)) as f32)
}
