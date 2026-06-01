use std::path::Path;

use super::*;

fn create_file_symlink(link: &Path, target: &Path) -> bool {
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

#[test]
fn resolve_existing_asset_path_accepts_regular_project_file(
) -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    std::fs::create_dir_all(dir.path().join("assets/audio"))?;
    std::fs::write(dir.path().join("assets/audio/theme.ogg"), b"audio")?;

    assert_eq!(
        resolve_existing_asset_path(dir.path(), "audio/theme", &["ogg"]),
        Some("assets/audio/theme.ogg".to_string())
    );
    Ok(())
}

#[test]
fn resolve_existing_asset_path_rejects_symlink_escape() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let outside = tempfile::tempdir()?;
    std::fs::create_dir_all(dir.path().join("assets/audio"))?;
    let outside_audio = outside.path().join("escape.ogg");
    std::fs::write(&outside_audio, b"outside audio")?;
    let link = dir.path().join("assets/audio/escape.ogg");
    if !create_file_symlink(&link, &outside_audio) {
        eprintln!("file symlink creation not supported on this platform");
        return Ok(());
    }

    assert_eq!(
        resolve_existing_asset_path(dir.path(), "audio/escape", &["ogg"]),
        None
    );
    Ok(())
}
