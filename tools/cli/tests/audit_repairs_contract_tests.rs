use serde_json::{json, Value};
use std::fs;
use std::process::Command;

#[test]
fn invalid_theme_fails_in_text_and_json_modes() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("theme.json");
    let mut theme = visual_novel_engine::UiTheme::default();
    theme.colors.insert("broken".into(), "invalid".into());
    fs::write(&path, serde_json::to_vec(&theme).unwrap()).unwrap();
    for json_mode in [false, true] {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_vnengine"));
        if json_mode {
            cmd.arg("--json");
        }
        let out = cmd.args(["theme", "validate"]).arg(&path).output().unwrap();
        assert!(!out.status.success());
        if json_mode {
            let envelope: Value = serde_json::from_slice(&out.stdout).unwrap();
            assert_eq!(envelope["ok"], false);
            assert!(envelope["error"].as_str().unwrap().contains("broken"));
        } else {
            assert!(String::from_utf8_lossy(&out.stderr).contains("theme validation failed"));
        }
    }
}

fn script_fixture(path: &std::path::Path, looping: bool) {
    fs::write(
        path,
        serde_json::to_vec(&json!({
            "script_schema_version": visual_novel_engine::SCRIPT_SCHEMA_VERSION,
            "labels": {"start": 0}, "start": "start",
            "events": if looping { json!([{"type":"jump", "target":"start"}]) }
                else { json!([{"type":"dialogue", "speaker":"Narrator", "text":"Hello"}]) }
        }))
        .unwrap(),
    )
    .unwrap();
}

#[test]
fn authoring_validate_json_has_exactly_one_envelope() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("script.json");
    script_fixture(&path, false);
    let out = Command::new(env!("CARGO_BIN_EXE_vnengine"))
        .args(["--json", "authoring-validate"])
        .arg(path)
        .output()
        .unwrap();
    let envelope: Value = serde_json::from_slice(&out.stdout).expect("one JSON document");
    assert!(envelope["ok"].is_boolean());
    assert_eq!(envelope["ok"], out.status.success());
}

#[test]
fn route_tree_and_read_model_produce_output_without_json_flag() {
    let dir = tempfile::tempdir().unwrap();
    let script = dir.path().join("script.json");
    script_fixture(&script, false);
    for command in ["route-tree", "read-model"] {
        let out = Command::new(env!("CARGO_BIN_EXE_vnengine"))
            .arg(command)
            .arg(&script)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        serde_json::from_slice::<Value>(&out.stdout).expect("visible structured output");
    }
}

#[test]
fn trace_marks_step_limit_as_partial_and_eof_as_complete() {
    let dir = tempfile::tempdir().unwrap();
    let script = dir.path().join("script.json");
    let trace = dir.path().join("trace.json");
    for (looping, steps, partial) in [(true, 0, true), (true, 10, true), (false, 1, false)] {
        script_fixture(&script, looping);
        let out = Command::new(env!("CARGO_BIN_EXE_vnengine"))
            .arg("trace")
            .arg(&script)
            .arg("--steps")
            .arg(steps.to_string())
            .arg("--output")
            .arg(&trace)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        let report: Value = serde_json::from_slice(&fs::read(&trace).unwrap()).unwrap();
        assert_eq!(report["partial"], partial);
        assert_eq!(report["stopped_reason"].is_string(), partial);
    }
}
