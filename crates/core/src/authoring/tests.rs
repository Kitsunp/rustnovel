use std::collections::BTreeMap;

use crate::CharacterPlacementRaw;
use crate::{
    contract_for_authoring_node, ChoiceOptionRaw, ChoiceRaw, CondRaw, DialogueRaw, Engine,
    EventRaw, ResourceLimiter, ScriptRaw, SecurityPolicy, VnError,
};

use super::quick_fix::{apply_fix, suggest_fixes};
use super::*;

fn pos(x: f32, y: f32) -> AuthoringPosition {
    AuthoringPosition::new(x, y)
}

fn character(name: &str, image: &str) -> CharacterPlacementRaw {
    CharacterPlacementRaw {
        name: name.to_string(),
        expression: Some(image.to_string()),
        ..Default::default()
    }
}

#[test]
fn authoring_graph_roundtrips_script_without_gui_types() {
    let mut labels = BTreeMap::new();
    labels.insert("start".to_string(), 0);
    let script = ScriptRaw::new(
        vec![EventRaw::Dialogue(DialogueRaw {
            speaker: "Ava".to_string(),
            text: "Hola".to_string(),
        })],
        labels,
    );

    let graph = NodeGraph::from_script(&script);
    let roundtrip = graph.to_script();

    assert!(roundtrip.labels.contains_key("start"));
    assert!(matches!(
        roundtrip.events.first(),
        Some(EventRaw::Dialogue(_))
    ));
}

#[test]
fn authoring_validation_and_safe_quick_fix_are_headless() {
    let mut graph = NodeGraph::new();
    let start = graph.add_node(StoryNode::Start, pos(0.0, 0.0));
    let dialogue = graph.add_node(
        StoryNode::Dialogue {
            speaker: String::new(),
            text: "Hola".to_string(),
        },
        pos(0.0, 90.0),
    );
    let end = graph.add_node(StoryNode::End, pos(0.0, 180.0));
    graph.connect(start, dialogue);
    graph.connect(dialogue, end);

    let issue = validate_authoring_graph(&graph)
        .into_iter()
        .find(|issue| issue.code == LintCode::EmptySpeakerName)
        .expect("empty speaker issue");
    let fix = suggest_fixes(&issue, &graph)
        .into_iter()
        .find(|fix| fix.risk == QuickFixRisk::Safe)
        .expect("safe speaker fix");

    assert_eq!(fix.fix_id, "dialogue_fill_speaker");
    assert!(apply_fix(&mut graph, &issue, fix.fix_id).expect("fix applies"));
    assert!(validate_authoring_graph(&graph)
        .iter()
        .all(|issue| issue.code != LintCode::EmptySpeakerName));
}

#[test]
fn missing_start_fix_is_structural_review() {
    let graph = NodeGraph::new();
    let issue = validate_authoring_graph(&graph)
        .into_iter()
        .find(|issue| issue.code == LintCode::MissingStart)
        .expect("missing start issue");
    let candidate = suggest_fixes(&issue, &graph)
        .into_iter()
        .next()
        .expect("start fix candidate");

    assert_eq!(candidate.fix_id, "graph_add_start");
    assert_eq!(candidate.risk, QuickFixRisk::Review);
    assert!(candidate.structural);
}

#[test]
fn authoring_graph_flow_analysis_reports_cycles_without_gui_helpers() {
    let mut graph = NodeGraph::new();
    let start = graph.add_node(StoryNode::Start, pos(0.0, 0.0));
    let first = graph.add_node(
        StoryNode::Dialogue {
            speaker: "Ava".to_string(),
            text: "Uno".to_string(),
        },
        pos(0.0, 80.0),
    );
    let second = graph.add_node(
        StoryNode::Dialogue {
            speaker: "Ava".to_string(),
            text: "Dos".to_string(),
        },
        pos(0.0, 160.0),
    );
    let hidden = graph.add_node(StoryNode::End, pos(220.0, 0.0));
    graph.connect(start, first);
    graph.connect(first, second);
    graph.connect(second, first);

    let flow = graph.flow_analysis(&[start]);

    assert!(flow.reachable.contains(&start));
    assert!(flow.reachable.contains(&first));
    assert!(flow.reachable.contains(&second));
    assert_eq!(flow.unreachable, vec![hidden]);
    assert!(flow.reachable_cycle_nodes.contains(&first));
    assert!(flow.reachable_cycle_nodes.contains(&second));
    let issues = validate_authoring_graph(&graph);
    assert!(issues
        .iter()
        .any(|issue| issue.code == LintCode::PotentialLoop));
}

#[test]
fn authoring_quick_fix_clears_empty_scene_music() {
    let mut graph = NodeGraph::new();
    let scene = graph.add_node(
        StoryNode::Scene {
            profile: None,
            background: None,
            music: Some(" ".to_string()),
            characters: Vec::new(),
        },
        pos(0.0, 0.0),
    );
    let issue = validate_authoring_graph(&graph)
        .into_iter()
        .find(|issue| issue.node_id == Some(scene) && issue.code == LintCode::AudioAssetEmpty)
        .expect("empty scene music issue");
    let fix = suggest_fixes(&issue, &graph)
        .into_iter()
        .find(|fix| fix.fix_id == "scene_clear_empty_music")
        .expect("scene music fix");

    assert!(apply_fix(&mut graph, &issue, fix.fix_id).expect("fix applies"));
    assert!(matches!(
        graph.get_node(scene),
        Some(StoryNode::Scene { music: None, .. })
    ));
}

#[test]
fn authoring_quick_fix_clears_unsafe_background_reference() {
    let mut graph = NodeGraph::new();
    let scene = graph.add_node(
        StoryNode::Scene {
            profile: None,
            background: Some("../outside.png".to_string()),
            music: None,
            characters: Vec::new(),
        },
        pos(0.0, 0.0),
    );
    let issue = validate_authoring_graph(&graph)
        .into_iter()
        .find(|issue| issue.node_id == Some(scene) && issue.code == LintCode::UnsafeAssetPath)
        .expect("unsafe background issue");
    let fix = suggest_fixes(&issue, &graph)
        .into_iter()
        .find(|fix| fix.fix_id == "clear_unsafe_asset_reference")
        .expect("unsafe asset fix");

    assert!(apply_fix(&mut graph, &issue, fix.fix_id).expect("fix applies"));
    assert!(matches!(
        graph.get_node(scene),
        Some(StoryNode::Scene {
            background: None,
            ..
        })
    ));
}

#[test]
fn authoring_quick_fix_normalizes_play_without_audio_asset() {
    let mut graph = NodeGraph::new();
    let audio = graph.add_node(
        StoryNode::AudioAction {
            channel: "bgm".to_string(),
            action: "play".to_string(),
            asset: None,
            volume: None,
            fade_duration_ms: None,
            loop_playback: None,
        },
        pos(0.0, 0.0),
    );
    let issue = LintIssue::warning(
        Some(audio),
        ValidationPhase::Graph,
        LintCode::AudioAssetMissing,
        "AudioAction is play without a valid asset",
    );
    let fix = suggest_fixes(&issue, &graph)
        .into_iter()
        .find(|fix| fix.fix_id == "audio_missing_asset_to_stop")
        .expect("audio missing asset fix");

    assert!(apply_fix(&mut graph, &issue, fix.fix_id).expect("fix applies"));
    assert!(matches!(
        graph.get_node(audio),
        Some(StoryNode::AudioAction { action, asset, .. })
            if action == "stop" && asset.is_none()
    ));
}

#[test]
fn scene_profile_contract_records_layers_and_pose_catalog() {
    let mut graph = NodeGraph::new();
    let scene = graph.add_node(
        StoryNode::Scene {
            profile: None,
            background: Some("bg/classroom.png".to_string()),
            music: None,
            characters: vec![character("Ava", "sprites/ava_smile.png")],
        },
        pos(0.0, 0.0),
    );

    assert!(graph.save_scene_profile("classroom_intro", scene));
    let profile = graph
        .scene_profile("classroom_intro")
        .expect("profile should be stored in core authoring");

    assert_eq!(profile.layers.len(), 2);
    assert_eq!(
        profile.layers[0].background.as_deref(),
        Some("bg/classroom.png")
    );
    assert_eq!(profile.layers[1].characters[0].name, "Ava");
    assert_eq!(profile.poses[0].pose, "ava_smile");
}

#[test]
fn end_label_security_policy_and_runtime_finish_cleanly() {
    let script = ScriptRaw::new(
        vec![EventRaw::Choice(ChoiceRaw {
            prompt: "Finish?".to_string(),
            options: vec![ChoiceOptionRaw {
                text: "End".to_string(),
                target: "__end".to_string(),
            }],
        })],
        BTreeMap::from([("start".to_string(), 0), ("__end".to_string(), 1)]),
    );
    let policy = SecurityPolicy::default();

    policy
        .validate_raw(&script, ResourceLimiter::default())
        .expect("__end label at events.len() is a valid terminal");
    let compiled = script.compile().expect("compile terminal choice");
    policy
        .validate_compiled(&compiled, ResourceLimiter::default())
        .expect("compiled terminal target is valid");
    let mut engine = Engine::from_compiled(
        compiled,
        SecurityPolicy::default(),
        ResourceLimiter::default(),
    )
    .expect("engine init");

    engine.choose(0).expect("choose terminal route");
    assert!(matches!(engine.current_event(), Err(VnError::EndOfScript)));
}

#[test]
fn jump_if_roundtrip_preserves_true_and_false_ports() {
    let script = ScriptRaw::new(
        vec![
            EventRaw::JumpIf {
                cond: CondRaw::Flag {
                    key: "seen".to_string(),
                    is_set: true,
                },
                target: "true_branch".to_string(),
            },
            EventRaw::Dialogue(DialogueRaw {
                speaker: "Narrator".to_string(),
                text: "False branch".to_string(),
            }),
            EventRaw::Dialogue(DialogueRaw {
                speaker: "Narrator".to_string(),
                text: "True branch".to_string(),
            }),
        ],
        BTreeMap::from([("start".to_string(), 0), ("true_branch".to_string(), 2)]),
    );

    let graph = NodeGraph::from_script(&script);
    let jump_id = graph
        .nodes()
        .find_map(|(id, node, _)| matches!(node, StoryNode::JumpIf { .. }).then_some(*id))
        .expect("jump_if node");
    let false_id = graph
        .nodes()
        .find_map(|(id, node, _)| {
            matches!(
                node,
                StoryNode::Dialogue { text, .. } if text == "False branch"
            )
            .then_some(*id)
        })
        .expect("false branch node");
    let true_id = graph
        .nodes()
        .find_map(|(id, node, _)| {
            matches!(
                node,
                StoryNode::Dialogue { text, .. } if text == "True branch"
            )
            .then_some(*id)
        })
        .expect("true branch node");

    assert!(graph
        .connections()
        .any(|conn| conn.from == jump_id && conn.from_port == 0 && conn.to == true_id));
    assert!(graph
        .connections()
        .any(|conn| conn.from == jump_id && conn.from_port == 1 && conn.to == false_id));

    let roundtrip = graph.to_script();
    let EventRaw::JumpIf { target, .. } = &roundtrip.events[0] else {
        panic!("first event should remain JumpIf");
    };
    assert_eq!(roundtrip.labels.get(target), Some(&2));
}

#[test]
fn setflag_authoring_contract_roundtrips_as_semantic_node() {
    let script = ScriptRaw::new(
        vec![EventRaw::SetFlag {
            key: "met_ava".to_string(),
            value: true,
        }],
        BTreeMap::from([("start".to_string(), 0)]),
    );

    let graph = NodeGraph::from_script(&script);
    let node = graph
        .nodes()
        .find_map(|(_, node, _)| matches!(node, StoryNode::SetFlag { .. }).then_some(node))
        .expect("set flag node");
    let contract = contract_for_authoring_node(node);
    assert!(contract.runtime_supported);
    assert!(contract.export_supported);
    assert!(matches!(
        graph.to_script().events.first(),
        Some(EventRaw::SetFlag { key, value }) if key == "met_ava" && *value
    ));
}

#[test]
fn windows_drive_path_is_unsafe_in_core_policy() {
    assert!(is_unsafe_asset_ref(r"C:\temp\evil.png"));
}

#[test]
fn quickfix_rejects_stale_issue_after_node_changed() {
    let mut graph = NodeGraph::new();
    let dialogue = graph.add_node(
        StoryNode::Dialogue {
            speaker: String::new(),
            text: "Hola".to_string(),
        },
        pos(0.0, 0.0),
    );
    let issue = validate_authoring_graph(&graph)
        .into_iter()
        .find(|issue| issue.node_id == Some(dialogue) && issue.code == LintCode::EmptySpeakerName)
        .expect("speaker issue");
    let Some(StoryNode::Dialogue { speaker, .. }) = graph.get_node_mut(dialogue) else {
        panic!("dialogue node should still exist");
    };
    *speaker = "Narrator".to_string();

    assert!(apply_fix(&mut graph, &issue, "dialogue_fill_speaker").is_err());
}

#[test]
fn route_coverage_report_marks_route_and_depth_limits() {
    let many_options = (0..40)
        .map(|idx| ChoiceOptionRaw {
            text: format!("Route {idx}"),
            target: "__end".to_string(),
        })
        .collect::<Vec<_>>();
    let script = ScriptRaw::new(
        vec![EventRaw::Choice(ChoiceRaw {
            prompt: "Route?".to_string(),
            options: many_options,
        })],
        BTreeMap::from([("start".to_string(), 0), ("__end".to_string(), 1)]),
    );

    let limited_routes = compiler::enumerate_choice_routes_with_report(&script, 100, 32, 12);
    assert!(limited_routes.route_limit_hit);
    assert_eq!(limited_routes.routes.len(), 32);

    let depth_limited = compiler::enumerate_choice_routes_with_report(&script, 100, 32, 0);
    assert!(depth_limited.depth_limit_hit);
}

#[test]
fn authoring_report_fingerprint_tracks_script_graph_and_asset_refs() {
    let mut graph = NodeGraph::new();
    let scene = graph.add_node(
        StoryNode::Scene {
            profile: None,
            background: Some("bg/room.png".to_string()),
            music: Some("bgm/theme.ogg".to_string()),
            characters: vec![character("Ava", "char/ava_smile.png")],
        },
        pos(0.0, 0.0),
    );
    assert!(graph.save_scene_profile("room", scene));

    let first_script = graph.to_script();
    let first = build_authoring_report_fingerprint(&graph, &first_script);
    assert_eq!(first.script_sha256.len(), 64);
    assert_eq!(first.graph_sha256.len(), 64);
    assert_eq!(first.asset_refs_sha256.len(), 64);
    assert_eq!(first.semantic_sha256.len(), 64);
    assert_eq!(first.semantic.graph_sha256, first.graph_sha256);
    assert_eq!(first.asset_refs_count, 3);

    let Some(StoryNode::Scene { background, .. }) = graph.get_node_mut(scene) else {
        panic!("scene node should still exist");
    };
    *background = Some("bg/room-night.png".to_string());
    let changed = build_authoring_report_fingerprint(&graph, &graph.to_script());

    assert_ne!(first.script_sha256, changed.script_sha256);
    assert_ne!(first.graph_sha256, changed.graph_sha256);
    assert_ne!(first.asset_refs_sha256, changed.asset_refs_sha256);
    assert_ne!(first.semantic_sha256, changed.semantic_sha256);
}
