use std::collections::BTreeMap;
use std::fs;
use std::process::Command;

use tempfile::TempDir;
use visual_novel_engine::{
    authoring::{
        composer::LayerOverride, AuthoringDocument, AuthoringPosition, NodeGraph, StoryNode,
    },
    runtime::{CharacterPlacementRaw, DialogueRaw, EventRaw, SceneUpdateRaw, ScriptRaw},
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
        report.contains("\"schema\": \"vnengine.authoring_validation_report.v2\""),
        "report={report}"
    );
    assert!(
        report.contains("\"fingerprint_schema_version\": \"vnengine.authoring.fingerprint.v2\""),
        "report={report}"
    );
    assert!(
        report.contains("\"story_semantic_sha256\""),
        "report={report}"
    );
}

#[test]
fn authoring_validate_command_fingerprints_authoring_document_metadata() {
    let (_tmp, script_path) = write_authoring_document();
    let baseline_output = script_path.with_file_name("authoring_report_before.json");
    let baseline = Command::new(env!("CARGO_BIN_EXE_vnengine"))
        .arg("authoring-validate")
        .arg(script_path.as_os_str())
        .arg("--output")
        .arg(baseline_output.as_os_str())
        .output()
        .expect("run baseline authoring validate");
    assert!(
        baseline.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&baseline.stdout),
        String::from_utf8_lossy(&baseline.stderr)
    );
    let baseline_report: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&baseline_output).expect("baseline report json"))
            .expect("baseline report value");

    let source = fs::read_to_string(&script_path).expect("authoring source");
    let mut document = AuthoringDocument::from_json(&source).expect("authoring doc");
    document.composer_layer_overrides.insert(
        "node:2:DialogueUi:0:graph_nodes_2_visual_dialogue".to_string(),
        LayerOverride {
            visible: false,
            locked: true,
        },
    );
    fs::write(&script_path, document.to_json().expect("authoring json")).expect("write doc");
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
    let changed_report: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(output_path).expect("report json"))
            .expect("report value");
    let before = &baseline_report["fingerprints"];
    let after = &changed_report["fingerprints"];
    assert_eq!(
        before["story_semantic_sha256"], after["story_semantic_sha256"],
        "composer metadata must not stale semantic reports"
    );
    assert_eq!(
        after["semantic_sha256"], after["story_semantic_sha256"],
        "legacy semantic alias should still point at semantic hash"
    );
    assert_ne!(
        before["layout_sha256"], after["layout_sha256"],
        "composer layer overrides must change the layout hash"
    );
    assert_ne!(
        before["full_document_sha256"], after["full_document_sha256"],
        "composer layer overrides must change the document hash"
    );
}

#[test]
fn authoring_apply_command_serializes_core_document_outcome() {
    let (_tmp, script_path) = write_authoring_document();
    let command_path = script_path.with_file_name("command.json");
    let output_path = script_path.with_file_name("mutated.vnauthoring");
    fs::write(
        &command_path,
        r#"{"command":"set_background_fit_override","node_id":0,"fit":"contain"}"#,
    )
    .expect("command json");

    let output = Command::new(env!("CARGO_BIN_EXE_vnengine"))
        .arg("authoring")
        .arg("apply-command")
        .arg(script_path.as_os_str())
        .arg("--command")
        .arg(command_path.as_os_str())
        .arg("--output")
        .arg(output_path.as_os_str())
        .arg("--json")
        .output()
        .expect("run authoring apply-command");

    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let outcome: serde_json::Value = serde_json::from_slice(&output.stdout).expect("outcome json");
    assert_eq!(
        outcome["delta"]["BackgroundFitChanged"]["after"],
        serde_json::json!("contain")
    );
    assert_eq!(
        outcome["operation"]["operation_kind"],
        serde_json::json!("field_edited")
    );
    assert_eq!(
        outcome["operation"]["operation_id"],
        outcome["verification"]["operation_id"]
    );

    let document = AuthoringDocument::from_json(
        &fs::read_to_string(output_path).expect("mutated authoring document"),
    )
    .expect("parse mutated authoring document");
    assert_eq!(
        document.composer_background_fit_overrides.get("0"),
        Some(&visual_novel_engine::authoring::composer::BackgroundFit::Contain)
    );
    assert_eq!(document.operation_log.len(), 1);
    assert_eq!(document.verification_runs.len(), 1);
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
    assert!(
        report.contains("\"schema\": \"vnengine.diagnostic_envelope.v2\""),
        "report={report}"
    );
    assert!(report.contains("\"target\""), "report={report}");
    assert!(report.contains("\"field_path\""), "report={report}");
    assert!(report.contains("\"evidence_trace\""), "report={report}");
    assert!(
        report.contains("docs/diagnostics/authoring.md#val-character-name-empty"),
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
        .arg("--format")
        .arg("yaml")
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
fn cli_trace_json_contract() {
    let (_tmp, path) = write_authoring_document();
    let json_path = path.with_file_name("trace.json");
    let yaml_path = path.with_file_name("trace.yaml");
    let mismatched_path = path.with_file_name("trace.json");

    let json = Command::new(env!("CARGO_BIN_EXE_vnengine"))
        .arg("trace")
        .arg(path.as_os_str())
        .arg("--format")
        .arg("json")
        .arg("--output")
        .arg(json_path.as_os_str())
        .output()
        .expect("run json trace");
    assert!(
        json.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&json.stdout),
        String::from_utf8_lossy(&json.stderr)
    );
    let parsed_json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&json_path).expect("json trace"))
            .expect("trace must be JSON");
    assert_eq!(parsed_json["trace_format_version"], 1);

    let yaml = Command::new(env!("CARGO_BIN_EXE_vnengine"))
        .arg("trace")
        .arg(path.as_os_str())
        .arg("--format")
        .arg("yaml")
        .arg("--output")
        .arg(yaml_path.as_os_str())
        .output()
        .expect("run yaml trace");
    assert!(
        yaml.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&yaml.stdout),
        String::from_utf8_lossy(&yaml.stderr)
    );
    let parsed_yaml: serde_json::Value =
        serde_norway::from_str(&fs::read_to_string(&yaml_path).expect("yaml trace"))
            .expect("trace must be YAML");
    assert_eq!(parsed_yaml["trace_format_version"], 1);

    let mismatch = Command::new(env!("CARGO_BIN_EXE_vnengine"))
        .arg("trace")
        .arg(path.as_os_str())
        .arg("--format")
        .arg("yaml")
        .arg("--output")
        .arg(mismatched_path.as_os_str())
        .output()
        .expect("run mismatched trace");
    assert!(
        !mismatch.status.success(),
        "YAML output to .json must not succeed silently"
    );
    assert!(String::from_utf8_lossy(&mismatch.stderr).contains("incompatible"));
}

#[test]
fn example_entrypoint_authoring_validate() {
    let repo_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let script = repo_root.join("examples/scripts/demo_story.json");
    let project_root = repo_root.join("examples/scripts");
    let tmp = TempDir::new().expect("temp dir");
    let report_path = tmp.path().join("example-authoring-report.json");

    let runtime = Command::new(env!("CARGO_BIN_EXE_vnengine"))
        .arg("validate")
        .arg(script.as_os_str())
        .output()
        .expect("run runtime validate");
    assert!(
        runtime.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&runtime.stdout),
        String::from_utf8_lossy(&runtime.stderr)
    );

    let authoring = Command::new(env!("CARGO_BIN_EXE_vnengine"))
        .arg("authoring")
        .arg("validate")
        .arg(script.as_os_str())
        .arg("--project-root")
        .arg(project_root.as_os_str())
        .arg("--output")
        .arg(report_path.as_os_str())
        .output()
        .expect("run authoring validate");
    assert!(
        authoring.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&authoring.stdout),
        String::from_utf8_lossy(&authoring.stderr)
    );
    let report: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(report_path).expect("report json"))
            .expect("authoring report json");
    assert_eq!(report["error_count"], 0);
}

#[test]
fn contract_fixture_cli_authoring_validate_reports_expected_asset_error() {
    let repo_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let project_root = repo_root.join("tests/fixtures/authoring_contract");
    let fixture = project_root.join("contract_fixture.authoring.json");
    let tmp = TempDir::new().expect("temp dir");
    let report_path = tmp.path().join("contract-authoring-report.json");

    let output = Command::new(env!("CARGO_BIN_EXE_vnengine"))
        .arg("authoring")
        .arg("validate")
        .arg(fixture.as_os_str())
        .arg("--project-root")
        .arg(project_root.as_os_str())
        .arg("--output")
        .arg(report_path.as_os_str())
        .output()
        .expect("run authoring validate");
    assert!(
        !output.status.success(),
        "fixture intentionally contains missing assets\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(report_path).expect("report json"))
            .expect("authoring report json");
    assert!(report["error_count"].as_u64().unwrap_or_default() >= 1);
    assert!(report["issues"]
        .as_array()
        .expect("issues")
        .iter()
        .any(|issue| issue["code"] == "VAL_ASSET_NOT_FOUND"));
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
