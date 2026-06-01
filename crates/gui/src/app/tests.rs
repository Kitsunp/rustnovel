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
    VnApp {
        engine: minimal_engine(),
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
        script_id: [7u8; 32],
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
fn quickload_button_reports_save_store_probe_errors() {
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
}
