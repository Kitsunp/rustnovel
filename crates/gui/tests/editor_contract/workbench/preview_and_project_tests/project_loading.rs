use super::*;

#[test]
fn load_project_with_status_reports_error_in_silent_mode() {
    let config = VnConfig::default();
    let mut workbench = EditorWorkbench::new(config);
    let missing_manifest = std::path::PathBuf::from("__missing__/project.vnm");

    let result = workbench.load_project_with_status(missing_manifest, false);
    assert!(result.is_err());
    assert!(
        workbench.toast.is_none(),
        "silent mode should not overwrite UI toast state"
    );
}

#[test]
fn load_project_with_status_reports_error_with_toast() {
    let config = VnConfig::default();
    let mut workbench = EditorWorkbench::new(config);
    let missing_manifest = std::path::PathBuf::from("__missing__/project.vnm");

    let result = workbench.load_project_with_status(missing_manifest, true);
    assert!(result.is_err());
    let message = workbench
        .toast
        .as_ref()
        .map(|toast| toast.message.clone())
        .unwrap_or_default();
    assert!(
        message.contains("Failed to load project"),
        "error toast should expose load failure"
    );
}

#[test]
fn load_project_with_status_can_open_recently_imported_renpy_project() {
    let config = VnConfig::default();
    let mut workbench = EditorWorkbench::new(config);
    let dir = tempfile::tempdir().expect("tempdir");
    let project_root = dir.path().join("renpy_project");
    let game_dir = project_root.join("game");
    std::fs::create_dir_all(&game_dir).expect("mkdir game");
    std::fs::write(
        game_dir.join("script.rpy"),
        r#"
label start:
    "Hello from import"
"#,
    )
    .expect("write script");

    let output_root = dir.path().join("imported_out");
    visual_novel_engine::import_renpy_project(visual_novel_engine::ImportRenpyOptions {
        project_root,
        output_root: output_root.clone(),
        entry_label: "start".to_string(),
        report_path: None,
        profile: visual_novel_engine::ImportProfile::StoryFirst,
        include_tl: None,
        include_ui: None,
        include_patterns: Vec::new(),
        exclude_patterns: Vec::new(),
        strict_mode: false,
        fallback_policy: visual_novel_engine::ImportFallbackPolicy::DegradeWithTrace,
    })
    .expect("import renpy");

    let manifest_path = output_root.join("project.vnm");
    workbench
        .load_project_with_status(manifest_path, false)
        .expect("workbench should load imported project");
    assert!(workbench.current_script.is_some(), "script must be loaded");
    assert!(
        workbench
            .node_graph
            .nodes()
            .any(|(_, node, _)| matches!(node, StoryNode::Dialogue { .. })),
        "graph should contain imported dialogue node"
    );
    assert!(
        workbench.engine.is_some(),
        "player engine should be initialized"
    );
    assert!(
        workbench
            .engine
            .as_ref()
            .and_then(|engine| engine.current_event().ok())
            .is_some(),
        "engine should expose a current event after load"
    );
}

#[test]
fn loaded_example_project_can_prepare_player_mode() {
    let config = VnConfig::default();
    let mut workbench = EditorWorkbench::new(config);
    let manifest_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("examples")
        .join("scripts")
        .join("project.vnm");

    workbench
        .load_project_with_status(manifest_path, false)
        .expect("example project should load");
    let prepared = workbench.prepare_player_mode();
    let toast = workbench
        .toast
        .as_ref()
        .map(|toast| toast.message.as_str())
        .unwrap_or("<no toast>");

    assert!(prepared, "example project must enter Play mode: {toast}");
    assert!(
        workbench.engine.is_some(),
        "player engine should remain available after preparing Play mode"
    );
}

#[test]
fn load_project_with_status_ignores_locales_outside_locale_root() {
    let config = VnConfig::default();
    let mut workbench = EditorWorkbench::new(config);
    let dir = tempdir().expect("tempdir");
    let project_root = dir.path().join("project");
    let locale_root = project_root.join("locales");
    fs::create_dir_all(&locale_root).expect("mkdir locales");

    fs::write(
        project_root.join("main.json"),
        r#"{
  "script_schema_version": "1.0",
  "events": [
    { "type": "dialogue", "speaker": "Narrator", "text": "Hola" }
  ],
  "labels": { "start": 0 }
}"#,
    )
    .expect("write script");
    fs::write(
        locale_root.join("en.json"),
        r#"{"hello":"hola","start":"inicio"}"#,
    )
    .expect("write safe locale");
    fs::write(
        project_root.join("escape.json"),
        r#"{"hello":"pwned","start":"escape"}"#,
    )
    .expect("write escaping locale");

    fs::write(
        project_root.join("project.vnm"),
        r#"
manifest_schema_version = "1.0"

[metadata]
name = "Locale Safety"
author = "QA"
version = "0.1.0"

[settings]
resolution = [1280, 720]
default_language = "en"
supported_languages = ["en", "../escape"]
entry_point = "main.json"

[assets]
"#,
    )
    .expect("write manifest");

    workbench
        .load_project_with_status(project_root.join("project.vnm"), false)
        .expect("project should load");

    assert_eq!(
        workbench.localization_catalog.locale_codes(),
        vec!["en".to_string()],
        "only locale files contained in locales/ should be loaded"
    );
    assert_eq!(workbench.localization_catalog.default_locale, "en");
    assert!(
        workbench.current_script.is_some(),
        "entry script should load"
    );
}

#[test]
fn load_standalone_script_replaces_previous_project_root_and_manifest_state() {
    let config = VnConfig::default();
    let mut workbench = EditorWorkbench::new(config);
    let dir = tempdir().expect("tempdir");
    let first_root = dir.path().join("first_project");
    let second_root = dir.path().join("second_project");
    fs::create_dir_all(first_root.join("locales")).expect("mkdir first locales");
    fs::create_dir_all(second_root.join("locales")).expect("mkdir second locales");

    fs::write(
        first_root.join("main.json"),
        r#"{
  "script_schema_version": "1.0",
  "events": [
    { "type": "scene", "background": "backgrounds/old.png" }
  ],
  "labels": { "start": 0 }
}"#,
    )
    .expect("write first script");
    fs::write(
        first_root.join("project.vnm"),
        r#"
manifest_schema_version = "1.0"

[metadata]
name = "First"
author = "QA"
version = "0.1.0"

[settings]
resolution = [1280, 720]
default_language = "es"
supported_languages = ["es"]
entry_point = "main.json"

[assets]
"#,
    )
    .expect("write first manifest");
    fs::write(first_root.join("locales/es.json"), r#"{"old":"viejo"}"#)
        .expect("write first locale");

    fs::write(
        second_root.join("main.json"),
        r#"{
  "script_schema_version": "1.0",
  "events": [
    { "type": "scene", "background": "backgrounds/new.png" }
  ],
  "labels": { "start": 0 }
}"#,
    )
    .expect("write second script");
    fs::write(second_root.join("locales/fr.json"), r#"{"new":"nouveau"}"#)
        .expect("write second locale");

    workbench
        .load_project_with_status(first_root.join("project.vnm"), false)
        .expect("first project should load");
    assert_eq!(
        workbench.project_root.as_deref(),
        Some(first_root.as_path())
    );
    assert!(workbench.manifest.is_some());

    workbench.load_script(second_root.join("main.json"));

    assert_eq!(
        workbench.project_root.as_deref(),
        Some(second_root.as_path()),
        "standalone script load must not keep resolving assets against the old project"
    );
    assert!(
        workbench.manifest.is_none(),
        "standalone scripts should not keep stale manifest assets/settings from a previous project"
    );
    assert!(
        workbench.manifest_path.is_none(),
        "standalone script load should clear manifest path"
    );
    assert_eq!(
        workbench.localization_catalog.locale_codes(),
        vec!["fr".to_string()],
        "locale discovery should use the new script directory"
    );
    assert_eq!(workbench.player_locale, "fr");
    assert!(
        workbench.scene.iter().any(|entity| matches!(
            &entity.kind,
            visual_novel_engine::EntityKind::Image(image)
                if image.path.as_ref() == "backgrounds/new.png"
        )),
        "preview scene should come from the newly loaded script"
    );
}
