use std::collections::BTreeMap;
use std::fs;
use std::process::Command;

use tempfile::TempDir;
use visual_novel_engine::{
    CharacterPlacementRaw, DialogueRaw, EventRaw, SceneUpdateRaw, ScriptRaw,
};

fn write_script(script: ScriptRaw) -> (TempDir, std::path::PathBuf) {
    let tmp = TempDir::new().expect("temp dir");
    let path = tmp.path().join("main.json");
    fs::write(&path, script.to_json().expect("script json")).expect("script");
    (tmp, path)
}

#[test]
fn authoring_validate_command_writes_clean_report() {
    let (_tmp, script_path) = write_script(ScriptRaw::new(
        vec![EventRaw::Dialogue(DialogueRaw {
            speaker: "Narrator".to_string(),
            text: "Ready".to_string(),
        })],
        BTreeMap::from([("start".to_string(), 0)]),
    ));
    let output_path = script_path.with_file_name("authoring_report.json");

    let output = Command::new(env!("CARGO_BIN_EXE_vnengine"))
        .arg("authoring-validate")
        .arg(script_path.as_os_str())
        .arg("--output")
        .arg(output_path.as_os_str())
        .output()
        .expect("run authoring validate command");

    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let report = fs::read_to_string(output_path).expect("report json");
    assert!(report.contains("\"issue_count\": 0"), "report={report}");
    assert!(
        report.contains("\"fingerprint_schema_version\": \"vnengine.authoring.fingerprint.v1\""),
        "report={report}"
    );
}

#[test]
fn authoring_validate_command_reports_graph_errors_before_failing() {
    let (_tmp, script_path) = write_script(ScriptRaw::new(
        vec![EventRaw::Scene(SceneUpdateRaw {
            background: None,
            music: None,
            characters: vec![CharacterPlacementRaw {
                name: String::new(),
                ..Default::default()
            }],
        })],
        BTreeMap::from([("start".to_string(), 0)]),
    ));
    let output_path = script_path.with_file_name("authoring_report.json");

    let output = Command::new(env!("CARGO_BIN_EXE_vnengine"))
        .arg("authoring-validate")
        .arg(script_path.as_os_str())
        .arg("--output")
        .arg(output_path.as_os_str())
        .output()
        .expect("run authoring validate command");

    assert!(
        !output.status.success(),
        "authoring errors should make command fail"
    );
    let report = fs::read_to_string(output_path).expect("report json");
    assert!(
        report.contains("VAL_CHARACTER_NAME_EMPTY"),
        "report={report}"
    );
}

#[test]
fn authoring_validate_command_checks_assets_against_script_root() {
    let (_tmp, script_path) = write_script(ScriptRaw::new(
        vec![EventRaw::Scene(SceneUpdateRaw {
            background: Some("bg/missing.png".to_string()),
            music: None,
            characters: Vec::new(),
        })],
        BTreeMap::from([("start".to_string(), 0)]),
    ));
    let output_path = script_path.with_file_name("authoring_report.json");

    let output = Command::new(env!("CARGO_BIN_EXE_vnengine"))
        .arg("authoring-validate")
        .arg(script_path.as_os_str())
        .arg("--output")
        .arg(output_path.as_os_str())
        .output()
        .expect("run authoring validate command");

    assert!(
        !output.status.success(),
        "missing asset should make command fail"
    );
    let report = fs::read_to_string(output_path).expect("report json");
    assert!(report.contains("VAL_ASSET_NOT_FOUND"), "report={report}");
    assert!(report.contains("bg/missing.png"), "report={report}");
}
