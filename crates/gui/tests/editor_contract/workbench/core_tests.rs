use super::super::*;
use crate::editor::StoryNode;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

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
fn composer_node_selection_schedules_graph_focus() {
    let mut workbench = EditorWorkbench::new(VnConfig::default());
    let scene = workbench.node_graph.add_node(
        StoryNode::Scene {
            profile: None,
            background: Some("bg.png".to_string()),
            music: None,
            characters: Vec::new(),
        },
        eframe::egui::pos2(1200.0, 900.0),
    );

    workbench.handle_composer_actions(
        vec![crate::editor::visual_composer::VisualComposerAction::SelectNode(
            scene,
        )],
        None,
    );

    assert_eq!(workbench.node_graph.selected, Some(scene));
    assert_eq!(workbench.selected_node, Some(scene));
    assert_eq!(workbench.pending_graph_focus, Some(scene));
}

#[test]
fn workbench_update_uses_supplied_delta_ticks_and_seconds() {
    let mut workbench = EditorWorkbench::new(VnConfig::default());
    let mut track = visual_novel_engine::Track::new(
        visual_novel_engine::EntityId::new(2),
        visual_novel_engine::PropertyType::PositionY,
    );
    track
        .add_keyframe(visual_novel_engine::Keyframe::new(
            120,
            0,
            visual_novel_engine::Easing::Linear,
        ))
        .unwrap();
    workbench.timeline.add_track(track).unwrap();
    workbench.is_playing = true;

    workbench.update(30);
    assert_eq!(workbench.timeline.current_time(), 30);
    workbench.update_seconds(0.5);
    assert_eq!(workbench.timeline.current_time(), 60);
}

#[test]
fn export_wizard_initializes_project_defaults_without_executing() {
    let mut workbench = EditorWorkbench::new(VnConfig::default());
    let tmp = tempfile::tempdir().expect("tempdir");
    workbench.project_root = Some(tmp.path().to_path_buf());
    workbench.manifest = Some(visual_novel_engine::ProjectManifest::new("game", "studio"));

    workbench.open_export_wizard();

    assert!(workbench.show_export_wizard);
    assert!(workbench.export_wizard.dry_run);
    assert!(workbench
        .export_wizard
        .output_root
        .contains(workbench.export_wizard.target.as_str()));
    assert_eq!(workbench.export_wizard.entry_script, "main.json");
}

fn minimal_pe_exe() -> Vec<u8> {
    let mut bytes = vec![0u8; 128];
    bytes[0..2].copy_from_slice(b"MZ");
    bytes[0x3c..0x40].copy_from_slice(&0x40u32.to_le_bytes());
    bytes[0x40..0x44].copy_from_slice(b"PE\0\0");
    bytes[0x44..0x46].copy_from_slice(&0x8664u16.to_le_bytes());
    bytes
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>()
}

fn read_json(path: &Path) -> serde_json::Value {
    serde_json::from_str(&fs::read_to_string(path).expect("json file")).expect("json")
}

#[test]
fn export_wizard_report_summary_matches_real_export_flow_outputs() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let project_root = tmp.path().join("project");
    fs::create_dir_all(project_root.join("assets/bgm")).expect("assets dir");
    fs::create_dir_all(project_root.join("runtime")).expect("runtime dir");

    visual_novel_engine::ProjectManifest::new("game", "studio")
        .save(&project_root.join("project.vnm"))
        .expect("manifest save");
    let script = visual_novel_engine::runtime::ScriptRaw::new(
        vec![
            visual_novel_engine::runtime::EventRaw::Dialogue(
                visual_novel_engine::runtime::DialogueRaw {
                    speaker: "Narrator".to_string(),
                    text: "hello from export".to_string(),
                },
            ),
            visual_novel_engine::runtime::EventRaw::AudioAction(
                visual_novel_engine::runtime::AudioActionRaw {
                    channel: "bgm".to_string(),
                    action: "play".to_string(),
                    asset: Some("assets/bgm/theme.ogg".to_string()),
                    volume: Some(0.8),
                    fade_duration_ms: None,
                    loop_playback: Some(true),
                },
            ),
        ],
        BTreeMap::from([("start".to_string(), 0)]),
    );
    fs::write(
        project_root.join("main.json"),
        script.to_json().expect("script json"),
    )
    .expect("script write");
    fs::write(project_root.join("assets/bgm/theme.ogg"), [1u8, 2, 3, 4]).expect("asset");
    let runtime_bytes = minimal_pe_exe();
    fs::write(project_root.join("runtime/vn_player.exe"), &runtime_bytes).expect("runtime");

    let output_root = project_root.join("dist");
    let report = visual_novel_engine::export_bundle(visual_novel_engine::ExportBundleSpec {
        project_root: project_root.clone(),
        output_root: output_root.clone(),
        target_platform: visual_novel_engine::ExportTargetPlatform::Windows,
        entry_script: None,
        runtime_artifact: Some(std::path::PathBuf::from("runtime/vn_player.exe")),
        integrity: visual_novel_engine::BundleIntegrity::HmacSha256,
        output_layout_version: 1,
        hmac_key: Some("wizard-flow-secret".to_string()),
    })
    .expect("export flow");

    let package_report = read_json(&output_root.join("meta/package_report.json"));
    let compat_report = read_json(&output_root.join("meta/compat_report.json"));
    let bundle_manifest = read_json(&output_root.join("meta/bundle_file_manifest.json"));
    let runtime_hash = sha256_hex(&runtime_bytes);
    let signature = fs::read_to_string(output_root.join("meta/bundle.hmac_sha256"))
        .expect("bundle signature");

    assert_eq!(
        package_report,
        serde_json::to_value(&report).expect("report json"),
        "wizard summary should be driven by the same report written to disk"
    );
    assert_eq!(package_report["runtime_artifact"], "runtime/vn_player.exe");
    assert_eq!(package_report["runtime_artifact_sha256"], runtime_hash);
    assert_eq!(compat_report["runtime_artifact_sha256"], runtime_hash);
    assert_eq!(package_report["generator_os"], compat_report["generator_os"]);
    assert_eq!(
        package_report["expected_executable"],
        compat_report["expected_executable"]
    );
    assert_eq!(
        package_report["graphics_backend"],
        compat_report["graphics_backend"]
    );
    assert_eq!(package_report["wgpu_fallback"], compat_report["wgpu_fallback"]);
    assert_eq!(package_report["total_size"], compat_report["total_size"]);
    assert_eq!(package_report["hashes"], bundle_manifest["files"]);
    assert_eq!(compat_report["hashes"], bundle_manifest["files"]);
    assert_eq!(
        package_report["bundle_file_manifest_sha256"],
        compat_report["bundle_file_manifest_sha256"]
    );
    assert_eq!(package_report["bundle_hmac_sha256"], signature);
    assert_eq!(compat_report["bundle_hmac_sha256"], signature);
    assert_eq!(
        package_report["smoke_result"],
        compat_report["smoke_result"],
        "package and compat reports must expose the same smoke state"
    );
    assert!(bundle_manifest["files"]
        .as_array()
        .expect("manifest files")
        .iter()
        .any(|entry| entry["path"] == "runtime/vn_player.exe"
            && entry["sha256"] == runtime_hash));

    let lines = EditorWorkbench::export_report_summary_lines(&report);
    assert!(lines
        .iter()
        .any(|line| line == &format!("Runtime: runtime/vn_player.exe sha256={runtime_hash}")));
    assert!(lines
        .iter()
        .any(|line| line == "Expected executable: game.exe"));
    assert!(lines
        .iter()
        .any(|line| line == "Backend: software fallback=true"));
    assert!(lines.iter().any(|line| {
        line == &format!(
            "Payload: files={} total_size={}",
            bundle_manifest["files"].as_array().expect("manifest files").len(),
            package_report["total_size"].as_u64().expect("total size")
        )
    }));
    assert!(lines.iter().any(|line| line
        == &format!(
            "Manifest: meta/bundle_file_manifest.json sha256={}",
            package_report["bundle_file_manifest_sha256"]
                .as_str()
                .expect("manifest hash")
        )));
    assert!(lines
        .iter()
        .any(|line| line == "Compat: meta/compat_report.json"));
    assert!(lines
        .iter()
        .any(|line| line == &format!("Integrity: hmac_sha256 scope=bundle_file_manifest_v2_signed_manifest_covers_payload_files hmac={signature}")));
    assert!(lines
        .iter()
        .any(|line| line.starts_with("Smoke: status=not_run backend=software trace_id=export-smoke-")));
}

#[test]
fn play_mode_refuses_empty_workbench_without_stale_preview_state() {
    let config = VnConfig::default();
    let mut workbench = EditorWorkbench::new(config);
    workbench.current_script = Some(visual_novel_engine::runtime::ScriptRaw::new(
        Vec::new(),
        std::collections::BTreeMap::new(),
    ));
    workbench
        .scene
        .spawn(visual_novel_engine::EntityKind::Image(
            visual_novel_engine::ImageData {
                path: visual_novel_engine::runtime::SharedStr::from("bg/stale.png"),
                tint: None,
            },
        ));
    workbench.composer_entity_owners.insert(1, 42);

    assert!(!workbench.prepare_player_mode());

    assert!(workbench.engine.is_none());
    assert!(workbench.current_script.is_none());
    assert!(workbench.scene.is_empty());
    assert!(workbench.composer_entity_owners.is_empty());
    assert!(workbench.selected_entity.is_none());
    let message = workbench
        .toast
        .as_ref()
        .map(|toast| toast.message.as_str())
        .unwrap_or_default();
    assert!(
        message.contains("Abre o crea un proyecto"),
        "unexpected toast: {message}"
    );
}

#[test]
fn player_visual_preferences_follow_composer_fit_and_quality() {
    let config = VnConfig::default();
    let mut workbench = EditorWorkbench::new(config);
    workbench.composer_preview_quality = crate::editor::PreviewQuality::High;
    workbench.composer_stage_fit = crate::editor::StageFit::Compact;

    let prefs = workbench.player_visual_preferences();

    assert_eq!(prefs.preview_quality, crate::editor::PreviewQuality::High);
    assert_eq!(prefs.stage_fit, crate::editor::StageFit::Compact);
}

#[test]
fn workspace_layout_changes_are_persisted_without_dirtying_story_graph() {
    let config = VnConfig::default();
    let mut workbench = EditorWorkbench::new(config);
    workbench.node_graph.clear_modified();

    workbench.show_graph = false;
    workbench.show_validation = true;
    workbench.validation_collapsed = true;
    workbench.node_editor_window_open = true;
    workbench.sync_workspace_layout_from_flags();
    workbench.layout_overrides.graph_width = Some(480.0);
    workbench.apply_layout_size_overrides();

    assert!(
        !workbench.node_graph.is_modified(),
        "layout-only changes must not dirty the semantic graph"
    );
    let prefs = workbench.collect_layout_prefs();
    assert!(
        !prefs
            .workspace_layout
            .panel(super::super::layout::WorkspacePanelId::Graph)
            .visible
    );
    assert!(
        prefs
            .workspace_layout
            .panel(super::super::layout::WorkspacePanelId::Validation)
            .collapsed
    );
    assert!(
        prefs
            .workspace_layout
            .panel(super::super::layout::WorkspacePanelId::NodeEditor)
            .visible
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
fn dry_validation_phase_trace_uses_distinct_identity_not_dry_finished() {
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

    assert!(workbench.run_dry_validation());

    assert!(
        !workbench.validation_issues.iter().any(|issue| {
            issue.phase == ValidationPhase::Graph && issue.code == LintCode::DryRunFinished
        }),
        "GRAPH phase traces must not reuse DRY_FINISHED identity"
    );

    let phase_trace_issues = workbench
        .validation_issues
        .iter()
        .filter(|issue| issue.code == LintCode::PhaseTraceOk)
        .collect::<Vec<_>>();
    assert!(
        phase_trace_issues.len() >= 2,
        "expected graph phase trace diagnostics"
    );
    let ids = phase_trace_issues
        .iter()
        .map(|issue| issue.diagnostic_id())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        ids.len(),
        phase_trace_issues.len(),
        "phase trace diagnostic IDs must be unique per pipeline phase"
    );
    assert!(ids.iter().any(|id| {
        id.starts_with("authoring-diagnostic-v2:GRAPH:TRACE_PHASE_OK:scope=global")
            && id.contains("field=phase_trace_GRAPH_SYNC")
    }));
    assert!(ids.iter().any(|id| {
        id.starts_with("authoring-diagnostic-v2:GRAPH:TRACE_PHASE_OK:scope=global")
            && id.contains("field=phase_trace_GRAPH_VALIDATION")
    }));

    let graph_sync = phase_trace_issues
        .iter()
        .find(|issue| {
            issue
                .field_path
                .as_ref()
                .is_some_and(|path| path.value == "phase_trace.GRAPH_SYNC")
        })
        .expect("GRAPH_SYNC phase trace");
    let explanation = graph_sync.explanation(DiagnosticLanguage::Es);
    assert_eq!(explanation.title, "Fase completada");
    assert!(!explanation.root_cause.contains("EndOfScript"));
    assert!(explanation.why_failed.starts_with("No fallo"));
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
fn report_import_rejects_malformed_diagnostic_target() {
    let config = VnConfig::default();
    let mut workbench = EditorWorkbench::new(config);
    let report = serde_json::json!({
        "schema": "vnengine.authoring_validation_report.v2",
        "issues": [
            {
                "phase": "GRAPH",
                "code": "VAL_ASSET_NOT_FOUND",
                "severity": "error",
                "message": "Imported malformed target",
                "target": {
                    "target_kind": "node",
                    "node_id": "not-a-node-id"
                }
            }
        ]
    });
    let payload = serde_json::to_string(&report).expect("serialize report");

    let err = workbench
        .apply_diagnostic_report_json(&payload)
        .expect_err("malformed target should not import silently");

    assert!(err.contains("issue 0 target is invalid"), "{err}");
    assert!(workbench.validation_issues.is_empty());
}

#[test]
fn report_import_rejects_malformed_issue_location_numbers() {
    let config = VnConfig::default();
    let mut workbench = EditorWorkbench::new(config);
    let report = serde_json::json!({
        "schema": "vnengine.authoring_validation_report.v2",
        "issues": [
            {
                "phase": "GRAPH",
                "code": "VAL_ASSET_NOT_FOUND",
                "severity": "error",
                "message": "Imported malformed node id",
                "node_id": "12"
            }
        ]
    });
    let payload = serde_json::to_string(&report).expect("serialize report");

    let err = workbench
        .apply_diagnostic_report_json(&payload)
        .expect_err("string node_id should not import as missing location");

    assert!(err.contains("issue 0 node_id must be an unsigned integer"), "{err}");
    assert!(workbench.validation_issues.is_empty());
}

#[test]
fn report_import_rejects_malformed_selection_fields() {
    let config = VnConfig::default();
    let mut workbench = EditorWorkbench::new(config);
    let report = serde_json::json!({
        "schema": "vnengine.authoring_validation_report.v2",
        "selected_issue": "0",
        "issues": []
    });
    let payload = serde_json::to_string(&report).expect("serialize report");

    let err = workbench
        .apply_diagnostic_report_json(&payload)
        .expect_err("string selected_issue should not be silently ignored");

    assert!(err.contains("selected_issue must be an unsigned integer"), "{err}");
    assert!(workbench.validation_issues.is_empty());
    assert_eq!(workbench.selected_issue, None);
}

#[test]
fn report_import_rejects_malformed_semantic_values() {
    let config = VnConfig::default();
    let mut workbench = EditorWorkbench::new(config);
    let report = serde_json::json!({
        "schema": "vnengine.authoring_validation_report.v2",
        "issues": [
            {
                "phase": "GRAPH",
                "code": "VAL_ASSET_NOT_FOUND",
                "severity": "error",
                "message": "Imported malformed semantic values",
                "semantic_values": "asset:bg/missing.png"
            }
        ]
    });
    let payload = serde_json::to_string(&report).expect("serialize report");

    let err = workbench
        .apply_diagnostic_report_json(&payload)
        .expect_err("malformed semantic values should not import silently");

    assert!(err.contains("issue 0 semantic_values must be an array"), "{err}");
    assert!(workbench.validation_issues.is_empty());
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
        "schema": format!("vneditor.diagnostic_report.{}", "v1"),
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
    let err = workbench
        .apply_diagnostic_report_json(&payload)
        .expect_err("legacy report should be rejected");
    assert!(err.contains("unsupported report schema"));
    assert_eq!(workbench.selected_issue, None);
    assert_ne!(workbench.selected_node, Some(scene));
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

#[path = "core_tests/repro.rs"]
mod repro;
