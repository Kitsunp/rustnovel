use std::collections::{HashSet, VecDeque};

use crate::assets::AssetId;
use crate::event::{CmpOp, CondCompiled, EventCompiled};

use super::runtime::Engine;

/// Prefetch traversal strategy used by runtime adapters.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrefetchMode {
    /// Preserve the historical behavior: inspect the next `depth` linear events only.
    Linear { depth: usize },
    /// Follow the likely runtime path using current flags/vars and the latest matching choice.
    LikelyPath { depth: usize },
    /// Explore all reachable branch targets up to `depth`, capped by `max_assets`.
    BranchAware { depth: usize, max_assets: usize },
}

impl Engine {
    /// Returns unique upcoming asset paths that can be prefetched safely.
    ///
    /// This intentionally excludes non-path semantic fields to avoid prefetching invalid resources.
    pub fn peek_next_asset_paths(&self, depth: usize) -> Vec<String> {
        self.peek_next_asset_paths_with_mode(PrefetchMode::Linear { depth })
    }

    /// Returns unique upcoming asset paths using an explicit traversal strategy.
    pub fn peek_next_asset_paths_with_mode(&self, mode: PrefetchMode) -> Vec<String> {
        match mode {
            PrefetchMode::Linear { depth } => self.peek_linear_asset_paths(depth),
            PrefetchMode::LikelyPath { depth } => self.peek_likely_asset_paths(depth),
            PrefetchMode::BranchAware { depth, max_assets } => {
                self.peek_branch_asset_paths(depth, max_assets)
            }
        }
    }

    fn peek_linear_asset_paths(&self, depth: usize) -> Vec<String> {
        let mut seen = HashSet::new();
        let mut paths = Vec::new();
        let start = self.state().position as usize;
        let end = (start + depth).min(self.script().events.len());
        for event in &self.script().events[start..end] {
            collect_prefetch_paths_from_event(event, &mut seen, &mut paths);
        }
        paths
    }

    /// Returns the unique upcoming asset ids that can be prefetched safely.
    pub fn peek_next_assets(&self, depth: usize) -> Vec<AssetId> {
        self.peek_next_asset_paths(depth)
            .into_iter()
            .map(|path| AssetId::from_path(&path))
            .collect()
    }

    /// Returns upcoming asset ids using an explicit traversal strategy.
    pub fn peek_next_assets_with_mode(&self, mode: PrefetchMode) -> Vec<AssetId> {
        self.peek_next_asset_paths_with_mode(mode)
            .into_iter()
            .map(|path| AssetId::from_path(&path))
            .collect()
    }

    /// Follows the path the engine would most likely take without mutating runtime state.
    pub fn peek_likely_asset_paths(&self, depth: usize) -> Vec<String> {
        let mut seen = HashSet::new();
        let mut paths = Vec::new();
        let mut ip = self.state().position as usize;
        let mut visited = HashSet::new();
        for _ in 0..depth {
            if ip >= self.script().events.len() || !visited.insert(ip) {
                break;
            }
            let event = &self.script().events[ip];
            collect_prefetch_paths_from_event(event, &mut seen, &mut paths);
            match likely_successor_ip(self, ip, event) {
                Some(next) => ip = next,
                None => break,
            }
        }
        paths
    }

    /// Explores branch targets from the current instruction pointer with explicit budgets.
    pub fn peek_branch_asset_paths(&self, depth: usize, max_assets: usize) -> Vec<String> {
        if depth == 0 || max_assets == 0 {
            return Vec::new();
        }
        let mut seen_paths = HashSet::new();
        let mut paths = Vec::new();
        let mut visited_ips = HashSet::new();
        let mut queue = VecDeque::from([(self.state().position as usize, 0usize)]);
        while let Some((ip, distance)) = queue.pop_front() {
            if distance >= depth || ip >= self.script().events.len() || !visited_ips.insert(ip) {
                continue;
            }
            let event = &self.script().events[ip];
            collect_prefetch_paths_from_event(event, &mut seen_paths, &mut paths);
            if paths.len() >= max_assets {
                paths.truncate(max_assets);
                break;
            }
            for next_ip in branch_successor_ips(self, ip, event) {
                queue.push_back((next_ip, distance + 1));
            }
        }
        paths
    }
}

fn likely_successor_ip(engine: &Engine, ip: usize, event: &EventCompiled) -> Option<usize> {
    match event {
        EventCompiled::Choice(choice) => engine
            .choice_history()
            .iter()
            .rev()
            .find(|entry| entry.event_ip as usize == ip)
            .and_then(|entry| choice.options.get(entry.option_index))
            .or_else(|| choice.options.first())
            .map(|option| option.target_ip as usize),
        EventCompiled::Jump { target_ip } => Some(*target_ip as usize),
        EventCompiled::JumpIf { cond, target_ip } if evaluate_cond(engine, cond) => {
            Some(*target_ip as usize)
        }
        EventCompiled::ExtCall { .. } => ip.checked_add(1),
        EventCompiled::JumpIf { .. } => ip.checked_add(1),
        _ => ip.checked_add(1),
    }
}

fn branch_successor_ips(engine: &Engine, ip: usize, event: &EventCompiled) -> Vec<usize> {
    let script_len = engine.script().events.len();
    match event {
        EventCompiled::Choice(choice) => choice
            .options
            .iter()
            .filter_map(|option| in_script(option.target_ip as usize, script_len))
            .collect(),
        EventCompiled::Jump { target_ip } => in_script(*target_ip as usize, script_len)
            .into_iter()
            .collect(),
        EventCompiled::JumpIf { target_ip, .. } => {
            let mut successors = Vec::with_capacity(2);
            if let Some(next) = in_script(*target_ip as usize, script_len) {
                successors.push(next);
            }
            if let Some(next) = ip
                .checked_add(1)
                .and_then(|next| in_script(next, script_len))
            {
                successors.push(next);
            }
            successors
        }
        EventCompiled::ExtCall { .. } => ip
            .checked_add(1)
            .and_then(|next| in_script(next, script_len))
            .into_iter()
            .collect(),
        _ => ip
            .checked_add(1)
            .and_then(|next| in_script(next, script_len))
            .into_iter()
            .collect(),
    }
}

fn in_script(ip: usize, script_len: usize) -> Option<usize> {
    (ip < script_len).then_some(ip)
}

fn evaluate_cond(engine: &Engine, cond: &CondCompiled) -> bool {
    match cond {
        CondCompiled::Flag { flag_id, is_set } => engine.state().get_flag(*flag_id) == *is_set,
        CondCompiled::VarCmp { var_id, op, value } => {
            let var_val = engine.state().get_var(*var_id);
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

fn collect_prefetch_paths_from_event(
    event: &EventCompiled,
    seen: &mut HashSet<String>,
    output: &mut Vec<String>,
) {
    match event {
        EventCompiled::Scene(scene) => {
            if let Some(background) = &scene.background {
                push_unique_prefetch_path(background.as_ref(), seen, output);
            }
            if let Some(music) = &scene.music {
                push_unique_prefetch_path(music.as_ref(), seen, output);
            }
            for character in &scene.characters {
                if let Some(expression) = &character.expression {
                    push_unique_prefetch_path(expression.as_ref(), seen, output);
                }
            }
        }
        EventCompiled::Patch(patch) => {
            if let Some(background) = &patch.background {
                push_unique_prefetch_path(background.as_ref(), seen, output);
            }
            if let Some(music) = &patch.music {
                push_unique_prefetch_path(music.as_ref(), seen, output);
            }
            for character in &patch.add {
                if let Some(expression) = &character.expression {
                    push_unique_prefetch_path(expression.as_ref(), seen, output);
                }
            }
            for character in &patch.update {
                if let Some(expression) = &character.expression {
                    push_unique_prefetch_path(expression.as_ref(), seen, output);
                }
            }
        }
        EventCompiled::AudioAction(action) if action.action == 0 => {
            if let Some(asset) = &action.asset {
                push_unique_prefetch_path(asset.as_ref(), seen, output);
            }
        }
        _ => {}
    }
}

fn push_unique_prefetch_path(value: &str, seen: &mut HashSet<String>, output: &mut Vec<String>) {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return;
    }
    if seen.insert(trimmed.to_string()) {
        output.push(trimmed.to_string());
    }
}
