use super::*;
use crate::editor::node_graph::NodeGraph;
use crate::editor::node_types::StoryNode;
use eframe::egui;

fn p(x: f32, y: f32) -> egui::Pos2 {
    egui::pos2(x, y)
}

fn build_linear_graph() -> NodeGraph {
    let mut graph = NodeGraph::new();
    let start = graph.add_node(StoryNode::Start, p(0.0, 0.0));
    let dialogue = graph.add_node(
        StoryNode::Dialogue {
            speaker: "Ava".to_string(),
            text: "Hola".to_string(),
        },
        p(0.0, 100.0),
    );
    let end = graph.add_node(StoryNode::End, p(0.0, 200.0));
    graph.connect(start, dialogue);
    graph.connect(dialogue, end);
    graph
}

fn build_branching_graph() -> NodeGraph {
    let mut graph = NodeGraph::new();
    let start = graph.add_node(StoryNode::Start, p(0.0, 0.0));
    let intro = graph.add_node(
        StoryNode::Dialogue {
            speaker: "Narrador".to_string(),
            text: "Inicio".to_string(),
        },
        p(0.0, 100.0),
    );
    let choice = graph.add_node(
        StoryNode::Choice {
            prompt: "Ruta".to_string(),
            options: vec!["A".to_string(), "B".to_string()],
        },
        p(0.0, 200.0),
    );
    let branch_a = graph.add_node(
        StoryNode::Dialogue {
            speaker: "A".to_string(),
            text: "Ruta A".to_string(),
        },
        p(-120.0, 300.0),
    );
    let branch_b = graph.add_node(
        StoryNode::Dialogue {
            speaker: "B".to_string(),
            text: "Ruta B".to_string(),
        },
        p(120.0, 300.0),
    );
    let end = graph.add_node(StoryNode::End, p(0.0, 400.0));

    graph.connect(start, intro);
    graph.connect(intro, choice);
    graph.connect_port(choice, 0, branch_a);
    graph.connect_port(choice, 1, branch_b);
    graph.connect(branch_a, end);
    graph.connect(branch_b, end);

    graph
}

fn build_scene_bootstrap_graph() -> NodeGraph {
    let mut graph = NodeGraph::new();
    let start = graph.add_node(StoryNode::Start, p(0.0, 0.0));
    let scene = graph.add_node(
        StoryNode::Scene {
            profile: None,
            background: Some("bg/classroom.png".to_string()),
            music: Some("bgm/theme.ogg".to_string()),
            characters: vec![visual_novel_engine::CharacterPlacementRaw {
                name: "Ava".to_string(),
                expression: Some("neutral".to_string()),
                position: Some("center".to_string()),
                x: Some(0),
                y: Some(0),
                scale: Some(1.0),
            }],
        },
        p(0.0, 100.0),
    );
    let dialogue = graph.add_node(
        StoryNode::Dialogue {
            speaker: "Ava".to_string(),
            text: "Inicio".to_string(),
        },
        p(0.0, 200.0),
    );
    let end = graph.add_node(StoryNode::End, p(0.0, 300.0));
    graph.connect(start, scene);
    graph.connect(scene, dialogue);
    graph.connect(dialogue, end);
    graph
}

#[test]
fn compile_project_emits_expected_phase_trace_order() {
    let graph = build_linear_graph();
    let result = compile_project(&graph);

    let phases: Vec<CompilationPhase> = result.phase_trace.iter().map(|p| p.phase).collect();
    assert_eq!(
        phases,
        vec![
            CompilationPhase::GraphSync,
            CompilationPhase::GraphValidation,
            CompilationPhase::ScriptCompile,
            CompilationPhase::RuntimeInit,
            CompilationPhase::DryRun,
        ]
    );
}

#[test]
fn compile_project_reports_dry_run_completion() {
    let graph = build_linear_graph();
    let result = compile_project(&graph);

    assert!(result.engine_result.is_ok());
    assert!(result
        .issues
        .iter()
        .any(|issue| issue.code == LintCode::DryRunFinished));
}

#[test]
fn preview_runtime_sequence_matches_raw_sequence_for_default_route() {
    let graph = build_branching_graph();
    let result = compile_project(&graph);
    let report = result.dry_run_report.expect("dry run report");
    let runtime_seq: Vec<String> = report
        .steps
        .iter()
        .map(|step| step.event_signature.clone())
        .collect();
    let first = ChoicePolicy::Strategy(ChoiceStrategy::First);
    let raw_seq: Vec<String> = simulate_raw_sequence(&result.script, 32, &first)
        .into_iter()
        .map(|step| step.event_signature)
        .collect();
    assert_eq!(runtime_seq, raw_seq);
    assert!(!result
        .issues
        .iter()
        .any(|issue| issue.code == LintCode::DryRunParityMismatch));
}

#[test]
fn parity_uses_scene_bootstrap_state_like_runtime() {
    let graph = build_scene_bootstrap_graph();
    let result = compile_project(&graph);
    let report = result.dry_run_report.expect("dry run report");

    assert!(!result
        .issues
        .iter()
        .any(|issue| issue.code == LintCode::DryRunParityMismatch));

    let first = report.steps.first().expect("first dry-run step");
    assert_eq!(first.visual_background.as_deref(), Some("bg/classroom.png"));
    assert_eq!(first.visual_music.as_deref(), Some("bgm/theme.ogg"));
    assert_eq!(first.character_count, 1);
}

#[test]
fn raw_simulation_supports_multiple_choice_routes() {
    let graph = build_branching_graph();
    let script = crate::editor::script_sync::to_script(&graph);

    let first_policy = ChoicePolicy::Strategy(ChoiceStrategy::First);
    let last_policy = ChoicePolicy::Strategy(ChoiceStrategy::Last);
    let alternating_policy = ChoicePolicy::Strategy(ChoiceStrategy::Alternating);
    let first = simulate_raw_sequence(&script, 32, &first_policy);
    let last = simulate_raw_sequence(&script, 32, &last_policy);
    let alternating = simulate_raw_sequence(&script, 32, &alternating_policy);

    assert!(!first.is_empty());
    assert!(!last.is_empty());
    assert!(!alternating.is_empty());
    assert_ne!(
        first.iter().map(|s| &s.event_signature).collect::<Vec<_>>(),
        last.iter().map(|s| &s.event_signature).collect::<Vec<_>>()
    );
}

#[test]
fn route_enumerator_covers_choice_branches() {
    let graph = build_branching_graph();
    let script = crate::editor::script_sync::to_script(&graph);
    let routes = enumerate_choice_routes(&script, 64, 16, 8);

    assert!(routes.iter().any(|route| route.as_slice() == [0]));
    assert!(routes.iter().any(|route| route.as_slice() == [1]));
}

#[test]
fn dry_run_report_contains_step_snapshots() {
    let graph = build_linear_graph();
    let result = compile_project(&graph);
    let report = result.dry_run_report.expect("dry run report");

    assert!(!report.steps.is_empty());
    assert!(report
        .steps
        .iter()
        .enumerate()
        .all(|(idx, trace)| trace.step == idx));
}

#[test]
fn minimal_repro_script_is_compileable() {
    let graph = build_branching_graph();
    let result = compile_project(&graph);
    let repro = result.minimal_repro_script().expect("repro script");
    assert!(repro.compile().is_ok());
}

#[test]
fn choice_connection_auto_creates_option_and_avoids_dry_run_runtime_error() {
    let mut graph = NodeGraph::new();
    let start = graph.add_node(StoryNode::Start, p(0.0, 0.0));
    let choice = graph.add_node(
        StoryNode::Choice {
            prompt: "Auto options".to_string(),
            options: Vec::new(),
        },
        p(0.0, 100.0),
    );
    let end = graph.add_node(StoryNode::End, p(0.0, 200.0));
    graph.connect(start, choice);
    graph.connect(choice, end);

    let result = compile_project(&graph);
    let dry_error = result
        .issues
        .iter()
        .find(|issue| issue.code == LintCode::DryRunRuntimeError);
    assert!(dry_error.is_none());

    let Some(StoryNode::Choice { options, .. }) = graph.get_node(choice) else {
        panic!("choice node should exist");
    };
    assert_eq!(options.len(), 1);
}

#[test]
fn critical_bug_auto_repro() {
    let mut graph = NodeGraph::new();
    let start = graph.add_node(StoryNode::Start, p(0.0, 0.0));
    let a = graph.add_node(
        StoryNode::Dialogue {
            speaker: "A".to_string(),
            text: "A".to_string(),
        },
        p(0.0, 100.0),
    );
    let b = graph.add_node(
        StoryNode::Dialogue {
            speaker: "B".to_string(),
            text: "B".to_string(),
        },
        p(0.0, 200.0),
    );
    graph.connect(start, a);
    graph.connect(a, b);
    graph.connect(b, a);

    let result = compile_project(&graph);
    assert!(result.issues.iter().any(|issue| {
        matches!(
            issue.code,
            LintCode::DryRunStepLimit | LintCode::PotentialLoop
        )
    }));
    let repro = result
        .minimal_repro_script()
        .expect("critical dry-run issue should produce minimal repro");
    assert!(repro.compile().is_ok());
}

#[test]
fn snapshot_replay_determinism() {
    let graph = build_branching_graph();
    let first = compile_project(&graph);
    let second = compile_project(&graph);
    let first_report = first.dry_run_report.expect("first report");
    let second_report = second.dry_run_report.expect("second report");

    let left: Vec<String> = first_report
        .steps
        .iter()
        .map(|step| {
            format!(
                "{}|{}|{}|{:?}|{:?}|{}",
                step.step,
                step.event_ip,
                step.event_signature,
                step.visual_background,
                step.visual_music,
                step.character_count
            )
        })
        .collect();
    let right: Vec<String> = second_report
        .steps
        .iter()
        .map(|step| {
            format!(
                "{}|{}|{}|{:?}|{:?}|{}",
                step.step,
                step.event_ip,
                step.event_signature,
                step.visual_background,
                step.visual_music,
                step.character_count
            )
        })
        .collect();
    assert_eq!(left, right);
}
