use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::process::Command;

use sha2::{Digest, Sha256};
use tempfile::TempDir;
use visual_novel_engine::{
    authoring::{AuthoringDocument, AuthoringPosition, NodeGraph, StoryNode},
    runtime::{AudioActionRaw, DialogueRaw, EventRaw, ScriptRaw},
    ProjectManifest,
};

fn build_project_fixture() -> (TempDir, std::path::PathBuf) {
    let tmp = TempDir::new().expect("temp dir");
    let root = tmp.path().join("project");
    fs::create_dir_all(root.join("assets/sfx")).expect("assets");

    let manifest = ProjectManifest::new("cli-fixture", "qa");
    manifest
        .save(&root.join("project.vnm"))
        .expect("manifest save");

    let script = ScriptRaw::new(
        vec![
            EventRaw::Dialogue(DialogueRaw {
                speaker: "Narrator".to_string(),
                text: "pack me".to_string(),
            }),
            EventRaw::AudioAction(AudioActionRaw {
                channel: "sfx".to_string(),
                action: "play".to_string(),
                asset: Some("assets/sfx/click.ogg".to_string()),
                volume: None,
                fade_duration_ms: None,
                loop_playback: None,
            }),
        ],
        BTreeMap::from([("start".to_string(), 0)]),
    );
    fs::write(
        root.join("main.json"),
        script.to_json().expect("script json"),
    )
    .expect("script");
    fs::write(root.join("assets/sfx/click.ogg"), [0u8, 1, 2, 3]).expect("asset");
    fs::write(root.join("assets/sfx/unused.ogg"), [4u8, 5, 6, 7]).expect("unused asset");
    (tmp, root)
}

fn minimal_pe_exe() -> Vec<u8> {
    let mut bytes = vec![0u8; 128];
    bytes[0..2].copy_from_slice(b"MZ");
    bytes[0x3c..0x40].copy_from_slice(&0x40u32.to_le_bytes());
    bytes[0x40..0x44].copy_from_slice(b"PE\0\0");
    bytes[0x44..0x46].copy_from_slice(&0x8664u16.to_le_bytes());
    bytes
}

fn minimal_linux_elf() -> Vec<u8> {
    let mut bytes = vec![0u8; 64];
    bytes[0..4].copy_from_slice(b"\x7fELF");
    bytes[4] = 2;
    bytes[5] = 1;
    bytes[16] = 2;
    bytes[17] = 0;
    bytes[18] = 0x3e;
    bytes[19] = 0;
    bytes
}

fn read_json(path: &Path) -> serde_json::Value {
    serde_json::from_str(&fs::read_to_string(path).expect("json file")).expect("json")
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>()
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
    assert!(output_root.join("scripts/compiled.vnc").is_file());
    assert!(output_root.join("scripts/compiled.vnscript.json").is_file());
    assert!(!output_root.join("scripts/main.vnc").exists());
    assert!(!output_root.join("scripts/main.json").exists());
    assert!(output_root.join("assets/sfx/click.ogg").is_file());
    assert!(!output_root.join("assets/sfx/unused.ogg").exists());
    assert!(output_root.join("meta/package_report.json").is_file());
    assert!(output_root.join("launch.bat").is_file());
}

#[test]
fn package_command_global_json_emits_envelope_only() {
    let (_tmp, project_root) = build_project_fixture();
    let output_root = project_root.join("dist_json");

    let output = Command::new(env!("CARGO_BIN_EXE_vnengine"))
        .arg("--json")
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
    assert!(
        output.stderr.is_empty(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let envelope: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("json envelope");
    assert_eq!(envelope["ok"], serde_json::json!(true));
    assert_eq!(envelope["code"], serde_json::json!("ok"));
    assert_eq!(
        envelope["data"]["schema"],
        serde_json::json!("vnengine.export_bundle_report.v1")
    );
    assert_eq!(envelope["data"]["assets_copied"], serde_json::json!(1));
    assert_eq!(
        envelope["data"]["compat_report"],
        serde_json::json!("meta/compat_report.json")
    );
    assert!(envelope["data"]["diagnostics"]
        .as_array()
        .expect("diagnostics array")
        .iter()
        .any(
            |diagnostic| diagnostic["code"] == "export.runtime_artifact.missing"
                && diagnostic["trace_id"]
                    .as_str()
                    .is_some_and(|trace_id| trace_id.starts_with("export-"))
        ));
    assert!(!String::from_utf8_lossy(&output.stdout).contains("packaged project =>"));
}

#[test]
fn package_command_json_execute_matches_written_report_manifest_and_compat_flow() {
    let (_tmp, project_root) = build_project_fixture();
    let runtime_dir = project_root.join("runtime");
    fs::create_dir_all(&runtime_dir).expect("runtime dir");
    let runtime_bytes = minimal_pe_exe();
    fs::write(runtime_dir.join("vn-runtime.exe"), &runtime_bytes).expect("runtime exe");
    let output_root = project_root.join("dist_json_execute_flow");

    let output = Command::new(env!("CARGO_BIN_EXE_vnengine"))
        .arg("--json")
        .arg("package")
        .arg(project_root.as_os_str())
        .arg("--output")
        .arg(output_root.as_os_str())
        .arg("--target")
        .arg("windows")
        .arg("--runtime-artifact")
        .arg("runtime/vn-runtime.exe")
        .arg("--require-executable")
        .arg("--integrity")
        .arg("hmac-sha256")
        .arg("--hmac-key")
        .arg("cli-flow-secret")
        .arg("--execute")
        .output()
        .expect("run package command");

    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output.stderr.is_empty(),
        "stderr should stay empty in --json mode: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.contains("packaged project =>"));
    assert!(!stdout.contains("bundle_hmac_sha256="));
    let envelope: serde_json::Value = serde_json::from_slice(&output.stdout).expect("envelope");
    assert_eq!(envelope["ok"], true);
    assert_eq!(envelope["code"], "ok");

    let package_report = read_json(&output_root.join("meta/package_report.json"));
    let compat_report = read_json(&output_root.join("meta/compat_report.json"));
    let manifest_text =
        fs::read_to_string(output_root.join("meta/bundle_file_manifest.json")).expect("manifest");
    let file_manifest: serde_json::Value =
        serde_json::from_str(&manifest_text).expect("manifest json");
    let signature =
        fs::read_to_string(output_root.join("meta/bundle.hmac_sha256")).expect("signature");

    assert_eq!(
        envelope["data"], package_report,
        "CLI JSON payload must be the same report written to disk"
    );
    assert_eq!(package_report["target_platform"], "windows");
    assert_eq!(package_report["runtime_artifact"], "runtime/vn-runtime.exe");
    assert_eq!(
        package_report["runtime_artifact_sha256"],
        sha256_hex(&runtime_bytes)
    );
    assert_eq!(package_report["executable"], "game.exe");
    assert_eq!(package_report["bundle_hmac_sha256"], signature);
    assert_eq!(
        compat_report["target_platform"],
        package_report["target_platform"]
    );
    assert_eq!(
        compat_report["runtime_artifact"],
        package_report["runtime_artifact"]
    );
    assert_eq!(
        compat_report["runtime_artifact_sha256"],
        package_report["runtime_artifact_sha256"]
    );
    assert_eq!(
        compat_report["bundle_hmac_sha256"],
        package_report["bundle_hmac_sha256"]
    );
    assert_eq!(
        compat_report["bundle_file_manifest_sha256"],
        sha256_hex(manifest_text.as_bytes())
    );
    assert_eq!(compat_report["diagnostics"], package_report["diagnostics"]);

    let manifest_files = file_manifest["files"].as_array().expect("manifest files");
    assert_eq!(compat_report["hashes"], file_manifest["files"]);
    assert!(manifest_files
        .iter()
        .any(|entry| entry["path"] == "game.exe"));
    assert!(manifest_files
        .iter()
        .any(|entry| entry["path"] == "runtime/vn-runtime.exe"));
    assert!(manifest_files.iter().any(|entry| {
        entry["path"] == package_report["runtime_artifact"]
            && entry["sha256"] == package_report["runtime_artifact_sha256"]
    }));
    assert!(manifest_files
        .iter()
        .any(|entry| entry["path"] == "assets/sfx/click.ogg"));
    assert!(!manifest_files
        .iter()
        .any(|entry| entry["path"] == "assets/sfx/unused.ogg"));
    assert_eq!(
        fs::read(output_root.join("game.exe")).expect("game exe"),
        runtime_bytes
    );
    let mut total_size = 0u64;
    for entry in manifest_files {
        let rel = entry["path"].as_str().expect("manifest path");
        let bytes = fs::read(output_root.join(rel)).unwrap_or_else(|err| {
            panic!("manifest entry '{rel}' should exist and be readable: {err}")
        });
        total_size += bytes.len() as u64;
        assert_eq!(entry["size"], bytes.len() as u64, "size mismatch for {rel}");
        assert_eq!(
            entry["sha256"],
            sha256_hex(&bytes),
            "hash mismatch for {rel}"
        );
    }
    assert_eq!(compat_report["total_size"], total_size);
}

#[test]
fn package_command_plan_dry_run_does_not_write_bundle() {
    let (_tmp, project_root) = build_project_fixture();
    let output_root = project_root.join("dist_plan");

    let output = Command::new(env!("CARGO_BIN_EXE_vnengine"))
        .arg("--json")
        .arg("package")
        .arg(project_root.as_os_str())
        .arg("--output")
        .arg(output_root.as_os_str())
        .arg("--target")
        .arg("windows")
        .arg("--dry-run")
        .output()
        .expect("run package plan command");

    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let envelope: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("json envelope");
    assert_eq!(envelope["ok"], serde_json::json!(true));
    assert_eq!(
        envelope["data"]["schema"],
        serde_json::json!("vnengine.export_plan.v1")
    );
    assert!(envelope["data"]["diagnostics"]
        .as_array()
        .expect("diagnostics array")
        .iter()
        .any(|diagnostic| diagnostic["message"]
            .as_str()
            .is_some_and(|message| message.contains("game.exe"))));
    assert!(
        !output_root.exists(),
        "dry-run package must not materialize bundle"
    );
}

#[test]
fn package_command_require_executable_rejects_missing_runtime() {
    let (_tmp, project_root) = build_project_fixture();
    let output_root = project_root.join("dist_requires_exe");

    let output = Command::new(env!("CARGO_BIN_EXE_vnengine"))
        .arg("package")
        .arg(project_root.as_os_str())
        .arg("--output")
        .arg(output_root.as_os_str())
        .arg("--target")
        .arg("windows")
        .arg("--require-executable")
        .output()
        .expect("run package command");

    assert!(!output.status.success(), "missing executable should fail");
    assert!(
        String::from_utf8_lossy(&output.stderr).contains(".exe runtime_artifact"),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("trace_id=export-"),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("export.runtime_artifact.missing"),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn package_command_require_executable_creates_game_exe() {
    let (_tmp, project_root) = build_project_fixture();
    let runtime_dir = project_root.join("runtime");
    fs::create_dir_all(&runtime_dir).expect("runtime dir");
    let runtime_bytes = minimal_pe_exe();
    fs::write(runtime_dir.join("vn-runtime.exe"), &runtime_bytes).expect("runtime exe");
    let output_root = project_root.join("dist_exe");

    let output = Command::new(env!("CARGO_BIN_EXE_vnengine"))
        .arg("package")
        .arg(project_root.as_os_str())
        .arg("--output")
        .arg(output_root.as_os_str())
        .arg("--target")
        .arg("windows")
        .arg("--runtime-artifact")
        .arg("runtime/vn-runtime.exe")
        .arg("--require-executable")
        .output()
        .expect("run package command");

    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        fs::read(output_root.join("game.exe")).expect("game exe"),
        runtime_bytes
    );
    let launcher = fs::read_to_string(output_root.join("launch.bat")).expect("launcher");
    assert!(launcher.contains("scripts\\compiled.vnscript.json"));
    assert!(launcher.contains("--assets-root \"%~dp0.\""));
    assert!(launcher.contains("--require-manifest"));
    assert!(String::from_utf8_lossy(&output.stdout).contains("executable=game.exe"));
}

#[test]
fn package_command_require_executable_rejects_fake_exe_payload() {
    let (_tmp, project_root) = build_project_fixture();
    let runtime_dir = project_root.join("runtime");
    fs::create_dir_all(&runtime_dir).expect("runtime dir");
    fs::write(runtime_dir.join("vn-runtime.exe"), b"fake runtime exe").expect("runtime exe");
    let output_root = project_root.join("dist_fake_exe");

    let output = Command::new(env!("CARGO_BIN_EXE_vnengine"))
        .arg("package")
        .arg(project_root.as_os_str())
        .arg("--output")
        .arg(output_root.as_os_str())
        .arg("--target")
        .arg("windows")
        .arg("--runtime-artifact")
        .arg("runtime/vn-runtime.exe")
        .arg("--require-executable")
        .output()
        .expect("run package command");

    assert!(!output.status.success(), "fake executable should fail");
    assert!(
        String::from_utf8_lossy(&output.stderr).contains(".exe runtime_artifact"),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn package_command_require_executable_creates_linux_game_launcher() {
    let (_tmp, project_root) = build_project_fixture();
    let runtime_dir = project_root.join("runtime");
    fs::create_dir_all(&runtime_dir).expect("runtime dir");
    let runtime_bytes = minimal_linux_elf();
    fs::write(runtime_dir.join("vn-runtime"), &runtime_bytes).expect("runtime");
    let output_root = project_root.join("dist_linux_exe");

    let output = Command::new(env!("CARGO_BIN_EXE_vnengine"))
        .arg("package")
        .arg(project_root.as_os_str())
        .arg("--output")
        .arg(output_root.as_os_str())
        .arg("--target")
        .arg("linux")
        .arg("--runtime-artifact")
        .arg("runtime/vn-runtime")
        .arg("--require-executable")
        .output()
        .expect("run package command");

    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        fs::read(output_root.join("game")).expect("game"),
        runtime_bytes
    );
    let launcher = fs::read_to_string(output_root.join("launch.sh")).expect("launcher");
    assert!(launcher.contains("exec \"$DIR/game\""));
    assert!(launcher.contains("--assets-root \"$DIR\""));
    assert!(launcher.contains("--require-manifest"));
    assert!(String::from_utf8_lossy(&output.stdout).contains("executable=game"));
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

#[test]
fn package_command_accepts_authoring_entry_script() {
    let (_tmp, project_root) = build_project_fixture();
    let mut manifest = ProjectManifest::new("cli-fixture", "qa");
    manifest.settings.entry_point = "main.vnauthoring".to_string();
    manifest
        .save(&project_root.join("project.vnm"))
        .expect("manifest save");

    let mut graph = NodeGraph::new();
    let start = graph.add_node(StoryNode::Start, AuthoringPosition::new(0.0, 0.0));
    let line = graph.add_node(
        StoryNode::Dialogue {
            speaker: "Narrator".to_string(),
            text: "pack authoring".to_string(),
        },
        AuthoringPosition::new(0.0, 90.0),
    );
    let end = graph.add_node(StoryNode::End, AuthoringPosition::new(0.0, 180.0));
    graph.connect(start, line);
    graph.connect(line, end);
    fs::write(
        project_root.join("main.vnauthoring"),
        AuthoringDocument::new(graph)
            .to_json()
            .expect("authoring json"),
    )
    .expect("authoring");

    let output_root = project_root.join("dist_authoring");
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
    assert!(output_root.join("scripts/compiled.vnc").is_file());
    assert!(output_root.join("scripts/compiled.vnscript.json").is_file());
    assert!(!output_root.join("scripts/main.vnauthoring").exists());
}
