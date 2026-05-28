#[test]
fn character_drag_payload_keeps_name_and_path() {
    let payload = super::asset_drag_payload("char", "furina", "assets/characters/furina.png");
    assert_eq!(payload, "asset://char/furina\nassets/characters/furina.png");
}

#[test]
fn thumbnail_cache_keys_do_not_overlap_scene_cache() {
    let root = std::path::Path::new("C:/project-one");
    let asset_path = "assets/bg/room.png";
    let thumbnail = super::thumbnail_cache_key(root, asset_path);
    let stage = crate::editor::image_asset_cache::scene_stage_cache_key(
        root,
        crate::editor::PreviewQuality::Draft,
        asset_path,
    );
    assert!(thumbnail.starts_with("asset_browser::thumb::C:/project-one::assets/bg/room.png::"));
    assert_ne!(thumbnail, stage);
}

#[test]
fn asset_cards_shrink_for_narrow_window_panels() {
    let narrow = super::asset_card_size(52.0);
    let normal = super::asset_card_size(300.0);
    assert_eq!(narrow.x, 52.0);
    assert!(narrow.y < normal.y);
    assert_eq!(normal.x, 96.0);

    let rect = eframe::egui::Rect::from_min_size(eframe::egui::Pos2::ZERO, narrow);
    let image = super::asset_card_image_rect(rect);
    assert!(rect.contains_rect(image));
    assert!(super::asset_label_capacity(narrow.x) >= 4);
}

#[test]
fn asset_grid_wraps_many_backgrounds_vertically_in_narrow_panels() {
    let narrow_columns = super::asset_grid_columns(52.0);
    assert_eq!(narrow_columns, 1);
    assert_eq!(super::asset_grid_rows(6, narrow_columns), 6);

    let roomy_columns = super::asset_grid_columns(320.0);
    assert!(roomy_columns > narrow_columns);
    assert!(super::asset_grid_rows(roomy_columns * 2 + 1, roomy_columns) > 2);
}

#[test]
fn asset_grid_and_cards_tolerate_adverse_panel_widths() {
    for width in [0.0, -40.0, f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        let card = super::asset_card_size(width);
        assert!(card.x.is_finite());
        assert!(card.y.is_finite());
        assert!(card.x >= 48.0);
        assert!(super::asset_grid_columns(width) >= 1);
    }

    assert_eq!(super::asset_grid_rows(0, 0), 0);
    assert_eq!(super::asset_grid_rows(3, 0), 3);
}

#[test]
fn thumbnail_cache_keys_include_project_root() {
    let asset_path = "assets/bg/room.png";
    assert_ne!(
        super::thumbnail_cache_key(std::path::Path::new("C:/project-one"), asset_path),
        super::thumbnail_cache_key(std::path::Path::new("C:/project-two"), asset_path)
    );
}

#[test]
fn thumbnail_cache_can_dedupe_equivalent_resolved_asset_paths() {
    let temp = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir_all(temp.path().join("assets/bg")).expect("mkdir assets");
    std::fs::write(temp.path().join("assets/bg/room.png"), b"placeholder").expect("write asset");
    let store = vnengine_assets::AssetStore::new(
        temp.path().to_path_buf(),
        vnengine_assets::SecurityMode::Trusted,
        None,
        false,
    )
    .expect("asset store");

    let resolved_short = store
        .resolve_image_path("bg/room")
        .expect("short path should resolve");
    let resolved_full = store
        .resolve_image_path("assets/bg/room.png")
        .expect("full path should resolve");

    assert_eq!(resolved_short, resolved_full);
    assert_eq!(
        super::thumbnail_cache_key(temp.path(), &resolved_short),
        super::thumbnail_cache_key(temp.path(), &resolved_full)
    );
}

#[test]
fn audio_position_format_clamps_to_minutes_and_seconds() {
    assert_eq!(super::format_audio_position(-4.0, 61.4), "0:00 / 1:01");
    assert_eq!(super::format_audio_position(125.0, 3661.0), "2:05 / 61:01");
}

#[test]
fn audio_preview_offset_contract_is_integer_ms_and_finite() {
    assert_eq!(super::secs_to_ms(1.234), 1234);
    assert_eq!(super::secs_to_ms(-2.0), 0);
    assert_eq!(super::secs_to_ms(f32::NAN), 0);
    assert_eq!(super::secs_to_ms(f32::INFINITY), 0);

    let action = super::AssetBrowserAction::PreviewAudio {
        path: "audio/theme.ogg".to_string(),
        offset_ms: 1234,
    };
    assert_eq!(
        action,
        super::AssetBrowserAction::PreviewAudio {
            path: "audio/theme.ogg".to_string(),
            offset_ms: 1234
        }
    );
}

#[test]
fn asset_browser_audio_duration_uses_shared_resource_service_fingerprint_invalidation() {
    let temp = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir_all(temp.path().join("assets/audio")).expect("mkdir audio");
    let audio_path = temp.path().join("assets/audio/tone.wav");
    std::fs::write(
        &audio_path,
        tiny_wav(std::time::Duration::from_millis(120), 8_000),
    )
    .expect("write first wav");

    let manifest = visual_novel_engine::manifest::ProjectManifest::new("Test", "Author");
    let mut image_cache = std::collections::HashMap::new();
    let mut image_failures = std::collections::HashMap::new();
    let mut resource_service = crate::editor::resource_service::EditorResourceService::new();

    let first = {
        let mut panel = super::AssetBrowserPanel::new(
            &manifest,
            Some(temp.path()),
            &mut image_cache,
            &mut image_failures,
            &mut resource_service,
        );
        panel
            .audio_duration_secs("assets/audio/tone.wav")
            .expect("first duration")
    };

    std::fs::write(
        &audio_path,
        tiny_wav(std::time::Duration::from_millis(260), 8_000),
    )
    .expect("write second wav");

    let second = {
        let mut panel = super::AssetBrowserPanel::new(
            &manifest,
            Some(temp.path()),
            &mut image_cache,
            &mut image_failures,
            &mut resource_service,
        );
        panel
            .audio_duration_secs("assets/audio/tone.wav")
            .expect("second duration")
    };

    assert!(second > first);
    assert!(resource_service.metrics().evictions >= 1);
}

fn tiny_wav(duration: std::time::Duration, sample_rate: u32) -> Vec<u8> {
    let samples = (duration.as_secs_f32() * sample_rate as f32).round() as u32;
    let data_len = samples * 2;
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + data_len).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&sample_rate.to_le_bytes());
    bytes.extend_from_slice(&(sample_rate * 2).to_le_bytes());
    bytes.extend_from_slice(&2u16.to_le_bytes());
    bytes.extend_from_slice(&16u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&data_len.to_le_bytes());
    bytes.resize(bytes.len() + data_len as usize, 0);
    bytes
}
