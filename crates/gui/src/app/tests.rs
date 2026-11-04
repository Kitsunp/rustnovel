use super::*;
use visual_novel_engine::runtime::ScriptRaw;

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
    let script_id = engine.script_id().unwrap();
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
fn player_paints_character_textures_with_core_scene_positions_and_scale() {
    use visual_novel_engine::runtime::{
        CharacterPlacementRaw, EventRaw, SceneUpdateRaw, SetCharacterPositionRaw,
    };
    let directory = tempfile::tempdir().unwrap();
    for (name, width, height) in [("background.png", 16, 9), ("character.png", 2, 4)] {
        image::RgbaImage::from_pixel(width, height, image::Rgba([80, 160, 220, 255]))
            .save(directory.path().join(name))
            .unwrap();
    }
    let mut app = test_app_with_save_root(directory.path().join("saves"));
    app.assets = AssetManager::new(
        AssetStore::new(
            directory.path().to_path_buf(),
            SecurityMode::Trusted,
            None,
            false,
        )
        .unwrap(),
        1024 * 1024,
    );
    let script = ScriptRaw::new(
        vec![
            EventRaw::Scene(SceneUpdateRaw {
                background: Some("background.png".to_string()),
                characters: vec![
                    CharacterPlacementRaw {
                        name: "Ava".to_string(),
                        expression: Some("character.png".to_string()),
                        x: Some(640),
                        y: Some(100),
                        ..Default::default()
                    },
                    CharacterPlacementRaw {
                        name: "Narrator".to_string(),
                        ..Default::default()
                    },
                ],
                ..Default::default()
            }),
            EventRaw::SetCharacterPosition(SetCharacterPositionRaw {
                name: "Ava".to_string(),
                x: 320,
                y: 180,
                scale: Some(0.5),
            }),
            EventRaw::SetCharacterPosition(SetCharacterPositionRaw {
                name: "Ava".to_string(),
                x: 900,
                y: 220,
                scale: None,
            }),
        ],
        std::collections::BTreeMap::from([("start".to_string(), 0)]),
    );
    app.engine = Engine::new(
        script,
        SecurityPolicy::default(),
        ResourceLimiter::default(),
    )
    .unwrap();
    let ctx = egui::Context::default();
    let background = app
        .assets
        .texture_for_asset(&ctx, "background.png")
        .unwrap()
        .unwrap()
        .id();
    let character = app
        .assets
        .texture_for_asset(&ctx, "character.png")
        .unwrap()
        .unwrap()
        .id();
    for (x, y, scale) in [
        (640.0, 100.0, 1.0),
        (320.0, 180.0, 0.5),
        (900.0, 220.0, 0.5),
    ] {
        let output = ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1400.0, 900.0),
                )),
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| app.render_player_stage(ui, ctx, 0.0));
            },
        );
        let bounds = |id| {
            output
                .shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    egui::Shape::Mesh(mesh) if mesh.texture_id == id => Some(mesh.calc_bounds()),
                    _ => None,
                })
                .unwrap_or_else(|| panic!("player did not paint texture {id:?}"))
        };
        let stage = bounds(background);
        let sprite = bounds(character);
        assert!((sprite.center().x - (stage.left() + x / 1280.0 * stage.width())).abs() < 0.1);
        assert!((sprite.top() - (stage.top() + y / 720.0 * stage.height())).abs() < 0.1);
        assert!((sprite.height() - 420.0 * scale / 720.0 * stage.height()).abs() < 0.1);
        assert!((sprite.width() / sprite.height() - 0.5).abs() < 0.001);
        assert!(app.last_error.is_none(), "{:?}", app.last_error);
        app.engine.step().unwrap();
    }
    let mut missing = app.engine.state().clone();
    missing.visual.characters[0].expression = Some("missing.png".into());
    app.engine.set_state(missing).unwrap();
    let _ = ctx.run(egui::RawInput::default(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| app.render_player_stage(ui, ctx, 0.0));
    });
    assert!(app
        .last_error
        .as_deref()
        .is_some_and(|message| message.contains("missing.png")));
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

    let save_dir = tempfile::tempdir().unwrap();
    let save_root = save_dir.path().to_path_buf();
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
    assert_eq!(
        app.last_status.as_deref(),
        Some("Quick loaded from backup. Primary save: missing")
    );

    std::fs::write(save_root.join("slots/quicksave.vnsav"), b"broken").unwrap();
    app.engine.step().unwrap();
    app.quickload();
    assert!(app.last_error.is_none());
    assert_eq!(app.engine.state().position, 0);
    assert_eq!(
        app.last_status.as_deref(),
        Some("Quick loaded from backup. Primary save: save data too small")
    );
}
