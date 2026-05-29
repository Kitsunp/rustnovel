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
fn authoring_apply_command_dry_run_reports_without_writing() {
    let (_tmp, script_path) = write_authoring_document();
    let command_path = script_path.with_file_name("command_dry_run.json");
    let output_path = script_path.with_file_name("dry_run_mutated.vnauthoring");
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
        .arg("--dry-run")
        .arg("--json")
        .output()
        .expect("run authoring apply-command dry-run");

    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("dry-run report json");
    assert_eq!(report["dry_run"], serde_json::json!(true));
    assert_eq!(report["wrote"], serde_json::json!(false));
    assert_eq!(
        report["outcome"]["delta"]["BackgroundFitChanged"]["after"],
        serde_json::json!("contain")
    );
    assert!(
        !output_path.exists(),
        "dry-run must not create mutated document"
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
fn cli_global_json_route_theme_and_layout_contracts() {
    let (_tmp, script_path) = write_script(ScriptRaw::new(
        vec![EventRaw::Scene(SceneUpdateRaw {
            background: Some("bg.png".to_string()),
            ..Default::default()
        })],
        BTreeMap::from([("start".to_string(), 0)]),
    ));

    let route = Command::new(env!("CARGO_BIN_EXE_vnengine"))
        .arg("--json")
        .arg("route-tree")
        .arg(script_path.as_os_str())
        .output()
        .expect("route tree");
    assert!(route.status.success());
    let route_json: serde_json::Value =
        serde_json::from_slice(&route.stdout).expect("route envelope");
    assert_eq!(route_json["ok"], true);
    assert_eq!(route_json["data"]["root"], 0);

    let theme_path = script_path.with_file_name("theme.json");
    fs::write(
        &theme_path,
        r##"{"id":"test","colors":{"dialogue.text":"#FFFFFF"},"typography":{},"spacing":{},"radii":{},"alpha":{},"action_text":{},"components":{"components":{}}}"##,
    )
    .expect("theme");
    let theme = Command::new(env!("CARGO_BIN_EXE_vnengine"))
        .arg("--json")
        .arg("theme")
        .arg("validate")
        .arg(theme_path.as_os_str())
        .output()
        .expect("theme validate");
    assert!(theme.status.success());
    let theme_json: serde_json::Value =
        serde_json::from_slice(&theme.stdout).expect("theme envelope");
    assert_eq!(theme_json["data"]["valid"], true);

    let display_path = script_path.with_file_name("display.json");
    fs::write(
        &display_path,
        r#"{"logical_size":[800.0,600.0],"physical_size":[1600,1200],"dpi":null,"ppi":null,"tpi":null,"scale_factor":2.0,"user_scale":1.0,"safe_area":{"left":0.0,"right":0.0,"top":0.0,"bottom":0.0},"window_mode":"windowed","orientation":"landscape"}"#,
    )
    .expect("display");
    let layout = Command::new(env!("CARGO_BIN_EXE_vnengine"))
        .arg("--json")
        .arg("layout")
        .arg("resolve")
        .arg("--display")
        .arg(display_path.as_os_str())
        .output()
        .expect("layout resolve");
    assert!(layout.status.success());
    let layout_json: serde_json::Value =
        serde_json::from_slice(&layout.stdout).expect("layout envelope");
    assert_eq!(layout_json["data"]["breakpoint"], "normal");
}

#[test]
fn cli_global_json_error_envelope_has_stable_exit_code() {
    let tmp = TempDir::new().expect("temp dir");
    let missing = tmp.path().join("missing.json");

    let output = Command::new(env!("CARGO_BIN_EXE_vnengine"))
        .arg("--json")
        .arg("validate")
        .arg(missing.as_os_str())
        .output()
        .expect("validate missing script");

    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(2));
    assert!(
        output.stderr.is_empty(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let envelope: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("error envelope");
    assert_eq!(envelope["ok"], false);
    assert_eq!(envelope["code"], "engine_error");
    assert!(envelope["error"]
        .as_str()
        .unwrap_or_default()
        .contains("load script"));
}

#[test]
fn migrate_script_command_requires_explicit_output_and_reports_json() {
    let tmp = TempDir::new().expect("temp dir");
    let input_path = tmp.path().join("legacy.json");
    let output_path = tmp.path().join("current.json");
    fs::write(
        &input_path,
        r#"{"script_schema_version":"0.9","events":[{"type":"dialogue","speaker":"Ava","text":"Legacy"}],"labels":{"start":0}}"#,
    )
    .expect("legacy script");

    let missing_output = Command::new(env!("CARGO_BIN_EXE_vnengine"))
        .arg("--json")
        .arg("migrate-script")
        .arg(input_path.as_os_str())
        .output()
        .expect("migrate missing output");
    assert!(!missing_output.status.success());

    let migrated = Command::new(env!("CARGO_BIN_EXE_vnengine"))
        .arg("--json")
        .arg("migrate-script")
        .arg(input_path.as_os_str())
        .arg("--output")
        .arg(output_path.as_os_str())
        .output()
        .expect("migrate script");
    assert!(
        migrated.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&migrated.stdout),
        String::from_utf8_lossy(&migrated.stderr)
    );
    let envelope: serde_json::Value =
        serde_json::from_slice(&migrated.stdout).expect("migrate envelope");
    assert_eq!(envelope["ok"], true);
    assert_eq!(envelope["data"]["changed"], true);
    let output_json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(output_path).expect("migrated json"))
            .expect("migrated value");
    assert_eq!(
        output_json["script_schema_version"],
        serde_json::json!(visual_novel_engine::SCRIPT_SCHEMA_VERSION)
    );
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
