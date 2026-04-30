use super::*;
use crate::EventRaw;

fn pos(x: f32, y: f32) -> AuthoringPosition {
    AuthoringPosition::new(x, y)
}

#[test]
fn diagnostic_catalog_is_specific_and_docs_refs_exist() {
    let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let docs = std::fs::read_to_string(repo_root.join("docs/diagnostics/authoring.md"))
        .expect("diagnostic docs should exist");
    let generic_root = "The authoring graph violates the semantic contract.";

    for code in LintCode::ALL {
        let issue = LintIssue::warning(
            Some(7),
            ValidationPhase::Graph,
            *code,
            "actual diagnostic detail",
        )
        .with_event_ip(Some(3))
        .with_asset_path(Some("assets/example.png".to_string()));
        let en = issue.explanation(DiagnosticLanguage::En);
        let es = issue.explanation(DiagnosticLanguage::Es);

        assert_ne!(
            en.root_cause, generic_root,
            "generic root cause for {code:?}"
        );
        assert!(
            !en.action_steps.is_empty(),
            "missing action steps for {code:?}"
        );
        assert!(
            !es.action_steps.is_empty(),
            "missing ES action steps for {code:?}"
        );
        assert!(
            en.docs_ref.starts_with("docs/diagnostics/authoring.md#"),
            "bad docs ref for {code:?}: {}",
            en.docs_ref
        );
        let anchor = en.docs_ref.split('#').nth(1).expect("docs anchor");
        assert!(
            docs.contains(&format!("## {anchor}")),
            "missing docs anchor {anchor} for {code:?}"
        );

        let envelope = issue.envelope_v2();
        assert_eq!(envelope.schema, "vnengine.diagnostic_envelope.v2");
        assert_eq!(envelope.docs_ref, en.docs_ref);
        assert_eq!(envelope.text_en.message_key, en.message_key);
        assert_eq!(
            envelope.message_args.get("asset_path").map(String::as_str),
            Some("assets/example.png")
        );
    }
}

#[test]
fn dry_run_reports_extcall_as_simulated_capability() {
    let mut graph = NodeGraph::new();
    let start = graph.add_node(StoryNode::Start, pos(0.0, 0.0));
    let ext = graph.add_node(
        StoryNode::Generic(EventRaw::ExtCall {
            command: "plugin.fade".to_string(),
            args: Vec::new(),
        }),
        pos(0.0, 90.0),
    );
    let end = graph.add_node(StoryNode::End, pos(0.0, 180.0));
    graph.connect(start, ext);
    graph.connect(ext, end);

    let result = compiler::compile_authoring_graph(&graph, None);

    assert!(result.issues.iter().any(|issue| {
        issue.code == LintCode::DryRunExtCallSimulated
            && issue.severity == LintSeverity::Warning
            && issue.event_ip == Some(0)
    }));
    let report = result.dry_run_report.expect("dry-run report");
    assert!(report.steps.iter().any(|step| {
        step.event_kind == "ext_call"
            && step.simulation_note.as_deref() == Some("external_call_simulated")
    }));
}

#[test]
fn verification_run_tracks_resolved_and_introduced_diagnostics() {
    let graph = NodeGraph::new();
    let script = graph.to_script_lossy_for_diagnostics();
    let fingerprint = build_authoring_report_fingerprint(&graph, &script);
    let before = vec![
        LintIssue::error(
            Some(1),
            ValidationPhase::Graph,
            LintCode::MissingStart,
            "missing start",
        ),
        LintIssue::warning(
            Some(2),
            ValidationPhase::Graph,
            LintCode::UnreachableNode,
            "unreachable",
        ),
    ];
    let after = vec![LintIssue::warning(
        Some(3),
        ValidationPhase::DryRun,
        LintCode::DryRunExtCallSimulated,
        "simulated extcall",
    )];

    let run = VerificationRun::from_diagnostics("op-1", "contract", &fingerprint, &before, &after);

    assert_eq!(run.schema, "vnengine.verification_run.v1");
    assert_eq!(run.operation_id, "op-1");
    assert_eq!(run.semantic_fingerprint_sha256, fingerprint.semantic_sha256);
    assert_eq!(run.diagnostic_ids.len(), 1);
    assert_eq!(run.resolved_diagnostic_ids.len(), 2);
    assert_eq!(run.introduced_diagnostic_ids.len(), 1);

    let log = OperationLogEntry::new("op-1", "quick_fix", "applied", "fixed")
        .with_diagnostic(&before[0])
        .with_fingerprint(&fingerprint);
    assert_eq!(log.schema, "vnengine.operation_log.v1");
    assert_eq!(log.diagnostic_id, Some(before[0].diagnostic_id()));
    assert_eq!(
        log.semantic_fingerprint_sha256.as_deref(),
        Some(fingerprint.semantic_sha256.as_str())
    );
}

#[test]
fn granular_targets_make_same_node_choice_diagnostics_distinct() {
    let mut graph = NodeGraph::new();
    let start = graph.add_node(StoryNode::Start, pos(0.0, 0.0));
    let choice = graph.add_node(
        StoryNode::Choice {
            prompt: "Route?".to_string(),
            options: vec!["Option 1".to_string(), "Option 2".to_string()],
        },
        pos(0.0, 90.0),
    );
    graph.connect(start, choice);

    let ids = validate_authoring_graph_no_io(&graph)
        .into_iter()
        .filter(|issue| issue.code == LintCode::PlaceholderChoiceOption)
        .map(|issue| issue.diagnostic_id())
        .collect::<std::collections::BTreeSet<_>>();

    assert_eq!(ids.len(), 2, "each placeholder option needs its own id");
    assert!(ids
        .iter()
        .any(|id| id.contains("choice_1_option_0") || id.contains("choice_")));

    let mut graph = NodeGraph::new();
    graph.add_node(StoryNode::Start, pos(0.0, 0.0));
    graph.add_node(
        StoryNode::Scene {
            profile: None,
            background: Some("bg/missing.png".to_string()),
            music: Some("audio/missing.ogg".to_string()),
            characters: Vec::new(),
        },
        pos(0.0, 90.0),
    );
    let asset_ids = validate_authoring_graph_with_resolver(&graph, |_asset| false)
        .into_iter()
        .filter(|issue| issue.code == LintCode::AssetReferenceMissing)
        .map(|issue| issue.diagnostic_id())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        asset_ids.len(),
        2,
        "background and music refs in one node need separate ids"
    );
}

#[test]
fn fingerprints_split_story_layout_assets_and_document_hashes() {
    let mut graph = NodeGraph::new();
    let start = graph.add_node(StoryNode::Start, pos(0.0, 0.0));
    let scene = graph.add_node(
        StoryNode::Scene {
            profile: None,
            background: Some("bg/room.png".to_string()),
            music: None,
            characters: Vec::new(),
        },
        pos(0.0, 90.0),
    );
    graph.connect(start, scene);
    let before =
        build_authoring_report_fingerprint(&graph, &graph.to_script_lossy_for_diagnostics());

    graph.set_node_pos(scene, pos(500.0, 600.0));
    let after_layout =
        build_authoring_report_fingerprint(&graph, &graph.to_script_lossy_for_diagnostics());

    assert_eq!(
        before.story_semantic_sha256, after_layout.story_semantic_sha256,
        "moving nodes must not stale semantic reports"
    );
    assert_ne!(before.layout_sha256, after_layout.layout_sha256);
    assert_ne!(
        before.full_document_sha256,
        after_layout.full_document_sha256
    );

    if let Some(StoryNode::Scene { background, .. }) = graph.get_node_mut(scene) {
        *background = Some("bg/other.png".to_string());
    }
    let after_story =
        build_authoring_report_fingerprint(&graph, &graph.to_script_lossy_for_diagnostics());
    assert_ne!(
        after_layout.story_semantic_sha256,
        after_story.story_semantic_sha256
    );
    assert_ne!(after_layout.assets_sha256, after_story.assets_sha256);
}

#[test]
fn evidence_trace_explains_asset_jump_and_generic_failures() {
    let mut graph = NodeGraph::new();
    let start = graph.add_node(StoryNode::Start, pos(0.0, 0.0));
    let scene = graph.add_node(
        StoryNode::Scene {
            profile: None,
            background: Some("bg/missing.png".to_string()),
            music: None,
            characters: Vec::new(),
        },
        pos(0.0, 90.0),
    );
    let jump = graph.add_node(
        StoryNode::Jump {
            target: "missing_label".to_string(),
        },
        pos(0.0, 180.0),
    );
    let generic = graph.add_node(
        StoryNode::Generic(EventRaw::Jump {
            target: "start".to_string(),
        }),
        pos(0.0, 270.0),
    );
    graph.connect(start, scene);
    graph.connect(scene, jump);
    graph.connect(jump, generic);

    let issues = validate_authoring_graph_with_resolver(&graph, |_asset| false);
    for code in [
        LintCode::AssetReferenceMissing,
        LintCode::MissingJumpTarget,
        LintCode::ContractUnsupportedExport,
    ] {
        let issue = issues
            .iter()
            .find(|issue| issue.code == code)
            .unwrap_or_else(|| panic!("expected {code:?}"));
        let envelope = issue.envelope_v2();
        assert!(
            envelope.target.is_some(),
            "{code:?} should include a diagnostic target"
        );
        assert!(
            envelope.evidence_trace.is_some(),
            "{code:?} should include evidence"
        );
    }
}

#[test]
fn strict_export_blocks_unreachable_drafts_and_generic_payloads() {
    let mut graph = NodeGraph::new();
    let start = graph.add_node(StoryNode::Start, pos(0.0, 0.0));
    let live = graph.add_node(
        StoryNode::Dialogue {
            speaker: "Ava".to_string(),
            text: "Live".to_string(),
        },
        pos(0.0, 90.0),
    );
    let end = graph.add_node(StoryNode::End, pos(0.0, 180.0));
    graph.connect(start, live);
    graph.connect(live, end);
    graph.add_node(
        StoryNode::Dialogue {
            speaker: "Draft".to_string(),
            text: "Not connected".to_string(),
        },
        pos(300.0, 90.0),
    );
    let err = graph
        .to_script_strict()
        .expect_err("unreachable draft must block strict export");
    assert!(err.to_string().contains("unreachable"));

    let mut graph = NodeGraph::new();
    let start = graph.add_node(StoryNode::Start, pos(0.0, 0.0));
    let generic = graph.add_node(
        StoryNode::Generic(EventRaw::Jump {
            target: "start".to_string(),
        }),
        pos(0.0, 90.0),
    );
    graph.connect(start, generic);
    let err = graph
        .to_script_strict()
        .expect_err("unsupported generic must block strict export");
    assert!(err.to_string().contains("not export-supported"));
}

#[test]
fn graph_fragments_are_stable_authoring_metadata() {
    let mut graph = NodeGraph::new();
    let start = graph.add_node(StoryNode::Start, pos(0.0, 0.0));
    let choice = graph.add_node(
        StoryNode::Choice {
            prompt: "Loop?".to_string(),
            options: vec!["Again".to_string(), "End".to_string()],
        },
        pos(0.0, 90.0),
    );
    let end = graph.add_node(StoryNode::End, pos(0.0, 180.0));
    graph.connect(start, choice);
    graph.connect_port(choice, 0, choice);
    graph.connect_port(choice, 1, end);

    assert!(graph.create_fragment("intro_loop", "Intro Loop", vec![choice, start, choice]));
    let fragment = graph.fragment("intro_loop").unwrap();
    assert_eq!(fragment.node_ids, vec![start, choice]);
    assert!(fragment.inputs.is_empty());
    assert_eq!(fragment.outputs.len(), 1);
    assert_eq!(fragment.outputs[0].node_id, Some(choice));

    let script = graph.to_script();
    assert!(script.labels.contains_key("fragment_intro_loop_node_1"));
    assert!(script
        .labels
        .keys()
        .any(|label| label.starts_with("fragment_intro_loop_port_out_")));
    let flow = graph.flow_analysis(&[start]);
    assert!(flow.reachable.contains(&choice));
    assert!(flow.reachable_cycle_nodes.contains(&choice));
}
