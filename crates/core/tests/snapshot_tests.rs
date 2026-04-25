//! Integration tests using snapshot testing with insta.
//!
//! These tests execute complex "golden scripts" and verify the entire
//! execution trace remains deterministic and unchanged.

mod common;
use common::run_headless;

#[test]
fn golden_script_branching_v1() {
    let script_json = r#"
    {
        "script_schema_version": "1.0",
        "events": [
            {"type": "dialogue", "speaker": "Narrator", "text": "Testing branching"},
            {"type": "set_var", "key": "counter", "value": 0},
            {"type": "set_var", "key": "counter", "value": 5},
            {"type": "jump_if", "cond": {"kind": "var_cmp", "key": "counter", "op": "gt", "value": 3}, "target": "high"},
            {"type": "dialogue", "speaker": "Narrator", "text": "Counter is low"},
            {"type": "jump", "target": "end"},
            {"type": "dialogue", "speaker": "Narrator", "text": "Counter is high"},
            {"type": "set_flag", "key": "completed", "value": true},
            {"type": "jump_if", "cond": {"kind": "flag", "key": "completed", "is_set": true}, "target": "victory"},
            {"type": "dialogue", "speaker": "Narrator", "text": "Not completed"},
            {"type": "dialogue", "speaker": "Narrator", "text": "Victory!"}
        ],
        "labels": {
            "start": 0,
            "high": 6,
            "end": 7,
            "victory": 10
        }
    }
    "#;

    let trace = run_headless(script_json, 20);
    insta::assert_yaml_snapshot!(trace);
}

#[test]
fn golden_script_loop_with_condition() {
    let script_json = r#"
    {
        "script_schema_version": "1.0",
        "events": [
            {"type": "set_var", "key": "i", "value": 0},
            {"type": "dialogue", "speaker": "Loop", "text": "Iteration"},
            {"type": "set_var", "key": "i", "value": 1},
            {"type": "jump_if", "cond": {"kind": "var_cmp", "key": "i", "op": "lt", "value": 3}, "target": "loop"},
            {"type": "dialogue", "speaker": "Done", "text": "Loop finished"}
        ],
        "labels": {
            "start": 0,
            "loop": 1
        }
    }
    "#;

    let trace = run_headless(script_json, 10);
    insta::assert_yaml_snapshot!(trace);
}

#[test]
fn golden_script_comparison_operators() {
    let script_json = r#"
    {
        "script_schema_version": "1.0",
        "events": [
            {"type": "set_var", "key": "x", "value": 10},
            {"type": "jump_if", "cond": {"kind": "var_cmp", "key": "x", "op": "eq", "value": 10}, "target": "eq_ok"},
            {"type": "dialogue", "speaker": "Test", "text": "EQ failed"},
            {"type": "dialogue", "speaker": "Test", "text": "EQ passed"},
            {"type": "jump_if", "cond": {"kind": "var_cmp", "key": "x", "op": "ne", "value": 5}, "target": "ne_ok"},
            {"type": "dialogue", "speaker": "Test", "text": "NE failed"},
            {"type": "dialogue", "speaker": "Test", "text": "NE passed"},
            {"type": "jump_if", "cond": {"kind": "var_cmp", "key": "x", "op": "ge", "value": 10}, "target": "ge_ok"},
            {"type": "dialogue", "speaker": "Test", "text": "GE failed"},
            {"type": "dialogue", "speaker": "Test", "text": "All tests passed"}
        ],
        "labels": {
            "start": 0,
            "eq_ok": 3,
            "ne_ok": 6,
            "ge_ok": 9
        }
    }
    "#;

    let trace = run_headless(script_json, 15);
    insta::assert_yaml_snapshot!(trace);
}
