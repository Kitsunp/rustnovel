use std::collections::{HashMap, HashSet};

use super::signatures::{event_kind_raw, raw_event_signature};
use super::{ChoicePolicy, ChoiceStrategy};
use crate::event::{CmpOp, CondRaw, EventRaw};
use crate::script::ScriptRaw;

pub fn select_choice_index(
    policy: &ChoicePolicy,
    step: usize,
    option_len: usize,
    choice_cursor: usize,
) -> Option<usize> {
    if option_len == 0 {
        return None;
    }
    let index = match policy {
        ChoicePolicy::Strategy(ChoiceStrategy::First) => 0,
        ChoicePolicy::Strategy(ChoiceStrategy::Last) => option_len.saturating_sub(1),
        ChoicePolicy::Strategy(ChoiceStrategy::Alternating) => step % option_len,
        ChoicePolicy::Scripted(path) => path.get(choice_cursor).copied()?,
    };
    (index < option_len).then_some(index)
}

#[derive(Debug, Clone, Default)]
struct RawVisualState {
    background: Option<String>,
    music: Option<String>,
    characters: HashSet<String>,
}

#[derive(Debug, Clone, Default)]
struct RawSimulationState {
    flags: HashMap<String, bool>,
    vars: HashMap<String, i32>,
    visual: RawVisualState,
}

#[derive(Debug, Clone)]
pub struct RawStepTrace {
    pub event_ip: u32,
    pub event_kind: String,
    pub event_signature: String,
    pub visual_background: Option<String>,
    pub visual_music: Option<String>,
    pub character_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RawSimulationStopReason {
    Finished,
    StepLimit,
    LoopDetected,
    StartError,
    MissingJumpTarget,
    MissingChoiceTarget,
    InvalidChoice,
}

impl RawSimulationStopReason {
    pub fn is_error(self) -> bool {
        !matches!(self, Self::Finished | Self::StepLimit | Self::LoopDetected)
    }
}

#[derive(Debug, Clone)]
pub struct RawSimulationReport {
    pub steps: Vec<RawStepTrace>,
    pub stop_reason: RawSimulationStopReason,
    pub stop_message: String,
    pub failing_event_ip: Option<u32>,
    pub errors: Vec<String>,
}

impl RawSimulationReport {
    fn new() -> Self {
        Self {
            steps: Vec::new(),
            stop_reason: RawSimulationStopReason::Finished,
            stop_message: "Raw simulation finished".to_string(),
            failing_event_ip: None,
            errors: Vec::new(),
        }
    }

    fn stop(
        &mut self,
        stop_reason: RawSimulationStopReason,
        failing_event_ip: Option<u32>,
        message: String,
    ) {
        self.stop_reason = stop_reason;
        self.stop_message = message.clone();
        self.failing_event_ip = failing_event_ip;
        if stop_reason.is_error() {
            self.errors.push(message);
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct RouteEnumerationReport {
    pub routes: Vec<Vec<usize>>,
    pub routes_discovered: usize,
    pub route_limit_hit: bool,
    pub depth_limit_hit: bool,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone)]
struct RawRouteFrame {
    ip: usize,
    steps: usize,
    choice_depth: usize,
    choices: Vec<usize>,
    state: RawSimulationState,
}

pub fn enumerate_choice_routes(
    script: &ScriptRaw,
    max_steps: usize,
    max_routes: usize,
    max_choice_depth: usize,
) -> Vec<Vec<usize>> {
    enumerate_choice_routes_with_report(script, max_steps, max_routes, max_choice_depth).routes
}

pub fn enumerate_choice_routes_with_report(
    script: &ScriptRaw,
    max_steps: usize,
    max_routes: usize,
    max_choice_depth: usize,
) -> RouteEnumerationReport {
    let mut routes = Vec::new();
    let mut route_limit_hit = false;
    let mut depth_limit_hit = false;
    let mut errors = Vec::new();
    let mut visited = HashSet::new();
    let start_ip = match script.start_index() {
        Ok(idx) => idx,
        Err(err) => {
            return RouteEnumerationReport {
                errors: vec![format!("route enumeration start error: {err}")],
                ..Default::default()
            };
        }
    };
    if start_ip >= script.events.len() {
        return RouteEnumerationReport {
            errors: vec![format!(
                "route enumeration start label points outside executable events: start={start_ip}, events={}",
                script.events.len()
            )],
            ..Default::default()
        };
    }

    let mut initial_state = RawSimulationState::default();
    bootstrap_initial_state(script, start_ip, &mut initial_state);
    let mut stack = vec![RawRouteFrame {
        ip: start_ip,
        steps: 0,
        choice_depth: 0,
        choices: Vec::new(),
        state: initial_state,
    }];

    while let Some(frame) = stack.pop() {
        if routes.len() >= max_routes {
            route_limit_hit = true;
            break;
        }
        if !visited.insert(route_frame_signature(&frame)) {
            depth_limit_hit = true;
            routes.push(frame.choices);
            continue;
        }
        if frame.steps >= max_steps || frame.ip >= script.events.len() {
            routes.push(frame.choices);
            continue;
        }

        let event = &script.events[frame.ip];
        if let EventRaw::Choice(choice) = event {
            if choice.options.is_empty() {
                routes.push(frame.choices);
                continue;
            }
            if frame.choice_depth >= max_choice_depth {
                depth_limit_hit = true;
                routes.push(frame.choices);
                continue;
            }

            let mut pushed = false;
            for option_idx in (0..choice.options.len()).rev() {
                let Some(target_ip) = script
                    .labels
                    .get(&choice.options[option_idx].target)
                    .copied()
                else {
                    errors.push(format!(
                        "choice at ip {} option {} targets missing label '{}'",
                        frame.ip, option_idx, choice.options[option_idx].target
                    ));
                    continue;
                };
                let mut next = frame.clone();
                next.steps = next.steps.saturating_add(1);
                next.choice_depth = next.choice_depth.saturating_add(1);
                next.ip = target_ip;
                next.choices.push(option_idx);
                stack.push(next);
                pushed = true;
            }
            if !pushed {
                routes.push(frame.choices);
            }
            continue;
        }

        let mut next = frame;
        let mut next_ip = next.ip.saturating_add(1);
        apply_state_mutations(event, &mut next.state);
        match event {
            EventRaw::Jump { target } => {
                let Some(target_ip) = script.labels.get(target).copied() else {
                    errors.push(format!(
                        "jump at ip {} targets missing label '{}'",
                        next.ip, target
                    ));
                    routes.push(next.choices);
                    continue;
                };
                next_ip = target_ip;
            }
            EventRaw::JumpIf { cond, target } if eval_cond_raw(cond, &next.state) => {
                let Some(target_ip) = script.labels.get(target).copied() else {
                    errors.push(format!(
                        "conditional jump at ip {} targets missing label '{}'",
                        next.ip, target
                    ));
                    routes.push(next.choices);
                    continue;
                };
                next_ip = target_ip;
            }
            _ => {}
        }

        next.ip = next_ip;
        next.steps = next.steps.saturating_add(1);
        stack.push(next);
    }

    if routes.is_empty() {
        routes.push(Vec::new());
    }
    routes.sort();
    routes.dedup();
    let routes_discovered = routes.len();
    if routes.len() > max_routes {
        route_limit_hit = true;
    }
    routes.truncate(max_routes);
    RouteEnumerationReport {
        routes,
        routes_discovered,
        route_limit_hit,
        depth_limit_hit,
        errors,
    }
}

pub fn simulate_raw_sequence(
    script: &ScriptRaw,
    max_steps: usize,
    policy: &ChoicePolicy,
) -> RawSimulationReport {
    let mut report = RawSimulationReport::new();
    let mut state = RawSimulationState::default();
    let mut steps = 0usize;
    let mut choice_cursor = 0usize;
    let mut visited = HashSet::new();
    let mut ip = match script.start_index() {
        Ok(idx) => idx,
        Err(err) => {
            report.stop(
                RawSimulationStopReason::StartError,
                None,
                format!("raw simulation start error: {err}"),
            );
            return report;
        }
    };
    if ip >= script.events.len() {
        report.stop(
            RawSimulationStopReason::StartError,
            Some(ip as u32),
            format!(
                "raw simulation start label points outside executable events: start={ip}, events={}",
                script.events.len()
            ),
        );
        return report;
    }
    bootstrap_initial_state(script, ip, &mut state);

    loop {
        if steps >= max_steps {
            report.stop(
                RawSimulationStopReason::StepLimit,
                Some(ip as u32),
                format!("Raw simulation reached {max_steps} steps; possible loop or blocking flow"),
            );
            return report;
        }
        if ip >= script.events.len() {
            report.stop(
                RawSimulationStopReason::Finished,
                None,
                format!("Raw simulation finished in {steps} step(s)"),
            );
            return report;
        }
        if !visited.insert(raw_state_signature(ip, &state)) {
            report.stop(
                RawSimulationStopReason::LoopDetected,
                Some(ip as u32),
                format!("Raw simulation detected a repeated state at ip {ip}"),
            );
            return report;
        }
        let event = &script.events[ip];
        report.steps.push(RawStepTrace {
            event_ip: ip as u32,
            event_kind: event_kind_raw(event).to_string(),
            event_signature: raw_event_signature(event),
            visual_background: state.visual.background.clone(),
            visual_music: state.visual.music.clone(),
            character_count: state.visual.characters.len(),
        });

        let mut next_ip = ip + 1;
        apply_state_mutations(event, &mut state);
        match event {
            EventRaw::Jump { target } => {
                let Some(target_ip) = script.labels.get(target).copied() else {
                    report.stop(
                        RawSimulationStopReason::MissingJumpTarget,
                        Some(ip as u32),
                        format!("jump at ip {ip} targets missing label '{target}'"),
                    );
                    return report;
                };
                next_ip = target_ip;
            }
            EventRaw::Choice(choice) => {
                let Some(choice_idx) =
                    select_choice_index(policy, steps, choice.options.len(), choice_cursor)
                else {
                    report.stop(
                        RawSimulationStopReason::InvalidChoice,
                        Some(ip as u32),
                        format!(
                            "choice at ip {ip} could not select a valid option for policy {}",
                            policy.label().as_str()
                        ),
                    );
                    return report;
                };
                choice_cursor = choice_cursor.saturating_add(1);
                let Some(target_label) = choice
                    .options
                    .get(choice_idx)
                    .map(|option| option.target.as_str())
                else {
                    report.stop(
                        RawSimulationStopReason::InvalidChoice,
                        Some(ip as u32),
                        format!("choice at ip {ip} selected missing option index {choice_idx}"),
                    );
                    return report;
                };
                let Some(target_ip) = script.labels.get(target_label).copied() else {
                    report.stop(
                        RawSimulationStopReason::MissingChoiceTarget,
                        Some(ip as u32),
                        format!(
                            "choice at ip {ip} option {choice_idx} targets missing label '{target_label}'"
                        ),
                    );
                    return report;
                };
                next_ip = target_ip;
            }
            EventRaw::JumpIf { cond, target } if eval_cond_raw(cond, &state) => {
                let Some(target_ip) = script.labels.get(target).copied() else {
                    report.stop(
                        RawSimulationStopReason::MissingJumpTarget,
                        Some(ip as u32),
                        format!("conditional jump at ip {ip} targets missing label '{target}'"),
                    );
                    return report;
                };
                next_ip = target_ip;
            }
            _ => {}
        }

        ip = next_ip;
        steps += 1;
    }
}

fn route_frame_signature(frame: &RawRouteFrame) -> String {
    raw_state_signature(frame.ip, &frame.state)
}

fn raw_state_signature(ip: usize, state: &RawSimulationState) -> String {
    let mut flags = state
        .flags
        .iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect::<Vec<_>>();
    flags.sort();
    let mut vars = state
        .vars
        .iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect::<Vec<_>>();
    vars.sort();
    let mut characters = state.visual.characters.iter().cloned().collect::<Vec<_>>();
    characters.sort();
    format!(
        "ip={ip};bg={};music={};flags={};vars={};chars={}",
        state.visual.background.as_deref().unwrap_or(""),
        state.visual.music.as_deref().unwrap_or(""),
        flags.join(","),
        vars.join(","),
        characters.join(",")
    )
}

fn bootstrap_initial_state(script: &ScriptRaw, ip: usize, state: &mut RawSimulationState) {
    if let Some(event @ EventRaw::Scene(_)) = script.events.get(ip) {
        apply_state_mutations(event, state);
    }
}

fn apply_state_mutations(event: &EventRaw, state: &mut RawSimulationState) {
    match event {
        EventRaw::Scene(scene) => {
            if let Some(bg) = &scene.background {
                state.visual.background = Some(bg.clone());
            }
            if let Some(music) = &scene.music {
                state.visual.music = Some(music.clone());
            }
            if !scene.characters.is_empty() {
                state.visual.characters.clear();
                for character in &scene.characters {
                    state.visual.characters.insert(character.name.clone());
                }
            }
        }
        EventRaw::Patch(patch) => {
            if let Some(bg) = &patch.background {
                state.visual.background = Some(bg.clone());
            }
            if let Some(music) = &patch.music {
                state.visual.music = Some(music.clone());
            }
            for removed in &patch.remove {
                state.visual.characters.remove(removed);
            }
            for added in &patch.add {
                state.visual.characters.insert(added.name.clone());
            }
            for updated in &patch.update {
                state.visual.characters.insert(updated.name.clone());
            }
        }
        EventRaw::SetCharacterPosition(pos) => {
            state.visual.characters.insert(pos.name.clone());
        }
        EventRaw::SetFlag { key, value } => {
            state.flags.insert(key.clone(), *value);
        }
        EventRaw::SetVar { key, value } => {
            state.vars.insert(key.clone(), *value);
        }
        _ => {}
    }
}

fn eval_cond_raw(cond: &CondRaw, state: &RawSimulationState) -> bool {
    match cond {
        CondRaw::Flag { key, is_set } => state.flags.get(key).copied().unwrap_or(false) == *is_set,
        CondRaw::VarCmp { key, op, value } => {
            let current = state.vars.get(key).copied().unwrap_or(0);
            match op {
                CmpOp::Eq => current == *value,
                CmpOp::Ne => current != *value,
                CmpOp::Lt => current < *value,
                CmpOp::Le => current <= *value,
                CmpOp::Gt => current > *value,
                CmpOp::Ge => current >= *value,
            }
        }
    }
}
