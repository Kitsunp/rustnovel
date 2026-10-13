use serde_json::{json, Value};
use std::{
    fs,
    path::Path,
    process::{Command, Output},
};
use tempfile::tempdir;
use visual_novel_engine::{authoring::*, runtime::CharacterPlacementRaw};

fn run(project: &Path, commands: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_vnengine"))
        .args(["authoring", "apply-command"])
        .arg(project)
        .arg("--command")
        .arg(commands)
        .args(args)
        .output()
        .unwrap()
}

fn assert_success(output: &Output) -> Value {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

#[test]
fn batch_builds_story_moves_character_and_matches_core_session_with_undo_redo() {
    let temp = tempdir().unwrap();
    let project = temp.path().join("story.vnauthoring");
    let commands_path = temp.path().join("commands.json");
    let document = AuthoringDocument::new(NodeGraph::new());
    fs::write(&project, document.to_json().unwrap()).unwrap();
    let scene = StoryNode::Scene {
        profile: None,
        background: Some("bg/room.png".into()),
        music: None,
        characters: vec![CharacterPlacementRaw {
            name: "Ava".into(),
            expression: Some("characters/ava.png".into()),
            x: Some(10),
            y: Some(20),
            scale: Some(1.0),
            ..Default::default()
        }],
    };
    let mut graph = NodeGraph::new();
    graph.add_node(StoryNode::Start, AuthoringPosition::new(0.0, 0.0));
    let scene_id = graph.add_node(scene.clone(), AuthoringPosition::new(0.0, 90.0));
    let layer = composer::list_layered_objects(&graph, Some(scene_id))
        .into_iter()
        .find(|object| object.character_name.as_deref() == Some("Ava"))
        .unwrap()
        .object_id;
    let mut commands: Vec<_> = [
        StoryNode::Start,
        scene,
        StoryNode::Dialogue {
            speaker: "Ava".into(),
            text: "Before".into(),
        },
        StoryNode::End,
    ]
    .into_iter()
    .enumerate()
    .map(|(index, node)| {
        AuthoringDocumentCommand::Graph(AuthoringCommand::CreateNode {
            node_id: index as u32,
            node,
            position: AuthoringPosition::new(0.0, index as f32 * 90.0),
        })
    })
    .collect();
    for from in 0..3 {
        commands.push(AuthoringDocumentCommand::Graph(AuthoringCommand::Connect {
            from,
            from_port: 0,
            to: from + 1,
        }));
    }
    commands.extend([
        AuthoringDocumentCommand::Graph(AuthoringCommand::MoveLayer {
            object_id: layer.clone(),
            x: 320,
            y: 240,
            scale: Some(1.25),
        }),
        AuthoringDocumentCommand::Graph(AuthoringCommand::EditDialogue {
            node_id: 2,
            speaker: "Ava".into(),
            text: "Written from code".into(),
        }),
        AuthoringDocumentCommand::UndoLast,
        AuthoringDocumentCommand::RedoLast,
        AuthoringDocumentCommand::SetLayerLocked {
            object_id: layer.clone(),
            locked: true,
        },
        AuthoringDocumentCommand::SetLayerVisible {
            object_id: layer,
            visible: false,
        },
        AuthoringDocumentCommand::SetBackgroundFitOverride {
            node_id: 1,
            fit: composer::BackgroundFit::Contain,
        },
    ]);
    fs::write(&commands_path, serde_json::to_vec(&commands).unwrap()).unwrap();
    let expected = apply_authoring_document_commands_headless(document, commands).unwrap();
    let report = assert_success(&run(&project, &commands_path, &["--in-place", "--json"]));
    assert_eq!(report["wrote"], true);
    assert_eq!(report["dry_run"], false);
    let actual = AuthoringDocument::from_json(&fs::read_to_string(&project).unwrap()).unwrap();
    assert!(actual.graph.has_same_content(&expected.document.graph));
    assert_eq!(
        authoring_document_layout_sha256(&actual),
        authoring_document_layout_sha256(&expected.document)
    );
    assert_eq!(actual.operation_log.len(), expected.outcomes.len());
    for (reported, expected) in report["outcomes"]
        .as_array()
        .unwrap()
        .iter()
        .zip(&expected.outcomes)
    {
        let fingerprint = serde_json::to_value(&expected.after_fingerprint).unwrap();
        for field in ["story_semantic_sha256", "layout_sha256", "assets_sha256"] {
            assert_eq!(reported["after_fingerprint"][field], fingerprint[field]);
        }
        assert_eq!(
            reported["verification"]["diagnostic_ids"],
            json!(expected.verification.diagnostic_ids)
        );
        assert_eq!(
            reported["verification"]["resolved_diagnostic_ids"],
            json!(expected.verification.resolved_diagnostic_ids)
        );
        assert_eq!(
            reported["verification"]["introduced_diagnostic_ids"],
            json!(expected.verification.introduced_diagnostic_ids)
        );
    }
    assert_eq!(
        report["outcomes"].as_array().unwrap().len(),
        expected.outcomes.len()
    );
    assert!(
        matches!(actual.graph.get_node(2), Some(StoryNode::Dialogue {text,..}) if text == "Written from code")
    );
    let Some(StoryNode::Scene { characters, .. }) = actual.graph.get_node(1) else {
        panic!("scene missing")
    };
    assert_eq!(
        (characters[0].x, characters[0].y, characters[0].scale),
        (Some(320), Some(240), Some(1.25))
    );
}

#[test]
fn graph_command_object_keeps_single_json_output_contract() {
    let temp = tempdir().unwrap();
    let project = temp.path().join("story.vnauthoring");
    let commands = temp.path().join("command.json");
    fs::write(
        &project,
        AuthoringDocument::new(NodeGraph::new()).to_json().unwrap(),
    )
    .unwrap();
    fs::write(&commands, r#"{"command":"graph","operation":"create_node","node_id":0,"node":"Start","position":{"x":0,"y":0}}"#).unwrap();
    let report = assert_success(&run(&project, &commands, &["--in-place", "--json"]));
    assert!(report.get("operation").is_some());
    assert!(report.get("outcomes").is_none());
    let actual = AuthoringDocument::from_json(&fs::read_to_string(project).unwrap()).unwrap();
    assert!(matches!(actual.graph.get_node(0), Some(StoryNode::Start)));
}

#[test]
fn failed_or_malformed_batches_leave_input_and_existing_output_untouched() {
    let temp = tempdir().unwrap();
    let project = temp.path().join("story.vnauthoring");
    let destination = temp.path().join("output.vnauthoring");
    let commands = temp.path().join("commands.json");
    fs::write(&destination, b"existing output").unwrap();
    let first = json!({"command":"set_background_fit_override","node_id":0,"fit":"contain"});
    // Use a valid node so the first command succeeds before the second fails.
    let mut graph = NodeGraph::new();
    graph.add_node(StoryNode::Start, AuthoringPosition::new(0.0, 0.0));
    let original = AuthoringDocument::new(graph).to_json().unwrap();
    fs::write(&project, &original).unwrap();
    for (input, error) in [
        (
            json!([first.clone(), {"command":"graph","operation":"remove_node","node_id":999}]),
            "command 2 failed",
        ),
        (
            json!([first, {"command":"graph","operation":"connect","from":0}]),
            "parse Core command 2",
        ),
        (json!([]), "empty"),
    ] {
        fs::write(&commands, serde_json::to_vec(&input).unwrap()).unwrap();
        for args in [
            vec!["--in-place"],
            vec!["--output", destination.to_str().unwrap()],
        ] {
            let result = run(&project, &commands, &args);
            assert!(!result.status.success());
            assert!(
                String::from_utf8_lossy(&result.stderr).contains(error),
                "{}",
                String::from_utf8_lossy(&result.stderr)
            );
            assert_eq!(fs::read_to_string(&project).unwrap(), original);
            assert_eq!(fs::read(&destination).unwrap(), b"existing output");
        }
    }
}

#[test]
fn dry_run_batch_reports_every_outcome_without_writing() {
    let temp = tempdir().unwrap();
    let project = temp.path().join("story.vnauthoring");
    let commands = temp.path().join("commands.json");
    let original = AuthoringDocument::new(NodeGraph::new()).to_json().unwrap();
    fs::write(&project, &original).unwrap();
    fs::write(&commands,r#"[{"command":"graph","operation":"create_node","node_id":0,"node":"Start","position":{"x":0,"y":0}},{"command":"undo_last"},{"command":"redo_last"}]"#).unwrap();
    let report = assert_success(&run(
        &project,
        &commands,
        &["--dry-run", "--in-place", "--json"],
    ));
    assert_eq!(report["dry_run"], true);
    assert_eq!(report["wrote"], false);
    assert_eq!(report["outcomes"].as_array().unwrap().len(), 3);
    assert_eq!(fs::read_to_string(&project).unwrap(), original);
}
