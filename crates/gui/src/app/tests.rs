use super::*;

use std::time::{SystemTime, UNIX_EPOCH};

fn unique_test_path(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("test clock should be after epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("vn_gui_{name}_{}_{}", std::process::id(), nanos))
}

fn minimal_engine() -> Engine {
    let script = ScriptRaw::from_json(
            r#"{"script_schema_version":"1.0","events":[{"type":"dialogue","speaker":"Narrator","text":"Test"}],"labels":{"start":0}}"#,
        )
        .expect("minimal test script should parse");
    Engine::new(
        script,
        SecurityPolicy::default(),
        ResourceLimiter::default(),
    )
    .expect("minimal test engine should initialize")
}

fn test_app_with_save_root(save_root: PathBuf) -> VnApp {
    let assets_root = unique_test_path("assets");
    std::fs::create_dir_all(&assets_root).expect("create test assets root");
    let prefs_path = unique_test_path("prefs").join("prefs.json");
    let config = VnConfig {
        assets_root: Some(assets_root.clone()),
        player_menu: PlayerMenuConfig {
            quick_actions: vec![visual_novel_engine::PlayerMenuActionConfig {
                action: PlayerMenuAction::QuickLoad,
                label: "Quick Load".to_string(),
                visible: true,
            }],
            ..PlayerMenuConfig::default()
        },
        ..VnConfig::default()
    }
    .resolve(None);
    let audio = PlayerAudioController::new(
        config.assets_root.clone(),
        config.security_mode,
        config.manifest_path.clone(),
        config.require_manifest,
    );
    let asset_store =
        AssetStore::new(assets_root, SecurityMode::Trusted, None, false).expect("asset store");
    let engine = minimal_engine();
    let script_id = visual_novel_engine::compute_script_id(&engine.script().to_binary().unwrap());
    VnApp {
        engine,
        config,
        prefs: UserPreferences::default(),
        prefs_path,
        show_menu: false,
        show_history: false,
        show_inspector: false,
        last_error: None,
        assets: AssetManager::new(asset_store, 1024 * 1024),
        audio,
        applied_scale: 1.0,
        label_jump_input: String::new(),
        script_id,
        last_status: None,
        save_store: SaveSlotStore::new(save_root),
        menu_tab: PlayerMenuTabKind::Saves,
    }
}

#[test]
fn corrupt_user_preferences_are_not_silently_defaulted() {
    let prefs_path = unique_test_path("corrupt_prefs").join("prefs.json");
    std::fs::create_dir_all(prefs_path.parent().expect("prefs parent"))
        .expect("create prefs parent");
    std::fs::write(&prefs_path, "{not-json").expect("write corrupt prefs");

    let err = load_user_preferences_for_run(&prefs_path)
        .expect_err("corrupt preferences must propagate instead of defaulting");

    assert!(err.to_string().contains("prefs.json"));
    assert!(err.to_string().contains("preferences"));

    let _ = std::fs::remove_file(prefs_path);
}

#[test]
fn save_load_reports_errors_and_restores_state() {
    let blocked_save_root = unique_test_path("blocked_save_root");
    std::fs::write(&blocked_save_root, b"not a directory")
        .expect("create file that blocks save root directory");
    let mut app = test_app_with_save_root(blocked_save_root.clone());
    let ctx = egui::Context::default();

    let _ = ctx.run(egui::RawInput::default(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            app.render_quick_action_buttons(ui);
        });
    });

    let error = app
        .last_error
        .as_deref()
        .expect("save store probe failure should be visible to the player");
    assert!(error.contains("Quick load availability check failed"));
    assert!(error.contains("save store io error"));

    let _ = std::fs::remove_file(blocked_save_root);

    let save_root = unique_test_path("load_paths");
    let mut app = test_app_with_save_root(save_root.clone());
    let save_path = save_root.join("manual.sav");
    app.save_state(&save_path);
    app.save_slot(1);
    assert!(app.last_error.is_none());
    for file in [false, true] {
        app.engine.step().unwrap();
        assert_eq!(app.engine.state().position, 1);
        if file {
            app.load_state(&save_path);
        } else {
            app.load_slot(1);
        }
        assert!(app.last_error.is_none(), "{:?}", app.last_error);
        assert_eq!(app.engine.state().position, 0);
        assert!(app.last_status.as_deref().unwrap().contains("Loaded"));
    }
    let mut mismatched =
        visual_novel_engine::SaveData::new(app.script_id, app.engine.state().clone());
    mismatched.script_id[0] ^= 1;
    crate::persist::save_state_to(&save_path, &mismatched).unwrap();
    app.save_store.save_slot(1, &mismatched).unwrap();
    let before_error = serde_json::to_value(app.engine.state()).unwrap();
    for file in [false, true] {
        if file {
            app.load_state(&save_path);
        } else {
            app.load_slot(1);
        }
        assert!(app
            .last_error
            .as_deref()
            .unwrap()
            .contains("does not match current script"));
        assert_eq!(
            serde_json::to_value(app.engine.state()).unwrap(),
            before_error
        );
    }
    app.save_slot(2);
    std::fs::write(save_root.join("meta/slot_001.json"), b"{broken").unwrap();
    let entries = app.list_save_slots_for_menu();
    assert_eq!(
        entries.len(),
        1,
        "corrupt metadata must not hide healthy slots"
    );
    assert_eq!(entries[0].metadata.slot_id, 2);
    assert!(app.last_error.as_deref().unwrap().contains("slot_001.json"));
    app.load_slot(2);
    assert!(app.last_error.is_none());
    app.quicksave();
    app.engine.step().unwrap();
    app.quicksave();
    std::fs::remove_file(save_root.join("slots/quicksave.vnsav")).unwrap();
    assert!(app.save_store.has_quicksave().unwrap());
    assert!(app
        .list_save_slots_for_menu()
        .iter()
        .any(|entry| entry.metadata.quick));
    app.quickload();
    assert!(app.last_error.is_none());
    assert_eq!(
        app.engine.state().position,
        0,
        "quickload restores the backup"
    );
    std::fs::remove_dir_all(save_root).unwrap();
}
