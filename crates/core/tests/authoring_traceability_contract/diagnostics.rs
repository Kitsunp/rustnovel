use super::*;

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
fn dry_run_reports_extcall_as_host_required_blocker() {
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
        issue.code == LintCode::DryRunExtCallBlocked
            && issue.severity == LintSeverity::Warning
            && issue.event_ip == Some(0)
    }));
    let report = result.dry_run_report.expect("dry-run report");
    assert_eq!(
        report.stop_reason,
        visual_novel_engine::authoring::compiler::DryRunStopReason::ExternalCallBlocked
    );
    assert!(report.steps.iter().any(|step| {
        step.event_kind == "ext_call"
            && step.execution_note.as_deref() == Some("external_call_requires_host:plugin.fade")
            && step.simulation_note.is_none()
    }));
}

#[test]
fn extcall_host_required_fidelity_contract() {
    let mut graph = NodeGraph::new();
    let start = graph.add_node(StoryNode::Start, pos(0.0, 0.0));
    let ext_node = StoryNode::Generic(EventRaw::ExtCall {
        command: "plugin.fade".to_string(),
        args: vec!["fast".to_string()],
    });
    let ext = graph.add_node(ext_node.clone(), pos(0.0, 90.0));
    let end = graph.add_node(StoryNode::End, pos(0.0, 180.0));
    graph.connect(start, ext);
    graph.connect(ext, end);

    let runtime_contract = crate::runtime::contract_for_authoring_node(&ext_node);
    assert_eq!(
        runtime_contract.fidelity,
        crate::runtime::FidelityClass::HostRequired
    );
    assert!(runtime_contract.runtime_supported);
    assert!(!runtime_contract.export_supported);

    let result = compiler::compile_authoring_graph(&graph, None);
    let dry_run = result.dry_run_report.expect("dry-run report");
    let dry_step = dry_run
        .steps
        .iter()
        .find(|step| step.event_kind == "ext_call")
        .expect("extcall step");
    assert_eq!(
        dry_step.execution_fidelity,
        crate::runtime::FidelityClass::HostRequired
    );
    assert_eq!(
        dry_step.execution_note.as_deref(),
        Some("external_call_requires_host:plugin.fade")
    );
    assert!(dry_step.simulation_note.is_none());

    let repro = crate::ReproCase::new("extcall fidelity", result.script);
    let repro_report = crate::run_repro_case(&repro);
    let repro_step = repro_report
        .steps
        .iter()
        .find(|step| step.event_kind == "ext_call")
        .expect("repro extcall step");
    assert_eq!(
        repro_step.execution_fidelity,
        crate::runtime::FidelityClass::HostRequired
    );
    assert_eq!(
        repro_step.execution_note.as_deref(),
        Some("external_call_requires_host:plugin.fade")
    );
    assert!(repro_step.simulation_note.is_none());
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
        LintCode::DryRunExtCallBlocked,
        "host-required extcall",
    )];

    let run = VerificationRun::from_diagnostics("op-1", "contract", &fingerprint, &before, &after);

    assert_eq!(run.schema, "vnengine.verification_run.v2");
    assert_eq!(run.operation_id, "op-1");
    assert_eq!(run.semantic_fingerprint_sha256, fingerprint.semantic_sha256);
    assert_eq!(run.diagnostic_ids.len(), 1);
    assert_eq!(run.resolved_diagnostic_ids.len(), 2);
    assert_eq!(run.introduced_diagnostic_ids.len(), 1);

    let log = OperationLogEntry::new(
        "op-1",
        OperationKind::QuickFixApplied,
        OperationStatus::Applied,
        "fixed",
    )
    .with_diagnostic(&before[0])
    .with_fingerprint(&fingerprint);
    assert_eq!(log.schema, "vnengine.operation_log.v2");
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
