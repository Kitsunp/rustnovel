use std::fs;
use std::path::Path;
use std::process::Command;

use tempfile::TempDir;

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

fn create_escape_dir_symlink(link: &Path, target: &Path) -> bool {
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(target, link).is_ok()
    }

    #[cfg(windows)]
    {
        std::os::windows::fs::symlink_dir(target, link).is_ok()
    }

    #[cfg(not(any(unix, windows)))]
    {
        let _ = link;
        let _ = target;
        false
    }
}

fn build_assets_fixture() -> (TempDir, std::path::PathBuf) {
    let tmp = TempDir::new().expect("temp dir");
    let root = tmp.path().join("assets");
    fs::create_dir_all(root.join("bg")).expect("assets");
    fs::write(root.join("bg").join("room.png"), [1u8, 2, 3]).expect("asset");
    (tmp, root)
}

#[test]
fn manifest_command_excludes_existing_output_file_inside_assets_root() {
    let (_tmp, assets_root) = build_assets_fixture();
    let output_path = assets_root.join("assets_manifest.json");
    fs::write(&output_path, br#"{"old":true}"#).expect("preexisting manifest");

    let output = Command::new(env!("CARGO_BIN_EXE_vnengine"))
        .arg("manifest")
        .arg(assets_root.as_os_str())
        .arg("--output")
        .arg(output_path.as_os_str())
        .output()
        .expect("run manifest command");

    assert!(
        output.status.success(),
        "manifest should succeed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let manifest: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&output_path).expect("manifest json"))
            .expect("parse manifest");
    let assets = manifest["assets"].as_object().expect("assets object");
    assert!(
        assets.contains_key("bg/room.png"),
        "real asset should remain in manifest: {manifest}"
    );
    assert!(
        !assets.contains_key("assets_manifest.json"),
        "manifest output must not include a stale copy of itself: {manifest}"
    );
}

#[test]
fn manifest_command_rejects_output_symlink_without_replacing_target() {
    let (tmp, assets_root) = build_assets_fixture();
    let outside_output = tmp.path().join("outside_manifest.json");
    fs::write(&outside_output, b"sentinel").expect("outside output");
    let output_path = tmp.path().join("linked_manifest.json");
    if !create_escape_symlink(&output_path, &outside_output) {
        eprintln!("symlink creation not supported on this platform");
        return;
    }

    let output = Command::new(env!("CARGO_BIN_EXE_vnengine"))
        .arg("manifest")
        .arg(assets_root.as_os_str())
        .arg("--output")
        .arg(output_path.as_os_str())
        .output()
        .expect("run manifest command");

    assert!(
        !output.status.success(),
        "symlink output should fail: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("output path is not a regular file"),
        "stderr={stderr}"
    );
    assert_eq!(
        fs::read(&outside_output).expect("outside output untouched"),
        b"sentinel",
        "manifest writer must not follow or replace the symlink target"
    );
    assert!(
        fs::symlink_metadata(&output_path)
            .expect("output link still exists")
            .file_type()
            .is_symlink(),
        "failed writes must leave the output symlink in place"
    );
}

#[test]
fn manifest_command_rejects_asset_symlink_escape() {
    let (tmp, assets_root) = build_assets_fixture();
    let escaped = tmp.path().join("escape.png");
    fs::write(&escaped, [9u8, 9, 9]).expect("write escaped asset");
    let symlink_path = assets_root.join("bg").join("escape.png");
    if !create_escape_symlink(&symlink_path, &escaped) {
        eprintln!("symlink creation not supported on this platform");
        return;
    }

    let output_path = tmp.path().join("assets_manifest.json");
    let output = Command::new(env!("CARGO_BIN_EXE_vnengine"))
        .arg("manifest")
        .arg(assets_root.as_os_str())
        .arg("--output")
        .arg(output_path.as_os_str())
        .output()
        .expect("run manifest command");

    assert!(
        !output.status.success(),
        "symlink escape should fail: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("escapes root"), "stderr={stderr}");
}

#[test]
fn manifest_command_rejects_directory_symlink_escape() {
    let (tmp, assets_root) = build_assets_fixture();
    let escaped_dir = tmp.path().join("outside_assets");
    fs::create_dir_all(&escaped_dir).expect("outside dir");
    fs::write(escaped_dir.join("secret.png"), [9u8, 9, 9]).expect("outside asset");
    let symlink_path = assets_root.join("bg").join("external_pack");
    if !create_escape_dir_symlink(&symlink_path, &escaped_dir) {
        eprintln!("directory symlink creation not supported on this platform");
        return;
    }

    let output_path = tmp.path().join("assets_manifest.json");
    let output = Command::new(env!("CARGO_BIN_EXE_vnengine"))
        .arg("manifest")
        .arg(assets_root.as_os_str())
        .arg("--output")
        .arg(output_path.as_os_str())
        .output()
        .expect("run manifest command");

    assert!(
        !output.status.success(),
        "directory symlink escape should fail: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("escapes root"), "stderr={stderr}");
}
