use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use tempfile::TempDir;
use visual_novel_engine::{
    authoring::{AuthoringDocument, AuthoringPosition, NodeGraph, StoryNode},
    export_bundle, BundleIntegrity, DialogueRaw, EventRaw, ExportBundleSpec, ExportTargetPlatform,
    ProjectManifest, ScriptRaw,
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
        vec![EventRaw::Dialogue(DialogueRaw {
            speaker: "Narrator".to_string(),
            text: "hello".to_string(),
        })],
        BTreeMap::from([("start".to_string(), 0)]),
    );
    fs::write(
        root.join("main.json"),
        script.to_json().expect("script json"),
    )
    .expect("script");
    fs::write(root.join("assets/bgm/theme.ogg"), [1u8, 2, 3, 4]).expect("asset");

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
    assert_eq!(report.assets_copied, 1);
    assert!(Path::new(&out.join("scripts/main.vnc")).is_file());
    assert!(Path::new(&out.join("scripts/main.json")).is_file());
    assert!(Path::new(&out.join("assets/bgm/theme.ogg")).is_file());
    assert!(Path::new(&out.join("meta/assets_manifest.json")).is_file());
    assert!(Path::new(&out.join("meta/package_report.json")).is_file());
    assert!(Path::new(&out.join("launch.bat")).is_file());

    let manifest_raw =
        fs::read_to_string(out.join("meta/assets_manifest.json")).expect("assets manifest");
    let manifest: serde_json::Value =
        serde_json::from_str(&manifest_raw).expect("assets manifest json");
    assert!(manifest.get("bgm/theme.ogg").is_some());
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

    assert_eq!(report.script_source, "scripts/main.vnauthoring");
    assert!(Path::new(&out.join("scripts/main.vnc")).is_file());
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
    let symlink_path = project_root.join("assets").join("bgm").join("escape.ogg");
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

#[test]
fn export_bundle_hmac_integrity_writes_signature() {
    let (_tmp, project_root) = build_project_fixture();
    let out = project_root.join("dist");

    let report = export_bundle(ExportBundleSpec {
        project_root: project_root.clone(),
        output_root: out.clone(),
        target_platform: ExportTargetPlatform::Linux,
        entry_script: None,
        runtime_artifact: None,
        integrity: BundleIntegrity::HmacSha256,
        output_layout_version: 1,
        hmac_key: Some("top-secret".to_string()),
    })
    .expect("bundle export with hmac");

    assert_eq!(report.integrity, "hmac_sha256");
    let signature = report.bundle_hmac_sha256.expect("signature in report");
    assert!(!signature.is_empty());

    let signature_file =
        fs::read_to_string(out.join("meta/bundle.hmac_sha256")).expect("signature file");
    assert_eq!(signature, signature_file);
    assert!(Path::new(&out.join("launch.sh")).is_file());
}

#[test]
fn export_bundle_hmac_requires_key() {
    let (_tmp, project_root) = build_project_fixture();
    let out = project_root.join("dist");

    let err = export_bundle(ExportBundleSpec {
        project_root: project_root.clone(),
        output_root: out,
        target_platform: ExportTargetPlatform::Windows,
        entry_script: None,
        runtime_artifact: None,
        integrity: BundleIntegrity::HmacSha256,
        output_layout_version: 1,
        hmac_key: None,
    })
    .expect_err("missing key should fail");

    assert!(format!("{err}").contains("requires hmac_key"));
}
