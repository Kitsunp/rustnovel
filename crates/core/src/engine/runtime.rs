use std::collections::{BTreeSet, VecDeque};

use crate::audio::AudioCommand;
use crate::error::{VnError, VnResult};
use crate::event::{CmpOp, CondCompiled, EventCompiled, SceneTransitionCompiled};
use crate::render::{RenderBackend, RenderOutput};
use crate::resource::ResourceLimiter;
use crate::script::{ScriptCompiled, ScriptRaw};
use crate::security::SecurityPolicy;
use crate::state::EngineState;

use super::audio::{append_music_delta, audio_command_from_action, initial_audio_commands};

const CHOICE_HISTORY_LIMIT: usize = 512;

/// Recorded decision made by the player at a Choice event.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChoiceHistoryEntry {
    pub event_ip: u32,
    pub option_index: usize,
    pub option_text: String,
    pub target_ip: u32,
}

/// Execution engine for compiled scripts.
#[derive(Clone, Debug)]
pub struct Engine {
    script: ScriptCompiled,
    state: EngineState,
    policy: SecurityPolicy,
    queued_audio: Vec<AudioCommand>,
    pending_transition: Option<SceneTransitionCompiled>,
    read_dialogue_ips: BTreeSet<u32>,
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
        Ok(Self::from_validated_compiled(script, policy))
    }

    fn from_validated_compiled(script: ScriptCompiled, policy: SecurityPolicy) -> Self {
        let state = initialize_state(&script);
        let queued_audio = initial_audio_commands(&state);
        Self {
            script,
            state,
            policy,
            queued_audio,
            pending_transition: None,
            read_dialogue_ips: BTreeSet::new(),
            choice_history: VecDeque::with_capacity(64),
        }
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
        let current_ip = self.state.position;
        self.pending_transition = None;
        match event {
            EventCompiled::Jump { target_ip } => {
                self.jump_to_ip_with_audio(*target_ip, audio_commands)
            }
            EventCompiled::SetFlag { flag_id, value } => {
                self.state.set_flag(*flag_id, *value);
                self.advance_position()
            }
            EventCompiled::Scene(scene) => {
                let before_music = self.state.visual.music.clone();
                self.state.visual.apply_scene(scene);
                append_music_delta(before_music, &self.state.visual.music, audio_commands);
                self.advance_position()
            }
            EventCompiled::Choice(_) => Ok(()),
            EventCompiled::Dialogue(dialogue) => {
                self.state.record_dialogue(dialogue);
                self.read_dialogue_ips.insert(current_ip);
                self.advance_position()
            }
            EventCompiled::SetVar { var_id, value } => {
                self.state.set_var(*var_id, *value);
                self.advance_position()
            }
            EventCompiled::JumpIf { cond, target_ip } => {
                if self.evaluate_cond(cond) {
                    self.jump_to_ip_with_audio(*target_ip, audio_commands)
                } else {
                    self.advance_position()
                }
            }
            EventCompiled::Patch(patch) => {
                let before_music = self.state.visual.music.clone();
                self.state.visual.apply_patch(patch);
                append_music_delta(before_music, &self.state.visual.music, audio_commands);
                self.advance_position()
            }
            EventCompiled::ExtCall { .. } => Ok(()),
            EventCompiled::AudioAction(action) => {
                if let Some(command) = audio_command_from_action(action) {
                    audio_commands.push(command);
                }
                self.advance_position()
            }
            EventCompiled::SetCharacterPosition(pos) => {
                self.state.visual.set_character_position(pos);
                self.advance_position()
            }
            EventCompiled::Transition(transition) => {
                self.pending_transition = Some(transition.clone());
                self.advance_position()
            }
        }
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

    fn advance_position(&mut self) -> VnResult<()> {
        let next = self.state.position.saturating_add(1);
        if next as usize >= self.script.events.len() {
            self.state.position = self.script.events.len() as u32;
            return Ok(());
        }
        self.state.position = next;
        Ok(())
    }

    fn jump_to_ip(&mut self, target_ip: u32) -> VnResult<()> {
        let mut audio_commands = self.take_audio_commands();
        let result = self.jump_to_ip_with_audio(target_ip, &mut audio_commands);
        self.queued_audio = audio_commands;
        result
    }

    fn jump_to_ip_with_audio(
        &mut self,
        target_ip: u32,
        audio_commands: &mut Vec<AudioCommand>,
    ) -> VnResult<()> {
        if target_ip as usize > self.script.events.len() {
            return Err(VnError::InvalidScript(format!(
                "jump target '{target_ip}' outside script"
            )));
        }
        if target_ip as usize == self.script.events.len() {
            self.state.position = target_ip;
            return Ok(());
        }
        let scene = match self.script.events.get(target_ip as usize) {
            Some(EventCompiled::Scene(scene)) => Some(scene.clone()),
            _ => None,
        };
        self.state.position = target_ip;
        if let Some(scene) = scene {
            let before_music = self.state.visual.music.clone();
            self.state.visual.apply_scene(&scene);
            append_music_delta(before_music, &self.state.visual.music, audio_commands);
        }
        Ok(())
    }

    /// Returns the full engine state.
    pub fn state(&self) -> &EngineState {
        &self.state
    }

    /// Returns the security policy in use.
    pub fn policy(&self) -> &SecurityPolicy {
        &self.policy
    }

    /// Returns the current visual state.
    pub fn visual_state(&self) -> &crate::visual::VisualState {
        &self.state.visual
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

    pub fn resume(&mut self) -> VnResult<()> {
        let event = self.current_event()?;
        match event {
            EventCompiled::ExtCall { .. } => self.advance_position(),
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
        self.read_dialogue_ips.clear();
        self.choice_history.clear();
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
        self.choice_history.clear();
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
        option_index: usize,
        option_text: &str,
        target_ip: u32,
    ) {
        if self.choice_history.len() >= CHOICE_HISTORY_LIMIT {
            self.choice_history.pop_front();
        }
        self.choice_history.push_back(ChoiceHistoryEntry {
            event_ip,
            option_index,
            option_text: option_text.to_string(),
            target_ip,
        });
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
