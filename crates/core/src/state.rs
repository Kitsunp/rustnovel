//! Engine state storage for execution.

use std::collections::VecDeque;

use serde::{Deserialize, Serialize};

use crate::event::DialogueCompiled;
use crate::visual::VisualState;

const HISTORY_LIMIT: usize = 200;

/// Runtime state for the engine, including position, flags, variables, and visuals.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct EngineState {
    pub position: u32,
    pub flags: Vec<u64>,
    pub vars: Vec<i32>,
    pub visual: VisualState,
    pub history: VecDeque<DialogueCompiled>,
}

impl EngineState {
    /// Creates a new engine state with the given starting position and flag capacity.
    pub fn new(position: u32, flag_count: u32) -> Self {
        Self {
            position,
            flags: vec![0; bitset_len(flag_count)],
            vars: Vec::new(),
            visual: VisualState::default(),
            history: VecDeque::with_capacity(HISTORY_LIMIT),
        }
    }

    /// Sets a flag value by id.
    pub fn set_flag(&mut self, id: u32, value: bool) {
        let (word, mask) = flag_bit(id);
        if word >= self.flags.len() {
            self.flags.resize(word + 1, 0);
        }
        if value {
            self.flags[word] |= mask;
        } else {
            self.flags[word] &= !mask;
        }
    }

    /// Reads a flag value by id.
    pub fn get_flag(&self, id: u32) -> bool {
        let (word, mask) = flag_bit(id);
        self.flags
            .get(word)
            .map(|bits| bits & mask != 0)
            .unwrap_or(false)
    }

    /// Sets a variable value by id.
    pub fn set_var(&mut self, id: u32, value: i32) {
        let idx = id as usize;
        if idx >= self.vars.len() {
            self.vars.resize(idx + 1, 0);
        }
        self.vars[idx] = value;
    }

    /// Reads a variable value by id.
    pub fn get_var(&self, id: u32) -> i32 {
        self.vars.get(id as usize).copied().unwrap_or(0)
    }

    /// Records a dialogue line into the history buffer.
    pub fn record_dialogue(&mut self, dialogue: &DialogueCompiled) {
        if self.history.len() >= HISTORY_LIMIT {
            self.history.pop_front();
        }
        self.history.push_back(dialogue.clone());
    }
}

fn bitset_len(flag_count: u32) -> usize {
    let count = usize::try_from(flag_count).unwrap_or(0);
    count.div_ceil(64)
}

fn flag_bit(id: u32) -> (usize, u64) {
    let idx = id as usize;
    let word = idx / 64;
    let mask = 1u64 << (idx % 64);
    (word, mask)
}
