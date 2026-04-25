use std::fs;

use tempfile::tempdir;
use visual_novel_engine::{EngineState, SaveData};
use visual_novel_gui::{load_state_from, save_state_to, DisplayInfo, UserPreferences, VnConfig};

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
    };

    prefs.save_to(&path).expect("save prefs");
    let loaded = UserPreferences::load_from(&path).expect("load prefs");

    assert_eq!(prefs, loaded);
    let stored = fs::read_to_string(&path).expect("read prefs");
    assert!(stored.contains("\"fullscreen\": true"));
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
fn loads_legacy_plain_state_files() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("legacy_state.vns");
    let mut state = EngineState::new(4, 1);
    state.position = 7;
    let data = SaveData {
        script_id: [9u8; 32],
        state,
    };

    fs::write(&path, data.to_binary().expect("plain save")).expect("write plain state");
    let loaded = load_state_from(&path).expect("load legacy plain state");
    assert_eq!(loaded.script_id, [9u8; 32]);
    assert_eq!(loaded.state.position, 7);
}
