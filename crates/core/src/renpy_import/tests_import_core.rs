use super::*;

#[test]
fn import_writes_project_files_and_valid_script() {
    let dir = tempdir().expect("tempdir");
    let project_root = dir.path().join("renpy_project");
    let game_dir = project_root.join("game");
    fs::create_dir_all(&game_dir).expect("mkdir game");

    let script = r#"
label start:
    "Hello"
    menu:
        "Go":
            jump end_route
        "Stay":
            jump start

label end_route:
    "End"
"#;
    fs::write(game_dir.join("script.rpy"), script).expect("write script");

    let output_root = dir.path().join("out_project");
    let report = import_renpy_project(ImportRenpyOptions {
        project_root: project_root.clone(),
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

    assert!(output_root.join("main.json").exists());
    assert!(output_root.join("project.vnm").exists());
    assert!(output_root.join("import_report.json").exists());
    assert!(report.events_generated >= 3);

    let json = fs::read_to_string(output_root.join("main.json")).expect("read main");
    let script = ScriptRaw::from_json(&json).expect("parse imported script");
    assert!(script.labels.contains_key("start"));
    assert!(script.compile().is_ok(), "imported script must compile");
}

#[test]
fn import_degrades_unsupported_statements_to_ext_call() {
    let dir = tempdir().expect("tempdir");
    let project_root = dir.path().join("renpy_project");
    let game_dir = project_root.join("game");
    fs::create_dir_all(&game_dir).expect("mkdir game");

    let script = r#"
label start:
    call route_a
    queue music "audio/theme.ogg"
    return

label route_a:
    "X"
"#;
    fs::write(game_dir.join("script.rpy"), script).expect("write script");

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

    assert!(report.degraded_events >= 2);
    assert!(report
        .issues
        .iter()
        .any(|issue| issue.code == "unsupported_call"));
    assert!(report
        .issues
        .iter()
        .any(|issue| issue.code == "unsupported_audio_queue"));

    let json = fs::read_to_string(output_root.join("main.json")).expect("read main");
    let script = ScriptRaw::from_json(&json).expect("parse script");
    assert!(script
        .events
        .iter()
        .any(|event| matches!(event, EventRaw::ExtCall { .. })));
}

#[test]
fn import_patches_missing_targets_and_keeps_script_compileable() {
    let dir = tempdir().expect("tempdir");
    let project_root = dir.path().join("renpy_project");
    let game_dir = project_root.join("game");
    fs::create_dir_all(&game_dir).expect("mkdir game");

    fs::write(
        game_dir.join("script.rpy"),
        r#"
label start:
    jump nowhere
"#,
    )
    .expect("write");

    let output_root = dir.path().join("out_project");
    let _ = import_renpy_project(ImportRenpyOptions {
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
    assert!(script.labels.contains_key("nowhere"));
    assert!(script.compile().is_ok());
}

#[test]
fn import_rewrites_and_copies_assets() {
    let dir = tempdir().expect("tempdir");
    let project_root = dir.path().join("renpy_project");
    let game_dir = project_root.join("game");
    fs::create_dir_all(game_dir.join("bg")).expect("mkdir bg");
    fs::write(game_dir.join("bg").join("room.png"), b"img").expect("write asset");
    fs::write(
        game_dir.join("script.rpy"),
        r#"
label start:
    scene "bg/room.png"
"#,
    )
    .expect("write script");

    let output_root = dir.path().join("out_project");
    import_renpy_project(ImportRenpyOptions {
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

    assert!(output_root
        .join("assets")
        .join("bg")
        .join("room.png")
        .exists());
    let json = fs::read_to_string(output_root.join("main.json")).expect("read main");
    let script = ScriptRaw::from_json(&json).expect("parse script");
    let scene_bg = script.events.iter().find_map(|event| match event {
        EventRaw::Scene(scene) => scene.background.clone(),
        _ => None,
    });
    assert_eq!(scene_bg.as_deref(), Some("assets/bg/room.png"));
}

#[test]
fn import_collapses_unsupported_blocks_into_single_event() {
    let dir = tempdir().expect("tempdir");
    let project_root = dir.path().join("renpy_project");
    let game_dir = project_root.join("game");
    fs::create_dir_all(&game_dir).expect("mkdir game");

    fs::write(
        game_dir.join("script.rpy"),
        r#"
label start:
    python:
        score = 1
        points = score + 1
    play music 'audio/theme.ogg'
    "After"
"#,
    )
    .expect("write script");
    fs::create_dir_all(game_dir.join("audio")).expect("mkdir audio");
    fs::write(game_dir.join("audio").join("theme.ogg"), b"snd").expect("write audio");

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

    let unexpected_indent = report
        .issues_by_code
        .get("unexpected_indent")
        .copied()
        .unwrap_or(0);
    assert_eq!(unexpected_indent, 0);
    assert!(
        report
            .issues
            .iter()
            .any(|issue| issue.code == "unsupported_block_statement"),
        "unsupported blocks must be reported once"
    );

    let json = fs::read_to_string(output_root.join("main.json")).expect("read main");
    let script = ScriptRaw::from_json(&json).expect("parse script");
    assert!(script.compile().is_ok());
    let audio_asset = script.events.iter().find_map(|event| match event {
        EventRaw::AudioAction(action) => action.asset.clone(),
        _ => None,
    });
    assert_eq!(audio_asset.as_deref(), Some("assets/audio/theme.ogg"));
}
