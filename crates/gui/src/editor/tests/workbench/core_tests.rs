use super::super::*;
use crate::editor::StoryNode;

#[test]
fn test_workbench_initialization() {
    let config = VnConfig::default();
    let mut workbench = EditorWorkbench::new(config);

    // Assert default state
    assert_eq!(workbench.mode, EditorMode::Editor);
    assert!(workbench.node_graph.is_empty());
    assert!(!workbench.is_playing);

    // Add dummy track
    let mut track = visual_novel_engine::Track::new(
        visual_novel_engine::EntityId::new(1),
        visual_novel_engine::PropertyType::PositionX,
    );
    track
        .add_keyframe(visual_novel_engine::Keyframe::new(
            100,
            0,
            visual_novel_engine::Easing::Linear,
        ))
        .unwrap();
    workbench.timeline.add_track(track).unwrap();

    // Test simple update
    workbench.is_playing = true;
    workbench.update(1);
    assert!(
        workbench.current_time > 0.0,
        "Time should advance when playing"
    );
}

#[test]
fn workbench_reuses_compilation_cache_until_graph_changes() {
    let config = VnConfig::default();
    let mut workbench = EditorWorkbench::new(config);

    let start = workbench
        .node_graph
        .add_node(StoryNode::Start, egui::pos2(0.0, 0.0));
    let dialogue = workbench.node_graph.add_node(
        StoryNode::Dialogue {
            speaker: "Ava".to_string(),
            text: "Hola".to_string(),
        },
        egui::pos2(0.0, 120.0),
    );
    let end = workbench
        .node_graph
        .add_node(StoryNode::End, egui::pos2(0.0, 240.0));
    workbench.node_graph.connect(start, dialogue);
    workbench.node_graph.connect(dialogue, end);

    let _ = workbench.run_dry_validation();
    assert_eq!(workbench.compilation_cache_stats(), (0, 1));

    let _ = workbench.build_repro_case_from_current_graph();
    assert_eq!(workbench.compilation_cache_stats(), (1, 1));

    if let Some(StoryNode::Dialogue { text, .. }) = workbench.node_graph.get_node_mut(dialogue) {
        text.push('!');
    }
    workbench.node_graph.mark_modified();

    let _ = workbench.run_dry_validation();
    assert_eq!(workbench.compilation_cache_stats(), (1, 2));
}

#[test]
fn composer_created_node_connects_from_selected_node() {
    let config = VnConfig::default();
    let mut workbench = EditorWorkbench::new(config);
    let start = workbench
        .node_graph
        .add_node(StoryNode::Start, egui::pos2(0.0, 0.0));
    workbench.node_graph.selected = Some(start);

    let created = workbench.add_composer_created_node(
        StoryNode::AudioAction {
            channel: "bgm".to_string(),
            action: "play".to_string(),
            asset: Some("audio/theme.ogg".to_string()),
            volume: None,
            fade_duration_ms: None,
            loop_playback: Some(true),
        },
        egui::pos2(80.0, 120.0),
    );

    assert!(workbench
        .node_graph
        .connections()
        .any(|connection| connection.from == start && connection.to == created));
}

#[test]
fn composer_created_node_uses_next_choice_port() {
    let config = VnConfig::default();
    let mut workbench = EditorWorkbench::new(config);
    let choice = workbench.node_graph.add_node(
        StoryNode::Choice {
            prompt: "Route?".to_string(),
            options: vec!["A".to_string(), "B".to_string()],
        },
        egui::pos2(0.0, 0.0),
    );
    let first = workbench.node_graph.add_node(
        StoryNode::Dialogue {
            speaker: "Ava".to_string(),
            text: "A".to_string(),
        },
        egui::pos2(-80.0, 120.0),
    );
    workbench.node_graph.connect_port(choice, 0, first);
    workbench.node_graph.selected = Some(choice);

    let second = workbench.add_composer_created_node(
        StoryNode::Dialogue {
            speaker: "Ava".to_string(),
            text: "B".to_string(),
        },
        egui::pos2(80.0, 120.0),
    );

    assert!(workbench.node_graph.connections().any(|connection| {
        connection.from == choice && connection.from_port == 1 && connection.to == second
    }));
}

#[test]
fn workbench_can_apply_and_revert_quick_fix() {
    let config = VnConfig::default();
    let mut workbench = EditorWorkbench::new(config);

    let start = workbench
        .node_graph
        .add_node(StoryNode::Start, egui::pos2(0.0, 0.0));
    let dialogue = workbench.node_graph.add_node(
        StoryNode::Dialogue {
            speaker: "".to_string(),
            text: "Hola".to_string(),
        },
        egui::pos2(0.0, 120.0),
    );
    workbench.node_graph.connect(start, dialogue);

    let _ = workbench.run_dry_validation();
    let issue_index = workbench
        .validation_issues
        .iter()
        .position(|issue| issue.code == LintCode::EmptySpeakerName)
        .expect("expected EmptySpeakerName issue");

    workbench
        .apply_issue_fix(issue_index, "dialogue_fill_speaker")
        .expect("speaker fix should be applied");
    let Some(StoryNode::Dialogue { speaker, .. }) = workbench.node_graph.get_node(dialogue) else {
        panic!("expected dialogue node");
    };
    assert_eq!(speaker, "Narrator");
    assert!(!workbench.quick_fix_audit.is_empty());

    assert!(workbench.revert_last_fix());
    let Some(StoryNode::Dialogue { speaker, .. }) = workbench.node_graph.get_node(dialogue) else {
        panic!("expected dialogue node");
    };
    assert_eq!(speaker, "");
}

#[test]
fn workbench_diagnostic_report_json_contains_bilingual_fields() {
    let config = VnConfig::default();
    let mut workbench = EditorWorkbench::new(config);
    workbench.validation_issues.push(
        crate::editor::validator::LintIssue::error(
            Some(7),
            crate::editor::ValidationPhase::Graph,
            crate::editor::LintCode::EmptySpeakerName,
            "Speaker is empty",
        )
        .with_event_ip(Some(3)),
    );

    let payload = workbench
        .diagnostic_report_json()
        .expect("report json should be built");
    let parsed: serde_json::Value = serde_json::from_str(&payload).expect("valid json");
    let issue = &parsed["issues"][0];

    assert_eq!(
        issue["diagnostic_id"],
        "authoring-diagnostic-v2:GRAPH:VAL_SPEAKER_EMPTY:7:3:na:na:na"
    );
    assert!(issue["message_es"].as_str().is_some());
    assert!(issue["message_en"].as_str().is_some());
    assert!(issue["why_failed_es"].as_str().is_some());
    assert!(issue["why_failed_en"].as_str().is_some());
    assert_eq!(
        parsed["fingerprints"]["fingerprint_schema_version"],
        "vnengine.authoring.fingerprint.v1"
    );
    assert_eq!(
        parsed["fingerprints"]["script_sha256"]
            .as_str()
            .expect("script hash")
            .len(),
        64
    );
    assert_eq!(
        parsed["fingerprints"]["graph_sha256"]
            .as_str()
            .expect("graph hash")
            .len(),
        64
    );
}

#[test]
fn workbench_reports_missing_localization_keys() {
    let config = VnConfig::default();
    let mut workbench = EditorWorkbench::new(config);
    workbench
        .localization_catalog
        .insert_locale_table("en", std::collections::BTreeMap::new());
    workbench.player_locale = "en".to_string();

    let start = workbench
        .node_graph
        .add_node(StoryNode::Start, egui::pos2(0.0, 0.0));
    let dialogue = workbench.node_graph.add_node(
        StoryNode::Dialogue {
            speaker: "loc:speaker.narrator".to_string(),
            text: "loc:dialogue.intro".to_string(),
        },
        egui::pos2(0.0, 120.0),
    );
    workbench.node_graph.connect(start, dialogue);

    let _ = workbench.run_dry_validation();
    assert!(workbench
        .validation_issues
        .iter()
        .any(|issue| issue.message.contains("[i18n] Missing key")));
}

#[test]
fn workbench_requires_preview_for_structural_fix() {
    let config = VnConfig::default();
    let mut workbench = EditorWorkbench::new(config);

    let dialogue = workbench.node_graph.add_node(
        StoryNode::Dialogue {
            speaker: "Narrador".to_string(),
            text: "Hola".to_string(),
        },
        egui::pos2(0.0, 0.0),
    );
    assert!(workbench
        .node_graph
        .get_node(dialogue)
        .is_some_and(|node| matches!(node, StoryNode::Dialogue { .. })));

    let _ = workbench.run_dry_validation();
    let issue_index = workbench
        .validation_issues
        .iter()
        .position(|issue| issue.code == LintCode::MissingStart)
        .expect("expected MissingStart issue");

    workbench
        .prepare_structural_fix_confirmation(issue_index, "graph_add_start")
        .expect("preview for structural fix should be prepared");
    assert!(workbench.show_fix_confirm);
    assert!(workbench.pending_structural_fix.is_some());
    assert!(
        !workbench
            .node_graph
            .nodes()
            .any(|(_, node, _)| matches!(node, StoryNode::Start)),
        "structural fix must not apply before explicit confirmation"
    );

    workbench
        .apply_pending_structural_fix()
        .expect("confirmed structural fix should apply");
    assert!(
        workbench
            .node_graph
            .nodes()
            .any(|(_, node, _)| matches!(node, StoryNode::Start)),
        "start node should exist after confirmed structural fix"
    );
}

#[test]
fn report_export_import() {
    let config = VnConfig::default();
    let mut source = EditorWorkbench::new(config.clone());
    source.validation_issues.push(
        crate::editor::validator::LintIssue::warning(
            Some(12),
            crate::editor::ValidationPhase::Graph,
            crate::editor::LintCode::ChoiceOptionUnlinked,
            "Choice option 2 has no outgoing connection",
        )
        .with_event_ip(Some(4))
        .with_edge(Some(12), Some(21))
        .with_asset_path(Some("bg/room.png".to_string())),
    );
    source.selected_issue = Some(0);
    source.selected_node = Some(12);
    source.diagnostic_language = crate::editor::DiagnosticLanguage::En;

    let payload = source
        .diagnostic_report_json()
        .expect("report should serialize");

    let mut target = EditorWorkbench::new(config);
    target
        .apply_diagnostic_report_json(&payload)
        .expect("report should import");

    assert_eq!(target.validation_issues.len(), 1);
    let issue = &target.validation_issues[0];
    assert_eq!(issue.code, crate::editor::LintCode::ChoiceOptionUnlinked);
    assert_eq!(issue.edge_from, Some(12));
    assert_eq!(issue.edge_to, Some(21));
    assert_eq!(issue.asset_path.as_deref(), Some("bg/room.png"));
    assert_eq!(target.selected_issue, Some(0));
    assert_eq!(target.selected_node, Some(12));
}

#[test]
fn deep_link_to_graph_entities() {
    let config = VnConfig::default();
    let mut workbench = EditorWorkbench::new(config);
    let start = workbench
        .node_graph
        .add_node(StoryNode::Start, egui::pos2(0.0, 0.0));
    let scene = workbench.node_graph.add_node(
        StoryNode::Scene {
            profile: None,
            background: Some("bg/forest.png".to_string()),
            music: None,
            characters: Vec::new(),
        },
        egui::pos2(0.0, 120.0),
    );
    workbench.node_graph.connect(start, scene);
    let _ = workbench.sync_graph_to_script();

    let report = serde_json::json!({
        "schema": "vneditor.diagnostic_report.v1",
        "language": "es",
        "player_locale": "es",
        "selected_node": null,
        "selected_issue": 0,
        "issues": [
            {
                "phase": "GRAPH",
                "code": "VAL_ASSET_NOT_FOUND",
                "severity": "error",
                "node_id": null,
                "event_ip": null,
                "edge_from": null,
                "edge_to": null,
                "asset_path": "bg/forest.png",
                "message_es": "Asset faltante",
                "message_en": "Missing asset"
            }
        ]
    });
    let payload = serde_json::to_string(&report).expect("serialize report");
    workbench
        .apply_diagnostic_report_json(&payload)
        .expect("imported report should be valid");

    assert_eq!(workbench.selected_issue, Some(0));
    assert_eq!(workbench.selected_node, Some(scene));
}

#[test]
fn workbench_autofix_selected_issue_applies_specific_fix() {
    let config = VnConfig::default();
    let mut workbench = EditorWorkbench::new(config);

    let start = workbench
        .node_graph
        .add_node(StoryNode::Start, egui::pos2(0.0, 0.0));
    let dialogue = workbench.node_graph.add_node(
        StoryNode::Dialogue {
            speaker: "".to_string(),
            text: "Hola".to_string(),
        },
        egui::pos2(0.0, 120.0),
    );
    workbench.node_graph.connect(start, dialogue);
    let _ = workbench.run_dry_validation();

    let issue_index = workbench
        .validation_issues
        .iter()
        .position(|issue| issue.code == LintCode::EmptySpeakerName)
        .expect("expected EmptySpeakerName issue");

    let outcome = workbench
        .apply_best_fix_for_issue(issue_index, false)
        .expect("specific autofix should apply");
    assert!(outcome.contains("dialogue_fill_speaker"));

    let Some(StoryNode::Dialogue { speaker, .. }) = workbench.node_graph.get_node(dialogue) else {
        panic!("expected dialogue node");
    };
    assert_eq!(speaker, "Narrator");
}

#[test]
fn workbench_can_import_repro_case_json() {
    let config = VnConfig::default();
    let mut source = EditorWorkbench::new(config.clone());
    let start = source
        .node_graph
        .add_node(StoryNode::Start, egui::pos2(0.0, 0.0));
    let dialogue = source.node_graph.add_node(
        StoryNode::Dialogue {
            speaker: "Narrator".to_string(),
            text: "Hola".to_string(),
        },
        egui::pos2(0.0, 120.0),
    );
    source.node_graph.connect(start, dialogue);

    let case = source
        .build_repro_case_from_current_graph()
        .expect("repro case should be generated");
    let payload = case.to_json().expect("repro case should serialize");

    let mut target = EditorWorkbench::new(config);
    target
        .apply_repro_case_json(&payload)
        .expect("repro case should import");

    assert!(target.loaded_repro_case.is_some());
    assert!(target.current_script.is_some());
    assert!(target
        .node_graph
        .nodes()
        .any(|(_, node, _)| matches!(node, StoryNode::Dialogue { .. })));
}

#[test]
fn workbench_runs_loaded_repro_case_and_records_report() {
    let config = VnConfig::default();
    let mut workbench = EditorWorkbench::new(config);
    let script = visual_novel_engine::ScriptRaw::new(
        vec![visual_novel_engine::EventRaw::Dialogue(
            visual_novel_engine::DialogueRaw {
                speaker: "Narrator".to_string(),
                text: "Hola".to_string(),
            },
        )],
        std::collections::BTreeMap::from([("start".to_string(), 0usize)]),
    );
    let mut case = visual_novel_engine::ReproCase::new("editor_repro", script);
    case.oracle
        .monitors
        .push(visual_novel_engine::ReproMonitor::EventKindAtStep {
            monitor_id: "dialogue_step_0".to_string(),
            step: 0,
            expected: "dialogue".to_string(),
        });
    workbench.loaded_repro_case = Some(case);

    workbench.run_loaded_repro_case();
    let report = workbench
        .last_repro_report
        .as_ref()
        .expect("repro report should be available");
    assert!(report.oracle_triggered);
    assert!(report
        .matched_monitors
        .iter()
        .any(|id| id == "dialogue_step_0"));
}
