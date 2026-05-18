use super::*;

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
fn document_fingerprint_tracks_composer_overrides_without_semantic_stale() {
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

    let script = graph.to_script_lossy_for_diagnostics();
    let mut before_doc = AuthoringDocument::new(graph.clone());
    let mut after_doc = AuthoringDocument::new(graph);
    after_doc.composer_layer_overrides.insert(
        "node:2:Background:0:graph_nodes_2_visual_background".to_string(),
        composer::LayerOverride {
            visible: false,
            locked: true,
        },
    );

    let before = build_authoring_document_report_fingerprint(&before_doc, &script);
    let after = build_authoring_document_report_fingerprint(&after_doc, &script);

    assert_eq!(
        before.story_semantic_sha256, after.story_semantic_sha256,
        "composer-only overrides must not stale semantic diagnostic reports"
    );
    assert_ne!(
        before.layout_sha256, after.layout_sha256,
        "composer layer overrides are visual layout state"
    );
    assert_ne!(
        before.full_document_sha256, after.full_document_sha256,
        "saved authoring document metadata must be fingerprinted"
    );

    before_doc.operation_log.push(OperationLogEntry::new_typed(
        OperationKind::LayerVisibilityChanged,
        "applied",
        "layer visibility changed",
    ));
    let with_log = build_authoring_document_report_fingerprint(&before_doc, &script);
    assert_eq!(before.story_semantic_sha256, with_log.story_semantic_sha256);
    assert_eq!(before.layout_sha256, with_log.layout_sha256);
    assert_ne!(before.full_document_sha256, with_log.full_document_sha256);
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
fn evidence_trace_chain_is_connected_for_diagnostics_without_semantic_values() {
    let graph = NodeGraph::new();
    let issue = validate_authoring_graph_no_io(&graph)
        .into_iter()
        .find(|issue| issue.code == LintCode::MissingStart)
        .expect("missing start diagnostic");
    let trace = issue.evidence_trace.expect("evidence trace");
    let atom_ids = trace
        .atoms
        .iter()
        .map(|atom| atom.atom_id.as_str())
        .collect::<std::collections::BTreeSet<_>>();

    for required in [
        "operation_applied",
        "field_changed",
        "resolver_lookup",
        "rule_evaluated",
        "failure",
        "runtime_consequence",
        "fix_suggested",
    ] {
        assert!(atom_ids.contains(required), "missing atom {required}");
    }
    for (from, to) in [
        ("operation_applied", "field_changed"),
        ("field_changed", "resolver_lookup"),
        ("resolver_lookup", "rule_evaluated"),
        ("rule_evaluated", "failure"),
        ("failure", "runtime_consequence"),
        ("failure", "fix_suggested"),
    ] {
        assert!(
            trace
                .edges
                .iter()
                .any(|edge| edge.from == from && edge.to == to),
            "missing trace edge {from} -> {to}"
        );
    }
}

#[test]
fn validation_report_imports_legacy_v1_as_untrusted_v2() {
    let payload = serde_json::json!({
        "schema": "vneditor.diagnostic_report.v1",
        "issues": [
            {
                "phase": "GRAPH",
                "code": "VAL_ASSET_NOT_FOUND",
                "severity": "error",
                "asset_path": "bg/missing.png",
                "message_es": "Asset faltante",
                "message_en": "Missing asset"
            }
        ]
    });

    let report = AuthoringValidationReport::from_json(&payload.to_string())
        .expect("legacy report should import");

    assert_eq!(report.schema, "vnengine.authoring_validation_report.v2");
    assert_eq!(report.error_count, 1);
    assert_eq!(
        report.fingerprints.story_semantic_sha256,
        "legacy-untrusted"
    );
    assert_eq!(report.issues[0].code, "VAL_ASSET_NOT_FOUND");
    assert_eq!(
        report.issues[0].message_args["legacy_schema"],
        "vneditor.diagnostic_report.v1"
    );
}
