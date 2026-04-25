use super::*;

#[test]
fn import_issues_include_traceability_fields() {
    let dir = tempdir().expect("tempdir");
    let project_root = dir.path().join("renpy_project");
    let game_dir = project_root.join("game");
    fs::create_dir_all(&game_dir).expect("mkdir game");
    fs::write(
        game_dir.join("script.rpy"),
        r#"
label start:
    return
"#,
    )
    .expect("write script");

    let output_root = dir.path().join("out_project");
    let report = import_renpy_project(ImportRenpyOptions {
        project_root,
        output_root,
        entry_label: "start".to_string(),
        report_path: None,
        profile: ImportProfile::StoryFirst,
        include_tl: None,
        include_ui: None,
        include_patterns: Vec::new(),
        exclude_patterns: Vec::new(),
        strict_mode: false,
        fallback_policy: super::super::ImportFallbackPolicy::DegradeWithTrace,
    })
    .expect("import");

    assert!(
        !report.issues.is_empty(),
        "import should produce diagnostics"
    );
    let traces: BTreeSet<String> = report
        .issues
        .iter()
        .map(|issue| issue.trace_id.clone())
        .collect();
    assert_eq!(traces.len(), report.issues.len(), "trace_id must be unique");
    assert!(report
        .issues
        .iter()
        .all(|issue| issue.trace_id.starts_with("imp-")));
    assert!(report
        .issues
        .iter()
        .all(|issue| !issue.root_cause.trim().is_empty()));
    assert!(report
        .issues
        .iter()
        .all(|issue| issue.root_cause.contains("area=") && issue.root_cause.contains("phase=")));
    assert!(report
        .issues
        .iter()
        .all(|issue| !issue.how_to_fix.trim().is_empty()));
    assert!(report
        .issues
        .iter()
        .all(|issue| !issue.docs_ref.trim().is_empty()));
    assert!(report
        .issues
        .iter()
        .all(|issue| issue.docs_ref.ends_with(&issue.code)));
}

#[test]
fn ext_call_events_are_decorated_with_trace_envelope_v2() {
    let dir = tempdir().expect("tempdir");
    let project_root = dir.path().join("renpy_project");
    let game_dir = project_root.join("game");
    fs::create_dir_all(&game_dir).expect("mkdir game");
    fs::write(
        game_dir.join("script.rpy"),
        r#"
label start:
    call route_a
    queue music "audio/theme.ogg"
    return
"#,
    )
    .expect("write script");

    let output_root = dir.path().join("out_project");
    let report = import_renpy_project(ImportRenpyOptions {
        project_root,
        output_root: output_root.clone(),
        entry_label: "start".to_string(),
        report_path: None,
        profile: ImportProfile::StoryFirst,
        include_tl: None,
        include_ui: None,
        include_patterns: Vec::new(),
        exclude_patterns: Vec::new(),
        strict_mode: false,
        fallback_policy: super::super::ImportFallbackPolicy::DegradeWithTrace,
    })
    .expect("import");

    let fallback_issue_traces: BTreeSet<String> = report
        .issues
        .iter()
        .filter(|issue| issue.fallback_applied.as_deref() == Some("event_raw.ext_call"))
        .map(|issue| issue.trace_id.clone())
        .collect();
    assert!(
        !fallback_issue_traces.is_empty(),
        "expected ext_call fallback issues"
    );

    let json = fs::read_to_string(output_root.join("main.json")).expect("read main");
    let script = ScriptRaw::from_json(&json).expect("parse script");
    let mut decorated_count = 0usize;
    for (event_ip, event) in script.events.iter().enumerate() {
        let EventRaw::ExtCall { command, args } = event else {
            continue;
        };
        decorated_count += 1;
        assert_eq!(command, "vn.import.renpy.ext_v2");
        assert!(
            !args.is_empty(),
            "decorated ext_call must include envelope as first arg"
        );
        let envelope: serde_json::Value =
            serde_json::from_str(&args[0]).expect("valid extcall envelope json");
        assert_eq!(
            envelope.get("schema").and_then(serde_json::Value::as_str),
            Some("vn.import.trace.extcall.v2")
        );
        let trace_id = envelope
            .get("trace_id")
            .and_then(serde_json::Value::as_str)
            .expect("trace_id");
        assert!(
            fallback_issue_traces.contains(trace_id),
            "every ext_call envelope trace_id must map to an issue"
        );
        assert_eq!(
            envelope
                .get("event_ip")
                .and_then(serde_json::Value::as_u64)
                .map(|value| value as usize),
            Some(event_ip)
        );
        assert!(
            envelope
                .get("action_id")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|value| value.starts_with("renpy.")),
            "action_id should be canonicalized for imported behavior"
        );
        let payload_len = envelope
            .get("payload")
            .and_then(serde_json::Value::as_array)
            .map(|payload| payload.len())
            .unwrap_or(0);
        assert_eq!(
            args.len().saturating_sub(1),
            payload_len,
            "raw payload should be preserved after envelope arg"
        );
    }

    assert!(
        decorated_count >= 3,
        "expected decorated call/queue/return extcalls"
    );
}

#[test]
fn degraded_events_keep_one_to_one_issue_traceability() {
    let dir = tempdir().expect("tempdir");
    let project_root = dir.path().join("renpy_project");
    let game_dir = project_root.join("game");
    fs::create_dir_all(&game_dir).expect("mkdir game");
    fs::write(
        game_dir.join("script.rpy"),
        r#"
label start:
    call route_a
    queue music "audio/theme.ogg"
    return
    $ score += 1
    python:
        x = 1
"#,
    )
    .expect("write script");

    let output_root = dir.path().join("out_project");
    let report = import_renpy_project(ImportRenpyOptions {
        project_root,
        output_root: output_root.clone(),
        entry_label: "start".to_string(),
        report_path: None,
        profile: ImportProfile::StoryFirst,
        include_tl: None,
        include_ui: None,
        include_patterns: Vec::new(),
        exclude_patterns: Vec::new(),
        strict_mode: false,
        fallback_policy: super::super::ImportFallbackPolicy::DegradeWithTrace,
    })
    .expect("import");

    let json = fs::read_to_string(output_root.join("main.json")).expect("read main");
    let script = ScriptRaw::from_json(&json).expect("parse script");
    let ext_calls = script
        .events
        .iter()
        .filter(|event| matches!(event, EventRaw::ExtCall { .. }))
        .count();
    let fallback_issues = report
        .issues
        .iter()
        .filter(|issue| issue.fallback_applied.as_deref() == Some("event_raw.ext_call"))
        .count();

    assert_eq!(report.degraded_events, ext_calls);
    assert_eq!(fallback_issues, ext_calls);
    assert!(
        report
            .issues_by_code
            .get("unsupported_block_statement")
            .copied()
            .unwrap_or(0)
            >= 1
    );
}
