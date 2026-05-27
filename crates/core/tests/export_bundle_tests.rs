use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use tempfile::TempDir;
use visual_novel_engine::{
    authoring::{AuthoringDocument, AuthoringPosition, NodeGraph, StoryNode},
    build_export_plan, export_bundle,
    runtime::{AudioActionRaw, DialogueRaw, EventRaw, SceneTransitionRaw, ScriptRaw},
    BundleIntegrity, ExportBundleSpec, ExportTargetPlatform, ProjectManifest,
};

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

fn build_project_fixture() -> (TempDir, std::path::PathBuf) {
    let tmp = TempDir::new().expect("temp dir");
    let root = tmp.path().join("project");
    fs::create_dir_all(root.join("assets/bgm")).expect("assets dir");

    let manifest = ProjectManifest::new("fixture", "qa");
    manifest
        .save(&root.join("project.vnm"))
        .expect("manifest save");

    let script = ScriptRaw::new(
        vec![
            EventRaw::Dialogue(DialogueRaw {
                speaker: "Narrator".to_string(),
                text: "hello".to_string(),
            }),
            EventRaw::AudioAction(AudioActionRaw {
                channel: "bgm".to_string(),
                action: "play".to_string(),
                asset: Some("assets/bgm/theme.ogg".to_string()),
                volume: Some(0.8),
                fade_duration_ms: None,
                loop_playback: Some(true),
            }),
        ],
        BTreeMap::from([("start".to_string(), 0)]),
    );
    fs::write(
        root.join("main.json"),
        script.to_json().expect("script json"),
    )
    .expect("script");
    fs::write(root.join("assets/bgm/theme.ogg"), [1u8, 2, 3, 4]).expect("asset");
    fs::write(root.join("assets/bgm/unused.ogg"), [5u8, 6, 7, 8]).expect("unused asset");

    (tmp, root)
}

#[test]
fn export_bundle_builds_expected_layout_and_manifest() {
    let (_tmp, project_root) = build_project_fixture();
    let out = project_root.join("dist");

    let report = export_bundle(ExportBundleSpec {
        project_root: project_root.clone(),
        output_root: out.clone(),
        target_platform: ExportTargetPlatform::Windows,
        entry_script: None,
        runtime_artifact: None,
        integrity: BundleIntegrity::None,
        output_layout_version: 1,
        hmac_key: None,
    })
    .expect("bundle export");

    assert_eq!(report.target_platform, "windows");
    assert_eq!(report.integrity, "none");
    assert_eq!(report.script_source, "scripts/compiled.vnscript.json");
    assert_eq!(report.script_binary, "scripts/compiled.vnc");
    assert_eq!(report.assets_copied, 1);
    assert_eq!(
        report.capabilities.audio_actions,
        vec!["bgm:play".to_string()]
    );
    assert!(report
        .capabilities
        .warnings
        .contains(&"audio_requires_runtime_audio_backend".to_string()));
    assert!(!Path::new(&out.join("scripts/main.vnc")).exists());
    assert!(Path::new(&out.join("scripts/compiled.vnc")).is_file());
    assert!(Path::new(&out.join("scripts/compiled.vnscript.json")).is_file());
    assert!(!Path::new(&out.join("scripts/main.json")).exists());
    assert!(Path::new(&out.join("assets/bgm/theme.ogg")).is_file());
    assert!(!Path::new(&out.join("assets/bgm/unused.ogg")).exists());
    assert!(Path::new(&out.join("meta/assets_manifest.json")).is_file());
    assert!(Path::new(&out.join("meta/package_report.json")).is_file());
    assert!(Path::new(&out.join("launch.bat")).is_file());

    let manifest_raw =
        fs::read_to_string(out.join("meta/assets_manifest.json")).expect("assets manifest");
    let manifest: serde_json::Value =
        serde_json::from_str(&manifest_raw).expect("assets manifest json");
    assert!(manifest.get("assets/bgm/theme.ogg").is_some());
    assert!(manifest.get("assets/bgm/unused.ogg").is_none());
}

#[test]
fn export_plan_cli_py_gui_parity() {
    let (_tmp, project_root) = build_project_fixture();
    let spec = ExportBundleSpec {
        project_root: project_root.clone(),
        output_root: project_root.join("dist"),
        target_platform: ExportTargetPlatform::Windows,
        entry_script: None,
        runtime_artifact: None,
        integrity: BundleIntegrity::None,
        output_layout_version: 1,
        hmac_key: None,
    };

    let cli_plan = build_export_plan(&spec).expect("cli plan");
    let py_plan_json = serde_json::to_string(&cli_plan).expect("plan json");
    let py_plan: visual_novel_engine::ExportPlan =
        serde_json::from_str(&py_plan_json).expect("py/gui plan");

    assert_eq!(cli_plan.script_sha256, py_plan.script_sha256);
    assert_eq!(cli_plan.layout, py_plan.layout);
    assert_eq!(cli_plan.capabilities, py_plan.capabilities);
    assert!(cli_plan
        .warnings
        .contains(&"missing_runtime_artifact".to_string()));
}

#[test]
fn export_plan_extcall_audio_transition_missing_runtime() {
    let tmp = TempDir::new().expect("temp dir");
    let root = tmp.path().join("project");
    fs::create_dir_all(root.join("assets/bgm")).expect("assets dir");
    ProjectManifest::new("fixture", "qa")
        .save(&root.join("project.vnm"))
        .expect("manifest");
    let script = ScriptRaw::new(
        vec![
            EventRaw::AudioAction(AudioActionRaw {
                channel: "bgm".to_string(),
                action: "play".to_string(),
                asset: Some("assets/bgm/theme.ogg".to_string()),
                volume: Some(0.5),
                fade_duration_ms: Some(300),
                loop_playback: Some(true),
            }),
            EventRaw::Transition(SceneTransitionRaw {
                kind: "fade".to_string(),
                duration_ms: 250,
                color: None,
            }),
            EventRaw::ExtCall {
                command: "plugin".to_string(),
                args: vec![],
            },
        ],
        BTreeMap::from([("start".to_string(), 0)]),
    );
    fs::write(root.join("main.json"), script.to_json().expect("script")).expect("script");
    fs::write(root.join("assets/bgm/theme.ogg"), [1u8, 2, 3, 4]).expect("asset");

    let plan = build_export_plan(&ExportBundleSpec {
        project_root: root.clone(),
        output_root: root.join("dist"),
        target_platform: ExportTargetPlatform::Windows,
        entry_script: None,
        runtime_artifact: None,
        integrity: BundleIntegrity::None,
        output_layout_version: 1,
        hmac_key: None,
    })
    .expect("plan");

    assert!(plan
        .warnings
        .contains(&"missing_runtime_artifact".to_string()));
    assert!(plan
        .capabilities
        .ext_call_commands
        .contains(&"plugin".to_string()));
    assert!(!plan.capabilities.audio_actions.is_empty());
    assert!(!plan.capabilities.transitions.is_empty());
}

#[test]
fn export_plan_capability_policy_contract() {
    let (_tmp, project_root) = build_project_fixture();
    let plan = build_export_plan(&ExportBundleSpec {
        project_root: project_root.clone(),
        output_root: project_root.join("dist"),
        target_platform: ExportTargetPlatform::Windows,
        entry_script: None,
        runtime_artifact: Some(PathBuf::from("missing-runtime.exe")),
        integrity: BundleIntegrity::HmacSha256,
        output_layout_version: 1,
        hmac_key: None,
    })
    .expect("plan with policy errors");

    assert!(plan
        .errors
        .iter()
        .any(|error| error.contains("runtime artifact")));
    assert!(plan
        .errors
        .iter()
        .any(|error| error.contains("requires hmac_key")));
}

#[test]
fn export_bundle_reports_extcall_audio_transition_capabilities() {
    let (_tmp, project_root) = build_project_fixture();
    let script = ScriptRaw::new(
        vec![
            EventRaw::AudioAction(AudioActionRaw {
                channel: "sfx".to_string(),
                action: "play".to_string(),
                asset: Some("assets/bgm/theme.ogg".to_string()),
                volume: None,
                fade_duration_ms: None,
                loop_playback: None,
            }),
            EventRaw::Transition(SceneTransitionRaw {
                kind: "dissolve".to_string(),
                duration_ms: 250,
                color: None,
            }),
            EventRaw::ExtCall {
                command: "plugin.unlock".to_string(),
                args: Vec::new(),
            },
        ],
        BTreeMap::from([("start".to_string(), 0)]),
    );
    fs::write(
        project_root.join("main.json"),
        script.to_json().expect("script json"),
    )
    .expect("script");

    let report = export_bundle(ExportBundleSpec {
        project_root: project_root.clone(),
        output_root: project_root.join("dist_caps"),
        target_platform: ExportTargetPlatform::Windows,
        entry_script: None,
        runtime_artifact: None,
        integrity: BundleIntegrity::None,
        output_layout_version: 1,
        hmac_key: None,
    })
    .expect("bundle export");

    assert_eq!(
        report.capabilities.ext_call_commands,
        vec!["plugin.unlock".to_string()]
    );
    assert_eq!(
        report.capabilities.audio_actions,
        vec!["sfx:play".to_string()]
    );
    assert_eq!(
        report.capabilities.transitions,
        vec!["dissolve".to_string()]
    );
    assert!(report
        .capabilities
        .warnings
        .contains(&"ext_call_requires_runtime_handler".to_string()));
    assert!(report
        .capabilities
        .warnings
        .contains(&"transitions_require_visual_runtime_support".to_string()));
}

#[test]
fn export_bundle_accepts_authoring_document_entry() {
    let (_tmp, project_root) = build_project_fixture();
    let mut manifest = ProjectManifest::new("fixture", "qa");
    manifest.settings.entry_point = "main.vnauthoring".to_string();
    manifest
        .save(&project_root.join("project.vnm"))
        .expect("manifest save");

    let mut graph = NodeGraph::new();
    let start = graph.add_node(StoryNode::Start, AuthoringPosition::new(0.0, 0.0));
    let line = graph.add_node(
        StoryNode::Dialogue {
            speaker: "Narrator".to_string(),
            text: "from authoring".to_string(),
        },
        AuthoringPosition::new(0.0, 90.0),
    );
    let end = graph.add_node(StoryNode::End, AuthoringPosition::new(0.0, 180.0));
    graph.connect(start, line);
    graph.connect(line, end);
    let document = AuthoringDocument::new(graph);
    fs::write(
        project_root.join("main.vnauthoring"),
        document.to_json().expect("authoring json"),
    )
    .expect("write authoring");

    let out = project_root.join("dist_authoring");
    let report = export_bundle(ExportBundleSpec {
        project_root: project_root.clone(),
        output_root: out.clone(),
        target_platform: ExportTargetPlatform::Windows,
        entry_script: None,
        runtime_artifact: None,
        integrity: BundleIntegrity::None,
        output_layout_version: 1,
        hmac_key: None,
    })
    .expect("bundle export from authoring");

    assert_eq!(report.script_source, "scripts/compiled.vnscript.json");
    assert!(Path::new(&out.join("scripts/compiled.vnc")).is_file());
    assert!(!Path::new(&out.join("scripts/main.vnauthoring")).exists());
}

#[test]
fn export_bundle_windows_runtime_exe_creates_top_level_executable() {
    let (_tmp, project_root) = build_project_fixture();
    let runtime_dir = project_root.join("runtime");
    fs::create_dir_all(&runtime_dir).expect("mkdir runtime");
    fs::write(runtime_dir.join("vn-runtime.exe"), b"fake-exe").expect("write runtime");
    let out = project_root.join("dist_exe");

    let report = export_bundle(ExportBundleSpec {
        project_root: project_root.clone(),
        output_root: out.clone(),
        target_platform: ExportTargetPlatform::Windows,
        entry_script: None,
        runtime_artifact: Some(PathBuf::from("runtime/vn-runtime.exe")),
        integrity: BundleIntegrity::None,
        output_layout_version: 1,
        hmac_key: None,
    })
    .expect("bundle export with runtime exe");

    assert_eq!(
        report.runtime_artifact.as_deref(),
        Some("runtime/vn-runtime.exe")
    );
    assert_eq!(report.executable.as_deref(), Some("game.exe"));
    assert_eq!(
        fs::read(out.join("game.exe")).expect("game exe"),
        b"fake-exe"
    );
    let launcher = fs::read_to_string(out.join("launch.bat")).expect("launcher");
    assert!(
        launcher.contains("\"%~dp0game.exe\""),
        "launcher should execute the top-level exe: {launcher}"
    );
}

#[test]
fn export_bundle_rejects_entry_script_traversal() {
    let (_tmp, project_root) = build_project_fixture();
    let out = project_root.join("dist");

    let err = export_bundle(ExportBundleSpec {
        project_root: project_root.clone(),
        output_root: out,
        target_platform: ExportTargetPlatform::Windows,
        entry_script: Some("../outside.json".into()),
        runtime_artifact: None,
        integrity: BundleIntegrity::None,
        output_layout_version: 1,
        hmac_key: None,
    })
    .expect_err("entry traversal must fail");

    let message = format!("{err}");
    assert!(message.contains("path traversal"));
}

#[test]
fn export_bundle_rejects_entry_script_symlink_escape() {
    let (tmp, project_root) = build_project_fixture();
    let escaped = tmp.path().join("escape.json");
    fs::write(
        &escaped,
        r#"{
  "script_schema_version": "1.0",
  "events": [],
  "labels": {}
}"#,
    )
    .expect("write escaped script");

    let entry_script_path = project_root.join("main.json");
    if !create_escape_symlink(&entry_script_path, &escaped) {
        eprintln!("symlink creation not supported on this platform");
        return;
    }

    let out = project_root.join("dist");
    let err = export_bundle(ExportBundleSpec {
        project_root: project_root.clone(),
        output_root: out,
        target_platform: ExportTargetPlatform::Windows,
        entry_script: None,
        runtime_artifact: None,
        integrity: BundleIntegrity::None,
        output_layout_version: 1,
        hmac_key: None,
    })
    .expect_err("entry symlink escape must fail");

    assert!(format!("{err}").contains("escapes project root"));
}

#[test]
fn export_bundle_rejects_asset_symlink_escape() {
    let (tmp, project_root) = build_project_fixture();
    let escaped = tmp.path().join("escape.ogg");
    fs::write(&escaped, [9u8, 9, 9]).expect("write escaped asset");
    let symlink_path = project_root.join("assets").join("bgm").join("theme.ogg");
    fs::remove_file(&symlink_path).expect("remove normal referenced asset");
    if !create_escape_symlink(&symlink_path, &escaped) {
        eprintln!("symlink creation not supported on this platform");
        return;
    }

    let out = project_root.join("dist");
    let err = export_bundle(ExportBundleSpec {
        project_root: project_root.clone(),
        output_root: out,
        target_platform: ExportTargetPlatform::Windows,
        entry_script: None,
        runtime_artifact: None,
        integrity: BundleIntegrity::None,
        output_layout_version: 1,
        hmac_key: None,
    })
    .expect_err("asset symlink escape must fail");

    assert!(format!("{err}").contains("escapes project root"));
}

#[test]
fn export_bundle_rejects_runtime_artifact_symlink_escape() {
    let (tmp, project_root) = build_project_fixture();
    let escaped = tmp.path().join("escape-runtime.bin");
    fs::write(&escaped, b"runtime").expect("write escaped runtime");
    let runtime_dir = project_root.join("runtime");
    fs::create_dir_all(&runtime_dir).expect("mkdir runtime");
    let runtime_path = runtime_dir.join("engine.bin");
    if !create_escape_symlink(&runtime_path, &escaped) {
        eprintln!("symlink creation not supported on this platform");
        return;
    }

    let out = project_root.join("dist");
    let err = export_bundle(ExportBundleSpec {
        project_root: project_root.clone(),
        output_root: out,
        target_platform: ExportTargetPlatform::Windows,
        entry_script: None,
        runtime_artifact: Some(PathBuf::from("runtime/engine.bin")),
        integrity: BundleIntegrity::None,
        output_layout_version: 1,
        hmac_key: None,
    })
    .expect_err("runtime artifact symlink escape must fail");

    assert!(format!("{err}").contains("escapes project root"));
}
