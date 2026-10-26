use std::{fs, process::Command};

use visual_novel_engine::{
    authoring::AuthoringDocumentCommand, export_bundle, import_renpy_project, BundleIntegrity,
    ExportBundleSpec, ExportTargetPlatform, ImportFallbackPolicy, ImportProfile,
    ImportRenpyOptions,
};
use visual_novel_gui::{
    editor::{AssetImportKind, StoryNode},
    EditorWorkbench, VnConfig,
};

#[test]
fn imported_project_can_be_edited_saved_exported_and_run_by_the_real_player() {
    if std::env::var_os("RUSTNOVEL_RELATIVE_PATH_PROBE").is_some() {
        let project = std::env::current_dir().unwrap();
        let loaded = visual_novel_gui::editor::project_io::load_project("project.vnm".into())
            .expect("project I/O accepts a manifest in the current directory");
        assert!(loaded.entry_point_script.is_some());
        let mut editor = EditorWorkbench::new(VnConfig::default());
        editor
            .load_project_with_status("project.vnm".into(), false)
            .unwrap();
        assert!(editor.project_root.as_ref().unwrap().is_absolute());
        editor.load_script("main.json".into());
        assert!(
            editor.manifest.is_none(),
            "standalone script must load successfully"
        );
        assert!(editor.project_root.as_ref().unwrap().is_absolute());
        assert!(editor.pending_save_path.as_ref().unwrap().is_absolute());
        // This process runs only this probe, so changing its cwd cannot race tests.
        std::env::set_current_dir(project.parent().unwrap()).unwrap();
        assert!(
            editor.run_dry_validation(),
            "{:?}",
            editor.validation_issues
        );
        return;
    }
    let tmp = tempfile::tempdir().unwrap();
    let source = tmp.path().join("Renpy source with spaces");
    let game = source.join("game");
    fs::create_dir_all(game.join("images")).unwrap();
    image::RgbaImage::from_pixel(32, 18, image::Rgba([30, 60, 90, 255]))
        .save(game.join("images/room.png"))
        .unwrap();
    image::RgbaImage::from_pixel(8, 16, image::Rgba([190, 90, 60, 255]))
        .save(game.join("images/ava.png"))
        .unwrap();
    // A short PCM WAV keeps the real project self-contained without media dependencies.
    let mut wave = Vec::from(&b"RIFF"[..]);
    wave.extend_from_slice(&196u32.to_le_bytes());
    wave.extend_from_slice(b"WAVEfmt ");
    wave.extend_from_slice(&16u32.to_le_bytes());
    wave.extend_from_slice(&1u16.to_le_bytes());
    wave.extend_from_slice(&1u16.to_le_bytes());
    wave.extend_from_slice(&8000u32.to_le_bytes());
    wave.extend_from_slice(&16000u32.to_le_bytes());
    wave.extend_from_slice(&2u16.to_le_bytes());
    wave.extend_from_slice(&16u16.to_le_bytes());
    wave.extend_from_slice(b"data");
    wave.extend_from_slice(&160u32.to_le_bytes());
    wave.extend_from_slice(&[0; 160]);
    fs::create_dir_all(game.join("audio")).unwrap();
    fs::write(game.join("audio/theme.wav"), wave).unwrap();
    fs::write(game.join("script.rpy"), "image bg room = \"images/room.png\"\nimage ava = \"images/ava.png\"\nlabel start:\n    scene bg room\n    show ava\n    play music \"audio/theme.wav\"\n    \"Before editing\"\n    menu:\n        \"Go outside\":\n            \"Outside\"\n        \"Stay\":\n            \"Inside\"\n    hide ava\n    stop music\n    \"The end\"\n").unwrap();
    let project = tmp.path().join("Imported project");
    let report = import_renpy_project(ImportRenpyOptions {
        project_root: source.clone(),
        output_root: project.clone(),
        entry_label: "start".into(),
        report_path: None,
        profile: ImportProfile::StoryFirst,
        include_tl: None,
        include_ui: None,
        include_patterns: vec![],
        exclude_patterns: vec![],
        strict_mode: true,
        fallback_policy: ImportFallbackPolicy::Strict,
    })
    .unwrap();
    assert_eq!(report.degraded_events, 0);
    let probe = Command::new(std::env::current_exe().unwrap())
        .arg("--exact")
        .arg(std::thread::current().name().unwrap())
        .env("RUSTNOVEL_RELATIVE_PATH_PROBE", "1")
        .current_dir(&project)
        .output()
        .unwrap();
    assert!(
        probe.status.success(),
        "relative-path probe: {} {}",
        String::from_utf8_lossy(&probe.stdout),
        String::from_utf8_lossy(&probe.stderr)
    );
    assert!(
        String::from_utf8_lossy(&probe.stdout).contains("1 passed"),
        "the child must execute the probe"
    );
    let mut editor = EditorWorkbench::new(VnConfig::default());
    editor
        .load_project_with_status(project.join("project.vnm"), false)
        .unwrap();
    assert_eq!(
        editor.manifest.as_ref().unwrap().assets.backgrounds.len(),
        1,
        "imported backgrounds must appear immediately in the asset browser"
    );
    assert!(
        editor.run_dry_validation(),
        "{:?}",
        editor.validation_issues
    );
    assert_eq!(editor.manifest.as_ref().unwrap().assets.characters.len(), 1);
    assert_eq!(editor.manifest.as_ref().unwrap().assets.audio.len(), 1);
    let dialogue = editor
        .node_graph
        .nodes()
        .find_map(|(id, node, _)| {
            matches!(node, StoryNode::Dialogue { ref text, .. } if text == "Before editing")
                .then_some(id)
        })
        .unwrap();
    editor.node_graph.edit_node(dialogue, |node| {
        if let StoryNode::Dialogue { text, .. } = node {
            *text = "Edited in the actual workbench".into();
        }
    });
    assert!(editor.apply_graph_undo());
    assert!(editor.apply_graph_redo());
    // Importing an already copied background reuses its existing path.
    let imported = editor
        .import_asset_file(&game.join("images/room.png"), AssetImportKind::Background)
        .unwrap();
    let again = editor
        .import_asset_file(&game.join("images/room.png"), AssetImportKind::Background)
        .unwrap();
    assert_eq!(imported, again);
    assert!(editor.prepare_player_mode());
    let local_report = tmp.path().join("relative-player-smoke.json");
    let local_player = Command::new(env!("CARGO_BIN_EXE_vn_player"))
        .current_dir(&project)
        .arg("main.json")
        .arg("--smoke-report")
        .arg(&local_report)
        .arg("--preferences-path")
        .arg(tmp.path().join("local-preferences.json"))
        .output()
        .unwrap();
    assert!(
        local_player.status.success(),
        "local player stdout={} stderr={}",
        String::from_utf8_lossy(&local_player.stdout),
        String::from_utf8_lossy(&local_player.stderr)
    );
    let local_smoke: serde_json::Value =
        serde_json::from_slice(&fs::read(local_report).unwrap()).unwrap();
    assert_eq!(local_smoke["status"], "passed", "{local_smoke}");
    editor.execute_save(&project.join("main.json"), "");
    assert!(!editor.node_graph.is_modified());
    let bundle = tmp.path().join("Exported game with spaces");
    export_bundle(ExportBundleSpec {
        project_root: project.clone(),
        output_root: bundle.clone(),
        target_platform: if cfg!(windows) {
            ExportTargetPlatform::Windows
        } else if cfg!(target_os = "macos") {
            ExportTargetPlatform::Macos
        } else {
            ExportTargetPlatform::Linux
        },
        entry_script: None,
        runtime_artifact: None,
        integrity: BundleIntegrity::None,
        output_layout_version: 1,
        hmac_key: None,
    })
    .unwrap();
    let exported_script = bundle.join("scripts/compiled.vnscript.json");
    assert!(fs::read_to_string(&exported_script)
        .unwrap()
        .contains("Edited in the actual workbench"));
    let smoke_report = bundle.join("meta/end_to_end_smoke.json");
    let result = Command::new(env!("CARGO_BIN_EXE_vn_player"))
        .arg(&exported_script)
        .arg("--assets-root")
        .arg(&bundle)
        .arg("--manifest")
        .arg(bundle.join("meta/assets_manifest.json"))
        .arg("--require-manifest")
        .arg("--smoke-report")
        .arg(&smoke_report)
        .arg("--preferences-path")
        .arg(tmp.path().join("qa-preferences.json"))
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "player stdout={} stderr={}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    let smoke: serde_json::Value =
        serde_json::from_slice(&fs::read(smoke_report).unwrap()).unwrap();
    assert_eq!(smoke["status"], "passed", "{smoke}");
    // Package and launch the actual executable too, from a different working
    // directory. This catches missing bundle paths that --help cannot detect.
    let native_bundle = tmp.path().join("Standalone executable");
    let packaged = visual_novel_engine::export_executable_bundle(ExportBundleSpec {
        project_root: project.clone(),
        output_root: native_bundle.clone(),
        target_platform: if cfg!(windows) {
            ExportTargetPlatform::Windows
        } else if cfg!(target_os = "macos") {
            ExportTargetPlatform::Macos
        } else {
            ExportTargetPlatform::Linux
        },
        entry_script: None,
        runtime_artifact: Some(env!("CARGO_BIN_EXE_vn_player").into()),
        integrity: BundleIntegrity::None,
        output_layout_version: 1,
        hmac_key: None,
    })
    .unwrap();
    let executable = native_bundle.join(packaged.executable.unwrap());
    let result = Command::new(executable)
        .current_dir(tmp.path())
        .arg("--smoke-report")
        .arg(native_bundle.join("meta/executable_smoke.json"))
        .arg("--preferences-path")
        .arg(tmp.path().join("executable-preferences.json"))
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "packaged executable stdout={} stderr={}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    let smoke: serde_json::Value = serde_json::from_slice(
        &fs::read(native_bundle.join("meta/executable_smoke.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(smoke["status"], "passed", "{smoke}");
    let mut reopened = EditorWorkbench::new(VnConfig::default());
    reopened
        .load_project_with_status(project.join("project.vnm"), false)
        .unwrap();
    assert!(reopened.node_graph.nodes().any(|(_, node, _)| matches!(node, StoryNode::Dialogue { ref text, .. } if text == "Edited in the actual workbench")));
    assert!(reopened.run_dry_validation());
    assert_eq!(reopened.node_graph.authoring_session.undo_delta_count(), 0);
    assert!(reopened
        .node_graph
        .authoring_session
        .apply(AuthoringDocumentCommand::UndoLast)
        .is_err());
    assert!(game.join("script.rpy").is_file());
}
