use std::collections::BTreeMap;

use visual_novel_engine::authoring::{
    parse_authoring_document_or_script, parse_runtime_script_from_entry,
    validate_authoring_graph_with_project_root, AuthoringDocument, AuthoringPosition, LintCode,
    LintSeverity, NodeGraph, SceneProfile, StoryNode,
};
use visual_novel_engine::{
    AssetId, CharacterPlacementRaw, CondRaw, DialogueRaw, Engine, EventRaw, ReproCase,
    ResourceLimiter, SceneTransitionRaw, ScriptRaw, SecurityPolicy,
};

fn pos(x: f32, y: f32) -> AuthoringPosition {
    AuthoringPosition::new(x, y)
}

fn connected_dialogue_graph() -> NodeGraph {
    let mut graph = NodeGraph::new();
    let start = graph.add_node(StoryNode::Start, pos(0.0, 0.0));
    let line = graph.add_node(
        StoryNode::Dialogue {
            speaker: "Narrator".to_string(),
            text: "Hello".to_string(),
        },
        pos(0.0, 90.0),
    );
    let end = graph.add_node(StoryNode::End, pos(0.0, 180.0));
    graph.connect(start, line);
    graph.connect(line, end);
    graph
}

#[test]
fn runtime_entry_loader_accepts_script_and_authoring_document() {
    let script = ScriptRaw::new(
        vec![EventRaw::Dialogue(DialogueRaw {
            speaker: "Narrator".to_string(),
            text: "Legacy".to_string(),
        })],
        BTreeMap::from([("start".to_string(), 0)]),
    );
    let script_json = script.to_json().expect("script json");
    let loaded_script =
        parse_runtime_script_from_entry(&script_json).expect("legacy script should load");
    assert_eq!(loaded_script.events.len(), 1);

    let document = AuthoringDocument::new(connected_dialogue_graph());
    let document_json = document.to_json().expect("authoring json");
    let loaded_authoring =
        parse_runtime_script_from_entry(&document_json).expect("authoring document should export");
    assert_eq!(loaded_authoring.events.len(), 1);
}

#[test]
fn authoring_document_future_schema_is_explicitly_rejected() {
    let document = AuthoringDocument::new(connected_dialogue_graph());
    let mut value = serde_json::to_value(document).expect("authoring value");
    value["authoring_schema_version"] = serde_json::Value::String("99.0".to_string());
    let err = parse_authoring_document_or_script(&value.to_string())
        .expect_err("future authoring schema must fail");
    assert!(format!("{err}").contains("unsupported authoring_schema_version"));
}

#[test]
fn self_loop_choice_and_jump_roundtrip_without_structural_rejection() {
    let mut choice_graph = NodeGraph::new();
    let start = choice_graph.add_node(StoryNode::Start, pos(-120.0, 0.0));
    let choice = choice_graph.add_node(
        StoryNode::Choice {
            prompt: "Again?".to_string(),
            options: vec!["Loop".to_string()],
        },
        pos(0.0, 0.0),
    );
    choice_graph.connect(start, choice);
    choice_graph.connect_port(choice, 0, choice);
    assert!(choice_graph
        .connections()
        .any(|conn| conn.from == choice && conn.to == choice));
    let choice_issues = visual_novel_engine::authoring::validate_authoring_graph(&choice_graph);
    assert!(choice_issues
        .iter()
        .any(|issue| issue.code == LintCode::PotentialLoop));

    let script = ScriptRaw::new(
        vec![EventRaw::Jump {
            target: "start".to_string(),
        }],
        BTreeMap::from([("start".to_string(), 0)]),
    );
    let jump_graph = NodeGraph::from_script(&script);
    let jump_id = jump_graph
        .nodes()
        .find_map(|(id, node, _)| matches!(node, StoryNode::Jump { .. }).then_some(*id))
        .expect("jump node");
    assert!(jump_graph
        .connections()
        .any(|conn| conn.from == jump_id && conn.to == jump_id));
}

#[test]
fn missing_target_is_preserved_and_strict_export_fails() {
    let script = ScriptRaw::new(
        vec![EventRaw::Jump {
            target: "missing_label".to_string(),
        }],
        BTreeMap::from([("start".to_string(), 0)]),
    );
    let graph = NodeGraph::from_script(&script);
    let jump_id = graph
        .nodes()
        .find_map(|(id, node, _)| matches!(node, StoryNode::Jump { .. }).then_some(*id))
        .expect("jump node");
    assert!(graph.connections().all(|conn| conn.from != jump_id));

    let issues = visual_novel_engine::authoring::validate_authoring_graph(&graph);
    assert!(issues
        .iter()
        .any(|issue| issue.code == LintCode::MissingJumpTarget));
    let err = graph
        .to_script_strict()
        .expect_err("broken target should not export strictly");
    assert!(format!("{err}").contains("missing_label"));
}

#[test]
fn validation_covers_empty_keys_layout_and_scene_profile_assets() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut graph = NodeGraph::new();
    graph.add_node(StoryNode::Start, pos(0.0, 0.0));
    graph.add_node(StoryNode::Start, pos(10.0, 0.0));
    graph.add_node(
        StoryNode::SetFlag {
            key: " ".to_string(),
            value: true,
        },
        pos(f32::NAN, 0.0),
    );
    graph.add_node(
        StoryNode::JumpIf {
            target: "start".to_string(),
            cond: CondRaw::Flag {
                key: String::new(),
                is_set: true,
            },
        },
        pos(0.0, 90.0),
    );
    graph.insert_scene_profile(
        "profile",
        SceneProfile {
            background: Some("assets/missing_bg.png".to_string()),
            music: None,
            characters: vec![CharacterPlacementRaw {
                name: "Ava".to_string(),
                expression: Some("assets/missing_pose.png".to_string()),
                ..Default::default()
            }],
            layers: Vec::new(),
            poses: Vec::new(),
        },
    );

    let issues = validate_authoring_graph_with_project_root(&graph, dir.path());
    assert!(issues.iter().any(|issue| {
        issue.code == LintCode::MultipleStart && issue.severity == LintSeverity::Error
    }));
    assert!(issues
        .iter()
        .any(|issue| issue.code == LintCode::EmptyStateKey));
    assert!(issues
        .iter()
        .any(|issue| issue.code == LintCode::InvalidLayoutPosition));
    assert!(
        issues
            .iter()
            .filter(|issue| issue.code == LintCode::AssetReferenceMissing)
            .count()
            >= 2
    );
}

#[test]
fn repro_and_dry_run_simulate_extcall_without_hitting_step_limit() {
    let script = ScriptRaw::new(
        vec![
            EventRaw::ExtCall {
                command: "analytics".to_string(),
                args: vec!["ping".to_string()],
            },
            EventRaw::Dialogue(DialogueRaw {
                speaker: "Narrator".to_string(),
                text: "After".to_string(),
            }),
        ],
        BTreeMap::from([("start".to_string(), 0)]),
    );
    let mut case = ReproCase::new("extcall", script.clone());
    case.max_steps = 8;
    let report = visual_novel_engine::run_repro_case(&case);
    assert_eq!(
        report.stop_reason,
        visual_novel_engine::ReproStopReason::Finished
    );
    assert_eq!(
        report.steps[0].simulation_note.as_deref(),
        Some("external_call_simulated")
    );

    let graph = NodeGraph::from_script(&script);
    let result = visual_novel_engine::authoring::compiler::compile_authoring_graph(&graph, None);
    let dry = result.dry_run_report.expect("dry-run report");
    assert!(dry
        .steps
        .iter()
        .any(|step| step.simulation_note.as_deref() == Some("external_call_simulated")));
}

#[test]
fn prefetch_uses_expression_asset_not_character_name() {
    let script = ScriptRaw::new(
        vec![EventRaw::Scene(visual_novel_engine::SceneUpdateRaw {
            background: Some("bg/room.png".to_string()),
            music: None,
            characters: vec![CharacterPlacementRaw {
                name: "Ava".to_string(),
                expression: Some("characters/ava.png".to_string()),
                ..Default::default()
            }],
        })],
        BTreeMap::from([("start".to_string(), 0)]),
    );
    let engine = Engine::new(
        script,
        SecurityPolicy::default(),
        ResourceLimiter::default(),
    )
    .expect("engine");
    let paths = engine.peek_next_asset_paths(1);
    assert!(paths.contains(&"characters/ava.png".to_string()));
    assert!(!paths.contains(&"Ava".to_string()));

    let assets = engine
        .peek_next_assets(1)
        .into_iter()
        .map(|asset| asset.as_u64())
        .collect::<Vec<_>>();
    assert!(assets.contains(&AssetId::from_path("characters/ava.png").as_u64()));
    assert!(!assets.contains(&AssetId::from_path("Ava").as_u64()));
}

#[test]
fn transition_is_observable_from_ui_and_engine() {
    let script = ScriptRaw::new(
        vec![EventRaw::Transition(SceneTransitionRaw {
            kind: "fade".to_string(),
            duration_ms: 250,
            color: Some("#000000".to_string()),
        })],
        BTreeMap::from([("start".to_string(), 0)]),
    );
    let mut engine = Engine::new(
        script,
        SecurityPolicy::default(),
        ResourceLimiter::default(),
    )
    .expect("engine");
    let event = engine.current_event().expect("transition event");
    let ui = visual_novel_engine::UiState::from_event(&event, engine.visual_state());
    assert_eq!(
        ui.pending_transition
            .as_ref()
            .map(|transition| transition.kind.as_str()),
        Some("fade")
    );
    engine.step().expect("step transition");
    assert!(engine.pending_transition().is_some());
}
