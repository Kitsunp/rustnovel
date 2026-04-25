use std::collections::BTreeMap;

use crate::ScriptRaw;
use crate::{ChoiceOptionRaw, ChoiceRaw, DialogueRaw, EventRaw};

use super::*;

fn linear_script() -> ScriptRaw {
    ScriptRaw::new(
        vec![EventRaw::Dialogue(DialogueRaw {
            speaker: "Narrator".to_string(),
            text: "Hola".to_string(),
        })],
        BTreeMap::from([("start".to_string(), 0usize)]),
    )
}

#[test]
fn repro_case_json_roundtrip() {
    let mut case = ReproCase::new("repro", linear_script());
    case.oracle.expected_stop_reason = Some(ReproStopReason::Finished);
    case.oracle.monitors.push(ReproMonitor::EventKindAtStep {
        monitor_id: "m_event".to_string(),
        step: 0,
        expected: "dialogue".to_string(),
    });
    let payload = case.to_json().expect("serialize repro");
    let loaded = ReproCase::from_json(&payload).expect("deserialize repro");
    assert_eq!(loaded.schema, REPRO_CASE_SCHEMA);
    assert_eq!(loaded.oracle.monitors.len(), 1);
}

#[test]
fn run_repro_case_matches_monitor() {
    let mut case = ReproCase::new("monitor", linear_script());
    case.oracle.monitors.push(ReproMonitor::EventKindAtStep {
        monitor_id: "kind0".to_string(),
        step: 0,
        expected: "dialogue".to_string(),
    });
    let report = run_repro_case(&case);
    assert_eq!(report.stop_reason, ReproStopReason::Finished);
    assert!(report.oracle_triggered);
    assert!(report.matched_monitors.iter().any(|id| id == "kind0"));
}

#[test]
fn run_repro_case_honors_choice_route() {
    let script = ScriptRaw::new(
        vec![
            EventRaw::Choice(ChoiceRaw {
                prompt: "Pick".to_string(),
                options: vec![
                    ChoiceOptionRaw {
                        text: "A".to_string(),
                        target: "left".to_string(),
                    },
                    ChoiceOptionRaw {
                        text: "B".to_string(),
                        target: "right".to_string(),
                    },
                ],
            }),
            EventRaw::Dialogue(DialogueRaw {
                speaker: "L".to_string(),
                text: "Left".to_string(),
            }),
            EventRaw::Dialogue(DialogueRaw {
                speaker: "R".to_string(),
                text: "Right".to_string(),
            }),
        ],
        BTreeMap::from([
            ("start".to_string(), 0usize),
            ("left".to_string(), 1usize),
            ("right".to_string(), 2usize),
        ]),
    );
    let mut case = ReproCase::new("choice", script);
    case.choice_route = vec![1];
    case.oracle
        .monitors
        .push(ReproMonitor::EventSignatureContains {
            monitor_id: "right_dialogue".to_string(),
            step: 1,
            needle: "Right".to_string(),
        });
    let report = run_repro_case(&case);
    assert!(report.oracle_triggered);
    assert!(report
        .matched_monitors
        .iter()
        .any(|id| id == "right_dialogue"));
}
