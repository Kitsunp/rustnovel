use std::collections::BTreeMap;

use crate::CharacterPlacementRaw;
use crate::{DialogueRaw, EventRaw, ScriptRaw};

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
