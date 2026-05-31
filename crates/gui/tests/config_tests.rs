use std::fs;

use tempfile::tempdir;
use visual_novel_engine::{runtime::EngineState, SaveData};
use visual_novel_gui::{load_state_from, save_state_to, DisplayInfo, UserPreferences, VnConfig};

fn create_file_symlink(link: &std::path::Path, target: &std::path::Path) -> bool {
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
fn resolves_defaults_for_small_display() {
    let config = VnConfig::default();
    let display = DisplayInfo {
        width: 1024.0,
        height: 600.0,
        scale_factor: 1.5,
    };

    let resolved = config.resolve(Some(display));

    assert!(
        resolved.fullscreen,
        "small displays should default to fullscreen"
    );
    assert_eq!(resolved.width, 1024.0);
    assert_eq!(resolved.height, 600.0);
    assert!(resolved.ui_scale > 1.0);
    assert!(resolved.scale_factor >= 1.5);
}

#[test]
fn saves_and_loads_preferences() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("prefs.json");
    let prefs = UserPreferences {
        fullscreen: true,
        ui_scale: 1.4,
        vsync: false,
        master_volume: 0.6,
        bgm_volume: 0.7,
        sfx_volume: 0.8,
        voice_volume: 0.9,
        audio_muted: true,
        advance_on_text_panel_click: Some(false),
    };

    prefs.save_to(&path).expect("save prefs");
    let loaded = UserPreferences::load_from(&path).expect("load prefs");

    assert_eq!(prefs, loaded);
    let stored = fs::read_to_string(&path).expect("read prefs");
    assert!(stored.contains("\"fullscreen\": true"));
}

#[test]
fn loads_legacy_preferences_with_audio_defaults() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("prefs.json");
    fs::write(
        &path,
        r#"{"fullscreen":true,"ui_scale":1.25,"vsync":false}"#,
    )
    .expect("write legacy prefs");

    let loaded = UserPreferences::load_from(&path).expect("load prefs");

    assert!(loaded.fullscreen);
    assert_eq!(loaded.ui_scale, 1.25);
    assert!(!loaded.vsync);
    assert!(!loaded.audio_muted);
    assert_eq!(loaded.master_volume, 1.0);
    assert_eq!(loaded.bgm_volume, 1.0);
    assert_eq!(loaded.sfx_volume, 1.0);
    assert_eq!(loaded.voice_volume, 1.0);
    assert_eq!(loaded.advance_on_text_panel_click, None);
}

#[test]
fn loading_preferences_reports_dangling_symlink_instead_of_defaults() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("prefs.json");
    let missing_target = dir.path().join("missing-target.json");
    if !create_file_symlink(&path, &missing_target) {
        eprintln!("file symlink creation not supported on this platform");
        return;
    }

    let err = UserPreferences::load_from(&path)
        .expect_err("dangling preference symlink must not silently load defaults");

    assert_eq!(err.kind(), std::io::ErrorKind::NotFound);
}

#[test]
fn saves_and_loads_state() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("state.vns");
    let mut state = EngineState::new(1, 1);
    state.position = 2;
    let data = SaveData {
        script_id: [7u8; 32],
        state,
    };

    save_state_to(&path, &data).expect("save state");
    let stored = fs::read(&path).expect("read stored state");
    assert!(
        SaveData::from_binary(&stored).is_err(),
        "GUI saves should use authenticated payloads"
    );
    let loaded = load_state_from(&path).expect("load state");

    assert_eq!(loaded.script_id, [7u8; 32]);
    assert_eq!(loaded.state.position, 2);
}

#[test]
fn rejects_legacy_plain_state_files() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("legacy_state.vns");
    let mut state = EngineState::new(4, 1);
    state.position = 7;
    let data = SaveData {
        script_id: [9u8; 32],
        state,
    };

    fs::write(&path, data.to_binary().expect("plain save")).expect("write plain state");
    let err = load_state_from(&path).expect_err("legacy plain state must be rejected");
    let message = err.to_string();
    assert!(
        message.contains("invalid save file magic bytes"),
        "unexpected error: {err}"
    );
}
