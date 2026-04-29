use std::collections::BTreeMap;
use std::fs;
use std::process::Command;

use tempfile::TempDir;
use visual_novel_engine::{
    authoring::{AuthoringDocument, AuthoringPosition, NodeGraph, StoryNode},
    CharacterPlacementRaw, DialogueRaw, EventRaw, SceneUpdateRaw, ScriptRaw,
};

fn write_script(script: ScriptRaw) -> (TempDir, std::path::PathBuf) {
    let tmp = TempDir::new().expect("temp dir");
    let path = tmp.path().join("main.json");
    fs::write(&path, script.to_json().expect("script json")).expect("script");
    (tmp, path)
}

fn write_authoring_document() -> (TempDir, std::path::PathBuf) {
    let tmp = TempDir::new().expect("temp dir");
    let path = tmp.path().join("main.vnauthoring");
    let mut graph = NodeGraph::new();
    let start = graph.add_node(StoryNode::Start, AuthoringPosition::new(0.0, 0.0));
    let line = graph.add_node(
        StoryNode::Dialogue {
            speaker: "Narrator".to_string(),
            text: "Ready".to_string(),
        },
        AuthoringPosition::new(0.0, 90.0),
    );
    let end = graph.add_node(StoryNode::End, AuthoringPosition::new(0.0, 180.0));
    graph.connect(start, line);
    graph.connect(line, end);
    let document = AuthoringDocument::new(graph);
    fs::write(&path, document.to_json().expect("authoring json")).expect("authoring");
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

#[test]
fn compile_and_trace_accept_authoring_document() {
    let (_tmp, path) = write_authoring_document();
    let compiled_path = path.with_extension("vnc");
    let trace_path = path.with_file_name("trace.yml");

    let compile = Command::new(env!("CARGO_BIN_EXE_vnengine"))
        .arg("compile")
        .arg(path.as_os_str())
        .arg("--output")
        .arg(compiled_path.as_os_str())
        .output()
        .expect("run compile");
    assert!(
        compile.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr)
    );
    assert!(compiled_path.is_file());

    let trace = Command::new(env!("CARGO_BIN_EXE_vnengine"))
        .arg("trace")
        .arg(path.as_os_str())
        .arg("--output")
        .arg(trace_path.as_os_str())
        .output()
        .expect("run trace");
    assert!(
        trace.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&trace.stdout),
        String::from_utf8_lossy(&trace.stderr)
    );
    assert!(fs::read_to_string(trace_path)
        .expect("trace")
        .contains("trace_format_version"));
}

#[test]
fn authoring_validate_project_root_argument_controls_asset_resolution() {
    let tmp = TempDir::new().expect("temp dir");
    let project_root = tmp.path().join("project");
    fs::create_dir_all(project_root.join("assets/bg")).expect("assets dir");
    fs::write(project_root.join("assets/bg/room.png"), b"png").expect("asset");
    let script_path = tmp.path().join("main.json");
    fs::write(
        &script_path,
        ScriptRaw::new(
            vec![EventRaw::Scene(SceneUpdateRaw {
                background: Some("assets/bg/room.png".to_string()),
                music: None,
                characters: Vec::new(),
            })],
            BTreeMap::from([("start".to_string(), 0)]),
        )
        .to_json()
        .expect("script json"),
    )
    .expect("script");

    let output = Command::new(env!("CARGO_BIN_EXE_vnengine"))
        .arg("authoring-validate")
        .arg(script_path.as_os_str())
        .arg("--project-root")
        .arg(project_root.as_os_str())
        .output()
        .expect("run authoring validate");
    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
