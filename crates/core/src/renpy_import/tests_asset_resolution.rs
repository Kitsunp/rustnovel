use std::fs;
use std::path::Path;

use super::import_renpy_project;
use super::syntax::{parse_menu_caption_line, parse_show_decl, AssignmentValue};
use super::tests::{temp_renpy_fixture, write_renpy_file};
use super::{ImportFallbackPolicy, ImportProfile, ImportRenpyOptions};
use crate::{EventRaw, ScriptRaw};

fn create_escape_symlink(link: &Path, target: &Path) -> bool {
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(target, link).is_ok()
    }

    #[cfg(windows)]
    {
        std::os::windows::fs::symlink_file(target, link).is_ok()
    }

    #[cfg(not(any(unix, windows)))]
    {
        let _ = link;
        let _ = target;
        false
    }
}

#[test]
fn parse_show_bg_without_alias_infers_background_patch() {
    let aliases = std::collections::HashMap::new();
    let parsed = parse_show_decl("show bg lecturehall", &aliases).expect("show parse");
    assert_eq!(parsed.patch.background.as_deref(), Some("bg/lecturehall"));
    assert!(parsed.patch.add.is_empty());
}

#[test]
fn parse_menu_caption_line_recognizes_prompt_only_line() {
    let caption = parse_menu_caption_line("\"As soon as she catches my eye, I decide...\"");
    assert_eq!(
        caption.as_deref(),
        Some("As soon as she catches my eye, I decide...")
    );
    assert!(parse_menu_caption_line("\"Option\":").is_none());
}

#[test]
fn assignment_parser_marks_unsupported_payload() {
    let parsed = super::syntax::parse_assignment_decl("$ score += 1").expect("assignment");
    match parsed.1 {
        AssignmentValue::Unsupported(raw) => assert!(raw.contains("score += 1")),
        _ => panic!("expected unsupported assignment"),
    }
}

#[test]
fn import_resolves_extensionless_background_assets() {
    let (_dir, project_root, game_dir, output_root) = temp_renpy_fixture();
    fs::create_dir_all(game_dir.join("bg")).expect("mkdir bg");
    fs::write(game_dir.join("bg").join("room.png"), b"img").expect("write asset");
    write_renpy_file(
        &game_dir.join("script.rpy"),
        r#"
label start:
    scene "bg/room"
"#,
    );
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
        fallback_policy: ImportFallbackPolicy::DegradeWithTrace,
    })
    .expect("import");

    assert!(
        report
            .issues
            .iter()
            .all(|issue| issue.code != "asset_not_found"),
        "extensionless path should resolve to existing asset"
    );
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
fn import_does_not_report_symbolic_black_background_as_missing_asset() {
    let (_dir, project_root, game_dir, output_root) = temp_renpy_fixture();
    fs::create_dir_all(&game_dir).expect("mkdir game");
    write_renpy_file(
        &game_dir.join("script.rpy"),
        r#"
label start:
    scene black
"#,
    );
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
        fallback_policy: ImportFallbackPolicy::DegradeWithTrace,
    })
    .expect("import");

    assert!(
        report
            .issues
            .iter()
            .all(|issue| issue.code != "asset_not_found"),
        "symbolic scene values should not be treated as missing files"
    );

    let json = fs::read_to_string(output_root.join("main.json")).expect("read main");
    let script = ScriptRaw::from_json(&json).expect("parse script");
    let scene_bg = script.events.iter().find_map(|event| match event {
        EventRaw::Scene(scene) => scene.background.clone(),
        _ => None,
    });
    assert_eq!(scene_bg.as_deref(), Some("black"));
}

#[test]
fn import_rejects_asset_symlink_escape_outside_project_root() {
    let (dir, project_root, game_dir, output_root) = temp_renpy_fixture();
    fs::create_dir_all(game_dir.join("bg")).expect("mkdir bg");

    let outside_asset = dir.path().join("secret.png");
    fs::write(&outside_asset, b"secret").expect("write outside asset");
    let symlink_path = game_dir.join("bg").join("escape.png");
    if !create_escape_symlink(&symlink_path, &outside_asset) {
        eprintln!("symlink creation not supported on this platform");
        return;
    }

    write_renpy_file(
        &game_dir.join("script.rpy"),
        r#"
label start:
    scene "bg/escape"
"#,
    );

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
        fallback_policy: ImportFallbackPolicy::DegradeWithTrace,
    })
    .expect("import");

    assert!(
        report
            .issues
            .iter()
            .any(|issue| issue.code == "asset_path_traversal"),
        "symlink escape must be reported"
    );
    assert!(
        !output_root
            .join("assets")
            .join("bg")
            .join("escape.png")
            .exists(),
        "escaped asset must never be copied"
    );
    let json = fs::read_to_string(output_root.join("main.json")).expect("read main");
    let script = ScriptRaw::from_json(&json).expect("parse imported script");
    let bg_path = script.events.iter().find_map(|event| match event {
        EventRaw::Scene(scene) => scene.background.clone(),
        _ => None,
    });
    assert_eq!(bg_path.as_deref(), Some("bg/escape"));
}
