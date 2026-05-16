use std::collections::BTreeMap;
use std::fs;
use std::process::Command;

use tempfile::TempDir;
use visual_novel_engine::{
    authoring::{
        composer::LayerOverride, AuthoringDocument, AuthoringPosition, NodeGraph, StoryNode,
    },
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

#[test]
fn authoring_namespace_fragments_report_and_repro_are_operational() {
    let (tmp, path) = write_authoring_document();
    let fragment_path = tmp.path().join("fragmented.vnauthoring");
    let report_path = tmp.path().join("report.json");
    let sarif_path = tmp.path().join("report.sarif.json");
    let repro_path = tmp.path().join("repro.json");

    let create = Command::new(env!("CARGO_BIN_EXE_vnengine"))
        .args(["authoring", "fragments", "create"])
        .arg(path.as_os_str())
        .args(["--id", "intro", "--title", "Intro", "--nodes", "1"])
        .arg("--output")
        .arg(fragment_path.as_os_str())
        .output()
        .expect("create fragment");
    assert!(
        create.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&create.stdout),
        String::from_utf8_lossy(&create.stderr)
    );

    let list = Command::new(env!("CARGO_BIN_EXE_vnengine"))
        .args(["authoring", "fragments", "list"])
        .arg(fragment_path.as_os_str())
        .output()
        .expect("list fragments");
    assert!(list.status.success());
    assert!(
        String::from_utf8_lossy(&list.stdout).contains("\"fragment_id\": \"intro\""),
        "stdout={}",
        String::from_utf8_lossy(&list.stdout)
    );
    let fragmented_document = AuthoringDocument::from_json(
        &fs::read_to_string(&fragment_path).expect("fragmented authoring"),
    )
    .expect("fragmented document");
    assert_eq!(fragmented_document.operation_log.len(), 1);
    assert_eq!(fragmented_document.verification_runs.len(), 1);
    assert_eq!(
        fragmented_document.operation_log[0].operation_kind,
        "fragment_created"
    );

    let validate = Command::new(env!("CARGO_BIN_EXE_vnengine"))
        .args(["authoring", "validate"])
        .arg(fragment_path.as_os_str())
        .arg("--output")
        .arg(report_path.as_os_str())
        .output()
        .expect("validate authoring namespace");
    assert!(
        validate.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&validate.stdout),
        String::from_utf8_lossy(&validate.stderr)
    );
    let report = fs::read_to_string(&report_path).expect("report");
    assert!(report.contains("\"schema\": \"vnengine.authoring_validation_report.v2\""));

    let sarif = Command::new(env!("CARGO_BIN_EXE_vnengine"))
        .args(["authoring", "report", "sarif"])
        .arg(report_path.as_os_str())
        .arg("--output")
        .arg(sarif_path.as_os_str())
        .output()
        .expect("sarif");
    assert!(sarif.status.success());
    assert!(fs::read_to_string(&sarif_path)
        .expect("sarif json")
        .contains("\"version\": \"2.1.0\""));

    let bad_script = tmp.path().join("bad.json");
    let bad_report_path = tmp.path().join("bad_report.json");
    fs::write(
        &bad_script,
        ScriptRaw::new(
            vec![EventRaw::Scene(SceneUpdateRaw {
                background: Some("bg/missing.png".to_string()),
                music: None,
                characters: Vec::new(),
            })],
            BTreeMap::from([("start".to_string(), 0)]),
        )
        .to_json()
        .expect("bad script json"),
    )
    .expect("bad script");
    let bad_validate = Command::new(env!("CARGO_BIN_EXE_vnengine"))
        .args(["authoring", "validate"])
        .arg(bad_script.as_os_str())
        .arg("--output")
        .arg(bad_report_path.as_os_str())
        .output()
        .expect("validate bad report");
    assert!(!bad_validate.status.success());
    let bad_report = fs::read_to_string(&bad_report_path).expect("bad report");
    assert!(bad_report.contains("\"typed_message_args\""));
    let parsed: serde_json::Value = serde_json::from_str(&bad_report).expect("bad report json");
    let diagnostic_id = parsed["issues"]
        .as_array()
        .and_then(|issues| issues.first())
        .and_then(|issue| issue["diagnostic_id"].as_str())
        .expect("diagnostic id")
        .to_string();

    let explain = Command::new(env!("CARGO_BIN_EXE_vnengine"))
        .args(["authoring", "explain"])
        .arg(bad_script.as_os_str())
        .arg("--report")
        .arg(bad_report_path.as_os_str())
        .arg("--diagnostic-id")
        .arg(&diagnostic_id)
        .output()
        .expect("explain");
    assert!(explain.status.success());
    assert!(
        String::from_utf8_lossy(&explain.stdout).contains("\"evidence_trace\""),
        "stdout={}",
        String::from_utf8_lossy(&explain.stdout)
    );

    let repro = Command::new(env!("CARGO_BIN_EXE_vnengine"))
        .args(["authoring", "repro", "from-diagnostic"])
        .arg(bad_script.as_os_str())
        .arg("--report")
        .arg(bad_report_path.as_os_str())
        .arg("--diagnostic-id")
        .arg(diagnostic_id)
        .arg("--output")
        .arg(repro_path.as_os_str())
        .output()
        .expect("repro from diagnostic");
    assert!(repro.status.success());
    assert!(fs::read_to_string(repro_path)
        .expect("repro")
        .contains("\"diagnostic_id\""));
}

#[test]
fn authoring_fragment_validate_fails_on_fragment_errors_and_sarif_keeps_error_level() {
    let tmp = TempDir::new().expect("temp dir");
    let path = tmp.path().join("broken_fragment.vnauthoring");
    let report_path = tmp.path().join("broken_report.json");
    let sarif_path = tmp.path().join("broken_report.sarif.json");
    let mut graph = NodeGraph::new();
    let start = graph.add_node(StoryNode::Start, AuthoringPosition::new(0.0, 0.0));
    let call = graph.add_node(
        StoryNode::SubgraphCall {
            fragment_id: "missing_fragment".to_string(),
            entry_port: None,
            exit_port: None,
        },
        AuthoringPosition::new(0.0, 90.0),
    );
    graph.connect(start, call);
    fs::write(
        &path,
        AuthoringDocument::new(graph)
            .to_json()
            .expect("authoring json"),
    )
    .expect("authoring");

    let fragment_validate = Command::new(env!("CARGO_BIN_EXE_vnengine"))
        .args(["authoring", "fragments", "validate"])
        .arg(path.as_os_str())
        .output()
        .expect("fragment validate");
    assert!(
        !fragment_validate.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&fragment_validate.stdout),
        String::from_utf8_lossy(&fragment_validate.stderr)
    );

    let validate = Command::new(env!("CARGO_BIN_EXE_vnengine"))
        .args(["authoring", "validate"])
        .arg(path.as_os_str())
        .arg("--output")
        .arg(report_path.as_os_str())
        .output()
        .expect("validate report");
    assert!(!validate.status.success());

    let sarif = Command::new(env!("CARGO_BIN_EXE_vnengine"))
        .args(["authoring", "report", "sarif"])
        .arg(report_path.as_os_str())
        .arg("--output")
        .arg(sarif_path.as_os_str())
        .output()
        .expect("sarif");
    assert!(sarif.status.success());
    let sarif_json = fs::read_to_string(sarif_path).expect("sarif json");
    assert!(
        sarif_json.contains("\"level\": \"error\""),
        "sarif={sarif_json}"
    );
}
