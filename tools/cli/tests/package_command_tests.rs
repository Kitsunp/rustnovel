use std::collections::BTreeMap;
use std::fs;
use std::process::Command;

use tempfile::TempDir;
use visual_novel_engine::{DialogueRaw, EventRaw, ProjectManifest, ScriptRaw};

fn build_project_fixture() -> (TempDir, std::path::PathBuf) {
    let tmp = TempDir::new().expect("temp dir");
    let root = tmp.path().join("project");
    fs::create_dir_all(root.join("assets/sfx")).expect("assets");

    let manifest = ProjectManifest::new("cli-fixture", "qa");
    manifest
        .save(&root.join("project.vnm"))
        .expect("manifest save");

    let script = ScriptRaw::new(
        vec![EventRaw::Dialogue(DialogueRaw {
            speaker: "Narrator".to_string(),
            text: "pack me".to_string(),
        })],
        BTreeMap::from([("start".to_string(), 0)]),
    );
    fs::write(
        root.join("main.json"),
        script.to_json().expect("script json"),
    )
    .expect("script");
    fs::write(root.join("assets/sfx/click.ogg"), [0u8, 1, 2, 3]).expect("asset");
    (tmp, root)
}

#[test]
fn package_command_creates_bundle_layout() {
    let (_tmp, project_root) = build_project_fixture();
    let output_root = project_root.join("dist");

    let output = Command::new(env!("CARGO_BIN_EXE_vnengine"))
        .arg("package")
        .arg(project_root.as_os_str())
        .arg("--output")
        .arg(output_root.as_os_str())
        .arg("--target")
        .arg("windows")
        .output()
        .expect("run package command");

    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output_root.join("scripts/main.vnc").is_file());
    assert!(output_root.join("scripts/main.json").is_file());
    assert!(output_root.join("assets/sfx/click.ogg").is_file());
    assert!(output_root.join("meta/package_report.json").is_file());
    assert!(output_root.join("launch.bat").is_file());
}

#[test]
fn package_command_hmac_mode_requires_key() {
    let (_tmp, project_root) = build_project_fixture();
    let output_root = project_root.join("dist_hmac");

    let output = Command::new(env!("CARGO_BIN_EXE_vnengine"))
        .arg("package")
        .arg(project_root.as_os_str())
        .arg("--output")
        .arg(output_root.as_os_str())
        .arg("--target")
        .arg("windows")
        .arg("--integrity")
        .arg("hmac-sha256")
        .output()
        .expect("run package command");

    assert!(
        !output.status.success(),
        "hmac mode without key should fail"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("requires hmac_key"), "stderr={stderr}");
}
