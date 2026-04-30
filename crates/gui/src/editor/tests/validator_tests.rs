use super::*;
use crate::editor::node_graph::NodeGraph;
use crate::editor::node_types::StoryNode;
use eframe::egui;
use std::fs;
use tempfile::tempdir;

fn p(x: f32, y: f32) -> egui::Pos2 {
    egui::pos2(x, y)
}

#[test]
fn diagnostic_id_is_stable_and_includes_phase_code_node_and_ip() {
    let issue = LintIssue::warning(
        Some(7),
        ValidationPhase::Graph,
        LintCode::UnreachableNode,
        "dead code",
    );
    assert_eq!(
        issue.diagnostic_id(),
        "authoring-diagnostic-v2:GRAPH:VAL_UNREACHABLE:7:na:na:na:na"
    );

    let issue = issue.with_event_ip(Some(3));
    assert_eq!(
        issue.diagnostic_id(),
        "authoring-diagnostic-v2:GRAPH:VAL_UNREACHABLE:7:3:na:na:na"
    );
}

#[test]
fn diagnostic_labels_roundtrip_from_single_contract() {
    for phase in ValidationPhase::ALL {
        assert_eq!(ValidationPhase::from_label(phase.label()), Some(*phase));
        assert_eq!(
            ValidationPhase::from_label(&phase.label().to_ascii_lowercase()),
            Some(*phase)
        );
    }

    for severity in LintSeverity::ALL {
        assert_eq!(LintSeverity::from_label(severity.label()), Some(*severity));
        assert_eq!(
            LintSeverity::from_label(&severity.label().to_ascii_uppercase()),
            Some(*severity)
        );
    }

    for code in LintCode::ALL {
        assert_eq!(LintCode::from_label(code.label()), Some(*code));
        assert_eq!(
            LintCode::from_label(&code.label().to_ascii_lowercase()),
            Some(*code)
        );
    }
}

#[test]
fn validate_reports_choice_unlinked_with_explicit_code() {
    let mut graph = NodeGraph::new();
    let start = graph.add_node(StoryNode::Start, p(0.0, 0.0));
    let choice = graph.add_node(
        StoryNode::Choice {
            prompt: "Choose".to_string(),
            options: vec!["A".to_string(), "B".to_string()],
        },
        p(0.0, 100.0),
    );
    graph.connect(start, choice);

    let issues = validate(&graph);
    assert!(issues
        .iter()
        .any(|i| i.code == LintCode::ChoiceOptionUnlinked));
    assert!(issues.iter().any(|i| i.phase == ValidationPhase::Graph));
    let choice_issue = issues
        .iter()
        .find(|i| i.code == LintCode::ChoiceOptionUnlinked)
        .expect("choice issue");
    assert_eq!(choice_issue.edge_from, Some(choice));
    assert_eq!(choice_issue.edge_to, None);
}

#[test]
fn validate_reports_unsafe_asset_paths_and_transition_duration() {
    let mut graph = NodeGraph::new();
    let start = graph.add_node(StoryNode::Start, p(0.0, 0.0));
    let scene = graph.add_node(
        StoryNode::Scene {
            profile: None,
            background: Some("../secrets/bg.png".to_string()),
            music: None,
            characters: Vec::new(),
        },
        p(0.0, 80.0),
    );
    let transition = graph.add_node(
        StoryNode::Transition {
            kind: "unknown".to_string(),
            duration_ms: 0,
            color: None,
        },
        p(0.0, 160.0),
    );
    let end = graph.add_node(StoryNode::End, p(0.0, 240.0));
    graph.connect(start, scene);
    graph.connect(scene, transition);
    graph.connect(transition, end);

    let issues = validate(&graph);
    let unsafe_issue = issues
        .iter()
        .find(|i| i.code == LintCode::UnsafeAssetPath)
        .expect("unsafe path issue");
    assert_eq!(
        unsafe_issue.asset_path.as_deref(),
        Some("../secrets/bg.png"),
        "unsafe issue should preserve exact asset path for traceability"
    );
    assert!(issues
        .iter()
        .any(|i| i.code == LintCode::InvalidTransitionDuration));
    assert!(issues
        .iter()
        .any(|i| i.code == LintCode::InvalidTransitionKind));
}

#[test]
fn validate_reports_empty_character_name() {
    let mut graph = NodeGraph::new();
    let start = graph.add_node(StoryNode::Start, p(0.0, 0.0));
    let placement = graph.add_node(
        StoryNode::CharacterPlacement {
            name: "".to_string(),
            x: 10,
            y: 10,
            scale: Some(1.0),
        },
        p(0.0, 100.0),
    );
    let end = graph.add_node(StoryNode::End, p(0.0, 200.0));
    graph.connect(start, placement);
    graph.connect(placement, end);

    let issues = validate(&graph);
    assert!(issues
        .iter()
        .any(|i| i.code == LintCode::EmptyCharacterName));
}

#[test]
fn validate_reports_missing_assets_when_probe_fails() {
    let mut graph = NodeGraph::new();
    let start = graph.add_node(StoryNode::Start, p(0.0, 0.0));
    let scene = graph.add_node(
        StoryNode::Scene {
            profile: None,
            background: Some("assets/bg_forest.png".to_string()),
            music: None,
            characters: Vec::new(),
        },
        p(0.0, 100.0),
    );
    let end = graph.add_node(StoryNode::End, p(0.0, 200.0));
    graph.connect(start, scene);
    graph.connect(scene, end);

    let issues = validate_with_asset_probe(&graph, |_asset| false);
    let issue = issues
        .iter()
        .find(|i| i.code == LintCode::AssetReferenceMissing)
        .expect("missing asset issue");
    assert_eq!(issue.asset_path.as_deref(), Some("assets/bg_forest.png"));
}

#[test]
fn validate_reports_invalid_audio_params() {
    let mut graph = NodeGraph::new();
    let start = graph.add_node(StoryNode::Start, p(0.0, 0.0));
    let audio = graph.add_node(
        StoryNode::AudioAction {
            channel: "music".to_string(),
            action: "boom".to_string(),
            asset: Some("assets/sfx/beep.wav".to_string()),
            volume: Some(1.5),
            fade_duration_ms: Some(0),
            loop_playback: Some(true),
        },
        p(0.0, 100.0),
    );
    let end = graph.add_node(StoryNode::End, p(0.0, 200.0));
    graph.connect(start, audio);
    graph.connect(audio, end);

    let issues = validate_with_asset_probe(&graph, |_asset| true);
    assert!(issues
        .iter()
        .any(|i| i.code == LintCode::InvalidAudioChannel));
    assert!(issues
        .iter()
        .any(|i| i.code == LintCode::InvalidAudioAction));
    assert!(issues
        .iter()
        .any(|i| i.code == LintCode::InvalidAudioVolume));
}

#[test]
fn validate_reports_scene_patch_and_generic_limits() {
    let mut graph = NodeGraph::new();
    let start = graph.add_node(StoryNode::Start, p(0.0, 0.0));
    let patch = graph.add_node(
        StoryNode::ScenePatch(visual_novel_engine::ScenePatchRaw {
            background: Some("../unsafe/bg.png".to_string()),
            music: None,
            add: vec![visual_novel_engine::CharacterPlacementRaw {
                name: "".to_string(),
                ..Default::default()
            }],
            update: Vec::new(),
            remove: Vec::new(),
        }),
        p(0.0, 100.0),
    );
    let generic = graph.add_node(
        StoryNode::Generic(visual_novel_engine::EventRaw::Jump {
            target: "node_1".to_string(),
        }),
        p(0.0, 200.0),
    );
    let end = graph.add_node(StoryNode::End, p(0.0, 300.0));

    graph.connect(start, patch);
    graph.connect(patch, generic);
    graph.connect(generic, end);

    let issues = validate_with_asset_probe(&graph, |_asset| true);
    let unsafe_issue = issues
        .iter()
        .find(|i| i.code == LintCode::UnsafeAssetPath)
        .expect("unsafe path issue");
    assert_eq!(
        unsafe_issue.asset_path.as_deref(),
        Some("../unsafe/bg.png"),
        "unsafe scene patch issue should preserve asset path"
    );
    assert!(issues
        .iter()
        .any(|i| i.code == LintCode::GenericEventUnchecked));
    assert!(issues
        .iter()
        .any(|i| i.code == LintCode::ContractUnsupportedExport));
}

#[test]
fn extcall_generic_is_exportable_and_preserves_trace_context() {
    let dir = tempdir().expect("tempdir");
    let project_root = dir.path().join("renpy_project");
    let game_dir = project_root.join("game");
    fs::create_dir_all(&game_dir).expect("mkdir game");
    fs::write(
        game_dir.join("script.rpy"),
        r#"
label start:
    call route_a
"#,
    )
    .expect("write script");

    let output_root = dir.path().join("import_out");
    visual_novel_engine::import_renpy_project(visual_novel_engine::ImportRenpyOptions {
        project_root,
        output_root: output_root.clone(),
        entry_label: "start".to_string(),
        report_path: None,
        profile: visual_novel_engine::ImportProfile::StoryFirst,
        include_tl: None,
        include_ui: None,
        include_patterns: Vec::new(),
        exclude_patterns: Vec::new(),
        strict_mode: false,
        fallback_policy: visual_novel_engine::ImportFallbackPolicy::DegradeWithTrace,
    })
    .expect("import renpy with trace envelope");

    let imported_json = fs::read_to_string(output_root.join("main.json")).expect("read main");
    let imported_script =
        visual_novel_engine::ScriptRaw::from_json(&imported_json).expect("parse script");
    let (ext_command, ext_args) = imported_script
        .events
        .iter()
        .find_map(|event| match event {
            visual_novel_engine::EventRaw::ExtCall { command, args } => {
                Some((command.clone(), args.clone()))
            }
            _ => None,
        })
        .expect("imported script must contain decorated extcall");

    let mut graph = NodeGraph::new();
    let start = graph.add_node(StoryNode::Start, p(0.0, 0.0));
    let ext = graph.add_node(
        StoryNode::Generic(visual_novel_engine::EventRaw::ExtCall {
            command: ext_command,
            args: ext_args,
        }),
        p(0.0, 100.0),
    );
    let end = graph.add_node(StoryNode::End, p(0.0, 200.0));
    graph.connect(start, ext);
    graph.connect(ext, end);

    let issues = validate_with_asset_probe(&graph, |_asset| true);
    assert!(
        !issues
            .iter()
            .any(|issue| issue.code == LintCode::ContractUnsupportedExport),
        "ExtCall generic should be exportable by contract"
    );

    let trace_issue = issues
        .iter()
        .find(|issue| issue.code == LintCode::GenericEventUnchecked)
        .expect("trace warning issue");
    assert!(
        trace_issue
            .blocked_by
            .as_ref()
            .is_some_and(|ctx| ctx.contains("script.rpy") && ctx.contains("ip=")),
        "trace warning should include source flow location"
    );
    assert!(
        trace_issue.message.contains("area=story") && trace_issue.message.contains("phase=parse"),
        "trace warning should expose envelope context for graph-driven diagnosis"
    );
}

#[test]
fn unreachable_node_reports_blocked_flow_context() {
    let mut graph = NodeGraph::new();
    let start = graph.add_node(StoryNode::Start, p(0.0, 0.0));
    let reachable = graph.add_node(
        StoryNode::Dialogue {
            speaker: "A".to_string(),
            text: "ok".to_string(),
        },
        p(0.0, 100.0),
    );
    let unreachable = graph.add_node(
        StoryNode::Dialogue {
            speaker: "B".to_string(),
            text: "dead".to_string(),
        },
        p(200.0, 100.0),
    );
    graph.connect(start, reachable);
    graph.connect(unreachable, reachable);

    let issues = validate(&graph);
    let issue = issues
        .iter()
        .find(|entry| entry.code == LintCode::UnreachableNode && entry.node_id == Some(unreachable))
        .expect("unreachable issue");
    assert!(issue.blocked_by.is_some());
}

#[test]
fn dead_route_detection() {
    let mut graph = NodeGraph::new();
    let start = graph.add_node(StoryNode::Start, p(0.0, 0.0));
    let a = graph.add_node(
        StoryNode::Dialogue {
            speaker: "A".to_string(),
            text: "Loop A".to_string(),
        },
        p(0.0, 100.0),
    );
    let b = graph.add_node(
        StoryNode::Dialogue {
            speaker: "B".to_string(),
            text: "Loop B".to_string(),
        },
        p(0.0, 200.0),
    );
    let unreachable = graph.add_node(
        StoryNode::Dialogue {
            speaker: "X".to_string(),
            text: "Dead route".to_string(),
        },
        p(200.0, 100.0),
    );
    graph.connect(start, a);
    graph.connect(a, b);
    graph.connect(b, a);

    let issues = validate(&graph);
    assert!(issues
        .iter()
        .any(|i| i.code == LintCode::UnreachableNode && i.node_id == Some(unreachable)));
    assert!(issues.iter().any(
        |i| i.code == LintCode::PotentialLoop && (i.node_id == Some(a) || i.node_id == Some(b))
    ));
}

#[test]
fn unreachable_cycle_is_not_reported_as_reachable_loop() {
    let mut graph = NodeGraph::new();
    let start = graph.add_node(StoryNode::Start, p(0.0, 0.0));
    let end = graph.add_node(StoryNode::End, p(0.0, 100.0));
    let a = graph.add_node(
        StoryNode::Dialogue {
            speaker: "A".to_string(),
            text: "Hidden loop A".to_string(),
        },
        p(200.0, 100.0),
    );
    let b = graph.add_node(
        StoryNode::Dialogue {
            speaker: "B".to_string(),
            text: "Hidden loop B".to_string(),
        },
        p(200.0, 200.0),
    );
    graph.connect(start, end);
    graph.connect(a, b);
    graph.connect(b, a);

    let issues = validate(&graph);

    assert!(issues
        .iter()
        .any(|issue| issue.code == LintCode::UnreachableNode && issue.node_id == Some(a)));
    assert!(!issues
        .iter()
        .any(|issue| issue.code == LintCode::PotentialLoop && issue.node_id == Some(a)));
    assert!(!issues
        .iter()
        .any(|issue| issue.code == LintCode::PotentialLoop && issue.node_id == Some(b)));
}

#[test]
fn validate_with_project_root_resolves_relative_assets_from_loaded_project() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let project_root = tmp.path().join("project");
    std::fs::create_dir_all(project_root.join("assets")).expect("mkdir assets");
    std::fs::write(project_root.join("assets").join("bg_forest.png"), b"ok")
        .expect("write bg asset");

    let mut graph = NodeGraph::new();
    let start = graph.add_node(StoryNode::Start, p(0.0, 0.0));
    let scene = graph.add_node(
        StoryNode::Scene {
            profile: None,
            background: Some("assets/bg_forest.png".to_string()),
            music: None,
            characters: Vec::new(),
        },
        p(0.0, 100.0),
    );
    let end = graph.add_node(StoryNode::End, p(0.0, 200.0));
    graph.connect(start, scene);
    graph.connect(scene, end);

    let issues = validate_with_project_root(&graph, &project_root);
    assert!(
        issues
            .iter()
            .all(|issue| issue.code != LintCode::AssetReferenceMissing),
        "asset should resolve against project_root, not process current_dir"
    );
}
