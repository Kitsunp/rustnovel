use std::collections::BTreeSet;

use crate::event::EventRaw;

use super::types::ImportState;

pub(super) fn patch_missing_targets(state: &mut ImportState) {
    let mut missing = BTreeSet::new();
    for event in &state.events {
        match event {
            EventRaw::Jump { target } => {
                if !state.labels.contains_key(target) {
                    missing.insert(target.clone());
                }
            }
            EventRaw::JumpIf { target, .. } => {
                if !state.labels.contains_key(target) {
                    missing.insert(target.clone());
                }
            }
            EventRaw::Choice(choice) => {
                for option in &choice.options {
                    if !state.labels.contains_key(&option.target) {
                        missing.insert(option.target.clone());
                    }
                }
            }
            _ => {}
        }
    }

    for label in missing {
        let fallback = state.next_synthetic_label("missing_target");
        state.push_ext_call(
            "renpy_missing_target",
            vec![label.clone()],
            None,
            "missing_target_label",
            format!("Missing target label '{label}' remapped to synthetic fallback"),
        );
        let idx = state.events.len().saturating_sub(1);
        state.labels.insert(fallback.clone(), idx);
        state.labels.insert(label, idx);
    }
}

pub(super) fn enforce_start_label(state: &mut ImportState, entry_label: &str) {
    if let Some(start_idx) = state.labels.get(entry_label).copied() {
        state.labels.insert("start".to_string(), start_idx);
        return;
    }
    if let Some(start_idx) = state.labels.get("start").copied() {
        state.labels.insert("start".to_string(), start_idx);
        return;
    }
    if !state.events.is_empty() {
        state.labels.insert("start".to_string(), 0);
        state.push_issue(
            "warning",
            "entry_label_missing",
            format!("Entry label '{entry_label}' not found; using first event as start"),
            None,
            Some("labels.start=0".to_string()),
        );
    } else {
        state.labels.insert("start".to_string(), 0);
    }
}
