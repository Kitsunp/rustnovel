use std::collections::BTreeMap;
use std::fs;
use std::process::Command;

use tempfile::TempDir;
use visual_novel_engine::{
    authoring::{AuthoringDocument, AuthoringPosition, NodeGraph, StoryNode},
    runtime::{EventRaw, SceneUpdateRaw, ScriptRaw},
};

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
