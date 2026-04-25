use std::fs;
use tempfile::tempdir;

use super::tests::{temp_renpy_fixture, write_renpy_file};
use super::{import_renpy_project, ImportFallbackPolicy, ImportProfile, ImportRenpyOptions};
use crate::ScriptRaw;

#[test]
fn story_first_profile_excludes_tl_and_ui_files_by_default() {
    let (_dir, project_root, game_dir, output_root) = temp_renpy_fixture();
    fs::create_dir_all(game_dir.join("tl").join("es")).expect("mkdir tl/es");

    write_renpy_file(
        &game_dir.join("script.rpy"),
        r#"
label start:
    "Hello"
"#,
    );
    write_renpy_file(
        &game_dir.join("screens.rpy"),
        r#"
call ui_route
"#,
    );
    write_renpy_file(
        &game_dir.join("tl").join("es").join("script.rpy"),
        r#"
call translated_route
"#,
    );
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
        fallback_policy: super::ImportFallbackPolicy::DegradeWithTrace,
    })
    .expect("import story-first");

    assert_eq!(report.profile, "story_first");
    assert_eq!(
        report.files_scanned, 1,
        "only game/script.rpy should be included"
    );
    assert_eq!(report.issues_by_area.get("translation").copied(), Some(0));
    assert_eq!(report.issues_by_area.get("ui").copied(), Some(0));
    assert!(
        report.issues.iter().all(|issue| issue
            .file
            .as_deref()
            .is_none_or(|f| !f.contains("/game/tl/"))),
        "tl files must be excluded in story-first"
    );
    assert!(
        report.issues.iter().all(|issue| issue
            .file
            .as_deref()
            .is_none_or(|f| !f.ends_with("/game/screens.rpy"))),
        "ui files must be excluded in story-first"
    );
}

#[test]
fn full_profile_includes_tl_and_ui_files() {
    let (_dir, project_root, game_dir, output_root) = temp_renpy_fixture();
    fs::create_dir_all(game_dir.join("tl").join("es")).expect("mkdir tl/es");

    write_renpy_file(
        &game_dir.join("script.rpy"),
        r#"
label start:
    "Hello"
"#,
    );
    write_renpy_file(
        &game_dir.join("screens.rpy"),
        r#"
call ui_route
"#,
    );
    write_renpy_file(
        &game_dir.join("tl").join("es").join("script.rpy"),
        r#"
call translated_route
"#,
    );
    let report = import_renpy_project(ImportRenpyOptions {
        project_root,
        output_root,
        entry_label: "start".to_string(),
        report_path: None,
        profile: ImportProfile::Full,
        include_tl: None,
        include_ui: None,
        include_patterns: Vec::new(),
        exclude_patterns: Vec::new(),
        strict_mode: false,
        fallback_policy: super::ImportFallbackPolicy::DegradeWithTrace,
    })
    .expect("import full");

    assert_eq!(report.profile, "full");
    assert_eq!(report.files_scanned, 3);
    assert!(
        report
            .issues_by_area
            .get("translation")
            .copied()
            .unwrap_or(0)
            > 0
    );
    assert!(report.issues_by_area.get("ui").copied().unwrap_or(0) > 0);
    assert!(
        report.issues.iter().any(|issue| issue
            .file
            .as_deref()
            .is_some_and(|f| f.contains("/game/tl/"))),
        "full profile must parse translation files"
    );
    assert!(
        report.issues.iter().any(|issue| issue
            .file
            .as_deref()
            .is_some_and(|f| f.ends_with("/game/screens.rpy"))),
        "full profile must parse ui files"
    );
}

#[test]
fn story_first_can_override_include_ui() {
    let (_dir, project_root, game_dir, output_root) = temp_renpy_fixture();
    fs::create_dir_all(game_dir.join("tl").join("es")).expect("mkdir tl/es");

    write_renpy_file(
        &game_dir.join("script.rpy"),
        r#"
label start:
    "Hello"
"#,
    );
    write_renpy_file(
        &game_dir.join("screens.rpy"),
        r#"
call ui_route
"#,
    );
    write_renpy_file(
        &game_dir.join("tl").join("es").join("script.rpy"),
        r#"
call translated_route
"#,
    );
    let report = import_renpy_project(ImportRenpyOptions {
        project_root,
        output_root,
        entry_label: "start".to_string(),
        report_path: None,
        profile: ImportProfile::StoryFirst,
        include_tl: None,
        include_ui: Some(true),
        include_patterns: Vec::new(),
        exclude_patterns: Vec::new(),
        strict_mode: false,
        fallback_policy: super::ImportFallbackPolicy::DegradeWithTrace,
    })
    .expect("import story-first with ui override");

    assert_eq!(report.files_scanned, 2);
    assert!(
        report.issues.iter().any(|issue| issue
            .file
            .as_deref()
            .is_some_and(|f| f.ends_with("/game/screens.rpy"))),
        "ui override must include ui files"
    );
    assert!(
        report.issues.iter().all(|issue| issue
            .file
            .as_deref()
            .is_none_or(|f| !f.contains("/game/tl/"))),
        "translation files should remain excluded unless explicitly enabled"
    );
}

#[test]
fn import_blocks_asset_path_traversal_during_copy() {
    let dir = tempdir().expect("tempdir");
    let project_root = dir.path().join("renpy_project");
    let game_dir = project_root.join("game");
    fs::create_dir_all(&game_dir).expect("mkdir game");
    fs::write(project_root.join("secret.png"), b"secret").expect("write outside asset");
    fs::write(
        game_dir.join("script.rpy"),
        r#"
label start:
    scene "../secret.png"
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
        fallback_policy: super::ImportFallbackPolicy::DegradeWithTrace,
    })
    .expect("import");

    assert!(
        report
            .issues
            .iter()
            .any(|issue| issue.code == "asset_path_traversal"),
        "path traversal should be reported explicitly"
    );
    assert!(
        !output_root.join("assets").join("secret.png").exists(),
        "traversal path should never be copied into assets output"
    );

    let json = fs::read_to_string(output_root.join("main.json")).expect("read script");
    let script = ScriptRaw::from_json(&json).expect("parse imported script");
    let bg_path = script.events.iter().find_map(|event| match event {
        crate::EventRaw::Scene(scene) => scene.background.clone(),
        _ => None,
    });
    assert_eq!(bg_path.as_deref(), Some("../secret.png"));
}

#[test]
fn strict_mode_rejects_degraded_import() {
    let dir = tempdir().expect("tempdir");
    let project_root = dir.path().join("renpy_project");
    let game_dir = project_root.join("game");
    fs::create_dir_all(&game_dir).expect("mkdir game");
    fs::write(
        game_dir.join("script.rpy"),
        r#"
label start:
    call route_a
"#,
    )
    .expect("write script");

    let output_root = dir.path().join("out_project");
    let result = import_renpy_project(ImportRenpyOptions {
        project_root,
        output_root,
        entry_label: "start".to_string(),
        report_path: None,
        profile: ImportProfile::StoryFirst,
        include_tl: None,
        include_ui: None,
        include_patterns: Vec::new(),
        exclude_patterns: Vec::new(),
        strict_mode: true,
        fallback_policy: ImportFallbackPolicy::DegradeWithTrace,
    });

    let err = result.expect_err("strict mode should reject degraded import");
    let msg = err.to_string();
    assert!(msg.contains("strict policy rejected degraded import"));
    assert!(msg.contains("unsupported_call"));
}

#[test]
fn fallback_policy_strict_rejects_degraded_import() {
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
    let result = import_renpy_project(ImportRenpyOptions {
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
        fallback_policy: ImportFallbackPolicy::Strict,
    });

    let err = result.expect_err("strict fallback policy should reject degraded import");
    let msg = err.to_string();
    assert!(msg.contains("strict policy rejected degraded import"));
    assert!(msg.contains("unsupported_return"));
}

#[test]
fn custom_profile_respects_include_exclude_patterns() {
    let dir = tempdir().expect("tempdir");
    let project_root = dir.path().join("renpy_project");
    let game_dir = project_root.join("game");
    fs::create_dir_all(game_dir.join("extra")).expect("mkdir game/extra");
    fs::write(
        game_dir.join("script.rpy"),
        r#"
label start:
    "Main"
"#,
    )
    .expect("write script");
    fs::write(
        game_dir.join("extra").join("debug.rpy"),
        r#"
label debug:
    call debug_route
"#,
    )
    .expect("write debug");

    let output_root = dir.path().join("out_project");
    let report = import_renpy_project(ImportRenpyOptions {
        project_root,
        output_root,
        entry_label: "start".to_string(),
        report_path: None,
        profile: ImportProfile::Custom,
        include_tl: None,
        include_ui: None,
        include_patterns: vec!["game/*.rpy".to_string(), "game/extra/*.rpy".to_string()],
        exclude_patterns: vec!["*debug*".to_string()],
        strict_mode: false,
        fallback_policy: ImportFallbackPolicy::DegradeWithTrace,
    })
    .expect("import custom");

    assert_eq!(report.profile, "custom");
    assert_eq!(report.files_scanned, 1, "debug file must be excluded");
    assert_eq!(
        report
            .issues
            .iter()
            .filter(|issue| issue.code == "unsupported_call")
            .count(),
        0
    );
}

#[test]
fn story_first_ignores_non_game_rpy_when_game_dir_exists() {
    let dir = tempdir().expect("tempdir");
    let project_root = dir.path().join("renpy_project");
    let game_dir = project_root.join("game");
    let launcher_dir = project_root.join("launcher");
    fs::create_dir_all(&game_dir).expect("mkdir game");
    fs::create_dir_all(&launcher_dir).expect("mkdir launcher");

    fs::write(
        game_dir.join("script.rpy"),
        r#"
label start:
    "Hello"
"#,
    )
    .expect("write game script");
    fs::write(
        launcher_dir.join("ui.rpy"),
        r#"
label launcher:
    call nowhere
"#,
    )
    .expect("write launcher script");

    let output_root = dir.path().join("out_story_first");
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
        fallback_policy: ImportFallbackPolicy::DegradeWithTrace,
    })
    .expect("import story-first");

    assert_eq!(
        report.files_scanned, 1,
        "must scan only game/*.rpy by default"
    );
    assert!(
        report.issues.iter().all(|issue| issue
            .file
            .as_deref()
            .is_none_or(|f| !f.contains("/launcher/"))),
        "non-game directories should be ignored by default when game/ exists"
    );
    assert!(
        report.scan_root.ends_with("/game"),
        "scan_root should point to game dir"
    );
}

#[test]
fn story_first_on_game_root_still_excludes_tl_and_ui() {
    let dir = tempdir().expect("tempdir");
    let project_root = dir.path().join("renpy_project");
    let game_dir = project_root.join("game");
    fs::create_dir_all(game_dir.join("tl").join("es")).expect("mkdir tl/es");
    fs::create_dir_all(&game_dir).expect("mkdir game");

    fs::write(
        game_dir.join("script.rpy"),
        r#"
label start:
    "Hello"
"#,
    )
    .expect("write story script");
    fs::write(
        game_dir.join("screens.rpy"),
        r#"
call ui_route
"#,
    )
    .expect("write ui script");
    fs::write(
        game_dir.join("tl").join("es").join("script.rpy"),
        r#"
call translated_route
"#,
    )
    .expect("write tl script");

    let output_root = dir.path().join("out_story_first");
    let report = import_renpy_project(ImportRenpyOptions {
        project_root: game_dir.clone(),
        output_root,
        entry_label: "start".to_string(),
        report_path: None,
        profile: ImportProfile::StoryFirst,
        include_tl: None,
        include_ui: None,
        include_patterns: Vec::new(),
        exclude_patterns: Vec::new(),
        strict_mode: false,
        fallback_policy: ImportFallbackPolicy::DegradeWithTrace,
    })
    .expect("import story-first from game root");

    assert_eq!(report.files_scanned, 1);
    assert!(
        report
            .issues
            .iter()
            .all(|issue| issue.file.as_deref().is_none_or(|f| !f.contains("/tl/"))),
        "tl files must be excluded even when selecting game/ as root"
    );
    assert!(
        report.issues.iter().all(|issue| issue
            .file
            .as_deref()
            .is_none_or(|f| !f.ends_with("/screens.rpy"))),
        "ui files must be excluded even when selecting game/ as root"
    );
    assert_eq!(
        normalize_win_path(&report.scan_root),
        normalize_win_path(&game_dir.to_string_lossy())
    );
}

fn normalize_win_path(value: &str) -> String {
    value
        .replace('\\', "/")
        .trim_start_matches("//?/")
        .to_string()
}
