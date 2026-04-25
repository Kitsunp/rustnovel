//! Runtime engine that executes compiled scripts.

mod audio;
mod prefetch;
mod runtime;

pub use runtime::{ChoiceHistoryEntry, Engine, StateChange};

#[cfg(test)]
#[path = "tests/engine_tests.rs"]
mod tests;
