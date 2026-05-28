use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use tempfile::TempDir;
use visual_novel_engine::{
    export_executable_bundle, export_windows_executable_bundle,
    runtime::{DialogueRaw, EventRaw, ScriptRaw},
    BundleIntegrity, ExportBundleSpec, ExportTargetPlatform, ProjectManifest,
};

fn build_project_fixture() -> (TempDir, std::path::PathBuf) {
    let tmp = TempDir::new().expect("temp dir");
    let root = tmp.path().join("project");
    fs::create_dir_all(&root).expect("project dir");

    ProjectManifest::new("exe-fixture", "qa")
        .save(&root.join("project.vnm"))
        .expect("manifest save");

    let script = ScriptRaw::new(
        vec![EventRaw::Dialogue(DialogueRaw {
            speaker: "Narrator".to_string(),
            text: "hello executable".to_string(),
        })],
        BTreeMap::from([("start".to_string(), 0)]),
    );
    fs::write(
        root.join("main.json"),
        script.to_json().expect("script json"),
    )
    .expect("script");

    (tmp, root)
}

fn windows_spec(
    project_root: std::path::PathBuf,
    output_root: std::path::PathBuf,
    runtime_artifact: Option<PathBuf>,
) -> ExportBundleSpec {
    ExportBundleSpec {
        project_root,
        output_root,
        target_platform: ExportTargetPlatform::Windows,
        entry_script: None,
        runtime_artifact,
        integrity: BundleIntegrity::None,
        output_layout_version: 1,
        hmac_key: None,
    }
}

fn linux_spec(
    project_root: std::path::PathBuf,
    output_root: std::path::PathBuf,
    runtime_artifact: Option<PathBuf>,
) -> ExportBundleSpec {
    ExportBundleSpec {
        project_root,
        output_root,
        target_platform: ExportTargetPlatform::Linux,
        entry_script: None,
        runtime_artifact,
        integrity: BundleIntegrity::None,
        output_layout_version: 1,
        hmac_key: None,
    }
}

fn macos_spec(
    project_root: std::path::PathBuf,
    output_root: std::path::PathBuf,
    runtime_artifact: Option<PathBuf>,
) -> ExportBundleSpec {
    ExportBundleSpec {
        project_root,
        output_root,
        target_platform: ExportTargetPlatform::Macos,
        entry_script: None,
        runtime_artifact,
        integrity: BundleIntegrity::None,
        output_layout_version: 1,
        hmac_key: None,
    }
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

fn minimal_macho() -> Vec<u8> {
    vec![0xFE, 0xED, 0xFA, 0xCF, 0, 0, 0, 0]
}

#[test]
fn windows_executable_bundle_rejects_missing_runtime_exe() {
    let (_tmp, project_root) = build_project_fixture();
    let err = export_windows_executable_bundle(windows_spec(
        project_root.clone(),
        project_root.join("dist_missing_runtime"),
        None,
    ))
    .expect_err("missing exe runtime must fail");

    assert!(
        err.to_string().contains(".exe runtime_artifact"),
        "unexpected error: {err}"
    );
}

#[test]
fn windows_executable_bundle_writes_top_level_game_exe_and_launcher() {
    let (_tmp, project_root) = build_project_fixture();
    let runtime_dir = project_root.join("runtime");
    fs::create_dir_all(&runtime_dir).expect("runtime dir");
    let runtime_bytes = minimal_pe_exe();
    fs::write(runtime_dir.join("vn-runtime.exe"), &runtime_bytes).expect("runtime exe");
    let output_root = project_root.join("dist_with_exe");

    let report = export_windows_executable_bundle(windows_spec(
        project_root,
        output_root.clone(),
        Some(PathBuf::from("runtime/vn-runtime.exe")),
    ))
    .expect("executable bundle");

    assert_eq!(report.executable.as_deref(), Some("game.exe"));
    assert_eq!(
        fs::read(output_root.join("game.exe")).expect("game exe"),
        runtime_bytes
    );
    let launcher = fs::read_to_string(output_root.join("launch.bat")).expect("launcher");
    assert!(launcher.contains("\"%~dp0game.exe\""));
    assert!(launcher.contains("\"%~dp0scripts\\compiled.vnscript.json\""));
    assert!(launcher.contains("--assets-root \"%~dp0.\""));
    assert!(launcher.contains("--manifest \"%~dp0meta\\assets_manifest.json\""));
    assert!(launcher.contains("--require-manifest"));
}

#[test]
fn windows_executable_bundle_rejects_fake_exe_payload() {
    let (_tmp, project_root) = build_project_fixture();
    let runtime_dir = project_root.join("runtime");
    fs::create_dir_all(&runtime_dir).expect("runtime dir");
    fs::write(runtime_dir.join("vn-runtime.exe"), b"fake runtime exe").expect("runtime exe");

    let err = export_windows_executable_bundle(windows_spec(
        project_root,
        runtime_dir.join("dist_invalid_exe"),
        Some(PathBuf::from("runtime/vn-runtime.exe")),
    ))
    .expect_err("fake exe payload must fail executable export");

    assert!(
        err.to_string().contains(".exe runtime_artifact"),
        "unexpected error: {err}"
    );
}

#[test]
fn windows_executable_bundle_rejects_mz_without_pe_signature() {
    let (_tmp, project_root) = build_project_fixture();
    let runtime_dir = project_root.join("runtime");
    fs::create_dir_all(&runtime_dir).expect("runtime dir");
    let mut mz_only = vec![0u8; 128];
    mz_only[0..2].copy_from_slice(b"MZ");
    fs::write(runtime_dir.join("vn-runtime.exe"), mz_only).expect("runtime exe");

    let err = export_windows_executable_bundle(windows_spec(
        project_root,
        runtime_dir.join("dist_mz_only"),
        Some(PathBuf::from("runtime/vn-runtime.exe")),
    ))
    .expect_err("mz-only payload must fail executable export");

    assert!(
        err.to_string().contains(".exe runtime_artifact"),
        "unexpected error: {err}"
    );
}

#[test]
fn linux_executable_bundle_writes_top_level_game_and_launcher() {
    let (_tmp, project_root) = build_project_fixture();
    let runtime_dir = project_root.join("runtime");
    fs::create_dir_all(&runtime_dir).expect("runtime dir");
    let runtime_bytes = minimal_linux_elf();
    fs::write(runtime_dir.join("vn-runtime"), &runtime_bytes).expect("runtime");
    let output_root = project_root.join("dist_linux_exe");

    let report = export_executable_bundle(linux_spec(
        project_root,
        output_root.clone(),
        Some(PathBuf::from("runtime/vn-runtime")),
    ))
    .expect("linux executable bundle");

    assert_eq!(report.executable.as_deref(), Some("game"));
    assert_eq!(
        fs::read(output_root.join("game")).expect("game"),
        runtime_bytes
    );
    let launcher = fs::read_to_string(output_root.join("launch.sh")).expect("launcher");
    assert!(launcher.contains("exec \"$DIR/game\""));
    assert!(launcher.contains("\"$DIR/scripts/compiled.vnscript.json\""));
    assert!(launcher.contains("--assets-root \"$DIR\""));
    assert!(launcher.contains("--manifest \"$DIR/meta/assets_manifest.json\""));
    assert!(launcher.contains("--require-manifest"));
}

#[test]
fn linux_executable_bundle_rejects_non_elf_runtime() {
    let (_tmp, project_root) = build_project_fixture();
    let runtime_dir = project_root.join("runtime");
    fs::create_dir_all(&runtime_dir).expect("runtime dir");
    fs::write(runtime_dir.join("vn-runtime"), b"fake linux runtime").expect("runtime");

    let err = export_executable_bundle(linux_spec(
        project_root,
        runtime_dir.join("dist_invalid_linux"),
        Some(PathBuf::from("runtime/vn-runtime")),
    ))
    .expect_err("fake linux runtime must fail executable export");

    assert!(
        err.to_string().contains("linux runtime_artifact"),
        "unexpected error: {err}"
    );
}

#[test]
fn macos_executable_bundle_accepts_mach_o_runtime() {
    let (_tmp, project_root) = build_project_fixture();
    let runtime_dir = project_root.join("runtime");
    fs::create_dir_all(&runtime_dir).expect("runtime dir");
    let runtime_bytes = minimal_macho();
    fs::write(runtime_dir.join("vn-runtime"), &runtime_bytes).expect("runtime");
    let output_root = project_root.join("dist_macos_exe");

    let report = export_executable_bundle(macos_spec(
        project_root,
        output_root.clone(),
        Some(PathBuf::from("runtime/vn-runtime")),
    ))
    .expect("macos executable bundle");

    assert_eq!(report.executable.as_deref(), Some("game"));
    assert_eq!(
        fs::read(output_root.join("game")).expect("game"),
        runtime_bytes
    );
}

#[test]
fn linux_executable_bundle_rejects_missing_runtime() {
    let (_tmp, project_root) = build_project_fixture();
    let err = export_executable_bundle(linux_spec(
        project_root.clone(),
        project_root.join("dist_linux_missing_runtime"),
        None,
    ))
    .expect_err("missing linux runtime must fail");

    assert!(
        err.to_string().contains("linux runtime_artifact"),
        "unexpected error: {err}"
    );
}
