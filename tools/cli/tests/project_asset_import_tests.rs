use std::{fs, process::Command};
use tempfile::tempdir;
use visual_novel_engine::manifest::*;

#[test]
fn cli_import_matches_core_and_reuses_existing_resource() {
    let temp = tempdir().unwrap();
    let source = temp.path().join("theme.ogg");
    fs::write(&source, b"audio fixture").unwrap();
    let mut reports = Vec::new();
    for client in ["core", "cli"] {
        let root = temp.path().join(client);
        fs::create_dir(&root).unwrap();
        let path = root.join("project.vnm");
        let mut manifest = ProjectManifest::new("Import", "Tests");
        manifest.save(&path).unwrap();
        let report = if client == "core" {
            serde_json::to_value(
                import_project_asset(
                    &root,
                    &path,
                    &mut manifest,
                    &source,
                    AssetImportKind::Audio,
                    DEFAULT_ASSET_IMPORT_LIMIT,
                )
                .unwrap(),
            )
            .unwrap()
        } else {
            let run = || {
                Command::new(env!("CARGO_BIN_EXE_vnengine"))
                    .args(["--json", "asset", "import"])
                    .arg(&path)
                    .arg(&source)
                    .args(["--kind", "audio"])
                    .output()
                    .unwrap()
            };
            let first = run();
            assert!(
                first.status.success(),
                "{}",
                String::from_utf8_lossy(&first.stderr)
            );
            let first: serde_json::Value = serde_json::from_slice(&first.stdout).unwrap();
            let saved = fs::read(&path).unwrap();
            let repeated = run();
            assert!(repeated.status.success());
            let repeated: serde_json::Value = serde_json::from_slice(&repeated.stdout).unwrap();
            assert_eq!(repeated["data"]["imported"], false);
            assert_eq!(fs::read(&path).unwrap(), saved);
            first["data"].clone()
        };
        assert_eq!(ProjectManifest::load(&path).unwrap().assets.audio.len(), 1);
        assert_eq!(
            fs::read(root.join(report["path"].as_str().unwrap())).unwrap(),
            b"audio fixture"
        );
        reports.push(report);
    }
    assert_eq!(reports[0], reports[1]);
}

#[test]
fn cli_invalid_import_returns_error_without_changing_project() {
    let temp = tempdir().unwrap();
    let source = temp.path().join("theme.ogg");
    fs::write(&source, b"too large").unwrap();
    let root = temp.path().join("project");
    fs::create_dir(&root).unwrap();
    let path = root.join("project.vnm");
    ProjectManifest::new("Limits", "Tests").save(&path).unwrap();
    let saved = fs::read(&path).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_vnengine"))
        .args(["--json", "asset", "import"])
        .arg(&path)
        .arg(&source)
        .args(["--kind", "audio", "--max-bytes", "1"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["ok"], false);
    assert_eq!(fs::read(&path).unwrap(), saved);
    assert!(!root.join("assets").exists());
}
