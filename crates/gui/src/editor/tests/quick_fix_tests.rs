use super::*;
use crate::editor::{validate_graph, LintCode, StoryNode, ValidationPhase};

fn p(x: f32, y: f32) -> egui::Pos2 {
    egui::pos2(x, y)
}

#[test]
fn missing_start_fix_adds_start_node() {
    let mut graph = NodeGraph::new();
    let issue = LintIssue::error(
        None,
        ValidationPhase::Graph,
        LintCode::MissingStart,
        "Missing Start node",
    );

    let changed = apply_fix(&mut graph, &issue, "graph_add_start")
        .expect("missing start fix should be applied");
    assert!(changed);
    assert!(graph
        .nodes()
        .any(|(_, node, _)| matches!(node, StoryNode::Start)));
}

#[test]
fn choice_unlinked_fix_connects_to_end() {
    let mut graph = NodeGraph::new();
    let start = graph.add_node(StoryNode::Start, p(0.0, 0.0));
    let choice = graph.add_node(
        StoryNode::Choice {
            prompt: "Pick".to_string(),
            options: vec!["A".to_string(), "B".to_string()],
        },
        p(0.0, 100.0),
    );
    graph.connect(start, choice);

    let issue = LintIssue::warning(
        Some(choice),
        ValidationPhase::Graph,
        LintCode::ChoiceOptionUnlinked,
        "Choice option 1 has no outgoing connection",
    );
    let changed = apply_fix(&mut graph, &issue, "choice_link_unlinked_to_end")
        .expect("choice unlinked fix should be applied");
    assert!(changed);
    assert!(graph
        .connections()
        .any(|conn| conn.from == choice && conn.from_port == 0));
    assert!(graph
        .connections()
        .any(|conn| conn.from == choice && conn.from_port == 1));
}

#[test]
fn audio_volume_fix_clamps_to_valid_range() {
    let mut graph = NodeGraph::new();
    let node_id = graph.add_node(
        StoryNode::AudioAction {
            channel: "bgm".to_string(),
            action: "play".to_string(),
            asset: Some("audio/theme.ogg".to_string()),
            volume: Some(4.0),
            fade_duration_ms: None,
            loop_playback: None,
        },
        p(0.0, 0.0),
    );

    let issue = LintIssue::error(
        Some(node_id),
        ValidationPhase::Graph,
        LintCode::InvalidAudioVolume,
        "Audio volume must be finite and in range [0.0, 1.0]",
    );
    let changed =
        apply_fix(&mut graph, &issue, "audio_clamp_volume").expect("volume fix should apply");
    assert!(changed);

    let Some(StoryNode::AudioAction { volume, .. }) = graph.get_node(node_id) else {
        panic!("expected audio action node");
    };
    assert_eq!(*volume, Some(1.0));
}

#[test]
fn audio_asset_empty_on_scene_uses_scene_music_fix() {
    let mut graph = NodeGraph::new();
    let node_id = graph.add_node(
        StoryNode::Scene {
            profile: None,
            background: None,
            music: Some("   ".to_string()),
            characters: Vec::new(),
        },
        p(0.0, 0.0),
    );
    let issue = LintIssue::warning(
        Some(node_id),
        ValidationPhase::Graph,
        LintCode::AudioAssetEmpty,
        "Scene music path is empty",
    );

    let fixes = suggest_fixes(&issue, &graph);
    assert!(fixes
        .iter()
        .any(|fix| fix.fix_id == "scene_clear_empty_music"));
    assert!(!fixes
        .iter()
        .any(|fix| fix.fix_id == "audio_clear_empty_asset"));

    let changed = apply_fix(&mut graph, &issue, "scene_clear_empty_music")
        .expect("scene music fix should apply");
    assert!(changed);

    let Some(StoryNode::Scene { music, .. }) = graph.get_node(node_id) else {
        panic!("expected scene node");
    };
    assert_eq!(*music, None);
}

#[test]
fn missing_asset_reference_fix_clears_only_targeted_field() {
    let mut graph = NodeGraph::new();
    let node_id = graph.add_node(
        StoryNode::Scene {
            profile: None,
            background: Some("bg/missing.png".to_string()),
            music: Some("bg/keep.ogg".to_string()),
            characters: Vec::new(),
        },
        p(0.0, 0.0),
    );
    let issue = LintIssue::error(
        Some(node_id),
        ValidationPhase::Graph,
        LintCode::AssetReferenceMissing,
        "Background asset does not exist",
    )
    .with_asset_path(Some("bg/missing.png".to_string()));

    let fixes = suggest_fixes(&issue, &graph);
    assert!(fixes
        .iter()
        .any(|fix| fix.fix_id == "clear_missing_asset_reference"));

    let changed = apply_fix(&mut graph, &issue, "clear_missing_asset_reference")
        .expect("missing-asset clear fix should apply");
    assert!(changed);

    let Some(StoryNode::Scene {
        background, music, ..
    }) = graph.get_node(node_id)
    else {
        panic!("expected scene node");
    };
    assert_eq!(*background, None);
    assert_eq!(music.as_deref(), Some("bg/keep.ogg"));
}

#[test]
fn unsafe_asset_reference_fix_clears_audio_asset_and_normalizes_action() {
    let mut graph = NodeGraph::new();
    let node_id = graph.add_node(
        StoryNode::AudioAction {
            channel: "bgm".to_string(),
            action: "play".to_string(),
            asset: Some("../secret.ogg".to_string()),
            volume: None,
            fade_duration_ms: None,
            loop_playback: None,
        },
        p(0.0, 0.0),
    );
    let issue = LintIssue::error(
        Some(node_id),
        ValidationPhase::Graph,
        LintCode::UnsafeAssetPath,
        "Unsafe audio asset path",
    )
    .with_asset_path(Some("../secret.ogg".to_string()));

    let fixes = suggest_fixes(&issue, &graph);
    assert!(fixes
        .iter()
        .any(|fix| fix.fix_id == "clear_unsafe_asset_reference"));

    let changed = apply_fix(&mut graph, &issue, "clear_unsafe_asset_reference")
        .expect("unsafe-asset clear fix should apply");
    assert!(changed);

    let Some(StoryNode::AudioAction { action, asset, .. }) = graph.get_node(node_id) else {
        panic!("expected audio node");
    };
    assert_eq!(asset, &None);
    assert_eq!(action, "stop");
}

#[test]
fn audio_missing_asset_fix_normalizes_to_stop() {
    let mut graph = NodeGraph::new();
    let node_id = graph.add_node(
        StoryNode::AudioAction {
            channel: "bgm".to_string(),
            action: "play".to_string(),
            asset: None,
            volume: None,
            fade_duration_ms: None,
            loop_playback: None,
        },
        p(0.0, 0.0),
    );
    let issue = LintIssue::warning(
        Some(node_id),
        ValidationPhase::Graph,
        LintCode::AudioAssetMissing,
        "Audio asset path is missing",
    );

    let fixes = suggest_fixes(&issue, &graph);
    assert!(fixes
        .iter()
        .any(|fix| fix.fix_id == "audio_missing_asset_to_stop"));

    let changed = apply_fix(&mut graph, &issue, "audio_missing_asset_to_stop")
        .expect("audio missing-asset fix should apply");
    assert!(changed);
    let Some(StoryNode::AudioAction { action, asset, .. }) = graph.get_node(node_id) else {
        panic!("expected audio node");
    };
    assert_eq!(action, "stop");
    assert_eq!(asset, &None);
}

#[test]
fn safe_fix_suggestions_exist_for_common_issue() {
    let mut graph = NodeGraph::new();
    let node_id = graph.add_node(
        StoryNode::Dialogue {
            speaker: "".to_string(),
            text: "hello".to_string(),
        },
        p(0.0, 0.0),
    );
    let issue = validate_graph(&graph)
        .into_iter()
        .find(|issue| issue.node_id == Some(node_id) && issue.code == LintCode::EmptySpeakerName)
        .expect("expected EmptySpeakerName issue");

    let fixes = suggest_fixes(&issue, &graph);
    assert!(!fixes.is_empty());
    assert!(fixes.iter().any(|fix| fix.risk == QuickFixRisk::Safe));
}

#[test]
fn autofix_preconditions() {
    let mut graph = NodeGraph::new();
    let issue = LintIssue::error(
        None,
        ValidationPhase::Graph,
        LintCode::InvalidAudioVolume,
        "Audio volume must be finite and in range [0.0, 1.0]",
    );
    let err = apply_fix(&mut graph, &issue, "audio_clamp_volume")
        .expect_err("fix must reject missing node_id precondition");
    assert!(
        err.contains("preconditions failed") || err.contains("requires node_id"),
        "unexpected error: {err}"
    );
}

#[test]
fn autofix_catalog_coverage() {
    let mut graph = NodeGraph::new();
    let start = graph.add_node(StoryNode::Start, p(0.0, 0.0));
    let choice = graph.add_node(
        StoryNode::Choice {
            prompt: "Q".to_string(),
            options: vec!["A".to_string()],
        },
        p(0.0, 120.0),
    );
    let dialogue = graph.add_node(
        StoryNode::Dialogue {
            speaker: "".to_string(),
            text: "Hola".to_string(),
        },
        p(0.0, 240.0),
    );
    let jump = graph.add_node(
        StoryNode::Jump {
            target: "".to_string(),
        },
        p(0.0, 360.0),
    );
    let transition = graph.add_node(
        StoryNode::Transition {
            kind: "bad".to_string(),
            duration_ms: 0,
            color: None,
        },
        p(0.0, 480.0),
    );
    let audio = graph.add_node(
        StoryNode::AudioAction {
            channel: "music".to_string(),
            action: "x".to_string(),
            asset: Some("".to_string()),
            volume: Some(9.0),
            fade_duration_ms: Some(0),
            loop_playback: None,
        },
        p(220.0, 120.0),
    );
    let scene = graph.add_node(
        StoryNode::Scene {
            profile: None,
            background: Some("".to_string()),
            music: None,
            characters: vec![visual_novel_engine::CharacterPlacementRaw {
                name: "".to_string(),
                expression: None,
                position: Some("center".to_string()),
                x: None,
                y: None,
                scale: None,
            }],
        },
        p(220.0, 240.0),
    );
    let character = graph.add_node(
        StoryNode::CharacterPlacement {
            name: "".to_string(),
            x: 0,
            y: 0,
            scale: Some(-1.0),
        },
        p(220.0, 360.0),
    );
    let scene_missing = graph.add_node(
        StoryNode::Scene {
            profile: None,
            background: Some("bg/missing.png".to_string()),
            music: None,
            characters: Vec::new(),
        },
        p(420.0, 240.0),
    );
    let audio_unsafe = graph.add_node(
        StoryNode::AudioAction {
            channel: "bgm".to_string(),
            action: "play".to_string(),
            asset: Some("../secret.ogg".to_string()),
            volume: None,
            fade_duration_ms: None,
            loop_playback: None,
        },
        p(420.0, 120.0),
    );
    graph.connect(start, choice);
    graph.connect(choice, dialogue);
    graph.connect(dialogue, jump);
    graph.connect(jump, transition);
    graph.connect(transition, scene);
    graph.connect(scene, audio);
    graph.connect(audio, character);
    graph.connect(character, scene_missing);
    graph.connect(scene_missing, audio_unsafe);

    let frequent = [
        LintIssue::error(
            None,
            ValidationPhase::Graph,
            LintCode::MissingStart,
            "Missing Start node",
        ),
        LintIssue::error(
            Some(choice),
            ValidationPhase::Graph,
            LintCode::ChoiceNoOptions,
            "Choice node has no options",
        ),
        LintIssue::warning(
            Some(choice),
            ValidationPhase::Graph,
            LintCode::ChoiceOptionUnlinked,
            "Choice option 1 has no outgoing connection",
        ),
        LintIssue::warning(
            Some(dialogue),
            ValidationPhase::Graph,
            LintCode::EmptySpeakerName,
            "Dialogue speaker is empty",
        ),
        LintIssue::warning(
            Some(jump),
            ValidationPhase::Graph,
            LintCode::EmptyJumpTarget,
            "Jump target is empty",
        ),
        LintIssue::warning(
            Some(transition),
            ValidationPhase::Graph,
            LintCode::InvalidTransitionKind,
            "Transition kind is invalid",
        ),
        LintIssue::warning(
            Some(transition),
            ValidationPhase::Graph,
            LintCode::InvalidTransitionDuration,
            "Transition duration should be > 0 ms",
        ),
        LintIssue::error(
            Some(audio),
            ValidationPhase::Graph,
            LintCode::InvalidAudioChannel,
            "Invalid audio channel",
        ),
        LintIssue::error(
            Some(audio),
            ValidationPhase::Graph,
            LintCode::InvalidAudioAction,
            "Invalid audio action",
        ),
        LintIssue::error(
            Some(audio),
            ValidationPhase::Graph,
            LintCode::InvalidAudioVolume,
            "Invalid audio volume",
        ),
        LintIssue::error(
            Some(audio),
            ValidationPhase::Graph,
            LintCode::InvalidAudioFade,
            "Invalid audio fade",
        ),
        LintIssue::warning(
            Some(scene),
            ValidationPhase::Graph,
            LintCode::SceneBackgroundEmpty,
            "Scene background path is empty",
        ),
        LintIssue::warning(
            Some(audio),
            ValidationPhase::Graph,
            LintCode::AudioAssetEmpty,
            "Audio action requires a non-empty asset path for play/fade",
        ),
        LintIssue::warning(
            Some(audio),
            ValidationPhase::Graph,
            LintCode::AudioAssetMissing,
            "Audio asset path is missing",
        ),
        LintIssue::error(
            Some(scene_missing),
            ValidationPhase::Graph,
            LintCode::AssetReferenceMissing,
            "Background asset does not exist: 'bg/missing.png'",
        )
        .with_asset_path(Some("bg/missing.png".to_string())),
        LintIssue::error(
            Some(audio_unsafe),
            ValidationPhase::Graph,
            LintCode::UnsafeAssetPath,
            "Unsafe audio asset path: '../secret.ogg'",
        )
        .with_asset_path(Some("../secret.ogg".to_string())),
        LintIssue::warning(
            Some(scene),
            ValidationPhase::Graph,
            LintCode::EmptyCharacterName,
            "Character name cannot be empty",
        ),
        LintIssue::warning(
            Some(character),
            ValidationPhase::Graph,
            LintCode::InvalidCharacterScale,
            "Character scale must be finite and > 0",
        ),
    ];

    let covered = frequent
        .iter()
        .filter(|issue| !suggest_fixes(issue, &graph).is_empty())
        .count();
    let coverage = covered as f32 / frequent.len() as f32;
    assert!(
        coverage >= 0.80,
        "quick-fix coverage must be >= 80%, got {:.2}%",
        coverage * 100.0
    );
}
