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

fn build_assets_fixture() -> (TempDir, std::path::PathBuf) {
    let tmp = TempDir::new().expect("temp dir");
    let root = tmp.path().join("assets");
    fs::create_dir_all(root.join("bg")).expect("assets");
    fs::write(root.join("bg").join("room.png"), [1u8, 2, 3]).expect("asset");
    (tmp, root)
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
