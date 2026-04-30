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
